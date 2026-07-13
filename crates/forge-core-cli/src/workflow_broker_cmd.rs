//! Operator-only trust lifecycle for external workflow origin brokers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    AuthorizedWorkflowBrokerRegistry, WorkflowBrokerEnrollmentDeclaration,
    WorkflowBrokerIssuerEntry, WorkflowBrokerIssuerProfile, WorkflowBrokerIssuerStatus,
    WorkflowBrokerRegistryDocument, WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::{CliEnvelope, StableId};
use forge_core_kernel::WorkflowGovernanceProjectAdapter;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "workflow broker";

#[derive(Debug)]
struct BrokerPaths {
    project_root: PathBuf,
    state_root: PathBuf,
    operator_dir: PathBuf,
    registry: PathBuf,
    audience: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BrokerResult {
    action: String,
    registry_path: String,
    audience: String,
    issuer_id: Option<String>,
    profile: Option<WorkflowBrokerIssuerProfile>,
    public_key_fingerprint: Option<String>,
    issuers: Vec<BrokerStatusRow>,
    trust_boundary: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BrokerStatusRow {
    issuer_id: String,
    profile: WorkflowBrokerIssuerProfile,
    status: WorkflowBrokerIssuerStatus,
    public_key_fingerprint: String,
    ceremony_ref: String,
    ceremony_digest: String,
    declared_at_unix: u64,
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
        "trust" => trust(&flags, None, want_json),
        "rotate" => trust(
            &flags,
            Some(required(&flags, "--replaces")?.to_owned()),
            want_json,
        ),
        "revoke" => revoke(&flags, want_json),
        "status" => status(&flags, want_json),
        _ => Err(ExitError::usage(usage())),
    }
}

fn trust(
    flags: &BTreeMap<String, Vec<String>>,
    replaces: Option<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let issuer_id = required(flags, "--issuer-id")?.trim().to_owned();
    let profile = parse_profile(required(flags, "--profile")?)?;
    let public_key_path = required_path(flags, "--public-key-file")?;
    let ceremony_ref = required(flags, "--ceremony-ref")?.trim().to_owned();
    let ceremony_path = required_path(flags, "--ceremony-file")?;
    require_nonblank("--issuer-id", &issuer_id)?;
    require_nonblank("--ceremony-ref", &ceremony_ref)?;
    let public_key_hex = std::fs::read_to_string(&public_key_path)
        .map_err(|error| {
            ExitError::env_config(format!("read {}: {error}", public_key_path.display()))
        })?
        .trim()
        .to_ascii_lowercase();
    let ceremony_bytes = std::fs::read(&ceremony_path).map_err(|error| {
        ExitError::env_config(format!("read {}: {error}", ceremony_path.display()))
    })?;
    if ceremony_bytes.is_empty() {
        return Err(ExitError::usage(
            "--ceremony-file must contain the operator trust record".to_owned(),
        ));
    }
    let _lock = acquire_lock(&paths)?;
    let mut document = load_or_new(&paths)?;
    if document
        .issuers
        .iter()
        .any(|entry| entry.issuer_id.0 == issuer_id)
    {
        return Err(ExitError::conflict(format!(
            "workflow broker issuer '{issuer_id}' already exists"
        )));
    }
    if let Some(old) = replaces.as_ref() {
        let old_entry = document
            .issuers
            .iter_mut()
            .find(|entry| entry.issuer_id.0 == *old)
            .ok_or_else(|| ExitError::env_config(format!("unknown replaced issuer '{old}'")))?;
        old_entry.status = WorkflowBrokerIssuerStatus::Revoked;
    }
    document.issuers.push(WorkflowBrokerIssuerEntry {
        issuer_id: StableId(issuer_id.clone()),
        profile,
        public_key_hex: public_key_hex.clone(),
        status: WorkflowBrokerIssuerStatus::Active,
        enrollment: WorkflowBrokerEnrollmentDeclaration {
            ceremony_ref,
            ceremony_digest: digest(&ceremony_bytes),
            declared_at_unix: now_unix()?,
        },
    });
    document
        .issuers
        .sort_by(|left, right| left.issuer_id.cmp(&right.issuer_id));
    validate(&document, &paths.audience)?;
    write_registry(&paths.registry, &document)?;
    emit(
        &paths,
        format!(
            "{}_broker_trust",
            if replaces.is_some() {
                "rotated"
            } else {
                "added"
            }
        ),
        Some(&issuer_id),
        Some(profile),
        Some(fingerprint(&public_key_hex)),
        &document,
        want_json,
    )
}

fn revoke(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let issuer_id = required(flags, "--issuer-id")?;
    let _lock = acquire_lock(&paths)?;
    let mut document = load_registry(&paths.registry)?;
    let entry = document
        .issuers
        .iter_mut()
        .find(|entry| entry.issuer_id.0 == issuer_id)
        .ok_or_else(|| ExitError::env_config(format!("unknown broker issuer '{issuer_id}'")))?;
    entry.status = WorkflowBrokerIssuerStatus::Revoked;
    validate(&document, &paths.audience)?;
    write_registry(&paths.registry, &document)?;
    emit(
        &paths,
        "revoked_broker_trust".to_owned(),
        Some(issuer_id),
        None,
        None,
        &document,
        want_json,
    )
}

fn status(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let document = load_registry(&paths.registry)?;
    validate(&document, &paths.audience)?;
    emit(
        &paths,
        "broker_trust_status".to_owned(),
        None,
        None,
        None,
        &document,
        want_json,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit(
    paths: &BrokerPaths,
    action: String,
    issuer_id: Option<&str>,
    profile: Option<WorkflowBrokerIssuerProfile>,
    public_key_fingerprint: Option<String>,
    document: &WorkflowBrokerRegistryDocument,
    want_json: bool,
) -> Result<(), ExitError> {
    let issuers = document
        .issuers
        .iter()
        .map(|entry| BrokerStatusRow {
            issuer_id: entry.issuer_id.0.clone(),
            profile: entry.profile,
            status: entry.status,
            public_key_fingerprint: fingerprint(&entry.public_key_hex),
            ceremony_ref: entry.enrollment.ceremony_ref.clone(),
            ceremony_digest: entry.enrollment.ceremony_digest.clone(),
            declared_at_unix: entry.enrollment.declared_at_unix,
        })
        .collect();
    emit_envelope(
        CliEnvelope::ok(
            "workflow.broker",
            BrokerResult {
                action,
                registry_path: paths.registry.display().to_string(),
                audience: document.audience.clone(),
                issuer_id: issuer_id.map(str::to_owned),
                profile,
                public_key_fingerprint,
                issuers,
                trust_boundary: "Forge stores broker public keys and ceremony digests only; the external host retains private keys and authenticates origin subjects"
                    .to_owned(),
            },
        ),
        want_json,
    )
}

fn broker_paths(root: PathBuf) -> Result<BrokerPaths, ExitError> {
    let project = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(
            "Forge state is missing; run forge-core start before trusting a workflow broker"
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
    let registry = adapter.trusted_broker_registry_path();
    let operator_dir = registry
        .parent()
        .ok_or_else(|| ExitError::env_config("broker registry has no operator parent".to_owned()))?
        .to_path_buf();
    reject_existing_links(&operator_dir)?;
    reject_existing_links(&registry)?;
    let physical = physical_candidate(&operator_dir)?;
    if physical.starts_with(&project_root) || physical.starts_with(&state_root) {
        return Err(ExitError::env_config(
            "workflow broker trust store physically overlaps project or Forge state".to_owned(),
        ));
    }
    Ok(BrokerPaths {
        project_root,
        state_root,
        operator_dir,
        registry,
        audience: format!("forge-core:workflow:{}", project.project_id),
    })
}

fn acquire_lock(paths: &BrokerPaths) -> Result<crate::io_util::DirLock, ExitError> {
    std::fs::create_dir_all(&paths.operator_dir).map_err(|error| {
        ExitError::env_config(format!("create broker operator directory: {error}"))
    })?;
    reject_existing_links(&paths.operator_dir)?;
    let physical = physical_candidate(&paths.operator_dir)?;
    if physical.starts_with(&paths.project_root) || physical.starts_with(&paths.state_root) {
        return Err(ExitError::env_config(
            "workflow broker trust store physically overlaps project or Forge state".to_owned(),
        ));
    }
    crate::io_util::DirLock::acquire(&paths.operator_dir, ".workflow-broker.lock").map_err(
        |error| ExitError::conflict(format!("cannot acquire workflow broker lock: {error}")),
    )
}

fn load_or_new(paths: &BrokerPaths) -> Result<WorkflowBrokerRegistryDocument, ExitError> {
    if paths.registry.exists() {
        return load_registry(&paths.registry);
    }
    Ok(WorkflowBrokerRegistryDocument {
        schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
        audience: paths.audience.clone(),
        issuers: Vec::new(),
    })
}

fn load_registry(path: &Path) -> Result<WorkflowBrokerRegistryDocument, ExitError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| ExitError::env_config(format!("read {}: {error}", path.display())))?;
    yaml_serde::from_str(&raw)
        .map_err(|error| ExitError::env_config(format!("parse {}: {error}", path.display())))
}

fn validate(
    document: &WorkflowBrokerRegistryDocument,
    expected_audience: &str,
) -> Result<(), ExitError> {
    AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
        document.clone(),
        expected_audience,
    )
    .map(|_| ())
    .map_err(|error| ExitError::env_config(format!("invalid workflow broker registry: {error}")))
}

fn write_registry(path: &Path, document: &WorkflowBrokerRegistryDocument) -> Result<(), ExitError> {
    let serialized = yaml_serde::to_string(document)
        .map_err(|error| ExitError::env_config(format!("serialize broker registry: {error}")))?;
    crate::io_util::atomic_write(path, &serialized)
        .map_err(|error| ExitError::env_config(format!("write {}: {error}", path.display())))
}

fn reject_existing_links(path: &Path) -> Result<(), ExitError> {
    for current in path.ancestors() {
        match std::fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ExitError::env_config(format!(
                    "workflow broker path contains a symlink, junction, or reparse-point alias: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ExitError::env_config(format!(
                    "inspect workflow broker path {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn physical_candidate(path: &Path) -> Result<PathBuf, ExitError> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            ExitError::env_config(format!(
                "workflow broker path has no existing ancestor: {}",
                path.display()
            ))
        })?;
    }
    let canonical = std::fs::canonicalize(existing).map_err(|error| {
        ExitError::env_config(format!("canonicalize {}: {error}", existing.display()))
    })?;
    let suffix = path.strip_prefix(existing).map_err(|error| {
        ExitError::env_config(format!("resolve broker path {}: {error}", path.display()))
    })?;
    Ok(canonical.join(suffix))
}

