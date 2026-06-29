use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_contracts::{EvalRunContractDocument, RepoPath};
use forge_core_eval::{
    compare_eval_runs, EvalArmLabel, EvalCompareSuiteDocument, EvalComparisonReport, EvalRunInput,
};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_EVAL_COMPARE_SUITE: &str =
    "docs/fixtures/eval-run-v0/eval-compare-smoke-suite.yaml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalCompareCommandInput {
    pub root: PathBuf,
    pub suite_path: Option<PathBuf>,
    pub baseline: EvalArmLabel,
    pub candidate: EvalArmLabel,
    pub allow_bootstrap_core: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalCommandError {
    ProjectResolve(ProjectResolveError),
    ReadSuite { path: PathBuf, source: String },
    ParseSuite { path: PathBuf, source: String },
    UnsupportedSuiteSchemaVersion { path: PathBuf, found: String },
    ReadRun { path: PathBuf, source: String },
    ParseRun { path: PathBuf, source: String },
    UnsupportedRunSchemaVersion { path: PathBuf, found: String },
    InvalidRunPath { path: String },
}

impl fmt::Display for EvalCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::ReadSuite { path, source } => {
                write!(
                    formatter,
                    "read eval suite {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseSuite { path, source } => {
                write!(
                    formatter,
                    "parse eval suite {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsupportedSuiteSchemaVersion { path, found } => write!(
                formatter,
                "eval suite {} has unsupported schema_version '{}', expected 0.1",
                path.display(),
                found
            ),
            Self::ReadRun { path, source } => {
                write!(
                    formatter,
                    "read eval run {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseRun { path, source } => {
                write!(
                    formatter,
                    "parse eval run {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsupportedRunSchemaVersion { path, found } => write!(
                formatter,
                "eval run {} has unsupported schema_version '{}', expected 0.1",
                path.display(),
                found
            ),
            Self::InvalidRunPath { path } => write!(
                formatter,
                "eval run ref '{path}' is invalid; refs must stay under the project root"
            ),
        }
    }
}

impl std::error::Error for EvalCommandError {}

impl From<ProjectResolveError> for EvalCommandError {
    fn from(error: ProjectResolveError) -> Self {
        Self::ProjectResolve(error)
    }
}

/// Runs fixture-backed eval comparison for precomputed eval-run contracts.
///
/// # Errors
///
/// Returns an error when project resolution fails, the suite or run fixtures
/// cannot be read or parsed, or a run ref escapes the resolved project root.
pub fn run_compare(
    input: &EvalCompareCommandInput,
) -> Result<EvalComparisonReport, EvalCommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let suite_path = resolve_project_relative_path(
        &project_root,
        input
            .suite_path
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_EVAL_COMPARE_SUITE)),
    )?;
    let suite_document = read_suite(&suite_path)?;
    let suite = suite_document.eval_compare_suite;
    let baseline_runs = read_run_inputs(&project_root, &suite.baseline.run_refs)?;
    let candidate_runs = read_run_inputs(&project_root, &suite.candidate.run_refs)?;

    Ok(compare_eval_runs(
        &suite,
        input.baseline,
        input.candidate,
        &baseline_runs,
        &candidate_runs,
    ))
}

fn read_suite(path: &Path) -> Result<EvalCompareSuiteDocument, EvalCommandError> {
    let text = fs::read_to_string(path).map_err(|source| EvalCommandError::ReadSuite {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    let document: EvalCompareSuiteDocument =
        serde_yaml::from_str(strip_utf8_bom(&text)).map_err(|source| {
            EvalCommandError::ParseSuite {
                path: path.to_path_buf(),
                source: source.to_string(),
            }
        })?;
    if document.schema_version != "0.1" {
        return Err(EvalCommandError::UnsupportedSuiteSchemaVersion {
            path: path.to_path_buf(),
            found: document.schema_version,
        });
    }
    Ok(document)
}

fn read_run_inputs(
    project_root: &Path,
    refs: &[RepoPath],
) -> Result<Vec<EvalRunInput>, EvalCommandError> {
    refs.iter()
        .map(|reference| read_run_input(project_root, reference))
        .collect()
}

fn read_run_input(
    project_root: &Path,
    reference: &RepoPath,
) -> Result<EvalRunInput, EvalCommandError> {
    let path = resolve_safe_repo_path(project_root, &reference.0)?;
    let text = fs::read_to_string(&path).map_err(|source| EvalCommandError::ReadRun {
        path: path.clone(),
        source: source.to_string(),
    })?;
    let document: EvalRunContractDocument =
        serde_yaml::from_str(strip_utf8_bom(&text)).map_err(|source| {
            EvalCommandError::ParseRun {
                path: path.clone(),
                source: source.to_string(),
            }
        })?;
    if document.schema_version != "0.1" {
        return Err(EvalCommandError::UnsupportedRunSchemaVersion {
            path,
            found: document.schema_version,
        });
    }
    Ok(EvalRunInput {
        source_ref: reference.clone(),
        document,
    })
}

fn resolve_project_relative_path(
    project_root: &Path,
    path: &Path,
) -> Result<PathBuf, EvalCommandError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        resolve_safe_repo_path(project_root, &path.to_string_lossy())
    }
}

fn resolve_safe_repo_path(
    project_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, EvalCommandError> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(forbidden_relative_component)
    {
        return Err(EvalCommandError::InvalidRunPath {
            path: relative_path.to_string(),
        });
    }
    Ok(project_root.join(path))
}

fn forbidden_relative_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

fn strip_utf8_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}
