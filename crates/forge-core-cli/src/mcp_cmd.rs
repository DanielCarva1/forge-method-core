//! `forge-core mcp` — CLI surface for the MCP adapter (F08.6, ADR-0006).
//!
//! One subcommand today:
//! - `serve` — run the stdio JSON-RPC MCP server over stdin/stdout, exposing
//!   the Allowlisted `forge-core` commands as MCP tools. Compatible with MCP
//!   clients like Claude Desktop.
//!
//! `serve` is a long-running process: it speaks JSON-RPC on stdout and emits
//! diagnostics to stderr. It does NOT emit a `CliEnvelope` on stdout (that
//! would corrupt the protocol stream); startup errors (bad allowlist,
//! missing binary) emit a `CliEnvelope` to stderr-shaped output before the
//! protocol loop begins, or to stdout if `--json` was requested for a
//! non-interactive validation run.
//!
//! The default Allowlist is read-only (the safe surface). Pass
//! `--allowlist <yaml>` to override (operator opt-in to mutate tools, which
//! remain gated by the `MutateGate` + Tool-Call Attestation at call time).

use std::path::PathBuf;

use forge_core_command_surface::{command_names, COMMAND_MCP};
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_protocol_mcp::{
    Allowlist, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry, ForgeMcpServer,
    McpServerConfig, DEFAULT_MAX_ATTESTATION_AGE_SECONDS, DEFAULT_MAX_FUTURE_SKEW_SECONDS,
};

use crate::cli_error::ExitError;

const MCP_COMMAND: &str = "mcp";
const SERVE_COMMAND: &str = "mcp serve";

/// Parse and run `forge-core mcp <subcommand>`.
///
/// # Errors
///
/// Returns `ExitError::usage` (via envelope emission) when the subcommand is
/// unknown or argument parsing fails. `serve` returns `ExitError` if the
/// MCP server loop fails to start.
pub fn run_mcp_command(args: &[String]) -> Result<(), ExitError> {
    match parse_mcp_args(args) {
        Ok(McpArgs::Serve(parsed)) => run_serve(parsed),
        Ok(McpArgs::Help) => {
            print_mcp_usage();
            Ok(())
        }
        Err(McpArgsError::UnknownSubcommand {
            subcommand,
            want_json,
        }) => emit_err(
            MCP_COMMAND,
            &mcp_message_with_usage(&format!("unknown subcommand '{subcommand}'. Try: serve")),
            want_json,
        ),
        Err(McpArgsError::Serve { error, want_json }) => emit_err(
            SERVE_COMMAND,
            &mcp_serve_parse_error_with_usage(&error),
            want_json,
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum McpArgs {
    Serve(ServeArgs),
    Help,
}

/// Top-level `forge-core mcp` parser errors. Hand-rolled (no anyhow/thiserror).
#[derive(Debug, Clone, PartialEq, Eq)]
enum McpArgsError {
    UnknownSubcommand {
        subcommand: String,
        want_json: bool,
    },
    Serve {
        error: ServeArgsError,
        want_json: bool,
    },
}

fn parse_mcp_args(args: &[String]) -> Result<McpArgs, McpArgsError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "serve" => parse_serve_args(&args[2..])
            .map(McpArgs::Serve)
            .map_err(|error| McpArgsError::Serve {
                error,
                want_json: json_output_unless_text_selected(&args[2..]),
            }),
        "--help" | "-h" | "help" => Ok(McpArgs::Help),
        other => Err(McpArgsError::UnknownSubcommand {
            subcommand: other.to_string(),
            want_json: json_output_unless_text_selected(&args[2..]),
        }),
    }
}

fn print_mcp_usage() {
    println!("{}", COMMAND_MCP.canonical_usage().trim_start());
    println!();
    println!("  serve runs the stdio JSON-RPC MCP server (ADR-0006). Default Allowlist");
    println!("  is read-only; --allowlist overrides with the named YAML file.");
    println!("  Any mutating allowlist also requires --principal-registry <yaml>.");
}

fn mcp_serve_usage_line() -> &'static str {
    COMMAND_MCP
        .usage_line_for_subcommand("serve")
        .unwrap_or("forge-core mcp serve [options]")
}

fn mcp_message_with_usage(message: &str) -> String {
    format!("{message}\n\nusage:\n  {}", mcp_serve_usage_line())
}

