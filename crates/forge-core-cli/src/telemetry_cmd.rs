use crate::cli_util::{next_arg, telemetry_usage, usage};
use crate::project_cmd::{resolve_project, ProjectResolveError, ProjectResolvePayload};
use forge_core_contracts::telemetry::{
    PrivacyPolicy, TelemetryContract, TelemetryContractDocument, TelemetryEventKind,
    TelemetryEventSpec, TelemetrySink,
};
use forge_core_store::{
    query_trace_events, TraceEventQuery, TraceEventQueryReason, TraceEventQueryStatus,
};
use forge_core_trace::{TraceActor, TraceEvent, TraceEventKind, TraceRef};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_TELEMETRY_CONTRACT_PATH: &str = "contracts/examples/telemetry.yaml";
const TELEMETRY_EXPORT_RECORD_SCHEMA_VERSION: &str = "0.1";
const TELEMETRY_EXPORT_RECORD_KIND: &str = "telemetry_export_record";
const OTEL_JSON_SCHEMA_VERSION: &str = "forge_otel_json_v0";
const REDACTED: &str = "[redacted]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryExportFormat {
    Jsonl,
    OtelJson,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryExportCommandInput {
    pub root: PathBuf,
    pub contract_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub format: TelemetryExportFormat,
    pub trace_id: Option<String>,
    pub run_id: Option<String>,
    pub latest_run: bool,
    pub allow_bootstrap_core: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryExportStatus {
    Exported,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryExportReport {
    pub status: TelemetryExportStatus,
    pub project_id: String,
    pub project_root: String,
    pub sidecar_root: String,
    pub state_root: String,
    pub contract_path: String,
    pub input_event_count: usize,
    pub exported_event_count: usize,
    pub skipped_event_count: usize,
    pub output_path: Option<String>,
    pub format: TelemetryExportFormat,
    pub missing_field_count: usize,
    pub field_gaps: Vec<String>,
    pub missing_field_counts: BTreeMap<String, usize>,
    pub diagnostics: Vec<String>,
    pub records: Vec<TelemetryExportRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryExportRecord {
    pub schema_version: String,
    pub record_kind: String,
    pub project_id: String,
    pub trace_id: String,
    pub run_id: String,
    pub event_id: String,
    pub recorded_at: String,
    pub source_event_kind: TraceEventKind,
    pub telemetry_kind: TelemetryEventKind,
    pub actor: TelemetryExportActor,
    pub risk: forge_core_trace::TraceRisk,
    pub cost: forge_core_trace::TraceCost,
    pub inputs: Vec<TelemetryExportRef>,
    pub outputs: Vec<TelemetryExportRef>,
    pub message: String,
    pub operation_id: Option<String>,
    pub fields: BTreeMap<String, Value>,
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryExportActor {
    pub principal_id: String,
    pub agent_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryExportRef {
    pub ref_kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryExportError {
    ProjectResolve(ProjectResolveError),
    MissingSidecarStateRoot {
        project_root: PathBuf,
        state_root: PathBuf,
    },
    ReadContract {
        path: PathBuf,
        source: String,
    },
    ParseContract {
        path: PathBuf,
        source: String,
    },
    UnsupportedContractSchemaVersion {
        path: PathBuf,
        found: String,
    },
    InvalidContractPath {
        project_root: PathBuf,
        path: PathBuf,
    },
    InvalidOutputPath {
        project_root: PathBuf,
        path: PathBuf,
    },
    CanonicalizeProjectRoot {
        path: PathBuf,
        source: String,
    },
    TraceQueryFailed {
        state_root: PathBuf,
        reasons: Vec<TraceEventQueryReason>,
        diagnostics: Vec<String>,
    },
    CreateOutputParent {
        path: PathBuf,
        source: String,
    },
    CreateOutputTemp {
        path: PathBuf,
        source: String,
    },
    WriteOutput {
        path: PathBuf,
        source: String,
    },
    SyncOutput {
        path: PathBuf,
        source: String,
    },
    RemoveExistingOutput {
        path: PathBuf,
        source: String,
    },
    RenameOutput {
        temp_path: PathBuf,
        output_path: PathBuf,
        source: String,
    },
    SerializeRecord {
        source: String,
    },
}

impl fmt::Display for TelemetryExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::MissingSidecarStateRoot {
                project_root,
                state_root,
            } => write!(
                formatter,
                "env/config failure: Forge Project Link for project root '{}' resolves to missing sidecar state_root '{}'; restore the Forge Runtime Sidecar or run project init before telemetry export",
                project_root.display(),
                state_root.display()
            ),
            Self::ReadContract { path, source } => {
                write!(
                    formatter,
                    "read telemetry contract {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseContract { path, source } => {
                write!(
                    formatter,
                    "parse telemetry contract {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsupportedContractSchemaVersion { path, found } => write!(
                formatter,
                "telemetry contract {} has unsupported schema_version '{}', expected 0.1",
                path.display(),
                found
            ),
            Self::InvalidContractPath { project_root, path } => write!(
                formatter,
                "telemetry contract path '{}' is invalid; contract must resolve under project root '{}'",
                path.display(),
                project_root.display()
            ),
            Self::InvalidOutputPath { project_root, path } => write!(
                formatter,
                "telemetry output path '{}' is invalid; output must resolve under project root '{}'",
                path.display(),
                project_root.display()
            ),
            Self::CanonicalizeProjectRoot { path, source } => write!(
                formatter,
                "canonicalize project root {} failed: {source}",
                path.display()
            ),
            Self::TraceQueryFailed {
                state_root,
                reasons,
                diagnostics,
            } => write!(
                formatter,
                "query trace events from {} failed: reasons={reasons:?} diagnostics={diagnostics:?}",
                state_root.display()
            ),
            Self::CreateOutputParent { path, source } => write!(
                formatter,
                "create telemetry output parent {} failed: {source}",
                path.display()
            ),
            Self::CreateOutputTemp { path, source } => write!(
                formatter,
                "create telemetry temporary output {} failed: {source}",
                path.display()
            ),
            Self::WriteOutput { path, source } => {
                write!(
                    formatter,
                    "write telemetry output {} failed: {source}",
                    path.display()
                )
            }
            Self::SyncOutput { path, source } => {
                write!(
                    formatter,
                    "sync telemetry output {} failed: {source}",
                    path.display()
                )
            }
            Self::RemoveExistingOutput { path, source } => write!(
                formatter,
                "remove existing telemetry output {} failed before replace: {source}",
                path.display()
            ),
            Self::RenameOutput {
                temp_path,
                output_path,
                source,
            } => write!(
                formatter,
                "install telemetry output {} -> {} failed: {source}",
                temp_path.display(),
                output_path.display()
            ),
            Self::SerializeRecord { source } => {
                write!(formatter, "serialize telemetry export record failed: {source}")
            }
        }
    }
}

impl std::error::Error for TelemetryExportError {}

impl From<ProjectResolveError> for TelemetryExportError {
    fn from(error: ProjectResolveError) -> Self {
        Self::ProjectResolve(error)
    }
}

/// Exports Forge M1 trace events into a local telemetry interchange format.
///
/// # Errors
///
/// Returns an error when project resolution fails, the resolved sidecar is
/// missing outside the bootstrap-core exception, the telemetry contract cannot
/// be read or parsed, trace querying fails, or an output file cannot be written.
pub fn run_export(
    input: &TelemetryExportCommandInput,
) -> Result<TelemetryExportReport, TelemetryExportError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let state_root = existing_state_root_for_telemetry(&resolved)?;
    let raw_contract_path = resolve_input_path(
        &project_root,
        input
            .contract_path
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_TELEMETRY_CONTRACT_PATH)),
    );
    let contract_path =
        resolve_contract_path_under_project_root(&project_root, &raw_contract_path)?;
    let contract_document = read_contract(&contract_path)?;
    let contract = contract_document.telemetry_contract;
    let output_path = input
        .output_path
        .as_deref()
        .map(|path| resolve_output_path_under_project_root(&project_root, path))
        .transpose()?;

    if !contract.enabled || contract.sink == TelemetrySink::Disabled {
        let diagnostics = vec![format!(
            "telemetry export noop: contract enabled={} sink={:?}",
            contract.enabled, contract.sink
        )];
        let report = build_report(
            &TelemetryReportContext {
                resolved: &resolved,
                contract_path: &contract_path,
                output_path: output_path.as_deref(),
                format: input.format,
            },
            0,
            Vec::new(),
            diagnostics,
        );
        if let Some(path) = output_path.as_deref() {
            write_records_atomically(path, &report.records, input.format)?;
        }
        return Ok(report);
    }

    let query = TraceEventQuery {
        trace_id: input.trace_id.clone(),
        run_id: input.run_id.clone(),
        latest_run: input.latest_run,
        limit: None,
    };
    let query_result = query_trace_events(&state_root, &query);
    if query_result.status == TraceEventQueryStatus::Failed {
        return Err(TelemetryExportError::TraceQueryFailed {
            state_root,
            reasons: query_result.reasons,
            diagnostics: query_result.diagnostics,
        });
    }

    let input_event_count = query_result.events.len();
    let mut diagnostics = query_result.diagnostics;
    diagnostics.extend(query_result.reasons.iter().map(|reason| {
        format!(
            "trace query reason: {:?}; scanned={} matched={} returned={}",
            reason,
            query_result.scanned_events,
            query_result.matched_events,
            query_result.returned_events
        )
    }));
    let records = export_records_from_events(&resolved.project_id, &contract, &query_result.events);
    let report = build_report(
        &TelemetryReportContext {
            resolved: &resolved,
            contract_path: &contract_path,
            output_path: output_path.as_deref(),
            format: input.format,
        },
        input_event_count,
        records,
        diagnostics,
    );

    if let Some(path) = output_path.as_deref() {
        write_records_atomically(path, &report.records, input.format)?;
    }

    Ok(report)
}

struct TelemetryReportContext<'a> {
    resolved: &'a ProjectResolvePayload,
    contract_path: &'a Path,
    output_path: Option<&'a Path>,
    format: TelemetryExportFormat,
}

fn build_report(
    context: &TelemetryReportContext<'_>,
    input_event_count: usize,
    records: Vec<TelemetryExportRecord>,
    diagnostics: Vec<String>,
) -> TelemetryExportReport {
    let missing_field_counts = missing_field_counts(&records);
    let missing_field_count = records
        .iter()
        .map(|record| record.missing_fields.len())
        .sum::<usize>();
    let field_gaps = missing_field_counts
        .iter()
        .map(|(field, count)| format!("{field}:{count}"))
        .collect::<Vec<_>>();
    let exported_event_count = records.len();
    let skipped_event_count = input_event_count.saturating_sub(exported_event_count);
    let status = if exported_event_count == 0 {
        TelemetryExportStatus::Noop
    } else {
        TelemetryExportStatus::Exported
    };
    TelemetryExportReport {
        status,
        project_id: context.resolved.project_id.clone(),
        project_root: context.resolved.project_root.clone(),
        sidecar_root: context.resolved.sidecar_root.clone(),
        state_root: context.resolved.state_root.clone(),
        contract_path: display_path(context.contract_path),
        input_event_count,
        exported_event_count,
        skipped_event_count,
        output_path: context.output_path.map(display_path),
        format: context.format,
        missing_field_count,
        field_gaps,
        missing_field_counts,
        diagnostics,
        records,
    }
}

fn existing_state_root_for_telemetry(
    resolved: &ProjectResolvePayload,
) -> Result<PathBuf, TelemetryExportError> {
    let state_root = PathBuf::from(&resolved.state_root);
    if !resolved.state_exists && !resolved.bootstrap_core_exception {
        return Err(TelemetryExportError::MissingSidecarStateRoot {
            project_root: PathBuf::from(&resolved.project_root),
            state_root,
        });
    }
    Ok(state_root)
}

fn read_contract(path: &Path) -> Result<TelemetryContractDocument, TelemetryExportError> {
    let text = fs::read_to_string(path).map_err(|source| TelemetryExportError::ReadContract {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    let document: TelemetryContractDocument =
        serde_yaml::from_str(strip_utf8_bom(&text)).map_err(|source| {
            TelemetryExportError::ParseContract {
                path: path.to_path_buf(),
                source: source.to_string(),
            }
        })?;
    if document.schema_version != "0.1" {
        return Err(TelemetryExportError::UnsupportedContractSchemaVersion {
            path: path.to_path_buf(),
            found: document.schema_version,
        });
    }
    Ok(document)
}

fn resolve_contract_path_under_project_root(
    project_root: &Path,
    path: &Path,
) -> Result<PathBuf, TelemetryExportError> {
    let canonical_project_root = fs::canonicalize(project_root).map_err(|source| {
        TelemetryExportError::CanonicalizeProjectRoot {
            path: project_root.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let canonical_path =
        fs::canonicalize(path).map_err(|source| TelemetryExportError::ReadContract {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    if canonical_path.starts_with(&canonical_project_root) {
        Ok(canonical_path)
    } else {
        Err(TelemetryExportError::InvalidContractPath {
            project_root: canonical_project_root,
            path: canonical_path,
        })
    }
}

fn resolve_output_path_under_project_root(
    project_root: &Path,
    path: &Path,
) -> Result<PathBuf, TelemetryExportError> {
    if path.as_os_str().is_empty() || path.components().any(forbidden_output_component) {
        return Err(TelemetryExportError::InvalidOutputPath {
            project_root: project_root.to_path_buf(),
            path: path.to_path_buf(),
        });
    }
    let candidate = resolve_input_path(project_root, path);
    let canonical_project_root = fs::canonicalize(project_root).map_err(|source| {
        TelemetryExportError::CanonicalizeProjectRoot {
            path: project_root.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let parent = candidate.parent().unwrap_or(project_root);
    let canonical_parent = canonical_existing_ancestor(parent).map_err(|path| {
        TelemetryExportError::InvalidOutputPath {
            project_root: canonical_project_root.clone(),
            path,
        }
    })?;
    if canonical_parent.starts_with(&canonical_project_root) {
        Ok(candidate)
    } else {
        Err(TelemetryExportError::InvalidOutputPath {
            project_root: canonical_project_root,
            path: candidate,
        })
    }
}

fn canonical_existing_ancestor(path: &Path) -> Result<PathBuf, PathBuf> {
    let mut cursor = Some(path);
    while let Some(candidate) = cursor {
        if candidate.exists() {
            return fs::canonicalize(candidate).map_err(|_| candidate.to_path_buf());
        }
        cursor = candidate.parent();
    }
    Err(path.to_path_buf())
}

fn forbidden_output_component(component: std::path::Component<'_>) -> bool {
    matches!(component, std::path::Component::ParentDir)
}

fn export_records_from_events(
    resolved_project_id: &str,
    contract: &TelemetryContract,
    events: &[TraceEvent],
) -> Vec<TelemetryExportRecord> {
    events
        .iter()
        .filter_map(|event| export_record_from_event(resolved_project_id, contract, event))
        .collect()
}

fn export_record_from_event(
    resolved_project_id: &str,
    contract: &TelemetryContract,
    event: &TraceEvent,
) -> Option<TelemetryExportRecord> {
    let telemetry_kind = map_trace_event_kind(event.event_kind);
    if !contract.should_record(telemetry_kind) {
        return None;
    }
    let spec = event_spec_for_kind(contract, telemetry_kind);
    let (fields, missing_fields) = requested_fields(event, telemetry_kind, spec, &contract.privacy);
    Some(TelemetryExportRecord {
        schema_version: TELEMETRY_EXPORT_RECORD_SCHEMA_VERSION.to_string(),
        record_kind: TELEMETRY_EXPORT_RECORD_KIND.to_string(),
        project_id: sanitize_string(
            "project_id",
            event.project_id.as_deref().unwrap_or(resolved_project_id),
            &contract.privacy,
        ),
        trace_id: sanitize_string("trace_id", &event.trace_id, &contract.privacy),
        run_id: sanitize_string("run_id", &event.run_id, &contract.privacy),
        event_id: sanitize_string("event_id", &event.event_id, &contract.privacy),
        recorded_at: sanitize_string("recorded_at", &event.recorded_at, &contract.privacy),
        source_event_kind: event.event_kind,
        telemetry_kind,
        actor: export_actor(&event.actor, &contract.privacy),
        risk: event.risk.clone(),
        cost: event.cost.clone(),
        inputs: export_refs(&event.inputs, &contract.privacy),
        outputs: export_refs(&event.outputs, &contract.privacy),
        message: sanitize_string("message", &event.message, &contract.privacy),
        operation_id: event
            .authority
            .operation_id
            .as_deref()
            .map(|value| sanitize_string("operation_id", value, &contract.privacy)),
        fields,
        missing_fields,
    })
}

fn map_trace_event_kind(kind: TraceEventKind) -> TelemetryEventKind {
    match kind {
        TraceEventKind::GatePassed | TraceEventKind::GateBlocked => {
            TelemetryEventKind::GateEvaluated
        }
        TraceEventKind::PreviewCompleted | TraceEventKind::ReadyCompleted => {
            TelemetryEventKind::VerificationRun
        }
        TraceEventKind::EffectStaged | TraceEventKind::EffectApplied => {
            TelemetryEventKind::ToolCall
        }
        TraceEventKind::RunStarted
        | TraceEventKind::OperationPlanned
        | TraceEventKind::RunCompleted
        | TraceEventKind::RunFailed => TelemetryEventKind::PhaseTransition,
    }
}

fn event_spec_for_kind(
    contract: &TelemetryContract,
    kind: TelemetryEventKind,
) -> Option<&TelemetryEventSpec> {
    contract.events.iter().find(|spec| spec.kind == kind)
}

fn requested_fields(
    event: &TraceEvent,
    telemetry_kind: TelemetryEventKind,
    spec: Option<&TelemetryEventSpec>,
    privacy: &PrivacyPolicy,
) -> (BTreeMap<String, Value>, Vec<String>) {
    let Some(spec) = spec else {
        return (BTreeMap::new(), Vec::new());
    };
    let mut fields = BTreeMap::new();
    let mut missing_fields = Vec::new();
    for field in &spec.fields {
        if let Some(value) = trace_field_value(event, telemetry_kind, field, privacy) {
            fields.insert(field.clone(), value);
        } else {
            missing_fields.push(field.clone());
        }
    }
    (fields, missing_fields)
}

fn trace_field_value(
    event: &TraceEvent,
    telemetry_kind: TelemetryEventKind,
    field: &str,
    privacy: &PrivacyPolicy,
) -> Option<Value> {
    match field {
        "project_id" => event
            .project_id
            .as_deref()
            .map(|value| json!(sanitize_string(field, value, privacy))),
        "trace_id" => Some(json!(sanitize_string(field, &event.trace_id, privacy))),
        "run_id" => Some(json!(sanitize_string(field, &event.run_id, privacy))),
        "event_id" => Some(json!(sanitize_string(field, &event.event_id, privacy))),
        "recorded_at" => Some(json!(sanitize_string(field, &event.recorded_at, privacy))),
        "source_event_kind" => Some(json!(trace_event_kind_name(event.event_kind))),
        "telemetry_kind" => Some(json!(telemetry_event_kind_name(telemetry_kind))),
        "message" | "reason" => Some(json!(sanitize_string(field, &event.message, privacy))),
        "operation_id" | "decision_id" => event
            .authority
            .operation_id
            .as_deref()
            .map(|value| json!(sanitize_string(field, value, privacy))),
        "principal_id" => Some(json!(sanitize_actor_id(&event.actor.principal_id, privacy))),
        "agent_id" => Some(json!(sanitize_actor_id(&event.actor.agent_id, privacy))),
        "actor_role" => Some(json!(sanitize_string(field, &event.actor.role, privacy))),
        "risk_level" => Some(json!(risk_level_name(event.risk.risk_level))),
        "destructive" => Some(json!(event.risk.destructive)),
        "model_calls" => Some(json!(event.cost.model_calls)),
        "tool_calls" => Some(json!(event.cost.tool_calls)),
        "estimated_tokens" => Some(json!(event.cost.estimated_tokens)),
        "input_count" => Some(json!(event.inputs.len())),
        "output_count" => Some(json!(event.outputs.len())),
        "inputs" => Some(json!(export_refs(&event.inputs, privacy))),
        "outputs" => Some(json!(export_refs(&event.outputs, privacy))),
        "to_phase" => Some(json!(phase_to(event.event_kind))),
        "gate_status" => gate_status(event.event_kind).map(|status| json!(status)),
        "result" => event_result(event.event_kind).map(|result| json!(result)),
        "command" => command_ref(event, privacy).map(|command| json!(command)),
        _ => None,
    }
}

fn export_actor(actor: &TraceActor, privacy: &PrivacyPolicy) -> TelemetryExportActor {
    TelemetryExportActor {
        principal_id: sanitize_actor_id(&actor.principal_id, privacy),
        agent_id: sanitize_actor_id(&actor.agent_id, privacy),
        role: sanitize_string("actor_role", &actor.role, privacy),
    }
}

fn export_refs(references: &[TraceRef], privacy: &PrivacyPolicy) -> Vec<TelemetryExportRef> {
    references
        .iter()
        .map(|reference| TelemetryExportRef {
            ref_kind: sanitize_string("ref_kind", &reference.ref_kind, privacy),
            reference: sanitize_reference(&reference.reference, privacy),
        })
        .collect()
}

fn sanitize_actor_id(value: &str, privacy: &PrivacyPolicy) -> String {
    if privacy.hash_agent_ids {
        stable_sha256_reference(value)
    } else {
        sanitize_string("actor_id", value, privacy)
    }
}

fn sanitize_reference(value: &str, privacy: &PrivacyPolicy) -> String {
    if privacy.redact_paths {
        stable_sha256_reference(value)
    } else {
        sanitize_string("ref", value, privacy)
    }
}

fn sanitize_string(field_name: &str, value: &str, privacy: &PrivacyPolicy) -> String {
    if privacy.redact_secrets && should_redact_string(field_name, value, privacy) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

fn should_redact_string(field_name: &str, value: &str, privacy: &PrivacyPolicy) -> bool {
    field_name_is_secretish(field_name)
        || privacy
            .denylist_field_globs
            .iter()
            .any(|pattern| glob_matches(pattern, field_name))
        || text_is_secretish(value)
}

fn field_name_is_secretish(field_name: &str) -> bool {
    let normalized = normalize_secret_probe(field_name);
    normalized == "key"
        || normalized.ends_with("_key")
        || normalized
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix == "key")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("password")
        || normalized.contains("passwd")
        || normalized.contains("credential")
        || normalized.contains("private_key")
        || normalized.contains("api_key")
}

fn text_is_secretish(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let normalized = normalize_secret_probe(value);
    lower.contains("password=")
        || lower.contains("password:")
        || lower.contains("passwd=")
        || lower.contains("passwd:")
        || lower.contains("token=")
        || lower.contains("token:")
        || lower.contains("secret=")
        || lower.contains("secret:")
        || lower.contains("api_key=")
        || lower.contains("api_key:")
        || lower.contains("apikey=")
        || lower.contains("apikey:")
        || normalized.contains("private_key")
        || normalized.contains("bearer ")
        || normalized.starts_with("sk-")
}

fn normalize_secret_probe(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else if character == '-' || character == '_' || character == '.' {
                character
            } else {
                ' '
            }
        })
        .collect()
}

fn glob_matches(pattern: &str, field_name: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let field_name = field_name.to_ascii_lowercase();
    if pattern == "*" {
        return true;
    }
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == field_name;
    };
    field_name.starts_with(prefix) && field_name.ends_with(suffix)
}

fn command_ref(event: &TraceEvent, privacy: &PrivacyPolicy) -> Option<String> {
    event
        .inputs
        .iter()
        .chain(event.outputs.iter())
        .find(|reference| reference.ref_kind == "command")
        .map(|reference| sanitize_reference(&reference.reference, privacy))
}

fn phase_to(kind: TraceEventKind) -> &'static str {
    match kind {
        TraceEventKind::RunStarted => "running",
        TraceEventKind::OperationPlanned => "operation_planned",
        TraceEventKind::PreviewCompleted => "preview_completed",
        TraceEventKind::ReadyCompleted => "ready_completed",
        TraceEventKind::GatePassed | TraceEventKind::GateBlocked => "gate_evaluated",
        TraceEventKind::EffectStaged => "effect_staged",
        TraceEventKind::EffectApplied => "effect_applied",
        TraceEventKind::RunCompleted => "completed",
        TraceEventKind::RunFailed => "failed",
    }
}

fn gate_status(kind: TraceEventKind) -> Option<&'static str> {
    match kind {
        TraceEventKind::GatePassed => Some("passed"),
        TraceEventKind::GateBlocked => Some("blocked"),
        _ => None,
    }
}

fn event_result(kind: TraceEventKind) -> Option<&'static str> {
    match kind {
        TraceEventKind::GatePassed | TraceEventKind::RunCompleted => Some("passed"),
        TraceEventKind::GateBlocked => Some("blocked"),
        TraceEventKind::RunFailed => Some("failed"),
        TraceEventKind::PreviewCompleted | TraceEventKind::ReadyCompleted => Some("completed"),
        _ => None,
    }
}

fn missing_field_counts(records: &[TelemetryExportRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        for field in &record.missing_fields {
            let count = counts.entry(field.clone()).or_insert(0);
            *count += 1;
        }
    }
    counts
}

fn write_records_atomically(
    path: &Path,
    records: &[TelemetryExportRecord],
    format: TelemetryExportFormat,
) -> Result<(), TelemetryExportError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| {
                TelemetryExportError::CreateOutputParent {
                    path: parent.to_path_buf(),
                    source: source.to_string(),
                }
            })?;
        }
    }

    let payload = serialize_records(records, format)?;
    let temp_path = temp_output_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|source| TelemetryExportError::CreateOutputTemp {
            path: temp_path.clone(),
            source: source.to_string(),
        })?;
    file.write_all(payload.as_bytes())
        .map_err(|source| TelemetryExportError::WriteOutput {
            path: temp_path.clone(),
            source: source.to_string(),
        })?;
    file.sync_all()
        .map_err(|source| TelemetryExportError::SyncOutput {
            path: temp_path.clone(),
            source: source.to_string(),
        })?;
    drop(file);

    if path.exists() {
        fs::remove_file(path).map_err(|source| TelemetryExportError::RemoveExistingOutput {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    }
    fs::rename(&temp_path, path).map_err(|source| TelemetryExportError::RenameOutput {
        temp_path,
        output_path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn serialize_records(
    records: &[TelemetryExportRecord],
    format: TelemetryExportFormat,
) -> Result<String, TelemetryExportError> {
    let mut output = String::new();
    for record in records {
        let line = match format {
            TelemetryExportFormat::Jsonl => serde_json::to_string(record),
            TelemetryExportFormat::OtelJson => serde_json::to_string(&otel_json_line(record)),
        }
        .map_err(|source| TelemetryExportError::SerializeRecord {
            source: source.to_string(),
        })?;
        output.push_str(&line);
        output.push('\n');
    }
    Ok(output)
}

fn otel_json_line(record: &TelemetryExportRecord) -> Value {
    json!({
        "schema_version": OTEL_JSON_SCHEMA_VERSION,
        "resource": {
            "service.name": "forge-core",
            "forge.project_id": record.project_id,
        },
        "scope": {
            "name": "forge-core.telemetry-export",
            "version": TELEMETRY_EXPORT_RECORD_SCHEMA_VERSION,
        },
        "span": {
            "trace_id": stable_sha256_hex_prefix(&record.trace_id, 32),
            "span_id": stable_sha256_hex_prefix(&record.event_id, 16),
            "name": format!("forge.{}", telemetry_event_kind_name(record.telemetry_kind)),
            "kind": "internal",
            "start_time": record.recorded_at,
            "end_time": record.recorded_at,
            "attributes": {
                "forge.project_id": record.project_id,
                "forge.trace_id": record.trace_id,
                "forge.run_id": record.run_id,
                "forge.event_id": record.event_id,
                "forge.source_event_kind": trace_event_kind_name(record.source_event_kind),
                "forge.telemetry_kind": telemetry_event_kind_name(record.telemetry_kind),
                "forge.operation_id": record.operation_id,
                "forge.actor.principal_id": record.actor.principal_id,
                "forge.actor.agent_id": record.actor.agent_id,
                "forge.actor.role": record.actor.role,
                "forge.risk.level": risk_level_name(record.risk.risk_level),
                "forge.risk.destructive": record.risk.destructive,
                "forge.cost.model_calls": record.cost.model_calls,
                "forge.cost.tool_calls": record.cost.tool_calls,
                "forge.cost.estimated_tokens": record.cost.estimated_tokens,
                "forge.missing_fields": record.missing_fields,
                "forge.fields": record.fields,
            },
            "events": [
                {
                    "name": trace_event_kind_name(record.source_event_kind),
                    "time": record.recorded_at,
                    "attributes": {
                        "message": record.message,
                        "inputs": record.inputs,
                        "outputs": record.outputs,
                    }
                }
            ]
        }
    })
}

fn temp_output_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().map_or_else(
        || "telemetry-export.ndjson".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(".{file_name}.tmp-{}-{suffix}", std::process::id()))
}

fn resolve_input_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\")
        .map_or(raw.clone(), std::string::ToString::to_string)
}

fn strip_utf8_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn stable_sha256_reference(value: &str) -> String {
    format!("sha256:{}", stable_sha256_hex(value))
}

fn stable_sha256_hex_prefix(value: &str, chars: usize) -> String {
    let digest = stable_sha256_hex(value);
    digest.chars().take(chars).collect()
}

fn stable_sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(nibble_to_hex(byte >> 4));
        hex.push(nibble_to_hex(byte & 0x0f));
    }
    hex
}

fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        10..=15 => char::from(b'a' + (nibble - 10)),
        _ => '0',
    }
}

