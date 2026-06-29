//! Project sidecar resolution.
//!
//! The Forge core repo is allowed to keep a local `.forge-method` only as a
//! bootstrap exception. Consumer projects should carry a small
//! `.forge-method.yaml` pointer to a sibling Forge Runtime Sidecar.

use forge_core_contracts::{
    CliEnvelope, ExitReason, ProjectLinkDocument, PROJECT_LINK_FILE_NAME,
    PROJECT_LINK_SCHEMA_VERSION,
};
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectLayoutKind {
    Sidecar,
    BootstrapCoreLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectResolvePayload {
    pub project_id: String,
    pub project_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_path: Option<String>,
    pub sidecar_root: String,
    pub state_root: String,
    pub state_exists: bool,
    pub layout: ProjectLayoutKind,
    pub bootstrap_core_exception: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectResolveError {
    RootNotFound {
        root: String,
    },
    RootCanonicalize {
        root: String,
        source: String,
    },
    LinkRead {
        path: String,
        source: String,
    },
    LinkParse {
        path: String,
        source: String,
    },
    UnsupportedSchemaVersion {
        path: String,
        found: String,
    },
    EmptyField {
        path: String,
        field: &'static str,
    },
    StateRootOutsideSidecar {
        path: String,
        state_root: String,
        sidecar_root: String,
    },
    ConsumerLocalStateRoot {
        path: String,
        state_root: String,
        project_root: String,
    },
    MissingProjectLink {
        root: String,
    },
}

impl ProjectResolveError {
    #[must_use]
    pub const fn exit_reason(&self) -> ExitReason {
        match self {
            Self::LinkParse { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::EmptyField { .. }
            | Self::StateRootOutsideSidecar { .. }
            | Self::ConsumerLocalStateRoot { .. } => ExitReason::InvalidDecisionShape,
            Self::RootNotFound { .. }
            | Self::RootCanonicalize { .. }
            | Self::LinkRead { .. }
            | Self::MissingProjectLink { .. } => ExitReason::EnvConfig,
        }
    }
}

impl fmt::Display for ProjectResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotFound { root } => write!(f, "project root does not exist: {root}"),
            Self::RootCanonicalize { root, source } => {
                write!(f, "could not canonicalize project root '{root}': {source}")
            }
            Self::LinkRead { path, source } => {
                write!(f, "could not read Forge Project Link '{path}': {source}")
            }
            Self::LinkParse { path, source } => {
                write!(f, "could not parse Forge Project Link '{path}': {source}")
            }
            Self::UnsupportedSchemaVersion { path, found } => write!(
                f,
                "Forge Project Link '{path}' has unsupported schema_version '{found}', expected '{PROJECT_LINK_SCHEMA_VERSION}'"
            ),
            Self::EmptyField { path, field } => {
                write!(f, "Forge Project Link '{path}' has empty required field '{field}'")
            }
            Self::StateRootOutsideSidecar {
                path,
                state_root,
                sidecar_root,
            } => write!(
                f,
                "Forge Project Link '{path}' is invalid: state_root '{state_root}' must be inside sidecar_root '{sidecar_root}'"
            ),
            Self::ConsumerLocalStateRoot {
                path,
                state_root,
                project_root,
            } => write!(
                f,
                "Forge Project Link '{path}' is invalid: consumer project state_root '{state_root}' must not live inside project_root '{project_root}'; use a Forge Runtime Sidecar instead"
            ),
            Self::MissingProjectLink { root } => write!(
                f,
                "missing Forge Project Link at '{root}\\{PROJECT_LINK_FILE_NAME}'; consumer projects must point at a Forge Runtime Sidecar"
            ),
        }
    }
}

#[must_use]
pub fn run_resolve(root: &Path, allow_bootstrap_core: bool) -> CliEnvelope<ProjectResolvePayload> {
    match resolve_project(root, allow_bootstrap_core) {
        Ok(payload) => CliEnvelope::ok("project.resolve", payload),
        Err(err) => CliEnvelope::err("project.resolve", err.exit_reason(), err.to_string()),
    }
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}

