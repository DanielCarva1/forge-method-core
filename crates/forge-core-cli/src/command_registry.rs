//! Command registry — the CLI adapter over the shared `forge-core` Command
//! Surface.
//!
//! Before F15.4, adding a new command meant touching four manual edit points:
//! `main.rs` (match arm + `use`), `lib.rs` (`pub mod`), `cli_util.rs`
//! (`usage()` concat line), and the new module file. This module collapses the
//! first three into one: a single entry in [`COMMANDS`].
//!
//! Adding a new command now requires exactly two semantic edits:
//! 1. Create the command module (file + `pub mod` in `lib.rs`).
//! 2. Add shared metadata in `forge-core-command-surface` and a handler row in
//!    [`COMMANDS`].
//!
//! The dispatch match in `main.rs` is derived from [`COMMANDS`], while global
//! `usage()` strings are projected from the shared Command Surface metadata.
//! This keeps CLI help and MCP tool projection aligned without linking the MCP
//! adapter to CLI handlers.
//!
//! ## Design notes
//!
//! `forge-core-command-surface` is a deep module by the deletion test: deleting
//! it would scatter command names, usage strings, JSON mode, authority class,
//! and MCP visibility back across CLI and MCP. Concentrating those facts there
//! gives locality (one seam for shared command facts) and leverage (CLI help,
//! handler metadata, MCP defaults, and descriptor text are projections).
//!
//! Handlers are stored as `fn(&[String]) -> Result<(), ExitError>` pointers so
//! the registry is a plain `const` (no `inventory` crate, no proc macros, no
//! global ctors). The three `m1_cmd` variants (`preview`, `ready`, `explain`)
//! share one underlying handler distinguished by [`M1CommandKind`]; thin
//! wrapper functions adapt them to the uniform handler signature.

use forge_core_command_surface as surface;

use crate::cli_error::ExitError;
use crate::m1_cmd::M1CommandKind;

/// One row in the CLI dispatch table.
///
/// Reusable command metadata lives in `forge-core-command-surface`; this CLI
/// row adds only the handler pointer. Keeping those separate makes the
/// Command Surface module usable by MCP without creating a dependency cycle.
pub struct CommandSpec {
    /// The `argv[1]` token that selects this command (e.g. `"validate"`,
    /// `"host-adapter-verify-rekor-entry"`).
    pub name: &'static str,
    /// One or more `usage:` lines for the global `--help` text, without a
    /// trailing newline (the joiner adds `\n`).
    pub usage_lines: &'static [&'static str],
    /// Coarse authority class shared with adapters and generated docs.
    pub authority: surface::CommandAuthority,
    /// JSON/envelope output mode shared with adapters and generated docs.
    pub json_mode: surface::JsonMode,
    /// MCP visibility classification shared with adapters and generated docs.
    pub mcp_visibility: surface::McpVisibility,
    /// The dispatcher invoked when `argv[1] == name`.
    pub handler: fn(&[String]) -> Result<(), ExitError>,
}

impl CommandSpec {
    const fn from_surface(
        spec: &'static surface::CommandSpec,
        handler: fn(&[String]) -> Result<(), ExitError>,
    ) -> Self {
        Self {
            name: spec.name,
            usage_lines: spec.usage_lines,
            authority: spec.authority,
            json_mode: spec.json_mode,
            mcp_visibility: spec.mcp_visibility,
            handler,
        }
    }
}

fn run_preview(args: &[String]) -> Result<(), ExitError> {
    crate::m1_cmd::run_m1_command(args, M1CommandKind::Preview)
}

fn run_ready(args: &[String]) -> Result<(), ExitError> {
    crate::m1_cmd::run_m1_command(args, M1CommandKind::Ready)
}

fn run_explain(args: &[String]) -> Result<(), ExitError> {
    crate::m1_cmd::run_m1_command(args, M1CommandKind::Explain)
}

