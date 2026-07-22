//! Agent-facing lifecycle for operator-owned workflow authorization keys.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    AttestationInput, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry,
    CanonicalIntent, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, WorkflowApplicabilityAuthorizationRequest,
    WorkflowAuthorizationKind, WorkflowCapabilityAuthorizationRequest,
    WorkflowDecisionAuthorizationRequest, WorkflowEvidenceAuthorizationRequest,
    WorkflowSignalAuthorizationRequest, WorkflowWaiverAuthorizationRequest,
    PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{CliEnvelope, PrincipalId, StableId};
use forge_core_kernel::WorkflowGovernanceProjectAdapter;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "workflow credential";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialProfile {
    Human,
    Agent,
    Runtime,
}

impl CredentialProfile {
    fn parse(value: &str) -> Result<Self, ExitError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" | "reviewer" => Ok(Self::Agent),
            "runtime" => Ok(Self::Runtime),
            _ => Err(ExitError::usage(
                "--profile must be human, agent, reviewer, or runtime".to_owned(),
            )),
        }
    }

    const fn role(self) -> CallerRole {
        match self {
            Self::Human => CallerRole::Human,
            Self::Agent => CallerRole::Worker,
            Self::Runtime => CallerRole::Runtime,
        }
    }

    fn grants(self) -> Vec<StableId> {
        let values: &[&str] = match self {
            Self::Human => &[
                "workflow.applicability.assess",
                "workflow.decision.resolve",
                "workflow.evidence.authorize_human",
                "workflow.waiver.authorize",
            ],
            Self::Agent => &[
                "workflow.evidence.authorize_review",
                "workflow.evidence.authorize_external",
                "workflow.signal.authorize",
            ],
            Self::Runtime => &[
                "workflow.capability.authorize",
                "workflow.evidence.authorize_runtime",
                "workflow.evidence.authorize_external",
                "workflow.signal.authorize",
            ],
        };
        values
            .iter()
            .map(|value| StableId((*value).to_owned()))
            .collect()
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Runtime => "runtime",
        }
    }
}

#[derive(Debug)]
struct AuthorityPaths {
    project_id: String,
    project_root: PathBuf,
    state_root: PathBuf,
    operator_dir: PathBuf,
    registry: PathBuf,
    secrets: PathBuf,
}

#[allow(dead_code)]
const MAX_PUBLIC_REGISTRY_BYTES: u64 = 8 * 1024 * 1024;

#[allow(dead_code)]
/// Opaque retained producer authority for the public principal registry.
pub(crate) struct LockedWorkflowCredentialRegistry {
    registry_file: PathBuf,
    expected_audience: String,
    lock: crate::io_util::DirLock,
}

#[allow(dead_code)]
/// Exact public principal-registry bytes; private workflow keys are excluded.
pub(crate) struct WorkflowCredentialRegistrySnapshot {
    raw_registry: Option<Vec<u8>>,
    raw_sha256: Option<String>,
}

#[allow(dead_code)]
impl WorkflowCredentialRegistrySnapshot {
    pub(crate) fn raw_registry(&self) -> Option<&[u8]> {
        self.raw_registry.as_deref()
    }

