use crate::claim::{conflict_code_str, load_claims};
use crate::cli_error::ExitError;
use crate::cli_util::graph_usage;
use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_contracts::{
    claim::ClaimContract,
    tool_effect::{AccessMode, EffectTargetKind},
    OperationContractDocument, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{check_write_against_claims, WriteCheck};
use forge_core_graph::{
    dry_run_graph_with_context, validate_graph, GraphClaimPreflightBlock,
    GraphClaimPreflightEvaluation, GraphClaimPreflightStatus, GraphDryRunContext,
    GraphOperationEvaluation, GraphOperationStatus, WorkflowGraph,
};
use forge_core_kernel::{
    preview_operation_with_snapshot, ready_operation_with_snapshot, RuntimePreviewReport,
    RuntimePreviewStatus, RuntimeReadyReport,
};
use forge_core_store::{
    build_reference_index, resolve_effect_physical_ref, ReferenceIndexBuildError,
};
use forge_core_validate::ReferenceIndex;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphCommandKind {
    Validate,
    RunDryRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCommandInput {
    pub root: PathBuf,
    pub graph_path: Option<PathBuf>,
    pub agent_id: Option<String>,
    pub claims_dir: Option<PathBuf>,
    pub now_unix: Option<i64>,
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
    pub claim_preflight_required: bool,
    pub claim_preflight_executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims_dir: Option<String>,
    pub validation_report: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphCommandError {
    MissingGraphPath,
    ProjectResolve(ProjectResolveError),
    GraphPathCanonicalize {
        path: PathBuf,
        source: String,
    },
    GraphPathOutsideProjectRoot {
        graph_path: PathBuf,
        resolved_graph_path: PathBuf,
        project_root: PathBuf,
    },
    StateRootUnavailable {
        path: PathBuf,
    },
    ReadGraph {
        path: PathBuf,
        source: String,
    },
    ParseGraph {
        path: PathBuf,
        source: String,
    },
    ReferenceIndexBuild(String),
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
            Self::GraphPathCanonicalize { path, source } => {
                write!(
                    formatter,
                    "canonicalize graph boundary path {} failed: {source}",
                    path.display()
                )
            }
            Self::GraphPathOutsideProjectRoot {
                graph_path,
                resolved_graph_path,
                project_root,
            } => write!(
                formatter,
                "graph file path {} resolves to {} which escapes project root {}",
                graph_path.display(),
                resolved_graph_path.display(),
                project_root.display()
            ),
            Self::StateRootUnavailable { path } => write!(
                formatter,
                "env_config: resolved Forge state_root {} is missing or is not a directory",
                path.display()
            ),
            Self::ReadGraph { path, source } => {
                write!(formatter, "read graph {} failed: {source}", path.display())
            }
            Self::ParseGraph { path, source } => {
                write!(formatter, "parse graph {} failed: {source}", path.display())
            }
            Self::ReferenceIndexBuild(message) => {
                write!(formatter, "reference index build failed: {message}")
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
    let resolved = resolve_project(&input.root)
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
    let resolved = resolve_project(&input.root)
        .map_err(GraphCommandError::ProjectResolve)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let graph_path = resolve_graph_path(&project_root, input.graph_path.as_deref())?;
    let state_root = PathBuf::from(&resolved.state_root);
    ensure_state_root_available(&state_root)?;
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
            claim_preflight_required: false,
            claim_preflight_executed: false,
            agent_id: input.agent_id.clone(),
            claims_dir: None,
            validation_report,
            report: None,
        });
    }

    let index =
        build_reference_index(&project_root).map_err(|error| reference_index_error(&error))?;
    let claims_dir = input
        .claims_dir
        .clone()
        .unwrap_or_else(|| state_root.join("claims-active"));
    let operation_evaluations = evaluate_graph_operations(
        &project_root,
        &claims_dir,
        &graph,
        &index,
        input.agent_id.as_deref(),
        input.now_unix,
    );
    let claim_preflight_required = operation_evaluations
        .iter()
        .any(|evaluation| evaluation.mutation_capable);
    let claim_preflight_executed = operation_evaluations
        .iter()
        .any(|evaluation| evaluation.claim_preflight.is_some());
    let dry_run_report = report_value(
        "dry-run",
        dry_run_graph_with_context(
            &graph,
            GraphDryRunContext::requiring_operation_contracts(&operation_evaluations),
        ),
    )?;
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
        claim_preflight_required,
        claim_preflight_executed,
        agent_id: input.agent_id.clone(),
        claims_dir: claim_preflight_required.then(|| display_path(&claims_dir)),
        validation_report,
        report: Some(dry_run_report),
    })
}

