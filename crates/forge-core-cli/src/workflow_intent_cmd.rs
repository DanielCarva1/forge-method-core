//! High-level admission of strict human intent or same-owner objective history.
//!
//! The chat host persists neither transcripts nor caller-authored readiness.
//! It forwards one signed, bounded human-broker envelope to the generic broker
//! mutation path, narrowed to the `intent_revision` semantic kind.

use std::collections::BTreeMap;
use std::path::PathBuf;

use forge_core_authority::WorkflowBrokerEventKind;
use forge_core_contracts::{
    CliEnvelope, WorkflowCooperativeObjectiveInput, MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES,
};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    if action == "accept-cooperative" {
        return accept_cooperative(&args[1..]);
    }
    if action != "record" {
        return Err(ExitError::usage(usage()));
    }
    let flags = parse_flags(&args[1..])?;
    let root = required_path(&flags, "--root")?;
    let envelope_path = required_path(&flags, "--origin-envelope-file")?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    crate::workflow_action_cmd::apply_origin_envelope(
        &root,
        &envelope_path,
        Some(WorkflowBrokerEventKind::IntentRevision),
        "workflow.intent.record",
        want_json,
    )
}

fn accept_cooperative(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_cooperative_flags(args)?;
    let root = required_path(&flags, "--root")?;
    let packet_digest = required_value(&flags, "--packet-digest")?;
    if !is_lower_sha256(&packet_digest) {
        return Err(ExitError::invalid_value(
            "--packet-digest must be a canonical lowercase sha256:<64-hex> digest",
        ));
    }
    let input_path = required_path(&flags, "--input-file")?;
    let raw = crate::io_util::read_regular_file_no_follow_bounded(
        &input_path,
        MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES,
    )
    .map_err(|error| {
        ExitError::invalid_value(format!(
            "--input-file {} must be one no-follow regular UTF-8 JSON file no larger than {} bytes: {error}",
            input_path.display(),
            MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES
        ))
    })?;
    let input: WorkflowCooperativeObjectiveInput =
        serde_json::from_slice(&raw).map_err(|error| {
            ExitError::invalid_value(format!(
                "parse cooperative objective input {}: {error}",
                input_path.display()
            ))
        })?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(message) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.intent.accept_cooperative",
                forge_core_contracts::ExitReason::EnvConfig,
                message,
                want_json,
            );
        }
    };
    match adapter.accept_cooperative_objective(&packet_digest, input) {
        Ok(value) => emit_envelope(
            CliEnvelope::ok("workflow.intent.accept_cooperative", value),
            want_json,
        ),
        Err(error) => crate::workflow_cmd::emit_failure(
            "workflow.intent.accept_cooperative",
            crate::workflow_cmd::classify_error(&error),
            error.to_string(),
            want_json,
        ),
    }
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
        if !matches!(flag, "--root" | "--origin-envelope-file") {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow intent record"
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

fn parse_cooperative_flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        if !matches!(flag, "--root" | "--packet-digest" | "--input-file") {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow intent accept-cooperative"
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

fn required_value(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<String, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn is_lower_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(PathBuf::from)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn usage() -> String {
    "usage:\n  forge-core workflow intent accept-cooperative --root <project> --packet-digest <sha256> --input-file <cooperative-input.json> [--json|--no-json]\n    input contract: use the cooperative_objective UTF-8 JSON templates and limits from the current packet\n  forge-core workflow intent record --root <project> --origin-envelope-file <signed-json> [--json|--no-json]"
        .to_owned()
}