pub fn resolve_project(
    root: &Path,
    allow_bootstrap_core: bool,
) -> Result<ProjectResolvePayload, ProjectResolveError> {
    if !root.exists() {
        return Err(ProjectResolveError::RootNotFound {
            root: display_path(&root),
        });
    }
    let root = root
        .canonicalize()
        .map_err(|source| ProjectResolveError::RootCanonicalize {
            root: display_path(&root),
            source: source.to_string(),
        })?;
    let link_path = root.join(PROJECT_LINK_FILE_NAME);
    if link_path.exists() {
        return resolve_from_link(&root, &link_path);
    }
    if allow_bootstrap_core && is_bootstrap_core_root(&root) {
        let state_root = normalize_path(root.join(".forge-method"));
        return Ok(ProjectResolvePayload {
            project_id: "forge-method-core".to_string(),
            project_root: display_path(&root),
            link_path: None,
            sidecar_root: display_path(&root),
            state_exists: state_root.exists(),
            state_root: display_path(&state_root),
            layout: ProjectLayoutKind::BootstrapCoreLocal,
            bootstrap_core_exception: true,
        });
    }
    Err(ProjectResolveError::MissingProjectLink {
        root: display_path(&root),
    })
}

fn resolve_from_link(
    project_root: &Path,
    link_path: &Path,
) -> Result<ProjectResolvePayload, ProjectResolveError> {
    let raw = fs::read_to_string(link_path).map_err(|source| ProjectResolveError::LinkRead {
        path: display_path(link_path),
        source: source.to_string(),
    })?;
    let link: ProjectLinkDocument =
        serde_yaml::from_str(strip_utf8_bom(&raw)).map_err(|source| {
            ProjectResolveError::LinkParse {
                path: display_path(link_path),
                source: source.to_string(),
            }
        })?;
    validate_link(&link, link_path)?;
    let sidecar_root = resolve_repo_path(project_root, &link.sidecar_root.0);
    let state_root = resolve_repo_path(project_root, &link.state_root.0);
    validate_resolved_sidecar_paths(project_root, link_path, &sidecar_root, &state_root)?;
    Ok(ProjectResolvePayload {
        project_id: link.project_id.0,
        project_root: display_path(project_root),
        link_path: Some(display_path(link_path)),
        sidecar_root: display_path(&sidecar_root),
        state_exists: state_root.exists(),
        state_root: display_path(&state_root),
        layout: ProjectLayoutKind::Sidecar,
        bootstrap_core_exception: false,
    })
}

fn validate_resolved_sidecar_paths(
    project_root: &Path,
    link_path: &Path,
    sidecar_root: &Path,
    state_root: &Path,
) -> Result<(), ProjectResolveError> {
    if !state_root.starts_with(sidecar_root) {
        return Err(ProjectResolveError::StateRootOutsideSidecar {
            path: display_path(link_path),
            state_root: display_path(state_root),
            sidecar_root: display_path(sidecar_root),
        });
    }
    if state_root.starts_with(project_root) {
        return Err(ProjectResolveError::ConsumerLocalStateRoot {
            path: display_path(link_path),
            state_root: display_path(state_root),
            project_root: display_path(project_root),
        });
    }
    Ok(())
}

fn validate_link(link: &ProjectLinkDocument, link_path: &Path) -> Result<(), ProjectResolveError> {
    let path = display_path(link_path);
    if link.schema_version != PROJECT_LINK_SCHEMA_VERSION {
        return Err(ProjectResolveError::UnsupportedSchemaVersion {
            path,
            found: link.schema_version.clone(),
        });
    }
    if link.project_id.0.trim().is_empty() {
        return Err(ProjectResolveError::EmptyField {
            path,
            field: "project_id",
        });
    }
    if link.sidecar_root.0.trim().is_empty() {
        return Err(ProjectResolveError::EmptyField {
            path,
            field: "sidecar_root",
        });
    }
    if link.state_root.0.trim().is_empty() {
        return Err(ProjectResolveError::EmptyField {
            path,
            field: "state_root",
        });
    }
    Ok(())
}