fn evaluate_graph_operations(
    project_root: &Path,
    claims_dir: &Path,
    graph: &WorkflowGraph,
    index: &ReferenceIndex,
    agent_id: Option<&str>,
    now_unix: Option<i64>,
) -> Vec<GraphOperationEvaluation> {
    let claim_context = GraphClaimPreflightCliContext::new(claims_dir, agent_id, now_unix);
    operation_refs(graph)
        .into_iter()
        .map(|operation_ref| {
            evaluate_graph_operation(project_root, index, operation_ref, &claim_context)
        })
        .collect()
}

fn operation_refs(graph: &WorkflowGraph) -> Vec<RepoPath> {
    let references: BTreeSet<String> = graph
        .nodes
        .iter()
        .filter(|node| node.node_kind == forge_core_graph::GraphNodeKind::Operation)
        .filter_map(|node| node.operation_ref.as_ref())
        .filter(|reference| !reference.0.trim().is_empty())
        .map(|reference| reference.0.clone())
        .collect();
    references.into_iter().map(RepoPath).collect()
}

fn evaluate_graph_operation(
    project_root: &Path,
    index: &ReferenceIndex,
    operation_ref: RepoPath,
    claim_context: &GraphClaimPreflightCliContext,
) -> GraphOperationEvaluation {
    let operation_path = match resolve_operation_path(project_root, &operation_ref) {
        Ok(path) => path,
        Err(error) => {
            return GraphOperationEvaluation {
                operation_ref,
                contract_id: None,
                mutation_capable: false,
                runtime_ready: false,
                plan_allowed: false,
                status: GraphOperationStatus::Invalid,
                preview_status: None,
                ready_status: None,
                blocking_reasons: vec![error.to_string()],
                claim_preflight: None,
                touched_refs: Vec::new(),
            };
        }
    };
    let operation = match read_operation(&operation_path) {
        Ok(operation) => operation,
        Err(error) => {
            return GraphOperationEvaluation {
                operation_ref,
                contract_id: None,
                mutation_capable: false,
                runtime_ready: false,
                plan_allowed: false,
                status: match error {
                    ReadGraphOperationError::Read { .. } => GraphOperationStatus::Missing,
                    ReadGraphOperationError::Parse { .. }
                    | ReadGraphOperationError::Canonicalize { .. }
                    | ReadGraphOperationError::UnsafeReference { .. }
                    | ReadGraphOperationError::UnsupportedClaimTarget { .. } => {
                        GraphOperationStatus::Invalid
                    }
                },
                preview_status: None,
                ready_status: None,
                blocking_reasons: vec![error.to_string()],
                claim_preflight: None,
                touched_refs: Vec::new(),
            };
        }
    };

    let preview = preview_operation_with_snapshot(
        &operation,
        forge_core_kernel::RuntimeReadSnapshot::new(index),
    );
    let ready = ready_operation_with_snapshot(
        &operation,
        forge_core_kernel::RuntimeReadSnapshot::new(index),
    );
    let plan_allowed = graph_operation_plan_allowed(&preview, &ready);
    let status = if plan_allowed {
        if ready.ready {
            GraphOperationStatus::Ready
        } else {
            GraphOperationStatus::SafeReadOnly
        }
    } else {
        GraphOperationStatus::NotReady
    };

    let mutation_capable = preview.operation_mutates_state;
    let claim_preflight =
        claim_preflight_for_operation(project_root, claim_context, &operation, mutation_capable);

    GraphOperationEvaluation {
        operation_ref,
        contract_id: Some(preview.operation_id.clone()),
        mutation_capable,
        runtime_ready: ready.ready,
        plan_allowed,
        status,
        preview_status: Some(serialized_value(&preview.status)),
        ready_status: Some(serialized_value(&ready.status)),
        blocking_reasons: if plan_allowed {
            Vec::new()
        } else {
            ready
                .blocking_reasons
                .iter()
                .map(serialized_value)
                .collect()
        },
        claim_preflight,
        touched_refs: preview.touched_refs.clone(),
    }
}

fn graph_operation_plan_allowed(
    preview: &RuntimePreviewReport,
    ready: &RuntimeReadyReport,
) -> bool {
    if preview.operation_mutates_state {
        return ready.ready;
    }
    matches!(
        preview.status,
        RuntimePreviewStatus::ReadOnly | RuntimePreviewStatus::Ready
    )
}

