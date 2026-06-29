use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_graph::{dry_run_graph, validate_graph, WorkflowGraph};
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphCommandKind {
    Validate,
    RunDryRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCommandInput {
    pub root: PathBuf,
    pub graph_path: Option<PathBuf>,
    pub allow_bootstrap_core: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphCommandStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphValidateCommandOutput {
    pub project_id: String,
    pub project_root: String,
    pub state_root: String,
    pub graph_path: String,
    pub status: GraphCommandStatus,
    pub report: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRunCommandOutput {
    pub project_id: String,
    pub project_root: String,
    pub state_root: String,
    pub graph_path: String,
    pub status: GraphCommandStatus,
    pub dry_run_executed: bool,
    pub validation_report: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphCommandError {
    MissingGraphPath,
    ProjectResolve(ProjectResolveError),
    ReadGraph {
        path: PathBuf,
        source: String,
    },
    ParseGraph {
        path: PathBuf,
        source: String,
    },
    SerializeReport {
        report: &'static str,
        source: String,
    },
}

impl fmt::Display for GraphCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGraphPath => write!(formatter, "--graph is required"),
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::ReadGraph { path, source } => {
                write!(formatter, "read graph {} failed: {source}", path.display())
            }
            Self::ParseGraph { path, source } => {
                write!(formatter, "parse graph {} failed: {source}", path.display())
            }
            Self::SerializeReport { report, source } => {
                write!(formatter, "serialize {report} report failed: {source}")
            }
        }
    }
}

impl std::error::Error for GraphCommandError {}

/// Validates a workflow graph after resolving the Forge project root.
///
/// # Errors
///
/// Returns an error when project resolution fails, the graph path is missing,
/// the graph cannot be read or parsed, or the graph report cannot be serialized.
pub fn run_validate(
    input: &GraphCommandInput,
) -> Result<GraphValidateCommandOutput, GraphCommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)
        .map_err(GraphCommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let graph_path = resolve_graph_path(&project_root, input.graph_path.as_deref())?;
    let graph = read_graph(&graph_path)?;
    let report = validate_graph(&graph);
    let report = report_value("validation", report)?;
    let status = if validation_report_has_errors(&report) {
        GraphCommandStatus::Blocked
    } else {
        GraphCommandStatus::Passed
    };

    Ok(GraphValidateCommandOutput {
        project_id: resolved.project_id,
        project_root: resolved.project_root,
        state_root: resolved.state_root,
        graph_path: display_path(&graph_path),
        status,
        report,
    })
}

/// Runs a non-mutating graph dry-run after resolving the Forge project root.
///
/// # Errors
///
/// Returns an error when project resolution fails, the graph path is missing,
/// the graph cannot be read or parsed, or a graph report cannot be serialized.
pub fn run_dry_run(input: &GraphCommandInput) -> Result<GraphRunCommandOutput, GraphCommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)
        .map_err(GraphCommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let graph_path = resolve_graph_path(&project_root, input.graph_path.as_deref())?;
    let graph = read_graph(&graph_path)?;

    let validation_report = report_value("validation", validate_graph(&graph))?;
    if validation_report_has_errors(&validation_report) {
        return Ok(GraphRunCommandOutput {
            project_id: resolved.project_id,
            project_root: resolved.project_root,
            state_root: resolved.state_root,
            graph_path: display_path(&graph_path),
            status: GraphCommandStatus::Blocked,
            dry_run_executed: false,
            validation_report,
            report: None,
        });
    }

    let dry_run_report = report_value("dry-run", dry_run_graph(&graph))?;
    let status = if dry_run_report_is_blocked(&dry_run_report) {
        GraphCommandStatus::Blocked
    } else {
        GraphCommandStatus::Passed
    };

    Ok(GraphRunCommandOutput {
        project_id: resolved.project_id,
        project_root: resolved.project_root,
        state_root: resolved.state_root,
        graph_path: display_path(&graph_path),
        status,
        dry_run_executed: true,
        validation_report,
        report: Some(dry_run_report),
    })
}

fn read_graph(path: &Path) -> Result<WorkflowGraph, GraphCommandError> {
    let text = fs::read_to_string(path).map_err(|source| GraphCommandError::ReadGraph {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    serde_yaml::from_str(&text).map_err(|source| GraphCommandError::ParseGraph {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn resolve_graph_path(
    project_root: &Path,
    graph_path: Option<&Path>,
) -> Result<PathBuf, GraphCommandError> {
    let graph_path = graph_path.ok_or(GraphCommandError::MissingGraphPath)?;
    if graph_path.is_absolute() {
        Ok(graph_path.to_path_buf())
    } else {
        Ok(project_root.join(graph_path))
    }
}

fn report_value<T: Serialize>(report: &'static str, value: T) -> Result<Value, GraphCommandError> {
    serde_json::to_value(value).map_err(|source| GraphCommandError::SerializeReport {
        report,
        source: source.to_string(),
    })
}

fn validation_report_has_errors(report: &Value) -> bool {
    report
        .get("has_errors")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || report
            .get("errors")
            .and_then(Value::as_u64)
            .is_some_and(|errors| errors > 0)
        || report
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(diagnostic_is_error))
}

fn diagnostic_is_error(diagnostic: &Value) -> bool {
    diagnostic
        .get("severity")
        .and_then(Value::as_str)
        .is_some_and(|severity| severity == "error")
}

fn dry_run_report_is_blocked(report: &Value) -> bool {
    validation_report_has_errors(report) || value_contains_blocking_signal(report)
}

fn value_contains_blocking_signal(value: &Value) -> bool {
    match value {
        Value::Object(fields) => {
            has_blocking_field(fields) || fields.values().any(value_contains_blocking_signal)
        }
        Value::Array(items) => items.iter().any(value_contains_blocking_signal),
        _ => false,
    }
}

fn has_blocking_field(fields: &serde_json::Map<String, Value>) -> bool {
    fields
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(is_blocking_status)
        || fields
            .get("blocked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || fields
            .get("ready")
            .and_then(Value::as_bool)
            .is_some_and(|ready| !ready)
        || fields
            .get("blocking_reasons")
            .and_then(Value::as_array)
            .is_some_and(|reasons| !reasons.is_empty())
}

fn is_blocking_status(status: &str) -> bool {
    matches!(
        status,
        "blocked" | "failed" | "failure" | "invalid" | "not_ready"
    )
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}
