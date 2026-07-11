//! Operator-facing external replay anchor lifecycle for P4b.5a.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_store::replay_anchor::{
    advance_replay_anchor, provision_replay_anchor, verify_replay_anchor,
};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "mcp replay-anchor";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Provision,
    Verify,
    Advance,
}

pub(crate) fn run_replay_anchor_command(args: &[String]) -> Result<(), ExitError> {
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    let result = parse(args).and_then(|(action, flags)| execute(action, &flags));
    match result {
        Ok(value) => emit_envelope(CliEnvelope::ok(COMMAND, value), want_json),
        Err(error) => emit_envelope(
            CliEnvelope::<()>::err(COMMAND, ExitReason::EnvConfig, error.to_string()),
            want_json,
        ),
    }
}

fn execute(
    action: Action,
    flags: &BTreeMap<String, String>,
) -> Result<serde_json::Value, ExitError> {
    let root = canonical(required_path(flags, "--root")?, "project root")?;
    let resolved = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("Project Link failed: {error}")))?;
    let state_root = canonical(PathBuf::from(resolved.state_root), "state root")?;
    let anchor_path = canonical_external_target(required_path(flags, "--anchor")?)?;
    crate::mcp_credential_cmd::ensure_operator_owned_location(&anchor_path, &root, &state_root)?;
    let result = match action {
        Action::Provision => {
            let deployment_id = required(flags, "--deployment-id")?;
            serde_json::to_value(
                provision_replay_anchor(&state_root, &anchor_path, deployment_id)
                    .map_err(|error| ExitError::env_config(error.to_string()))?,
            )
        }
        Action::Verify => serde_json::to_value(
            verify_replay_anchor(&state_root, &anchor_path)
                .map_err(|error| ExitError::env_config(error.to_string()))?,
        ),
        Action::Advance => serde_json::to_value(
            advance_replay_anchor(&state_root, &anchor_path)
                .map_err(|error| ExitError::env_config(error.to_string()))?,
        ),
    };
    result.map_err(|error| ExitError::env_config(error.to_string()))
}

fn parse(args: &[String]) -> Result<(Action, BTreeMap<String, String>), ExitError> {
    let action = match args.first().map(String::as_str) {
        Some("provision") => Action::Provision,
        Some("verify") => Action::Verify,
        Some("advance") => Action::Advance,
        Some(other) => {
            return Err(ExitError::usage(format!(
                "unknown replay-anchor action '{other}'"
            )));
        }
        None => return Err(ExitError::usage("replay-anchor action is required")),
    };
    let mut flags = BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        if matches!(args[index].as_str(), "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        let flag = args[index].clone();
        if !matches!(flag.as_str(), "--root" | "--anchor" | "--deployment-id") {
            return Err(ExitError::usage(format!(
                "unknown replay-anchor flag '{flag}'"
            )));
        }
        index += 1;
        let value = args
            .get(index)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        if flags.insert(flag.clone(), value.clone()).is_some() {
            return Err(ExitError::usage(format!(
                "duplicate replay-anchor flag '{flag}'"
            )));
        }
        index += 1;
    }
    if action != Action::Provision && flags.contains_key("--deployment-id") {
        return Err(ExitError::usage(
            "--deployment-id is valid only for replay-anchor provision",
        ));
    }
    Ok((action, flags))
}

fn canonical_external_target(path: PathBuf) -> Result<PathBuf, ExitError> {
    let path = crate::mcp_credential_cmd::absolute_path(path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| ExitError::env_config("anchor file name is required"))?;
    let parent = path
        .parent()
        .ok_or_else(|| ExitError::env_config("anchor parent is required"))?;
    let parent = canonical(parent, "anchor parent")?;
    Ok(parent.join(file_name))
}

fn required<'a>(flags: &'a BTreeMap<String, String>, flag: &str) -> Result<&'a str, ExitError> {
    flags
        .get(flag)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn required_path(flags: &BTreeMap<String, String>, flag: &str) -> Result<PathBuf, ExitError> {
    required(flags, flag).map(PathBuf::from)
}

fn canonical(path: impl AsRef<Path>, label: &str) -> Result<PathBuf, ExitError> {
    std::fs::canonicalize(path.as_ref()).map_err(|error| {
        ExitError::env_config(format!(
            "cannot resolve {label} {}: {error}",
            path.as_ref().display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn parser_accepts_lifecycle_and_rejects_authority_drift() {
        let (_, provision) = parse(&args(&[
            "provision",
            "--root",
            ".",
            "--anchor",
            "anchor.json",
            "--deployment-id",
            "deployment.test",
        ]))
        .expect("provision args");
        assert_eq!(provision["--deployment-id"], "deployment.test");
        assert!(parse(&args(&[
            "verify",
            "--root",
            ".",
            "--anchor",
            "anchor.json",
            "--deployment-id",
            "forbidden",
        ]))
        .is_err());
        assert!(parse(&args(&["advance", "--unknown", "value"])).is_err());
    }
}
