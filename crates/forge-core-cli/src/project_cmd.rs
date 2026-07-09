//! Project sidecar resolution.
//!
//! Every project (including the Forge core repo itself) carries a small
//! `.forge-method.yaml` pointer to a sibling Forge Runtime Sidecar. There is
//! no special-case layout for the core repo — it is a consumer of its own
//! protocol, dogfooding the same path as any other project.

use forge_core_command_surface::COMMAND_PROJECT;
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
    /// Current phase read from `<state_root>/state.yaml` when present.
    /// `None` when the state file does not exist yet (fresh project); callers
    /// fall back to `1-discovery` as the funnel entry point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
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

/// Payload for [`ProjectInitError::ExistingProjectLinkMismatch`].
///
/// Boxed at the variant to keep `ProjectInitError` small: this payload alone
/// would dominate the enum layout (7 `String`s ~ 168 B), forcing every other
/// variant to carry the same weight on every `Result` return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLinkMismatch {
    pub path: String,
    pub expected_project_id: String,
    pub found_project_id: String,
    pub expected_sidecar_root: String,
    pub found_sidecar_root: String,
    pub expected_state_root: String,
    pub found_state_root: String,
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
    /// The consumer root is nested inside another git repository (it is not a
    /// git root of its own). Initializing Forge here would create sidecar
    /// state inside the parent repo. The user should run `git init` on the
    /// consumer root first, or pick a root that is its own repository.
    RootNestedInAnotherRepo {
        root: String,
        parent_repo: String,
    },
    /// The resolved sidecar root would land inside a different git repository
    /// than the consumer root. This pollutes that other repo with runtime
    /// state. The user should pass an explicit `--sidecar-root` outside any
    /// foreign repo, or run `forge-core start` from a root whose default
    /// `../forge-<id>` sibling is not inside another repository.
    SidecarInsideAnotherRepo {
        sidecar_root: String,
        foreign_repo: String,
        consumer_root: String,
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
    ExistingProjectLinkMismatch(Box<ProjectLinkMismatch>),
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
            | Self::ProjectLinkSerialize { .. }
            | Self::RootNestedInAnotherRepo { .. }
            | Self::SidecarInsideAnotherRepo { .. } => ExitReason::InvalidDecisionShape,
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
            Self::RootNestedInAnotherRepo { root, parent_repo } => write!(
                f,
                "consumer root '{root}' is nested inside another git repository ('{parent_repo}'); \
                 run 'git init' on the consumer root first, or pick a root that is its own \
                 repository, so Forge runtime state does not pollute the parent"
            ),
            Self::SidecarInsideAnotherRepo {
                sidecar_root,
                foreign_repo,
                consumer_root,
            } => write!(
                f,
                "resolved sidecar root '{sidecar_root}' would land inside git repository \
                 '{foreign_repo}', which is not the consumer root '{consumer_root}'; pass an \
                 explicit --sidecar-root outside any foreign repo, or run forge-core start from \
                 a root whose default ../forge-<id> sibling is not inside another repository"
            ),
            Self::StateRootNotDotForgeMethod { path, state_root } => write!(
                f,
                "Forge Project Link '{path}' is invalid: state_root '{state_root}' must end with .forge-method"
            ),
            Self::ExistingProjectLinkInvalid { path, source, .. } => write!(
                f,
                "existing Forge Project Link '{path}' is invalid; refusing to overwrite: {source}"
            ),
            Self::ExistingProjectLinkMismatch(mismatch) => write!(
                f,
                "existing Forge Project Link '{}' differs; refusing to overwrite: expected project_id='{}', sidecar_root='{}', state_root='{}' but found project_id='{}', sidecar_root='{}', state_root='{}'",
                mismatch.path,
                mismatch.expected_project_id,
                mismatch.expected_sidecar_root,
                mismatch.expected_state_root,
                mismatch.found_project_id,
                mismatch.found_sidecar_root,
                mismatch.found_state_root,
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
pub fn run_resolve(root: &Path) -> CliEnvelope<ProjectResolvePayload> {
    match resolve_project(root) {
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

/// Initializes a Forge project at `root`, creating the sidecar state tree
/// and writing the project link file atomically.
///
/// # Errors
///
/// Returns [`ProjectInitError::RootNotFound`] when `root` does not exist,
/// [`ProjectInitError::RootCanonicalize`] when the canonical path cannot be
/// resolved, and variants describing an existing project link, sidecar state
/// conflict, or atomic write failure when applicable.
pub fn init_project(
    root: &Path,
    project_id: Option<&str>,
    sidecar_root: Option<&Path>,
    state_root: Option<&Path>,
) -> Result<ProjectInitPayload, ProjectInitError> {
    let root = canonical_project_root_for_init(root)?;
    let plan = build_init_plan(&root, project_id, sidecar_root, state_root)?;
    reject_existing_consumer_local_state(&plan.project_root)?;
    validate_repo_identity(&plan.project_root, &plan.sidecar_root)?;
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
    let sidecar_link = sidecar_root.map_or(default_sidecar, path_arg_to_link_value);
    validate_link_path_value(&sidecar_link, "sidecar-root")?;

    let state_link = state_root.map_or_else(
        || default_state_root_for_sidecar(&sidecar_link),
        path_arg_to_link_value,
    );
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
    let sidecar = sidecar_link.trim_end_matches(['/', '\\']);
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

/// Find the nearest enclosing `.git` directory walking up from `start`.
/// Returns the repository root (the parent of the `.git` dir) when one is
/// found, or `None` when the walk reaches the filesystem root without finding
/// one. Used to detect nested repos and sidecars landing inside a foreign
/// repository.
///
/// Pure filesystem walk; does not invoke `git`. A `.git` file (worktree
/// pointer) is also recognized, since `git init` in a worktree writes a file
/// rather than a directory.
fn find_enclosing_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let git_marker = dir.join(".git");
        if git_marker.exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// Validate that the consumer root is its own git repository (not nested inside
/// another), and that the resolved sidecar root does not land inside a
/// *different* git repository than the consumer. This prevents the structural
/// incident where running `forge-core start` in a subfolder of an existing repo
/// pollutes the parent repo with sibling sidecar state.
///
/// The check is fail-closed: a consumer root with no `.git` at all (e.g. a
/// brand-new folder that has not been `git init`-ed yet) is allowed, because
/// `forge-core start` is a reasonable way to bootstrap a brand-new project
/// before its first commit. Only an *enclosing* `.git` (one above the consumer
/// root) is treated as a nesting violation.
fn validate_repo_identity(
    project_root: &Path,
    sidecar_root: &Path,
) -> Result<(), ProjectInitError> {
    // (a) Reject a consumer root nested inside another repo ONLY when the
    // consumer is not its own git root. A consumer that has run `git init` on
    // itself is a deliberate nested repository (common in monorepos and
    // worktrees); Forge respects that. The pollution scenario is a subfolder
    // that has NOT been initialized — it silently inherits the parent repo,
    // which is exactly the incident this check prevents.
    let consumer_is_own_repo = project_root.join(".git").exists();
    if !consumer_is_own_repo {
        if let Some(parent_git) = project_root.parent().and_then(find_enclosing_git_root) {
            return Err(ProjectInitError::RootNestedInAnotherRepo {
                root: display_path(project_root),
                parent_repo: display_path(&parent_git),
            });
        }
    }
    // (b) Reject a sidecar that lands inside a foreign repo. The sidecar's
    // enclosing git root must either be the consumer root itself (when the
    // consumer is a repo and the sidecar is somehow inside it — already
    // rejected by the state-root checks) or none (the sidecar lives in a
    // folder with no `.git`, the normal sibling case). A foreign `.git`
    // above the sidecar is the pollution scenario.
    if let Some(sidecar_git) = find_enclosing_git_root(sidecar_root) {
        // The sidecar living inside the consumer repo is handled by the
        // state-root placement checks; only a FOREIGN repo is rejected here.
        let sidecar_in_consumer = consumer_is_own_repo && sidecar_git == *project_root;
        if !sidecar_in_consumer {
            return Err(ProjectInitError::SidecarInsideAnotherRepo {
                sidecar_root: display_path(sidecar_root),
                foreign_repo: display_path(&sidecar_git),
                consumer_root: display_path(project_root),
            });
        }
    }
    Ok(())
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
        return Err(ProjectInitError::ExistingProjectLinkMismatch(Box::new(
            ProjectLinkMismatch {
                path: display_path(&plan.link_path),
                expected_project_id: plan.project_id.clone(),
                found_project_id: existing.project_id,
                expected_sidecar_root,
                found_sidecar_root: existing.sidecar_root,
                expected_state_root,
                found_state_root: existing.state_root,
            },
        )));
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

/// Write the initial `state.yaml` at `<state_root>/state.yaml` carrying the
/// funnel entry-point phase (`1-discovery`). Idempotent: if the file already
/// exists and parses, this is a no-op (does not overwrite a prior phase). Used
/// by `start` after a fresh `init_project` so the runtime has an authoritative
/// phase record instead of trusting the agent's `--phase` string.
///
/// # Errors
/// Returns `ProjectInitError::CreateStateDir` if the `state_root` cannot be
/// created or the file cannot be written.
pub fn write_initial_project_state(state_root: &Path) -> Result<(), ProjectInitError> {
    let path = state_root.join("state.yaml");
    if path.exists() {
        return Ok(());
    }
    fs::create_dir_all(state_root).map_err(|source| ProjectInitError::CreateStateDir {
        path: display_path(state_root),
        source: source.to_string(),
    })?;
    // Keep this minimal and hand-written (no schema migration needed for the
    // bootstrap record): the runtime reads `current_phase:` via
    // `read_current_phase`, which does a tolerant key scan.
    let body = "schema_version: forge_project_state_v1\n\
                current_phase: \"1-discovery\"\n\
                updated_at: null\n";
    let tmp = state_root.join(format!("state.yaml.tmp-{}", std::process::id()));
    std::fs::write(&tmp, body).map_err(|source| ProjectInitError::CreateStateDir {
        path: display_path(&tmp),
        source: source.to_string(),
    })?;
    std::fs::rename(&tmp, &path).map_err(|source| ProjectInitError::CreateStateDir {
        path: display_path(&path),
        source: source.to_string(),
    })
}

fn write_project_link_atomically(plan: &ProjectInitPlan) -> Result<(), ProjectInitError> {
    let document = ProjectLinkDocument {
        schema_version: PROJECT_LINK_SCHEMA_VERSION.to_string(),
        project_id: StableId(plan.project_id.clone()),
        sidecar_root: RepoPath(plan.sidecar_link.clone()),
        state_root: RepoPath(plan.state_link.clone()),
    };
    let mut raw = yaml_serde::to_string(&document).map_err(|source| {
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
        .map_or(0, |duration| duration.as_nanos());
    let temp_name = format!(
        ".{}.tmp-{}-{suffix}",
        PROJECT_LINK_FILE_NAME.trim_start_matches('.'),
        std::process::id()
    );
    link_path.with_file_name(temp_name)
}

/// Resolves a Forge project from `root`, following the `.forge-method.yaml`
/// link file (or falling back to bootstrap-core when allowed).
///
/// # Errors
///
/// Returns [`ProjectResolveError::RootNotFound`] when `root` is missing,
/// [`ProjectResolveError::RootCanonicalize`] when canonicalization fails,
/// and link/state-related variants when the project link points at a
/// non-existent or malformed state root.
pub fn resolve_project(root: &Path) -> Result<ProjectResolvePayload, ProjectResolveError> {
    if !root.exists() {
        return Err(ProjectResolveError::RootNotFound {
            root: display_path(root),
        });
    }
    let root = root
        .canonicalize()
        .map_err(|source| ProjectResolveError::RootCanonicalize {
            root: display_path(root),
            source: source.to_string(),
        })?;
    let link_path = root.join(PROJECT_LINK_FILE_NAME);
    if link_path.exists() {
        return resolve_from_link(&root, &link_path);
    }
    Err(ProjectResolveError::MissingProjectLink {
        root: display_path(&root),
    })
}

/// Read the current phase from `<state_root>/state.yaml`, returning the raw
/// phase tag (e.g. `"1-discovery"`) when present and parseable. Returns `None`
/// when the file is missing, unreadable, or carries no recognized `current_phase`
/// key. Errors are best-effort: phase is an advisory read, not a hard gate on
/// resolution (callers fall back to `1-discovery`).
fn read_current_phase(state_root: &Path) -> Option<String> {
    let path = state_root.join("state.yaml");
    let raw = fs::read_to_string(&path).ok()?;
    // Minimal parse: look for a `current_phase:` key. We do not require a full
    // ProjectStateDocument schema here so that a hand-edited or partial file
    // still surfaces a phase when the key is recognizable.
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("current_phase:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
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
        yaml_serde::from_str(strip_utf8_bom(&raw)).map_err(|source| {
            ProjectResolveError::LinkParse {
                path: display_path(link_path),
                source: source.to_string(),
            }
        })?;
    validate_link(&link, link_path)?;
    let sidecar_root = resolve_repo_path(project_root, &link.sidecar_root.0);
    let state_root = resolve_repo_path(project_root, &link.state_root.0);
    validate_resolved_sidecar_paths(project_root, link_path, &sidecar_root, &state_root)?;
    let state_exists = state_root.exists();
    let current_phase = if state_exists {
        read_current_phase(&state_root)
    } else {
        None
    };
    Ok(ProjectResolvePayload {
        project_id: link.project_id.0,
        project_root: display_path(project_root),
        link_path: Some(display_path(link_path)),
        sidecar_root: display_path(&sidecar_root),
        state_exists,
        state_root: display_path(&state_root),
        layout: ProjectLayoutKind::Sidecar,
        current_phase,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectArgs {
    Init,
    Resolve,
    Help,
}

/// Top-level `forge-core project` parser errors. Hand-rolled (no anyhow/thiserror).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectArgsError {
    UnknownSubcommand { subcommand: String },
}

fn parse_project_args(args: &[String]) -> Result<ProjectArgs, ProjectArgsError> {
    match args.get(1).map_or("--help", String::as_str) {
        "init" => Ok(ProjectArgs::Init),
        "resolve" => Ok(ProjectArgs::Resolve),
        "--help" | "-h" | "help" => Ok(ProjectArgs::Help),
        other => Err(ProjectArgsError::UnknownSubcommand {
            subcommand: other.to_string(),
        }),
    }
}

#[must_use]
pub fn dispatch(args: &[String]) -> (String, i32) {
    match parse_project_args(args) {
        Ok(ProjectArgs::Init) => dispatch_init(&args[2..]),
        Ok(ProjectArgs::Resolve) => dispatch_resolve(&args[2..]),
        Ok(ProjectArgs::Help) => (project_usage(), 0),
        Err(ProjectArgsError::UnknownSubcommand { subcommand }) => (
            project_message_with_usage(&format!(
                "forge-core project: unknown subcommand '{subcommand}'. Try: {hint}",
                hint = project_subcommand_hint()
            )),
            2,
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectInitArgs {
    root: PathBuf,
    project_id: Option<String>,
    sidecar_root: Option<PathBuf>,
    state_root: Option<PathBuf>,
    want_json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectInitArgsError {
    MissingValue { flag: &'static str },
    FlagAsValue { flag: &'static str, value: String },
    UnknownArgument { argument: String },
}

impl std::fmt::Display for ProjectInitArgsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue { flag } => {
                write!(formatter, "project init: {flag} requires a value")
            }
            Self::FlagAsValue { flag, value } => {
                write!(
                    formatter,
                    "project init: {flag} requires a value, got another flag '{value}'"
                )
            }
            Self::UnknownArgument { argument } => {
                write!(
                    formatter,
                    "project init: unrecognized argument '{argument}'"
                )
            }
        }
    }
}

impl std::error::Error for ProjectInitArgsError {}

fn require_project_init_value(
    args: &[String],
    index: usize,
    flag: &'static str,
) -> Result<String, ProjectInitArgsError> {
    match args.get(index) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(ProjectInitArgsError::FlagAsValue {
                flag,
                value: value.clone(),
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(ProjectInitArgsError::MissingValue { flag }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectInitParseOutcome {
    Run(ProjectInitArgs),
    Help,
}

fn parse_project_init_args(
    args: &[String],
) -> Result<ProjectInitParseOutcome, ProjectInitArgsError> {
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
                let value = require_project_init_value(args, index, "--root")?;
                root = PathBuf::from(value);
            }
            "--project-id" => {
                index += 1;
                let value = require_project_init_value(args, index, "--project-id")?;
                project_id = Some(value);
            }
            "--sidecar-root" => {
                index += 1;
                let value = require_project_init_value(args, index, "--sidecar-root")?;
                sidecar_root = Some(PathBuf::from(value));
            }
            "--state-root" => {
                index += 1;
                let value = require_project_init_value(args, index, "--state-root")?;
                state_root = Some(PathBuf::from(value));
            }
            "--json" => want_json = true,
            "--no-json" => want_json = false,
            "--help" | "-h" => return Ok(ProjectInitParseOutcome::Help),
            other => {
                return Err(ProjectInitArgsError::UnknownArgument {
                    argument: other.to_string(),
                });
            }
        }
        index += 1;
    }

    Ok(ProjectInitParseOutcome::Run(ProjectInitArgs {
        root,
        project_id,
        sidecar_root,
        state_root,
        want_json,
    }))
}

fn dispatch_init(args: &[String]) -> (String, i32) {
    let parsed = match parse_project_init_args(args) {
        Ok(ProjectInitParseOutcome::Run(parsed)) => parsed,
        Ok(ProjectInitParseOutcome::Help) => return (project_usage(), 0),
        Err(error) => return (project_init_error_with_usage(&error), 3),
    };

    let envelope = run_init(
        &parsed.root,
        parsed.project_id.as_deref(),
        parsed.sidecar_root.as_deref(),
        parsed.state_root.as_deref(),
    );
    let exit_code = envelope.exit_code();
    if parsed.want_json {
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
            envelope.error.as_ref().map_or_else(
                || "project init failed".to_string(),
                |err| err.message.clone(),
            ),
            exit_code,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectResolveArgs {
    root: PathBuf,
    want_json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectResolveArgsError {
    MissingValue { flag: &'static str },
    FlagAsValue { flag: &'static str, value: String },
    UnknownArgument { argument: String },
}

impl std::fmt::Display for ProjectResolveArgsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue { flag } => {
                write!(formatter, "project resolve: {flag} requires a value")
            }
            Self::FlagAsValue { flag, value } => {
                write!(
                    formatter,
                    "project resolve: {flag} requires a value, got another flag '{value}'"
                )
            }
            Self::UnknownArgument { argument } => {
                write!(
                    formatter,
                    "project resolve: unrecognized argument '{argument}'"
                )
            }
        }
    }
}

impl std::error::Error for ProjectResolveArgsError {}

fn require_project_resolve_value(
    args: &[String],
    index: usize,
    flag: &'static str,
) -> Result<String, ProjectResolveArgsError> {
    match args.get(index) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(ProjectResolveArgsError::FlagAsValue {
                flag,
                value: value.clone(),
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(ProjectResolveArgsError::MissingValue { flag }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectResolveParseOutcome {
    Run(ProjectResolveArgs),
    Help,
}

fn parse_project_resolve_args(
    args: &[String],
) -> Result<ProjectResolveParseOutcome, ProjectResolveArgsError> {
    let mut root = PathBuf::from(".");
    let mut want_json = true;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let value = require_project_resolve_value(args, index, "--root")?;
                root = PathBuf::from(value);
            }
            "--json" => want_json = true,
            "--no-json" => want_json = false,
            "--help" | "-h" => return Ok(ProjectResolveParseOutcome::Help),
            other => {
                return Err(ProjectResolveArgsError::UnknownArgument {
                    argument: other.to_string(),
                });
            }
        }
        index += 1;
    }

    Ok(ProjectResolveParseOutcome::Run(ProjectResolveArgs {
        root,
        want_json,
    }))
}

fn dispatch_resolve(args: &[String]) -> (String, i32) {
    let parsed = match parse_project_resolve_args(args) {
        Ok(ProjectResolveParseOutcome::Run(parsed)) => parsed,
        Ok(ProjectResolveParseOutcome::Help) => return (project_usage(), 0),
        Err(error) => return (project_resolve_error_with_usage(&error), 3),
    };

    let envelope = run_resolve(&parsed.root);
    let exit_code = envelope.exit_code();
    if parsed.want_json {
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
            envelope.error.as_ref().map_or_else(
                || "project resolve failed".to_string(),
                |err| err.message.clone(),
            ),
            exit_code,
        )
    }
}

fn project_usage() -> String {
    let mut usage = String::from("forge-core project <subcommand> [options]");
    for line in COMMAND_PROJECT.local_usage_lines() {
        usage.push('\n');
        usage.push_str("  ");
        usage.push_str(line);
    }
    usage
}

fn project_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_PROJECT
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core project <subcommand> [options]")
}

fn project_message_with_usage(message: &str) -> String {
    format!("{message}\n\nusage:\n{}", project_usage())
}

fn project_subcommand_message_with_usage(subcommand: &str, message: &str) -> String {
    format!(
        "{message}\n\nusage:\n  {}",
        project_usage_line_for(subcommand)
    )
}

fn project_init_error_with_usage(error: &ProjectInitArgsError) -> String {
    project_subcommand_message_with_usage("init", &error.to_string())
}

fn project_resolve_error_with_usage(error: &ProjectResolveArgsError) -> String {
    project_subcommand_message_with_usage("resolve", &error.to_string())
}

fn project_subcommand_names() -> impl Iterator<Item = &'static str> {
    COMMAND_PROJECT.concrete_subcommand_names()
}

fn project_subcommand_hint() -> String {
    project_subcommand_names().collect::<Vec<_>>().join(" | ")
}

/// Dispatch entrypoint for the `forge-core project` command tree
/// (`init`, `resolve`, `link`, etc.).
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the dispatcher's non-zero exit
/// code so the entrypoint can translate it into `process::exit(code)`.
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
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "forge-project-resolve-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    fn assert_project_error_projects_only_subcommand_usage(
        message: &str,
        subcommand: &str,
        expected_diagnostic: &str,
    ) {
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        let projected = COMMAND_PROJECT
            .usage_line_for_subcommand(subcommand)
            .expect("project subcommand usage");
        assert!(
            message.contains(projected),
            "error should project {subcommand} Command Surface usage {projected:?}: {message}"
        );
        for sibling in ["init", "resolve"] {
            if sibling != subcommand {
                let sibling_usage = COMMAND_PROJECT
                    .usage_line_for_subcommand(sibling)
                    .expect("sibling usage");
                assert!(
                    !message.contains(sibling_usage),
                    "error for {subcommand} should not leak {sibling} usage: {message}"
                );
            }
        }
    }

    #[test]
    fn parse_project_args_routes_top_level_subcommands() {
        assert_eq!(
            parse_project_args(&argv(&["project", "init"])),
            Ok(ProjectArgs::Init)
        );
        assert_eq!(
            parse_project_args(&argv(&["project", "resolve"])),
            Ok(ProjectArgs::Resolve)
        );
        assert_eq!(
            parse_project_args(&argv(&["project", "--help"])),
            Ok(ProjectArgs::Help)
        );
        assert_eq!(
            parse_project_args(&argv(&["project", "bogus"])),
            Err(ProjectArgsError::UnknownSubcommand {
                subcommand: "bogus".to_string()
            })
        );
    }

    #[test]
    fn project_usage_projects_command_surface_lines() {
        let usage = project_usage();
        assert!(
            usage.starts_with("forge-core project <subcommand> [options]"),
            "project usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_PROJECT.usage_lines {
            let subcommand_usage = COMMAND_PROJECT.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "project usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
    }

    #[test]
    fn project_unknown_subcommand_hint_comes_from_command_surface() {
        let (output, exit) = dispatch(&argv(&["project", "bogus"]));
        assert_eq!(exit, 2);
        assert!(
            output.contains(&project_subcommand_hint()),
            "unknown-subcommand hint should be projected from Command Surface: {output}"
        );
        assert!(
            output.contains("forge-core project <subcommand> [options]"),
            "unknown-subcommand error should include project usage: {output}"
        );
        for name in project_subcommand_names() {
            assert!(
                output.contains(name),
                "unknown-subcommand hint should name projected subcommand {name:?}: {output}"
            );
        }
    }

    #[test]
    fn project_help_paths_use_project_usage() {
        for args in [
            argv(&["project", "--help"]),
            argv(&["project", "init", "--help"]),
            argv(&["project", "resolve", "--help"]),
        ] {
            let (output, exit) = dispatch(&args);
            assert_eq!(exit, 0, "help path should succeed for args {args:?}");
            assert_eq!(output, project_usage());
        }
    }

    #[test]
    fn parse_project_init_args_returns_typed_options() {
        let parsed = parse_project_init_args(&argv(&[
            "--root",
            "app",
            "--project-id",
            "custom",
            "--sidecar-root",
            "../forge-app",
            "--state-root",
            "../forge-app/.forge-method",
            "--no-json",
        ]))
        .expect("parse init args");

        let ProjectInitParseOutcome::Run(options) = parsed else {
            panic!("expected runnable init options");
        };
        assert_eq!(options.root, PathBuf::from("app"));
        assert_eq!(options.project_id.as_deref(), Some("custom"));
        assert_eq!(
            options.sidecar_root.as_deref(),
            Some(Path::new("../forge-app"))
        );
        assert_eq!(
            options.state_root.as_deref(),
            Some(Path::new("../forge-app/.forge-method"))
        );
        assert!(!options.want_json);
    }

    #[test]
    fn parse_project_init_args_short_circuits_help() {
        let parsed = parse_project_init_args(&argv(&["--help"])).expect("parse init help");
        assert_eq!(parsed, ProjectInitParseOutcome::Help);
    }

    #[test]
    fn parse_project_init_args_reports_typed_errors() {
        let missing = parse_project_init_args(&argv(&["--sidecar-root"])).unwrap_err();
        assert_eq!(
            missing,
            ProjectInitArgsError::MissingValue {
                flag: "--sidecar-root"
            }
        );
        assert_eq!(
            missing.to_string(),
            "project init: --sidecar-root requires a value"
        );

        let unknown = parse_project_init_args(&argv(&["--surprise"])).unwrap_err();
        assert_eq!(
            unknown,
            ProjectInitArgsError::UnknownArgument {
                argument: "--surprise".to_string()
            }
        );
        assert_eq!(
            unknown.to_string(),
            "project init: unrecognized argument '--surprise'"
        );
    }

    #[test]
    fn project_init_parse_errors_project_init_usage() {
        let missing = parse_project_init_args(&argv(&["--sidecar-root"])).unwrap_err();
        let message = project_init_error_with_usage(&missing);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "init",
            "project init: --sidecar-root requires a value",
        );

        let flag_as_value =
            parse_project_init_args(&argv(&["--state-root", "--json"])).unwrap_err();
        assert_eq!(
            flag_as_value,
            ProjectInitArgsError::FlagAsValue {
                flag: "--state-root",
                value: "--json".to_string(),
            }
        );
        let message = project_init_error_with_usage(&flag_as_value);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "init",
            "project init: --state-root requires a value, got another flag '--json'",
        );

        let unknown = parse_project_init_args(&argv(&["--surprise"])).unwrap_err();
        let message = project_init_error_with_usage(&unknown);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "init",
            "project init: unrecognized argument '--surprise'",
        );
    }

    #[test]
    fn parse_project_resolve_args_returns_typed_options() {
        let parsed = parse_project_resolve_args(&argv(&["--root", "app", "--no-json"]))
            .expect("parse resolve args");

        let ProjectResolveParseOutcome::Run(options) = parsed else {
            panic!("expected runnable resolve options");
        };
        assert_eq!(options.root, PathBuf::from("app"));
        assert!(!options.want_json);
    }

    #[test]
    fn parse_project_resolve_args_short_circuits_help() {
        let parsed = parse_project_resolve_args(&argv(&["--help"])).expect("parse resolve help");
        assert_eq!(parsed, ProjectResolveParseOutcome::Help);
    }

    #[test]
    fn parse_project_resolve_args_reports_typed_errors() {
        let missing = parse_project_resolve_args(&argv(&["--root"])).unwrap_err();
        assert_eq!(
            missing,
            ProjectResolveArgsError::MissingValue { flag: "--root" }
        );
        assert_eq!(
            missing.to_string(),
            "project resolve: --root requires a value"
        );

        let unknown = parse_project_resolve_args(&argv(&["--surprise"])).unwrap_err();
        assert_eq!(
            unknown,
            ProjectResolveArgsError::UnknownArgument {
                argument: "--surprise".to_string()
            }
        );
        assert_eq!(
            unknown.to_string(),
            "project resolve: unrecognized argument '--surprise'"
        );
    }

    #[test]
    fn project_resolve_parse_errors_project_resolve_usage() {
        let missing = parse_project_resolve_args(&argv(&["--root"])).unwrap_err();
        let message = project_resolve_error_with_usage(&missing);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "resolve",
            "project resolve: --root requires a value",
        );

        let flag_as_value = parse_project_resolve_args(&argv(&["--root", "--json"])).unwrap_err();
        assert_eq!(
            flag_as_value,
            ProjectResolveArgsError::FlagAsValue {
                flag: "--root",
                value: "--json".to_string(),
            }
        );
        let message = project_resolve_error_with_usage(&flag_as_value);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "resolve",
            "project resolve: --root requires a value, got another flag '--json'",
        );

        let unknown = parse_project_resolve_args(&argv(&["--surprise"])).unwrap_err();
        let message = project_resolve_error_with_usage(&unknown);
        assert_project_error_projects_only_subcommand_usage(
            &message,
            "resolve",
            "project resolve: unrecognized argument '--surprise'",
        );
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

        let payload = resolve_project(&app).unwrap();

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
        let err = resolve_project(&root).unwrap_err();
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

        let err = resolve_project(&app).unwrap_err();

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

        let err = resolve_project(&app).unwrap_err();

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

        let err = resolve_project(&app).unwrap_err();

        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
        assert!(err.to_string().contains("project_root"));
        assert!(err.to_string().contains("Sidecar"));
    }

    /// Simulate a `.git` repository at `dir` for the repo-identity checks. An
    /// empty `.git` directory is enough: `find_enclosing_git_root` only tests
    /// `.git` existence (it does not invoke `git`).
    fn seed_git_repo(dir: &Path) {
        fs::create_dir_all(dir.join(".git")).expect("seed .git directory");
        assert!(dir.join(".git").is_dir(), ".git should be a directory");
    }

    /// Repo-identity validation (incident closure): a consumer root that is NOT
    /// its own git repository, but is nested inside a parent that has `.git`,
    /// must be rejected. This is the structural incident — running `forge-core
    /// start` in a subfolder of an existing repo would silently inherit the
    /// parent and pollute it with sibling sidecar state.
    #[test]
    fn init_rejects_consumer_nested_in_foreign_repo_without_own_git() {
        // Parent that looks like an existing git repository.
        let parent = temp_root("nested-foreign-parent");
        seed_git_repo(&parent);

        // Consumer root nested inside the parent, deliberately NOT git-init'd.
        let app = parent.join("app");
        fs::create_dir_all(&app).expect("create nested consumer root");

        let err = init_project(&app, None, None, None).unwrap_err();
        assert_eq!(
            err,
            ProjectInitError::RootNestedInAnotherRepo {
                root: display_path(&app),
                parent_repo: display_path(&parent),
            },
            "a consumer nested in a foreign repo without its own .git must be rejected"
        );
        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
    }

    /// A consumer that is nested inside another repo but has run `git init` on
    /// itself is a deliberate nested repository (common in monorepos /
    /// worktrees). Forge respects that and does NOT reject it — the nesting
    /// check only fires when the consumer silently inherits the parent repo.
    /// The sidecar must land somewhere that is not inside a foreign repo, so
    /// the test passes an explicit absolute sidecar root in a clean temp dir.
    #[test]
    fn init_allows_consumer_nested_in_foreign_repo_when_consumer_is_own_repo() {
        let parent = temp_root("nested-own-repo-parent");
        seed_git_repo(&parent);

        // Consumer root nested inside the parent, but it IS its own git repo.
        let app = parent.join("app");
        fs::create_dir_all(&app).expect("create nested consumer root");
        seed_git_repo(&app);

        // Sidecar must NOT resolve into the parent repo; put it in a separate
        // clean temp location with no `.git` above it.
        let sidecar = temp_root("nested-own-repo-sidecar");
        let payload = init_project(
            &app,
            None,
            Some(&sidecar),
            Some(&sidecar.join(".forge-method")),
        )
        .expect("a consumer that is its own repo is allowed even when nested");

        assert_eq!(payload.status, ProjectInitStatus::Initialized);
        assert!(app.join(PROJECT_LINK_FILE_NAME).is_file());
        assert!(sidecar.join(".forge-method").is_dir());
    }

    /// Repo-identity validation: a consumer that IS its own git repo, but whose
    /// resolved sidecar root lands inside a DIFFERENT git repository, must be
    /// rejected — that sidecar state would pollute the foreign repo.
    #[test]
    fn init_rejects_sidecar_inside_foreign_repo() {
        // The consumer is a clean repo of its own (no foreign parent).
        let app = temp_root("sidecar-foreign-consumer");
        seed_git_repo(&app);

        // A separate foreign git repository; the sidecar lands inside it.
        let foreign = temp_root("sidecar-foreign-repo");
        seed_git_repo(&foreign);
        let sidecar_in_foreign = foreign.join("forge-app");

        let err = init_project(
            &app,
            None,
            Some(&sidecar_in_foreign),
            Some(&sidecar_in_foreign.join(".forge-method")),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ProjectInitError::SidecarInsideAnotherRepo {
                sidecar_root: display_path(&sidecar_in_foreign),
                foreign_repo: display_path(&foreign),
                consumer_root: display_path(&app),
            },
            "a sidecar landing inside a foreign git repo must be rejected"
        );
        assert_eq!(err.exit_reason(), ExitReason::InvalidDecisionShape);
        // The failing init must not have created the sidecar state tree.
        assert!(
            !sidecar_in_foreign.join(".forge-method").exists(),
            "rejected sidecar init must not pollute the foreign repo"
        );
    }
}
