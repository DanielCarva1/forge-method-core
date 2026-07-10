//! Canonical, bounded local loaders for the dormant trusted MCP executor.
//!
//! This module performs filesystem reads only. It has no network, model,
//! subprocess, or mutation capability, and successful loading still ends in a
//! dormant rejection when used through [`DormantTrustedMcpExecutor`].

use std::collections::BTreeSet;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use forge_core_authority::{
    ExecutionError, ExecutionExecutor, ExecutionRequest, ExecutionResult, VerifiedExecutionCall,
};
use forge_core_contracts::{
    AssuranceCaseDocument, CommandContractDocument, FieldEvidenceRegistry,
    OperationContractDocument, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, authority_snapshot_token, command_contract_token, effect_contract_token,
    execution_intent_digest, operation_contract_token, ClaimSnapshotObservation,
    ExecutionAdmissionRequest, GateSnapshotObservation, SnapshotCompleteness,
};
use forge_core_kernel::{
    LateExecutionSnapshot, LateExecutionSnapshotSource, LateSnapshotError,
    PreparedExecutionMaterial, RuntimeEffectPayloadKind, RuntimeOperationEffectPayload,
    TrustedCitationMaterial,
};
use forge_core_store::sha256_content_hash;
use forge_core_validate::risk_audit::{validate_risk_audit_rule_set, RiskAuditRuleSet};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{McpDeploymentActivationState, ValidatedMcpDeploymentPolicy};

pub const MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION: &str = "0.1";
pub const MAX_TRUSTED_CONTRACT_BYTES: u64 = 1024 * 1024;
pub const MAX_TRUSTED_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_TRUSTED_TOTAL_PAYLOAD_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_TRUSTED_SNAPSHOT_BYTES: u64 = 8 * 1024 * 1024;
const CURATED_EVIDENCE_REF: &str = "contracts/research/field-evidence-20260625.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedMcpLoaderLimits {
    pub max_contract_bytes: u64,
    pub max_payload_bytes: u64,
    pub max_total_payload_bytes: u64,
    pub max_snapshot_bytes: u64,
}

impl Default for TrustedMcpLoaderLimits {
    fn default() -> Self {
        Self {
            max_contract_bytes: MAX_TRUSTED_CONTRACT_BYTES,
            max_payload_bytes: MAX_TRUSTED_PAYLOAD_BYTES,
            max_total_payload_bytes: MAX_TRUSTED_TOTAL_PAYLOAD_BYTES,
            max_snapshot_bytes: MAX_TRUSTED_SNAPSHOT_BYTES,
        }
    }
}