/// The complete, ordered dispatch table for `forge-core`.
///
/// Order matters only for the `--help` output; dispatch is by exact name
/// match, so reordering does not change behaviour.
#[rustfmt::skip]
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec::from_surface(&surface::COMMAND_GUIDE, crate::guide::run_guide_command),
    CommandSpec::from_surface(&surface::COMMAND_CLAIM, crate::claim::run_claim_command),
    CommandSpec::from_surface(&surface::COMMAND_AUTONOMY, crate::autonomy_cmd::run_autonomy_command),
    CommandSpec::from_surface(&surface::COMMAND_CONTRACT, crate::contract_cmd::run_contract_command),
    CommandSpec::from_surface(&surface::COMMAND_ISOLATION, crate::isolation::run_isolation_command),
    CommandSpec::from_surface(&surface::COMMAND_MEMORY, crate::memory_cmd::run_memory_command),
    CommandSpec::from_surface(&surface::COMMAND_GOVERNANCE, crate::governance_cmd::run_governance_command),
    CommandSpec::from_surface(&surface::COMMAND_COORDINATION, crate::coordination::run_coordination_command),
    CommandSpec::from_surface(&surface::COMMAND_PROJECT, crate::project_cmd::run_project_command),
    CommandSpec::from_surface(&surface::COMMAND_GRAPH, crate::graph_cmd::run_graph_command),
    CommandSpec::from_surface(&surface::COMMAND_EVAL, crate::eval_cmd::run_eval_command),
    CommandSpec::from_surface(&surface::COMMAND_EVAL_HARNESS, crate::eval_harness_cmd::run_eval_harness_command),
    CommandSpec::from_surface(&surface::COMMAND_TELEMETRY, crate::telemetry_cmd::run_telemetry_command),
    CommandSpec::from_surface(&surface::COMMAND_PREVIEW, run_preview),
    CommandSpec::from_surface(&surface::COMMAND_READY, run_ready),
    CommandSpec::from_surface(&surface::COMMAND_EXPLAIN, run_explain),
    CommandSpec::from_surface(&surface::COMMAND_COST, crate::cost_cmd::run_cost_command),
    CommandSpec::from_surface(&surface::COMMAND_RISK_AUDIT, crate::risk_audit_cmd::run_risk_audit_command),
    CommandSpec::from_surface(&surface::COMMAND_VALIDATE, crate::validate::run_validate_command),
    CommandSpec::from_surface(&surface::COMMAND_PREFLIGHT, crate::preflight_cmd::run_preflight_command),
    CommandSpec::from_surface(&surface::COMMAND_EXECUTE_OPERATION, crate::execute_operation::run_execute_operation_command),
    CommandSpec::from_surface(&surface::COMMAND_REBUILD_EFFECT_INDEX, crate::effect_index::run_rebuild_effect_index_command),
    CommandSpec::from_surface(&surface::COMMAND_QUERY_EFFECT_INDEX, crate::effect_index::run_query_effect_index_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_MANIFEST, crate::host_adapter_policy_cmd::run_host_adapter_manifest_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_PROJECTION, crate::host_adapter_policy_cmd::run_host_adapter_projection_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_PROCESS_POLICY, crate::host_adapter_policy_cmd::run_host_adapter_process_policy_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_ADMIT_INVOCATION, crate::host_adapter_policy_cmd::run_host_adapter_admit_invocation_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY, crate::host_adapter_policy_cmd::run_host_adapter_distribution_policy_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION, crate::host_adapter_policy_cmd::run_host_adapter_admit_distribution_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT, crate::host_adapter_verify_cmd::run_host_adapter_verify_artifact_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE, crate::host_adapter_verify_cmd::run_host_adapter_verify_provenance_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY, crate::host_adapter_verify_cmd::run_host_adapter_verify_rekor_entry_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY, crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_trust_policy_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY, crate::host_adapter_verify_cmd::run_host_adapter_verify_fulcio_certificate_identity_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT, crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_bundle_subject_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT, crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_dsse_in_toto_subject_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY, crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_timestamp_authority_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT, crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_transparency_sct_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY, crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_revocation_policy_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS, crate::host_adapter_verify_cmd::run_host_adapter_verify_tuf_trusted_root_freshness_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS, crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_crl_status_command),
    CommandSpec::from_surface(&surface::COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS, crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_ocsp_status_command),
    CommandSpec::from_surface(&surface::COMMAND_START, crate::start_cmd::run_start_command),
    CommandSpec::from_surface(&surface::COMMAND_MCP, crate::mcp_cmd::run_mcp_command),
    CommandSpec::from_surface(&surface::COMMAND_RESEARCH, crate::research_cmd::run_research_command),
];