fn parse_profile(value: &str) -> Result<WorkflowBrokerIssuerProfile, ExitError> {
    match value {
        "human" => Ok(WorkflowBrokerIssuerProfile::Human),
        "reviewer" => Ok(WorkflowBrokerIssuerProfile::Reviewer),
        "runtime" => Ok(WorkflowBrokerIssuerProfile::Runtime),
        _ => Err(ExitError::usage(
            "--profile must be human, reviewer, or runtime".to_owned(),
        )),
    }
}

fn parse_flags(action: &str, args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let allowed: &[&str] = match action {
        "trust" => &[
            "--root",
            "--issuer-id",
            "--profile",
            "--public-key-file",
            "--ceremony-ref",
            "--ceremony-file",
        ],
        "rotate" => &[
            "--root",
            "--replaces",
            "--issuer-id",
            "--profile",
            "--public-key-file",
            "--ceremony-ref",
            "--ceremony-file",
        ],
        "revoke" => &["--root", "--issuer-id"],
        "status" => &["--root"],
        _ => return Err(ExitError::usage(usage())),
    };
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        if !allowed.contains(&flag) {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow broker {action}"
            )));
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        if value.starts_with('-') {
            return Err(ExitError::usage(format!(
                "{flag} requires a value, got flag '{value}'"
            )));
        }
        flags
            .entry(flag.to_owned())
            .or_default()
            .push(value.clone());
        index += 1;
    }
    if let Some((flag, _)) = flags.iter().find(|(_, values)| values.len() != 1) {
        return Err(ExitError::usage(format!(
            "{flag} may be supplied only once"
        )));
    }
    Ok(flags)
}

