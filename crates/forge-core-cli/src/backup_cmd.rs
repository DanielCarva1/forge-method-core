//! `forge-core backup create|verify` command adapter.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, StableId};
use forge_core_kernel::WorkflowGovernanceProjectAdapter;
use forge_core_store::backup::{
    create_project_backup, verify_project_backup, BackupCreateRequest, BackupError,
    BackupExpectedMember, BackupGovernanceProjection, BackupVerifyRequest,
};
use forge_core_store::producer_quiescence::quiesce_host_producers;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

const COMMAND_CREATE: &str = "backup.create";
const COMMAND_VERIFY: &str = "backup.verify";

#[derive(Debug, Serialize)]
pub struct BackupVerifyPayload {
    pub verified: bool,
    pub archive_path: PathBuf,
    pub archive_sha256: String,
    pub member_count: usize,
    pub manifest_set_digest: String,
    pub project_id: String,
    pub project_link_sha256: String,
    pub workflow_release_id: String,
    pub workflow_release_version: String,
    pub workflow_release_digest: String,
    pub effective_epoch_id: String,
    pub effective_epoch_generation: u64,
    pub replay_generation: u64,
    pub receipt_digest: String,
}

#[derive(Debug, Serialize)]
pub struct BackupCreatePayload {
    pub archive_path: PathBuf,
    pub archive_sha256: String,
    pub receipt_path: PathBuf,
    pub receipt_digest: String,
    pub manifest_set_digest: String,
    pub member_count: usize,
    pub already_published: bool,
    pub project_id: String,
    pub workflow_release_id: String,
    pub workflow_release_version: String,
    pub workflow_release_digest: String,
    pub effective_epoch_id: String,
    pub effective_epoch_generation: u64,
}

#[derive(Debug)]
struct BackupArgs {
    root: PathBuf,
    archive: PathBuf,
    authority: String,
    principal_registry: Option<PathBuf>,
    broker_registry: Option<PathBuf>,
    json: bool,
}

/// Run the public backup command family.
///
/// # Errors
///
/// Returns usage errors for malformed argv and typed exit errors after emitting
/// a non-success envelope for verification/configuration failures.
pub fn run_backup_command(args: &[String]) -> Result<(), ExitError> {
    let Some(subcommand) = args.get(1).map(String::as_str) else {
        return Err(ExitError::usage(backup_usage()));
    };
    if matches!(subcommand, "--help" | "-h" | "help") {
        println!("{}", backup_usage());
        return Ok(());
    }
    if args
        .get(2)
        .is_some_and(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", backup_usage());
        return Ok(());
    }
    let parsed = parse_args(args)?;
    match subcommand {
        "verify" => run_verify(parsed),
        "create" => run_create(parsed),
        _ => Err(ExitError::usage(backup_usage())),
    }
}

