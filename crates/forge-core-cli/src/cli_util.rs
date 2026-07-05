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
use forge_core_command_surface::{
    CommandSpec, COMMAND_COST, COMMAND_EVAL, COMMAND_EVAL_DEFAULT_SUITE, COMMAND_GRAPH,
    COMMAND_TELEMETRY, COMMAND_TELEMETRY_DEFAULT_CONTRACT_PATH,
    COMMAND_TELEMETRY_DEFAULT_TRACE_SOURCE,
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
    let resolved =
        crate::project_cmd::resolve_project(root, allow_bootstrap_core).map_err(|error| {
            StatefulRootsError::ProjectResolve {
                source: error.to_string(),
            }
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

/// The global `forge-core --help` / unknown-command usage text.
///
/// Before F15.4 this was a hand-maintained `concat!(...)` string that had to
/// be edited in lock-step with the `dispatch()` match in `main.rs` and the
/// `pub mod` declarations in `lib.rs`. As of F15.4 it is derived from the
/// [`command_registry::COMMANDS`](crate::command_registry::COMMANDS) table,
/// so adding a command only requires one entry there.
///
/// Returns an owned `String` because the text is now built by joining the
/// per-command `usage_lines` at call time. The cost is negligible (this is
/// only reached on `--help` or an unknown-command error path) and every
/// caller passes it straight to [`ExitError::usage`], which takes
/// `impl Into<String>`.
#[must_use]
pub fn usage() -> String {
    crate::command_registry::global_usage()
}

#[must_use]
pub fn graph_usage() -> String {
    format_command_surface_usage("usage:", &COMMAND_GRAPH)
}

#[must_use]
pub fn eval_usage() -> String {
    let mut usage = format_command_surface_usage("usage:", &COMMAND_EVAL);
    usage.push('\n');
    usage.push_str("default suite: ");
    usage.push_str(COMMAND_EVAL_DEFAULT_SUITE);
    usage
}

#[must_use]
pub fn telemetry_usage() -> String {
    let mut usage = format_command_surface_usage("usage:", &COMMAND_TELEMETRY);
    usage.push('\n');
    usage.push_str("default contract: ");
    usage.push_str(COMMAND_TELEMETRY_DEFAULT_CONTRACT_PATH);
    usage.push('\n');
    usage.push_str("default trace source: ");
    usage.push_str(COMMAND_TELEMETRY_DEFAULT_TRACE_SOURCE);
    usage
}

#[must_use]
pub fn cost_usage() -> String {
    format_command_surface_usage("usage:", &COMMAND_COST)
}

fn format_command_surface_usage(header: &str, command: &CommandSpec) -> String {
    let mut usage = String::from(header);
    for line in command.usage_lines {
        usage.push('\n');
        usage.push_str("  ");
        usage.push_str(line.trim_start());
    }
    usage
}

#[must_use]
pub fn resolve_now_unix(flag: Option<i64>) -> i64 {
    flag.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
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

/// Result variant of `next_arg`.
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

/// Result variant of `next_path`.
///
/// # Errors
///
/// Returns `ExitError::usage` when the underlying [`next_arg_or_err`] reports
/// that `index` is out of bounds for `args`.
pub fn next_path_or_err(args: &[String], index: usize) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_arg_or_err(args, index)?))
}

/// Result variant of `parse_payload_arg`.
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

/// Result variant of `parse_u64`.
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as a `u64`.
pub fn parse_u64_or_err(value: &str) -> Result<u64, ExitError> {
    value.parse::<u64>().map_err(|_| ExitError::usage(usage()))
}

/// Result variant of `parse_i64`.
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as an `i64`.
pub fn parse_i64_or_err(value: &str) -> Result<i64, ExitError> {
    value.parse::<i64>().map_err(|_| ExitError::usage(usage()))
}

/// Result variant of `parse_usize`.
///
/// # Errors
///
/// Returns `ExitError::usage` when `value` does not parse as a `usize`.
pub fn parse_usize_or_err(value: &str) -> Result<usize, ExitError> {
    value
        .parse::<usize>()
        .map_err(|_| ExitError::usage(usage()))
}

/// Result variant of `parse_target_kind`.
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

/// Result variant of `parse_runtime_kind`.
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

/// Result variant of `parse_metadata_consumer_use`.
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

/// Result variant of `parse_metadata_adapter_trigger`.
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

/// Result variant of `parse_host_adapter_projection_target`.
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

/// Result variant of `parse_host_adapter_process_target`.
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

/// Result variant of `parse_update_channel`.
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

/// Result variant of `require_value`.
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

/// Result variant of `parse_strict`.
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

/// Result variant of `resolve_stateful_roots_or_exit`.
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
    resolve_stateful_command_roots(root, allow_bootstrap_core)
        .map_err(|error| ExitError::failed(format!("{command} failed: {error}")))
}