/// Looks up a command by its `argv[1]` token and invokes its handler.
///
/// Falls back to printing the global usage text for `--help` / `-h` / no
/// args, the version string for `--version` / `-V`, and an actionable
/// unknown-command error otherwise.
///
/// # Errors
///
/// Returns `ExitError::usage` when `command` does not match any registered
/// [`CommandSpec::name`]. Propagates the handler's `ExitError` otherwise.
pub fn dispatch(command: &str, args: &[String]) -> Result<(), ExitError> {
    if let Some(spec) = COMMANDS.iter().find(|c| c.name == command) {
        return (spec.handler)(args);
    }
    match command {
        "--help" | "-h" => {
            println!("{}", global_usage());
            Ok(())
        }
        "--version" | "-V" => {
            println!("{}", version_string());
            Ok(())
        }
        _ => Err(ExitError::usage(format!(
            "forge-core: unknown command '{command}'.\n\n{global_usage_hint}",
            global_usage_hint = global_usage_hint()
        ))),
    }
}

/// The version line printed by `--version` / `-V`.
///
/// Sourced exclusively from the Cargo package version baked in at compile
/// time (`CARGO_PKG_VERSION`), which is the single source of truth for
/// releases. There is deliberately no runtime override: an earlier revision
/// read a `VERSION` file from the current working directory, which allowed a
/// stray file to spoof the reported version depending on where the binary
/// was invoked. That foot-gun was removed.
#[must_use]
pub fn version_string() -> String {
    format!("forge-core {}", env!("CARGO_PKG_VERSION"))
}

/// A short, framed hint shown above the full command list in `--help` and in
/// the unknown-command error. Points new users at `start` as the onboarding
/// entry point.
#[must_use]
pub fn global_usage_hint() -> String {
    // Reuse `version_string()` so the hint and `--version` can never disagree.
    // `version_string()` returns "forge-core <version>"; strip the prefix to
    // recover the bare version for the inline "Version <x>" sentence below.
    let version_line = version_string();
    let resolved_version = version_line
        .strip_prefix("forge-core ")
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let mut hint = String::with_capacity(512);
    hint.push_str("Forge Method Core — governance runtime for multi-agent builds.\n");
    hint.push_str("Version ");
    hint.push_str(resolved_version);
    hint.push_str(". Bring your own model; Forge coordinates many agents in one repo.\n\n");
    hint.push_str("First run? Start here:\n");
    hint.push_str("  forge-core start          diagnose a repo and get the next step\n");
    hint.push_str("  forge-core project init   create the Forge Project Link + sidecar\n");
    hint.push_str("  forge-core guide describe list every workflow in the catalog\n\n");
    hint.push_str("All commands accept --json for machine consumption.\n\n");
    hint.push_str("Commands:");
    hint
}