#[derive(Debug)]
struct GraphClaimPreflightCliContext {
    agent_id: Option<StableId>,
    claims: Vec<ClaimContract>,
    load_errors: Vec<String>,
    now_unix: i64,
}

impl GraphClaimPreflightCliContext {
    fn new(claims_dir: &Path, agent_id: Option<&str>, now_unix: Option<i64>) -> Self {
        let agent_id = agent_id.map(|value| StableId(value.to_string()));
        let (claims, load_errors) = if agent_id.is_some() {
            load_claims(claims_dir)
        } else {
            (Vec::new(), Vec::new())
        };
        Self {
            agent_id,
            claims,
            load_errors,
            now_unix: now_unix.unwrap_or_else(current_unix_seconds),
        }
    }
}

fn claim_preflight_for_operation(
    project_root: &Path,
    context: &GraphClaimPreflightCliContext,
    operation: &OperationContractDocument,
    mutation_capable: bool,
) -> Option<GraphClaimPreflightEvaluation> {
    if !mutation_capable {
        return None;
    }

    let targets = match claim_preflight_targets(project_root, operation) {
        Ok(targets) => targets,
        Err(error) => {
            return Some(blocked_claim_preflight(
                context.agent_id.clone(),
                Vec::new(),
                vec![error.to_string()],
            ));
        }
    };

    if context.agent_id.is_none() {
        return Some(blocked_claim_preflight(
            None,
            targets,
            vec![
                "graph run claim preflight requires --agent <id> for ready mutating operations"
                    .to_string(),
            ],
        ));
    }

    if targets.is_empty() {
        return Some(blocked_claim_preflight(
            context.agent_id.clone(),
            targets,
            vec!["ready mutating operation has no claim-preflight write targets".to_string()],
        ));
    }

    if !context.load_errors.is_empty() {
        return Some(blocked_claim_preflight(
            context.agent_id.clone(),
            targets,
            context.load_errors.clone(),
        ));
    }

    let agent_id = context.agent_id.as_ref().expect("agent checked above");
    match check_write_against_claims(&targets, agent_id, &context.claims, context.now_unix) {
        WriteCheck::Ok {
            governed_by_self,
            ungoverned,
        } if ungoverned.is_empty() => Some(GraphClaimPreflightEvaluation {
            status: GraphClaimPreflightStatus::Passed,
            agent_id: Some(agent_id.clone()),
            targets,
            governed_by_self,
            ungoverned,
            blocks: Vec::new(),
            reasons: Vec::new(),
        }),
        WriteCheck::Ok {
            governed_by_self,
            ungoverned,
        } => Some(GraphClaimPreflightEvaluation {
            status: GraphClaimPreflightStatus::Blocked,
            agent_id: Some(agent_id.clone()),
            targets,
            governed_by_self,
            reasons: ungoverned
                .iter()
                .map(|target| {
                    format!(
                        "target {} is not covered by the agent's live claim",
                        target.0
                    )
                })
                .collect(),
            ungoverned,
            blocks: Vec::new(),
        }),
        WriteCheck::Blocked { blocks } => Some(GraphClaimPreflightEvaluation {
            status: GraphClaimPreflightStatus::Blocked,
            agent_id: Some(agent_id.clone()),
            targets,
            governed_by_self: Vec::new(),
            ungoverned: Vec::new(),
            reasons: blocks
                .iter()
                .map(|block| {
                    format!(
                        "target {} is claimed by {} via {}",
                        block.blocked_path.0, block.claimant.0, block.blocking_claim_id.0
                    )
                })
                .collect(),
            blocks: blocks
                .into_iter()
                .map(|block| GraphClaimPreflightBlock {
                    blocked_path: block.blocked_path,
                    blocking_claim_id: block.blocking_claim_id.0,
                    claimant: block.claimant,
                    conflict_code: conflict_code_str(block.conflict_code).to_string(),
                })
                .collect(),
        }),
    }
}

fn blocked_claim_preflight(
    agent_id: Option<StableId>,
    targets: Vec<RepoPath>,
    reasons: Vec<String>,
) -> GraphClaimPreflightEvaluation {
    GraphClaimPreflightEvaluation {
        status: GraphClaimPreflightStatus::Blocked,
        agent_id,
        targets,
        governed_by_self: Vec::new(),
        ungoverned: Vec::new(),
        blocks: Vec::new(),
        reasons,
    }
}

