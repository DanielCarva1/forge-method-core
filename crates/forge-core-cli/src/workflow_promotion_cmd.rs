//! Host-neutral CLI for read-only governed-promotion previews.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, StableId};
use std::path::PathBuf;

const COMMAND: &str = "workflow.promotion.preview";

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let json_count = args.iter().filter(|arg| arg.as_str() == "--json").count();
    let no_json_count = args
        .iter()
        .filter(|arg| arg.as_str() == "--no-json")
        .count();
    let want_json = no_json_count == 0;
    if json_count > 1 || no_json_count > 1 || (json_count > 0 && no_json_count > 0) {
        return failure(
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
            "forge-core workflow promotion preview --root <canonical-project> --isolation-id <id> [--json|--no-json]"
        );
        return Ok(());
    }
    if args.first().is_none_or(|arg| arg != "preview") {
        return failure(
            ExitReason::InvalidDecisionShape,
            "unknown workflow promotion subcommand",
            want_json,
        );
    }

    let mut root = None;
    let mut isolation_id = None;
    let mut root_seen = false;
    let mut isolation_seen = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                if root_seen {
                    return failure(
                        ExitReason::InvalidDecisionShape,
                        "--root must be supplied exactly once",
                        want_json,
                    );
                }
                root_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return failure(
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
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id must be supplied exactly once",
                        want_json,
                    );
                }
                isolation_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return failure(
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id requires one id value",
                        want_json,
                    );
                };
                if value.trim().is_empty() {
                    return failure(
                        ExitReason::InvalidDecisionShape,
                        "--isolation-id must not be blank",
                        want_json,
                    );
                }
                isolation_id = Some(StableId(value.clone()));
            }
            "--json" | "--no-json" => {}
            other => {
                return failure(
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
            ExitReason::InvalidDecisionShape,
            "--root is required",
            want_json,
        );
    };
    let Some(isolation_id) = isolation_id else {
        return failure(
            ExitReason::InvalidDecisionShape,
            "--isolation-id is required",
            want_json,
        );
    };
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(error) => return failure(ExitReason::EnvConfig, error, want_json),
    };
    match adapter.preview_promotion(&isolation_id) {
        Ok(preview) => emit_envelope(CliEnvelope::ok(COMMAND, preview), want_json),
        Err(error) => failure(ExitReason::RejectedByGate, error.to_string(), want_json),
    }
}

fn failure(
    reason: ExitReason,
    message: impl Into<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    crate::workflow_cmd::emit_failure(COMMAND, reason, message.into(), want_json)
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
}