fn run_create(args: BackupArgs) -> Result<(), ExitError> {
    let project = crate::project_cmd::resolve_project(&args.root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(
            "backup create requires an existing linked Forge state root".to_owned(),
        ));
    }
    let project_root = PathBuf::from(&project.project_root);
    let state_root = PathBuf::from(&project.state_root);
    let adapter = WorkflowGovernanceProjectAdapter::new(
        StableId(project.project_id.clone()),
        &project_root,
        &state_root,
    )
    .map_err(|error| ExitError::env_config(format!("bind workflow governance: {error}")))?;
    let guidance = adapter
        .next()
        .map_err(|error| ExitError::env_config(format!("derive workflow authority: {error}")))?;

    let domain_pack =
        crate::domain_pack_cmd::lock_domain_pack_backup_authorities(&state_root, now_unix()?)?;
    let credential =
        crate::workflow_credential_cmd::lock_workflow_credential_registry(&project_root)?;
    let broker = crate::workflow_broker_cmd::lock_workflow_broker_registry(&project_root)?;
    let claims_dir = state_root.join("claims-active");
    let claim_cache = if claims_dir.is_dir() {
        Some(
            crate::claim::acquire_claim_cache_authority(&claims_dir).map_err(|error| {
                ExitError::conflict(format!("cannot lock claim cache for backup: {error}"))
            })?,
        )
    } else {
        None
    };
    let isolations_dir = state_root.join("contracts/isolations");
    let isolations = if isolations_dir.is_dir() {
        Some(
            crate::isolation::acquire_isolation_contracts_authority(&isolations_dir).map_err(
                |error| {
                    ExitError::conflict(format!(
                        "cannot lock isolation contracts for backup: {error}"
                    ))
                },
            )?,
        )
    } else {
        None
    };
    let quiescence =
        quiesce_host_producers(&state_root, &AtomicBool::new(false)).map_err(|error| {
            ExitError::conflict(format!("cannot quiesce project producers: {error}"))
        })?;
    let event_logs = forge_core_eventlog::capture_quiesced_event_logs(&state_root, &quiescence)
        .map_err(|error| ExitError::failed(format!("capture EventLog streams: {error}")))?;

    let mut expected_members = domain_pack.expected_members().to_vec();
    if let Some(locked) = claim_cache.as_ref() {
        let snapshot =
            crate::claim::snapshot_claim_cache_under_authority(locked).map_err(|error| {
                ExitError::failed(format!("snapshot claim cache for backup: {error}"))
            })?;
        expected_members.extend(snapshot.entries().iter().map(|entry| BackupExpectedMember {
            logical_path: format!(
                "sidecar/.forge-method/claims-active/{}",
                entry.relative_path()
            ),
            sha256: entry.raw_sha256().to_owned(),
        }));
    }
    if let Some(locked) = isolations.as_ref() {
        let snapshot = crate::isolation::snapshot_isolation_contracts_under_authority(locked)
            .map_err(|error| {
                ExitError::failed(format!("snapshot isolation contracts for backup: {error}"))
            })?;
        expected_members.extend(snapshot.entries().iter().map(|entry| BackupExpectedMember {
            logical_path: format!(
                "sidecar/.forge-method/contracts/isolations/{}",
                entry.relative_path()
            ),
            sha256: entry.raw_sha256().to_owned(),
        }));
    }
    let credential_snapshot =
        crate::workflow_credential_cmd::snapshot_workflow_credential_registry(&credential)?;
    let broker_snapshot = crate::workflow_broker_cmd::snapshot_workflow_broker_registry(&broker)?;
    if let Some(digest) = credential_snapshot.raw_sha256() {
        expected_members.push(BackupExpectedMember {
            logical_path: "sidecar/operator/workflow-principal-registry.yaml".to_owned(),
            sha256: digest.to_owned(),
        });
    }
    if let Some(digest) = broker_snapshot.raw_sha256() {
        expected_members.push(BackupExpectedMember {
            logical_path: "sidecar/operator/workflow-broker-registry.yaml".to_owned(),
            sha256: digest.to_owned(),
        });
    }
    expected_members.extend(event_logs.members().map(|member| BackupExpectedMember {
        logical_path: format!("sidecar/.forge-method/{}", member.relative_path()),
        sha256: sha256(member.bytes()),
    }));

    let principal_registry = registry_capture_path(
        args.principal_registry.as_deref(),
        &adapter.trusted_principal_registry_path(),
        credential_snapshot.raw_registry().is_some(),
        "workflow principal registry",
    )?;
    let broker_registry = registry_capture_path(
        args.broker_registry.as_deref(),
        &adapter.trusted_broker_registry_path(),
        broker_snapshot.raw_registry().is_some(),
        "workflow broker registry",
    )?;
    let effective_epoch_id = format!("workflow-effective:{}", project.project_id);
    let effective_epoch_generation = guidance.state_version.max(1);
    let governance = BackupGovernanceProjection {
        workflow_release: guidance.release.release.clone(),
        effective_bundle: guidance.effective.clone(),
        state_version: guidance.state_version,
        governance_ledger_head_digest: guidance.ledger_head_digest.clone(),
    };
    let request = BackupCreateRequest {
        project_root,
        archive_path: args.archive,
        authority_id: args.authority,
        governance: governance.clone(),
        current_principal_registry: principal_registry,
        current_broker_registry: broker_registry,
        expected_members,
    };
    let envelope = match create_project_backup(&request, &quiescence) {
        Ok(publication) => CliEnvelope::ok(
            COMMAND_CREATE,
            BackupCreatePayload {
                archive_path: publication.archive_path,
                archive_sha256: publication.archive_sha256,
                receipt_path: publication.receipt_path,
                receipt_digest: publication.receipt_digest,
                manifest_set_digest: publication.manifest_set_digest,
                member_count: publication.member_count,
                already_published: publication.already_published,
                project_id: project.project_id,
                workflow_release_id: governance.workflow_release.release_id.0,
                workflow_release_version: governance.workflow_release.release_version,
                workflow_release_digest: governance.workflow_release.release_digest,
                effective_epoch_id,
                effective_epoch_generation,
            },
        ),
        Err(error) => CliEnvelope::err(
            COMMAND_CREATE,
            backup_error_reason(&error),
            error.to_string(),
        ),
    };
    emit_envelope(envelope, args.json)
}