    pub(crate) fn raw_sha256(&self) -> Option<&str> {
        self.raw_sha256.as_deref()
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct CredentialResult {
    action: String,
    credential_id: Option<String>,
    profile: Option<String>,
    registry_path: String,
    public_key_fingerprint: Option<String>,
    secret_deleted: Option<bool>,
    principals: Option<Vec<CredentialStatusRow>>,
    storage_boundary: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct CredentialStatusRow {
    credential_id: String,
    principal_id: String,
    agent_id: String,
    role: CallerRole,
    grants: Vec<String>,
    status: PrincipalCredentialStatus,
    public_key_fingerprint: String,
}

/// In-process result for a high-level workflow action. Keeping this helper
/// crate-private lets the public action surface sign and authorize one exact
/// kernel-prepared request without serializing an intermediate attestation.
pub(crate) struct SignedWorkflowRequest {
    pub(crate) registry: AuthorizedPrincipalRegistry,
    pub(crate) attestation: AttestationInput,
}

#[derive(Debug)]
enum NormalizedWorkflowRequest {
    Applicability(WorkflowApplicabilityAuthorizationRequest),
    Capability(WorkflowCapabilityAuthorizationRequest),
    Decision(WorkflowDecisionAuthorizationRequest),
    Evidence(WorkflowEvidenceAuthorizationRequest),
    Signal(WorkflowSignalAuthorizationRequest),
    Waiver(WorkflowWaiverAuthorizationRequest),
}

impl NormalizedWorkflowRequest {
    fn json_value(&self) -> Result<Value, ExitError> {
        match self {
            Self::Applicability(value) => serde_json::to_value(value),
            Self::Capability(value) => serde_json::to_value(value),
            Self::Decision(value) => serde_json::to_value(value),
            Self::Evidence(value) => serde_json::to_value(value),
            Self::Signal(value) => serde_json::to_value(value),
            Self::Waiver(value) => serde_json::to_value(value),
        }
        .map_err(|error| ExitError::env_config(format!("serialize typed request: {error}")))
    }

    fn validate_authorization(
        &self,
        document: &PrincipalRegistryDocument,
        attestation: &AttestationInput,
    ) -> Result<(), ExitError> {
        let registry =
            AuthorizedPrincipalRegistry::from_document(document.clone()).map_err(|error| {
                ExitError::env_config(format!("invalid workflow registry: {error}"))
            })?;
        let verifier = AttestationVerifier::new(AttestationPolicy::Default);
        let result = match self {
            Self::Applicability(value) => registry
                .authorize_workflow_applicability(&verifier, value.clone(), attestation)
                .map(|_| ()),
            Self::Capability(value) => registry
                .authorize_workflow_capability(&verifier, value.clone(), attestation)
                .map(|_| ()),
            Self::Decision(value) => registry
                .authorize_workflow_decision(&verifier, value.clone(), attestation)
                .map(|_| ()),
            Self::Evidence(value) => registry
                .authorize_workflow_evidence(&verifier, value.clone(), attestation)
                .map(|_| ()),
            Self::Signal(value) => registry
                .authorize_workflow_signal(&verifier, value.clone(), attestation)
                .map(|_| ()),
            Self::Waiver(value) => registry
                .authorize_workflow_waiver(&verifier, value.clone(), attestation)
                .map(|_| ()),
        };
        result.map_err(|error| {
            ExitError::env_config(format!(
                "credential profile cannot authorize this request: {error}"
            ))
        })
    }
}

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let flags = parse_flags(action, &args[1..])?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    match action {
        "provision" => provision(&flags, None, want_json),
        "rotate" => provision(
            &flags,
            Some(required(&flags, "--replaces")?.to_owned()),
            want_json,
        ),
        "revoke" => revoke(&flags, want_json),
        "status" => status(&flags, want_json),
        _ => Err(ExitError::usage(usage())),
    }
}

fn provision(
    flags: &BTreeMap<String, Vec<String>>,
    replaces: Option<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    let paths = authority_paths(required_path(flags, "--root")?)?;
    let credential_id = required(flags, "--credential-id")?.to_owned();
    let principal_id = PrincipalId(required(flags, "--principal-id")?.to_owned());
    let agent_id = StableId(required(flags, "--agent-id")?.to_owned());
    let profile = CredentialProfile::parse(required(flags, "--profile")?)?;
    require_nonblank("--credential-id", &credential_id)?;
    require_nonblank("--principal-id", &principal_id.0)?;
    require_nonblank("--agent-id", &agent_id.0)?;

    if let Some(old) = replaces.as_deref() {
        require_nonblank("--replaces", old)?;
    }
    let audience = format!("forge-core:workflow:{}", paths.project_id);
    let _lock = acquire_authority_lock(&paths)?;
    let mut document = load_or_new_registry(&paths.registry, &audience)?;
    if document
        .principal_registry
        .principals
        .iter()
        .any(|entry| entry.credential_id == credential_id)
    {
        return Err(ExitError::env_config(format!(
            "workflow credential '{credential_id}' already exists"
        )));
    }
    if let Some(old) = replaces.as_ref() {
        let entry = document
            .principal_registry
            .principals
            .iter_mut()
            .find(|entry| entry.credential_id == *old)
            .ok_or_else(|| ExitError::env_config(format!("unknown replaced credential '{old}'")))?;
        entry.status = PrincipalCredentialStatus::Revoked;
    }

    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret)
        .map_err(|error| ExitError::env_config(format!("OS random generation failed: {error}")))?;
    let key = SigningKey::from_bytes(&secret);
    secret.fill(0);
    let public_key_hex = hex(key.verifying_key().as_bytes());
    document
        .principal_registry
        .principals
        .push(PrincipalRegistryEntry {
            credential_id: credential_id.clone(),
            principal_id,
            agent_id,
            role: profile.role(),
            public_key_hex: public_key_hex.clone(),
            allowed_tools: vec![StableId("workflow".to_owned())],
            authority_grants: profile.grants(),
            status: PrincipalCredentialStatus::Active,
        });
    validate_registry_for_audience(
        &document,
        &format!("forge-core:workflow:{}", paths.project_id),
    )?;
    create_private_dir(&paths.secrets)?;
    let new_secret = secret_path(&paths.secrets, &credential_id);
    write_secret_new(&new_secret, key.as_bytes())?;
    if let Err(error) = write_registry(&paths.registry, &document) {
        let _ = std::fs::remove_file(&new_secret);
        return Err(error);
    }
    let deleted = replaces.as_ref().map(|old| {
        let old_secret = secret_path(&paths.secrets, old);
        old_secret.exists() && std::fs::remove_file(old_secret).is_ok()
    });
    emit_result(
        CredentialResult {
            action: if replaces.is_some() { "rotated" } else { "provisioned" }.to_owned(),
            credential_id: Some(credential_id),
            profile: Some(profile.label().to_owned()),
            registry_path: display(&paths.registry),
            public_key_fingerprint: Some(fingerprint(&public_key_hex)),
            secret_deleted: deleted,
            principals: None,
            storage_boundary: "private key stored only in the derived operator directory outside project and Forge state; key bytes are never emitted".to_owned(),
        },
        want_json,
    )
}

fn revoke(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let paths = authority_paths(required_path(flags, "--root")?)?;
    let credential_id = required(flags, "--credential-id")?;
    require_nonblank("--credential-id", credential_id)?;
    let _lock = acquire_authority_lock(&paths)?;
    let mut document = load_registry(
        &paths.registry,
        &format!("forge-core:workflow:{}", paths.project_id),
    )?;
    let entry = document
        .principal_registry
        .principals
        .iter_mut()
        .find(|entry| entry.credential_id == credential_id)
        .ok_or_else(|| ExitError::env_config(format!("unknown credential '{credential_id}'")))?;
    entry.status = PrincipalCredentialStatus::Revoked;
    let public_key = entry.public_key_hex.clone();
    validate_registry_for_audience(
        &document,
        &format!("forge-core:workflow:{}", paths.project_id),
    )?;
    write_registry(&paths.registry, &document)?;
    let secret = secret_path(&paths.secrets, credential_id);
    let deleted = if secret.exists() {
        std::fs::remove_file(&secret).map_err(|error| {
            ExitError::env_config(format!(
                "credential is revoked but secret deletion {} failed: {error}",
                secret.display()
            ))
        })?;
        true
    } else {
        false
    };
    emit_result(
        CredentialResult {
            action: "revoked".to_owned(),
            credential_id: Some(credential_id.to_owned()),
            profile: None,
            registry_path: display(&paths.registry),
            public_key_fingerprint: Some(fingerprint(&public_key)),
            secret_deleted: Some(deleted),
            principals: None,
            storage_boundary: "registry revocation is committed before private-key deletion"
                .to_owned(),
        },
        want_json,
    )
}

fn status(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let paths = authority_paths(required_path(flags, "--root")?)?;
    let document = load_registry(
        &paths.registry,
        &format!("forge-core:workflow:{}", paths.project_id),
    )?;
    let principals = document
        .principal_registry
        .principals
        .iter()
        .map(|entry| CredentialStatusRow {
            credential_id: entry.credential_id.clone(),
            principal_id: entry.principal_id.0.clone(),
            agent_id: entry.agent_id.0.clone(),
            role: entry.role,
            grants: entry
                .authority_grants
                .iter()
                .map(|grant| grant.0.clone())
                .collect(),
            status: entry.status,
            public_key_fingerprint: fingerprint(&entry.public_key_hex),
        })
        .collect();
    emit_result(
        CredentialResult {
            action: "status".to_owned(),
            credential_id: None,
            profile: None,
            registry_path: display(&paths.registry),
            public_key_fingerprint: None,
            secret_deleted: None,
            principals: Some(principals),
            storage_boundary:
                "status exposes registry audit metadata and fingerprints, never private key bytes"
                    .to_owned(),
        },
        want_json,
    )
}

/// Sign one already-prepared typed request without exposing an intermediate
/// file. The caller must still pass the result through the matching authority
/// verifier and kernel late-binding check in the same command invocation.
pub(crate) fn sign_typed_request(
    root: &Path,
    credential_id: &str,
    kind: WorkflowAuthorizationKind,
    request_value: Value,
) -> Result<SignedWorkflowRequest, ExitError> {
    let paths = authority_paths(root.to_path_buf())?;
    require_nonblank("credential_id", credential_id)?;
    let request = normalized_request_value(kind, request_value)?;
    let (document, attestation) = sign_normalized_request(&paths, credential_id, kind, &request)?;
    let registry = AuthorizedPrincipalRegistry::from_document(document)
        .map_err(|error| ExitError::env_config(format!("invalid workflow registry: {error}")))?;
    Ok(SignedWorkflowRequest {
        registry,
        attestation,
    })
}

fn sign_normalized_request(
    paths: &AuthorityPaths,
    credential_id: &str,
    kind: WorkflowAuthorizationKind,
    request: &NormalizedWorkflowRequest,
) -> Result<(PrincipalRegistryDocument, AttestationInput), ExitError> {
    let document = load_registry(
        &paths.registry,
        &format!("forge-core:workflow:{}", paths.project_id),
    )?;
    let entry = document
        .principal_registry
        .principals
        .iter()
        .find(|entry| entry.credential_id == credential_id)
        .filter(|entry| entry.status == PrincipalCredentialStatus::Active)
        .ok_or_else(|| ExitError::env_config("credential is unknown or revoked".to_owned()))?;
    let key = read_signing_key(&secret_path(&paths.secrets, credential_id))?;
    if hex(key.verifying_key().as_bytes()) != entry.public_key_hex {
        return Err(ExitError::env_config(
            "private key does not match workflow registry public key".to_owned(),
        ));
    }
    let ts = now_unix()?;
    let mut nonce_bytes = [0_u8; 24];
    getrandom::fill(&mut nonce_bytes)
        .map_err(|error| ExitError::env_config(format!("OS random generation failed: {error}")))?;
    let nonce = format!("workflow-{}", hex(&nonce_bytes));
    let mut attestation = AttestationInput {
        credential_id: Some(credential_id.to_owned()),
        audience: Some(document.principal_registry.audience.clone()),
        execution_intent_digest: None,
        nonce: nonce.clone(),
        ts,
        signature: String::new(),
        public_key_hex: entry.public_key_hex.clone(),
    };
    let intent = CanonicalIntent {
        tool: "workflow".to_owned(),
        arguments: serde_json::json!({
            "action": kind.canonical_action(),
            "request": request.json_value()?,
        }),
        credential_id: attestation.credential_id.clone(),
        audience: attestation.audience.clone(),
        execution_intent_digest: None,
        nonce,
        ts,
    };
    attestation.signature = hex(&key
        .sign(
            &intent
                .canonical_bytes()
                .map_err(|error| ExitError::env_config(error.to_string()))?,
        )
        .to_bytes());
    request.validate_authorization(&document, &attestation)?;
    Ok((document, attestation))
}

fn normalized_request_value(
    kind: WorkflowAuthorizationKind,
    raw_value: Value,
) -> Result<NormalizedWorkflowRequest, ExitError> {
    match kind {
        WorkflowAuthorizationKind::Applicability => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Applicability)
        }
        WorkflowAuthorizationKind::Capability => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Capability)
        }
        WorkflowAuthorizationKind::Decision => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Decision)
        }
        WorkflowAuthorizationKind::Evidence => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Evidence)
        }
        WorkflowAuthorizationKind::IntentRevision => {
            return Err(ExitError::usage(
                "human intent revisions require `forge-core workflow intent record` with an external human-broker envelope; local workflow credentials cannot sign them"
                    .to_owned(),
            ));
        }
        WorkflowAuthorizationKind::Signal => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Signal)
        }
        WorkflowAuthorizationKind::Waiver => {
            serde_json::from_value(raw_value).map(NormalizedWorkflowRequest::Waiver)
        }
    }
    .map_err(|error| ExitError::env_config(format!("invalid typed request: {error}")))
}

