//! Generate the workspace-layout reference from Cargo metadata.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

const DEFAULT_OUTPUT: &str = "docs/generated/workspace-layout.md";
const GENERATE_COMMAND: &str =
    "cargo run --locked -p forge-core-validate --example generate_workspace_layout --";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    check: bool,
    output: PathBuf,
}

#[derive(Debug)]
enum GenerateError {
    MissingValue(&'static str),
    UnknownArgument(String),
    RepoRootUnavailable,
    CargoMetadata {
        code: i32,
        stderr: String,
    },
    InvalidMetadata(serde_json::Error),
    MissingWorkspaceMember(String),
    ManifestOutsideWorkspace {
        manifest: PathBuf,
        workspace_root: PathBuf,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    Stale(PathBuf),
}

impl GenerateError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::CargoMetadata { code, .. } => *code,
            _ => 1,
        }
    }
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(formatter, "{flag} requires a value"),
            Self::UnknownArgument(argument) => write!(formatter, "unknown argument: {argument}"),
            Self::RepoRootUnavailable => {
                formatter.write_str("could not resolve repository root from CARGO_MANIFEST_DIR")
            }
            Self::CargoMetadata { stderr, .. } => formatter.write_str(stderr.trim_end()),
            Self::InvalidMetadata(source) => write!(formatter, "invalid cargo metadata: {source}"),
            Self::MissingWorkspaceMember(id) => {
                write!(formatter, "cargo metadata omitted workspace member {id}")
            }
            Self::ManifestOutsideWorkspace {
                manifest,
                workspace_root,
            } => write!(
                formatter,
                "manifest {} is outside workspace {}",
                manifest.display(),
                workspace_root.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "failed to {action} {}: {source}", path.display()),
            Self::Stale(path) => write!(
                formatter,
                "{} is stale; run {GENERATE_COMMAND}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for GenerateError {}

#[derive(Debug, Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    workspace_members: Vec<String>,
    packages: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    #[serde(default)]
    targets: Vec<Target>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
struct Target {
    #[serde(default)]
    kind: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        println!("{GENERATE_COMMAND} [--check] [--output <path>]");
        return;
    }
    if let Err(error) = run(&arguments) {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}

fn run(arguments: &[String]) -> Result<(), GenerateError> {
    let options = parse_options(arguments)?;
    let root = repo_root()?;
    let output = if options.output.is_absolute() {
        options.output
    } else {
        root.join(options.output)
    };
    let raw = cargo_metadata(&root)?;
    let content = render_metadata(&raw)?;

    if options.check {
        let current = std::fs::read_to_string(&output).map_err(|source| GenerateError::Io {
            action: "read",
            path: output.clone(),
            source,
        })?;
        if current != content {
            return Err(GenerateError::Stale(repo_relative(&output, &root)));
        }
        return Ok(());
    }

    let parent = output.parent().ok_or(GenerateError::RepoRootUnavailable)?;
    std::fs::create_dir_all(parent).map_err(|source| GenerateError::Io {
        action: "create parent for",
        path: output.clone(),
        source,
    })?;
    std::fs::write(&output, content).map_err(|source| GenerateError::Io {
        action: "write",
        path: output.clone(),
        source,
    })?;
    println!("wrote {}", repo_relative(&output, &root).display());
    Ok(())
}

fn parse_options(arguments: &[String]) -> Result<Options, GenerateError> {
    let mut check = false;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--check" => check = true,
            "--output" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or(GenerateError::MissingValue("--output"))?;
                output = PathBuf::from(value);
            }
            output_value if output_value.starts_with("--output=") => {
                let value = output_value
                    .strip_prefix("--output=")
                    .filter(|value| !value.is_empty())
                    .ok_or(GenerateError::MissingValue("--output"))?;
                output = PathBuf::from(value);
            }
            unknown => return Err(GenerateError::UnknownArgument(unknown.to_owned())),
        }
        index += 1;
    }
    Ok(Options { check, output })
}

fn repo_root() -> Result<PathBuf, GenerateError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or(GenerateError::RepoRootUnavailable)
}