/// Result variant of [`emit_envelope`].
///
/// Legacy emit path retained for the `claim` / `isolation` families. Its
/// text-mode contract DIFFERS from the canonical [`emit_envelope`]: it prints
/// nothing on success (silent) and uses the passed-in `family` label (not
/// `env.command`) for the failure line. New command families should call
/// [`emit_envelope`] (the `"command: ok"` text path) instead; do not adopt
/// this helper for new code.
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

/// The SINGLE emit path for every CLI command.
///
/// Prints the envelope to stdout (JSON mode) or a one-line human summary
/// (text mode), and returns `Ok(())` when the envelope's exit code is 0 (or
/// `Err(ExitError::with_code(code, ..))` when it is non-zero) so the binary
/// entrypoint can translate the latter into `process::exit(code)`.
///
/// Text-mode contract (must stay stable — operators and scripts parse it):
/// - success: `"{command}: ok"` to stdout
/// - failure: `"{command} failed: {message}"` to stderr
///
/// where `command` is the envelope's own `CliEnvelope::command` field, so a
/// command never has to restate its name at the call site.
///
/// # Do not define per-module `emit` twins
///
/// Before V1.D, seven command modules each carried a byte-identical private
/// `emit(env, want_json)` (~15 lines each), and two had drifted:
/// `contract_cmd` hardcoded its command name, and `autonomy_cmd` printed an
/// extra "lane" line. Both needs are now served by THIS function and
/// [`emit_envelope_with`] (the rare-case variant that takes an optional
/// text-mode success line). If you find yourself writing another `fn emit`,
/// call [`emit_envelope`] instead.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the envelope's non-zero exit code
/// (the human-readable diagnostic was already written to stderr/stdout).
///
/// # Panics
///
/// Panics in JSON mode if `env` cannot be serialized by `serde_json`. `T:
/// Serialize` is bound, so this is a programming error that never occurs on
/// well-formed envelope types. (V4.A will replace this `expect` with a typed
/// error; for now the behavior is preserved exactly from the deleted twins.)
pub fn emit_envelope<T: serde::Serialize>(
    env: forge_core_contracts::CliEnvelope<T>,
    want_json: bool,
) -> Result<(), ExitError> {
    emit_envelope_with(env, want_json, None)
}

/// The rare-case variant of [`emit_envelope`] for commands whose text-mode
/// success line carries domain meaning beyond a plain `"{command}: ok"`.
///
/// `text_success_line` overrides the stdout success line in text mode only
/// (JSON mode is unaffected — the full envelope is still printed). Pass
/// `None` to get the standard `"{command}: ok"` line (identical to
/// [`emit_envelope`]); pass `Some(line)` to render a richer summary such as
/// the autonomy router's `"lane: fast"`.
///
/// The failure path is identical to [`emit_envelope`] regardless of the
/// override, so a rejection still prints `"{command} failed: {message}"`.
///
/// # When to use this
///
/// Only when a command's text-mode success summary is genuinely a different
/// *kind* of output the operator reads directly (e.g. the selected autonomy
/// lane is the command's whole point). Do NOT reach for it to add cosmetic
/// decoration — prefer [`emit_envelope`] and let the JSON payload carry detail.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the envelope's non-zero exit code
/// (the human-readable diagnostic was already written to stderr/stdout).
///
/// # Panics
///
/// This function does NOT panic on serialization failure (V4.A). In JSON mode,
/// if `env` cannot be serialized by `serde_json`, an error is written to stderr
/// and an `ExitError::env_config` (exit code 5) is returned. In practice this
/// never fires — `T: Serialize` is bound, so well-formed envelope types always
/// serialize — but a panic is the wrong tool in a shared/stdout-critical path.
pub fn emit_envelope_with<T: serde::Serialize>(
    env: forge_core_contracts::CliEnvelope<T>,
    want_json: bool,
    text_success_line: Option<&str>,
) -> Result<(), ExitError> {
    let code = env.exit_code();
    if want_json {
        // Serialize before printing so a failure is a typed error, not a
        // panic. `T: Serialize` makes this effectively infallible, but the
        // shared stdout emit path must fail gracefully (stderr + env-config
        // exit code 5) rather than abort the process.
        let json = serde_json::to_string_pretty(&env).map_err(|e| {
            eprintln!("internal error: failed to serialize envelope: {e}");
            ExitError::env_config(format!("failed to serialize envelope: {e}"))
        })?;
        println!("{json}");
    } else {
        let command = env.command.0.as_str();
        if env.ok {
            match text_success_line {
                Some(line) => println!("{line}"),
                None => println!("{command}: ok"),
            }
        } else {
            eprintln!(
                "{command} failed: {}",
                env.error
                    .as_ref()
                    .map_or("unknown", |error| error.message.as_str())
            );
        }
    }
    if code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(code, String::new()))
    }
}

