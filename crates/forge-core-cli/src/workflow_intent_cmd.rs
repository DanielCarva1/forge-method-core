//! High-level admission of a human-origin workflow intent revision.
//!
//! The chat host persists neither transcripts nor caller-authored readiness.
//! It forwards one signed, bounded human-broker envelope to the generic broker
//! mutation path, narrowed to the `intent_revision` semantic kind.

use std::collections::BTreeMap;
use std::path::PathBuf;

use forge_core_authority::WorkflowBrokerEventKind;

use crate::cli_error::ExitError;

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
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

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(PathBuf::from)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn usage() -> String {
    "usage:\n  forge-core workflow intent record --root <project> --origin-envelope-file <signed-json> [--json|--no-json]"
        .to_owned()
}