/// Builds the global `--help` / unknown-command usage text. A framed header
/// (what Forge is, the onboarding entry points) is prepended to the full
/// per-command usage line list, grouped so a new user can find `start`,
/// `project`, and `guide` before the long tail of host-adapter verifiers.
#[must_use]
pub fn global_usage() -> String {
    let mut out = String::with_capacity(12 * 1024);
    out.push_str(&global_usage_hint());
    out.push('\n');
    for spec in COMMANDS {
        for line in spec.usage_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    // Trim the trailing newline added by the last line; keeps the output
    // stable for snapshot-style assertions.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_name_is_unique() {
        let mut names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate command names in COMMANDS");
    }

    #[test]
    fn every_command_has_at_least_one_usage_line() {
        for spec in COMMANDS {
            assert!(
                !spec.usage_lines.is_empty(),
                "command {:?} has no usage_lines",
                spec.name
            );
            for line in spec.usage_lines {
                assert!(
                    line.starts_with("       forge-core "),
                    "usage line for {:?} does not start with '       forge-core ': {:?}",
                    spec.name,
                    line
                );
            }
        }
    }

    #[test]
    fn cli_registry_mirrors_shared_command_surface() {
        let cli_names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        let surface_names: Vec<&str> = surface::command_names().collect();
        assert_eq!(
            cli_names, surface_names,
            "CLI registry order must mirror command surface"
        );

        for (cli, shared) in COMMANDS.iter().zip(surface::COMMANDS.iter()) {
            assert_eq!(
                cli.usage_lines, shared.usage_lines,
                "usage drift for {}",
                cli.name
            );
            assert_eq!(
                cli.authority, shared.authority,
                "authority drift for {}",
                cli.name
            );
            assert_eq!(
                cli.json_mode, shared.json_mode,
                "json mode drift for {}",
                cli.name
            );
            assert_eq!(
                cli.mcp_visibility, shared.mcp_visibility,
                "MCP visibility drift for {}",
                cli.name
            );
        }
    }

    #[test]
    fn dispatch_finds_registered_command() {
        // validate is always registered and is a safe smoke target: we don't
        // invoke it, only confirm the lookup succeeds.
        assert!(COMMANDS.iter().any(|c| c.name == "validate"));
    }

    #[test]
    fn dispatch_returns_usage_for_unknown_command() {
        let result = dispatch("definitely-not-a-command", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn global_usage_mentions_every_command() {
        let usage = global_usage();
        for spec in COMMANDS {
            assert!(
                usage.contains(spec.name),
                "global_usage() missing command {:?}",
                spec.name
            );
        }
    }

    #[test]
    fn global_usage_has_no_trailing_newline() {
        let usage = global_usage();
        assert!(!usage.ends_with('\n'), "trailing newline in global_usage");
    }

    #[test]
    fn global_usage_is_nonempty() {
        assert!(!global_usage().is_empty());
    }

    #[test]
    fn global_usage_starts_with_framing_header() {
        // The header must introduce Forge and surface the onboarding entry
        // point (`start`) so a new user running --help knows where to begin.
        let usage = global_usage();
        assert!(
            usage.starts_with("Forge Method Core —"),
            "global_usage() must start with the framing header"
        );
        assert!(
            usage.contains("forge-core start"),
            "global_usage() must surface the `start` onboarding command"
        );
    }

    #[test]
    fn version_string_carries_binary_name_and_version() {
        let v = version_string();
        assert!(
            v.starts_with("forge-core "),
            "version_string must start with 'forge-core ': {v:?}"
        );
        // CARGO_PKG_VERSION is baked at compile time and is non-empty.
        assert!(
            v.len() > "forge-core ".len(),
            "version_string must carry a version after the name: {v:?}"
        );
    }

    #[test]
    fn dispatch_version_flag_returns_ok_with_version() {
        // --version and -V must not be treated as unknown commands (the old
        // bug: they fell through to ExitError::usage and exited 2).
        for flag in ["--version", "-V"] {
            let result = dispatch(flag, &[]);
            assert!(result.is_ok(), "dispatch({flag:?}) should succeed");
        }
    }

    #[test]
    fn start_usage_line_matches_start_command_constant() {
        let start = COMMANDS
            .iter()
            .find(|spec| spec.name == "start")
            .expect("start command registered");
        assert_eq!(start.usage_lines.len(), 1);
        assert_eq!(
            start.usage_lines[0].trim_start(),
            crate::start_cmd::START_USAGE_LINE
        );
    }
}
