use forge_core_contracts::{RepoPath, StableId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

pub const WORKFLOW_GRAPH_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_GRAPH_KIND: &str = "workflow_graph";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGraph {
    pub schema_version: String,
    #[serde(default = "default_workflow_graph_kind")]
    pub kind: String,
    pub graph_id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    #[serde(default, deserialize_with = "deserialize_graph_budgets")]
    pub budgets: Vec<GraphBudget>,
    pub stop_conditions: Vec<GraphStopCondition>,
    pub authority_boundary: GraphAuthorityBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub node_id: StableId,
    pub node_kind: GraphNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_ref: Option<RepoPath>,
    #[serde(default)]
    pub mutation_capable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass_condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_result: Option<GraphVerifierResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<GraphBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Operation,
    Verifier,
    HumanGate,
    MemoryRead,
    MemoryWriteCandidate,
    ProtocolCall,
    EvalProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphVerifierResult {
    Passed,
    Failed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub from: StableId,
    pub to: StableId,
    pub edge_kind: GraphEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeKind {
    /// Source must reach `RuntimePlanStatus::ReadyToCallOperation` (or an
    /// equivalent success terminal) before the target may be scheduled.
    /// Treated as a hard prerequisite by `unpassed_upstream_verifiers`.
    RequiresSuccess,
    /// Source must simply reach a terminal state (success or failure) before
    /// the target may be scheduled. Useful for fan-in where downstream work is
    /// observability rather than mutation.
    RequiresCompletion,
    /// Source must be a Verifier node whose `verifier_result == Passed` before
    /// any mutation-capable target may execute. This is the edge kind that
    /// powers `unpassed_upstream_verifiers`. Using it from a non-Verifier source
    /// produces an `EdgeKindSourceKindMismatch` warning: the runtime cannot
    /// evaluate "passed" for an `Operation`, `HumanGate`, or `Memory` node.
    BlocksUntilPassed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_model_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphStopCondition {
    ValidationErrors,
    BudgetExceeded,
    HumanRequired,
    VerifierFailed,
    AuthorityMissing,
    GateBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphAuthorityBoundary {
    pub source_of_truth: String,
    pub adapters_may_suggest: bool,
    pub adapters_may_mutate: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_authority_refs: Vec<RepoPath>,
}

// ---------------------------------------------------------------------------
// Diagnostics — migrated to the canonical `forge_core_validate` types (V2.B).
//
// `GraphDiagnostic`, `GraphDiagnosticSeverity`, and `GraphDiagnosticCode` were
// near-identical clones of `Diagnostic` / `DiagnosticSeverity` / `DiagnosticCode`.
// They are now aliases for the canonical types: the graph-specific code variants
// live in `forge_core_validate::DiagnosticCode` (prefixed `Graph*`), and the
// snake_case wire format is unchanged.
//
// `GraphValidationReport` is retained as a thin NEWTYPE-style wrapper because
// it carries a `graph_id` alongside the diagnostics (the canonical
// `ValidationReport` is a pure diagnostic bag with no identity). Every method
// delegates to the wrapped canonical report; the serialization shape (with
// `graph_id` and `diagnostics` fields) is preserved.
// ---------------------------------------------------------------------------

/// Canonical diagnostic, re-exported so graph callers keep their existing
/// import path (`GraphDiagnostic`) without touching call sites.
pub type GraphDiagnostic = forge_core_validate::Diagnostic;
/// Canonical diagnostic severity.
pub type GraphDiagnosticSeverity = forge_core_validate::DiagnosticSeverity;
/// Canonical diagnostic code (the graph-specific variants are `Graph*`-prefixed
/// members of the canonical enum).
pub type GraphDiagnosticCode = forge_core_validate::DiagnosticCode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphValidationReport {
    pub graph_id: StableId,
    #[serde(flatten)]
    pub report: forge_core_validate::ValidationReport,
}

impl GraphValidationReport {
    #[must_use]
    pub fn new(graph_id: StableId) -> Self {
        Self {
            graph_id,
            report: forge_core_validate::ValidationReport::new(),
        }
    }

    pub fn push(&mut self, diagnostic: GraphDiagnostic) {
        self.report.push(diagnostic);
    }

    pub fn extend(&mut self, other: Self) {
        self.report.extend(other.report);
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[GraphDiagnostic] {
        self.report.diagnostics()
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<GraphDiagnostic> {
        // `ValidationReport` keeps its `diagnostics` vec private; `diagnostics()`
        // borrows it, so clone out on conversion to preserve the public move API.
        self.report.diagnostics().to_vec()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.report.has_errors()
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.report.error_count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.report.warning_count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDryRunReport {
    pub graph_id: StableId,
    pub status: GraphDryRunStatus,
    pub steps: Vec<GraphDryRunStep>,
    pub diagnostics: Vec<GraphDiagnostic>,
    pub blocked_node_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDryRunStatus {
    Planned,
    Blocked,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDryRunStep {
    pub step_index: usize,
    pub node_id: StableId,
    pub node_kind: GraphNodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_ref: Option<RepoPath>,
    pub declared_mutation_capable: bool,
    pub mutation_capable: bool,
    pub mutation_source: GraphMutationSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_contract_id: Option<StableId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_status: Option<GraphOperationStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_preview_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_ready_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_runtime_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_plan_allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operation_blocking_reasons: Vec<String>,
    /// Repo paths the operation would touch if executed, sourced from the
    /// runtime preview. Empty for read-only nodes, verifier nodes, and any
    /// node whose operation contract could not be evaluated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operation_touched_refs: Vec<RepoPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_preflight: Option<GraphClaimPreflightEvaluation>,
    pub status: GraphDryRunStepStatus,
    pub reasons: Vec<GraphDryRunReason>,
    pub blocked_by: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDryRunStepStatus {
    Planned,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDryRunReason {
    TopologicalOrder,
    BlockedByVerifier,
    OperationContractMissing,
    OperationContractInvalid,
    OperationNotReady,
    ClaimPreflightBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphMutationSource {
    GraphDeclaration,
    OperationContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphOperationStatus {
    Ready,
    SafeReadOnly,
    NotReady,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOperationEvaluation {
    pub operation_ref: RepoPath,
    pub contract_id: Option<StableId>,
    pub mutation_capable: bool,
    pub runtime_ready: bool,
    pub plan_allowed: bool,
    pub status: GraphOperationStatus,
    pub preview_status: Option<String>,
    pub ready_status: Option<String>,
    pub blocking_reasons: Vec<String>,
    pub claim_preflight: Option<GraphClaimPreflightEvaluation>,
    /// Repo paths this operation would touch if executed, sourced from the
    /// runtime preview's `touched_refs` (union of `CoordinationScope.target.paths`
    /// and, where available, `ToolEffectContractDocument` write-sets). Empty for
    /// read-only operations, failed evaluations, and missing contracts.
    pub touched_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphClaimPreflightEvaluation {
    pub status: GraphClaimPreflightStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub governed_by_self: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ungoverned: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<GraphClaimPreflightBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphClaimPreflightStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphClaimPreflightBlock {
    pub blocked_path: RepoPath,
    pub blocking_claim_id: String,
    pub claimant: StableId,
    pub conflict_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphDryRunContext<'a> {
    pub operation_evaluations: &'a [GraphOperationEvaluation],
    pub require_operation_contracts: bool,
}

impl<'a> GraphDryRunContext<'a> {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            operation_evaluations: &[],
            require_operation_contracts: false,
        }
    }

    #[must_use]
    pub const fn requiring_operation_contracts(
        operation_evaluations: &'a [GraphOperationEvaluation],
    ) -> Self {
        Self {
            operation_evaluations,
            require_operation_contracts: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseWorkflowGraphError {
    YamlParseFailed { message: String },
}

impl fmt::Display for ParseWorkflowGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::YamlParseFailed { message } => {
                write!(formatter, "failed to parse workflow graph YAML: {message}")
            }
        }
    }
}

impl std::error::Error for ParseWorkflowGraphError {}

impl From<yaml_serde::Error> for ParseWorkflowGraphError {
    fn from(error: yaml_serde::Error) -> Self {
        Self::YamlParseFailed {
            message: error.to_string(),
        }
    }
}

/// Parse a workflow graph document from YAML.
///
/// # Errors
///
/// Returns [`ParseWorkflowGraphError::YamlParseFailed`] when the document is not
/// valid YAML or does not match the strict graph contract shape.
pub fn parse_workflow_graph_yaml(input: &str) -> Result<WorkflowGraph, ParseWorkflowGraphError> {
    yaml_serde::from_str(input).map_err(ParseWorkflowGraphError::from)
}

#[must_use]
pub fn validate_graph(graph: &WorkflowGraph) -> GraphValidationReport {
    let mut report = GraphValidationReport::new(graph.graph_id.clone());
    validate_graph_identity(&mut report, graph);
    validate_nodes(&mut report, graph);
    validate_edges(&mut report, graph);
    validate_cycles(&mut report, graph);
    validate_node_references(&mut report, graph);
    report
}

#[must_use]
pub fn dry_run_graph(graph: &WorkflowGraph) -> GraphDryRunReport {
    dry_run_graph_with_context(graph, GraphDryRunContext::empty())
}

#[must_use]
pub fn dry_run_graph_with_context(
    graph: &WorkflowGraph,
    context: GraphDryRunContext<'_>,
) -> GraphDryRunReport {
    let validation = validate_graph(graph);
    if validation.has_errors() {
        return GraphDryRunReport {
            graph_id: graph.graph_id.clone(),
            status: GraphDryRunStatus::Invalid,
            steps: Vec::new(),
            diagnostics: validation.into_diagnostics(),
            blocked_node_count: 0,
        };
    }

    let nodes_by_id = nodes_by_id(graph);
    let mut steps = Vec::with_capacity(graph.nodes.len());
    let mut blocked_node_count = 0usize;
    // Warnings (e.g. EdgeKindSourceKindMismatch) survive into the dry-run report
    // so consumers can surface them. Errors already short-circuited above.
    let mut diagnostics = validation.into_diagnostics();
    diagnostics.extend(operation_evaluation_diagnostics(
        context.operation_evaluations,
    ));
    let operation_evaluations = operation_evaluations_by_ref(context.operation_evaluations);

    for (step_index, node_id) in topological_order(graph).into_iter().enumerate() {
        let Some(node) = nodes_by_id.get(node_id.0.as_str()) else {
            continue;
        };
        let operation_evaluation = node
            .operation_ref
            .as_ref()
            .and_then(|reference| operation_evaluations.get(reference.0.as_str()).copied());
        append_mutation_declaration_diagnostic(&mut diagnostics, node, operation_evaluation);
        append_claim_preflight_diagnostic(&mut diagnostics, node, operation_evaluation);
        append_missing_operation_diagnostic(
            &mut diagnostics,
            node,
            operation_evaluation,
            context.require_operation_contracts,
        );
        let computation = dry_run_step(
            graph,
            node,
            node_id,
            step_index,
            operation_evaluation,
            context.require_operation_contracts,
        );
        if computation.blocked {
            blocked_node_count += 1;
        }
        steps.push(computation.step);
    }

    let has_diagnostic_errors = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == GraphDiagnosticSeverity::Error);

    GraphDryRunReport {
        graph_id: graph.graph_id.clone(),
        status: if blocked_node_count == 0 && !has_diagnostic_errors {
            GraphDryRunStatus::Planned
        } else {
            GraphDryRunStatus::Blocked
        },
        steps,
        diagnostics,
        blocked_node_count,
    }
}

struct StepComputation {
    blocked: bool,
    step: GraphDryRunStep,
}

fn dry_run_step(
    graph: &WorkflowGraph,
    node: &GraphNode,
    node_id: StableId,
    step_index: usize,
    operation_evaluation: Option<&GraphOperationEvaluation>,
    require_operation_contracts: bool,
) -> StepComputation {
    let effective_mutation_capable = operation_evaluation
        .map_or(node.mutation_capable, |evaluation| {
            evaluation.mutation_capable
        });
    let mutation_source = if operation_evaluation.is_some() {
        GraphMutationSource::OperationContract
    } else {
        GraphMutationSource::GraphDeclaration
    };
    let operation_block_reason =
        operation_block_reason(node, operation_evaluation, require_operation_contracts);
    let claim_preflight_blocked = operation_evaluation
        .and_then(|evaluation| evaluation.claim_preflight.as_ref())
        .is_some_and(|preflight| preflight.status == GraphClaimPreflightStatus::Blocked);
    let blocked_by = if node.node_kind == GraphNodeKind::Operation && effective_mutation_capable {
        unpassed_upstream_verifiers(graph, node_id.0.as_str())
    } else {
        Vec::new()
    };
    let blocked =
        operation_block_reason.is_some() || claim_preflight_blocked || !blocked_by.is_empty();
    let reasons =
        dry_run_step_reasons(operation_block_reason, claim_preflight_blocked, &blocked_by);

    StepComputation {
        blocked,
        step: GraphDryRunStep {
            step_index,
            node_id,
            node_kind: node.node_kind,
            operation_ref: node.operation_ref.clone(),
            declared_mutation_capable: node.mutation_capable,
            mutation_capable: effective_mutation_capable,
            mutation_source,
            operation_contract_id: operation_evaluation
                .and_then(|evaluation| evaluation.contract_id.clone()),
            operation_status: operation_evaluation.map(|evaluation| evaluation.status),
            operation_preview_status: operation_evaluation
                .and_then(|evaluation| evaluation.preview_status.clone()),
            operation_ready_status: operation_evaluation
                .and_then(|evaluation| evaluation.ready_status.clone()),
            operation_runtime_ready: operation_evaluation
                .map(|evaluation| evaluation.runtime_ready),
            operation_plan_allowed: operation_evaluation.map(|evaluation| evaluation.plan_allowed),
            operation_blocking_reasons: operation_evaluation
                .map_or_else(Vec::new, |evaluation| evaluation.blocking_reasons.clone()),
            operation_touched_refs: operation_evaluation
                .map_or_else(Vec::new, |evaluation| evaluation.touched_refs.clone()),
            claim_preflight: operation_evaluation
                .and_then(|evaluation| evaluation.claim_preflight.clone()),
            status: if blocked {
                GraphDryRunStepStatus::Blocked
            } else {
                GraphDryRunStepStatus::Planned
            },
            reasons,
            blocked_by,
        },
    }
}

fn dry_run_step_reasons(
    operation_block_reason: Option<GraphDryRunReason>,
    claim_preflight_blocked: bool,
    blocked_by: &[StableId],
) -> Vec<GraphDryRunReason> {
    let mut reasons = Vec::new();
    if let Some(reason) = operation_block_reason {
        reasons.push(reason);
    }
    if claim_preflight_blocked {
        reasons.push(GraphDryRunReason::ClaimPreflightBlocked);
    }
    if !blocked_by.is_empty() {
        reasons.push(GraphDryRunReason::BlockedByVerifier);
    }
    if reasons.is_empty() {
        reasons.push(GraphDryRunReason::TopologicalOrder);
    }
    reasons
}

fn operation_evaluations_by_ref(
    operation_evaluations: &[GraphOperationEvaluation],
) -> BTreeMap<&str, &GraphOperationEvaluation> {
    let mut by_ref = BTreeMap::new();
    for evaluation in operation_evaluations {
        by_ref
            .entry(evaluation.operation_ref.0.as_str())
            .or_insert(evaluation);
    }
    by_ref
}

fn operation_evaluation_diagnostics(
    operation_evaluations: &[GraphOperationEvaluation],
) -> Vec<GraphDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();
    for evaluation in operation_evaluations {
        let reference = evaluation.operation_ref.0.as_str();
        if !seen.insert(reference) {
            diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphDuplicateOperationEvaluation,
                format!("operation_ref.{reference}"),
                format!("operation_ref {reference} was evaluated more than once"),
            ));
        }
        match evaluation.status {
            GraphOperationStatus::Missing => diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphMissingOperationContract,
                format!("operation_ref.{reference}"),
                format!("operation_ref {reference} could not be read"),
            )),
            GraphOperationStatus::Invalid => diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphInvalidOperationContract,
                format!("operation_ref.{reference}"),
                format!("operation_ref {reference} could not be parsed or evaluated"),
            )),
            GraphOperationStatus::NotReady => diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphOperationNotReady,
                format!("operation_ref.{reference}"),
                format!("operation_ref {reference} is not ready for graph planning"),
            )),
            GraphOperationStatus::Ready | GraphOperationStatus::SafeReadOnly => {}
        }
    }
    diagnostics
}

fn append_missing_operation_diagnostic(
    diagnostics: &mut Vec<GraphDiagnostic>,
    node: &GraphNode,
    operation_evaluation: Option<&GraphOperationEvaluation>,
    require_operation_contracts: bool,
) {
    if !require_operation_contracts
        || node.node_kind != GraphNodeKind::Operation
        || operation_evaluation.is_some()
    {
        return;
    }
    let Some(reference) = &node.operation_ref else {
        return;
    };
    diagnostics.push(GraphDiagnostic::error(
        GraphDiagnosticCode::GraphMissingOperationContract,
        format!("nodes.{}.operation_ref", node.node_id.0),
        format!(
            "operation node {} requires operation_ref {} to be loaded before dry-run",
            node.node_id.0, reference.0
        ),
    ));
}

fn append_mutation_declaration_diagnostic(
    diagnostics: &mut Vec<GraphDiagnostic>,
    node: &GraphNode,
    operation_evaluation: Option<&GraphOperationEvaluation>,
) {
    let Some(evaluation) = operation_evaluation else {
        return;
    };
    if node.node_kind != GraphNodeKind::Operation
        || node.mutation_capable == evaluation.mutation_capable
    {
        return;
    }
    diagnostics.push(GraphDiagnostic::warning(
        GraphDiagnosticCode::GraphOperationMutationDeclarationMismatch,
        format!("nodes.{}.mutation_capable", node.node_id.0),
        format!(
            "operation node {} declared mutation_capable={}, but OperationContract {} derives mutation_capable={}",
            node.node_id.0,
            node.mutation_capable,
            evaluation.operation_ref.0,
            evaluation.mutation_capable
        ),
    ));
}

fn append_claim_preflight_diagnostic(
    diagnostics: &mut Vec<GraphDiagnostic>,
    node: &GraphNode,
    operation_evaluation: Option<&GraphOperationEvaluation>,
) {
    let Some(preflight) =
        operation_evaluation.and_then(|evaluation| evaluation.claim_preflight.as_ref())
    else {
        return;
    };
    if node.node_kind != GraphNodeKind::Operation
        || preflight.status != GraphClaimPreflightStatus::Blocked
    {
        return;
    }
    diagnostics.push(GraphDiagnostic::error(
        GraphDiagnosticCode::GraphClaimPreflightBlocked,
        format!("nodes.{}.claim_preflight", node.node_id.0),
        format!(
            "operation node {} failed claim preflight for {} target(s)",
            node.node_id.0,
            preflight.targets.len()
        ),
    ));
}

fn operation_block_reason(
    node: &GraphNode,
    operation_evaluation: Option<&GraphOperationEvaluation>,
    require_operation_contracts: bool,
) -> Option<GraphDryRunReason> {
    if node.node_kind != GraphNodeKind::Operation {
        return None;
    }
    let Some(evaluation) = operation_evaluation else {
        return require_operation_contracts.then_some(GraphDryRunReason::OperationContractMissing);
    };
    match evaluation.status {
        GraphOperationStatus::Missing => Some(GraphDryRunReason::OperationContractMissing),
        GraphOperationStatus::Invalid => Some(GraphDryRunReason::OperationContractInvalid),
        GraphOperationStatus::NotReady => Some(GraphDryRunReason::OperationNotReady),
        GraphOperationStatus::Ready | GraphOperationStatus::SafeReadOnly => None,
    }
}

fn validate_graph_identity(report: &mut GraphValidationReport, graph: &WorkflowGraph) {
    if graph.graph_id.0.trim().is_empty() {
        report.push(GraphDiagnostic::error(
            GraphDiagnosticCode::GraphEmptyGraphId,
            "graph_id",
            "graph_id must not be empty",
        ));
    }
    if graph.schema_version != WORKFLOW_GRAPH_SCHEMA_VERSION {
        report.push(GraphDiagnostic::error(
            GraphDiagnosticCode::GraphUnsupportedSchemaVersion,
            "schema_version",
            format!("workflow graph schema_version must be {WORKFLOW_GRAPH_SCHEMA_VERSION}"),
        ));
    }
    if graph.kind != WORKFLOW_GRAPH_KIND {
        report.push(GraphDiagnostic::error(
            GraphDiagnosticCode::GraphInvalidGraphKind,
            "kind",
            format!("workflow graph kind must be {WORKFLOW_GRAPH_KIND}"),
        ));
    }
}

fn validate_nodes(report: &mut GraphValidationReport, graph: &WorkflowGraph) {
    if graph.nodes.is_empty() {
        report.push(GraphDiagnostic::error(
            GraphDiagnosticCode::GraphEmptyGraph,
            "nodes",
            "workflow graph must contain at least one node",
        ));
    }

    let mut seen = HashSet::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        let node_path = format!("nodes.{index}.node_id");
        if node.node_id.0.trim().is_empty() {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphEmptyNodeId,
                node_path.clone(),
                "node_id must not be empty",
            ));
        }
        if !seen.insert(node.node_id.0.as_str()) {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphDuplicateNodeId,
                node_path,
                format!("node_id {} must be unique", node.node_id.0),
            ));
        }
        if node.node_kind == GraphNodeKind::Operation
            && node
                .operation_ref
                .as_ref()
                .is_none_or(|reference| reference.0.trim().is_empty())
        {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphEmptyOperationRef,
                format!("nodes.{index}.operation_ref"),
                "operation nodes require non-empty operation_ref",
            ));
        }
    }
}

