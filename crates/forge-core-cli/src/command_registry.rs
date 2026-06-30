//! Command registry — the single source of truth for the `forge-core` CLI
//! dispatch table and global `--help` usage text.
//!
//! Before F15.4, adding a new command meant touching four manual edit points:
//! `main.rs` (match arm + `use`), `lib.rs` (`pub mod`), `cli_util.rs`
//! (`usage()` concat line), and the new module file. This module collapses the
//! first three into one: a single entry in [`COMMANDS`].
//!
//! Adding a new command now requires exactly two edits:
//! 1. Create the command module (file + `pub mod` in `lib.rs`).
//! 2. Add one [`CommandSpec`] entry to [`COMMANDS`].
//!
//! The dispatch match in `main.rs` and the global `usage()` string are both
//! derived from [`COMMANDS`], so they stay in sync automatically.
//!
//! ## Design notes
//!
//! `CommandSpec` is a deep module by the deletion test: deleting it would
//! scatter the command table back across `main.rs`, `lib.rs`, and
//! `cli_util.rs`, with the usage strings drifting out of sync on the first
//! missed edit. Concentrating the table here gives locality (one place to add,
//! rename, or reorder commands) and leverage (the dispatch + usage generation
//! are free once the entry exists).
//!
//! Handlers are stored as `fn(&[String]) -> Result<(), ExitError>` pointers so
//! the registry is a plain `const` (no `inventory` crate, no proc macros, no
//! global ctors). The three `m1_cmd` variants (`preview`, `ready`, `explain`)
//! share one underlying handler distinguished by [`M1CommandKind`]; thin
//! wrapper functions adapt them to the uniform handler signature.

use crate::cli_error::ExitError;
use crate::m1_cmd::M1CommandKind;