fn claim_preflight_targets(
    project_root: &Path,
    operation: &OperationContractDocument,
) -> Result<Vec<RepoPath>, ReadGraphOperationError> {
    let mut targets = BTreeSet::new();
    for effect_ref in &operation.operation_contract.effect_contract_refs {
        let effect_path = resolve_repo_path_inside_project_root(project_root, effect_ref)?;
        let effect = read_tool_effect(&effect_path)?;
        for write in &effect.tool_effect_contract.write_set {
            if write.access_mode == AccessMode::Read {
                continue;
            }
            if write.target_kind == EffectTargetKind::Glob {
                return Err(ReadGraphOperationError::UnsupportedClaimTarget {
                    reference: write.reference.clone(),
                    reason: "glob write targets cannot be claim-preflighted without expansion"
                        .to_string(),
                });
            }
            match resolve_effect_physical_ref(project_root, write.target_kind, &write.reference) {
                Ok(physical_ref) => {
                    if !physical_ref.0.trim().is_empty() {
                        targets.insert(physical_ref.0);
                    }
                }
                Err(error) => {
                    return Err(ReadGraphOperationError::UnsupportedClaimTarget {
                        reference: write.reference.clone(),
                        reason: error.to_string(),
                    });
                }
            }
        }
    }
    if targets.is_empty() {
        for path in &operation.operation_contract.coordination_scope.target.paths {
            if !path.0.trim().is_empty() {
                targets.insert(path.0.clone());
            }
        }
    }
    Ok(targets.into_iter().map(RepoPath).collect())
}

