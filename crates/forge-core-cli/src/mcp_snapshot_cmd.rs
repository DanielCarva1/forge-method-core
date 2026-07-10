//! Agent-facing generation of trusted MCP Admission snapshots.

use std::path::{Component, Path, PathBuf};

use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_protocol_mcp::{
    build_trusted_execution_snapshot, AuthorizedPrincipalRegistry, PrincipalCredentialStatus,
    PrincipalRegistryDocument, TrustedSnapshotBuildInput, TrustedSnapshotPrincipal,
    MCP_EXECUTE_OPERATION_TOOL,
};
use serde::Serialize;

use crate::cli_error::ExitError;
use crate::cli_util::{emit_envelope, resolve_now_unix};

const COMMAND: &str = "mcp snapshot";
const DEFAULT_OUTPUT: &str = "runtime/mcp-execution-snapshot.yaml";

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotArgs {
    root: PathBuf,
    operation: PathBuf,
    assurance: PathBuf,
    commands: Vec<PathBuf>,
    principal_registry: PathBuf,
    credential_id: String,
    nonce: String,
    output: PathBuf,
    now_unix: Option<i64>,
    want_json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotGenerated {
    snapshot_ref: String,
    execution_intent_digest: String,
    authority_snapshot_token: String,
    credential_id: String,
    audience: String,
    operation_id: String,
    claim_count: usize,
    gate_count: usize,
    next_action: String,
}

pub(crate) fn run_snapshot_command(args: &[String]) -> Result<(), ExitError> {
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            return emit_error(&format!("{message}\n\nusage:\n  {}", usage()), want_json);
        }
    };
    let project_root = std::fs::canonicalize(&parsed.root).map_err(|error| {
        ExitError::env_config(format!(
            "cannot resolve project root {}: {error}",
            parsed.root.display()
        ))
    })?;
    let resolved = crate::project_cmd::resolve_project(&project_root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    let state_root = PathBuf::from(resolved.state_root);
    let registry_text = std::fs::read_to_string(&parsed.principal_registry).map_err(|error| {
        ExitError::env_config(format!(
            "cannot read principal registry {}: {error}",
            parsed.principal_registry.display()
        ))
    })?;
    let registry_document: PrincipalRegistryDocument = yaml_serde::from_str(&registry_text)
        .map_err(|error| {
            ExitError::env_config(format!("invalid principal registry YAML: {error}"))
        })?;
    AuthorizedPrincipalRegistry::from_document(registry_document.clone())
        .map_err(|error| ExitError::env_config(format!("invalid principal registry: {error}")))?;
    let principal = select_principal(&registry_document, &parsed.credential_id)?;
    let now_unix = resolve_now_unix(parsed.now_unix);
    let output = build_trusted_execution_snapshot(
        &project_root,
        &state_root,
        &TrustedSnapshotBuildInput {
            operation_ref: parsed.operation,
            assurance_ref: parsed.assurance,
            command_refs: parsed.commands,
            principal,
            nonce: parsed.nonce,
            issued_at_unix: now_unix,
            now_unix,
        },
    )
    .map_err(|error| ExitError::env_config(format!("snapshot generation failed: {error}")))?;
    let snapshot_yaml = yaml_serde::to_string(&output.snapshot).map_err(|error| {
        ExitError::env_config(format!("snapshot serialization failed: {error}"))
    })?;
    write_state_relative(&state_root, &parsed.output, &snapshot_yaml)?;
    let payload = SnapshotGenerated {
        snapshot_ref: path_string(&parsed.output)?,
        execution_intent_digest: output.execution_intent_digest,
        authority_snapshot_token: output.authority_snapshot_token,
        credential_id: parsed.credential_id,
        audience: registry_document.principal_registry.audience,
        operation_id: output.operation_id,
        claim_count: output.claim_count,
        gate_count: output.gate_count,
        next_action: "sign this exact execution_intent_digest with the selected credential, then call execute-operation using the same nonce and snapshot".to_owned(),
    };
    emit_envelope(CliEnvelope::ok(COMMAND, payload), parsed.want_json)
}

fn select_principal(
    document: &PrincipalRegistryDocument,
    credential_id: &str,
) -> Result<TrustedSnapshotPrincipal, ExitError> {
    let entry = document
        .principal_registry
        .principals
        .iter()
        .find(|entry| entry.credential_id == credential_id)
        .ok_or_else(|| ExitError::env_config(format!("unknown credential_id '{credential_id}'")))?;
    if entry.status != PrincipalCredentialStatus::Active
        || !entry
            .allowed_tools
            .iter()
            .any(|tool| tool.0 == MCP_EXECUTE_OPERATION_TOOL)
        || !entry
            .authority_grants
            .iter()
            .any(|grant| grant.0 == "operation.execute")
    {
        return Err(ExitError::env_config(format!(
            "credential '{credential_id}' is not active and authorized for execute-operation"
        )));
    }
    Ok(TrustedSnapshotPrincipal {
        credential_id: entry.credential_id.clone(),
        principal_id: entry.principal_id.clone(),
        agent_id: entry.agent_id.clone(),
        role: entry.role,
    })
}