fn authority_paths(root: PathBuf) -> Result<AuthorityPaths, ExitError> {
    let project = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(
            "Forge state is missing; run forge-core start before provisioning workflow authority"
                .to_owned(),
        ));
    }
    let project_root = std::fs::canonicalize(project.project_root)
        .map_err(|error| ExitError::env_config(format!("canonicalize project root: {error}")))?;
    let state_root = std::fs::canonicalize(project.state_root)
        .map_err(|error| ExitError::env_config(format!("canonicalize state root: {error}")))?;
    let adapter = WorkflowGovernanceProjectAdapter::new(
        StableId(project.project_id.clone()),
        &project_root,
        &state_root,
    )
    .map_err(|error| ExitError::env_config(error.to_string()))?;
    let registry = adapter.trusted_principal_registry_path();
    if registry.starts_with(&project_root) || registry.starts_with(&state_root) {
        return Err(ExitError::env_config(
            "derived workflow authority registry overlaps project or Forge state".to_owned(),
        ));
    }
    let secrets = registry
        .parent()
        .ok_or_else(|| {
            ExitError::env_config("workflow registry has no operator parent".to_owned())
        })?
        .join("workflow-secrets");
    let operator_dir = registry
        .parent()
        .ok_or_else(|| {
            ExitError::env_config("workflow registry has no operator parent".to_owned())
        })?
        .to_path_buf();
    reject_existing_links(&operator_dir)?;
    reject_existing_links(&registry)?;
    reject_existing_links(&secrets)?;
    validate_physical_boundary(&operator_dir, &project_root, &state_root)?;
    Ok(AuthorityPaths {
        project_id: project.project_id,
        project_root,
        state_root,
        operator_dir,
        registry,
        secrets,
    })
}

