//! `forge-core restore preflight|apply` command adapter.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, StableId};
use forge_core_kernel::WorkflowGovernanceProjectAdapter;
use forge_core_store::backup::BackupDestinationPlatform;
use forge_core_store::restore::{
    apply_project_restore, plan_project_restore, preflight_project_restore, RestoreError,
    RestorePlanRequest,
};
use serde::Serialize;
use std::path::PathBuf;

const COMMAND_PREFLIGHT: &str = "restore.preflight";
const COMMAND_APPLY: &str = "restore.apply";

#[derive(Debug, Serialize)]
pub struct RestorePreflightPayload {
    pub preflighted: bool,
    pub destination_sidecar: PathBuf,
    pub archive_sha256: String,
    pub manifest_set_digest: String,
    pub member_count: usize,
    pub destination_already_published: bool,
}

#[derive(Debug, Serialize)]
pub struct RestoreApplyPayload {
    pub restored: bool,
    pub destination_sidecar: PathBuf,
    pub archive_sha256: String,
    pub manifest_set_digest: String,
    pub receipt_path: PathBuf,
    pub receipt_digest: String,
    pub member_count: usize,
    pub already_restored: bool,
}

#[derive(Debug)]
struct RestoreArgs {
    project_root: PathBuf,
    archive: PathBuf,
    authority: String,
    destination_platform: BackupDestinationPlatform,
    principal_registry: Option<PathBuf>,
    broker_registry: Option<PathBuf>,
    json: bool,
}

struct RestoreRegistryAuthorities {
    _broker: Option<crate::workflow_broker_cmd::LockedWorkflowBrokerRegistry>,
    _credential: Option<crate::workflow_credential_cmd::LockedWorkflowCredentialRegistry>,
}

/// Run the public restore command family.
///
/// # Errors
///
/// Returns usage errors for malformed argv and emits one typed non-success
/// envelope for rejected plans, preflights, or publications.
pub fn run_restore_command(args: &[String]) -> Result<(), ExitError> {
    let Some(subcommand) = args.get(1).map(String::as_str) else {
        return Err(ExitError::usage(restore_usage()));
    };
    if matches!(subcommand, "--help" | "-h" | "help") {
        println!("{}", restore_usage());
        return Ok(());
    }
    if args
        .get(2)
        .is_some_and(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", restore_usage());
        return Ok(());
    }
    let parsed = parse_args(args)?;
    match subcommand {
        "preflight" => run_preflight(parsed),
        "apply" => run_apply(parsed),
        _ => Err(ExitError::usage(restore_usage())),
    }
}

fn run_preflight(args: RestoreArgs) -> Result<(), ExitError> {
    let json = args.json;
    let envelope = match plan_and_preflight(args) {
        Ok((preflight, _authorities)) => CliEnvelope::ok(
            COMMAND_PREFLIGHT,
            RestorePreflightPayload {
                preflighted: true,
                destination_sidecar: preflight.destination_sidecar().to_path_buf(),
                archive_sha256: preflight.archive_sha256().to_owned(),
                manifest_set_digest: preflight.manifest_set_digest().to_owned(),
                member_count: preflight.member_count(),
                destination_already_published: preflight.destination_already_published(),
            },
        ),
        Err(error) => CliEnvelope::err(
            COMMAND_PREFLIGHT,
            restore_error_reason(&error),
            error.to_string(),
        ),
    };
    emit_envelope(envelope, json)
}

fn run_apply(args: RestoreArgs) -> Result<(), ExitError> {
    let json = args.json;
    let envelope = match plan_and_preflight(args)
        .and_then(|(preflight, _authorities)| apply_project_restore(preflight))
    {
        Ok(publication) => CliEnvelope::ok(
            COMMAND_APPLY,
            RestoreApplyPayload {
                restored: true,
                destination_sidecar: publication.destination_sidecar,
                archive_sha256: publication.archive_sha256,
                manifest_set_digest: publication.manifest_set_digest,
                receipt_path: publication.receipt_path,
                receipt_digest: publication.receipt_digest,
                member_count: publication.member_count,
                already_restored: publication.already_restored,
            },
        ),
        Err(error) => CliEnvelope::err(
            COMMAND_APPLY,
            restore_error_reason(&error),
            error.to_string(),
        ),
    };
    emit_envelope(envelope, json)
}

fn path_occupied_nofollow(path: &std::path::Path) -> Result<bool, RestoreError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: format!("cannot inspect current workflow registry: {error}"),
        }),
    }
}

fn plan_and_preflight(
    args: RestoreArgs,
) -> Result<
    (
        forge_core_store::restore::RestorePreflight,
        RestoreRegistryAuthorities,
    ),
    RestoreError,