// ===========================================================================
// ArgvCursor — a small argv-walking type that collapses the per-command
// `while index < args.len() { match ... index += 1; }` boilerplate.
//
// Before F15.1 every command module (telemetry_cmd, eval_cmd, claim, ...)
// hand-wrote the same loop and its own `next_<cmd>_value_or_err` helper that
// differed only in the embedded command name. ArgvCursor is the deep module
// behind that seam: a four-method interface (peek_flag / expect_value /
// advance / exhausted) that owns bounds-checking, dash-rejection, and error
// formatting for every dispatcher in the crate.
//
// Adding a new command no longer requires writing a per-command value helper
// or an index-increment loop; the dispatcher becomes a flat `match` over
// `peek_flag()` that calls `expect_value(flag)` for value-bearing flags and
// `advance()` for boolean flags.
// ===========================================================================

/// Borrowed cursor over a flat `&[String]` argv slice, used by every CLI
/// dispatcher to walk flags without re-implementing bounds checks and
/// dash-rejection per command.
///
/// The cursor is created at the first flag position (typically `args[2]` for
/// `forge-core <command> <subcommand> [--flags...]`) and advances monotonically.
/// The `command` field is embedded in error messages so callers get
/// `"telemetry export: missing value for --root"` instead of a generic usage
/// dump, matching the pre-F15.1 error contract byte-for-byte.
pub struct ArgvCursor<'a> {
    args: &'a [String],
    index: usize,
    command: &'a str,
}

impl<'a> ArgvCursor<'a> {
    /// Creates a new cursor starting at `start`. The `command` string is used
    /// only for error context (e.g. `"telemetry export"`, `"eval compare"`).
    #[must_use]
    pub fn new(args: &'a [String], start: usize, command: &'a str) -> Self {
        Self {
            args,
            index: start,
            command,
        }
    }

    /// Returns the flag at the current position without consuming it, or
    /// `None` when the cursor is past the last argument.
    ///
    /// This is the loop condition for dispatcher `while let` loops: the
    /// dispatcher peeks, matches on the flag, and either calls
    /// [`expect_value`](Self::expect_value) (which advances past both flag and
    /// value) or [`advance`](Self::advance) (which advances past a boolean
    /// flag).
    #[must_use]
    pub fn peek_flag(&self) -> Option<&'a str> {
        self.args.get(self.index).map(String::as_str)
    }

    /// Consumes the current flag and returns the value that follows it.
    ///
    /// Used for value-bearing flags like `--root <path>`. The cursor advances
    /// past both the flag and its value, so the next [`peek_flag`](Self::peek_flag)
    /// call sees the following flag.
    ///
    /// `flag` is the flag name without the leading `--`; it is used only to
    /// build the error message and is NOT validated against the current
    /// position (the caller already matched on it in its `match` arm).
    ///
    /// # Errors
    ///
    /// Returns `ExitError::invalid_value` when no argument follows the flag
    /// (out of bounds) or when the following argument starts with `-` (looks
    /// like another flag rather than a value). The message embeds `command` and
    /// `flag` to match the historical per-command helper messages.
    pub fn expect_value(&mut self, flag: &str) -> Result<&'a str, ExitError> {
        let value_index = self.index + 1;
        let value = self.args.get(value_index).ok_or_else(|| {
            ExitError::invalid_value(format!("{}: missing value for --{flag}", self.command))
        })?;
        if value.starts_with('-') {
            return Err(ExitError::invalid_value(format!(
                "{}: missing value for --{flag}",
                self.command
            )));
        }
        self.index = value_index + 1;
        Ok(value.as_str())
    }

    /// Advances the cursor past the current flag without consuming a value.
    ///
    /// Used for boolean flags like `--json`, `--latest-run`,
    /// `--allow-bootstrap-core`. The dispatcher calls this after setting its
    /// boolean state.
    pub fn advance(&mut self) {
        self.index += 1;
    }

    /// Returns `true` when the cursor has consumed every argument.
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.index >= self.args.len()
    }
}

#[cfg(test)]
mod argv_cursor_tests {
    use super::*;