fn resolve_repo_path(project_root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        normalize_path(candidate)
    } else {
        normalize_path(project_root.join(candidate))
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn strip_utf8_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn is_bootstrap_core_root(root: &Path) -> bool {
    root.join("Cargo.toml").is_file()
        && root.join("crates").join("forge-core-cli").is_dir()
        && root.join(".forge-method").is_dir()
}

pub fn dispatch(args: &[String]) -> (String, i32) {
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");
    match sub {
        "resolve" => dispatch_resolve(&args[2..]),
        "--help" | "-h" | "help" => (project_usage().to_string(), 0),
        other => (
            format!("forge-core project: unknown subcommand '{other}'. Try: resolve"),
            2,
        ),
    }
}

fn dispatch_resolve(args: &[String]) -> (String, i32) {
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut want_json = true;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return ("project resolve: --root requires a value".to_string(), 3);
                };
                root = PathBuf::from(value);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--json" => want_json = true,
            "--no-json" => want_json = false,
            "--help" | "-h" => return (project_usage().to_string(), 0),
            other => {
                return (
                    format!("project resolve: unrecognized argument '{other}'"),
                    3,
                );
            }
        }
        index += 1;
    }

    let envelope = run_resolve(&root, allow_bootstrap_core);
    let exit_code = envelope.exit_code();
    if want_json {
        (
            serde_json::to_string_pretty(&envelope).expect("serialize project resolve envelope"),
            exit_code,
        )
    } else if let Some(data) = envelope.data.as_ref() {
        (
            format!(
                "project={} layout={:?} state_root={} state_exists={}",
                data.project_id, data.layout, data.state_root, data.state_exists
            ),
            exit_code,
        )
    } else {
        (
            envelope
                .error
                .as_ref()
                .map(|err| err.message.clone())
                .unwrap_or_else(|| "project resolve failed".to_string()),
            exit_code,
        )
    }
}

fn project_usage() -> &'static str {
    "forge-core project <subcommand> [options]\n  resolve [--root <path>] [--allow-bootstrap-core] [--json|--no-json]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "forge-project-resolve-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn resolves_sidecar_from_project_link() {
        let parent = temp_root("sidecar-parent");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::write(
            app.join(PROJECT_LINK_FILE_NAME),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
        )
        .unwrap();

        let payload = resolve_project(&app, false).unwrap();

        assert_eq!(payload.project_id, "app");
        assert_eq!(payload.layout, ProjectLayoutKind::Sidecar);
        assert!(payload.state_exists);
        assert!(PathBuf::from(payload.state_root)
            .ends_with(Path::new("forge-app").join(".forge-method")));
        assert!(state.exists());
    }

    #[test]
    fn missing_link_without_exception_fails_closed() {
        let root = temp_root("missing-link");
        let err = resolve_project(&root, false).unwrap_err();
        assert_eq!(err.exit_reason(), ExitReason::EnvConfig);
        assert!(err.to_string().contains(PROJECT_LINK_FILE_NAME));
    }

    #[test]
    fn rejects_state_root_outside_sidecar_root() {
        let parent = temp_root("outside-sidecar");
        let app = parent.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join(PROJECT_LINK_FILE_NAME),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-other/.forge-method\n",
        )
        .unwrap();

        let err = resolve_project(&app, false).unwrap_err();

        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
        assert!(err.to_string().contains("state_root"));
        assert!(err.to_string().contains("sidecar_root"));
    }

    #[test]
    fn rejects_consumer_local_state_root() {
        let parent = temp_root("consumer-local");
        let app = parent.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join(PROJECT_LINK_FILE_NAME),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: .\nstate_root: ./.forge-method\n",
        )
        .unwrap();

        let err = resolve_project(&app, false).unwrap_err();

        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
        assert!(err.to_string().contains("project_root"));
        assert!(err.to_string().contains("Sidecar"));
    }
}
