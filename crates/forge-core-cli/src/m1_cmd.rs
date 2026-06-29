use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_contracts::OperationContractDocument;
use forge_core_runtime::{
    preview_operation_with_snapshot, ready_operation_with_snapshot, RuntimePreviewReport,
    RuntimeReadyReport,
};
use forge_core_store::{
    append_trace_event, build_reference_index, query_trace_events, ReferenceIndexBuildError,
    TraceEventQuery, TraceEventQueryResult,
};
use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum M1CommandKind {
    Preview,
    Ready,
    Explain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M1CommandInput {
    pub kind: M1CommandKind,
    pub root: PathBuf,
    pub operation_path: Option<PathBuf>,
    pub allow_bootstrap_core: bool,
    pub recorded_at: String,
    pub agent_id: String,
    pub principal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewCommandOutput {
    pub project_id: String,
    pub project_root: String,
    pub state_root: String,
    pub run_id: String,
    pub trace_id: String,
    pub trace_appended: bool,
    pub report: RuntimePreviewReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyCommandOutput {
    pub project_id: String,
    pub project_root: String,
    pub state_root: String,
    pub run_id: String,
    pub trace_id: String,
    pub trace_appended: bool,
    pub report: RuntimeReadyReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplainCommandOutput {
    pub project_id: String,
    pub project_root: String,
    pub state_root: String,
    pub query: TraceEventQueryResult,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M1CommandError {
    MissingOperationPath,
    ProjectResolve(ProjectResolveError),
    ReferenceIndexBuild(String),
    ReadOperation { path: PathBuf, source: String },
    ParseOperation { path: PathBuf, source: String },
    TraceAppend(String),
}

impl fmt::Display for M1CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOperationPath => write!(formatter, "--operation is required"),
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::ReferenceIndexBuild(message) => {
                write!(formatter, "reference index build failed: {message}")
            }
            Self::ReadOperation { path, source } => {
                write!(
                    formatter,
                    "read operation {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseOperation { path, source } => {
                write!(
                    formatter,
                    "parse operation {} failed: {source}",
                    path.display()
                )
            }
            Self::TraceAppend(message) => write!(formatter, "append trace event failed: {message}"),
        }
    }
}

impl std::error::Error for M1CommandError {}

/// Runs an operation preview against the resolved project and appends M1 trace events.
///
/// # Errors
///
/// Returns an error when project resolution fails, the operation file cannot be
/// read or parsed, the reference index cannot be built, or trace persistence
/// fails.
pub fn run_preview(input: &M1CommandInput) -> Result<PreviewCommandOutput, M1CommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)
        .map_err(M1CommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let state_root = PathBuf::from(&resolved.state_root);
    let operation_path = input
        .operation_path
        .as_ref()
        .ok_or(M1CommandError::MissingOperationPath)?;
    let operation_path = resolve_input_path(&project_root, operation_path.as_path());
    let operation = read_operation(&operation_path)?;
    let index =
        build_reference_index(&project_root).map_err(|error| reference_index_error(&error))?;
    let report = preview_operation_with_snapshot(
        &operation,
        forge_core_runtime::RuntimeReadSnapshot::new(&index),
    );
    let trace_id = stable_run_id("trace", &report.operation_id.0, &input.recorded_at);
    let run_id = stable_run_id("run", &report.operation_id.0, &input.recorded_at);
    let events = preview_trace_events(
        &resolved.project_id,
        &trace_id,
        &run_id,
        input,
        &report.operation_id.0,
        display_path(&operation_path),
        &report,
    );
    append_trace_events(&state_root, &events)?;

    Ok(PreviewCommandOutput {
        project_id: resolved.project_id,
        project_root: resolved.project_root,
        state_root: resolved.state_root,
        run_id,
        trace_id,
        trace_appended: true,
        report,
    })
}

/// Runs the fail-closed readiness gate and appends M1 trace events.
///
/// # Errors
///
/// Returns an error when project resolution fails, the operation file cannot be
/// read or parsed, the reference index cannot be built, or trace persistence
/// fails.
pub fn run_ready(input: &M1CommandInput) -> Result<ReadyCommandOutput, M1CommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)
        .map_err(M1CommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let state_root = PathBuf::from(&resolved.state_root);
    let operation_path = input
        .operation_path
        .as_ref()
        .ok_or(M1CommandError::MissingOperationPath)?;
    let operation_path = resolve_input_path(&project_root, operation_path.as_path());
    let operation = read_operation(&operation_path)?;
    let index =
        build_reference_index(&project_root).map_err(|error| reference_index_error(&error))?;
    let report = ready_operation_with_snapshot(
        &operation,
        forge_core_runtime::RuntimeReadSnapshot::new(&index),
    );
    let trace_id = stable_run_id("trace", &report.operation_id.0, &input.recorded_at);
    let run_id = stable_run_id("run", &report.operation_id.0, &input.recorded_at);
    let events = ready_trace_events(
        &resolved.project_id,
        &trace_id,
        &run_id,
        input,
        &report.operation_id.0,
        display_path(&operation_path),
        &report,
    );
    append_trace_events(&state_root, &events)?;

    Ok(ReadyCommandOutput {
        project_id: resolved.project_id,
        project_root: resolved.project_root,
        state_root: resolved.state_root,
        run_id,
        trace_id,
        trace_appended: true,
        report,
    })
}

/// Explains the latest M1 run from the resolved project's sidecar trace log.
///
/// # Errors
///
/// Returns an error when project resolution fails.
pub fn run_explain(input: &M1CommandInput) -> Result<ExplainCommandOutput, M1CommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)
        .map_err(M1CommandError::ProjectResolve)?;
    let state_root = PathBuf::from(&resolved.state_root);
    let query = query_trace_events(
        &state_root,
        &TraceEventQuery {
            latest_run: true,
            ..TraceEventQuery::default()
        },
    );
    let explanation = explain_trace_query(&query);
    Ok(ExplainCommandOutput {
        project_id: resolved.project_id,
        project_root: resolved.project_root,
        state_root: resolved.state_root,
        query,
        explanation,
    })
}