fn cargo_metadata(root: &Path) -> Result<String, GenerateError> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|source| GenerateError::Io {
            action: "execute cargo metadata for",
            path: root.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(GenerateError::CargoMetadata {
            code: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn render_metadata(raw: &str) -> Result<String, GenerateError> {
    let metadata: Metadata = serde_json::from_str(raw).map_err(GenerateError::InvalidMetadata)?;
    let packages = metadata
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect::<HashMap<_, _>>();
    let workspace_names = metadata
        .workspace_members
        .iter()
        .map(|id| {
            packages
                .get(id.as_str())
                .map(|package| package.name.as_str())
                .ok_or_else(|| GenerateError::MissingWorkspaceMember(id.clone()))
        })
        .collect::<Result<HashSet<_>, _>>()?;
    let mut members = metadata
        .workspace_members
        .iter()
        .map(|id| {
            packages
                .get(id.as_str())
                .copied()
                .ok_or_else(|| GenerateError::MissingWorkspaceMember(id.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    members.sort_by_key(|package| {
        repo_path(&package.manifest_path, &metadata.workspace_root)
            .unwrap_or_else(|_| package.manifest_path.clone())
    });

    let mut lines = vec![
        "# Workspace Layout".to_owned(),
        String::new(),
        "<!-- Generated by the Rust generate_workspace_layout example; do not edit by hand. -->"
            .to_owned(),
        String::new(),
        "Workspace root: repository root".to_owned(),
        format!("Workspace members: **{}**", members.len()),
        String::new(),
        "| Package | Path | Targets | Direct workspace dependencies |".to_owned(),
        "|---|---|---|---|".to_owned(),
    ];
    for package in members {
        let path = repo_path(&package.manifest_path, &metadata.workspace_root)?;
        let targets = unique_values(package.targets.iter().flat_map(|target| &target.kind));
        let dependencies = unique_values(package.dependencies.iter().filter_map(|dependency| {
            workspace_names
                .contains(dependency.name.as_str())
                .then_some(&dependency.name)
        }));
        lines.push(format!(
            "| `{}` | `{}` | {} | {} |",
            package.name,
            path.display().to_string().replace('\\', "/"),
            display_values(&targets),
            display_values(&dependencies)
        ));
    }
    lines.extend([
        String::new(),
        "## Regeneration".to_owned(),
        String::new(),
        "```bash".to_owned(),
        GENERATE_COMMAND.to_owned(),
        format!("{GENERATE_COMMAND} --check"),
        "```".to_owned(),
        String::new(),
        "The generator consumes `cargo metadata --format-version=1 --no-deps`.".to_owned(),
        String::new(),
    ]);
    Ok(lines.join("\n"))
}

fn repo_path(manifest: &Path, root: &Path) -> Result<PathBuf, GenerateError> {
    let package = manifest
        .parent()
        .ok_or_else(|| GenerateError::ManifestOutsideWorkspace {
            manifest: manifest.to_path_buf(),
            workspace_root: root.to_path_buf(),
        })?;
    package
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| GenerateError::ManifestOutsideWorkspace {
            manifest: manifest.to_path_buf(),
            workspace_root: root.to_path_buf(),
        })
}

fn unique_values<'a>(values: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut seen = HashSet::new();
    values.filter(|value| seen.insert(value.as_str())).collect()
}

fn display_values(values: &[&String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn repo_relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{parse_options, render_metadata, GenerateError, Options};
    use std::path::PathBuf;

    #[test]
    fn renders_sorted_workspace_members_and_normalizes_targets_and_dependencies() {
        let metadata = r#"{
          "workspace_root": "/repo",
          "workspace_members": ["package-b", "package-a"],
          "packages": [
            {
              "id": "package-b",
              "name": "crate-b",
              "manifest_path": "/repo/crates/b/Cargo.toml",
              "targets": [{"kind": ["test", "lib", "test"]}],
              "dependencies": [
                {"name": "crate-a", "rename": "renamed-a"},
                {"name": "external", "rename": null}
              ]
            },
            {
              "id": "package-a",
              "name": "crate-a",
              "manifest_path": "/repo/crates/a/Cargo.toml",
              "targets": [{"kind": ["lib"]}],
              "dependencies": []
            }
          ]
        }"#;

        let rendered = render_metadata(metadata).expect("render metadata");

        assert!(rendered.contains("Workspace members: **2**"));
        let a = rendered
            .find("| `crate-a` | `crates/a`")
            .expect("crate a row");
        let b = rendered
            .find("| `crate-b` | `crates/b`")
            .expect("crate b row");
        assert!(a < b, "members are sorted by repository path");
        assert!(rendered.contains("| `crate-b` | `crates/b` | test, lib | crate-a |"));
        assert!(rendered.contains(
            "cargo run --locked -p forge-core-validate --example generate_workspace_layout -- --check"
        ));
    }

    #[test]
    fn parses_check_and_output_without_accepting_unknown_flags() {
        let options = parse_options(&[
            "--check".to_owned(),
            "--output".to_owned(),
            "custom.md".to_owned(),
        ])
        .expect("parse options");
        assert_eq!(
            options,
            Options {
                check: true,
                output: PathBuf::from("custom.md")
            }
        );
        assert!(matches!(
            parse_options(&["--unknown".to_owned()]),
            Err(GenerateError::UnknownArgument(_))
        ));
        assert_eq!(
            parse_options(&["--output=inline.md".to_owned()])
                .expect("parse inline output")
                .output,
            PathBuf::from("inline.md")
        );
        assert!(matches!(
            parse_options(&["--output".to_owned(), "--check".to_owned()]),
            Err(GenerateError::MissingValue("--output"))
        ));
    }

    #[test]
    fn preserves_cargo_metadata_failure_code() {
        let error = GenerateError::CargoMetadata {
            code: 7,
            stderr: "metadata failed".to_owned(),
        };
        assert_eq!(error.exit_code(), 7);
    }
}
