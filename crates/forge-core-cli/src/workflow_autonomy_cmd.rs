//! Host-neutral read-only assessment of the solo agent autonomy boundary.

use std::collections::BTreeMap;
use std::path::PathBuf;

use forge_core_contracts::{
    AgentAutonomyAssessmentInput, CliEnvelope, ExitReason, MAX_AGENT_AUTONOMY_INPUT_BYTES,
};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let want_json = match resolve_output_mode(args) {
        Ok(value) => value,
        Err((fallback, message)) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy",
                ExitReason::InvalidDecisionShape,
                message,
                fallback,
            );
        }
    };
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    if action != "assess" {
        return crate::workflow_cmd::emit_failure(
            "workflow.autonomy",
            ExitReason::InvalidDecisionShape,
            format!(
                "unknown workflow autonomy subcommand '{action}'

{}",
                usage()
            ),
            want_json,
        );
    }
    let flags = match parse_flags(&args[1..]) {
        Ok(flags) => flags,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::InvalidDecisionShape,
                error.message().to_owned(),
                want_json,
            );
        }
    };
    let root = match required_path(&flags, "--root") {
        Ok(root) => root,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::InvalidDecisionShape,
                error.message().to_owned(),
                want_json,
            );
        }
    };
    let input_path = match required_path(&flags, "--input-file") {
        Ok(path) => path,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::InvalidDecisionShape,
                error.message().to_owned(),
                want_json,
            );
        }
    };
    let raw = match crate::io_util::read_regular_file_no_follow_bounded(
        &input_path,
        MAX_AGENT_AUTONOMY_INPUT_BYTES,
    ) {
        Ok(raw) => raw,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::InvalidDecisionShape,
                format!(
                    "--input-file {} must be one no-follow regular UTF-8 JSON file no larger than {} bytes: {error}",
                    input_path.display(), MAX_AGENT_AUTONOMY_INPUT_BYTES
                ),
                want_json,
            );
        }
    };
    let input: AgentAutonomyAssessmentInput = match serde_json::from_slice(&raw) {
        Ok(input) => input,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::InvalidDecisionShape,
                format!(
                    "parse agent autonomy assessment input {}: {error}",
                    input_path.display()
                ),
                want_json,
            );
        }
    };
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(message) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.autonomy.assess",
                ExitReason::EnvConfig,
                message,
                want_json,
            );
        }
    };
    match adapter.assess_agent_autonomy(input) {
        Ok(value) => emit_envelope(
            CliEnvelope::ok("workflow.autonomy.assess", value),
            want_json,
        ),
        Err(error) => crate::workflow_cmd::emit_failure(
            "workflow.autonomy.assess",
            crate::workflow_cmd::classify_error(&error),
            error.to_string(),
            want_json,
        ),
    }
}

/// Resolve presentation before validating any other argument. More than one
/// output selector is rejected instead of applying order-dependent precedence.
fn resolve_output_mode(args: &[String]) -> Result<bool, (bool, String)> {
    let selectors = args
        .iter()
        .filter(|arg| matches!(arg.as_str(), "--json" | "--no-json" | "--text"))
        .collect::<Vec<_>>();
    let fallback = selectors
        .first()
        .is_none_or(|value| value.as_str() == "--json");
    if selectors.len() > 1 {
        return Err((
            fallback,
            "exactly zero or one of --json, --no-json, or --text may be supplied".to_owned(),
        ));
    }
    Ok(fallback)
}

fn parse_flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        if !matches!(flag, "--root" | "--input-file") {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow autonomy assess"
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

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(PathBuf::from)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn usage() -> String {
    "usage:
  forge-core workflow autonomy assess --root <project> --input-file <assessment.json> [--json|--no-json|--text]
    read-only: validates the current workflow-next autonomy binding and performs zero Forge state writes"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn output_mode_is_resolved_first_and_conflicts_are_rejected() {
        assert_eq!(resolve_output_mode(&argv(&["bogus", "--text"])), Ok(false));
        assert_eq!(
            resolve_output_mode(&argv(&["bogus", "--no-json"])),
            Ok(false)
        );
        assert!(resolve_output_mode(&argv(&["assess", "--json", "--text"])).is_err());
        assert!(resolve_output_mode(&argv(&["assess", "--json", "--json"])).is_err());
    }

    #[test]
    fn value_flags_are_unique_and_closed() {
        assert!(parse_flags(&argv(&["--root", "a", "--root", "b"])).is_err());
        assert!(parse_flags(&argv(&["--unknown"])).is_err());
    }
}