fn write_state_relative(state_root: &Path, output: &Path, yaml: &str) -> Result<(), ExitError> {
    if output.as_os_str().is_empty()
        || output.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(ExitError::env_config(
            "snapshot output must be a safe state-relative path".to_owned(),
        ));
    }
    let canonical_state = std::fs::canonicalize(state_root).map_err(|error| {
        ExitError::env_config(format!(
            "cannot resolve state root {}: {error}",
            state_root.display()
        ))
    })?;
    let target = canonical_state.join(output);
    let parent = target.parent().ok_or_else(|| {
        ExitError::env_config("snapshot output has no parent directory".to_owned())
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        ExitError::env_config(format!("cannot create snapshot directory: {error}"))
    })?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
        ExitError::env_config(format!("cannot resolve snapshot directory: {error}"))
    })?;
    if !canonical_parent.starts_with(&canonical_state) {
        return Err(ExitError::env_config(
            "snapshot output escapes the Project Link state root".to_owned(),
        ));
    }
    crate::io_util::atomic_write(&target, yaml)
        .map_err(|error| ExitError::env_config(format!("cannot write snapshot: {error}")))
}

fn parse(args: &[String]) -> Result<SnapshotArgs, String> {
    let mut root = None;
    let mut operation = None;
    let mut assurance = None;
    let mut commands = Vec::new();
    let mut principal_registry = None;
    let mut credential_id = None;
    let mut nonce = None;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut now_unix = None;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let mut value = || -> Result<String, String> {
            index += 1;
            args.get(index)
                .filter(|value| !value.starts_with("--"))
                .cloned()
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match flag {
            "--root" => root = Some(PathBuf::from(value()?)),
            "--operation" => operation = Some(PathBuf::from(value()?)),
            "--assurance" => assurance = Some(PathBuf::from(value()?)),
            "--command" => commands.push(PathBuf::from(value()?)),
            "--principal-registry" => principal_registry = Some(PathBuf::from(value()?)),
            "--credential-id" => credential_id = Some(value()?),
            "--nonce" => nonce = Some(value()?),
            "--output" => output = PathBuf::from(value()?),
            "--now-unix" => {
                now_unix = Some(
                    value()?
                        .parse::<i64>()
                        .map_err(|_| "--now-unix must be an integer".to_owned())?,
                );
            }
            "--json" | "--no-json" | "--text" => {}
            other => return Err(format!("unknown snapshot flag '{other}'")),
        }
        index += 1;
    }
    Ok(SnapshotArgs {
        root: root.ok_or("--root is required")?,
        operation: operation.ok_or("--operation is required")?,
        assurance: assurance.ok_or("--assurance is required")?,
        commands,
        principal_registry: principal_registry.ok_or("--principal-registry is required")?,
        credential_id: credential_id.ok_or("--credential-id is required")?,
        nonce: nonce.ok_or("--nonce is required")?,
        output,
        now_unix,
        want_json,
    })
}

fn usage() -> &'static str {
    "forge-core mcp snapshot --root <path> --operation <ref> --assurance <ref> [--command <ref>] --principal-registry <yaml> --credential-id <id> --nonce <value> [--output <state-relative-yaml>] [--now-unix <i64>] [--json|--no-json]"
}

fn path_string(path: &Path) -> Result<String, ExitError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ExitError::env_config("snapshot path is not valid UTF-8".to_owned()))
}

fn emit_error(message: &str, want_json: bool) -> Result<(), ExitError> {
    emit_envelope(
        CliEnvelope::<()>::err(COMMAND, ExitReason::InvalidDecisionShape, message),
        want_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn parser_requires_agent_operated_inputs_and_defaults_output() {
        let parsed = parse(&args(&[
            "--root",
            ".",
            "--operation",
            "op.yaml",
            "--assurance",
            "case.yaml",
            "--principal-registry",
            "registry.yaml",
            "--credential-id",
            "key.agent",
            "--nonce",
            "0123456789abcdef",
        ]))
        .expect("snapshot args");
        assert_eq!(parsed.output, PathBuf::from(DEFAULT_OUTPUT));
        assert!(parsed.want_json);
    }

    #[test]
    fn parser_rejects_missing_and_unknown_inputs() {
        assert!(parse(&args(&["--root", "."])).is_err());
        assert!(parse(&args(&["--bogus"])).is_err());
    }
}