pub(crate) fn registry_capture_path(
    supplied: Option<&Path>,
    derived: &Path,
    present: bool,
    label: &str,
) -> Result<Option<PathBuf>, ExitError> {
    if let Some(supplied) = supplied {
        let supplied = std::fs::canonicalize(supplied).map_err(|error| {
            ExitError::env_config(format!("canonicalize supplied {label}: {error}"))
        })?;
        let derived = std::fs::canonicalize(derived).map_err(|error| {
            ExitError::env_config(format!("canonicalize derived {label}: {error}"))
        })?;
        if supplied != derived {
            return Err(ExitError::env_config(format!(
                "supplied {label} differs from the project-bound producer authority"
            )));
        }
    }
    Ok(present.then(|| derived.to_path_buf()))
}

pub(crate) fn now_unix() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| ExitError::env_config(format!("system clock error: {error}")))
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn run_verify(args: BackupArgs) -> Result<(), ExitError> {
    let project = crate::project_cmd::resolve_project(&args.root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(
            "backup verify requires an existing linked Forge state root".to_owned(),
        ));
    }
    let project_root = PathBuf::from(&project.project_root);
    let state_root = PathBuf::from(&project.state_root);
    let adapter = WorkflowGovernanceProjectAdapter::new(
        StableId(project.project_id),
        &project_root,
        &state_root,
    )
    .map_err(|error| ExitError::env_config(format!("bind workflow governance: {error}")))?;
    let _domain_pack =
        crate::domain_pack_cmd::lock_domain_pack_backup_authorities(&state_root, now_unix()?)?;
    let credential =
        crate::workflow_credential_cmd::lock_workflow_credential_registry(&project_root)?;
    let broker = crate::workflow_broker_cmd::lock_workflow_broker_registry(&project_root)?;
    let credential_snapshot =
        crate::workflow_credential_cmd::snapshot_workflow_credential_registry(&credential)?;
    let broker_snapshot = crate::workflow_broker_cmd::snapshot_workflow_broker_registry(&broker)?;
    let principal_registry = registry_capture_path(
        args.principal_registry.as_deref(),
        &adapter.trusted_principal_registry_path(),
        credential_snapshot.raw_registry().is_some(),
        "workflow principal registry",
    )?;
    let broker_registry = registry_capture_path(
        args.broker_registry.as_deref(),
        &adapter.trusted_broker_registry_path(),
        broker_snapshot.raw_registry().is_some(),
        "workflow broker registry",
    )?;
    let request = BackupVerifyRequest {
        project_root,
        archive_path: args.archive,
        authority_id: args.authority,
        current_principal_registry: principal_registry,
        current_broker_registry: broker_registry,
    };
    let envelope = match verify_project_backup(&request) {
        Ok(verified) => {
            let manifest = &verified.manifest().backup_manifest;
            let receipt = &verified.receipt().backup_receipt;
            CliEnvelope::ok(
                COMMAND_VERIFY,
                BackupVerifyPayload {
                    verified: true,
                    archive_path: verified.archive_path().to_path_buf(),
                    archive_sha256: verified.archive_sha256().to_owned(),
                    member_count: verified.member_count(),
                    manifest_set_digest: manifest.manifest_set_digest.clone(),
                    project_id: manifest.project.project_link.project_id.0.clone(),
                    project_link_sha256: manifest.project.project_link_sha256.clone(),
                    workflow_release_id: manifest.workflow_release.release_id.0.clone(),
                    workflow_release_version: manifest.workflow_release.release_version.clone(),
                    workflow_release_digest: manifest.workflow_release.release_digest.clone(),
                    effective_epoch_id: manifest.effective_epoch.epoch_id.clone(),
                    effective_epoch_generation: manifest.effective_epoch.epoch_generation,
                    replay_generation: receipt.replay_monotonic_head.generation,
                    receipt_digest: receipt.receipt_digest.clone(),
                },
            )
        }
        Err(error) => CliEnvelope::err(
            COMMAND_VERIFY,
            backup_error_reason(&error),
            error.to_string(),
        ),
    };
    emit_envelope(envelope, args.json)
}

