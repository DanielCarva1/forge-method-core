//! Agent-facing construction of content-bound trusted execution snapshots.

use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument, GateContractDocument,
    OperationContractDocument, PrincipalId, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, authority_snapshot_token, command_contract_token, effect_contract_token,
    execution_intent_digest, operation_contract_token, ClaimRevisionObservation,
    ClaimSnapshotObservation, ContentAddressedBinding, ExecutionAdmissionRequest,
    GateRevisionObservation, GateSnapshotObservation, RevisionExpectation, SnapshotCompleteness,
};
use forge_core_store::derive_state;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::trusted_loader::ConfinedProjectReader;
use crate::{
    McpLocalExecutionSnapshot, McpLocalExecutionSnapshotDocument, TrustedMcpLoadError,
    MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION,
};

const MAX_BUILD_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SNAPSHOT_EFFECT_CONTRACTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedSnapshotPrincipal {
    pub credential_id: String,
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub role: CallerRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedSnapshotBuildInput {
    pub operation_ref: PathBuf,
    pub assurance_ref: PathBuf,
    pub command_refs: Vec<PathBuf>,
    pub principal: TrustedSnapshotPrincipal,
    pub nonce: String,
    pub issued_at_unix: i64,
    pub now_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedSnapshotBuildOutput {
    pub snapshot: McpLocalExecutionSnapshotDocument,
    pub execution_intent_digest: String,
    pub authority_snapshot_token: String,
    pub operation_id: String,
    pub effect_count: usize,
    pub claim_count: usize,
    pub gate_count: usize,
}

/// Build a snapshot from project contracts and the authoritative sidecar
/// claim WAL. This pure adapter boundary does not write the result.
///
/// # Errors
///
/// Fails closed on unsafe references, malformed contracts, state drift,
/// ambiguous claim authority, missing required gates, or corrupt claim WAL.
pub fn build_trusted_execution_snapshot(
    project_root: impl AsRef<Path>,
    state_root: impl AsRef<Path>,
    input: &TrustedSnapshotBuildInput,
) -> Result<TrustedSnapshotBuildOutput, TrustedSnapshotBuildError> {
    validate_input(input)?;
    let reader = ConfinedProjectReader::new(project_root.as_ref())?;
    let operation: OperationContractDocument =
        reader.parse_yaml(&input.operation_ref, MAX_BUILD_INPUT_BYTES)?;
    let assurance: AssuranceCaseDocument =
        reader.parse_yaml(&input.assurance_ref, MAX_BUILD_INPUT_BYTES)?;
    let commands = input
        .command_refs
        .iter()
        .map(|reference| reader.parse_yaml(reference, MAX_BUILD_INPUT_BYTES))
        .collect::<Result<Vec<CommandContractDocument>, _>>()?;
    let effect_refs = effect_refs(&operation)?;
    let effects = effect_refs
        .iter()
        .map(|reference| reader.parse_yaml(Path::new(&reference.0), MAX_BUILD_INPUT_BYTES))
        .collect::<Result<Vec<ToolEffectContractDocument>, _>>()?;
    let state_version = validate_state_versions(&operation, &assurance)?;
    let claim_snapshot = build_claim_snapshot(&reader, state_root.as_ref(), &operation)?;
    let gate_snapshot = build_gate_snapshot(&reader, &operation, state_version)?;
    let authority_token = authority_snapshot_token(
        &claim_snapshot,
        &gate_snapshot,
        state_version,
        input.now_unix,
    )
    .map_err(binding_error)?;
    let request = build_admission_request(
        input,
        &operation,
        &assurance,
        &commands,
        effect_refs,
        &effects,
        &claim_snapshot,
        &gate_snapshot,
        authority_token.clone(),
    )?;
    let intent_digest = execution_intent_digest(&request).map_err(binding_error)?;
    let operation_id = operation.operation_contract.contract_id.0.clone();
    let effect_count = effects.len();
    let claim_count = claim_snapshot.claims.len();
    let gate_count = gate_snapshot.gates.len();
    Ok(TrustedSnapshotBuildOutput {
        snapshot: McpLocalExecutionSnapshotDocument {
            schema_version: MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION.to_owned(),
            execution_snapshot: McpLocalExecutionSnapshot {
                admission_request: request,
                assurance_case: assurance,
                claim_snapshot,
                gate_snapshot,
                current_state_version: state_version,
                now_unix: input.now_unix,
            },
        },
        execution_intent_digest: intent_digest,
        authority_snapshot_token: authority_token,
        operation_id,
        effect_count,
        claim_count,
        gate_count,
    })
}

fn validate_input(input: &TrustedSnapshotBuildInput) -> Result<(), TrustedSnapshotBuildError> {
    if input.nonce.trim().len() < 16 {
        return Err(TrustedSnapshotBuildError::InvalidInput(
            "nonce must contain at least 16 non-whitespace characters".to_owned(),
        ));
    }
    if input.principal.credential_id.trim().is_empty()
        || input.principal.principal_id.0.trim().is_empty()
        || input.principal.agent_id.0.trim().is_empty()
    {
        return Err(TrustedSnapshotBuildError::InvalidInput(
            "credential, principal, and agent identity must be non-empty".to_owned(),
        ));
    }
    if input.command_refs.iter().collect::<BTreeSet<_>>().len() != input.command_refs.len() {
        return Err(TrustedSnapshotBuildError::InvalidInput(
            "command references must be unique".to_owned(),
        ));
    }
    Ok(())
}

fn effect_refs(
    operation: &OperationContractDocument,
) -> Result<&[RepoPath], TrustedSnapshotBuildError> {
    let refs = &operation.operation_contract.effect_contract_refs;
    if refs.is_empty() || refs.len() > MAX_SNAPSHOT_EFFECT_CONTRACTS {
        return Err(TrustedSnapshotBuildError::InvalidEffectCount {
            count: refs.len(),
            maximum: MAX_SNAPSHOT_EFFECT_CONTRACTS,
        });
    }
    if refs
        .iter()
        .map(|reference| reference.0.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != refs.len()
    {
        return Err(TrustedSnapshotBuildError::InvalidInput(
            "effect references must be unique".to_owned(),
        ));
    }
    Ok(refs)
}

fn validate_state_versions(
    operation: &OperationContractDocument,
    assurance: &AssuranceCaseDocument,
) -> Result<u64, TrustedSnapshotBuildError> {
    let assurance_version = assurance.assurance_case.project_snapshot.state_version;
    let operation_version = operation.operation_contract.project_ref.state_version;
    let expected = operation
        .operation_contract
        .coordination_scope
        .concurrency
        .expected_state_version;
    if assurance_version != operation_version || assurance_version != expected {
        return Err(TrustedSnapshotBuildError::StateVersionMismatch {
            assurance: assurance_version,
            operation: operation_version,
            expected,
        });
    }
    Ok(assurance_version)
}

fn build_claim_snapshot(
    reader: &ConfinedProjectReader,
    state_root: &Path,
    operation: &OperationContractDocument,
) -> Result<ClaimSnapshotObservation, TrustedSnapshotBuildError> {
    let projection = derive_state(state_root)
        .map_err(|error| TrustedSnapshotBuildError::ClaimAuthority(error.to_string()))?;
    if !projection.diagnostics.is_empty() {
        return Err(TrustedSnapshotBuildError::ClaimAuthority(format!(
            "claim WAL projection has {} diagnostic(s)",
            projection.diagnostics.len()
        )));
    }
    let required_ref = operation
        .operation_contract
        .coordination_scope
        .write_authority
        .claim_contract_ref
        .as_ref();
    let required_id = required_ref
        .map(|reference| {
            reader
                .parse_yaml::<ClaimContractDocument>(Path::new(&reference.0), MAX_BUILD_INPUT_BYTES)
                .map(|document| document.claim_contract.id.0)
        })
        .transpose()?;
    if required_id
        .as_ref()
        .is_some_and(|id| !projection.latest_by_claim_id.contains_key(id))
    {
        return Err(TrustedSnapshotBuildError::RequiredClaimMissing(
            required_id.expect("checked some"),
        ));
    }
    let mut claims = projection
        .latest_by_claim_id
        .iter()
        .map(|(claim_id, projected)| {
            let reference = if required_id.as_deref() == Some(claim_id.as_str()) {
                required_ref.expect("required ref exists").clone()
            } else {
                RepoPath(format!(".forge-method/claims-wal/{claim_id}.yaml"))
            };
            ClaimRevisionObservation {
                claim_ref: reference,
                revision: projected.last_seq,
                document: ClaimContractDocument {
                    schema_version: "0.1".to_owned(),
                    claim_contract: projected.claim_contract.clone(),
                },
            }
        })
        .collect::<Vec<_>>();
    claims.sort_by(|left, right| left.claim_ref.0.cmp(&right.claim_ref.0));
    Ok(ClaimSnapshotObservation {
        revision: projection.last_applied_seq,
        completeness: SnapshotCompleteness::Complete,
        claims,
    })
}

fn build_gate_snapshot(
    reader: &ConfinedProjectReader,
    operation: &OperationContractDocument,
    state_version: u64,
) -> Result<GateSnapshotObservation, TrustedSnapshotBuildError> {
    let mut references = BTreeSet::new();
    references.extend(
        operation
            .operation_contract
            .gates
            .gate_contract_refs
            .iter()
            .map(|reference| reference.0.clone()),
    );
    references.extend(
        operation
            .operation_contract
            .gates
            .required_before_mutation
            .iter()
            .map(|required| required.gate_contract_ref.0.clone()),
    );
    let mut gates = Vec::with_capacity(references.len());
    for reference in references {
        let document: GateContractDocument =
            reader.parse_yaml(Path::new(&reference), MAX_BUILD_INPUT_BYTES)?;
        gates.push(GateRevisionObservation {
            gate_ref: RepoPath(reference),
            revision: canonical_revision(&document)?,
            observed_state_version: state_version,
            document,
        });
    }
    Ok(GateSnapshotObservation {
        revision: canonical_revision(&gates)?,
        completeness: SnapshotCompleteness::Complete,
        gates,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_admission_request(
    input: &TrustedSnapshotBuildInput,
    operation: &OperationContractDocument,
    assurance: &AssuranceCaseDocument,
    commands: &[CommandContractDocument],
    effect_refs: &[RepoPath],
    effects: &[ToolEffectContractDocument],
    claim_snapshot: &ClaimSnapshotObservation,
    gate_snapshot: &GateSnapshotObservation,
    authority_snapshot_token: String,
) -> Result<ExecutionAdmissionRequest, TrustedSnapshotBuildError> {
    let command_bindings = commands
        .iter()
        .map(|command| {
            Ok(ContentAddressedBinding {
                reference: command.command_contract.id.0.clone(),
                token: command_contract_token(command).map_err(binding_error)?,
            })
        })
        .collect::<Result<Vec<_>, TrustedSnapshotBuildError>>()?;
    let effect_bindings = effect_refs
        .iter()
        .zip(effects)
        .map(|(reference, effect)| {
            Ok(ContentAddressedBinding {
                reference: reference.0.clone(),
                token: effect_contract_token(effect).map_err(binding_error)?,
            })
        })
        .collect::<Result<Vec<_>, TrustedSnapshotBuildError>>()?;
    let nonce_hash = Sha256::digest(input.nonce.as_bytes());
    let suffix = nonce_hash[..6].iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    Ok(ExecutionAdmissionRequest {
        id: StableId(format!(
            "admission.{}.{}",
            operation.operation_contract.contract_id.0, suffix
        )),
        principal_id: input.principal.principal_id.clone(),
        agent_id: input.principal.agent_id.clone(),
        principal_role: input.principal.role,
        operation_id: operation.operation_contract.contract_id.clone(),
        operation_token: operation_contract_token(operation).map_err(binding_error)?,
        assurance_case_id: assurance.assurance_case.id.clone(),
        assurance_case_token: assurance_case_token(assurance).map_err(binding_error)?,
        command_bindings,
        effect_bindings,
        expected_claim_snapshot_revision: claim_snapshot.revision,
        expected_claim_revisions: claim_snapshot
            .claims
            .iter()
            .map(|claim| RevisionExpectation {
                reference: claim.claim_ref.0.clone(),
                revision: claim.revision,
            })
            .collect(),
        expected_gate_snapshot_revision: gate_snapshot.revision,
        expected_gate_revisions: gate_snapshot
            .gates
            .iter()
            .map(|gate| RevisionExpectation {
                reference: gate.gate_ref.0.clone(),
                revision: gate.revision,
            })
            .collect(),
        authority_snapshot_token,
        expected_replay_reservation_revision: 1,
        nonce: input.nonce.clone(),
        issued_at_unix: input.issued_at_unix,
    })
}

fn canonical_revision<T: Serialize>(value: &T) -> Result<u64, TrustedSnapshotBuildError> {
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| TrustedSnapshotBuildError::Binding(error.to_string()))?;
    let digest = Sha256::digest(canonical);
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    Ok(u64::from_be_bytes(bytes).max(1))
}

#[allow(clippy::needless_pass_by_value)] // map_err requires the owned error signature
fn binding_error(error: impl ToString) -> TrustedSnapshotBuildError {
    TrustedSnapshotBuildError::Binding(error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedSnapshotBuildError {
    InvalidInput(String),
    Load(TrustedMcpLoadError),
    ClaimAuthority(String),
    RequiredClaimMissing(String),
    InvalidEffectCount {
        count: usize,
        maximum: usize,
    },
    StateVersionMismatch {
        assurance: u64,
        operation: u64,
        expected: u64,
    },
    Binding(String),
}

impl From<TrustedMcpLoadError> for TrustedSnapshotBuildError {
    fn from(error: TrustedMcpLoadError) -> Self {
        Self::Load(error)
    }
}

impl fmt::Display for TrustedSnapshotBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::Load(error) => write!(formatter, "trusted input load failed: {error}"),
            Self::ClaimAuthority(error) => write!(formatter, "claim authority failed: {error}"),
            Self::RequiredClaimMissing(id) => {
                write!(formatter, "required claim '{id}' is absent from the claim WAL")
            }
            Self::InvalidEffectCount { count, maximum } => write!(
                formatter,
                "operation must declare between 1 and {maximum} effects, found {count}"
            ),
            Self::StateVersionMismatch {
                assurance,
                operation,
                expected,
            } => write!(
                formatter,
                "state version mismatch: assurance={assurance}, operation={operation}, expected={expected}"
            ),
            Self::Binding(error) => write!(formatter, "content binding failed: {error}"),
        }
    }
}

impl std::error::Error for TrustedSnapshotBuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fresh_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forge-mcp-snapshot-builder-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("contracts/effects")).expect("project tree");
        fs::create_dir_all(root.join("contracts/assurance")).expect("assurance tree");
        fs::create_dir_all(root.join("state")).expect("state tree");
        root
    }

    #[test]
    fn builder_derives_content_bound_snapshot_without_manual_yaml() {
        let root = fresh_root("valid");
        let source = repo_root();
        let mut operation: OperationContractDocument =
            yaml_serde::from_str(
                &fs::read_to_string(source.join(
                    "docs/fixtures/operation-contract-v0/destructive-effect-with-inverse.yaml",
                ))
                .expect("operation fixture"),
            )
            .expect("typed operation");
        let assurance_text = fs::read_to_string(
            source.join("contracts/assurance/representative-slice-verified-assurance.yaml"),
        )
        .expect("assurance fixture");
        let assurance: AssuranceCaseDocument =
            yaml_serde::from_str(&assurance_text).expect("typed assurance");
        let state_version = assurance.assurance_case.project_snapshot.state_version;
        operation.operation_contract.project_ref.state_version = state_version;
        operation
            .operation_contract
            .coordination_scope
            .concurrency
            .expected_state_version = state_version;
        fs::write(
            root.join("operation.yaml"),
            yaml_serde::to_string(&operation).expect("operation yaml"),
        )
        .expect("write operation");
        fs::write(root.join("contracts/assurance/case.yaml"), assurance_text)
            .expect("write assurance");
        fs::copy(
            source.join("contracts/effects/file-delete-restore-inverse-effect.yaml"),
            root.join("contracts/effects/file-delete-restore-inverse-effect.yaml"),
        )
        .expect("copy effect");
        let output = build_trusted_execution_snapshot(
            &root,
            root.join("state"),
            &TrustedSnapshotBuildInput {
                operation_ref: PathBuf::from("operation.yaml"),
                assurance_ref: PathBuf::from("contracts/assurance/case.yaml"),
                command_refs: Vec::new(),
                principal: TrustedSnapshotPrincipal {
                    credential_id: "key.agent.1".to_owned(),
                    principal_id: PrincipalId("principal.agent".to_owned()),
                    agent_id: StableId("agent".to_owned()),
                    role: CallerRole::Driver,
                },
                nonce: "0123456789abcdef".to_owned(),
                issued_at_unix: 1_800_000_000,
                now_unix: 1_800_000_000,
            },
        )
        .expect("generated snapshot");
        assert_eq!(output.claim_count, 0);
        assert_eq!(output.gate_count, 0);
        assert_eq!(
            output
                .snapshot
                .execution_snapshot
                .admission_request
                .authority_snapshot_token,
            output.authority_snapshot_token
        );
        assert_eq!(
            execution_intent_digest(&output.snapshot.execution_snapshot.admission_request)
                .expect("intent digest"),
            output.execution_intent_digest
        );
    }
}