fn validate_edges(report: &mut GraphValidationReport, graph: &WorkflowGraph) {
    let node_ids: HashSet<&str> = graph
        .nodes
        .iter()
        .map(|node| node.node_id.0.as_str())
        .collect();

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.from.0.trim().is_empty() {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphEmptyEdgeEndpoint,
                format!("edges.{index}.from"),
                "edge from endpoint must not be empty",
            ));
        } else if !node_ids.contains(edge.from.0.as_str()) {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphMissingEdgeEndpoint,
                format!("edges.{index}.from"),
                format!(
                    "edge from endpoint {} does not reference a node",
                    edge.from.0
                ),
            ));
        }

        if edge.to.0.trim().is_empty() {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphEmptyEdgeEndpoint,
                format!("edges.{index}.to"),
                "edge to endpoint must not be empty",
            ));
        } else if !node_ids.contains(edge.to.0.as_str()) {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphMissingEdgeEndpoint,
                format!("edges.{index}.to"),
                format!("edge to endpoint {} does not reference a node", edge.to.0),
            ));
        }
    }
}

fn validate_cycles(report: &mut GraphValidationReport, graph: &WorkflowGraph) {
    if graph.nodes.is_empty() {
        return;
    }
    if topological_order_if_acyclic(graph).is_none() {
        report.push(GraphDiagnostic::error(
            GraphDiagnosticCode::GraphCycleDetected,
            "edges",
            "workflow graph edges must be acyclic",
        ));
    }
}