fn acquire_authority_lock(
    paths: &AuthorityPaths,
) -> Result<LockedWorkflowCredentialRegistry, ExitError> {
    std::fs::create_dir_all(&paths.operator_dir)
        .map_err(|error| ExitError::env_config(format!("create operator directory: {error}")))?;
    reject_existing_links(&paths.operator_dir)?;
    validate_physical_boundary(&paths.operator_dir, &paths.project_root, &paths.state_root)?;
    let lock = crate::io_util::DirLock::acquire(&paths.operator_dir, ".workflow-credential.lock")
        .map_err(|error| {
        ExitError::conflict(format!("cannot acquire workflow credential lock: {error}"))
    })?;
    let registry_file = paths
        .registry
        .strip_prefix(&paths.operator_dir)
        .map_err(|error| ExitError::env_config(format!("resolve public registry: {error}")))?
        .to_path_buf();
    Ok(LockedWorkflowCredentialRegistry {
        registry_file,
        expected_audience: format!("forge-core:workflow:{}", paths.project_id),
        lock,
    })
}

#[allow(dead_code)]
/// Resolve and retain the same credential-registry authority used by producers.
pub(crate) fn lock_workflow_credential_registry(
    project_root: &Path,
) -> Result<LockedWorkflowCredentialRegistry, ExitError> {
    let paths = authority_paths(project_root.to_path_buf())?;
    acquire_authority_lock(&paths)
}