fn trace_event_kind_name(kind: TraceEventKind) -> &'static str {
    match kind {
        TraceEventKind::RunStarted => "run_started",
        TraceEventKind::OperationPlanned => "operation_planned",
        TraceEventKind::PreviewCompleted => "preview_completed",
        TraceEventKind::ReadyCompleted => "ready_completed",
        TraceEventKind::GatePassed => "gate_passed",
        TraceEventKind::GateBlocked => "gate_blocked",
        TraceEventKind::EffectStaged => "effect_staged",
        TraceEventKind::EffectApplied => "effect_applied",
        TraceEventKind::RunCompleted => "run_completed",
        TraceEventKind::RunFailed => "run_failed",
    }
}

fn telemetry_event_kind_name(kind: TelemetryEventKind) -> &'static str {
    match kind {
        TelemetryEventKind::PhaseTransition => "phase_transition",
        TelemetryEventKind::ClaimAcquired => "claim_acquired",
        TelemetryEventKind::ClaimReleased => "claim_released",
        TelemetryEventKind::GateEvaluated => "gate_evaluated",
        TelemetryEventKind::ToolCall => "tool_call",
        TelemetryEventKind::ModelInvocation => "model_invocation",
        TelemetryEventKind::VerificationRun => "verification_run",
        TelemetryEventKind::ConflictDetected => "conflict_detected",
        TelemetryEventKind::HumanHandoff => "human_handoff",
    }
}

