//! Shared CLI parsing helpers and usage strings used by all command
//! dispatchers in the `forge-core-cli` crate.
//!
//! Extracted from the legacy god-file `main.rs` as part of the R11
//! main.rs decomposition (see
//! `docs/dev-docs/forge-method-core-dev-docs-v2/09_system_design_roadmap.md`).
//!
//! ## R8 (`process::exit` removal)
//!
//! Before R8 every helper here called `std::process::exit(N)` on a malformed
//! argv. As part of R8, the legacy helpers were replaced by their
//! `*_or_err` counterparts that return `Result<T, ExitError>`. The legacy
//! helpers were deleted once every dispatcher had migrated. The single
//! remaining `std::process::exit` lives at the top of `main.rs`.

use crate::cli_error::ExitError;
use crate::{
    HostAdapterProcessTarget, HostAdapterProjectionTarget, HostAdapterUpdateChannel,
    PayloadFileSpec,
};
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::EffectTargetKind;
use forge_core_store::{EffectMetadataAdapterTrigger, EffectMetadataConsumerUse};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct StatefulCommandRoots {
    pub project_root: PathBuf,
    pub effect_store_root: PathBuf,
}

/// Hand-rolled error enum for [`resolve_stateful_command_roots`]. Replaces the
/// legacy `Result<_, String>` signature so callers get typed variants instead
/// of opaque diagnostic strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatefulRootsError {
    /// `resolve_project` failed; carries its lossy display string.
    ProjectResolve { source: String },
    /// The resolved `state_root` does not exist on disk.
    StateRootMissing { state_root: String },
    /// The resolved `state_root` is not named `.forge-method`.
    StateRootNotSidecar { state_root: String },
    /// The resolved `state_root` has no parent directory.
    StateRootHasNoParent { state_root: String },
}

impl std::fmt::Display for StatefulRootsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectResolve { source } => {
                write!(formatter, "project resolve failed: {source}")
            }
            Self::StateRootMissing { state_root } => write!(
                formatter,
                "resolved Forge state_root does not exist for state-bearing command: {state_root}; create the sidecar .forge-method directory or fix .forge-method.yaml"
            ),
            Self::StateRootNotSidecar { state_root } => write!(
                formatter,
                "resolved Forge state_root must end with .forge-method for state-bearing operation/effect commands: {state_root}"
            ),
            Self::StateRootHasNoParent { state_root } => write!(
                formatter,
                "resolved Forge state_root has no parent sidecar root: {state_root}"
            ),
        }
    }
}

impl std::error::Error for StatefulRootsError {}

/// Resolves `project_root` and `effect_store_root` for any state-bearing
/// command (operation/effect) by reading the project's `.forge-method.yaml`.
///
/// # Errors
///
/// Returns [`StatefulRootsError`] when project resolution fails, when the
/// resolved `state_root` does not exist on disk, when it is not named
/// `.forge-method`, or when it has no parent sidecar root.
pub fn resolve_stateful_command_roots(
    root: &Path,
    allow_bootstrap_core: bool,
) -> Result<StatefulCommandRoots, StatefulRootsError> {
    let resolved = crate::project_cmd::resolve_project(root, allow_bootstrap_core)
        .map_err(|error| StatefulRootsError::ProjectResolve {
            source: error.to_string(),
        })?;
    let state_root = PathBuf::from(&resolved.state_root);
    if !state_root.exists() {
        return Err(StatefulRootsError::StateRootMissing {
            state_root: resolved.state_root.clone(),
        });
    }
    if state_root
        .file_name()
        .is_none_or(|name| name != std::ffi::OsStr::new(".forge-method"))
    {
        return Err(StatefulRootsError::StateRootNotSidecar {
            state_root: resolved.state_root.clone(),
        });
    }
    let Some(effect_store_root) = state_root.parent() else {
        return Err(StatefulRootsError::StateRootHasNoParent {
            state_root: resolved.state_root.clone(),
        });
    };
    Ok(StatefulCommandRoots {
        project_root: PathBuf::from(resolved.project_root),
        effect_store_root: effect_store_root.to_path_buf(),
    })
}