#[allow(dead_code)]
/// Read exact public registry bytes under the retained producer authority.
pub(crate) fn snapshot_workflow_credential_registry(
    locked: &LockedWorkflowCredentialRegistry,
) -> Result<WorkflowCredentialRegistrySnapshot, ExitError> {
    let raw_registry = locked
        .lock
        .directory_identity()
        .read_optional_direct_file_bounded(&locked.registry_file, MAX_PUBLIC_REGISTRY_BYTES)
        .map_err(|error| {
            ExitError::env_config(format!(
                "read public workflow credential registry {}: {error}",
                locked.registry_file.display()
            ))
        })?;
    if let Some(raw) = raw_registry.as_deref() {
        let document: PrincipalRegistryDocument = yaml_serde::from_slice(raw).map_err(|error| {
            ExitError::env_config(format!(
                "strict public workflow credential registry parse failed: {error}"
            ))
        })?;
        validate_registry_for_audience(&document, &locked.expected_audience)?;
    }
    let raw_sha256 = raw_registry
        .as_ref()
        .map(|raw| format!("sha256:{:x}", Sha256::digest(raw)));
    Ok(WorkflowCredentialRegistrySnapshot {
        raw_registry,
        raw_sha256,
    })
}

fn reject_existing_links(path: &Path) -> Result<(), ExitError> {
    for current in path.ancestors() {
        match std::fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ExitError::env_config(format!(
                    "workflow authority path contains a symlink, junction, or reparse-point alias: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ExitError::env_config(format!(
                    "inspect workflow authority path {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn validate_physical_boundary(
    candidate: &Path,
    project_root: &Path,
    state_root: &Path,
) -> Result<(), ExitError> {
    let physical = physical_candidate(candidate)?;
    if physical.starts_with(project_root) || physical.starts_with(state_root) {
        return Err(ExitError::env_config(
            "derived workflow authority path physically overlaps project or Forge state".to_owned(),
        ));
    }
    Ok(())
}

fn physical_candidate(path: &Path) -> Result<PathBuf, ExitError> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            ExitError::env_config(format!(
                "workflow authority path has no existing ancestor: {}",
                path.display()
            ))
        })?;
    }
    let canonical = std::fs::canonicalize(existing).map_err(|error| {
        ExitError::env_config(format!("canonicalize {}: {error}", existing.display()))
    })?;
    let suffix = path.strip_prefix(existing).map_err(|error| {
        ExitError::env_config(format!(
            "resolve workflow authority path {}: {error}",
            path.display()
        ))
    })?;
    Ok(canonical.join(suffix))
}