fn backup_error_reason(error: &BackupError) -> ExitReason {
    match error {
        BackupError::ExistingDifferent { .. } => ExitReason::Conflict,
        BackupError::InvalidPath { .. } | BackupError::Io { .. } => ExitReason::EnvConfig,
        _ => ExitReason::RejectedByGate,
    }
}

fn parse_args(args: &[String]) -> Result<BackupArgs, ExitError> {
    let mut root = None;
    let mut archive = None;
    let mut authority = None;
    let mut principal_registry = None;
    let mut broker_registry = None;
    let mut json = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
                continue;
            }
            "--no-json" => {
                json = false;
                index += 1;
                continue;
            }
            "--authority" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(backup_usage()));
                };
                if authority.replace(value.clone()).is_some() {
                    return Err(ExitError::usage(backup_usage()));
                }
                index += 1;
                continue;
            }
            _ => {}
        }
        let slot = match args[index].as_str() {
            "--root" => &mut root,
            "--archive" => &mut archive,
            "--principal-registry" => &mut principal_registry,
            "--broker-registry" => &mut broker_registry,
            _ => return Err(ExitError::usage(backup_usage())),
        };
        index += 1;
        let Some(value) = args.get(index) else {
            return Err(ExitError::usage(backup_usage()));
        };
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(ExitError::usage(backup_usage()));
        }
        index += 1;
    }
    Ok(BackupArgs {
        root: root.ok_or_else(|| ExitError::usage(backup_usage()))?,
        archive: archive.ok_or_else(|| ExitError::usage(backup_usage()))?,
        authority: authority.ok_or_else(|| ExitError::usage(backup_usage()))?,
        principal_registry,
        broker_registry,
        json,
    })
}

fn backup_usage() -> String {
    forge_core_command_surface::COMMAND_BACKUP
        .local_usage_lines()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parser_requires_authority_selector_and_rejects_direct_trust_paths() {
        assert!(parse_args(&argv(&["backup", "verify"])).is_err());
        assert!(parse_args(&argv(&[
            "backup",
            "verify",
            "--root",
            ".",
            "--archive",
            "a",
            "--authority",
            "production",
            "--receipt-store",
            "attacker-receipts",
        ]))
        .is_err());
    }

    #[test]
    fn parser_accepts_exact_verify_surface() {
        let parsed = parse_args(&argv(&[
            "backup",
            "verify",
            "--root",
            ".",
            "--archive",
            "a",
            "--authority",
            "production",
            "--json",
        ]))
        .expect("parse");
        assert!(parsed.json);
        assert_eq!(parsed.authority, "production");
    }
}