/// One row in the CLI dispatch table.
///
/// `name` is the argv[1] token the user types. `usage_lines` are the
/// `usage:` lines printed by the global `forge-core --help` output; a command
/// with subcommands contributes multiple lines. `handler` is the dispatcher
/// invoked when `name` matches argv[1].
pub struct CommandSpec {
    /// The argv[1] token that selects this command (e.g. `"validate"`,
    /// `"host-adapter-verify-rekor-entry"`).
    pub name: &'static str,
    /// One or more `usage:` lines for the global `--help` text, without a
    /// trailing newline (the joiner adds `\n`).
    pub usage_lines: &'static [&'static str],
    /// The dispatcher invoked when `argv[1] == name`.
    pub handler: fn(&[String]) -> Result<(), ExitError>,
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
    CommandSpec {
        name: "guide",
        usage_lines: &["       forge-core guide [--root <path>] [--allow-bootstrap-core] [--json]"],
        handler: crate::guide::run_guide_command,
    },
    CommandSpec {
        name: "claim",
        usage_lines: &[
            "       forge-core claim acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--claims-dir <path>] [--no-sync] [--no-json]",
            "       forge-core claim heartbeat [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--no-sync] [--no-json]",
            "       forge-core claim release [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--no-sync] [--no-json]",
            "       forge-core claim handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--no-sync] [--no-json]",
            "       forge-core claim status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--no-json]",
            "       forge-core claim reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--loop] [--interval-ms <ms>] [--max-ticks <n>] [--no-sync] [--no-json]",
            "       forge-core claim check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--claims-dir <path>] [--no-json]",
        ],
        handler: crate::claim::run_claim_command,
    },
    CommandSpec {
        name: "autonomy",
        usage_lines: &["       forge-core autonomy <subcommand> [flags]   (route|policy|admit|decision) [--json]"],
        handler: crate::autonomy_cmd::run_autonomy_command,
    },
    CommandSpec {
        name: "contract",
        usage_lines: &["       forge-core contract <subcommand>   (catalog|snapshot|explain) [--json]"],
        handler: crate::contract_cmd::run_contract_command,
    },
    CommandSpec {
        name: "isolation",
        usage_lines: &["       forge-core isolation [--root <path>] [--allow-bootstrap-core] [--json]"],
        handler: crate::isolation::run_isolation_command,
    },
    CommandSpec {
        name: "coordination",
        usage_lines: &["       forge-core coordination [--root <path>] [--allow-bootstrap-core] [--json]"],
        handler: crate::coordination::run_coordination_command,
    },
    CommandSpec {
        name: "project",
        usage_lines: &[
            "       forge-core project init [--root <path>] [--project-id <id>] [--sidecar-root <path>] [--state-root <path>] [--json|--no-json]",
            "       forge-core project resolve [--root <path>] [--allow-bootstrap-core] [--json|--no-json]",
        ],
        handler: crate::project_cmd::run_project_command,
    },
    CommandSpec {
        name: "graph",
        usage_lines: &[
            "       forge-core graph validate --root <project> --graph <path> [--allow-bootstrap-core] [--json]",
            "       forge-core graph run --root <project> --graph <path> --dry-run [--agent <id>] [--claims-dir <path>] [--now-unix <epoch>] [--allow-bootstrap-core] [--json]",
        ],
        handler: crate::graph_cmd::run_graph_command,
    },
    CommandSpec {
        name: "eval",
        usage_lines: &["       forge-core eval compare [--root <project>] [--suite <path>] --baseline <single-agent|graph|mas|manual> --candidate <single-agent|graph|mas|manual> [--allow-bootstrap-core] [--json|--no-json]"],
        handler: crate::eval_cmd::run_eval_command,
    },
    CommandSpec {
        name: "telemetry",
        usage_lines: &["       forge-core telemetry export [--root <project>] [--contract <path>] [--output <path>] [--format jsonl|otel-json] [--trace-id <id>|--run-id <id>|--latest-run] [--allow-bootstrap-core] [--json|--no-json]"],
        handler: crate::telemetry_cmd::run_telemetry_command,
    },
    CommandSpec {
        name: "preview",
        usage_lines: &["       forge-core preview [--root <path>] --operation <path> [--allow-bootstrap-core] [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json]"],
        handler: run_preview,
    },
    CommandSpec {
        name: "ready",
        usage_lines: &["       forge-core ready [--root <path>] --operation <path> [--allow-bootstrap-core] [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json]"],
        handler: run_ready,
    },
    CommandSpec {
        name: "explain",
        usage_lines: &["       forge-core explain [--root <path>] (--last-run | --run-id <id>) [--allow-bootstrap-core] [--json]"],
        handler: run_explain,
    },
    CommandSpec {
        name: "risk-audit",
        usage_lines: &["       forge-core risk-audit [--root <path>] --rules <path> [--json]"],
        handler: crate::risk_audit_cmd::run_risk_audit_command,
    },
    CommandSpec {
        name: "validate",
        usage_lines: &["       forge-core validate [--root <path>] [--json]"],
        handler: crate::validate::run_validate_command,
    },
    CommandSpec {
        name: "preflight",
        usage_lines: &["       forge-core preflight [--root <path>] [--allow-bootstrap-core] [--json|--no-json] [--gate <name>]... [--expected-anchor <count>]"],
        handler: crate::preflight_cmd::run_preflight_command,
    },
    CommandSpec {
        name: "execute-operation",
        usage_lines: &["       forge-core execute-operation --root <path> --operation <path> [--command <path>] [--effect <path>] [--payload <target_ref>=<path>] [--max-payload-bytes <bytes>] [--allow-payload-outside-root] [--allow-bootstrap-core] [--recorded-at <value>] [--tx-id-prefix <value>] [--no-sync] [--json]"],
        handler: crate::execute_operation::run_execute_operation_command,
    },
    CommandSpec {
        name: "rebuild-effect-index",
        usage_lines: &["       forge-core rebuild-effect-index [--root <path>] [--wal <path>] [--index <path>] [--lock <path>] [--allow-bootstrap-core] [--recorded-at <value>] [--no-sync] [--json]"],
        handler: crate::effect_index::run_rebuild_effect_index_command,
    },
    CommandSpec {
        name: "query-effect-index",
        usage_lines: &["       forge-core query-effect-index [--root <path>] [--index <path>] [--logical-ref <ref>] [--effect-id <id>] [--operation-id <id>] [--target-kind <kind>] [--consumer-use <discovery|diagnostics|handoff_context>] [--context] [--max-context-groups <n>] [--adapter-kind <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--adapter-trigger <evidence_discovery|diagnostics|handoff_preparation|manual_inspection>] [--latest] [--allow-bootstrap-core] [--json]"],
        handler: crate::effect_index::run_query_effect_index_command,
    },
    CommandSpec {
        name: "host-adapter-manifest",
        usage_lines: &["       forge-core host-adapter-manifest [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_manifest_command,
    },
    CommandSpec {
        name: "host-adapter-projection",
        usage_lines: &["       forge-core host-adapter-projection [--target <mcp_tools|borrowed_shell|app_ui>] [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_projection_command,
    },
    CommandSpec {
        name: "host-adapter-process-policy",
        usage_lines: &["       forge-core host-adapter-process-policy [--target <mcp_stdio|borrowed_shell|app_bridge>] [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_process_policy_command,
    },
    CommandSpec {
        name: "host-adapter-admit-invocation",
        usage_lines: &["       forge-core host-adapter-admit-invocation --command <name> [--target <mcp_stdio|borrowed_shell|app_bridge>] [--explicit] [--argv <arg>] [--cwd <path>] [--env-key <key>] [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_admit_invocation_command,
    },
    CommandSpec {
        name: "host-adapter-distribution-policy",
        usage_lines: &["       forge-core host-adapter-distribution-policy [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_distribution_policy_command,
    },
    CommandSpec {
        name: "host-adapter-admit-distribution",
        usage_lines: &["       forge-core host-adapter-admit-distribution --artifact <name> [--target <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--channel <stable|canary|dev>] [--sha256 <digest>] [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--explicit-canary-opt-in] [--json]"],
        handler: crate::host_adapter_policy_cmd::run_host_adapter_admit_distribution_command,
    },
    CommandSpec {
        name: "host-adapter-verify-artifact",
        usage_lines: &["       forge-core host-adapter-verify-artifact --artifact-path <path> --sha256 <digest> [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_artifact_command,
    },
    CommandSpec {
        name: "host-adapter-verify-provenance",
        usage_lines: &["       forge-core host-adapter-verify-provenance --artifact-path <path> --provenance-path <path> --signature-path <path> --public-key-path <path> --transparency-log-path <path> --sha256 <digest> --expected-builder-id <id> --expected-source-uri <uri> --expected-source-ref <ref> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_provenance_command,
    },
    CommandSpec {
        name: "host-adapter-verify-rekor-entry",
        usage_lines: &["       forge-core host-adapter-verify-rekor-entry --log-entry-path <path> --public-key-path <path> --expected-log-id <id> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_rekor_entry_command,
    },
    CommandSpec {
        name: "host-adapter-verify-sigstore-trust-policy",
        usage_lines: &["       forge-core host-adapter-verify-sigstore-trust-policy --policy-path <path> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_trust_policy_command,
    },
    CommandSpec {
        name: "host-adapter-verify-fulcio-certificate-identity",
        usage_lines: &["       forge-core host-adapter-verify-fulcio-certificate-identity --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --verification-time-unix <seconds> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_fulcio_certificate_identity_command,
    },
    CommandSpec {
        name: "host-adapter-verify-sigstore-bundle-subject",
        usage_lines: &["       forge-core host-adapter-verify-sigstore-bundle-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_bundle_subject_command,
    },
    CommandSpec {
        name: "host-adapter-verify-sigstore-dsse-in-toto-subject",
        usage_lines: &["       forge-core host-adapter-verify-sigstore-dsse-in-toto-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--expected-predicate-type <type>] [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_dsse_in_toto_subject_command,
    },
    CommandSpec {
        name: "host-adapter-verify-sigstore-timestamp-authority",
        usage_lines: &["       forge-core host-adapter-verify-sigstore-timestamp-authority --trust-policy-path <path> --certificate-path <path> [--rekor-log-entry-path <path>] [--rekor-public-key-path <path>] [--expected-rekor-log-id <id>] [--rfc3161-timestamp-token-path <path>] [--rfc3161-timestamped-signature-path <path>] [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_sigstore_timestamp_authority_command,
    },
    CommandSpec {
        name: "host-adapter-verify-certificate-transparency-sct",
        usage_lines: &["       forge-core host-adapter-verify-certificate-transparency-sct --trust-policy-path <path> --certificate-path <path> --sct-path <path> [--sct-path <path>] --verification-time-unix-ms <milliseconds> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_transparency_sct_command,
    },
    CommandSpec {
        name: "host-adapter-verify-certificate-revocation-policy",
        usage_lines: &["       forge-core host-adapter-verify-certificate-revocation-policy --trust-policy-path <path> --certificate-path <path> --trusted-signing-time-unix <seconds> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_revocation_policy_command,
    },
    CommandSpec {
        name: "host-adapter-verify-tuf-trusted-root-freshness",
        usage_lines: &["       forge-core host-adapter-verify-tuf-trusted-root-freshness --trust-policy-path <path> --root-metadata-path <path> [--timestamp-metadata-path <path>] [--snapshot-metadata-path <path>] [--targets-metadata-path <path>] --update-start-time-unix <seconds> [--min-root-version <n>] [--min-timestamp-version <n>] [--min-snapshot-version <n>] [--min-targets-version <n>] [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_tuf_trusted_root_freshness_command,
    },
    CommandSpec {
        name: "host-adapter-verify-certificate-crl-status",
        usage_lines: &["       forge-core host-adapter-verify-certificate-crl-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --crl-path <path> --verification-time-unix <seconds> [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_crl_status_command,
    },
    CommandSpec {
        name: "host-adapter-verify-certificate-ocsp-status",
        usage_lines: &["       forge-core host-adapter-verify-certificate-ocsp-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --ocsp-response-path <path> --verification-time-unix <seconds> [--expected-nonce-hex <hex>] [--json]"],
        handler: crate::host_adapter_verify_cmd::run_host_adapter_verify_certificate_ocsp_status_command,
    },
];

/// Looks up a command by its argv[1] token and invokes its handler.
///
/// Falls back to printing the global usage text for `--help` / `-h` and to
/// `ExitError::usage` for unknown commands. This replaces the 90-line match
/// that previously lived in `main.rs`.
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
        _ => Err(ExitError::usage(global_usage())),
    }
}

/// Builds the global `--help` / unknown-command usage text by joining every
/// [`CommandSpec::usage_lines`] entry, in registration order, separated by
/// newlines. The first line is the `usage: forge-core validate ...` header.
#[must_use]
pub fn global_usage() -> String {
    let mut out = String::with_capacity(8 * 1024);
    for spec in COMMANDS {
        for line in spec.usage_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    // Trim the trailing newline added by the last line; matches the legacy
    // `concat!` output which had no trailing newline.
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
}