fn read_operation(path: &Path) -> Result<OperationContractDocument, M1CommandError> {
    let text = fs::read_to_string(path).map_err(|source| M1CommandError::ReadOperation {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    serde_yaml::from_str(&text).map_err(|source| M1CommandError::ParseOperation {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn resolve_input_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn reference_index_error(error: &ReferenceIndexBuildError) -> M1CommandError {
    M1CommandError::ReferenceIndexBuild(error.to_string())
}

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn stable_run_id(prefix: &str, operation_id: &str, recorded_at: &str) -> String {
    let sanitized_operation = sanitize_id(operation_id);
    let sanitized_time = sanitize_id(recorded_at);
    let sanitized_instance = sanitize_id(&unique_run_instance());
    format!("{prefix}.{sanitized_operation}.{sanitized_time}.{sanitized_instance}")
}

fn unique_run_instance() -> String {
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("pid-{}-nanos-{nanos}-seq-{sequence}", std::process::id())
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '.'
            }
        })
        .collect()
}

struct TraceOperationContext<'a> {
    project_id: &'a str,
    trace_id: &'a str,
    run_id: &'a str,
    input: &'a M1CommandInput,
    operation_id: &'a str,
}

fn trace_event_for_operation(
    context: &TraceOperationContext<'_>,
    event_id: &str,
    event_kind: TraceEventKind,
    risk: TraceRisk,
    inputs: Vec<TraceRef>,
    outputs: Vec<TraceRef>,
    message: String,
) -> TraceEvent {
    let event_id = format!("{}.{}", context.run_id, event_id);
    TraceEvent::new(
        context.trace_id,
        context.run_id,
        event_id,
        event_kind,
        &context.input.recorded_at,
        message,
    )
    .with_project_id(context.project_id)
    .with_actor(TraceActor::new(
        &context.input.principal_id,
        &context.input.agent_id,
        "driver",
    ))
    .with_authority(TraceAuthority::for_operation(context.operation_id))
    .with_risk(risk)
    .with_cost(TraceCost::zero())
    .with_inputs(inputs)
    .with_outputs(outputs)
}

fn preview_trace_events(
    project_id: &str,
    trace_id: &str,
    run_id: &str,
    input: &M1CommandInput,
    operation_id: &str,
    operation_ref: String,
    report: &RuntimePreviewReport,
) -> Vec<TraceEvent> {
    let inputs = vec![TraceRef::new("operation", operation_ref)];
    let outputs = preview_outputs(report);
    let risk = trace_risk(report.destructive, report.risk_level);
    let context = TraceOperationContext {
        project_id,
        trace_id,
        run_id,
        input,
        operation_id,
    };
    vec![
        trace_event_for_operation(
            &context,
            "evt.run.started",
            TraceEventKind::RunStarted,
            TraceRisk::unknown(),
            inputs.clone(),
            Vec::new(),
            format!("Run started for operation {operation_id}"),
        ),
        trace_event_for_operation(
            &context,
            "evt.operation.planned",
            TraceEventKind::OperationPlanned,
            risk.clone(),
            inputs.clone(),
            outputs.clone(),
            format!(
                "Operation {operation_id} planned with {} command(s), {} effect(s), and {} blocker(s)",
                report.command_refs.len(),
                report.effect_contract_refs.len(),
                report.blockers.len()
            ),
        ),
        trace_event_for_operation(
            &context,
            "evt.preview.completed",
            TraceEventKind::PreviewCompleted,
            risk.clone(),
            inputs.clone(),
            outputs.clone(),
            format!("Preview completed for operation {operation_id} with status {:?}", report.status),
        ),
        trace_event_for_operation(
            &context,
            "evt.run.completed",
            TraceEventKind::RunCompleted,
            risk,
            inputs,
            outputs,
            format!("Run completed for operation {operation_id}"),
        ),
    ]
}

fn ready_trace_events(
    project_id: &str,
    trace_id: &str,
    run_id: &str,
    input: &M1CommandInput,
    operation_id: &str,
    operation_ref: String,
    report: &RuntimeReadyReport,
) -> Vec<TraceEvent> {
    let inputs = vec![TraceRef::new("operation", operation_ref)];
    let outputs = ready_outputs(report);
    let risk_level = if report.ready {
        TraceRiskLevel::Low
    } else {
        TraceRiskLevel::Blocked
    };
    let risk = TraceRisk::new(risk_level, false);
    let gate_kind = if report.ready {
        TraceEventKind::GatePassed
    } else {
        TraceEventKind::GateBlocked
    };
    let gate_message = if report.ready {
        format!("Required gates passed for operation {operation_id}")
    } else {
        format!(
            "Required gates blocked operation {operation_id}: {:?}",
            report.blocking_reasons
        )
    };
    let context = TraceOperationContext {
        project_id,
        trace_id,
        run_id,
        input,
        operation_id,
    };
    vec![
        trace_event_for_operation(
            &context,
            "evt.run.started",
            TraceEventKind::RunStarted,
            TraceRisk::unknown(),
            inputs.clone(),
            Vec::new(),
            format!("Run started for operation {operation_id}"),
        ),
        trace_event_for_operation(
            &context,
            "evt.ready.completed",
            TraceEventKind::ReadyCompleted,
            risk.clone(),
            inputs.clone(),
            outputs.clone(),
            format!(
                "Ready completed for operation {operation_id} with status {:?}",
                report.status
            ),
        ),
        trace_event_for_operation(
            &context,
            "evt.gate.evaluated",
            gate_kind,
            risk.clone(),
            inputs.clone(),
            outputs.clone(),
            gate_message,
        ),
        trace_event_for_operation(
            &context,
            "evt.run.completed",
            TraceEventKind::RunCompleted,
            risk,
            inputs,
            outputs,
            format!("Run completed for operation {operation_id}"),
        ),
    ]
}

fn preview_outputs(report: &RuntimePreviewReport) -> Vec<TraceRef> {
    let mut outputs = Vec::new();
    for command_ref in &report.command_refs {
        outputs.push(TraceRef::new("command", command_ref.id.0.clone()));
    }
    for effect_ref in &report.effect_contract_refs {
        outputs.push(TraceRef::new("effect", effect_ref.0.clone()));
    }
    for gate_ref in &report.gate_contract_refs {
        outputs.push(TraceRef::new("gate", gate_ref.0.clone()));
    }
    outputs
}

fn ready_outputs(report: &RuntimeReadyReport) -> Vec<TraceRef> {
    let mut outputs = Vec::new();
    for gate_ref in &report.required_gate_refs {
        outputs.push(TraceRef::new("gate", gate_ref.0.clone()));
    }
    for reason in &report.blocking_reasons {
        outputs.push(TraceRef::new("blocker", format!("{reason:?}")));
    }
    outputs
}

fn append_trace_events(state_root: &Path, events: &[TraceEvent]) -> Result<(), M1CommandError> {
    for event in events {
        append_trace_event(state_root, event)
            .map_err(|error| M1CommandError::TraceAppend(error.to_string()))?;
    }
    Ok(())
}

fn trace_risk(destructive: bool, level: forge_core_runtime::RuntimeRiskLevel) -> TraceRisk {
    let risk_level = match level {
        forge_core_runtime::RuntimeRiskLevel::Low => TraceRiskLevel::Low,
        forge_core_runtime::RuntimeRiskLevel::Medium => TraceRiskLevel::Medium,
        forge_core_runtime::RuntimeRiskLevel::High => TraceRiskLevel::High,
        forge_core_runtime::RuntimeRiskLevel::Blocked => TraceRiskLevel::Blocked,
    };
    TraceRisk::new(risk_level, destructive)
}

fn explain_trace_query(query: &TraceEventQueryResult) -> String {
    if query.events.is_empty() {
        return "No trace events were found for the last run.".to_string();
    }
    let Some(first) = query.events.first() else {
        return "No trace events were found for the last run.".to_string();
    };
    let Some(last) = query.events.last() else {
        return "No trace events were found for the last run.".to_string();
    };
    let input_refs = first
        .inputs
        .iter()
        .map(|reference| format!("{}={}", reference.ref_kind, reference.reference))
        .collect::<Vec<_>>()
        .join(", ");
    let output_refs = query
        .events
        .iter()
        .flat_map(|event| event.outputs.iter())
        .map(|reference| format!("{}={}", reference.ref_kind, reference.reference))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Last run {} for trace {} recorded {} event(s). Inputs: [{}]. Outputs: [{}]. Final event {:?}: {}",
        first.run_id,
        first.trace_id,
        query.returned_events,
        input_refs,
        output_refs,
        last.event_kind,
        last.message
    )
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}
