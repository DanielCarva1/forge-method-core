//! Project sidecar resolution.
//!
//! The Forge core repo is allowed to keep a local `.forge-method` only as a
//! bootstrap exception. Consumer projects should carry a small
//! `.forge-method.yaml` pointer to a sibling Forge Runtime Sidecar.

use forge_core_contracts::{
    CliEnvelope, ExitReason, ProjectLinkDocument, RepoPath, StableId, PROJECT_LINK_FILE_NAME,
    PROJECT_LINK_SCHEMA_VERSION,
};
use serde::Serialize;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use crate::cli_error::ExitError;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectInitStatus {
    Initialized,
    AlreadyInitialized,
}

impl ProjectInitStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::AlreadyInitialized => "already_initialized",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectInitPayload {
    pub status: ProjectInitStatus,
    pub project_id: String,
    pub project_root: String,
    pub link_path: String,
    pub sidecar_root: String,
    pub state_root: String,
    pub state_exists: bool,
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
    StateRootNotDotForgeMethod {
        path: String,
        state_root: String,
    },
    MissingProjectLink {
        root: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectInitError {
    RootNotFound {
        root: String,
    },
    RootNotDirectory {
        root: String,
    },
    RootCanonicalize {
        root: String,
        source: String,
    },
    MissingRootDirectoryName {
        root: String,
    },
    UnsafeProjectId {
        project_id: String,
        reason: &'static str,
    },
    EmptyPathField {
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
    ConsumerLocalStateExists {
        path: String,
    },
    StateRootNotDotForgeMethod {
        path: String,
        state_root: String,
    },
    ExistingProjectLinkInvalid {
        path: String,
        source: String,
        exit_reason: ExitReason,
    },
    ExistingProjectLinkMismatch {
        path: String,
        expected_project_id: String,
        found_project_id: String,
        expected_sidecar_root: String,
        found_sidecar_root: String,
        expected_state_root: String,
        found_state_root: String,
    },
    ProjectLinkSerialize {
        source: String,
    },
    CreateStateDir {
        path: String,
        source: String,
    },
    LedgerCreate {
        path: String,
        source: String,
    },
    LedgerSync {
        path: String,
        source: String,
    },
    LedgerNotFile {
        path: String,
    },
    LinkTempCreate {
        path: String,
        source: String,
    },
    LinkTempWrite {
        path: String,
        source: String,
    },
    LinkTempSync {
        path: String,
        source: String,
    },
    LinkRename {
        temp_path: String,
        link_path: String,
        source: String,
    },
    LinkExistsRace {
        path: String,
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
            | Self::ConsumerLocalStateRoot { .. }
            | Self::StateRootNotDotForgeMethod { .. } => ExitReason::InvalidDecisionShape,
            Self::RootNotFound { .. }
            | Self::RootCanonicalize { .. }
            | Self::LinkRead { .. }
            | Self::MissingProjectLink { .. } => ExitReason::EnvConfig,
        }
    }
}

impl ProjectInitError {
    #[must_use]
    pub const fn exit_reason(&self) -> ExitReason {
        match self {
            Self::UnsafeProjectId { .. }
            | Self::EmptyPathField { .. }
            | Self::StateRootOutsideSidecar { .. }
            | Self::ConsumerLocalStateRoot { .. }
            | Self::ConsumerLocalStateExists { .. }
            | Self::StateRootNotDotForgeMethod { .. }
            | Self::ExistingProjectLinkMismatch { .. }
            | Self::ProjectLinkSerialize { .. } => ExitReason::InvalidDecisionShape,
            Self::ExistingProjectLinkInvalid { exit_reason, .. } => *exit_reason,
            Self::LinkExistsRace { .. } => ExitReason::Conflict,
            Self::RootNotFound { .. }
            | Self::RootNotDirectory { .. }
            | Self::RootCanonicalize { .. }
            | Self::MissingRootDirectoryName { .. }
            | Self::CreateStateDir { .. }
            | Self::LedgerCreate { .. }
            | Self::LedgerSync { .. }
            | Self::LedgerNotFile { .. }
            | Self::LinkTempCreate { .. }
            | Self::LinkTempWrite { .. }
            | Self::LinkTempSync { .. }
            | Self::LinkRename { .. } => ExitReason::EnvConfig,
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
            Self::StateRootNotDotForgeMethod { path, state_root } => write!(
                f,
                "Forge Project Link '{path}' is invalid: state_root '{state_root}' must end with .forge-method"
            ),
            Self::MissingProjectLink { root } => write!(
                f,
                "missing Forge Project Link at '{root}\\{PROJECT_LINK_FILE_NAME}'; consumer projects must point at a Forge Runtime Sidecar"
            ),
        }
    }
}

impl fmt::Display for ProjectInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotFound { root } => write!(f, "project root does not exist: {root}"),
            Self::RootNotDirectory { root } => {
                write!(f, "project root is not a directory: {root}")
            }
            Self::RootCanonicalize { root, source } => {
                write!(f, "could not canonicalize project root '{root}': {source}")
            }
            Self::MissingRootDirectoryName { root } => write!(
                f,
                "could not derive default project_id because project root '{root}' has no directory name"
            ),
            Self::UnsafeProjectId { project_id, reason } => write!(
                f,
                "unsafe project_id '{project_id}': {reason}; expected lowercase ASCII alphanumeric segments separated by single '-'"
            ),
            Self::EmptyPathField { field } => {
                write!(f, "project init: --{field} requires a non-empty path")
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
            Self::ConsumerLocalStateExists { path } => write!(
                f,
                "consumer project already has local Forge state at '{path}'; move or quarantine it into a Forge Runtime Sidecar before running project init"
            ),
            Self::StateRootNotDotForgeMethod { path, state_root } => write!(
                f,
                "Forge Project Link '{path}' is invalid: state_root '{state_root}' must end with .forge-method"
            ),
            Self::ExistingProjectLinkInvalid { path, source, .. } => write!(
                f,
                "existing Forge Project Link '{path}' is invalid; refusing to overwrite: {source}"
            ),
            Self::ExistingProjectLinkMismatch {
                path,
                expected_project_id,
                found_project_id,
                expected_sidecar_root,
                found_sidecar_root,
                expected_state_root,
                found_state_root,
            } => write!(
                f,
                "existing Forge Project Link '{path}' differs; refusing to overwrite: expected project_id='{expected_project_id}', sidecar_root='{expected_sidecar_root}', state_root='{expected_state_root}' but found project_id='{found_project_id}', sidecar_root='{found_sidecar_root}', state_root='{found_state_root}'"
            ),
            Self::ProjectLinkSerialize { source } => {
                write!(f, "could not serialize Forge Project Link: {source}")
            }
            Self::CreateStateDir { path, source } => {
                write!(f, "could not create Forge state directory '{path}': {source}")
            }
            Self::LedgerCreate { path, source } => {
                write!(f, "could not create Forge ledger '{path}': {source}")
            }
            Self::LedgerSync { path, source } => {
                write!(f, "could not sync Forge ledger '{path}': {source}")
            }
            Self::LedgerNotFile { path } => {
                write!(f, "Forge ledger path exists but is not a file: {path}")
            }
            Self::LinkTempCreate { path, source } => write!(
                f,
                "could not create temporary Forge Project Link '{path}': {source}"
            ),
            Self::LinkTempWrite { path, source } => write!(
                f,
                "could not write temporary Forge Project Link '{path}': {source}"
            ),
            Self::LinkTempSync { path, source } => write!(
                f,
                "could not sync temporary Forge Project Link '{path}': {source}"
            ),
            Self::LinkRename {
                temp_path,
                link_path,
                source,
            } => write!(
                f,
                "could not atomically install Forge Project Link '{temp_path}' -> '{link_path}': {source}"
            ),
            Self::LinkExistsRace { path } => write!(
                f,
                "Forge Project Link '{path}' appeared during init; refusing to overwrite"
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

#[must_use]
pub fn run_init(
    root: &Path,
    project_id: Option<&str>,
    sidecar_root: Option<&Path>,
    state_root: Option<&Path>,
) -> CliEnvelope<ProjectInitPayload> {
    match init_project(root, project_id, sidecar_root, state_root) {
        Ok(payload) => CliEnvelope::ok("project.init", payload),
        Err(err) => CliEnvelope::err("project.init", err.exit_reason(), err.to_string()),
    }
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectInitPlan {
    project_id: String,
    project_root: PathBuf,
    link_path: PathBuf,
    sidecar_link: String,
    state_link: String,
    sidecar_root: PathBuf,
    state_root: PathBuf,
}

pub fn init_project(
    root: &Path,
    project_id: Option<&str>,
    sidecar_root: Option<&Path>,
    state_root: Option<&Path>,
) -> Result<ProjectInitPayload, ProjectInitError> {
    let root = canonical_project_root_for_init(root)?;
    let plan = build_init_plan(&root, project_id, sidecar_root, state_root)?;
    reject_existing_consumer_local_state(&plan.project_root)?;
    if plan.link_path.exists() {
        return init_existing_project_link(&plan);
    }
    create_state_tree(&plan.state_root)?;
    write_project_link_atomically(&plan)?;
    Ok(project_init_payload(&plan, ProjectInitStatus::Initialized))
}

fn canonical_project_root_for_init(root: &Path) -> Result<PathBuf, ProjectInitError> {
    if !root.exists() {
        return Err(ProjectInitError::RootNotFound {
            root: display_path(root),
        });
    }
    if !root.is_dir() {
        return Err(ProjectInitError::RootNotDirectory {
            root: display_path(root),
        });
    }
    root.canonicalize()
        .map_err(|source| ProjectInitError::RootCanonicalize {
            root: display_path(root),
            source: source.to_string(),
        })
}

fn build_init_plan(
    project_root: &Path,
    project_id: Option<&str>,
    sidecar_root: Option<&Path>,
    state_root: Option<&Path>,
) -> Result<ProjectInitPlan, ProjectInitError> {
    let project_id = match project_id {
        Some(raw) => validate_project_id(raw)?,
        None => default_project_id(project_root)?,
    };
    let default_sidecar = format!("../forge-{project_id}");
    let sidecar_link = sidecar_root
        .map(path_arg_to_link_value)
        .unwrap_or(default_sidecar);
    validate_link_path_value(&sidecar_link, "sidecar-root")?;

    let state_link = state_root
        .map(path_arg_to_link_value)
        .unwrap_or_else(|| default_state_root_for_sidecar(&sidecar_link));
    validate_link_path_value(&state_link, "state-root")?;

    let link_path = project_root.join(PROJECT_LINK_FILE_NAME);
    let sidecar_root = resolve_repo_path(project_root, &sidecar_link);
    let state_root = resolve_repo_path(project_root, &state_link);
    validate_init_resolved_sidecar_paths(project_root, &link_path, &sidecar_root, &state_root)?;

    Ok(ProjectInitPlan {
        project_id,
        project_root: project_root.to_path_buf(),
        link_path,
        sidecar_link,
        state_link,
        sidecar_root,
        state_root,
    })
}

fn default_project_id(project_root: &Path) -> Result<String, ProjectInitError> {
    let Some(name) = project_root.file_name() else {
        return Err(ProjectInitError::MissingRootDirectoryName {
            root: display_path(project_root),
        });
    };
    let raw = name.to_string_lossy();
    let project_id = slugify_project_id(&raw);
    if project_id.is_empty() {
        return Err(ProjectInitError::UnsafeProjectId {
            project_id: raw.into_owned(),
            reason: "root directory name does not contain ASCII alphanumeric characters",
        });
    }
    Ok(project_id)
}

fn validate_project_id(raw: &str) -> Result<String, ProjectInitError> {
    let project_id = raw.trim();
    if project_id.is_empty() {
        return Err(ProjectInitError::UnsafeProjectId {
            project_id: raw.to_string(),
            reason: "project id is empty",
        });
    }
    let safe = slugify_project_id(project_id);
    if safe != project_id {
        return Err(ProjectInitError::UnsafeProjectId {
            project_id: raw.to_string(),
            reason: "project id is not already a safe slug",
        });
    }
    Ok(project_id.to_string())
}

fn slugify_project_id(raw: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() {
            slug.push(char::from(byte.to_ascii_lowercase()));
            last_was_separator = false;
        } else if !slug.is_empty() && !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }
    if last_was_separator {
        slug.pop();
    }
    slug
}

fn path_arg_to_link_value(path: &Path) -> String {
    display_path(path)
}

fn validate_link_path_value(raw: &str, field: &'static str) -> Result<(), ProjectInitError> {
    if raw.trim().is_empty() {
        return Err(ProjectInitError::EmptyPathField { field });
    }
    Ok(())
}

fn default_state_root_for_sidecar(sidecar_link: &str) -> String {
    let sidecar = sidecar_link.trim_end_matches(|character| character == '/' || character == '\\');
    if sidecar.is_empty() {
        ".forge-method".to_string()
    } else {
        format!("{sidecar}/.forge-method")
    }
}

fn state_root_ends_with_dot_forge_method(state_root: &Path) -> bool {
    state_root
        .file_name()
        .is_some_and(|name| name == std::ffi::OsStr::new(".forge-method"))
}

fn path_starts_with_for_policy(path: &Path, base: &Path) -> bool {
    let path_components = comparable_path_components(path);
    let base_components = comparable_path_components(base);
    base_components.len() <= path_components.len()
        && path_components
            .iter()
            .zip(base_components.iter())
            .all(|(path_component, base_component)| path_component == base_component)
}

fn comparable_path_components(path: &Path) -> Vec<String> {
    let normalized = normalize_path(path.to_path_buf());
    let display = display_path(&normalized);
    PathBuf::from(display)
        .components()
        .map(|component| comparable_component(component.as_os_str().to_string_lossy().as_ref()))
        .collect()
}

#[cfg(windows)]
fn comparable_component(component: &str) -> String {
    component.to_ascii_lowercase()
}

#[cfg(not(windows))]
fn comparable_component(component: &str) -> String {
    component.to_string()
}

fn validate_init_resolved_sidecar_paths(
    project_root: &Path,
    link_path: &Path,
    sidecar_root: &Path,
    state_root: &Path,
) -> Result<(), ProjectInitError> {
    if !state_root_ends_with_dot_forge_method(state_root) {
        return Err(ProjectInitError::StateRootNotDotForgeMethod {
            path: display_path(link_path),
            state_root: display_path(state_root),
        });
    }
    if !path_starts_with_for_policy(state_root, sidecar_root) {
        return Err(ProjectInitError::StateRootOutsideSidecar {
            path: display_path(link_path),
            state_root: display_path(state_root),
            sidecar_root: display_path(sidecar_root),
        });
    }
    if path_starts_with_for_policy(state_root, project_root) {
        return Err(ProjectInitError::ConsumerLocalStateRoot {
            path: display_path(link_path),
            state_root: display_path(state_root),
            project_root: display_path(project_root),
        });
    }
    Ok(())
}

fn reject_existing_consumer_local_state(project_root: &Path) -> Result<(), ProjectInitError> {
    let local_state = project_root.join(".forge-method");
    if local_state.exists() {
        Err(ProjectInitError::ConsumerLocalStateExists {
            path: display_path(&local_state),
        })
    } else {
        Ok(())
    }
}

fn init_existing_project_link(
    plan: &ProjectInitPlan,
) -> Result<ProjectInitPayload, ProjectInitError> {
    let existing = resolve_from_link(&plan.project_root, &plan.link_path).map_err(|source| {
        ProjectInitError::ExistingProjectLinkInvalid {
            path: display_path(&plan.link_path),
            exit_reason: source.exit_reason(),
            source: source.to_string(),
        }
    })?;

    let expected_sidecar_root = display_path(&plan.sidecar_root);
    let expected_state_root = display_path(&plan.state_root);
    if existing.project_id != plan.project_id
        || existing.sidecar_root != expected_sidecar_root
        || existing.state_root != expected_state_root
    {
        return Err(ProjectInitError::ExistingProjectLinkMismatch {
            path: display_path(&plan.link_path),
            expected_project_id: plan.project_id.clone(),
            found_project_id: existing.project_id,
            expected_sidecar_root,
            found_sidecar_root: existing.sidecar_root,
            expected_state_root,
            found_state_root: existing.state_root,
        });
    }

    create_state_tree(&plan.state_root)?;
    Ok(project_init_payload(
        plan,
        ProjectInitStatus::AlreadyInitialized,
    ))
}

fn project_init_payload(plan: &ProjectInitPlan, status: ProjectInitStatus) -> ProjectInitPayload {
    ProjectInitPayload {
        status,
        project_id: plan.project_id.clone(),
        project_root: display_path(&plan.project_root),
        link_path: display_path(&plan.link_path),
        sidecar_root: display_path(&plan.sidecar_root),
        state_root: display_path(&plan.state_root),
        state_exists: plan.state_root.exists(),
    }
}

fn create_state_tree(state_root: &Path) -> Result<(), ProjectInitError> {
    let dirs = [
        state_root.to_path_buf(),
        state_root.join("artifacts"),
        state_root.join("claims-active"),
        state_root.join("evidence"),
        state_root.join("handoffs").join("expired-claims"),
        state_root.join("index"),
        state_root.join("locks"),
        state_root.join("traces"),
        state_root.join("wal"),
    ];
    for dir in dirs {
        fs::create_dir_all(&dir).map_err(|source| ProjectInitError::CreateStateDir {
            path: display_path(&dir),
            source: source.to_string(),
        })?;
    }

    let ledger = state_root.join("ledger.ndjson");
    if ledger.exists() {
        if !ledger.is_file() {
            return Err(ProjectInitError::LedgerNotFile {
                path: display_path(&ledger),
            });
        }
        return Ok(());
    }

    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ledger)
    {
        Ok(file) => file
            .sync_all()
            .map_err(|source| ProjectInitError::LedgerSync {
                path: display_path(&ledger),
                source: source.to_string(),
            }),
        Err(source) if source.kind() == ErrorKind::AlreadyExists => {
            if ledger.is_file() {
                Ok(())
            } else {
                Err(ProjectInitError::LedgerNotFile {
                    path: display_path(&ledger),
                })
            }
        }
        Err(source) => Err(ProjectInitError::LedgerCreate {
            path: display_path(&ledger),
            source: source.to_string(),
        }),
    }
}

fn write_project_link_atomically(plan: &ProjectInitPlan) -> Result<(), ProjectInitError> {
    let document = ProjectLinkDocument {
        schema_version: PROJECT_LINK_SCHEMA_VERSION.to_string(),
        project_id: StableId(plan.project_id.clone()),
        sidecar_root: RepoPath(plan.sidecar_link.clone()),
        state_root: RepoPath(plan.state_link.clone()),
    };
    let mut raw = serde_yaml::to_string(&document).map_err(|source| {
        ProjectInitError::ProjectLinkSerialize {
            source: source.to_string(),
        }
    })?;
    if !raw.ends_with('\n') {
        raw.push('\n');
    }

    let temp_path = temp_project_link_path(&plan.link_path);
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|source| ProjectInitError::LinkTempCreate {
            path: display_path(&temp_path),
            source: source.to_string(),
        })?;
    temp.write_all(raw.as_bytes())
        .map_err(|source| ProjectInitError::LinkTempWrite {
            path: display_path(&temp_path),
            source: source.to_string(),
        })?;
    temp.sync_all()
        .map_err(|source| ProjectInitError::LinkTempSync {
            path: display_path(&temp_path),
            source: source.to_string(),
        })?;
    drop(temp);

    match fs::rename(&temp_path, &plan.link_path) {
        Ok(()) => Ok(()),
        Err(_source) if plan.link_path.exists() => {
            let _ = fs::remove_file(&temp_path);
            Err(ProjectInitError::LinkExistsRace {
                path: display_path(&plan.link_path),
            })
        }
        Err(source) => {
            let source = source.to_string();
            let _ = fs::remove_file(&temp_path);
            Err(ProjectInitError::LinkRename {
                temp_path: display_path(&temp_path),
                link_path: display_path(&plan.link_path),
                source,
            })
        }
    }
}

fn temp_project_link_path(link_path: &Path) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp_name = format!(
        ".{}.tmp-{}-{suffix}",
        PROJECT_LINK_FILE_NAME.trim_start_matches('.'),
        std::process::id()
    );
    link_path.with_file_name(temp_name)
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
    if !state_root_ends_with_dot_forge_method(state_root) {
        return Err(ProjectResolveError::StateRootNotDotForgeMethod {
            path: display_path(link_path),
            state_root: display_path(state_root),
        });
    }
    if !path_starts_with_for_policy(state_root, sidecar_root) {
        return Err(ProjectResolveError::StateRootOutsideSidecar {
            path: display_path(link_path),
            state_root: display_path(state_root),
            sidecar_root: display_path(sidecar_root),
        });
    }
    if path_starts_with_for_policy(state_root, project_root) {
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
        "init" => dispatch_init(&args[2..]),
        "resolve" => dispatch_resolve(&args[2..]),
        "--help" | "-h" | "help" => (project_usage().to_string(), 0),
        other => (
            format!("forge-core project: unknown subcommand '{other}'. Try: init | resolve"),
            2,
        ),
    }
}

fn dispatch_init(args: &[String]) -> (String, i32) {
    let mut root = PathBuf::from(".");
    let mut project_id: Option<String> = None;
    let mut sidecar_root: Option<PathBuf> = None;
    let mut state_root: Option<PathBuf> = None;
    let mut want_json = true;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return ("project init: --root requires a value".to_string(), 3);
                };
                root = PathBuf::from(value);
            }
            "--project-id" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return ("project init: --project-id requires a value".to_string(), 3);
                };
                project_id = Some(value.clone());
            }
            "--sidecar-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return (
                        "project init: --sidecar-root requires a value".to_string(),
                        3,
                    );
                };
                sidecar_root = Some(PathBuf::from(value));
            }
            "--state-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return ("project init: --state-root requires a value".to_string(), 3);
                };
                state_root = Some(PathBuf::from(value));
            }
            "--json" => want_json = true,
            "--no-json" => want_json = false,
            "--help" | "-h" => return (project_usage().to_string(), 0),
            other => {
                return (format!("project init: unrecognized argument '{other}'"), 3);
            }
        }
        index += 1;
    }

    let envelope = run_init(
        &root,
        project_id.as_deref(),
        sidecar_root.as_deref(),
        state_root.as_deref(),
    );
    let exit_code = envelope.exit_code();
    if want_json {
        (
            serde_json::to_string_pretty(&envelope).expect("serialize project init envelope"),
            exit_code,
        )
    } else if let Some(data) = envelope.data.as_ref() {
        (
            format!(
                "project={} status={} link_path={} state_root={}",
                data.project_id,
                data.status.as_str(),
                data.link_path,
                data.state_root
            ),
            exit_code,
        )
    } else {
        (
            envelope
                .error
                .as_ref()
                .map(|err| err.message.clone())
                .unwrap_or_else(|| "project init failed".to_string()),
            exit_code,
        )
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
    "forge-core project <subcommand> [options]\n  init [--root <path>] [--project-id <id>] [--sidecar-root <path>] [--state-root <path>] [--json|--no-json]\n  resolve [--root <path>] [--allow-bootstrap-core] [--json|--no-json]"
}

pub fn run_project_command(args: &[String]) -> Result<(), ExitError> {
    let (output, exit) = dispatch(args);
    if !output.is_empty() {
        println!("{output}");
    }
    if exit == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(exit, String::new()))
    }
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
    fn rejects_state_root_without_dot_forge_method_leaf() {
        let parent = temp_root("missing-dot-forge-method");
        let app = parent.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join(PROJECT_LINK_FILE_NAME),
            "schema_version: forge_project_link_v1
project_id: app
sidecar_root: ../forge-app
state_root: ../forge-app/state
",
        )
        .unwrap();

        let err = resolve_project(&app, false).unwrap_err();

        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
        assert!(err.to_string().contains(".forge-method"));
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