fn risk_level_name(kind: forge_core_trace::TraceRiskLevel) -> &'static str {
    match kind {
        forge_core_trace::TraceRiskLevel::Unknown => "unknown",
        forge_core_trace::TraceRiskLevel::Low => "low",
        forge_core_trace::TraceRiskLevel::Medium => "medium",
        forge_core_trace::TraceRiskLevel::High => "high",
        forge_core_trace::TraceRiskLevel::Blocked => "blocked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::telemetry::{CorrelationPolicy, SamplingPolicy, TelemetryEventSpec};
    use forge_core_contracts::StableId;
    use forge_core_trace::{
        TraceActor, TraceAuthority, TraceCost, TraceRef, TraceRisk, TraceRiskLevel,
    };

    fn sample_contract(enabled: bool) -> TelemetryContract {
        TelemetryContract {
            id: StableId("telemetry.test".to_string()),
            enabled,
            sink: TelemetrySink::JsonlFile,
            events: vec![
                TelemetryEventSpec {
                    kind: TelemetryEventKind::PhaseTransition,
                    record: true,
                    fields: vec![
                        "from_phase".to_string(),
                        "to_phase".to_string(),
                        "duration_ms".to_string(),
                    ],
                },
                TelemetryEventSpec {
                    kind: TelemetryEventKind::ToolCall,
                    record: true,
                    fields: vec![
                        "tool".to_string(),
                        "command".to_string(),
                        "exit_code".to_string(),
                    ],
                },
                TelemetryEventSpec {
                    kind: TelemetryEventKind::GateEvaluated,
                    record: true,
                    fields: vec!["gate_status".to_string(), "duration_ms".to_string()],
                },
                TelemetryEventSpec {
                    kind: TelemetryEventKind::VerificationRun,
                    record: true,
                    fields: vec!["result".to_string(), "test_count".to_string()],
                },
            ],
            sampling: SamplingPolicy {
                rate: 10_000,
                max_per_second: Some(100),
                always_record_kinds: Vec::new(),
            },
            privacy: PrivacyPolicy {
                redact_secrets: true,
                redact_paths: false,
                hash_agent_ids: true,
                denylist_field_globs: vec!["env.*".to_string()],
            },
            correlation: CorrelationPolicy {
                trace_parent: None,
                run_id_ref: None,
                span_id_seed: None,
            },
        }
    }

    fn sample_event(kind: TraceEventKind) -> TraceEvent {
        TraceEvent::new(
            "trace.one",
            "run.one",
            "evt.one",
            kind,
            "2026-06-29T00:00:00Z",
            "completed without password=plain-text",
        )
        .with_project_id("project.one")
        .with_actor(TraceActor::new("principal.daniel", "agent.codex", "driver"))
        .with_authority(TraceAuthority::for_operation("op.one"))
        .with_cost(TraceCost {
            model_calls: 1,
            tool_calls: 2,
            estimated_tokens: 3,
        })
        .with_risk(TraceRisk::new(TraceRiskLevel::Low, false))
        .with_inputs(vec![TraceRef::new("operation", "contracts/op.yaml")])
        .with_outputs(vec![
            TraceRef::new("command", "cmd.check"),
            TraceRef::new("effect", "src/main.rs"),
        ])
    }

    #[test]
    fn maps_trace_events_to_contract_telemetry_kinds() {
        assert_eq!(
            map_trace_event_kind(TraceEventKind::GatePassed),
            TelemetryEventKind::GateEvaluated
        );
        assert_eq!(
            map_trace_event_kind(TraceEventKind::PreviewCompleted),
            TelemetryEventKind::VerificationRun
        );
        assert_eq!(
            map_trace_event_kind(TraceEventKind::EffectApplied),
            TelemetryEventKind::ToolCall
        );
        assert_eq!(
            map_trace_event_kind(TraceEventKind::RunStarted),
            TelemetryEventKind::PhaseTransition
        );
    }

    #[test]
    fn privacy_hashes_actors_redacts_paths_and_secret_text() {
        let mut contract = sample_contract(true);
        contract.privacy.redact_paths = true;
        let event = sample_event(TraceEventKind::EffectApplied);

        let records = export_records_from_events("project.one", &contract, &[event]);

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert!(record.actor.agent_id.starts_with("sha256:"));
        assert!(record.inputs[0].reference.starts_with("sha256:"));
        assert_eq!(record.message, REDACTED);
        assert_eq!(
            record.fields.get("command"),
            Some(&json!(stable_sha256_reference("cmd.check")))
        );
    }

    #[test]
    fn disabled_contract_exports_no_records() {
        let contract = sample_contract(false);
        let event = sample_event(TraceEventKind::GatePassed);

        let records = export_records_from_events("project.one", &contract, &[event]);

        assert!(records.is_empty());
    }

    #[test]
    fn records_missing_fields_requested_by_contract_but_absent_from_trace() {
        let contract = sample_contract(true);
        let event = sample_event(TraceEventKind::GatePassed);

        let records = export_records_from_events("project.one", &contract, &[event]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].fields.get("gate_status"), Some(&json!("passed")));
        assert_eq!(records[0].missing_fields, vec!["duration_ms"]);
        let counts = missing_field_counts(&records);
        assert_eq!(counts.get("duration_ms"), Some(&1));
    }

    #[test]
    fn jsonl_serialization_emits_one_json_object_per_line() {
        let contract = sample_contract(true);
        let event = sample_event(TraceEventKind::ReadyCompleted);
        let records = export_records_from_events("project.one", &contract, &[event]);

        let jsonl = serialize_records(&records, TelemetryExportFormat::Jsonl).unwrap();

        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 1);
        let value: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(value["record_kind"], TELEMETRY_EXPORT_RECORD_KIND);
        assert_eq!(value["telemetry_kind"], "verification_run");
    }

    #[test]
    fn otel_json_serialization_is_local_span_shape() {
        let contract = sample_contract(true);
        let event = sample_event(TraceEventKind::ReadyCompleted);
        let records = export_records_from_events("project.one", &contract, &[event]);

        let jsonl = serialize_records(&records, TelemetryExportFormat::OtelJson).unwrap();
        let line: Value = serde_json::from_str(jsonl.trim()).unwrap();

        assert_eq!(line["schema_version"], OTEL_JSON_SCHEMA_VERSION);
        assert_eq!(line["span"]["kind"], "internal");
        assert_eq!(
            line["span"]["attributes"]["forge.telemetry_kind"],
            "verification_run"
        );
    }
}
pub fn run_telemetry_command(args: &[String]) {
    let subcommand = args.get(1).map_or("--help", String::as_str);
    match subcommand {
        "export" => {
            let (input, json) = parse_telemetry_export_args(args);
            run_telemetry_export(&input, json);
        }
        "--help" | "-h" | "help" => {
            println!("{}", telemetry_usage());
        }
        _ => {
            eprintln!("{}", telemetry_usage());
            std::process::exit(2);
        }
    }
}