fn load_or_new_registry(
    path: &Path,
    audience: &str,
) -> Result<PrincipalRegistryDocument, ExitError> {
    if path.exists() {
        let document = load_registry(path, audience)?;
        if document.principal_registry.audience != audience {
            return Err(ExitError::env_config(
                "workflow registry audience mismatch".to_owned(),
            ));
        }
        Ok(document)
    } else {
        Ok(PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: audience.to_owned(),
                principals: Vec::new(),
            },
        })
    }
}

fn load_registry(
    path: &Path,
    expected_audience: &str,
) -> Result<PrincipalRegistryDocument, ExitError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| ExitError::env_config(format!("read {}: {error}", path.display())))?;
    let document = yaml_serde::from_str(&raw)
        .map_err(|error| ExitError::env_config(format!("invalid registry YAML: {error}")))?;
    validate_registry_for_audience(&document, expected_audience)?;
    Ok(document)
}

fn validate_registry_for_audience(
    document: &PrincipalRegistryDocument,
    expected_audience: &str,
) -> Result<(), ExitError> {
    if document.principal_registry.audience != expected_audience {
        return Err(ExitError::env_config(
            "workflow registry audience mismatch".to_owned(),
        ));
    }
    AuthorizedPrincipalRegistry::from_document(document.clone())
        .map(|_| ())
        .map_err(|error| ExitError::env_config(format!("invalid workflow registry: {error}")))
}