/// Validate secondary references that the basic node/edge passes do not cover:
/// `GraphNode.verifies` and `GraphBudget.node_id`. Both must point at existing
/// node ids; otherwise verifier-blocking logic and budget attribution become
/// silently ineffective.
///
/// Also surfaces `EdgeKindSourceKindMismatch` warnings for `BlocksUntilPassed`
/// edges that originate from non-Verifier nodes (only Verifier nodes carry a
/// `verifier_result`, so "passed" cannot be evaluated for other node kinds).
///
/// `GraphNode.operation_ref` and `GraphAuthorityBoundary.required_authority_refs`
/// are intentionally NOT validated here: the former requires filesystem access
/// (handled by `forge graph run --dry-run` via `evaluate_graph_operations`),
/// and the latter requires the project's authority store.
fn validate_node_references(report: &mut GraphValidationReport, graph: &WorkflowGraph) {
    let node_ids: HashSet<&str> = graph
        .nodes
        .iter()
        .map(|node| node.node_id.0.as_str())
        .collect();
    let nodes_by_id: BTreeMap<&str, &GraphNode> = graph
        .nodes
        .iter()
        .map(|node| (node.node_id.0.as_str(), node))
        .collect();

    for (index, node) in graph.nodes.iter().enumerate() {
        for (slot, verified) in node.verifies.iter().enumerate() {
            if verified.0.trim().is_empty() {
                continue;
            }
            if !node_ids.contains(verified.0.as_str()) {
                report.push(GraphDiagnostic::error(
                    GraphDiagnosticCode::GraphDanglingVerifiesRef,
                    format!("nodes.{index}.verifies.{slot}"),
                    format!(
                        "verifies entry {verified} does not reference a node",
                        verified = verified.0
                    ),
                ));
            }
        }
    }

    for (index, budget) in graph.budgets.iter().enumerate() {
        let Some(node_ref) = budget.node_id.as_ref() else {
            continue;
        };
        if node_ref.0.trim().is_empty() {
            continue;
        }
        if !node_ids.contains(node_ref.0.as_str()) {
            report.push(GraphDiagnostic::error(
                GraphDiagnosticCode::GraphDanglingBudgetNodeRef,
                format!("budgets.{index}.node_id"),
                format!(
                    "budget node_id {node_ref} does not reference a node",
                    node_ref = node_ref.0
                ),
            ));
        }
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.edge_kind != GraphEdgeKind::BlocksUntilPassed {
            continue;
        }
        let Some(source) = nodes_by_id.get(edge.from.0.as_str()) else {
            // Dangling edge endpoints are already flagged by validate_edges.
            continue;
        };
        if source.node_kind != GraphNodeKind::Verifier {
            report.push(GraphDiagnostic::warning(
                GraphDiagnosticCode::GraphEdgeKindSourceKindMismatch,
                format!("edges.{index}.edge_kind"),
                format!(
                    "blocks_until_passed edge originates from {kind} node {id}; \
                     only verifier nodes carry a verifier_result, so the edge \
                     will be treated as a generic upstream dependency",
                    kind = graph_node_kind_name(source.node_kind),
                    id = edge.from.0
                ),
            ));
        }
    }
}