fn mcp_serve_parse_error_with_usage(error: &ServeArgsError) -> String {
    mcp_message_with_usage(&error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServeArgs {
    allowlist: Option<PathBuf>,
    principal_registry: Option<PathBuf>,
    root: Option<PathBuf>,
    want_json: bool,
}

/// Parse the `serve` subcommand args.
///
/// # Errors
///
/// Returns [`ServeArgsError`] when a flag is missing its value or an unknown
/// flag/positional is present.
fn parse_serve_args(args: &[String]) -> Result<ServeArgs, ServeArgsError> {
    let mut allowlist = None;
    let mut principal_registry = None;
    let mut root = None;
    let want_json = json_output_unless_text_selected(args);

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--allowlist" => {
                i += 1;
                let p = require_value(args, i, "--allowlist")?;
                allowlist = Some(PathBuf::from(p));
            }
            "--root" => {
                i += 1;
                let p = require_value(args, i, "--root")?;
                root = Some(PathBuf::from(p));
            }
            "--principal-registry" => {
                i += 1;
                let path = require_value(args, i, "--principal-registry")?;
                principal_registry = Some(PathBuf::from(path));
            }
            "--json" | "--no-json" | "--text" => { /* handled by want_json */ }
            other if other.starts_with("--") => {
                return Err(ServeArgsError::UnknownFlag(other.to_string()));
            }
            other => return Err(ServeArgsError::UnexpectedPositional(other.to_string())),
        }
        i += 1;
    }
    Ok(ServeArgs {
        allowlist,
        principal_registry,
        root,
        want_json,
    })
}

/// Failures parsing `mcp serve` arguments. Hand-rolled (no anyhow/thiserror).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeArgsError {
    /// A flag that requires a value was given none.
    MissingValue(&'static str),
    /// A flag that requires a value received another flag.
    FlagAsValue {
        /// The flag requiring a value.
        flag: &'static str,
        /// The flag-like token that was passed where a value was expected.
        value: String,
    },
    /// An unrecognized flag (starts with `--`).
    UnknownFlag(String),
    /// An unexpected positional argument.
    UnexpectedPositional(String),
}

impl std::fmt::Display for ServeArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "{flag} requires a value"),
            Self::FlagAsValue { flag, value } => {
                write!(f, "{flag} requires a value, got another flag '{value}'")
            }
            Self::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            Self::UnexpectedPositional(arg) => {
                write!(f, "unexpected positional argument: {arg}")
            }
        }
    }
}

impl std::error::Error for ServeArgsError {}

fn require_value(
    args: &[String],
    idx: usize,
    flag: &'static str,
) -> Result<String, ServeArgsError> {
    match args.get(idx) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(ServeArgsError::FlagAsValue {
                flag,
                value: value.clone(),
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(ServeArgsError::MissingValue(flag)),
    }
}

fn run_serve(parsed: ServeArgs) -> Result<(), ExitError> {
    // Build the Allowlist: from --allowlist <yaml> if given, else default
    // read-only (the safe surface).
    let allowlist = match parsed.allowlist.as_ref() {
        Some(path) => {
            let yaml = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    return emit_config_err(
                        SERVE_COMMAND,
                        &format!("failed to read allowlist {}: {e}", path.display()),
                        parsed.want_json,
                    );
                }
            };
            let known: Vec<&str> = command_names().collect();
            let (allowlist, report) = Allowlist::from_yaml_str(&yaml, &known);
            if report.has_errors() {
                // Surface validation diagnostics via the envelope so the
                // operator sees every problem before the server starts.
                let messages: Vec<String> = report
                    .diagnostics()
                    .iter()
                    .map(|d| format!("{}: {}", d.path, d.message))
                    .collect();
                return emit_config_err(
                    SERVE_COMMAND,
                    &format!("allowlist validation failed:\n{}", messages.join("\n")),
                    parsed.want_json,
                );
            }
            allowlist
        }
        None => Allowlist::default_read_only(),
    };

    let principal_registry = match parsed.principal_registry.as_ref() {
        Some(path) => {
            let yaml = match std::fs::read_to_string(path) {
                Ok(yaml) => yaml,
                Err(error) => {
                    return emit_config_err(
                        SERVE_COMMAND,
                        &format!(
                            "failed to read principal registry {}: {error}",
                            path.display()
                        ),
                        parsed.want_json,
                    );
                }
            };
            match AuthorizedPrincipalRegistry::from_yaml_str(&yaml) {
                Ok(registry) => Some(registry),
                Err(error) => {
                    return emit_config_err(
                        SERVE_COMMAND,
                        &format!("principal registry {} is invalid: {error}", path.display()),
                        parsed.want_json,
                    );
                }
            }
        }
        None => None,
    };
    let requested_root = parsed.root.unwrap_or_else(|| PathBuf::from("."));
    let root = match std::fs::canonicalize(&requested_root) {
        Ok(root) => root,
        Err(error) => {
            return emit_config_err(
                SERVE_COMMAND,
                &format!(
                    "failed to resolve MCP repo root {}: {error}",
                    requested_root.display()
                ),
                parsed.want_json,
            );
        }
    };
    let forge_core_binary = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(error) => {
            return emit_config_err(
                SERVE_COMMAND,
                &format!("failed to pin current forge-core executable: {error}"),
                parsed.want_json,
            );
        }
    };
    let config = McpServerConfig {
        allowlist,
        attestation: AttestationVerifier::new(AttestationPolicy::Default),
        principal_registry,
        mutation_executor: None,
        max_attestation_age_seconds: DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
        max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        forge_core_binary,
        root: Some(root),
    };
    let server = match ForgeMcpServer::try_new(config) {
        Ok(server) => server,
        Err(error) => {
            return emit_config_err(SERVE_COMMAND, &error.to_string(), parsed.want_json);
        }
    };

    // `serve` runs the JSON-RPC loop on stdout. Startup diagnostics go to
    // stderr so they do not corrupt the protocol stream. A failure to start
    // (e.g. transport error) surfaces as an ExitError.
    eprintln!("forge-core mcp: serving stdio JSON-RPC (press Ctrl-C to stop)");
    match server.run_stdio() {
        Ok(()) => Ok(()),
        Err(e) => Err(ExitError::env_config(format!("MCP server error: {e}"))),
    }
}