pub fn parse_telemetry_export_args(args: &[String]) -> (TelemetryExportCommandInput, bool) {
    let mut root = PathBuf::from(".");
    let mut contract_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut format = TelemetryExportFormat::Jsonl;
    let mut trace_id: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut latest_run = false;
    let mut allow_bootstrap_core = false;
    let mut json = false;
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(next_telemetry_value(args, index, "root"));
            }
            "--contract" => {
                index += 1;
                contract_path = Some(PathBuf::from(next_telemetry_value(args, index, "contract")));
            }
            "--output" => {
                index += 1;
                output_path = Some(PathBuf::from(next_telemetry_value(args, index, "output")));
            }
            "--format" => {
                index += 1;
                format = parse_telemetry_format(next_telemetry_value(args, index, "format"));
            }
            "--trace-id" => {
                index += 1;
                trace_id = Some(next_telemetry_value(args, index, "trace-id").to_string());
            }
            "--run-id" => {
                index += 1;
                run_id = Some(next_telemetry_value(args, index, "run-id").to_string());
            }
            "--latest-run" => latest_run = true,
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", telemetry_usage());
                std::process::exit(0);
            }
            _ => {
                eprintln!("{}", telemetry_usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let selected_filters =
        usize::from(trace_id.is_some()) + usize::from(run_id.is_some()) + usize::from(latest_run);
    if selected_filters > 1 {
        eprintln!("telemetry export accepts only one of --trace-id, --run-id, or --latest-run");
        std::process::exit(3);
    }

    (
        TelemetryExportCommandInput {
            root,
            contract_path,
            output_path,
            format,
            trace_id,
            run_id,
            latest_run,
            allow_bootstrap_core,
        },
        json,
    )
}

pub fn next_telemetry_value<'a>(args: &'a [String], index: usize, flag: &str) -> &'a str {
    let value = args.get(index).map_or_else(
        || {
            eprintln!("telemetry export: missing value for --{flag}");
            std::process::exit(3);
        },
        String::as_str,
    );
    if value.starts_with('-') {
        eprintln!("telemetry export: missing value for --{flag}");
        std::process::exit(3);
    }
    value
}

pub fn parse_telemetry_format(value: &str) -> TelemetryExportFormat {
    match value {
        "jsonl" | "forge-jsonl" => TelemetryExportFormat::Jsonl,
        "otel-json" | "otel-jsonl" | "opentelemetry-json" => TelemetryExportFormat::OtelJson,
        other => {
            eprintln!("telemetry export: invalid value for --format '{other}'; expected jsonl or otel-json");
            std::process::exit(3);
        }
    }
}

pub fn run_telemetry_export(input: &TelemetryExportCommandInput, json: bool) {
    match run_export(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output)
                        .expect("serialize telemetry export output")
                );
            } else {
                println!(
                    "forge_core_telemetry_export status={:?} format={:?} exported={} skipped={} output={}",
                    output.status,
                    output.format,
                    output.exported_event_count,
                    output.skipped_event_count,
                    output.output_path.as_deref().unwrap_or("<memory>")
                );
                for diagnostic in &output.diagnostics {
                    println!("diagnostic={diagnostic}");
                }
            }
        }
        Err(error) => {
            eprintln!("telemetry export failed: {error}");
            std::process::exit(5);
        }
    }
}