fn write_registry(path: &Path, document: &PrincipalRegistryDocument) -> Result<(), ExitError> {
    let parent = path
        .parent()
        .ok_or_else(|| ExitError::env_config("workflow registry has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| ExitError::env_config(format!("create operator directory: {error}")))?;
    let yaml = yaml_serde::to_string(document)
        .map_err(|error| ExitError::env_config(format!("serialize registry: {error}")))?;
    crate::io_util::atomic_write(path, &yaml)
        .map_err(|error| ExitError::env_config(format!("write registry: {error}")))
}

fn write_secret_new(path: &Path, bytes: &[u8; 32]) -> Result<(), ExitError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| ExitError::env_config(format!("create private key: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| ExitError::env_config(format!("persist private key: {error}")))
}

fn read_signing_key(path: &Path) -> Result<SigningKey, ExitError> {
    let mut bytes = [0_u8; 32];
    let mut file = File::open(path)
        .map_err(|error| ExitError::env_config(format!("open private key: {error}")))?;
    file.read_exact(&mut bytes)
        .map_err(|error| ExitError::env_config(format!("read private key: {error}")))?;
    let key = SigningKey::from_bytes(&bytes);
    bytes.fill(0);
    Ok(key)
}

fn create_private_dir(path: &Path) -> Result<(), ExitError> {
    std::fs::create_dir_all(path)
        .map_err(|error| ExitError::env_config(format!("create secret directory: {error}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| ExitError::env_config(format!("protect secret directory: {error}")))?;
    }
    Ok(())
}

fn secret_path(directory: &Path, credential_id: &str) -> PathBuf {
    directory.join(format!("{}.ed25519", hex(&Sha256::digest(credential_id))))
}

fn parse_flags(action: &str, args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let allowed: &[&str] = match action {
        "provision" => &[
            "--root",
            "--credential-id",
            "--principal-id",
            "--agent-id",
            "--profile",
        ],
        "rotate" => &[
            "--root",
            "--replaces",
            "--credential-id",
            "--principal-id",
            "--agent-id",
            "--profile",
        ],
        "revoke" => &["--root", "--credential-id"],
        "status" => &["--root"],
        _ => return Err(ExitError::usage(usage())),
    };
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        if matches!(flag.as_str(), "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        if !flag.starts_with("--") {
            return Err(ExitError::usage(format!("unexpected argument '{flag}'")));
        }
        if !allowed.contains(&flag.as_str()) {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow credential {action}"
            )));
        }
        index += 1;
        let value = args
            .get(index)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        let values = flags.entry(flag.clone()).or_default();
        if !values.is_empty() {
            return Err(ExitError::usage(format!(
                "{flag} may be supplied only once"
            )));
        }
        values.push(value.clone());
        index += 1;
    }
    Ok(flags)
}

fn required<'a>(
    flags: &'a BTreeMap<String, Vec<String>>,
    flag: &str,
) -> Result<&'a str, ExitError> {
    optional(flags, flag).ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn optional<'a>(flags: &'a BTreeMap<String, Vec<String>>, flag: &str) -> Option<&'a str> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(String::as_str)
}

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    required(flags, flag).map(PathBuf::from)
}

fn require_nonblank(flag: &str, value: &str) -> Result<(), ExitError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        Err(ExitError::usage(format!(
            "{flag} must be 1..=128 bytes using only ASCII letters, digits, '.', '_', ':', or '-'"
        )))
    } else {
        Ok(())
    }
}

fn now_unix() -> Result<i64, ExitError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExitError::env_config("system clock is before Unix epoch".to_owned()))?
        .as_secs();
    i64::try_from(seconds).map_err(|_| ExitError::env_config("system time overflow".to_owned()))
}

fn fingerprint(public_key_hex: &str) -> String {
    let public_key = decode_public_key(public_key_hex).unwrap_or([0_u8; 32]);
    format!("sha256:{}", hex(&Sha256::digest(public_key)))
}

fn decode_public_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_nibble(pair[0])?;
        let low = decode_nibble(pair[1])?;
        output[index] = (high << 4) | low;
    }
    Some(output)
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn emit_result(result: CredentialResult, want_json: bool) -> Result<(), ExitError> {
    emit_envelope(CliEnvelope::ok(COMMAND, result), want_json)
}

