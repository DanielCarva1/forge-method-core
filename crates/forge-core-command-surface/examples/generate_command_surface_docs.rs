//! Generate the command-surface reference document from the Rust metadata.
//!
//! This is intentionally an example binary instead of a second parser over
//! source text. Cargo examples can use a library crate's public interface, so
//! this adapter renders docs from the same `COMMANDS` table consumed by CLI and
//! MCP projections.

use std::path::{Path, PathBuf};

use forge_core_command_surface::{
    mcp_default_mutate_tool_names, mcp_default_read_only_tool_names, COMMANDS,
};

const DEFAULT_OUTPUT: &str = "docs/generated/command-surface.md";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    check: bool,
    output: PathBuf,
}

/// Failures from the command-surface docs generator.
///
/// Hand-rolled per the repo convention: no `anyhow`, no `thiserror`, and no
/// `Result<_, String>`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GenerateDocsError {
    MissingValue {
        flag: &'static str,
    },
    UnknownArg {
        arg: String,
    },
    RepoRootUnavailable,
    Io {
        action: &'static str,
        path: String,
        source: String,
    },
    CheckFailed {
        path: String,
    },
}

impl std::fmt::Display for GenerateDocsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue { flag } => write!(f, "{flag} requires a value"),
            Self::UnknownArg { arg } => write!(f, "unknown argument: {arg}"),
            Self::RepoRootUnavailable => {
                f.write_str("could not resolve repository root from CARGO_MANIFEST_DIR")
            }
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} {path}: {source}"),
            Self::CheckFailed { path } => {
                write!(f, "{path} is stale; regenerate command-surface docs")
            }
        }
    }
}

impl std::error::Error for GenerateDocsError {}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return;
    }
    if let Err(error) = run(&args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn print_usage() {
    println!(
        "cargo run -p forge-core-command-surface --example generate_command_surface_docs -- [--check] [--output <path>]"
    );
}

fn run(args: &[String]) -> Result<(), GenerateDocsError> {
    let options = parse_args(args)?;
    let output = absolutize_output(&options.output)?;
    let content = render();

    if options.check {
        let current = match std::fs::read_to_string(&output) {
            Ok(value) => value,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(GenerateDocsError::CheckFailed {
                    path: repo_relative_display(&output),
                });
            }
            Err(source) => {
                return Err(GenerateDocsError::Io {
                    action: "read",
                    path: output.display().to_string(),
                    source: source.to_string(),
                });
            }
        };
        if current != content {
            return Err(GenerateDocsError::CheckFailed {
                path: repo_relative_display(&output),
            });
        }
        return Ok(());
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GenerateDocsError::Io {
            action: "create directory",
            path: parent.display().to_string(),
            source: source.to_string(),
        })?;
    }
    std::fs::write(&output, content).map_err(|source| GenerateDocsError::Io {
        action: "write",
        path: output.display().to_string(),
        source: source.to_string(),
    })?;
    println!("wrote {}", repo_relative_display(&output));
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Options, GenerateDocsError> {
    let mut check = false;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => check = true,
            "--output" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(GenerateDocsError::MissingValue { flag: "--output" });
                };
                output = PathBuf::from(value);
            }
            other => {
                return Err(GenerateDocsError::UnknownArg {
                    arg: other.to_string(),
                });
            }
        }
        index += 1;
    }
    Ok(Options { check, output })
}

fn render() -> String {
    let read_only_count = mcp_default_read_only_tool_names().count();
    let mutate_count = mcp_default_mutate_tool_names().count();
    let mut lines = vec![
        "# Command Surface",
        "",
        "<!-- Generated by cargo run -p forge-core-command-surface --example generate_command_surface_docs; do not edit by hand. -->",
        "",
        "This document is generated from `forge_core_command_surface::COMMANDS`.",
        "It is a projection of the same metadata used by CLI help and MCP tool descriptors.",
        "",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<String>>();

    lines.push(format!("Total commands: **{}**", COMMANDS.len()));
    lines.push(format!(
        "Default read-only MCP tools: **{read_only_count}**"
    ));
    lines.push(format!("Default mutate MCP tools: **{mutate_count}**"));
    lines.push(String::new());
    lines.push("| Command | Authority | JSON mode | MCP visibility | Usage |".to_string());
    lines.push("|---|---|---|---|---|".to_string());
    for command in COMMANDS {
        lines.push(format!(
            "| `{}` | `{}` | `{}` | `{}` | {} |",
            escape_markdown_cell(command.name),
            command.authority,
            command.json_mode,
            command.mcp_visibility,
            usage_cell(command.usage_lines)
        ));
    }
    lines.extend(
        [
            "",
            "## Regeneration",
            "",
            "```bash",
            "cargo run -p forge-core-command-surface --example generate_command_surface_docs",
            "cargo run -p forge-core-command-surface --example generate_command_surface_docs -- --check",
            "```",
            "",
        ]
        .into_iter()
        .map(str::to_string),
    );
    lines.join("\n")
}

fn usage_cell(usage_lines: &[&str]) -> String {
    usage_lines
        .iter()
        .map(|line| format!("<code>{}</code>", escape_markdown_cell(line.trim())))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn escape_markdown_cell(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('|', "\\|")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn repo_root() -> Result<PathBuf, GenerateDocsError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or(GenerateDocsError::RepoRootUnavailable)
}

fn absolutize_output(output: &Path) -> Result<PathBuf, GenerateDocsError> {
    if output.is_absolute() {
        return Ok(output.to_path_buf());
    }
    Ok(repo_root()?.join(output))
}

fn repo_relative_display(path: &Path) -> String {
    match repo_root() {
        Ok(root) => path
            .strip_prefix(&root)
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string()),
        Err(_) => path.display().to_string(),
    }
}
