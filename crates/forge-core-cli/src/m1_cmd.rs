use crate::cli_error::ExitError;
use crate::cli_util::command_surface_usage;
use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_command_surface::{CommandSpec, COMMAND_EXPLAIN, COMMAND_PREVIEW, COMMAND_READY};
use forge_core_contracts::OperationContractDocument;
use forge_core_kernel::{
    preview_operation_with_snapshot, ready_operation_with_snapshot, RuntimePreviewReport,
    RuntimeReadyReport,
};
use forge_core_store::{
    append_trace_event, build_reference_index, query_trace_events, ReferenceIndexBuildError,
    TraceEventQuery, TraceEventQueryResult, TraceEventQueryStatus,
};
use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};
use serde::Serialize;
use std::fmt::{self, Write};
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

impl M1CommandKind {
    #[must_use]
    fn command_spec(self) -> &'static CommandSpec {
        match self {
            Self::Preview => &COMMAND_PREVIEW,
            Self::Ready => &COMMAND_READY,
            Self::Explain => &COMMAND_EXPLAIN,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M1CommandInput {
    pub kind: M1CommandKind,
    pub root: PathBuf,
    pub operation_path: Option<PathBuf>,
    pub recorded_at: String,
    pub agent_id: String,
    pub principal_id: String,
    /// When set, `forge explain` narates this specific run instead of the
    /// latest one. Ignored by `preview` and `ready`.
    pub run_id: Option<String>,
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
    MissingSidecarStateRoot {
        project_root: PathBuf,
        state_root: PathBuf,
    },
    ReferenceIndexBuild(String),
    ReadOperation {
        path: PathBuf,
        source: String,
    },
    ParseOperation {
        path: PathBuf,
        source: String,
    },
    TraceAppend(String),
}

impl fmt::Display for M1CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOperationPath => write!(formatter, "--operation is required"),
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::MissingSidecarStateRoot {
                project_root,
                state_root,
            } => write!(
                formatter,
                "env/config failure: Forge Project Link for project root '{}' resolves to missing sidecar state_root '{}'; restore the Forge Runtime Sidecar or run project init before M1 commands",
                project_root.display(),
                state_root.display()
            ),
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
    let resolved = resolve_project(&input.root).map_err(M1CommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let state_root = existing_state_root_for_m1(&resolved)?;
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
        forge_core_kernel::RuntimeReadSnapshot::new(&index),
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
    let resolved = resolve_project(&input.root).map_err(M1CommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let state_root = existing_state_root_for_m1(&resolved)?;
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
        forge_core_kernel::RuntimeReadSnapshot::new(&index),
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

/// Explains an M1 run from the resolved project's sidecar trace log.
///
/// When `input.run_id` is set, only events from that run are narrated. When
/// it is `None`, the latest run is explained.
///
/// # Errors
///
/// Returns an error when project resolution fails.
pub fn run_explain(input: &M1CommandInput) -> Result<ExplainCommandOutput, M1CommandError> {
    let resolved = resolve_project(&input.root).map_err(M1CommandError::ProjectResolve)?;
    let state_root = existing_state_root_for_m1(&resolved)?;
    let latest_run = input.run_id.is_none();
    let query = query_trace_events(
        &state_root,
        &TraceEventQuery {
            run_id: input.run_id.clone(),
            latest_run,
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

fn existing_state_root_for_m1(
    resolved: &crate::project_cmd::ProjectResolvePayload,
) -> Result<PathBuf, M1CommandError> {
    let state_root = PathBuf::from(&resolved.state_root);
    if !resolved.state_exists {
        return Err(M1CommandError::MissingSidecarStateRoot {
            project_root: PathBuf::from(&resolved.project_root),
            state_root,
        });
    }
    Ok(state_root)
}

fn read_operation(path: &Path) -> Result<OperationContractDocument, M1CommandError> {
    let text = fs::read_to_string(path).map_err(|source| M1CommandError::ReadOperation {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| M1CommandError::ParseOperation {
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

fn trace_risk(destructive: bool, level: forge_core_kernel::RuntimeRiskLevel) -> TraceRisk {
    let risk_level = match level {
        forge_core_kernel::RuntimeRiskLevel::Low => TraceRiskLevel::Low,
        forge_core_kernel::RuntimeRiskLevel::Medium => TraceRiskLevel::Medium,
        forge_core_kernel::RuntimeRiskLevel::High => TraceRiskLevel::High,
        forge_core_kernel::RuntimeRiskLevel::Blocked => TraceRiskLevel::Blocked,
    };
    TraceRisk::new(risk_level, destructive)
}

fn explain_trace_query(query: &TraceEventQueryResult) -> String {
    // Non-matched queries (missing trace file, parse failure, etc.) get a
    // compact diagnostic block instead of an empty narrative.
    if query.status != TraceEventQueryStatus::Matched {
        return narrate_non_matched(query);
    }
    if query.events.is_empty() {
        return format!(
            "Trace query matched but returned no events.\n  scanned: {} | matched: {}",
            query.scanned_events, query.matched_events,
        );
    }

    // NDJSON is append-only; events may arrive out of chronological order on
    // disk (e.g. interleaved writes). Sort by recorded_at (RFC3339 -> lexical
    // order is correct) for a stable narrative.
    let mut ordered: Vec<&TraceEvent> = query.events.iter().collect();
    ordered.sort_by(|a, b| a.recorded_at.cmp(&b.recorded_at));

    let first = ordered.first().expect("non-empty checked above");
    let last = ordered.last().expect("non-empty checked above");

    let mut out = String::new();
    narrate_header(&mut out, first, last, ordered.len());

    let mut totals = NarrateTotals::default();
    for (idx, event) in ordered.iter().enumerate() {
        narrate_event(&mut out, idx + 1, event, &mut totals);
    }
    narrate_summary(&mut out, ordered.len(), &totals);
    out
}

#[derive(Default)]
struct NarrateTotals {
    outputs: usize,
    model_calls: u64,
    tool_calls: u64,
    tokens: u64,
    peak_rank: u8,
}

fn narrate_non_matched(query: &TraceEventQueryResult) -> String {
    let reasons: Vec<String> = query.reasons.iter().map(|r| format!("{r:?}")).collect();
    let diagnostics = if query.diagnostics.is_empty() {
        String::from("  (no diagnostics)")
    } else {
        format!(
            "  diagnostics:\n{}",
            query
                .diagnostics
                .iter()
                .map(|d| format!("    - {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!(
        "Trace query did not match.\n  status: {:?}\n  reasons: [{}]\n  scanned: {} | matched: {} | returned: {}\n{diagnostics}",
        query.status,
        reasons.join(", "),
        query.scanned_events,
        query.matched_events,
        query.returned_events,
    )
}

fn narrate_header(out: &mut String, first: &TraceEvent, last: &TraceEvent, count: usize) {
    let project_id = first
        .project_id
        .clone()
        .unwrap_or_else(|| "<unknown>".to_string());
    let _ = writeln!(
        out,
        "Run {} (project: {})\n  trace: {}\n  agent: {} (principal: {}, role: {})\n  events: {} | span: {} -> {}\n",
        first.run_id,
        project_id,
        first.trace_id,
        first.actor.agent_id,
        first.actor.principal_id,
        first.actor.role,
        count,
        first.recorded_at,
        last.recorded_at,
    );
}

fn narrate_event(out: &mut String, idx: usize, event: &TraceEvent, totals: &mut NarrateTotals) {
    let kind_str = format!("{:?}", event.event_kind);
    let _ = writeln!(
        out,
        "[{}] {} {}: {}",
        idx, event.recorded_at, kind_str, event.message
    );
    let op_id = event.authority.operation_id.as_deref().unwrap_or("none");
    let caps = if event.authority.capability_ids.is_empty() {
        String::from("[]")
    } else {
        format!("[{}]", event.authority.capability_ids.join(", "))
    };
    let _ = writeln!(
        out,
        "      authority: operation={op_id} capabilities={caps}"
    );
    let rank = risk_level_rank(event.risk.risk_level);
    if rank > totals.peak_rank {
        totals.peak_rank = rank;
    }
    let _ = writeln!(
        out,
        "      risk: {} destructive={}",
        rank_to_level_str(rank),
        event.risk.destructive
    );
    let inputs = refs_summary(&event.inputs);
    let outputs = refs_summary(&event.outputs);
    totals.outputs += event.outputs.len();
    let _ = writeln!(out, "      inputs:  [{inputs}]");
    let _ = writeln!(out, "      outputs: [{outputs}]");
    totals.model_calls += event.cost.model_calls;
    totals.tool_calls += event.cost.tool_calls;
    totals.tokens += event.cost.estimated_tokens;
    out.push('\n');
}

fn narrate_summary(out: &mut String, count: usize, totals: &NarrateTotals) {
    let _ = writeln!(
        out,
        "Totals:\n  events: {} | outputs: {}\n  model_calls: {} | tool_calls: {} | estimated_tokens: {}\n  peak risk: {}",
        count,
        totals.outputs,
        totals.model_calls,
        totals.tool_calls,
        totals.tokens,
        rank_to_level_str(totals.peak_rank),
    );
}

fn risk_level_rank(level: TraceRiskLevel) -> u8 {
    match level {
        TraceRiskLevel::Unknown => 0,
        TraceRiskLevel::Low => 1,
        TraceRiskLevel::Medium => 2,
        TraceRiskLevel::High => 3,
        TraceRiskLevel::Blocked => 4,
    }
}

fn rank_to_level_str(rank: u8) -> &'static str {
    match rank {
        1 => "low",
        2 => "medium",
        3 => "high",
        4 => "blocked",
        _ => "unknown",
    }
}

fn refs_summary(refs: &[TraceRef]) -> String {
    refs.iter()
        .map(|r| format!("{}={}", r.ref_kind, r.reference))
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}
/// Dispatch entrypoint for the `forge-core preview|ready|explain` commands.
///
/// Short-circuits on `--help`, otherwise parses argv and routes to the
/// matching `run_m1_<kind>` body based on `kind`.
///
/// # Errors
///
/// Returns `ExitError::usage` when argument parsing fails, and propagates
/// the dispatcher's `ExitError::failed` when the underlying command reports
/// a `Blocked` or otherwise non-success status.
pub fn run_m1_command(args: &[String], kind: M1CommandKind) -> Result<(), ExitError> {
    // --help short-circuits before parsing so the parser can return a
    // fully-formed M1CommandInput on the success path.
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("{}", m1_usage(kind));
        return Ok(());
    }
    let (input, json) = parse_m1_command_args(args, kind)?;

    match kind {
        M1CommandKind::Preview => run_m1_preview(&input, json),
        M1CommandKind::Ready => run_m1_ready(&input, json),
        M1CommandKind::Explain => run_m1_explain(&input, json),
    }
}

/// Parses argv into a typed [`M1CommandInput`] plus a JSON flag for the
/// `preview|ready|explain` family.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or a value
/// helper reports a missing/malformed argument.
pub fn parse_m1_command_args(
    args: &[String],
    kind: M1CommandKind,
) -> Result<(M1CommandInput, bool), ExitError> {
    let mut root = PathBuf::from(".");
    let mut operation_path: Option<PathBuf> = None;
    let mut recorded_at = "unknown".to_string();
    let mut agent_id = "agent.codex.local".to_string();
    let mut principal_id = "principal.unknown".to_string();
    let mut json = false;
    let mut last_run = false;
    let mut run_id: Option<String> = None;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = next_m1_path_or_err(args, index, kind)?;
            }
            "--operation" => {
                index += 1;
                operation_path = Some(next_m1_path_or_err(args, index, kind)?);
            }
            "--recorded-at" => {
                index += 1;
                recorded_at = next_m1_arg_or_err(args, index, kind)?.to_string();
            }
            "--agent-id" => {
                index += 1;
                agent_id = next_m1_arg_or_err(args, index, kind)?.to_string();
            }
            "--principal-id" => {
                index += 1;
                principal_id = next_m1_arg_or_err(args, index, kind)?.to_string();
            }
            "--last-run" => last_run = true,
            "--run-id" => {
                index += 1;
                run_id = Some(next_m1_arg_or_err(args, index, kind)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                // Already handled by run_m1_command; if we somehow reach here,
                // treat as success.
                break;
            }
            _ => {
                return Err(ExitError::usage(m1_usage(kind)));
            }
        }
        index += 1;
    }

    if kind == M1CommandKind::Explain && !last_run && run_id.is_none() {
        return Err(m1_relation_usage_error(
            kind,
            "explain requires --last-run or --run-id <id>",
        ));
    }
    if last_run && run_id.is_some() {
        return Err(m1_relation_usage_error(
            kind,
            "explain accepts either --last-run or --run-id, not both",
        ));
    }

    Ok((
        M1CommandInput {
            kind,
            root,
            operation_path,
            recorded_at,
            agent_id,
            principal_id,
            run_id,
        },
        json,
    ))
}

#[must_use]
fn m1_usage(kind: M1CommandKind) -> String {
    command_surface_usage(kind.command_spec())
}

fn m1_relation_usage_error(kind: M1CommandKind, message: &str) -> ExitError {
    ExitError::usage(format!("{message}\n\n{}", m1_usage(kind)))
}

fn next_m1_arg_or_err(
    args: &[String],
    index: usize,
    kind: M1CommandKind,
) -> Result<&str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(m1_usage(kind)))
}

fn next_m1_path_or_err(
    args: &[String],
    index: usize,
    kind: M1CommandKind,
) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_m1_arg_or_err(args, index, kind)?))
}

/// Runs the `forge-core preview` command body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying preview returns an
/// error or its status is `Blocked`.
///
/// # Panics
///
/// Panics in JSON mode if the preview output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_m1_preview(input: &M1CommandInput, json: bool) -> Result<(), ExitError> {
    match run_preview(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("serialize preview output")
                );
            } else {
                println!(
                    "forge_core_preview status={:?} operation={} trace={}",
                    output.report.status, output.report.operation_id.0, output.trace_id
                );
            }
            if output.report.status == forge_core_kernel::RuntimePreviewStatus::Blocked {
                return Err(ExitError::failed("preview status blocked"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::failed(format!("preview failed: {error}"))),
    }
}

/// Runs the `forge-core ready` command body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying ready check returns an
/// error or reports `ready == false`.
///
/// # Panics
///
/// Panics in JSON mode if the ready output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_m1_ready(input: &M1CommandInput, json: bool) -> Result<(), ExitError> {
    match run_ready(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("serialize ready output")
                );
            } else {
                println!(
                    "forge_core_ready status={:?} operation={} trace={}",
                    output.report.status, output.report.operation_id.0, output.trace_id
                );
            }
            if !output.report.ready {
                return Err(ExitError::failed("ready report: not ready"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::failed(format!("ready failed: {error}"))),
    }
}

/// Runs the `forge-core explain` command body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying explain returns an
/// error or the trace query yields no events.
///
/// # Panics
///
/// Panics in JSON mode if the explain output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_m1_explain(input: &M1CommandInput, json: bool) -> Result<(), ExitError> {
    match run_explain(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("serialize explain output")
                );
            } else {
                println!("{}", output.explanation);
            }
            if output.query.events.is_empty() {
                return Err(ExitError::failed("explain query returned no events"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::failed(format!("explain failed: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_store::TraceEventQueryReason;
    use forge_core_trace::TraceEventKind;

    fn make_event(
        run_id: &str,
        trace_id: &str,
        recorded_at: &str,
        kind: TraceEventKind,
        message: &str,
    ) -> TraceEvent {
        let mut event = TraceEvent::new(
            trace_id,
            run_id,
            format!("{run_id}-{recorded_at}"),
            kind,
            recorded_at,
            message,
        );
        event = event
            .with_actor(TraceActor::new("principal.test", "agent.test", "executor"))
            .with_authority(TraceAuthority::for_operation("op.alpha"))
            .with_risk(TraceRisk::new(TraceRiskLevel::Low, false))
            .with_inputs(vec![TraceRef::new("target", "file://input.yaml")])
            .with_outputs(vec![TraceRef::new("effect", "file://effect.json")])
            .with_cost(TraceCost {
                model_calls: 1,
                tool_calls: 2,
                estimated_tokens: 100,
            });
        event
    }

    fn matched_result(events: Vec<TraceEvent>) -> TraceEventQueryResult {
        let returned = events.len();
        TraceEventQueryResult {
            status: TraceEventQueryStatus::Matched,
            scanned_events: events.len(),
            matched_events: events.len(),
            returned_events: returned,
            events,
            reasons: vec![TraceEventQueryReason::Matched],
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn m1_usage_projects_command_surface_lines() {
        for (kind, command) in [
            (M1CommandKind::Preview, &COMMAND_PREVIEW),
            (M1CommandKind::Ready, &COMMAND_READY),
            (M1CommandKind::Explain, &COMMAND_EXPLAIN),
        ] {
            let usage = m1_usage(kind);
            assert!(usage.starts_with("usage:\n"));
            for line in command.usage_lines {
                let projected = format!("  {}", line.trim_start());
                assert!(
                    usage.contains(&projected),
                    "M1 usage for {kind:?} should include projected Command Surface line {projected:?}: {usage}"
                );
            }
        }
    }

    #[test]
    fn narrative_is_chronological_when_input_is_out_of_order() {
        // Simulate NDJSON written out of order: later-recorded event appears first.
        let events = vec![
            make_event(
                "run.1",
                "trace.1",
                "2026-06-30T10:00:05Z",
                TraceEventKind::RunCompleted,
                "done",
            ),
            make_event(
                "run.1",
                "trace.1",
                "2026-06-30T10:00:00Z",
                TraceEventKind::RunStarted,
                "start",
            ),
            make_event(
                "run.1",
                "trace.1",
                "2026-06-30T10:00:02Z",
                TraceEventKind::EffectApplied,
                "applied",
            ),
        ];
        let query = matched_result(events);
        let narrative = explain_trace_query(&query);

        // The [1] index must be RunStarted (earliest recorded_at),
        // [2] EffectApplied, [3] RunCompleted — regardless of input order.
        let start_pos = narrative
            .find("[1] 2026-06-30T10:00:00Z RunStarted")
            .expect("first line is RunStarted");
        let mid_pos = narrative
            .find("[2] 2026-06-30T10:00:02Z EffectApplied")
            .expect("second line is EffectApplied");
        let end_pos = narrative
            .find("[3] 2026-06-30T10:00:05Z RunCompleted")
            .expect("third line is RunCompleted");
        assert!(start_pos < mid_pos);
        assert!(mid_pos < end_pos);
    }

    #[test]
    fn narrative_mentions_run_agent_and_totals() {
        let events = vec![
            make_event(
                "run.42",
                "trace.99",
                "2026-06-30T10:00:00Z",
                TraceEventKind::RunStarted,
                "kickoff",
            ),
            make_event(
                "run.42",
                "trace.99",
                "2026-06-30T10:00:01Z",
                TraceEventKind::EffectApplied,
                "applied",
            ),
        ];
        let query = matched_result(events);
        let narrative = explain_trace_query(&query);

        assert!(
            narrative.contains("Run run.42"),
            "narrative must reference the run_id"
        );
        assert!(
            narrative.contains("trace.99"),
            "narrative must reference the trace_id"
        );
        assert!(
            narrative.contains("agent.test"),
            "narrative must reference the agent_id"
        );
        assert!(
            narrative.contains("principal.test"),
            "narrative must reference the principal_id"
        );
        assert!(
            narrative.contains("events: 2"),
            "narrative must report total event count"
        );
        // 2 events × 1 output each = 2 outputs
        assert!(
            narrative.contains("outputs: 2"),
            "narrative must aggregate outputs"
        );
        // 2 events × (model=1, tool=2, tokens=100)
        assert!(narrative.contains("model_calls: 2"));
        assert!(narrative.contains("tool_calls: 4"));
        assert!(narrative.contains("estimated_tokens: 200"));
        assert!(narrative.contains("peak risk: low"));
    }

    #[test]
    fn narrative_reports_peak_risk_from_highest_event() {
        let mut high = make_event(
            "run.1",
            "trace.1",
            "2026-06-30T10:00:01Z",
            TraceEventKind::GateBlocked,
            "blocked",
        );
        high = high.with_risk(TraceRisk::new(TraceRiskLevel::High, true));
        let events = vec![
            make_event(
                "run.1",
                "trace.1",
                "2026-06-30T10:00:00Z",
                TraceEventKind::RunStarted,
                "start",
            ),
            high,
        ];
        let query = matched_result(events);
        let narrative = explain_trace_query(&query);
        assert!(narrative.contains("peak risk: high"));
    }

    #[test]
    fn empty_events_match_has_clear_message() {
        let query = TraceEventQueryResult {
            status: TraceEventQueryStatus::Matched,
            scanned_events: 0,
            matched_events: 0,
            returned_events: 0,
            events: Vec::new(),
            reasons: vec![TraceEventQueryReason::Matched],
            diagnostics: Vec::new(),
        };
        let narrative = explain_trace_query(&query);
        assert!(narrative.contains("returned no events"));
    }

    #[test]
    fn non_matched_query_reports_status_and_reasons() {
        let query = TraceEventQueryResult {
            status: TraceEventQueryStatus::Failed,
            scanned_events: 0,
            matched_events: 0,
            returned_events: 0,
            events: Vec::new(),
            reasons: vec![TraceEventQueryReason::NoTraceFile],
            diagnostics: vec!["trace log not found".to_string()],
        };
        let narrative = explain_trace_query(&query);
        assert!(narrative.contains("did not match"));
        assert!(narrative.contains("NoTraceFile"));
        assert!(narrative.contains("trace log not found"));
    }

    #[test]
    fn parser_accepts_run_id_as_explain_selector() {
        let args = vec![
            "explain".to_string(),
            "--run-id".to_string(),
            "run.abc".to_string(),
        ];
        let (input, _json) = parse_m1_command_args(&args, M1CommandKind::Explain)
            .expect("--run-id is a valid explain selector");
        assert_eq!(input.run_id.as_deref(), Some("run.abc"));
    }

    #[test]
    fn parser_accepts_last_run_as_explain_selector() {
        let args = vec!["explain".to_string(), "--last-run".to_string()];
        let (input, _json) = parse_m1_command_args(&args, M1CommandKind::Explain)
            .expect("--last-run remains a valid explain selector");
        assert!(input.run_id.is_none());
    }

    #[test]
    fn parser_accepts_explicit_no_json_mode() {
        let args = vec![
            "preview".to_string(),
            "--json".to_string(),
            "--no-json".to_string(),
        ];
        let (_input, json) =
            parse_m1_command_args(&args, M1CommandKind::Preview).expect("--no-json is valid");
        assert!(!json, "--no-json must override explicit --json");
    }

    #[test]
    fn parser_rejects_explain_without_selector() {
        let args = vec!["explain".to_string()];
        let result = parse_m1_command_args(&args, M1CommandKind::Explain);
        let error = result.expect_err("explain with no selector must error");
        assert_explain_relation_usage_error(&error, "explain requires --last-run or --run-id <id>");
    }

    #[test]
    fn parser_rejects_both_run_id_and_last_run() {
        let args = vec![
            "explain".to_string(),
            "--last-run".to_string(),
            "--run-id".to_string(),
            "run.abc".to_string(),
        ];
        let result = parse_m1_command_args(&args, M1CommandKind::Explain);
        let error = result.expect_err("--last-run and --run-id are mutually exclusive");
        assert_explain_relation_usage_error(
            &error,
            "explain accepts either --last-run or --run-id, not both",
        );
    }

    #[test]
    fn parser_rejects_unknown_explain_flag() {
        let args = vec!["explain".to_string(), "--frobnicate".to_string()];
        let result = parse_m1_command_args(&args, M1CommandKind::Explain);
        let error = result.expect_err("unknown explain flag must fail");
        assert!(
            error.message().contains("forge-core explain"),
            "unknown explain flags should report command-specific usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core preview"),
            "explain usage must not fall back to the global M1/global surface: {error}"
        );
    }

    fn assert_explain_relation_usage_error(error: &ExitError, diagnostic: &str) {
        assert_eq!(error.exit_code(), 2);
        assert!(
            error.message().contains(diagnostic),
            "explain relation error should preserve the specific diagnostic {diagnostic:?}: {error}"
        );
        let projected = COMMAND_EXPLAIN.canonical_usage().trim_start();
        assert!(
            error.message().contains(projected),
            "explain relation error should include projected Command Surface line {projected:?}: {error}"
        );
        assert!(
            !error.message().contains("forge-core preview"),
            "explain relation error must not fall back to sibling M1 command usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core ready"),
            "explain relation error must not fall back to sibling M1 command usage: {error}"
        );
    }
}
