//! Conversationally projectable trusted MCP deployment readiness.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use forge_core_command_surface::command_names;
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_decisions::authority_snapshot_token;
use forge_core_protocol_mcp::{
    Allowlist, AuthorizedPrincipalRegistry, ExplicitTrustedSingleEffectOptIn,
    McpLocalExecutionSnapshotDocument, PrincipalCredentialStatus, PrincipalRegistryDocument,
    ReconciledTrustedMcpDeployment, ValidatedMcpDeploymentPolicy,
    DEFAULT_MAX_ATTESTATION_AGE_SECONDS, DEFAULT_MAX_FUTURE_SKEW_SECONDS,
    MCP_EXECUTE_OPERATION_TOOL, MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION,
};
use serde::Serialize;

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "mcp readiness";

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct ReadinessReport {
    verdict: &'static str,
    checks: Vec<&'static str>,
    project_root: String,
    state_root: String,
    snapshot_ref: String,
    replay_anchor_path: String,
    credential_id: String,
    client_config_path: Option<String>,
    next_actions: Vec<String>,
    replacement_agent_resume: String,
}

pub(crate) fn run_readiness_command(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    match evaluate(&flags) {
        Ok(report) => emit_envelope(CliEnvelope::ok(COMMAND, report), want_json),
        Err(error) => emit_envelope(
            CliEnvelope::<()>::err(COMMAND, ExitReason::EnvConfig, error.to_string()),
            want_json,
        ),
    }
}