impl TrustedMcpLoaderLimits {
    fn validate(self) -> Result<Self, TrustedMcpLoadError> {
        let values = [
            (
                "max_contract_bytes",
                self.max_contract_bytes,
                MAX_TRUSTED_CONTRACT_BYTES,
            ),
            (
                "max_payload_bytes",
                self.max_payload_bytes,
                MAX_TRUSTED_PAYLOAD_BYTES,
            ),
            (
                "max_total_payload_bytes",
                self.max_total_payload_bytes,
                MAX_TRUSTED_TOTAL_PAYLOAD_BYTES,
            ),
            (
                "max_snapshot_bytes",
                self.max_snapshot_bytes,
                MAX_TRUSTED_SNAPSHOT_BYTES,
            ),
        ];
        for (field, value, ceiling) in values {
            if value == 0 || value > ceiling {
                return Err(TrustedMcpLoadError::InvalidLimit {
                    field,
                    value,
                    ceiling,
                });
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfinedProjectReader {
    pub(crate) canonical_root: PathBuf,
}

impl ConfinedProjectReader {
    pub(crate) fn new(project_root: &Path) -> Result<Self, TrustedMcpLoadError> {
        let canonical_root =
            project_root
                .canonicalize()
                .map_err(|error| TrustedMcpLoadError::ProjectRoot {
                    path: project_root.to_path_buf(),
                    source: error.to_string(),
                })?;
        if !canonical_root.is_dir() {
            return Err(TrustedMcpLoadError::ProjectRoot {
                path: canonical_root,
                source: "canonical project root is not a directory".to_owned(),
            });
        }
        Ok(Self { canonical_root })
    }

    fn read(&self, reference: &Path, limit: u64) -> Result<LoadedFile, TrustedMcpLoadError> {
        validate_relative_reference(reference)?;
        let requested = self.canonical_root.join(reference);
        let canonical_before =
            requested
                .canonicalize()
                .map_err(|error| TrustedMcpLoadError::Unavailable {
                    reference: reference.to_path_buf(),
                    source: error.to_string(),
                })?;
        if !canonical_before.starts_with(&self.canonical_root) || !canonical_before.is_file() {
            return Err(TrustedMcpLoadError::EscapesProject {
                reference: reference.to_path_buf(),
            });
        }
        let mut file =
            File::open(&canonical_before).map_err(|error| TrustedMcpLoadError::Read {
                reference: reference.to_path_buf(),
                source: error.to_string(),
            })?;
        let metadata = file.metadata().map_err(|error| TrustedMcpLoadError::Read {
            reference: reference.to_path_buf(),
            source: error.to_string(),
        })?;
        if !metadata.is_file() || metadata.len() > limit {
            return Err(TrustedMcpLoadError::SizeLimit {
                reference: reference.to_path_buf(),
                observed: metadata.len(),
                limit,
            });
        }
        let mut content = Vec::new();
        file.by_ref()
            .take(limit.saturating_add(1))
            .read_to_end(&mut content)
            .map_err(|error| TrustedMcpLoadError::Read {
                reference: reference.to_path_buf(),
                source: error.to_string(),
            })?;
        let observed = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if observed > limit {
            return Err(TrustedMcpLoadError::SizeLimit {
                reference: reference.to_path_buf(),
                observed,
                limit,
            });
        }
        let canonical_after =
            requested
                .canonicalize()
                .map_err(|error| TrustedMcpLoadError::Unavailable {
                    reference: reference.to_path_buf(),
                    source: error.to_string(),
                })?;
        if canonical_before != canonical_after || !canonical_after.starts_with(&self.canonical_root)
        {
            return Err(TrustedMcpLoadError::PathChanged {
                reference: reference.to_path_buf(),
            });
        }
        Ok(LoadedFile {
            content_hash: sha256_content_hash(&content),
            content,
        })
    }

    pub(crate) fn parse_yaml<T: DeserializeOwned>(
        &self,
        reference: &Path,
        limit: u64,
    ) -> Result<T, TrustedMcpLoadError> {
        let loaded = self.read(reference, limit)?;
        let yaml =
            std::str::from_utf8(&loaded.content).map_err(|error| TrustedMcpLoadError::Parse {
                reference: reference.to_path_buf(),
                source: error.to_string(),
            })?;
        yaml_serde::from_str(yaml).map_err(|error| TrustedMcpLoadError::Parse {
            reference: reference.to_path_buf(),
            source: error.to_string(),
        })
    }
}

fn validate_relative_reference(reference: &Path) -> Result<(), TrustedMcpLoadError> {
    if reference.as_os_str().is_empty()
        || reference.is_absolute()
        || reference.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(TrustedMcpLoadError::InvalidReference(
            reference.to_path_buf(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedFile {
    content: Vec<u8>,
    content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpLocalExecutionSnapshotDocument {
    pub schema_version: String,
    pub execution_snapshot: McpLocalExecutionSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpLocalExecutionSnapshot {
    pub admission_request: ExecutionAdmissionRequest,
    pub assurance_case: AssuranceCaseDocument,
    pub claim_snapshot: ClaimSnapshotObservation,
    pub gate_snapshot: GateSnapshotObservation,
    pub current_state_version: u64,
    pub now_unix: i64,
}

#[derive(Debug, Clone)]
pub struct LocalMcpSnapshotSource {
    reader: ConfinedProjectReader,
    snapshot_ref: PathBuf,
    max_snapshot_bytes: u64,
}

impl LocalMcpSnapshotSource {
    /// Construct an operator-owned local snapshot source.
    ///
    /// # Errors
    ///
    /// Rejects an unavailable root, unsafe reference, or invalid byte limit.
    pub fn new(
        project_root: impl AsRef<Path>,
        snapshot_ref: impl Into<PathBuf>,
        max_snapshot_bytes: u64,
    ) -> Result<Self, TrustedMcpLoadError> {
        if max_snapshot_bytes == 0 || max_snapshot_bytes > MAX_TRUSTED_SNAPSHOT_BYTES {
            return Err(TrustedMcpLoadError::InvalidLimit {
                field: "max_snapshot_bytes",
                value: max_snapshot_bytes,
                ceiling: MAX_TRUSTED_SNAPSHOT_BYTES,
            });
        }
        let source = Self {
            reader: ConfinedProjectReader::new(project_root.as_ref())?,
            snapshot_ref: snapshot_ref.into(),
            max_snapshot_bytes,
        };
        validate_relative_reference(&source.snapshot_ref)?;
        Ok(source)
    }

    fn load_document(&self) -> Result<McpLocalExecutionSnapshotDocument, TrustedMcpLoadError> {
        let document: McpLocalExecutionSnapshotDocument = self
            .reader
            .parse_yaml(&self.snapshot_ref, self.max_snapshot_bytes)?;
        if document.schema_version != MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION {
            return Err(TrustedMcpLoadError::SnapshotSchemaVersion(
                document.schema_version,
            ));
        }
        Ok(document)
    }
}

impl LateExecutionSnapshotSource for LocalMcpSnapshotSource {
    fn capture(&self) -> Result<LateExecutionSnapshot, LateSnapshotError> {
        let document = self
            .load_document()
            .map_err(|error| LateSnapshotError::new(error.to_string()))?;
        let snapshot = document.execution_snapshot;
        Ok(LateExecutionSnapshot {
            assurance_case: snapshot.assurance_case,
            claim_snapshot: snapshot.claim_snapshot,
            gate_snapshot: snapshot.gate_snapshot,
            current_state_version: snapshot.current_state_version,
            now_unix: snapshot.now_unix,
        })
    }
}

pub struct LoadedMcpExecutionMaterial {
    call: VerifiedExecutionCall,
    admission_request: ExecutionAdmissionRequest,
    operation: OperationContractDocument,
    commands: Vec<CommandContractDocument>,
    effect: ToolEffectContractDocument,
    payloads: Vec<RuntimeOperationEffectPayload>,
    risk_audit_rules: Option<RiskAuditRuleSet>,
    citation_material: Option<TrustedCitationMaterial>,
    audit: LoadedMcpMaterialAudit,
}

impl fmt::Debug for LoadedMcpExecutionMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedMcpExecutionMaterial")
            .field("audit", &self.audit)
            .finish_non_exhaustive()
    }
}

impl LoadedMcpExecutionMaterial {
    #[must_use]
    pub const fn audit(&self) -> &LoadedMcpMaterialAudit {
        &self.audit
    }

    pub(crate) fn into_kernel_material(self) -> PreparedExecutionMaterial {
        PreparedExecutionMaterial::new_with_adapter_requirements(
            self.call,
            self.admission_request,
            self.operation,
            self.commands,
            self.effect,
            self.payloads,
            self.risk_audit_rules,
            self.citation_material,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoadedMcpMaterialAudit {
    pub operation_id: String,
    pub command_count: usize,
    pub effect_id: String,
    pub payload_count: usize,
    pub total_payload_bytes: u64,
    pub payload_hashes: Vec<String>,
    pub risk_audit_loaded: bool,
    pub citation_material_loaded: bool,
    pub claim_snapshot_revision: u64,
    pub gate_snapshot_revision: u64,
    pub state_version: u64,
}

#[derive(Debug, Clone)]
pub struct TrustedMcpMaterialLoader {
    policy: ValidatedMcpDeploymentPolicy,
    reader: ConfinedProjectReader,
    snapshot_source: LocalMcpSnapshotSource,
    limits: TrustedMcpLoaderLimits,
}

struct LoadedContracts {
    operation: OperationContractDocument,
    commands: Vec<CommandContractDocument>,
    effect_ref: PathBuf,
    effect: ToolEffectContractDocument,
}

impl TrustedMcpMaterialLoader {
    /// Bind trusted local loaders to a validated-but-dormant deployment policy.
    ///
    /// # Errors
    ///
    /// Rejects active read-only policy, unsafe root/snapshot configuration, or
    /// limits above the compiled ceilings.
    pub fn new(
        policy: ValidatedMcpDeploymentPolicy,
        project_root: impl AsRef<Path>,
        snapshot_ref: impl Into<PathBuf>,
        limits: TrustedMcpLoaderLimits,
    ) -> Result<Self, TrustedMcpLoadError> {
        let project_root = project_root.as_ref();
        Self::new_with_snapshot_root(policy, project_root, project_root, snapshot_ref, limits)
    }

    /// Bind project material and runtime snapshots to separate canonical
    /// roots. Sidecar deployments resolve the snapshot root from Project Link.
    ///
    /// # Errors
    ///
    /// Rejects active read-only policy, unsafe roots/snapshot configuration,
    /// or limits above the compiled ceilings.
    pub fn new_with_snapshot_root(
        policy: ValidatedMcpDeploymentPolicy,
        project_root: impl AsRef<Path>,
        snapshot_root: impl AsRef<Path>,
        snapshot_ref: impl Into<PathBuf>,
        limits: TrustedMcpLoaderLimits,
    ) -> Result<Self, TrustedMcpLoadError> {
        if policy.activation_state() != McpDeploymentActivationState::PolicyValidatedDormant {
            return Err(TrustedMcpLoadError::TrustedPolicyRequired);
        }
        let limits = limits.validate()?;
        let reader = ConfinedProjectReader::new(project_root.as_ref())?;
        let snapshot_source =
            LocalMcpSnapshotSource::new(snapshot_root, snapshot_ref, limits.max_snapshot_bytes)?;
        Ok(Self {
            policy,
            reader,
            snapshot_source,
            limits,
        })
    }

    #[must_use]
    pub const fn snapshot_source(&self) -> &LocalMcpSnapshotSource {
        &self.snapshot_source
    }

    #[must_use]
    pub(crate) fn project_root(&self) -> &Path {
        &self.reader.canonical_root
    }

    #[must_use]
    pub(crate) const fn policy(&self) -> &ValidatedMcpDeploymentPolicy {
        &self.policy
    }

    /// Load and cross-check one signed request without preparing or committing it.
    ///
    /// # Errors
    ///
    /// Fails closed on path escape, size/digest drift, YAML/schema errors,
    /// duplicate references, policy mismatch, or signed Admission mismatch.
    pub fn load(
        &self,
        call: VerifiedExecutionCall,
    ) -> Result<LoadedMcpExecutionMaterial, TrustedMcpLoadError> {
        let policy = &self.policy.document().mcp_deployment_policy;
        let required_audience = policy
            .required_audience
            .as_deref()
            .ok_or(TrustedMcpLoadError::TrustedPolicyRequired)?;
        if call.authorization().principal().audience() != required_audience {
            return Err(TrustedMcpLoadError::AudienceMismatch);
        }

        let request = call.request();
        let LoadedContracts {
            operation,
            commands,
            effect_ref,
            effect,
        } = self.load_contracts(request)?;
        let (payloads, total_payload_bytes) = self.load_payloads(request)?;
        let risk_audit_rules = self.load_risk_audit(request)?;
        let citation_material = self.load_citation_material(request)?;

        let snapshot_document = self.snapshot_source.load_document()?;
        validate_signed_material(
            &call,
            &snapshot_document.execution_snapshot,
            &operation,
            &commands,
            &effect_ref,
            &effect,
        )?;
        let snapshot = snapshot_document.execution_snapshot;
        let audit = LoadedMcpMaterialAudit {
            operation_id: operation.operation_contract.contract_id.0.clone(),
            command_count: commands.len(),
            effect_id: effect.tool_effect_contract.id.0.clone(),
            payload_count: payloads.len(),
            total_payload_bytes,
            payload_hashes: payloads
                .iter()
                .map(|payload| payload.content_hash.clone())
                .collect(),
            risk_audit_loaded: risk_audit_rules.is_some(),
            citation_material_loaded: citation_material.is_some(),
            claim_snapshot_revision: snapshot.claim_snapshot.revision,
            gate_snapshot_revision: snapshot.gate_snapshot.revision,
            state_version: snapshot.current_state_version,
        };
        Ok(LoadedMcpExecutionMaterial {
            call,
            admission_request: snapshot.admission_request,
            operation,
            commands,
            effect,
            payloads,
            risk_audit_rules,
            citation_material,
            audit,
        })
    }

    fn load_contracts(
        &self,
        request: &ExecutionRequest,
    ) -> Result<LoadedContracts, TrustedMcpLoadError> {
        let operation = self.reader.parse_yaml(
            request.operation_contract_ref(),
            self.limits.max_contract_bytes,
        )?;
        let mut commands = Vec::with_capacity(request.command_contract_refs().len());
        let mut command_refs = BTreeSet::new();
        for reference in request.command_contract_refs() {
            if !command_refs.insert(reference.clone()) {
                return Err(TrustedMcpLoadError::DuplicateReference(reference.clone()));
            }
            commands.push(
                self.reader
                    .parse_yaml(reference, self.limits.max_contract_bytes)?,
            );
        }
        let effect_ref = request
            .effect_contract_ref()
            .ok_or(TrustedMcpLoadError::SingleEffectRequired)?
            .to_path_buf();
        let effect: ToolEffectContractDocument = self
            .reader
            .parse_yaml(&effect_ref, self.limits.max_contract_bytes)?;
        Ok(LoadedContracts {
            operation,
            commands,
            effect_ref,
            effect,
        })
    }

    fn load_payloads(
        &self,
        request: &ExecutionRequest,
    ) -> Result<(Vec<RuntimeOperationEffectPayload>, u64), TrustedMcpLoadError> {
        let mut payloads = Vec::with_capacity(request.payloads().len());
        let mut payload_targets = BTreeSet::new();
        let mut total_payload_bytes = 0_u64;
        for binding in request.payloads() {
            if !payload_targets.insert(binding.target_ref().to_owned()) {
                return Err(TrustedMcpLoadError::DuplicatePayloadTarget(
                    binding.target_ref().to_owned(),
                ));
            }
            let expected = binding.expected_content_hash().ok_or_else(|| {
                TrustedMcpLoadError::PayloadDigestRequired {
                    target_ref: binding.target_ref().to_owned(),
                }
            })?;
            let loaded = self
                .reader
                .read(binding.path(), self.limits.max_payload_bytes)?;
            if loaded.content_hash != expected {
                return Err(TrustedMcpLoadError::PayloadDigestMismatch {
                    target_ref: binding.target_ref().to_owned(),
                    expected: expected.to_owned(),
                    observed: loaded.content_hash,
                });
            }
            let byte_len = u64::try_from(loaded.content.len()).unwrap_or(u64::MAX);
            total_payload_bytes = total_payload_bytes.saturating_add(byte_len);
            if total_payload_bytes > self.limits.max_total_payload_bytes {
                return Err(TrustedMcpLoadError::TotalPayloadSizeLimit {
                    observed: total_payload_bytes,
                    limit: self.limits.max_total_payload_bytes,
                });
            }
            payloads.push(RuntimeOperationEffectPayload {
                target_ref: binding.target_ref().to_owned(),
                payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
                content_hash: expected.to_owned(),
                content: loaded.content,
            });
        }
        Ok((payloads, total_payload_bytes))
    }

    fn load_risk_audit(
        &self,
        request: &ExecutionRequest,
    ) -> Result<Option<RiskAuditRuleSet>, TrustedMcpLoadError> {
        request
            .risk_audit_rules_ref()
            .map(|reference| {
                let rules: RiskAuditRuleSet = self
                    .reader
                    .parse_yaml(reference, self.limits.max_contract_bytes)?;
                let report = validate_risk_audit_rule_set(&rules);
                if report.has_errors() {
                    return Err(TrustedMcpLoadError::InvalidRiskAudit {
                        reference: reference.to_path_buf(),
                        errors: report.error_count(),
                    });
                }
                Ok(rules)
            })
            .transpose()
    }

    fn load_citation_material(
        &self,
        request: &ExecutionRequest,
    ) -> Result<Option<TrustedCitationMaterial>, TrustedMcpLoadError> {
        if !request.require_citation() {
            return Ok(None);
        }
        let evidence: FieldEvidenceRegistry = self.reader.parse_yaml(
            Path::new(CURATED_EVIDENCE_REF),
            self.limits.max_contract_bytes,
        )?;
        let projection = forge_core_research::project(&self.reader.canonical_root)
            .map_err(|error| TrustedMcpLoadError::CitationProjection(error.to_string()))?;
        let runtime_ids = projection.sources.keys().cloned().collect();
        Ok(Some(TrustedCitationMaterial::new(evidence, runtime_ids)))
    }
}

fn validate_signed_material(
    call: &VerifiedExecutionCall,
    snapshot: &McpLocalExecutionSnapshot,
    operation: &OperationContractDocument,
    commands: &[CommandContractDocument],
    effect_ref: &Path,
    effect: &ToolEffectContractDocument,
) -> Result<(), TrustedMcpLoadError> {
    let authorization = call.authorization();
    let principal = authorization.principal();
    let admission = &snapshot.admission_request;
    if admission.principal_id != *principal.principal_id()
        || admission.agent_id != *principal.agent_id()
        || admission.principal_role != principal.role()
        || admission.nonce != authorization.nonce()
        || admission.issued_at_unix != authorization.issued_at_unix()
    {
        return Err(TrustedMcpLoadError::AdmissionAuthorityMismatch);
    }
    let digest = execution_intent_digest(admission)
        .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
    if digest != authorization.execution_intent_digest() {
        return Err(TrustedMcpLoadError::AdmissionDigestMismatch);
    }
    let operation_token = operation_contract_token(operation)
        .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
    if admission.operation_id != operation.operation_contract.contract_id
        || admission.operation_token != operation_token
    {
        return Err(TrustedMcpLoadError::OperationBindingMismatch);
    }
    let assurance_token = assurance_case_token(&snapshot.assurance_case)
        .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
    if admission.assurance_case_id != snapshot.assurance_case.assurance_case.id
        || admission.assurance_case_token != assurance_token
    {
        return Err(TrustedMcpLoadError::AssuranceBindingMismatch);
    }
    validate_authority_snapshot_binding(admission, snapshot)?;
    if admission.command_bindings.len() != commands.len() {
        return Err(TrustedMcpLoadError::CommandBindingMismatch);
    }
    let distinct_command_bindings = admission
        .command_bindings
        .iter()
        .map(|binding| binding.reference.as_str())
        .collect::<BTreeSet<_>>();
    if distinct_command_bindings.len() != admission.command_bindings.len() {
        return Err(TrustedMcpLoadError::CommandBindingMismatch);
    }
    for command in commands {
        let token = command_contract_token(command)
            .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
        if !admission.command_bindings.iter().any(|binding| {
            binding.reference == command.command_contract.id.0 && binding.token == token
        }) {
            return Err(TrustedMcpLoadError::CommandBindingMismatch);
        }
    }
    let effect_token = effect_contract_token(effect)
        .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
    if admission.effect_bindings.len() != 1
        || admission.effect_bindings[0].reference != path_string(effect_ref)?
        || admission.effect_bindings[0].token != effect_token
    {
        return Err(TrustedMcpLoadError::EffectBindingMismatch);
    }
    if admission.expected_claim_snapshot_revision != snapshot.claim_snapshot.revision
        || admission.expected_gate_snapshot_revision != snapshot.gate_snapshot.revision
        || admission.expected_replay_reservation_revision != 1
        || snapshot.claim_snapshot.completeness != SnapshotCompleteness::Complete
        || snapshot.gate_snapshot.completeness != SnapshotCompleteness::Complete
    {
        return Err(TrustedMcpLoadError::SnapshotRevisionMismatch);
    }
    let expected_claims = admission
        .expected_claim_revisions
        .iter()
        .map(|item| (item.reference.as_str(), item.revision))
        .collect::<BTreeSet<_>>();
    let observed_claims = snapshot
        .claim_snapshot
        .claims
        .iter()
        .map(|item| (item.claim_ref.0.as_str(), item.revision))
        .collect::<BTreeSet<_>>();
    let expected_gates = admission
        .expected_gate_revisions
        .iter()
        .map(|item| (item.reference.as_str(), item.revision))
        .collect::<BTreeSet<_>>();
    let observed_gates = snapshot
        .gate_snapshot
        .gates
        .iter()
        .map(|item| (item.gate_ref.0.as_str(), item.revision))
        .collect::<BTreeSet<_>>();
    if expected_claims.len() != admission.expected_claim_revisions.len()
        || observed_claims.len() != snapshot.claim_snapshot.claims.len()
        || expected_gates.len() != admission.expected_gate_revisions.len()
        || observed_gates.len() != snapshot.gate_snapshot.gates.len()
        || expected_claims != observed_claims
        || expected_gates != observed_gates
    {
        return Err(TrustedMcpLoadError::SnapshotRevisionMismatch);
    }
    Ok(())
}

fn validate_authority_snapshot_binding(
    admission: &ExecutionAdmissionRequest,
    snapshot: &McpLocalExecutionSnapshot,
) -> Result<(), TrustedMcpLoadError> {
    let computed = authority_snapshot_token(
        &snapshot.claim_snapshot,
        &snapshot.gate_snapshot,
        snapshot.current_state_version,
        snapshot.now_unix,
    )
    .map_err(|error| TrustedMcpLoadError::Binding(error.to_string()))?;
    if admission.authority_snapshot_token != computed {
        return Err(TrustedMcpLoadError::AuthoritySnapshotBindingMismatch);
    }
    Ok(())
}

fn path_string(path: &Path) -> Result<String, TrustedMcpLoadError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| TrustedMcpLoadError::InvalidReference(path.to_path_buf()))
}

#[derive(Debug)]
pub struct DormantTrustedMcpExecutor {
    loader: TrustedMcpMaterialLoader,
}

impl DormantTrustedMcpExecutor {
    #[must_use]
    pub const fn new(loader: TrustedMcpMaterialLoader) -> Self {
        Self { loader }
    }
}

impl ExecutionExecutor for DormantTrustedMcpExecutor {
    fn execute(&self, call: VerifiedExecutionCall) -> Result<ExecutionResult, ExecutionError> {
        self.loader
            .load(call)
            .map_err(|error| ExecutionError::Rejected(error.to_string()))?;
        Err(ExecutionError::Rejected(
            "trusted MCP material validated but P4b.3b remains dormant".to_owned(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedMcpLoadError {
    TrustedPolicyRequired,
    InvalidLimit {
        field: &'static str,
        value: u64,
        ceiling: u64,
    },
    ProjectRoot {
        path: PathBuf,
        source: String,
    },
    InvalidReference(PathBuf),
    Unavailable {
        reference: PathBuf,
        source: String,
    },
    EscapesProject {
        reference: PathBuf,
    },
    Read {
        reference: PathBuf,
        source: String,
    },
    SizeLimit {
        reference: PathBuf,
        observed: u64,
        limit: u64,
    },
    TotalPayloadSizeLimit {
        observed: u64,
        limit: u64,
    },
    PathChanged {
        reference: PathBuf,
    },
    Parse {
        reference: PathBuf,
        source: String,
    },
    SnapshotSchemaVersion(String),
    AudienceMismatch,
    DuplicateReference(PathBuf),
    SingleEffectRequired,
    DuplicatePayloadTarget(String),
    PayloadDigestRequired {
        target_ref: String,
    },
    PayloadDigestMismatch {
        target_ref: String,
        expected: String,
        observed: String,
    },
    InvalidRiskAudit {
        reference: PathBuf,
        errors: usize,
    },
    CitationProjection(String),
    AdmissionAuthorityMismatch,
    AdmissionDigestMismatch,
    OperationBindingMismatch,
    AssuranceBindingMismatch,
    AuthoritySnapshotBindingMismatch,
    CommandBindingMismatch,
    EffectBindingMismatch,
    SnapshotRevisionMismatch,
    Binding(String),
}

impl fmt::Display for TrustedMcpLoadError {
    // Keeping the complete typed-error rendering in one exhaustive match makes
    // audit review easier than scattering security-boundary messages.
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrustedPolicyRequired => {
                formatter.write_str("validated dormant trusted policy required")
            }
            Self::InvalidLimit {
                field,
                value,
                ceiling,
            } => write!(formatter, "{field}={value} must be between 1 and {ceiling}"),
            Self::ProjectRoot { path, source } => write!(
                formatter,
                "project root {} unavailable: {source}",
                path.display()
            ),
            Self::InvalidReference(path) => write!(
                formatter,
                "unsafe project-relative reference {}",
                path.display()
            ),
            Self::Unavailable { reference, source } => write!(
                formatter,
                "reference {} unavailable: {source}",
                reference.display()
            ),
            Self::EscapesProject { reference } => write!(
                formatter,
                "reference {} escapes canonical project root",
                reference.display()
            ),
            Self::Read { reference, source } => {
                write!(formatter, "cannot read {}: {source}", reference.display())
            }
            Self::SizeLimit {
                reference,
                observed,
                limit,
            } => write!(
                formatter,
                "reference {} is {observed} bytes, limit {limit}",
                reference.display()
            ),
            Self::TotalPayloadSizeLimit { observed, limit } => {
                write!(formatter, "payload set is {observed} bytes, limit {limit}")
            }
            Self::PathChanged { reference } => write!(
                formatter,
                "reference {} changed during bounded read",
                reference.display()
            ),
            Self::Parse { reference, source } => write!(
                formatter,
                "cannot parse typed YAML {}: {source}",
                reference.display()
            ),
            Self::SnapshotSchemaVersion(found) => {
                write!(formatter, "unsupported local snapshot schema {found}")
            }
            Self::AudienceMismatch => {
                formatter.write_str("verified principal audience differs from deployment policy")
            }
            Self::DuplicateReference(path) => {
                write!(formatter, "duplicate contract reference {}", path.display())
            }
            Self::SingleEffectRequired => {
                formatter.write_str("exactly one effect contract reference is required")
            }
            Self::DuplicatePayloadTarget(target) => {
                write!(formatter, "duplicate payload target {target}")
            }
            Self::PayloadDigestRequired { target_ref } => write!(
                formatter,
                "payload {target_ref} lacks a signed sha256 digest"
            ),
            Self::PayloadDigestMismatch {
                target_ref,
                expected,
                observed,
            } => write!(
                formatter,
                "payload {target_ref} digest mismatch: expected {expected}, observed {observed}"
            ),
            Self::InvalidRiskAudit { reference, errors } => write!(
                formatter,
                "risk audit {} has {errors} validation error(s)",
                reference.display()
            ),
            Self::CitationProjection(source) => {
                write!(
                    formatter,
                    "cannot project trusted citation ledger: {source}"
                )
            }
            Self::AdmissionAuthorityMismatch => {
                formatter.write_str("snapshot Admission authority differs from verified call")
            }
            Self::AdmissionDigestMismatch => {
                formatter.write_str("snapshot Admission digest differs from signed intent")
            }
            Self::OperationBindingMismatch => {
                formatter.write_str("operation contract differs from signed Admission")
            }
            Self::AssuranceBindingMismatch => {
                formatter.write_str("Assurance Case differs from signed Admission")
            }
            Self::AuthoritySnapshotBindingMismatch => {
                formatter.write_str("mutable authority snapshot differs from signed Admission")
            }
            Self::CommandBindingMismatch => {
                formatter.write_str("command contracts differ from signed Admission")
            }
            Self::EffectBindingMismatch => {
                formatter.write_str("effect contract differs from signed Admission")
            }
            Self::SnapshotRevisionMismatch => {
                formatter.write_str("snapshot revisions differ from signed Admission")
            }
            Self::Binding(source) => write!(formatter, "cannot compute content binding: {source}"),
        }
    }
}

impl std::error::Error for TrustedMcpLoadError {}
