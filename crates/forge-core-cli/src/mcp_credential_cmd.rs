//! Operator-owned MCP credential provisioning, rotation, revocation, and signing.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{CliEnvelope, ExitReason, PrincipalId, StableId};
use forge_core_protocol_mcp::{
    AttestationInput, AuthorizedPrincipalRegistry, CanonicalIntent,
    McpLocalExecutionSnapshotDocument, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, MCP_EXECUTE_OPERATION_TOOL,
    PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "mcp credential";

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct CredentialResult {
    action: String,
    credential_id: String,
    registry_path: String,
    public_key_fingerprint: String,
    secret_deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mcp_meta: Option<Value>,
    storage_boundary: String,
}

pub(crate) fn run_credential_command(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    let flags = parse_flags(&args[1..])?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    match action {
        "provision" => provision(&flags, None, want_json),
        "rotate" => {
            let old = required(&flags, "--replaces")?.to_owned();
            provision(&flags, Some(old), want_json)
        }
        "revoke" => revoke(&flags, want_json),
        "sign" => sign(&flags, want_json),
        _ => emit_error(
            "expected credential action: provision, rotate, revoke, or sign",
            want_json,
        ),
    }
}

fn provision(
    flags: &BTreeMap<String, Vec<String>>,
    replaces: Option<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    let root = canonical(required_path(flags, "--root")?, "project root")?;
    let resolved = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    let state_root = canonical(PathBuf::from(resolved.state_root), "state root")?;
    let registry_path = absolute_path(required_path(flags, "--registry")?)?;
    let secret_dir = absolute_path(required_path(flags, "--secret-dir")?)?;
    ensure_operator_owned_location(&registry_path, &root, &state_root)?;
    ensure_operator_owned_location(&secret_dir, &root, &state_root)?;
    let credential_id = required(flags, "--credential-id")?.to_owned();
    let audience = required(flags, "--audience")?.to_owned();
    let principal_id = PrincipalId(required(flags, "--principal-id")?.to_owned());
    let agent_id = StableId(required(flags, "--agent-id")?.to_owned());
    let role = parse_role(required(flags, "--role")?)?;
    let mut document = load_or_new_registry(&registry_path, &audience)?;
    if document
        .principal_registry
        .principals
        .iter()
        .any(|entry| entry.credential_id == credential_id)
    {
        return Err(ExitError::env_config(format!(
            "credential_id '{credential_id}' already exists"
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
    let signing_key = SigningKey::from_bytes(&secret);
    secret.fill(0);
    let public_key_hex = hex(signing_key.verifying_key().as_bytes());
    document
        .principal_registry
        .principals
        .push(PrincipalRegistryEntry {
            credential_id: credential_id.clone(),
            principal_id,
            agent_id,
            role,
            public_key_hex: public_key_hex.clone(),
            allowed_tools: vec![StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned())],
            authority_grants: vec![StableId("operation.execute".to_owned())],
            status: PrincipalCredentialStatus::Active,
        });
    validate_registry(&document)?;
    fs_create_private_dir(&secret_dir)?;
    let secret_path = secret_path(&secret_dir, &credential_id);
    write_secret_new(&secret_path, signing_key.as_bytes())?;
    if let Err(error) = write_registry(&registry_path, &document) {
        let _ = std::fs::remove_file(&secret_path);
        return Err(error);
    }
    let replaced_secret_deleted = replaces.as_ref().map(|old| {
        let old_path = secret_path
            .parent()
            .expect("secret parent")
            .join(format!("{}.ed25519", hex(Sha256::digest(old))));
        old_path.exists() && std::fs::remove_file(old_path).is_ok()
    });
    let result = CredentialResult {
        action: if replaces.is_some() { "rotated" } else { "provisioned" }.to_owned(),
        credential_id,
        registry_path: path_string(&registry_path)?,
        public_key_fingerprint: format!("sha256:{}", hex(Sha256::digest(public_key_hex))),
        secret_deleted: replaced_secret_deleted,
        mcp_meta: None,
        storage_boundary: "private key stored only in the explicit operator directory; never emitted or written under project/state roots".to_owned(),
    };
    emit_envelope(CliEnvelope::ok(COMMAND, result), want_json)
}

fn revoke(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let root = canonical(required_path(flags, "--root")?, "project root")?;
    let resolved = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    let state_root = canonical(PathBuf::from(resolved.state_root), "state root")?;
    let registry_path = absolute_path(required_path(flags, "--registry")?)?;
    let secret_dir = absolute_path(required_path(flags, "--secret-dir")?)?;
    ensure_operator_owned_location(&registry_path, &root, &state_root)?;
    ensure_operator_owned_location(&secret_dir, &root, &state_root)?;
    let credential_id = required(flags, "--credential-id")?;
    let mut document = load_registry(&registry_path)?;
    let entry = document
        .principal_registry
        .principals
        .iter_mut()
        .find(|entry| entry.credential_id == credential_id)
        .ok_or_else(|| ExitError::env_config(format!("unknown credential '{credential_id}'")))?;
    entry.status = PrincipalCredentialStatus::Revoked;
    let public_key_hex = entry.public_key_hex.clone();
    validate_registry(&document)?;
    write_registry(&registry_path, &document)?;
    let path = secret_path(&secret_dir, credential_id);
    let deleted = if path.exists() {
        std::fs::remove_file(&path).map_err(|error| {
            ExitError::env_config(format!(
                "credential is revoked but secret deletion {} failed: {error}",
                path.display()
            ))
        })?;
        true
    } else {
        false
    };
    emit_envelope(
        CliEnvelope::ok(
            COMMAND,
            CredentialResult {
                action: "revoked".to_owned(),
                credential_id: credential_id.to_owned(),
                registry_path: path_string(&registry_path)?,
                public_key_fingerprint: format!("sha256:{}", hex(Sha256::digest(public_key_hex))),
                secret_deleted: Some(deleted),
                mcp_meta: None,
                storage_boundary: "registry revocation is committed before private-key deletion"
                    .to_owned(),
            },
        ),
        want_json,
    )
}

fn sign(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let root = canonical(required_path(flags, "--root")?, "project root")?;
    let resolved = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    let state_root = canonical(PathBuf::from(resolved.state_root), "state root")?;
    let registry_path = absolute_path(required_path(flags, "--registry")?)?;
    let secret_dir = absolute_path(required_path(flags, "--secret-dir")?)?;
    ensure_operator_owned_location(&registry_path, &root, &state_root)?;
    ensure_operator_owned_location(&secret_dir, &root, &state_root)?;
    let credential_id = required(flags, "--credential-id")?;
    let snapshot_ref = required_path(flags, "--snapshot")?;
    let arguments_path = required_path(flags, "--arguments-json")?;
    let document = load_registry(&registry_path)?;
    let entry = document
        .principal_registry
        .principals
        .iter()
        .find(|entry| entry.credential_id == credential_id)
        .filter(|entry| entry.status == PrincipalCredentialStatus::Active)
        .ok_or_else(|| ExitError::env_config("credential is unknown or revoked".to_owned()))?;
    let snapshot_text = read_state_relative(&state_root, &snapshot_ref)?;
    let snapshot: McpLocalExecutionSnapshotDocument = yaml_serde::from_str(&snapshot_text)
        .map_err(|error| ExitError::env_config(format!("invalid snapshot: {error}")))?;
    let request = &snapshot.execution_snapshot.admission_request;
    if request.principal_id != entry.principal_id || request.agent_id != entry.agent_id {
        return Err(ExitError::env_config(
            "snapshot principal differs from selected credential".to_owned(),
        ));
    }
    let arguments: Value =
        serde_json::from_str(&std::fs::read_to_string(&arguments_path).map_err(|error| {
            ExitError::env_config(format!("cannot read arguments JSON: {error}"))
        })?)
        .map_err(|error| ExitError::env_config(format!("invalid arguments JSON: {error}")))?;
    if !arguments.is_object() {
        return Err(ExitError::env_config(
            "arguments JSON must be an object".to_owned(),
        ));
    }
    let intent_digest = forge_core_decisions::execution_intent_digest(request)
        .map_err(|error| ExitError::env_config(error.to_string()))?;
    let intent = CanonicalIntent {
        tool: MCP_EXECUTE_OPERATION_TOOL.to_owned(),
        arguments,
        credential_id: Some(credential_id.to_owned()),
        audience: Some(document.principal_registry.audience.clone()),
        execution_intent_digest: Some(intent_digest.clone()),
        nonce: request.nonce.clone(),
        ts: request.issued_at_unix,
    };
    let secret_path = secret_path(&secret_dir, credential_id);
    let key = read_signing_key(&secret_path)?;
    if hex(key.verifying_key().as_bytes()) != entry.public_key_hex {
        return Err(ExitError::env_config(
            "private key does not match registry public key".to_owned(),
        ));
    }
    let signature = key.sign(
        &intent
            .canonical_bytes()
            .map_err(|error| ExitError::env_config(error.to_string()))?,
    );
    let attestation = AttestationInput {
        credential_id: intent.credential_id,
        audience: intent.audience,
        execution_intent_digest: intent.execution_intent_digest,
        nonce: intent.nonce,
        ts: intent.ts,
        signature: hex(signature.to_bytes()),
        public_key_hex: entry.public_key_hex.clone(),
    };
    emit_envelope(
        CliEnvelope::ok(
            COMMAND,
            CredentialResult {
                action: "signed".to_owned(),
                credential_id: credential_id.to_owned(),
                registry_path: path_string(&registry_path)?,
                public_key_fingerprint: format!(
                    "sha256:{}",
                    hex(Sha256::digest(&entry.public_key_hex))
                ),
                secret_deleted: None,
                mcp_meta: Some(serde_json::json!({"attestation": attestation})),
                storage_boundary:
                    "signature produced in process; private key bytes were not emitted".to_owned(),
            },
        ),
        want_json,
    )
}

fn load_or_new_registry(
    path: &Path,
    audience: &str,
) -> Result<PrincipalRegistryDocument, ExitError> {
    if path.exists() {
        let document = load_registry(path)?;
        if document.principal_registry.audience != audience {
            return Err(ExitError::env_config(
                "registry audience mismatch".to_owned(),
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

fn load_registry(path: &Path) -> Result<PrincipalRegistryDocument, ExitError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| ExitError::env_config(format!("cannot read registry: {error}")))?;
    let document = yaml_serde::from_str(&text)
        .map_err(|error| ExitError::env_config(format!("invalid registry YAML: {error}")))?;
    validate_registry(&document)?;
    Ok(document)
}

fn validate_registry(document: &PrincipalRegistryDocument) -> Result<(), ExitError> {
    AuthorizedPrincipalRegistry::from_document(document.clone())
        .map(|_| ())
        .map_err(|error| ExitError::env_config(format!("invalid registry: {error}")))
}

fn write_registry(path: &Path, document: &PrincipalRegistryDocument) -> Result<(), ExitError> {
    let parent = path
        .parent()
        .ok_or_else(|| ExitError::env_config("registry has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| ExitError::env_config(format!("cannot create registry dir: {error}")))?;
    let yaml = yaml_serde::to_string(document)
        .map_err(|error| ExitError::env_config(format!("cannot serialize registry: {error}")))?;
    crate::io_util::atomic_write(path, &yaml)
        .map_err(|error| ExitError::env_config(format!("cannot write registry: {error}")))
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
        .map_err(|error| ExitError::env_config(format!("cannot create private key: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| ExitError::env_config(format!("cannot persist private key: {error}")))
}

pub(crate) fn read_signing_key(path: &Path) -> Result<SigningKey, ExitError> {
    let mut bytes = [0_u8; 32];
    let mut file = File::open(path)
        .map_err(|error| ExitError::env_config(format!("cannot open private key: {error}")))?;
    file.read_exact(&mut bytes)
        .map_err(|error| ExitError::env_config(format!("cannot read private key: {error}")))?;
    let key = SigningKey::from_bytes(&bytes);
    bytes.fill(0);
    Ok(key)
}

fn read_state_relative(state_root: &Path, reference: &Path) -> Result<String, ExitError> {
    if reference.is_absolute()
        || reference
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ExitError::env_config(
            "snapshot must be state-relative".to_owned(),
        ));
    }
    let path = state_root.join(reference);
    let canonical = canonical(&path, "snapshot")?;
    if !canonical.starts_with(state_root) {
        return Err(ExitError::env_config(
            "snapshot escapes state root".to_owned(),
        ));
    }
    std::fs::read_to_string(canonical)
        .map_err(|error| ExitError::env_config(format!("cannot read snapshot: {error}")))
}

pub(crate) fn ensure_operator_owned_location(
    path: &Path,
    project: &Path,
    state: &Path,
) -> Result<(), ExitError> {
    if path.starts_with(project) || path.starts_with(state) {
        return Err(ExitError::env_config(
            "operator-owned authority paths must remain outside both project and Forge state roots"
                .to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn secret_path(secret_dir: &Path, credential_id: &str) -> PathBuf {
    secret_dir.join(format!("{}.ed25519", hex(Sha256::digest(credential_id))))
}

fn fs_create_private_dir(path: &Path) -> Result<(), ExitError> {
    std::fs::create_dir_all(path).map_err(|error| {
        ExitError::env_config(format!("cannot create secret directory: {error}"))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| ExitError::env_config(format!("cannot protect secret directory: {error}")),
        )?;
    }
    Ok(())
}

fn parse_flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
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
        index += 1;
        let value = args
            .get(index)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        flags.entry(flag.clone()).or_default().push(value.clone());
        index += 1;
    }
    Ok(flags)
}

fn required<'a>(
    flags: &'a BTreeMap<String, Vec<String>>,
    flag: &str,
) -> Result<&'a str, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.last())
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    required(flags, flag).map(PathBuf::from)
}

fn parse_role(value: &str) -> Result<CallerRole, ExitError> {
    match value {
        "driver" => Ok(CallerRole::Driver),
        "worker" => Ok(CallerRole::Worker),
        "runtime" => Ok(CallerRole::Runtime),
        _ => Err(ExitError::usage(
            "--role must be driver, worker, or runtime".to_owned(),
        )),
    }
}

pub(crate) fn absolute_path(path: PathBuf) -> Result<PathBuf, ExitError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| ExitError::env_config(error.to_string()))
    }
}

fn canonical(path: impl AsRef<Path>, label: &str) -> Result<PathBuf, ExitError> {
    std::fs::canonicalize(path.as_ref()).map_err(|error| {
        ExitError::env_config(format!(
            "cannot resolve {label} {}: {error}",
            path.as_ref().display()
        ))
    })
}

fn path_string(path: &Path) -> Result<String, ExitError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ExitError::env_config("path is not valid UTF-8".to_owned()))
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn emit_error(message: &str, want_json: bool) -> Result<(), ExitError> {
    emit_envelope(
        CliEnvelope::<()>::err(COMMAND, ExitReason::InvalidDecisionShape, message),
        want_json,
    )
}