fn usage() -> String {
    "usage:\n  forge-core workflow credential provision --root <project> --credential-id <id> --principal-id <id> --agent-id <id> --profile <human|agent|runtime> [--json|--no-json]\n  forge-core workflow credential rotate --root <project> --replaces <old-id> --credential-id <new-id> --principal-id <id> --agent-id <id> --profile <human|agent|runtime> [--json|--no-json]\n  forge-core workflow credential revoke --root <project> --credential-id <id> [--json|--no-json]\n  forge-core workflow credential status --root <project> [--json|--no-json]\n\nReusable attestation signing is intentionally unavailable. Local credentials are consumed only inside `workflow action authorize` after the kernel verifies an operator_credential_broker packet.".to_owned()
}

#[cfg(test)]
mod backup_seam_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn reusable_signing_is_absent_from_the_public_credential_surface() {
        assert!(!usage().contains("credential sign"));
        assert!(parse_flags("sign", &[]).is_err());
    }

    #[test]
    fn retained_credential_authority_snapshots_only_exact_public_registry() {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "forge-credential-lock-seam-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        let project_root = base.join("project");
        let state_root = base.join("state");
        let operator_dir = base.join("operator");
        let registry = operator_dir.join("principals.yaml");
        let secrets = operator_dir.join("workflow-secrets");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&state_root).unwrap();
        std::fs::create_dir_all(&secrets).unwrap();
        let raw = br#"schema_version: "0.1"
principal_registry:
  audience: forge-core:workflow:project.test
  principals:
    - credential_id: credential.test
      principal_id: principal.test
      agent_id: agent.test
      role: human
      public_key_hex: d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
      allowed_tools:
        - workflow
      authority_grants:
        - workflow.applicability.assess
      status: active
# retained exactly
"#;
        std::fs::write(&registry, raw).unwrap();
        std::fs::write(secrets.join("private.ed25519"), b"must-not-escape").unwrap();
        let paths = AuthorityPaths {
            project_id: "project.test".to_owned(),
            project_root,
            state_root,
            operator_dir,
            registry,
            secrets,
        };

        let locked = acquire_authority_lock(&paths).unwrap();
        let snapshot = snapshot_workflow_credential_registry(&locked).unwrap();
        assert_eq!(snapshot.raw_registry(), Some(raw.as_slice()));
        assert_eq!(
            snapshot.raw_sha256(),
            Some(format!("sha256:{:x}", Sha256::digest(raw)).as_str())
        );
        assert!(!snapshot
            .raw_registry()
            .unwrap()
            .windows(b"must-not-escape".len())
            .any(|window| window == b"must-not-escape"));
        drop(locked);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn retained_credential_snapshot_stays_bound_after_operator_parent_swap() {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "forge-credential-parent-swap-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        let project_root = base.join("project");
        let state_root = base.join("state");
        let operator_dir = base.join("operator");
        let registry = operator_dir.join("principals.yaml");
        let secrets = operator_dir.join("workflow-secrets");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&state_root).unwrap();
        std::fs::create_dir_all(&secrets).unwrap();
        let trusted = br#"schema_version: "0.1"
principal_registry:
  audience: forge-core:workflow:project.test
  principals:
    - credential_id: credential.test
      principal_id: principal.test
      agent_id: agent.test
      role: human
      public_key_hex: d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
      allowed_tools:
        - workflow
      authority_grants:
        - workflow.applicability.assess
      status: active
"#;
        std::fs::write(&registry, trusted).unwrap();
        let paths = AuthorityPaths {
            project_id: "project.test".to_owned(),
            project_root,
            state_root,
            operator_dir: operator_dir.clone(),
            registry: registry.clone(),
            secrets,
        };
        let locked = acquire_authority_lock(&paths).unwrap();
        let moved = base.join("operator-retained");
        std::fs::rename(&operator_dir, &moved).unwrap();
        std::fs::create_dir_all(operator_dir.join("workflow-secrets")).unwrap();
        std::fs::write(&registry, b"attacker: true\n").unwrap();
        let replacement = acquire_authority_lock(&paths).unwrap();

        let snapshot = snapshot_workflow_credential_registry(&locked).unwrap();
        assert_eq!(snapshot.raw_registry(), Some(trusted.as_slice()));
        drop(replacement);
        drop(locked);
        let _ = std::fs::remove_dir_all(base);
    }
}