> {
    let project = crate::project_cmd::resolve_project(&args.project_root).map_err(|error| {
        RestoreError::InvalidPath {
            path: args.project_root.clone(),
            reason: format!("cannot resolve Project Link: {error}"),
        }
    })?;
    let project_root = PathBuf::from(&project.project_root);
    let (principal_registry, broker_registry, authorities) = if project.state_exists {
        let state_root = PathBuf::from(&project.state_root);
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId(project.project_id),
            &project_root,
            &state_root,
        )
        .map_err(|error| RestoreError::InvalidPath {
            path: project_root.clone(),
            reason: format!("cannot bind workflow governance: {error}"),
        })?;
        // Restore already retains exclusive producer quiescence for the exact
        // state-root inode. Reusing the backup-only Domain Pack snapshot helper
        // here would create recovery placeholders inside a completed restore
        // before its exact tree can be verified.
        let principal_registry_path = adapter.trusted_principal_registry_path();
        let credential = if args.principal_registry.is_some()
            || path_occupied_nofollow(&principal_registry_path)?
        {
            Some(
                crate::workflow_credential_cmd::lock_workflow_credential_registry(&project_root)
                    .map_err(|error| RestoreError::InvalidPath {
                        path: project_root.clone(),
                        reason: error.to_string(),
                    })?,
            )
        } else {
            None
        };
        let credential_present = credential
            .as_ref()
            .map(crate::workflow_credential_cmd::snapshot_workflow_credential_registry)
            .transpose()
            .map_err(|error| RestoreError::Tampered {
                reason: error.to_string(),
            })?
            .is_some_and(|snapshot| snapshot.raw_registry().is_some());

        let broker_registry_path = adapter.trusted_broker_registry_path();
        let broker =
            if args.broker_registry.is_some() || path_occupied_nofollow(&broker_registry_path)? {
                Some(
                    crate::workflow_broker_cmd::lock_workflow_broker_registry(&project_root)
                        .map_err(|error| RestoreError::InvalidPath {
                            path: project_root.clone(),
                            reason: error.to_string(),
                        })?,
                )
            } else {
                None
            };
        let broker_present = broker
            .as_ref()
            .map(crate::workflow_broker_cmd::snapshot_workflow_broker_registry)
            .transpose()
            .map_err(|error| RestoreError::Tampered {
                reason: error.to_string(),
            })?
            .is_some_and(|snapshot| snapshot.raw_registry().is_some());

        let principal_registry = crate::backup_cmd::registry_capture_path(
            args.principal_registry.as_deref(),
            &principal_registry_path,
            credential_present,
            "workflow principal registry",
        )
        .map_err(|error| RestoreError::Tampered {
            reason: error.to_string(),
        })?;
        let broker_registry = crate::backup_cmd::registry_capture_path(
            args.broker_registry.as_deref(),
            &broker_registry_path,
            broker_present,
            "workflow broker registry",
        )
        .map_err(|error| RestoreError::Tampered {
            reason: error.to_string(),
        })?;
        (
            principal_registry,
            broker_registry,
            RestoreRegistryAuthorities {
                _credential: credential,
                _broker: broker,
            },
        )
    } else {
        if args.principal_registry.is_some() || args.broker_registry.is_some() {
            return Err(RestoreError::Tampered {
                reason: "current workflow registries cannot be supplied when the linked destination state is absent"
                    .to_owned(),
            });
        }
        (
            None,
            None,
            RestoreRegistryAuthorities {
                _credential: None,
                _broker: None,
            },
        )
    };
    let plan = plan_project_restore(RestorePlanRequest {
        project_root,
        archive_path: args.archive,
        authority_id: args.authority,
        destination_platform: args.destination_platform,
        current_principal_registry: principal_registry,
        current_broker_registry: broker_registry,
    })?;
    preflight_project_restore(plan).map(|preflight| (preflight, authorities))
}

fn restore_error_reason(error: &RestoreError) -> ExitReason {
    match error {
        RestoreError::Collision { .. } | RestoreError::Interrupted { .. } => ExitReason::Conflict,
        RestoreError::InvalidPath { .. } | RestoreError::Io { .. } => ExitReason::EnvConfig,
        RestoreError::Backup(_) | RestoreError::Rollback { .. } | RestoreError::Tampered { .. } => {
            ExitReason::RejectedByGate
        }
        _ => ExitReason::RejectedByGate,
    }
}

fn parse_args(args: &[String]) -> Result<RestoreArgs, ExitError> {
    let mut project_root = None;
    let mut archive = None;
    let mut authority = None;
    let mut destination_platform = None;
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
                    return Err(ExitError::usage(restore_usage()));
                };
                if authority.replace(value.clone()).is_some() {
                    return Err(ExitError::usage(restore_usage()));
                }
                index += 1;
                continue;
            }
            "--platform" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(restore_usage()));
                };
                let parsed = match value.as_str() {
                    "posix" => BackupDestinationPlatform::Posix,
                    "windows" => BackupDestinationPlatform::Windows,
                    _ => return Err(ExitError::usage(restore_usage())),
                };
                if destination_platform.replace(parsed).is_some() {
                    return Err(ExitError::usage(restore_usage()));
                }
                index += 1;
                continue;
            }
            _ => {}
        }
        let slot = match args[index].as_str() {
            "--root" => &mut project_root,
            "--archive" => &mut archive,
            "--principal-registry" => &mut principal_registry,
            "--broker-registry" => &mut broker_registry,
            _ => return Err(ExitError::usage(restore_usage())),
        };
        index += 1;
        let Some(value) = args.get(index) else {
            return Err(ExitError::usage(restore_usage()));
        };
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(ExitError::usage(restore_usage()));
        }
        index += 1;
    }
    Ok(RestoreArgs {
        project_root: project_root.ok_or_else(|| ExitError::usage(restore_usage()))?,
        archive: archive.ok_or_else(|| ExitError::usage(restore_usage()))?,
        authority: authority.ok_or_else(|| ExitError::usage(restore_usage()))?,
        destination_platform: destination_platform.unwrap_or_else(host_platform),
        principal_registry,
        broker_registry,
        json,
    })
}

#[cfg(windows)]
const fn host_platform() -> BackupDestinationPlatform {
    BackupDestinationPlatform::Windows
}

#[cfg(not(windows))]
const fn host_platform() -> BackupDestinationPlatform {
    BackupDestinationPlatform::Posix
}

fn restore_usage() -> String {
    forge_core_command_surface::COMMAND_RESTORE
        .local_usage_lines()
        .collect::<Vec<_>>()
        .join("\n")
}