/// Lower-case wire name of a [`GraphNodeKind`] variant, matching the
/// `#[serde(rename_all = "snake_case")]` mapping. Kept inline to avoid pulling
/// `serde_json` into the graph crate just for diagnostic strings.
#[must_use]
fn graph_node_kind_name(kind: GraphNodeKind) -> &'static str {
    match kind {
        GraphNodeKind::Operation => "operation",
        GraphNodeKind::Verifier => "verifier",
        GraphNodeKind::HumanGate => "human_gate",
        GraphNodeKind::MemoryRead => "memory_read",
        GraphNodeKind::MemoryWriteCandidate => "memory_write_candidate",
        GraphNodeKind::ProtocolCall => "protocol_call",
        GraphNodeKind::EvalProbe => "eval_probe",
    }
}

fn topological_order(graph: &WorkflowGraph) -> Vec<StableId> {
    topological_order_if_acyclic(graph).unwrap_or_default()
}

fn topological_order_if_acyclic(graph: &WorkflowGraph) -> Option<Vec<StableId>> {
    let node_ids: BTreeSet<String> = graph
        .nodes
        .iter()
        .map(|node| node.node_id.0.clone())
        .collect();
    let mut indegree: BTreeMap<String, usize> = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), 0))
        .collect();
    let mut outgoing: BTreeMap<String, BTreeSet<String>> = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), BTreeSet::new()))
        .collect();

    for edge in &graph.edges {
        if !node_ids.contains(edge.from.0.as_str()) || !node_ids.contains(edge.to.0.as_str()) {
            continue;
        }
        if outgoing
            .get_mut(edge.from.0.as_str())
            .expect("outgoing map initialized from node ids")
            .insert(edge.to.0.clone())
        {
            *indegree
                .get_mut(edge.to.0.as_str())
                .expect("indegree map initialized from node ids") += 1;
        }
    }

    let mut ready: BTreeSet<String> = indegree
        .iter()
        .filter_map(|(node_id, count)| (*count == 0).then_some(node_id.clone()))
        .collect();
    let mut order = Vec::with_capacity(node_ids.len());

    while let Some(node_id) = ready.pop_first() {
        order.push(StableId(node_id.clone()));
        let children = outgoing
            .get(node_id.as_str())
            .expect("outgoing map initialized from node ids");
        for child in children {
            let count = indegree
                .get_mut(child.as_str())
                .expect("indegree map initialized from node ids");
            *count -= 1;
            if *count == 0 {
                ready.insert(child.clone());
            }
        }
    }

    (order.len() == node_ids.len()).then_some(order)
}