fn read_tool_effect(path: &Path) -> Result<ToolEffectContractDocument, ReadGraphOperationError> {
    let text = fs::read_to_string(path).map_err(|source| ReadGraphOperationError::Read {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| ReadGraphOperationError::Parse {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn current_unix_seconds() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

fn read_operation(path: &Path) -> Result<OperationContractDocument, ReadGraphOperationError> {
    let text = fs::read_to_string(path).map_err(|source| ReadGraphOperationError::Read {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| ReadGraphOperationError::Parse {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn resolve_operation_path(
    project_root: &Path,
    operation_ref: &RepoPath,
) -> Result<PathBuf, ReadGraphOperationError> {
    resolve_repo_path_inside_project_root(project_root, operation_ref)
}

fn resolve_repo_path_inside_project_root(
    project_root: &Path,
    reference: &RepoPath,
) -> Result<PathBuf, ReadGraphOperationError> {
    let path = Path::new(reference.0.as_str());
    if path.as_os_str().is_empty() || path.components().any(operation_ref_component_escapes_root) {
        return Err(ReadGraphOperationError::UnsafeReference {
            reference: reference.0.clone(),
            reason: "repo ref must be a project-root-relative path without absolute or parent components".to_string(),
        });
    }
    let candidate = project_root.join(path);
    ensure_operation_path_inside_project_root(project_root, &candidate, reference)?;
    Ok(candidate)
}

fn operation_ref_component_escapes_root(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

fn ensure_operation_path_inside_project_root(
    project_root: &Path,
    candidate: &Path,
    operation_ref: &RepoPath,
) -> Result<(), ReadGraphOperationError> {
    if !candidate.exists() {
        return Ok(());
    }
    let canonical_root =
        fs::canonicalize(project_root).map_err(|source| ReadGraphOperationError::Canonicalize {
            path: project_root.to_path_buf(),
            source: source.to_string(),
        })?;
    let canonical_candidate =
        fs::canonicalize(candidate).map_err(|source| ReadGraphOperationError::Canonicalize {
            path: candidate.to_path_buf(),
            source: source.to_string(),
        })?;
    if canonical_candidate.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(ReadGraphOperationError::UnsafeReference {
            reference: operation_ref.0.clone(),
            reason: format!(
                "resolved operation path {} escapes project root {}",
                canonical_candidate.display(),
                canonical_root.display()
            ),
        })
    }
}

fn reference_index_error(error: &ReferenceIndexBuildError) -> GraphCommandError {
    GraphCommandError::ReferenceIndexBuild(error.to_string())
}

fn serialized_value<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(value)) => value,
        Ok(value) => value.to_string(),
        Err(error) => format!("serialization_failed:{error}"),
    }
}

fn read_graph(path: &Path) -> Result<WorkflowGraph, GraphCommandError> {
    let text = fs::read_to_string(path).map_err(|source| GraphCommandError::ReadGraph {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| GraphCommandError::ParseGraph {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn resolve_graph_path(
    project_root: &Path,
    graph_path: Option<&Path>,
) -> Result<PathBuf, GraphCommandError> {
    let graph_path = graph_path.ok_or(GraphCommandError::MissingGraphPath)?;
    let candidate = if graph_path.is_absolute() {
        graph_path.to_path_buf()
    } else {
        project_root.join(graph_path)
    };
    ensure_graph_path_inside_project_root(project_root, &candidate)?;
    Ok(candidate)
}

fn ensure_graph_path_inside_project_root(
    project_root: &Path,
    graph_path: &Path,
) -> Result<(), GraphCommandError> {
    let canonical_root = canonicalize_graph_boundary_path(project_root)?;
    let resolved_graph_path = resolve_graph_boundary_path(graph_path)?;
    if resolved_graph_path.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(GraphCommandError::GraphPathOutsideProjectRoot {
            graph_path: graph_path.to_path_buf(),
            resolved_graph_path,
            project_root: canonical_root,
        })
    }
}

fn resolve_graph_boundary_path(path: &Path) -> Result<PathBuf, GraphCommandError> {
    let normalized = normalize_path_lexically(path);
    if normalized.exists() {
        return canonicalize_graph_boundary_path(&normalized);
    }

    let mut existing = normalized.as_path();
    let mut missing_suffix: Vec<OsString> = Vec::new();
    while !existing.exists() {
        let Some(file_name) = existing.file_name() else {
            return canonicalize_graph_boundary_path(existing);
        };
        missing_suffix.push(file_name.to_os_string());
        let Some(parent) = existing.parent() else {
            return canonicalize_graph_boundary_path(existing);
        };
        existing = parent;
    }

    let mut resolved = canonicalize_graph_boundary_path(existing)?;
    for component in missing_suffix.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn canonicalize_graph_boundary_path(path: &Path) -> Result<PathBuf, GraphCommandError> {
    fs::canonicalize(path).map_err(|source| GraphCommandError::GraphPathCanonicalize {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
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

fn ensure_state_root_available(state_root: &Path) -> Result<(), GraphCommandError> {
    if state_root.is_dir() {
        Ok(())
    } else {
        Err(GraphCommandError::StateRootUnavailable {
            path: state_root.to_path_buf(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadGraphOperationError {
    Read { path: PathBuf, source: String },
    Parse { path: PathBuf, source: String },
    Canonicalize { path: PathBuf, source: String },
    UnsafeReference { reference: String, reason: String },
    UnsupportedClaimTarget { reference: String, reason: String },
}

impl fmt::Display for ReadGraphOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read operation {} failed: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    formatter,
                    "parse operation {} failed: {source}",
                    path.display()
                )
            }
            Self::Canonicalize { path, source } => {
                write!(
                    formatter,
                    "canonicalize path {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsafeReference { reference, reason } => {
                write!(formatter, "unsafe operation_ref {reference}: {reason}")
            }
            Self::UnsupportedClaimTarget { reference, reason } => {
                write!(
                    formatter,
                    "unsupported claim preflight target {reference}: {reason}"
                )
            }
        }
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
/// Dispatch entrypoint for the `forge-core graph` subcommand tree.
///
/// Routes to `validate` or `run` (dry-run only) based on `args[1]`, and
/// prints usage on `--help` / unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown, when `run`
/// is invoked without `--dry-run`, or when argument parsing fails.
pub fn run_graph_command(args: &[String]) -> Result<(), ExitError> {
    let subcommand = args.get(1).map_or("--help", String::as_str);
    match subcommand {
        "validate" => {
            if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
                println!("{}", graph_usage());
                return Ok(());
            }
            let (input, json, _dry_run) =
                parse_graph_command_args(args, GraphCommandKind::Validate)?;
            run_graph_validate(&input, json)
        }
        "run" => {
            if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
                println!("{}", graph_usage());
                return Ok(());
            }
            let (input, json, dry_run) =
                parse_graph_command_args(args, GraphCommandKind::RunDryRun)?;
            if !dry_run {
                return Err(ExitError::usage(graph_usage()));
            }
            run_graph_dry_run(&input, json)
        }
        "--help" | "-h" | "help" => {
            println!("{}", graph_usage());
            Ok(())
        }
        _ => Err(ExitError::usage(graph_usage())),
    }
}

/// Parses argv into a typed [`GraphCommandInput`] plus JSON / dry-run flags.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present, or when
/// `ArgvCursor::expect_value` reports a missing or malformed value, or when
/// `parse_graph_i64_or_err` reports a non-numeric `--now-unix`.
pub fn parse_graph_command_args(
    args: &[String],
    kind: GraphCommandKind,
) -> Result<(GraphCommandInput, bool, bool), ExitError> {
    use crate::cli_util::ArgvCursor;

    let mut root = PathBuf::from(".");
    let mut graph_path: Option<PathBuf> = None;
    let mut agent_id: Option<String> = None;
    let mut claims_dir: Option<PathBuf> = None;
    let mut now_unix: Option<i64> = None;
    let mut json = false;
    let mut dry_run = false;
    let mut cursor = ArgvCursor::new(args, 2, "graph");
    while let Some(flag) = cursor.peek_flag() {
        match flag {
            "--root" => root = PathBuf::from(cursor.expect_value("root")?),
            "--graph" => graph_path = Some(PathBuf::from(cursor.expect_value("graph")?)),
            "--agent" | "--agent-id" if kind == GraphCommandKind::RunDryRun => {
                agent_id = Some(cursor.expect_value("agent")?.to_string());
            }
            "--claims-dir" if kind == GraphCommandKind::RunDryRun => {
                claims_dir = Some(PathBuf::from(cursor.expect_value("claims-dir")?));
            }
            "--now-unix" if kind == GraphCommandKind::RunDryRun => {
                now_unix = Some(parse_graph_i64_or_err(cursor.expect_value("now-unix")?)?);
            }
            "--dry-run" if kind == GraphCommandKind::RunDryRun => {
                dry_run = true;
                cursor.advance();
            }
            "--json" => {
                json = true;
                cursor.advance();
            }
            "--help" | "-h" => break,
            _ => return Err(ExitError::usage(graph_usage())),
        }
    }

    Ok((
        GraphCommandInput {
            root,
            graph_path,
            agent_id,
            claims_dir,
            now_unix,
        },
        json,
        dry_run,
    ))
}

/// Parses a CLI string as an `i64`, scoped to graph commands.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `value` does not parse as `i64`.
pub fn parse_graph_i64_or_err(value: &str) -> Result<i64, ExitError> {
    value.parse::<i64>().map_err(|_| {
        ExitError::invalid_value(format!("graph: invalid value for --now-unix: '{value}'"))
    })
}

/// Runs the `forge-core graph validate` subcommand body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying validation returns an
/// error or when its status is `Blocked`.
///
/// # Panics
///
/// Panics in JSON mode if the validation output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_graph_validate(input: &GraphCommandInput, json: bool) -> Result<(), ExitError> {
    match run_validate(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output)
                        .expect("serialize graph validation output")
                );
            } else {
                println!(
                    "forge_core_graph_validate status={:?} graph={}",
                    output.status, output.graph_path
                );
            }
            if output.status == GraphCommandStatus::Blocked {
                return Err(ExitError::failed("graph validate status blocked"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::failed(format!("graph validate failed: {error}"))),
    }
}

/// Runs the `forge-core graph run --dry-run` subcommand body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying dry-run returns an
/// error or when its status is `Blocked`.
///
/// # Panics
///
/// Panics in JSON mode if the dry-run output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_graph_dry_run(input: &GraphCommandInput, json: bool) -> Result<(), ExitError> {
    match run_dry_run(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("serialize graph dry-run output")
                );
            } else {
                println!(
                    "forge_core_graph_run status={:?} graph={} dry_run_executed={}",
                    output.status, output.graph_path, output.dry_run_executed
                );
            }
            if output.status == GraphCommandStatus::Blocked {
                return Err(ExitError::failed("graph run status blocked"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::failed(format!("graph run failed: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_command_surface::COMMAND_GRAPH;

    #[test]
    fn graph_run_missing_dry_run_reports_graph_usage() {
        let error = run_graph_command(&args(&[
            "graph",
            "run",
            "--root",
            ".",
            "--graph",
            "graphs/workflow.yaml",
        ]))
        .expect_err("missing dry-run should fail before project resolution");

        assert_graph_run_usage_error(&error);
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn assert_graph_run_usage_error(error: &ExitError) {
        assert_eq!(error.exit_code(), 2);
        let graph_run_usage = COMMAND_GRAPH
            .usage_lines
            .iter()
            .find(|line| line.contains("graph run"))
            .expect("graph run usage line is present")
            .trim_start();
        assert!(
            error.message().contains(graph_run_usage),
            "graph run usage error should include projected Command Surface line {graph_run_usage:?}: {error}"
        );
        assert!(
            !error.message().contains("forge-core execute-operation"),
            "graph run usage error must not include unrelated mutating command usage: {error}"
        );
    }
}
