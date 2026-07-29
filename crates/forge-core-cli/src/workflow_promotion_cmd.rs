//! Host-neutral CLI for governed isolated-work preview, apply, and recovery.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, StableId, TypedFailure};
use forge_core_kernel::workflow_governance::{PromotionApplyError, WorkflowGovernanceAdapterError};
use std::path::{Path, PathBuf};

const PREVIEW_COMMAND: &str = "workflow.promotion.preview";
const APPLY_COMMAND: &str = "workflow.promotion.apply";
const RECOVER_COMMAND: &str = "workflow.promotion.recover";

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let json_count = args.iter().filter(|arg| arg.as_str() == "--json").count();
    let no_json_count = args
        .iter()
        .filter(|arg| arg.as_str() == "--no-json")
        .count();
    let want_json = no_json_count == 0;
    let command = match args.first().map(String::as_str) {
        Some("apply") => APPLY_COMMAND,
        Some("recover") => RECOVER_COMMAND,
        _ => PREVIEW_COMMAND,
    };
    if json_count > 1 || no_json_count > 1 || (json_count > 0 && no_json_count > 0) {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "output mode flags must be unique and cannot conflict",
            want_json,
        );
    }
    if args
        .first()
        .is_none_or(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!(
            "forge-core workflow promotion preview --root <canonical-project> --isolation-id <id> [--json|--no-json]\n\
             forge-core workflow promotion apply --root <canonical-project> --isolation-id <id> --expected-preview-digest <sha256:...> [--json|--no-json]\n\
             forge-core workflow promotion recover --root <canonical-project> --isolation-id <id> --expected-preview-digest <sha256:...> [--json|--no-json]"
        );
        return Ok(());
    }
    if args
        .first()
        .is_none_or(|arg| !matches!(arg.as_str(), "preview" | "apply" | "recover"))
    {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "unknown workflow promotion subcommand",
            want_json,
        );
    }

    let mut root = None;
    let mut isolation_id = None;
    let mut expected_preview_digest = None;
    let mut root_seen = false;
    let mut isolation_seen = false;
    let mut expected_preview_seen = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                if root_seen {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--root must be supplied exactly once",
                        want_json,
                    );
                }
                root_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--root requires one path value",
                        want_json,
                    );
                };
                root = Some(PathBuf::from(value));
            }
            "--isolation-id" => {
                if isolation_seen {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id must be supplied exactly once",
                        want_json,
                    );
                }
                isolation_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id requires one id value",
                        want_json,
                    );
                };
                if value.trim().is_empty() {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id must not be blank",
                        want_json,
                    );
                }
                isolation_id = Some(StableId(value.clone()));
            }
            "--expected-preview-digest" => {
                if expected_preview_seen {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--expected-preview-digest must be supplied exactly once",
                        want_json,
                    );
                }
                expected_preview_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return failure(
                        command,
                        ExitReason::InvalidDecisionShape,
                        "--expected-preview-digest requires one sha256 digest",
                        want_json,
                    );
                };
                expected_preview_digest = Some(value.clone());
            }
            "--json" | "--no-json" => {}
            other => {
                return failure(
                    command,
                    ExitReason::InvalidDecisionShape,
                    format!("unknown argument {other}"),
                    want_json,
                )
            }
        }
        index += 1;
    }
    let Some(root) = root else {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "--root is required",
            want_json,
        );
    };
    let Some(isolation_id) = isolation_id else {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "--isolation-id is required",
            want_json,
        );
    };
    if command == PREVIEW_COMMAND && expected_preview_digest.is_some() {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "--expected-preview-digest is apply/recover-only",
            want_json,
        );
    }
    if matches!(command, APPLY_COMMAND | RECOVER_COMMAND) && expected_preview_digest.is_none() {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "--expected-preview-digest is required for apply/recover",
            want_json,
        );
    }
    if matches!(command, APPLY_COMMAND | RECOVER_COMMAND)
        && expected_preview_digest
            .as_deref()
            .is_none_or(|digest| !valid_sha256_digest(digest))
    {
        return failure(
            command,
            ExitReason::InvalidDecisionShape,
            "--expected-preview-digest must be exactly sha256: followed by 64 hexadecimal characters",
            want_json,
        );
    }
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(error) if command == RECOVER_COMMAND => {
            return failure_typed(
                command,
                ExitReason::EnvConfig,
                error.clone(),
                TypedFailure::RecoveryRequired {
                    reason: error,
                    can_recover: false,
                    recovery_argv: None,
                },
                want_json,
            );
        }
        Err(error) => return failure(command, ExitReason::EnvConfig, error, want_json),
    };
    if command == PREVIEW_COMMAND {
        return match adapter.preview_promotion(&isolation_id) {
            Ok(preview) => emit_envelope(CliEnvelope::ok(command, preview), want_json),
            Err(error) => failure(
                command,
                ExitReason::RejectedByGate,
                error.to_string(),
                want_json,
            ),
        };
    }
    let Some(expected_preview_digest) = expected_preview_digest else {
        unreachable!("apply/recover digest presence was validated before adapter resolution");
    };
    let result = if command == APPLY_COMMAND {
        adapter.apply_promotion(&isolation_id, &expected_preview_digest)
    } else {
        adapter.recover_promotion(&isolation_id, &expected_preview_digest)
    };
    match result {
        Ok(application) => emit_envelope(CliEnvelope::ok(command, application), want_json),
        Err(error) => match &error {
            WorkflowGovernanceAdapterError::PromotionApply(
                PromotionApplyError::RecoveryRequired(reason),
            ) => {
                let can_recover = command == APPLY_COMMAND;
                let recovery_argv = can_recover
                    .then(|| exact_recovery_argv(&root, &isolation_id, &expected_preview_digest));
                failure_typed(
                    command,
                    ExitReason::Conflict,
                    error.to_string(),
                    TypedFailure::RecoveryRequired {
                        reason: reason.clone(),
                        can_recover,
                        recovery_argv,
                    },
                    want_json,
                )
            }
            _ if command == RECOVER_COMMAND => failure_typed(
                command,
                ExitReason::RejectedByGate,
                error.to_string(),
                TypedFailure::RecoveryRequired {
                    reason: error.to_string(),
                    can_recover: false,
                    recovery_argv: None,
                },
                want_json,
            ),
            _ => failure(
                command,
                ExitReason::RejectedByGate,
                error.to_string(),
                want_json,
            ),
        },
    }
}