fn json_output_unless_text_selected(args: &[String]) -> bool {
    !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"))
}

fn emit_err(command: &str, message: &str, want_json: bool) -> Result<(), ExitError> {
    let env: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    crate::cli_util::emit_envelope(env, want_json)
}

fn emit_config_err(command: &str, message: &str, want_json: bool) -> Result<(), ExitError> {
    let envelope: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::EnvConfig, message);
    crate::cli_util::emit_envelope(envelope, want_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    fn assert_mcp_error_projects_serve_usage(message: &str, expected_diagnostic: &str) {
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        let projected = COMMAND_MCP
            .usage_line_for_subcommand("serve")
            .expect("mcp serve usage");
        assert!(
            message.contains(projected),
            "error should project mcp serve Command Surface usage {projected:?}: {message}"
        );
        assert!(
            !message.contains("forge-core execute-operation"),
            "mcp error should not leak unrelated command usage: {message}"
        );
    }

    #[test]
    fn parse_mcp_args_routes_serve_to_typed_serve_args() {
        let parsed = parse_mcp_args(&args(&["mcp", "serve", "--root", "/proj", "--no-json"]))
            .expect("parse mcp serve");

        let McpArgs::Serve(serve) = parsed else {
            panic!("expected serve args");
        };
        assert_eq!(
            serve.root.as_ref().map(|p| p.to_str().unwrap()),
            Some("/proj")
        );
        assert!(!serve.want_json);
    }

    #[test]
    fn parse_mcp_args_short_circuits_help() {
        let parsed = parse_mcp_args(&args(&["mcp", "--help"])).expect("parse help");
        assert_eq!(parsed, McpArgs::Help);
    }

    #[test]
    fn parse_mcp_args_preserves_json_preference_on_errors() {
        let serve_error =
            parse_mcp_args(&args(&["mcp", "serve", "--no-json", "--allowlist"])).unwrap_err();
        assert_eq!(
            serve_error,
            McpArgsError::Serve {
                error: ServeArgsError::MissingValue("--allowlist"),
                want_json: false,
            }
        );

        let unknown = parse_mcp_args(&args(&["mcp", "bogus", "--json"])).unwrap_err();
        assert_eq!(
            unknown,
            McpArgsError::UnknownSubcommand {
                subcommand: "bogus".to_string(),
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_serve_defaults_to_no_allowlist() {
        let args: Vec<String> = vec![];
        let parsed = parse_serve_args(&args).unwrap();
        assert!(parsed.allowlist.is_none());
        assert!(parsed.principal_registry.is_none());
    }

    #[test]
    fn parse_serve_reads_allowlist_flag() {
        let args: Vec<String> = vec!["--allowlist".into(), "/tmp/x.yaml".into()];
        let parsed = parse_serve_args(&args).unwrap();
        assert_eq!(
            parsed.allowlist.as_ref().map(|p| p.to_str().unwrap()),
            Some("/tmp/x.yaml")
        );
    }

    #[test]
    fn parse_serve_reads_principal_registry_flag() {
        let parsed = parse_serve_args(&args(&[
            "--principal-registry",
            "/operator/forge-principals.yaml",
        ]))
        .expect("parse principal registry");
        assert_eq!(
            parsed
                .principal_registry
                .as_ref()
                .map(|path| path.to_str().unwrap()),
            Some("/operator/forge-principals.yaml")
        );
    }

    #[test]
    fn parse_serve_reads_root() {
        let args: Vec<String> = vec!["--root".into(), "/proj".into()];
        let parsed = parse_serve_args(&args).unwrap();
        assert_eq!(
            parsed.root.as_ref().map(|p| p.to_str().unwrap()),
            Some("/proj")
        );
    }

    #[test]
    fn parse_serve_rejects_allowlist_without_value() {
        let args: Vec<String> = vec!["--allowlist".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn parse_serve_rejects_flag_as_allowlist_value() {
        let args: Vec<String> = vec!["--allowlist".into(), "--root".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert_eq!(
            err,
            ServeArgsError::FlagAsValue {
                flag: "--allowlist",
                value: "--root".to_string(),
            }
        );
    }

    #[test]
    fn parse_serve_rejects_principal_registry_without_value() {
        let error = parse_serve_args(&args(&["--principal-registry"]))
            .expect_err("principal registry path is required");
        assert_eq!(error, ServeArgsError::MissingValue("--principal-registry"));
    }

    #[test]
    fn parse_serve_rejects_flag_as_principal_registry_value() {
        let error = parse_serve_args(&args(&["--principal-registry", "--root"]))
            .expect_err("another flag cannot be a registry path");
        assert_eq!(
            error,
            ServeArgsError::FlagAsValue {
                flag: "--principal-registry",
                value: "--root".to_owned(),
            }
        );
    }

    #[test]
    fn parse_serve_rejects_flag_as_root_value() {
        let args: Vec<String> = vec!["--root".into(), "--json".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert_eq!(
            err,
            ServeArgsError::FlagAsValue {
                flag: "--root",
                value: "--json".to_string(),
            }
        );
    }

    #[test]
    fn parse_serve_rejects_unknown_flag() {
        let args: Vec<String> = vec!["--bogus".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert!(err.to_string().contains("unknown flag"));
    }

    #[test]
    fn mcp_serve_missing_value_reports_serve_usage() {
        let err = parse_serve_args(&args(&["--allowlist"])).unwrap_err();
        let message = mcp_serve_parse_error_with_usage(&err);

        assert_mcp_error_projects_serve_usage(&message, "--allowlist requires a value");
    }

    #[test]
    fn mcp_serve_flag_as_value_reports_serve_usage() {
        let err = parse_serve_args(&args(&["--allowlist", "--root"])).unwrap_err();
        let message = mcp_serve_parse_error_with_usage(&err);

        assert_mcp_error_projects_serve_usage(
            &message,
            "--allowlist requires a value, got another flag '--root'",
        );
    }

    #[test]
    fn mcp_serve_unknown_flag_reports_serve_usage() {
        let err = parse_serve_args(&args(&["--bogus"])).unwrap_err();
        let message = mcp_serve_parse_error_with_usage(&err);

        assert_mcp_error_projects_serve_usage(&message, "unknown flag: --bogus");
    }

    #[test]
    fn mcp_serve_unexpected_positional_reports_serve_usage() {
        let err = parse_serve_args(&args(&["extra"])).unwrap_err();
        let message = mcp_serve_parse_error_with_usage(&err);

        assert_mcp_error_projects_serve_usage(&message, "unexpected positional argument: extra");
    }

    #[test]
    fn mcp_unknown_subcommand_reports_serve_usage() {
        let message = mcp_message_with_usage("unknown subcommand 'bogus'. Try: serve");

        assert_mcp_error_projects_serve_usage(&message, "unknown subcommand 'bogus'");
    }

    #[test]
    fn json_flag_handling() {
        assert!(json_output_unless_text_selected(&[]));
        assert!(!json_output_unless_text_selected(&["--no-json".into()]));
        assert!(!json_output_unless_text_selected(&["--text".into()]));
        assert!(json_output_unless_text_selected(&["--json".into()]));
    }

    #[test]
    fn unknown_subcommand_emits_usage_envelope() {
        // run_mcp_command with an unknown sub returns Err (exit code 3).
        let args: Vec<String> = vec!["mcp".into(), "bogus".into()];
        let result = run_mcp_command(&args);
        assert!(result.is_err());
    }
}