fn nodes_by_id(graph: &WorkflowGraph) -> BTreeMap<&str, &GraphNode> {
    graph
        .nodes
        .iter()
        .map(|node| (node.node_id.0.as_str(), node))
        .collect()
}

fn incoming_edges_by_target(graph: &WorkflowGraph) -> BTreeMap<&str, Vec<&str>> {
    let mut incoming: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &graph.edges {
        incoming
            .entry(edge.to.0.as_str())
            .or_default()
            .push(edge.from.0.as_str());
    }
    for sources in incoming.values_mut() {
        sources.sort_unstable();
        sources.dedup();
    }
    incoming
}

fn unpassed_upstream_verifiers(graph: &WorkflowGraph, target_id: &str) -> Vec<StableId> {
    let nodes = nodes_by_id(graph);
    let incoming = incoming_edges_by_target(graph);
    let mut blockers = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = incoming.get(target_id).cloned().unwrap_or_default();
    stack.sort_unstable_by(|left, right| right.cmp(left));

    while let Some(node_id) = stack.pop() {
        if !visited.insert(node_id) {
            continue;
        }
        if let Some(node) = nodes.get(node_id) {
            if node.node_kind == GraphNodeKind::Verifier
                && node.verifier_result != Some(GraphVerifierResult::Passed)
            {
                blockers.insert(node.node_id.0.clone());
            }
        }
        if let Some(parents) = incoming.get(node_id) {
            for parent in parents.iter().rev() {
                stack.push(parent);
            }
        }
    }

    blockers.into_iter().map(StableId).collect()
}

fn default_workflow_graph_kind() -> String {
    WORKFLOW_GRAPH_KIND.to_string()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GraphBudgetsInput {
    One(GraphBudget),
    Many(Vec<GraphBudget>),
}

fn deserialize_graph_budgets<'de, D>(deserializer: D) -> Result<Vec<GraphBudget>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(input) = Option::<GraphBudgetsInput>::deserialize(deserializer)? else {
        return Ok(Vec::new());
    };
    Ok(match input {
        GraphBudgetsInput::One(budget) => vec![budget],
        GraphBudgetsInput::Many(budgets) => budgets,
    })
}