fn required<'a>(
    flags: &'a BTreeMap<String, Vec<String>>,
    flag: &str,
) -> Result<&'a str, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    required(flags, flag).map(PathBuf::from)
}

fn require_nonblank(flag: &'static str, value: &str) -> Result<(), ExitError> {
    if value.trim().is_empty() {
        Err(ExitError::usage(format!("{flag} must not be blank")))
    } else {
        Ok(())
    }
}

fn fingerprint(public_key_hex: &str) -> String {
    digest(public_key_hex.as_bytes())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn now_unix() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| {
            ExitError::env_config(format!("system clock is before Unix epoch: {error}"))
        })
}

fn usage() -> String {
    format!(
        "usage:\n  forge-core {COMMAND} trust --root <project> --issuer-id <id> --profile <human|reviewer|runtime> --public-key-file <hex> --ceremony-ref <ref> --ceremony-file <artifact> [--json|--no-json]\n  forge-core {COMMAND} rotate --root <project> --replaces <old-id> --issuer-id <new-id> --profile <human|reviewer|runtime> --public-key-file <hex> --ceremony-ref <ref> --ceremony-file <artifact> [--json|--no-json]\n  forge-core {COMMAND} revoke --root <project> --issuer-id <id> [--json|--no-json]\n  forge-core {COMMAND} status --root <project> [--json|--no-json]"
    )
}