fn evaluate(flags: &BTreeMap<String, String>) -> Result<ReadinessReport, ExitError> {
    let root = canonical(required_path(flags, "--root")?, "project root")?;
    let resolved = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("Project Link failed: {error}")))?;
    let state_root = canonical(PathBuf::from(resolved.state_root), "state root")?;
    let allowlist_path = canonical(required_path(flags, "--allowlist")?, "allowlist")?;
    let registry_path = canonical(
        crate::mcp_credential_cmd::absolute_path(required_path(flags, "--principal-registry")?)?,
        "principal registry",
    )?;
    let policy_path = canonical(
        required_path(flags, "--deployment-policy")?,
        "deployment policy",
    )?;
    let snapshot_ref = required_path(flags, "--snapshot")?;
    let replay_anchor_path = canonical(
        required_path(flags, "--replay-anchor")?,
        "external replay anchor",
    )?;
    let secret_dir = canonical(
        crate::mcp_credential_cmd::absolute_path(required_path(flags, "--secret-dir")?)?,
        "secret directory",
    )?;
    crate::mcp_credential_cmd::ensure_operator_owned_location(&registry_path, &root, &state_root)?;
    crate::mcp_credential_cmd::ensure_operator_owned_location(&secret_dir, &root, &state_root)?;
    crate::mcp_credential_cmd::ensure_operator_owned_location(
        &replay_anchor_path,
        &root,
        &state_root,
    )?;
    let credential_id = required(flags, "--credential-id")?.to_owned();

    let allowlist_text = read(&allowlist_path, "allowlist")?;
    let known = command_names().collect::<Vec<_>>();
    let (allowlist, report) = Allowlist::from_yaml_str(&allowlist_text, &known);
    if report.has_errors()
        || allowlist
            .iter()
            .filter(|tool| tool.policy.is_mutate())
            .count()
            != 1
        || allowlist.get(MCP_EXECUTE_OPERATION_TOOL).is_none()
    {
        return Err(ExitError::env_config(
            "allowlist must expose exactly the trusted execute-operation mutation".to_owned(),
        ));
    }
    let registry_document: PrincipalRegistryDocument =
        yaml_serde::from_str(&read(&registry_path, "principal registry")?)
            .map_err(|error| ExitError::env_config(format!("invalid registry YAML: {error}")))?;
    AuthorizedPrincipalRegistry::from_document(registry_document.clone())
        .map_err(|error| ExitError::env_config(format!("invalid registry: {error}")))?;
    let entry = registry_document
        .principal_registry
        .principals
        .iter()
        .find(|entry| entry.credential_id == credential_id)
        .filter(|entry| entry.status == PrincipalCredentialStatus::Active)
        .ok_or_else(|| {
            ExitError::env_config("selected credential is unknown or revoked".to_owned())
        })?;
    if entry.allowed_tools.len() != 1
        || entry.allowed_tools[0].0 != MCP_EXECUTE_OPERATION_TOOL
        || !entry
            .authority_grants
            .iter()
            .any(|grant| grant.0 == "operation.execute")
    {
        return Err(ExitError::env_config(
            "selected credential must authorize exactly execute-operation with operation.execute"
                .to_owned(),
        ));
    }
    let key = crate::mcp_credential_cmd::read_signing_key(
        &crate::mcp_credential_cmd::secret_path(&secret_dir, &credential_id),
    )?;
    if hex(key.verifying_key().as_bytes()) != entry.public_key_hex {
        return Err(ExitError::env_config(
            "operator secret does not match registry public key".to_owned(),
        ));
    }
    let policy = ValidatedMcpDeploymentPolicy::from_yaml(&read(&policy_path, "deployment policy")?)
        .map_err(|error| ExitError::env_config(format!("invalid deployment policy: {error}")))?;
    if policy
        .document()
        .mcp_deployment_policy
        .required_audience
        .as_deref()
        != Some(registry_document.principal_registry.audience.as_str())
    {
        return Err(ExitError::env_config(
            "registry and deployment-policy audiences differ".to_owned(),
        ));
    }
    let snapshot_path = confined_snapshot(&state_root, &snapshot_ref)?;
    let snapshot: McpLocalExecutionSnapshotDocument =
        yaml_serde::from_str(&read(&snapshot_path, "execution snapshot")?)
            .map_err(|error| ExitError::env_config(format!("invalid snapshot: {error}")))?;
    if snapshot.schema_version != MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION {
        return Err(ExitError::env_config(format!(
            "unsupported execution snapshot schema {}",
            snapshot.schema_version
        )));
    }
    let material = &snapshot.execution_snapshot;
    let now_unix = crate::cli_util::resolve_now_unix(None);
    validate_snapshot_freshness(material.admission_request.issued_at_unix, now_unix)?;
    let computed = authority_snapshot_token(
        &material.claim_snapshot,
        &material.gate_snapshot,
        material.current_state_version,
        material.now_unix,
    )
    .map_err(|error| ExitError::env_config(error.to_string()))?;
    if computed != material.admission_request.authority_snapshot_token
        || material.admission_request.principal_id != entry.principal_id
        || material.admission_request.agent_id != entry.agent_id
    {
        return Err(ExitError::env_config(
            "snapshot content binding or principal binding is stale".to_owned(),
        ));
    }
    ReconciledTrustedMcpDeployment::reconcile(
        policy,
        &root,
        &state_root,
        &replay_anchor_path,
        ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
    )
    .map_err(|error| ExitError::env_config(format!("startup reconciliation failed: {error}")))?;
    let client_config_path = flags
        .get("--client-config-output")
        .map(PathBuf::from)
        .map(|path| {
            write_client_config(
                &path,
                &root,
                &allowlist_path,
                &registry_path,
                &policy_path,
                &snapshot_ref,
                &replay_anchor_path,
            )
        })
        .transpose()?;
    Ok(ReadinessReport {
        verdict: "ready",
        checks: vec![
            "project_link_resolved",
            "exact_allowlist",
            "active_registry_credential",
            "private_public_key_match",
            "policy_audience_match",
            "snapshot_content_binding",
            "replay_clean_and_reconciled",
            "external_replay_anchor_current",
        ],
        project_root: path_string(&root)?,
        state_root: path_string(&state_root)?,
        snapshot_ref: path_string(&snapshot_ref)?,
        replay_anchor_path: path_string(&replay_anchor_path)?,
        credential_id,
        client_config_path,
        next_actions: vec![
            "sign the exact call with `forge-core mcp credential sign`".to_owned(),
            "start the generated MCP stdio client configuration".to_owned(),
        ],
        replacement_agent_resume: "rerun this command; all authority comes from durable project, sidecar, registry, secret, policy, and snapshot paths".to_owned(),
    })
}