    fn args(slice: &[&str]) -> Vec<String> {
        slice.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn graph_usage_projects_command_surface_lines() {
        let usage = graph_usage();
        for line in COMMAND_GRAPH.usage_lines {
            let canonical = line.trim_start();
            assert!(
                usage.contains(canonical),
                "graph usage should include projected Command Surface line {canonical:?}: {usage}"
            );
        }
        assert!(
            usage.contains("[--json|--no-json]"),
            "graph usage should keep the shared JSON/text contract: {usage}"
        );
    }

    #[test]
    fn eval_usage_projects_command_surface_lines() {
        let usage = eval_usage();
        for line in COMMAND_EVAL.usage_lines {
            let canonical = line.trim_start();
            assert!(
                usage.contains(canonical),
                "eval usage should include projected Command Surface line {canonical:?}: {usage}"
            );
        }
        assert!(
            usage.contains(COMMAND_EVAL_DEFAULT_SUITE),
            "eval usage should keep the shared default suite path: {usage}"
        );
    }

    #[test]
    fn telemetry_usage_projects_command_surface_lines() {
        let usage = telemetry_usage();
        for line in COMMAND_TELEMETRY.usage_lines {
            let canonical = line.trim_start();
            assert!(
                usage.contains(canonical),
                "telemetry usage should include projected Command Surface line {canonical:?}: {usage}"
            );
        }
        assert!(
            usage.contains(COMMAND_TELEMETRY_DEFAULT_CONTRACT_PATH),
            "telemetry usage should keep the shared default contract path: {usage}"
        );
        assert!(
            usage.contains(COMMAND_TELEMETRY_DEFAULT_TRACE_SOURCE),
            "telemetry usage should keep the shared default trace source detail: {usage}"
        );
    }

    #[test]
    fn cost_usage_projects_command_surface_lines() {
        let usage = cost_usage();
        for line in COMMAND_COST.usage_lines {
            let canonical = line.trim_start();
            assert!(
                usage.contains(canonical),
                "cost usage should include projected Command Surface line {canonical:?}: {usage}"
            );
        }
        assert!(
            usage.contains("[--json|--no-json]"),
            "cost usage should keep the shared JSON/text contract: {usage}"
        );
    }

    #[test]
    fn peek_flag_returns_none_when_exhausted() {
        let args = args(&["telemetry", "export"]);
        let cursor = ArgvCursor::new(&args, 2, "telemetry export");
        assert!(cursor.peek_flag().is_none());
        assert!(cursor.exhausted());
    }

    #[test]
    fn peek_flag_returns_current_without_consuming() {
        let args = args(&["telemetry", "export", "--root", "."]);
        let cursor = ArgvCursor::new(&args, 2, "telemetry export");
        assert_eq!(cursor.peek_flag(), Some("--root"));
        // peek did not advance
        assert_eq!(cursor.peek_flag(), Some("--root"));
    }

    #[test]
    fn expect_value_advances_past_flag_and_value() {
        let args = args(&["telemetry", "export", "--root", ".", "--json"]);
        let mut cursor = ArgvCursor::new(&args, 2, "telemetry export");
        let value = cursor.expect_value("root").unwrap();
        assert_eq!(value, ".");
        // cursor is now at --json
        assert_eq!(cursor.peek_flag(), Some("--json"));
    }

    #[test]
    fn advance_skips_boolean_flag() {
        let args = args(&["telemetry", "export", "--json", "--root", "."]);
        let mut cursor = ArgvCursor::new(&args, 2, "telemetry export");
        cursor.advance();
        assert_eq!(cursor.peek_flag(), Some("--root"));
    }

    #[test]
    fn expect_value_errors_when_value_missing() {
        let args = args(&["telemetry", "export", "--root"]);
        let mut cursor = ArgvCursor::new(&args, 2, "telemetry export");
        let error = cursor.expect_value("root").unwrap_err();
        assert_eq!(
            error.message(),
            "telemetry export: missing value for --root"
        );
        assert_eq!(error.exit_code(), 3);
    }

    #[test]
    fn expect_value_errors_when_value_looks_like_flag() {
        let args = args(&["telemetry", "export", "--root", "--json"]);
        let mut cursor = ArgvCursor::new(&args, 2, "telemetry export");
        let error = cursor.expect_value("root").unwrap_err();
        assert_eq!(
            error.message(),
            "telemetry export: missing value for --root"
        );
    }

    #[test]
    fn full_telemetry_style_walk() {
        let args = args(&[
            "telemetry",
            "export",
            "--root",
            ".",
            "--format",
            "jsonl",
            "--json",
        ]);
        let mut cursor = ArgvCursor::new(&args, 2, "telemetry export");
        let mut root = String::new();
        let mut format = String::new();
        let mut json = false;
        while let Some(flag) = cursor.peek_flag() {
            match flag {
                "--root" => root = cursor.expect_value("root").unwrap().to_string(),
                "--format" => format = cursor.expect_value("format").unwrap().to_string(),
                "--json" => {
                    json = true;
                    cursor.advance();
                }
                _ => panic!("unexpected flag {flag}"),
            }
        }
        assert_eq!(root, ".");
        assert_eq!(format, "jsonl");
        assert!(json);
        assert!(cursor.exhausted());
    }
}