#[must_use]
pub fn usage() -> &'static str {
    concat!(
        "usage: forge-core validate [--root <path>] [--json]\n",
        "       forge-core project init [--root <path>] [--project-id <id>] [--sidecar-root <path>] [--state-root <path>] [--json|--no-json]\n",
        "       forge-core project resolve [--root <path>] [--allow-bootstrap-core] [--json|--no-json]\n",
        "       forge-core claim acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--claims-dir <path>] [--no-json]\n",
        "       forge-core claim heartbeat [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--no-json]\n",
        "       forge-core claim release [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--no-json]\n",
        "       forge-core claim handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--no-json]\n",
        "       forge-core claim status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--no-json]\n",
        "       forge-core claim reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--loop] [--interval-ms <ms>] [--max-ticks <n>] [--no-json]\n",
        "       forge-core claim check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--claims-dir <path>] [--no-json]\n",
        "       forge-core graph validate --root <project> --graph <path> [--allow-bootstrap-core] [--json]\n",
        "       forge-core graph run --root <project> --graph <path> --dry-run [--agent <id>] [--claims-dir <path>] [--now-unix <epoch>] [--allow-bootstrap-core] [--json]\n",
        "       forge-core eval compare [--root <project>] [--suite <path>] --baseline <single-agent|graph|mas|manual> --candidate <single-agent|graph|mas|manual> [--allow-bootstrap-core] [--json|--no-json]\n",
        "       forge-core telemetry export [--root <project>] [--contract <path>] [--output <path>] [--format jsonl|otel-json] [--trace-id <id>|--run-id <id>|--latest-run] [--allow-bootstrap-core] [--json|--no-json]\n",
        "       forge-core preview [--root <path>] --operation <path> [--allow-bootstrap-core] [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json]\n",
        "       forge-core ready [--root <path>] --operation <path> [--allow-bootstrap-core] [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json]\n",
        "       forge-core explain [--root <path>] (--last-run | --run-id <id>) [--allow-bootstrap-core] [--json]\n",
        "       forge-core execute-operation --root <path> --operation <path> [--command <path>] [--effect <path>] [--payload <target_ref>=<path>] [--max-payload-bytes <bytes>] [--allow-payload-outside-root] [--allow-bootstrap-core] [--recorded-at <value>] [--tx-id-prefix <value>] [--json]\n",
        "       forge-core rebuild-effect-index [--root <path>] [--wal <path>] [--index <path>] [--lock <path>] [--allow-bootstrap-core] [--recorded-at <value>] [--json]\n",
        "       forge-core query-effect-index [--root <path>] [--index <path>] [--logical-ref <ref>] [--effect-id <id>] [--operation-id <id>] [--target-kind <kind>] [--consumer-use <discovery|diagnostics|handoff_context>] [--context] [--max-context-groups <n>] [--adapter-kind <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--adapter-trigger <evidence_discovery|diagnostics|handoff_preparation|manual_inspection>] [--latest] [--allow-bootstrap-core] [--json]\n",
        "       forge-core host-adapter-manifest [--json]\n",
        "       forge-core host-adapter-projection [--target <mcp_tools|borrowed_shell|app_ui>] [--json]\n",
        "       forge-core host-adapter-process-policy [--target <mcp_stdio|borrowed_shell|app_bridge>] [--json]\n",
        "       forge-core host-adapter-admit-invocation --command <name> [--target <mcp_stdio|borrowed_shell|app_bridge>] [--explicit] [--argv <arg>] [--cwd <path>] [--env-key <key>] [--json]\n",
        "       forge-core host-adapter-distribution-policy [--json]\n",
        "       forge-core host-adapter-admit-distribution --artifact <name> [--target <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--channel <stable|canary|dev>] [--sha256 <digest>] [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--explicit-canary-opt-in] [--json]\n",
        "       forge-core host-adapter-verify-artifact --artifact-path <path> --sha256 <digest> [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--json]\n",
        "       forge-core host-adapter-verify-provenance --artifact-path <path> --provenance-path <path> --signature-path <path> --public-key-path <path> --transparency-log-path <path> --sha256 <digest> --expected-builder-id <id> --expected-source-uri <uri> --expected-source-ref <ref> [--json]\n",
        "       forge-core host-adapter-verify-rekor-entry --log-entry-path <path> --public-key-path <path> --expected-log-id <id> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-trust-policy --policy-path <path> [--json]\n",
        "       forge-core host-adapter-verify-fulcio-certificate-identity --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --verification-time-unix <seconds> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-bundle-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-dsse-in-toto-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--expected-predicate-type <type>] [--json]\n",
        "       forge-core host-adapter-verify-sigstore-timestamp-authority --trust-policy-path <path> --certificate-path <path> [--rekor-log-entry-path <path>] [--rekor-public-key-path <path>] [--expected-rekor-log-id <id>] [--rfc3161-timestamp-token-path <path>] [--rfc3161-timestamped-signature-path <path>] [--json]\n",
        "       forge-core host-adapter-verify-certificate-transparency-sct --trust-policy-path <path> --certificate-path <path> --sct-path <path> [--sct-path <path>] --verification-time-unix-ms <milliseconds> [--json]\n",
        "       forge-core host-adapter-verify-certificate-revocation-policy --trust-policy-path <path> --certificate-path <path> --trusted-signing-time-unix <seconds> [--json]\n",
        "       forge-core host-adapter-verify-tuf-trusted-root-freshness --trust-policy-path <path> --root-metadata-path <path> [--timestamp-metadata-path <path>] [--snapshot-metadata-path <path>] [--targets-metadata-path <path>] --update-start-time-unix <seconds> [--min-root-version <n>] [--min-timestamp-version <n>] [--min-snapshot-version <n>] [--min-targets-version <n>] [--json]",
        "\n       forge-core host-adapter-verify-certificate-crl-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --crl-path <path> --verification-time-unix <seconds> [--json]\n",
        "       forge-core host-adapter-verify-certificate-ocsp-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --ocsp-response-path <path> --verification-time-unix <seconds> [--expected-nonce-hex <hex>] [--json]",
    )
}