fn write_client_config(
    path: &Path,
    root: &Path,
    allowlist: &Path,
    registry: &Path,
    policy: &Path,
    snapshot: &Path,
    replay_anchor: &Path,
) -> Result<String, ExitError> {
    let exe = std::env::current_exe()
        .map_err(|error| ExitError::env_config(format!("cannot resolve executable: {error}")))?;
    let value = serde_json::json!({
        "mcpServers": {"forge-method": {
            "command": path_string(&exe)?,
            "args": ["mcp", "serve", "--root", path_string(root)?, "--allowlist", path_string(allowlist)?, "--principal-registry", path_string(registry)?, "--deployment-policy", path_string(policy)?, "--snapshot", path_string(snapshot)?, "--replay-anchor", path_string(replay_anchor)?, "--enable-trusted-single-effect"]
        }}
    });
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| ExitError::env_config(error.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| ExitError::env_config(format!("cannot create config dir: {error}")))?;
    }
    crate::io_util::atomic_write(path, &text)
        .map_err(|error| ExitError::env_config(format!("cannot write client config: {error}")))?;
    path_string(path)
}

fn confined_snapshot(state_root: &Path, reference: &Path) -> Result<PathBuf, ExitError> {
    if reference.is_absolute()
        || reference
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ExitError::env_config(
            "snapshot must be state-relative".to_owned(),
        ));
    }
    let path = canonical(state_root.join(reference), "snapshot")?;
    if !path.starts_with(state_root) {
        return Err(ExitError::env_config(
            "snapshot escapes state root".to_owned(),
        ));
    }
    Ok(path)
}

fn parse_flags(args: &[String]) -> Result<BTreeMap<String, String>, ExitError> {
    const VALUE_FLAGS: &[&str] = &[
        "--root",
        "--allowlist",
        "--principal-registry",
        "--deployment-policy",
        "--snapshot",
        "--replay-anchor",
        "--secret-dir",
        "--credential-id",
        "--client-config-output",
    ];
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        if matches!(args[i].as_str(), "--json" | "--no-json" | "--text") {
            i += 1;
            continue;
        }
        let flag = args[i].clone();
        if !VALUE_FLAGS.contains(&flag.as_str()) {
            return Err(ExitError::usage(format!("unknown readiness flag '{flag}'")));
        }
        i += 1;
        let value = args
            .get(i)
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        if out.insert(flag.clone(), value.clone()).is_some() {
            return Err(ExitError::usage(format!(
                "duplicate readiness flag '{flag}'"
            )));
        }
        i += 1;
    }
    Ok(out)
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
fn read(path: &Path, label: &str) -> Result<String, ExitError> {
    std::fs::read_to_string(path)
        .map_err(|e| ExitError::env_config(format!("cannot read {label} {}: {e}", path.display())))
}
fn canonical(path: impl AsRef<Path>, label: &str) -> Result<PathBuf, ExitError> {
    std::fs::canonicalize(path.as_ref()).map_err(|e| {
        ExitError::env_config(format!(
            "cannot resolve {label} {}: {e}",
            path.as_ref().display()
        ))
    })
}
fn path_string(path: &Path) -> Result<String, ExitError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ExitError::env_config("path is not UTF-8".to_owned()))
}

fn validate_snapshot_freshness(issued_at_unix: i64, now_unix: i64) -> Result<(), ExitError> {
    let age = now_unix.saturating_sub(issued_at_unix);
    let future_skew = issued_at_unix.saturating_sub(now_unix);
    if age > i64::try_from(DEFAULT_MAX_ATTESTATION_AGE_SECONDS).unwrap_or(i64::MAX)
        || future_skew > i64::try_from(DEFAULT_MAX_FUTURE_SKEW_SECONDS).unwrap_or(i64::MAX)
    {
        return Err(ExitError::env_config(
            "execution snapshot is outside the attestation freshness window; regenerate and sign it"
                .to_owned(),
        ));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn readiness_parser_rejects_unknown_and_duplicate_flags() {
        assert!(parse_flags(&args(&["--unknown", "value"])).is_err());
        assert!(parse_flags(&args(&["--root", ".", "--root", "."])).is_err());
    }

    #[test]
    fn snapshot_freshness_accepts_boundaries_and_rejects_drift() {
        let now = 1_800_000_000;
        assert!(validate_snapshot_freshness(
            now - i64::try_from(DEFAULT_MAX_ATTESTATION_AGE_SECONDS).unwrap(),
            now
        )
        .is_ok());
        assert!(validate_snapshot_freshness(
            now + i64::try_from(DEFAULT_MAX_FUTURE_SKEW_SECONDS).unwrap(),
            now
        )
        .is_ok());
        assert!(validate_snapshot_freshness(now - 301, now).is_err());
        assert!(validate_snapshot_freshness(now + 31, now).is_err());
    }
}
