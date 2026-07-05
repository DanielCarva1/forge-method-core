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
    Allowlist, AttestationPolicy, AttestationVerifier, ForgeMcpServer, McpServerConfig,
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
            &format!("unknown subcommand '{subcommand}'. Try: serve"),
            want_json,
        ),
        Err(McpArgsError::Serve { error, want_json }) => {
            emit_err(SERVE_COMMAND, &error.to_string(), want_json)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum McpArgs {
    Serve(ServeArgs),
    Help,
}

/// Top-level `forge-core mcp` parser errors. Hand-rolled per AGENTS.md.
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServeArgs {
    allowlist: Option<PathBuf>,
    root: Option<PathBuf>,
    allow_bootstrap_core: bool,
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
    let mut root = None;
    let mut allow_bootstrap_core = false;
    let want_json = json_output_unless_text_selected(args);

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--allowlist" => {
                i += 1;
                let p = args
                    .get(i)
                    .ok_or(ServeArgsError::MissingValue("--allowlist <yaml>"))?;
                allowlist = Some(PathBuf::from(p));
            }
            "--root" => {
                i += 1;
                let p = args
                    .get(i)
                    .ok_or(ServeArgsError::MissingValue("--root <path>"))?;
                root = Some(PathBuf::from(p));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
        root,
        allow_bootstrap_core,
        want_json,
    })
}

/// Failures parsing `mcp serve` arguments. Hand-rolled per AGENTS.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeArgsError {
    /// A flag that requires a value was given none.
    MissingValue(&'static str),
    /// An unrecognized flag (starts with `--`).
    UnknownFlag(String),
    /// An unexpected positional argument.
    UnexpectedPositional(String),
}

impl std::fmt::Display for ServeArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "{flag} requires a value"),
            Self::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            Self::UnexpectedPositional(arg) => {
                write!(f, "unexpected positional argument: {arg}")
            }
        }
    }
}

impl std::error::Error for ServeArgsError {}

fn run_serve(parsed: ServeArgs) -> Result<(), ExitError> {
    // Build the Allowlist: from --allowlist <yaml> if given, else default
    // read-only (the safe surface).
    let allowlist = match parsed.allowlist.as_ref() {
        Some(path) => {
            let yaml = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    return emit_err(
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
                return emit_err(
                    SERVE_COMMAND,
                    &format!("allowlist validation failed:\n{}", messages.join("\n")),
                    parsed.want_json,
                );
            }
            allowlist
        }
        None => Allowlist::default_read_only(),
    };

    let config = McpServerConfig {
        allowlist,
        attestation: AttestationVerifier::new(AttestationPolicy::Default),
        forge_core_binary: PathBuf::from("forge-core"),
        root: parsed.root,
        allow_bootstrap_core: parsed.allow_bootstrap_core,
    };
    let server = ForgeMcpServer::new(config);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    #[test]
    fn parse_mcp_args_routes_serve_to_typed_serve_args() {
        let parsed = parse_mcp_args(&args(&[
            "mcp",
            "serve",
            "--root",
            "/proj",
            "--allow-bootstrap-core",
            "--no-json",
        ]))
        .expect("parse mcp serve");

        let McpArgs::Serve(serve) = parsed else {
            panic!("expected serve args");
        };
        assert_eq!(
            serve.root.as_ref().map(|p| p.to_str().unwrap()),
            Some("/proj")
        );
        assert!(serve.allow_bootstrap_core);
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
                error: ServeArgsError::MissingValue("--allowlist <yaml>"),
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
        assert!(!parsed.allow_bootstrap_core);
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
    fn parse_serve_reads_root_and_bootstrap() {
        let args: Vec<String> = vec![
            "--root".into(),
            "/proj".into(),
            "--allow-bootstrap-core".into(),
        ];
        let parsed = parse_serve_args(&args).unwrap();
        assert_eq!(
            parsed.root.as_ref().map(|p| p.to_str().unwrap()),
            Some("/proj")
        );
        assert!(parsed.allow_bootstrap_core);
    }

    #[test]
    fn parse_serve_rejects_allowlist_without_value() {
        let args: Vec<String> = vec!["--allowlist".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn parse_serve_rejects_unknown_flag() {
        let args: Vec<String> = vec!["--bogus".into()];
        let err = parse_serve_args(&args).unwrap_err();
        assert!(err.to_string().contains("unknown flag"));
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