#[must_use]
pub fn graph_usage() -> &'static str {
    concat!(
        "usage: forge-core graph validate --root <project> --graph <path> [--allow-bootstrap-core] [--json]\n",
        "       forge-core graph run --root <project> --graph <path> --dry-run [--agent <id>] [--claims-dir <path>] [--now-unix <epoch>] [--allow-bootstrap-core] [--json]"
    )
}

#[must_use]
pub fn eval_usage() -> &'static str {
    concat!(
        "usage: forge-core eval compare [--root <project>] [--suite <path>] ",
        "--baseline <single-agent|graph|mas|manual> ",
        "--candidate <single-agent|graph|mas|manual> ",
        "[--allow-bootstrap-core] [--json|--no-json]\n",
        "default suite: ",
        "docs/fixtures/eval-run-v0/eval-compare-smoke-suite.yaml"
    )
}

#[must_use]
pub fn telemetry_usage() -> &'static str {
    concat!(
        "usage: forge-core telemetry export [--root <project>] ",
        "[--contract <path>] [--output <path>] [--format jsonl|otel-json] ",
        "[--trace-id <id>|--run-id <id>|--latest-run] ",
        "[--allow-bootstrap-core] [--json|--no-json]\n",
        "default contract: contracts/examples/telemetry.yaml\n",
        "default trace source: resolved <state_root>/traces/events.ndjson"
    )
}

#[must_use]
pub fn resolve_now_unix(flag: Option<i64>) -> i64 {
    flag.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
            .unwrap_or(0)
    })
}

// ===========================================================================
// R8 fallout: Result-returning variants of the legacy exit-on-error helpers.
//
// The legacy helpers above (`next_arg`, `parse_u64`, `require_value`, ...)
// call `std::process::exit` on a malformed argv. That makes the dispatchers
// impossible to unit-test as plain functions and forces business logic to
// live in the same layer as shell-exit policy.
//
// The `_or_err` variants below return `Result<T, ExitError>` instead. Each
// dispatcher migrates to its `_or_err` counterpart as part of R8; once every
// dispatcher is migrated, the legacy exit-on-error helpers will be deleted.
// ===========================================================================

/// Result variant of [`next_arg`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `index` is out of bounds for `args`,
/// surfacing the CLI usage string for the dispatcher.
pub fn next_arg_or_err(args: &[String], index: usize) -> Result<&str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(usage()))
}

/// Result variant of [`next_path`].
///
/// # Errors
///
/// Returns `ExitError::usage` when the underlying [`next_arg_or_err`] reports
/// that `index` is out of bounds for `args`.
pub fn next_path_or_err(args: &[String], index: usize) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_arg_or_err(args, index)?))
}