fn failure(
    command: &'static str,
    reason: ExitReason,
    message: impl Into<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    let message = message.into();
    if command == RECOVER_COMMAND {
        return failure_typed(
            command,
            reason,
            message.clone(),
            TypedFailure::RecoveryRequired {
                reason: message,
                can_recover: false,
                recovery_argv: None,
            },
            want_json,
        );
    }
    crate::workflow_cmd::emit_failure(command, reason, message, want_json)
}

fn failure_typed(
    command: &'static str,
    reason: ExitReason,
    message: impl Into<String>,
    typed: TypedFailure,
    want_json: bool,
) -> Result<(), ExitError> {
    emit_envelope(
        CliEnvelope::<serde_json::Value>::err_typed(command, reason, message, typed),
        want_json,
    )
}

fn valid_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn exact_recovery_argv(
    root: &Path,
    isolation_id: &StableId,
    expected_preview_digest: &str,
) -> Vec<String> {
    vec![
        "forge-core".to_owned(),
        "workflow".to_owned(),
        "promotion".to_owned(),
        "recover".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
        "--isolation-id".to_owned(),
        isolation_id.0.clone(),
        "--expected-preview-digest".to_owned(),
        expected_preview_digest.to_owned(),
        "--json".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_is_side_effect_free() {
        assert!(run(&["--help".to_owned()]).is_ok());
    }

    #[test]
    fn caller_cannot_substitute_a_source_path() {
        let result = run(&[
            "preview".to_owned(),
            "--root".to_owned(),
            "/canonical".to_owned(),
            "--isolation-id".to_owned(),
            "iso-1".to_owned(),
            "--source-root".to_owned(),
            "/ambient".to_owned(),
            "--json".to_owned(),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn exact_isolation_id_must_be_unique_in_argv() {
        let result = run(&[
            "preview".to_owned(),
            "--root".to_owned(),
            "/canonical".to_owned(),
            "--isolation-id".to_owned(),
            "iso-1".to_owned(),
            "--isolation-id".to_owned(),
            "iso-2".to_owned(),
            "--json".to_owned(),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn apply_requires_exact_preview_digest() {
        let result = run(&[
            "apply".to_owned(),
            "--root".to_owned(),
            "/canonical".to_owned(),
            "--isolation-id".to_owned(),
            "iso-1".to_owned(),
            "--json".to_owned(),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn caller_cannot_supply_principal_or_effect_to_apply() {
        let result = run(&[
            "apply".to_owned(),
            "--root".to_owned(),
            "/canonical".to_owned(),
            "--isolation-id".to_owned(),
            "iso-1".to_owned(),
            "--expected-preview-digest".to_owned(),
            format!("sha256:{}", "a".repeat(64)),
            "--principal".to_owned(),
            "forged".to_owned(),
            "--json".to_owned(),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn malformed_digest_is_rejected_before_project_resolution() {
        let result = run(&[
            "apply".to_owned(),
            "--root".to_owned(),
            "/definitely/not/a/project".to_owned(),
            "--isolation-id".to_owned(),
            "iso-1".to_owned(),
            "--expected-preview-digest".to_owned(),
            "sha256:not-a-digest".to_owned(),
            "--json".to_owned(),
        ]);
        let error = result.expect_err("malformed digest must fail");
        assert_eq!(
            error.exit_code(),
            ExitReason::InvalidDecisionShape.as_code()
        );
    }

    #[test]
    fn recovery_argv_keeps_a_root_with_spaces_as_one_component() {
        let argv = exact_recovery_argv(
            Path::new("/tmp/project with spaces"),
            &StableId("isolation.one".to_owned()),
            &format!("sha256:{}", "a".repeat(64)),
        );
        assert_eq!(argv[5], "/tmp/project with spaces");
        assert_eq!(argv.len(), 11);
        assert_eq!(argv[3], "recover");
        assert_eq!(argv[10], "--json");
    }
}