/// Result variant of [`parse_payload_arg`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not contain a `=` separating
/// the target reference from the payload path.
pub fn parse_payload_arg_or_err(value: &str) -> Result<PayloadFileSpec, ExitError> {
    let (target_ref, path) = value
        .split_once('=')
        .ok_or_else(|| ExitError::usage(usage()))?;
    Ok(PayloadFileSpec {
        target_ref: target_ref.to_string(),
        path: PathBuf::from(path),
    })
}

/// Result variant of [`parse_u64`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as a `u64`.
pub fn parse_u64_or_err(value: &str) -> Result<u64, ExitError> {
    value.parse::<u64>().map_err(|_| ExitError::usage(usage()))
}

/// Result variant of [`parse_i64`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as an `i64`.
pub fn parse_i64_or_err(value: &str) -> Result<i64, ExitError> {
    value.parse::<i64>().map_err(|_| ExitError::usage(usage()))
}

/// Result variant of [`parse_usize`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as a `usize`.
pub fn parse_usize_or_err(value: &str) -> Result<usize, ExitError> {
    value
        .parse::<usize>()
        .map_err(|_| ExitError::usage(usage()))
}

/// Result variant of [`parse_target_kind`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `EffectTargetKind` aliases.
pub fn parse_target_kind_or_err(value: &str) -> Result<EffectTargetKind, ExitError> {
    match value {
        "file_path" => Ok(EffectTargetKind::FilePath),
        "glob" => Ok(EffectTargetKind::Glob),
        "state_key" => Ok(EffectTargetKind::StateKey),
        "artifact_id" => Ok(EffectTargetKind::ArtifactId),
        "evidence_id" => Ok(EffectTargetKind::EvidenceId),
        "ledger_stream" => Ok(EffectTargetKind::LedgerStream),
        "request_stream" => Ok(EffectTargetKind::RequestStream),
        "completion_id" => Ok(EffectTargetKind::CompletionId),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_runtime_kind`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `RuntimeKind` aliases.
pub fn parse_runtime_kind_or_err(value: &str) -> Result<RuntimeKind, ExitError> {
    match value {
        "codex" => Ok(RuntimeKind::Codex),
        "cursor" => Ok(RuntimeKind::Cursor),
        "claude" => Ok(RuntimeKind::Claude),
        "opencode" => Ok(RuntimeKind::Opencode),
        "vscode" => Ok(RuntimeKind::Vscode),
        "pidev" => Ok(RuntimeKind::Pidev),
        "forge_standalone" => Ok(RuntimeKind::ForgeStandalone),
        "custom" => Ok(RuntimeKind::Custom),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_metadata_consumer_use`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `EffectMetadataConsumerUse` aliases.
pub fn parse_metadata_consumer_use_or_err(
    value: &str,
) -> Result<EffectMetadataConsumerUse, ExitError> {
    match value {
        "discovery" => Ok(EffectMetadataConsumerUse::Discovery),
        "diagnostics" => Ok(EffectMetadataConsumerUse::Diagnostics),
        "handoff_context" => Ok(EffectMetadataConsumerUse::HandoffContext),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_metadata_adapter_trigger`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `EffectMetadataAdapterTrigger` aliases.
pub fn parse_metadata_adapter_trigger_or_err(
    value: &str,
) -> Result<EffectMetadataAdapterTrigger, ExitError> {
    match value {
        "evidence_discovery" => Ok(EffectMetadataAdapterTrigger::EvidenceDiscovery),
        "diagnostics" => Ok(EffectMetadataAdapterTrigger::Diagnostics),
        "handoff_preparation" => Ok(EffectMetadataAdapterTrigger::HandoffPreparation),
        "manual_inspection" => Ok(EffectMetadataAdapterTrigger::ManualInspection),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_host_adapter_projection_target`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `HostAdapterProjectionTarget` aliases.
pub fn parse_host_adapter_projection_target_or_err(
    value: &str,
) -> Result<HostAdapterProjectionTarget, ExitError> {
    match value {
        "mcp_tools" => Ok(HostAdapterProjectionTarget::McpTools),
        "borrowed_shell" => Ok(HostAdapterProjectionTarget::BorrowedShell),
        "app_ui" => Ok(HostAdapterProjectionTarget::AppUi),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_host_adapter_process_target`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `HostAdapterProcessTarget` aliases.
pub fn parse_host_adapter_process_target_or_err(
    value: &str,
) -> Result<HostAdapterProcessTarget, ExitError> {
    match value {
        "mcp_stdio" => Ok(HostAdapterProcessTarget::McpStdio),
        "borrowed_shell" => Ok(HostAdapterProcessTarget::BorrowedShell),
        "app_bridge" => Ok(HostAdapterProcessTarget::AppBridge),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`parse_update_channel`].
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` is not one of the recognised
/// `HostAdapterUpdateChannel` aliases.
pub fn parse_update_channel_or_err(value: &str) -> Result<HostAdapterUpdateChannel, ExitError> {
    match value {
        "stable" => Ok(HostAdapterUpdateChannel::Stable),
        "canary" => Ok(HostAdapterUpdateChannel::Canary),
        "dev" => Ok(HostAdapterUpdateChannel::Dev),
        _ => Err(ExitError::usage(usage())),
    }
}

/// Result variant of [`require_value`].
///
/// Surfaces `ExitError::InvalidValue` (exit 3) to match the historical
/// strict-value rejection code used by governance commands.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when the slot at `idx` is missing,
/// empty, or starts with `--` (i.e. looks like the next flag rather than a
/// value for `--{flag}`).
pub fn require_value_or_err(args: &[String], idx: usize, flag: &str) -> Result<String, ExitError> {
    match args.get(idx) {
        Some(v) if !v.is_empty() && !v.starts_with("--") => Ok(v.clone()),
        _ => Err(ExitError::invalid_value(format!(
            "claim: --{flag} requires a value"
        ))),
    }
}

/// Result variant of [`parse_strict`].
///
/// Surfaces `ExitError::InvalidValue` (exit 3) on a malformed number, matching
/// the historical strict-parse rejection used by `claim` and `isolation`.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `s` does not parse as `T`.
pub fn parse_strict_or_err<T: std::str::FromStr>(s: &str, flag: &str) -> Result<T, ExitError> {
    s.parse::<T>()
        .map_err(|_| ExitError::invalid_value(format!("claim: invalid value for --{flag}: '{s}'")))
}

/// Result variant of [`resolve_stateful_roots_or_exit`].
///
/// The error variant is `ExitError::Failed` (exit 1) to match the historical
/// "command failed" code emitted by the legacy wrapper.
///
/// # Errors
///
/// Returns `ExitError::failed` when [`resolve_stateful_command_roots`] reports
/// that the project state cannot be resolved (missing `.forge-method`, wrong
/// directory name, missing parent sidecar, etc.).
pub fn resolve_stateful_roots_or_err(
    command: &str,
    root: &Path,
    allow_bootstrap_core: bool,
) -> Result<StatefulCommandRoots, ExitError> {
    resolve_stateful_command_roots(root, allow_bootstrap_core).map_err(|error| {
        ExitError::failed(format!("{command} failed: {error}"))
    })
}

/// Result variant of [`emit_envelope`].
///
/// Prints the envelope to stdout (JSON mode) or stderr (text-mode failure),
/// matching the legacy [`emit_envelope`] byte-for-byte. Returns `Ok(())` when
/// the envelope exit code is 0 so the caller can keep going (or simply return
/// `Ok(())` to terminate normally); returns `Err(ExitError::WithCode)` when
/// the envelope carries a non-zero code so the binary entrypoint can call
/// `process::exit(code)`.
///
/// Unlike [`emit_envelope`], this helper does NOT call `std::process::exit`;
/// the caller decides how to terminate. This makes it usable from library
/// code that needs to be unit-testable.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the envelope's non-zero exit code
/// so the entrypoint can translate it into `process::exit(code)`.
///
/// # Panics
///
/// Panics in JSON mode if `env` cannot be serialized by `serde_json`. `T:
/// Serialize` is bound on the function, so this is a programming error and
/// never occurs on well-formed envelope types.
pub fn emit_envelope_or_err<T: serde::Serialize>(
    family: &str,
    env: forge_core_contracts::CliEnvelope<T>,
    want_json: bool,
) -> Result<(), ExitError> {
    let code = env.exit_code();
    if want_json {
        println!("{}", serde_json::to_string_pretty(&env).unwrap());
    } else if !env.ok {
        eprintln!(
            "{} failed: {}",
            family,
            env.error.as_ref().map_or("unknown", |e| e.message.as_str())
        );
    }
    if code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(code, String::new()))
    }
}
