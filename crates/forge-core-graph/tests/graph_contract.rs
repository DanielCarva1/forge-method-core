use forge_core_contracts::{RepoPath, StableId};
use forge_core_graph::{
    dry_run_graph, dry_run_graph_with_context, parse_workflow_graph_yaml, validate_graph,
    GraphClaimPreflightEvaluation, GraphClaimPreflightStatus, GraphDiagnosticCode,
    GraphDryRunContext, GraphDryRunReason, GraphDryRunStatus, GraphDryRunStepStatus,
    GraphMutationSource, GraphOperationEvaluation, GraphOperationStatus, ParseWorkflowGraphError,
};

const VALID_GRAPH: &str = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.valid"
created_at: "2026-06-29T00:00:00Z"
budgets:
  - budget_id: "budget.graph"
    max_steps: 20
    max_tool_calls: 8
nodes:
  - node_id: "plan"
    node_kind: "operation"
    operation_ref: "contracts/operations/plan.yaml"
    mutation_capable: false
    budget:
      max_steps: 3
      max_tool_calls: 2
  - node_id: "verify"
    node_kind: "verifier"
    verifies: ["plan"]
    pass_condition: "all_required_evidence_present"
    verifier_result: "passed"
  - node_id: "apply"
    node_kind: "operation"
    operation_ref: "contracts/operations/apply.yaml"
    mutation_capable: true
edges:
  - from: "plan"
    to: "verify"
    edge_kind: "requires_success"
  - from: "verify"
    to: "apply"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
  - "budget_exceeded"
  - "human_required"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;

#[test]
fn valid_graph_parses_validates_and_dry_runs_in_stable_order() {
    let graph = parse_workflow_graph_yaml(VALID_GRAPH).expect("valid graph parses");

    assert_eq!(graph.graph_id.0, "graph.valid");
    assert_eq!(graph.nodes.len(), 3);

    let validation = validate_graph(&graph);
    assert!(!validation.has_errors(), "{validation:#?}");

    let dry_run = dry_run_graph(&graph);
    assert_eq!(dry_run.status, GraphDryRunStatus::Planned);
    assert_eq!(dry_run.blocked_node_count, 0);
    assert_eq!(
        dry_run
            .steps
            .iter()
            .map(|step| step.node_id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["plan", "verify", "apply"]
    );
    assert!(dry_run
        .steps
        .iter()
        .all(|step| step.status == GraphDryRunStepStatus::Planned));
}

#[test]
fn unknown_fields_and_empty_graph_shape_are_rejected() {
    let unknown_field = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.unknown"
nodes: []
edges: []
stop_conditions: []
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
unexpected: true
"#;
    let error = parse_workflow_graph_yaml(unknown_field).expect_err("unknown root field rejects");
    assert!(matches!(
        error,
        ParseWorkflowGraphError::YamlParseFailed { .. }
    ));

    let empty_graph = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.empty"
budgets: []
nodes: []
edges: []
stop_conditions: []
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(empty_graph).expect("empty graph shape parses");
    let validation = validate_graph(&graph);
    assert!(validation.has_errors());
    assert!(validation
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == GraphDiagnosticCode::EmptyGraph));
}

#[test]
fn validation_accumulates_duplicate_missing_and_invalid_operation_diagnostics() {
    let invalid_graph = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.invalid"
budgets:
  max_steps: 10
nodes:
  - node_id: "plan"
    node_kind: "operation"
    operation_ref: ""
  - node_id: "plan"
    node_kind: "verifier"
    verifier_result: "passed"
edges:
  - from: "plan"
    to: "missing"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(invalid_graph).expect("invalid graph shape parses");
    let validation = validate_graph(&graph);
    let codes = validation
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(validation.has_errors());
    assert!(codes.contains(&GraphDiagnosticCode::DuplicateNodeId));
    assert!(codes.contains(&GraphDiagnosticCode::MissingEdgeEndpoint));
    assert!(codes.contains(&GraphDiagnosticCode::EmptyOperationRef));
    assert_eq!(validation.error_count(), 3);
}

#[test]
fn dangling_verifies_and_budget_refs_are_reported_as_errors() {
    // Verifier node points at a node that does not exist, and a budget is
    // attributed to another non-existent node. Both must surface as errors so
    // verifier-blocking logic and budget attribution do not silently no-op.
    let graph_yaml = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.dangling-refs"
budgets:
  - budget_id: "budget.lost"
    node_id: "ghost"
    max_steps: 5
nodes:
  - node_id: "plan"
    node_kind: "operation"
    operation_ref: "contracts/operations/plan.yaml"
  - node_id: "verify"
    node_kind: "verifier"
    verifies: ["plan", "missing_target"]
    pass_condition: "all_required_evidence_present"
    verifier_result: "passed"
edges: []
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(graph_yaml).expect("dangling-refs graph parses");
    let validation = validate_graph(&graph);
    let codes = validation
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(validation.has_errors());
    assert!(
        codes.contains(&GraphDiagnosticCode::DanglingVerifiesRef),
        "expected DanglingVerifiesRef in {codes:?}"
    );
    assert!(
        codes.contains(&GraphDiagnosticCode::DanglingBudgetNodeRef),
        "expected DanglingBudgetNodeRef in {codes:?}"
    );
    // The existing `plan` verifies entry must NOT produce a diagnostic.
    let dangling_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == GraphDiagnosticCode::DanglingVerifiesRef
                || diagnostic.code == GraphDiagnosticCode::DanglingBudgetNodeRef
        })
        .count();
    assert_eq!(
        dangling_count, 2,
        "expected exactly one dangling verifies + one dangling budget, got {dangling_count}"
    );
}

#[test]
fn blocks_until_passed_from_non_verifier_emits_warning_not_error() {
    // A `blocks_until_passed` edge from an Operation node is semantically
    // suspect: only Verifier nodes carry a verifier_result. The graph must
    // still validate (warning, not error) and dry-run, so existing fixtures
    // that rely on the v0 transitive behavior keep working.
    let graph_yaml = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.edge-mismatch"
nodes:
  - node_id: "plan"
    node_kind: "operation"
    operation_ref: "contracts/operations/plan.yaml"
  - node_id: "apply"
    node_kind: "operation"
    operation_ref: "contracts/operations/apply.yaml"
edges:
  - from: "plan"
    to: "apply"
    edge_kind: "blocks_until_passed"
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(graph_yaml).expect("edge-mismatch graph parses");
    let validation = validate_graph(&graph);

    assert!(
        !validation.has_errors(),
        "warnings should not escalate to errors; diagnostics = {:?}",
        validation.diagnostics()
    );
    assert!(validation.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == GraphDiagnosticCode::EdgeKindSourceKindMismatch
            && diagnostic.severity == forge_core_graph::GraphDiagnosticSeverity::Warning
    }));
    assert_eq!(validation.warning_count(), 1);

    // The graph must still dry-run: the warning is advisory, not blocking.
    let dry_run = dry_run_graph(&graph);
    assert!(dry_run
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == GraphDiagnosticCode::EdgeKindSourceKindMismatch }));
    assert_eq!(dry_run.status, GraphDryRunStatus::Planned);
}

#[test]
fn cycle_detection_reports_error() {
    let cyclic_graph = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.cycle"
budgets: []
nodes:
  - node_id: "a"
    node_kind: "operation"
    operation_ref: "contracts/operations/a.yaml"
  - node_id: "b"
    node_kind: "operation"
    operation_ref: "contracts/operations/b.yaml"
edges:
  - from: "a"
    to: "b"
    edge_kind: "requires_success"
  - from: "b"
    to: "a"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(cyclic_graph).expect("cyclic graph shape parses");
    let validation = validate_graph(&graph);

    assert!(validation.has_errors());
    assert!(validation
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == GraphDiagnosticCode::CycleDetected));
}

#[test]
fn verifier_nodes_block_downstream_mutation_capable_operations() {
    let blocked_graph = r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.blocked"
budgets: []
nodes:
  - node_id: "plan"
    node_kind: "operation"
    operation_ref: "contracts/operations/plan.yaml"
    mutation_capable: false
  - node_id: "verify"
    node_kind: "verifier"
    verifies: ["plan"]
    pass_condition: "all_required_evidence_present"
    verifier_result: "failed"
  - node_id: "apply"
    node_kind: "operation"
    operation_ref: "contracts/operations/apply.yaml"
    mutation_capable: true
edges:
  - from: "plan"
    to: "verify"
    edge_kind: "requires_success"
  - from: "verify"
    to: "apply"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
  - "verifier_failed"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#;
    let graph = parse_workflow_graph_yaml(blocked_graph).expect("blocked graph shape parses");
    let validation = validate_graph(&graph);
    assert!(!validation.has_errors(), "{validation:#?}");

    let dry_run = dry_run_graph(&graph);
    assert_eq!(dry_run.status, GraphDryRunStatus::Blocked);
    assert_eq!(dry_run.blocked_node_count, 1);
    let apply_step = dry_run
        .steps
        .iter()
        .find(|step| step.node_id.0 == "apply")
        .expect("apply step exists");
    assert_eq!(apply_step.status, GraphDryRunStepStatus::Blocked);
    assert_eq!(apply_step.blocked_by[0].0, "verify");
    assert_eq!(
        apply_step.reasons,
        vec![GraphDryRunReason::BlockedByVerifier]
    );
}

#[test]
fn operation_context_missing_contract_blocks_when_required() {
    let graph = parse_workflow_graph_yaml(
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.operation-context-missing"
nodes:
  - node_id: "read_status"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-status.yaml"
edges: []
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#,
    )
    .expect("graph parses");

    let dry_run = dry_run_graph_with_context(
        &graph,
        GraphDryRunContext::requiring_operation_contracts(&[]),
    );

    assert_eq!(dry_run.status, GraphDryRunStatus::Blocked);
    assert_eq!(dry_run.blocked_node_count, 1);
    assert!(dry_run
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == GraphDiagnosticCode::MissingOperationContract));
    let step = dry_run.steps.first().expect("one step");
    assert_eq!(step.status, GraphDryRunStepStatus::Blocked);
    assert_eq!(
        step.reasons,
        vec![GraphDryRunReason::OperationContractMissing]
    );
}

#[test]
fn operation_contract_mutation_overrides_graph_declaration_for_verifier_blocking() {
    let graph = parse_workflow_graph_yaml(
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.operation-context-mutation"
nodes:
  - node_id: "read_status"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-status.yaml"
    mutation_capable: false
  - node_id: "verify"
    node_kind: "verifier"
    verifier_result: "failed"
  - node_id: "write_artifact"
    node_kind: "operation"
    operation_ref: "contracts/operations/write-artifact.yaml"
    mutation_capable: false
edges:
  - from: "read_status"
    to: "verify"
    edge_kind: "requires_success"
  - from: "verify"
    to: "write_artifact"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#,
    )
    .expect("graph parses");
    let evaluations = vec![
        operation_evaluation(
            "contracts/operations/read-status.yaml",
            "op.read-status",
            false,
            true,
            GraphOperationStatus::Ready,
        ),
        operation_evaluation(
            "contracts/operations/write-artifact.yaml",
            "op.write-artifact",
            true,
            true,
            GraphOperationStatus::Ready,
        ),
    ];

    let dry_run = dry_run_graph_with_context(
        &graph,
        GraphDryRunContext::requiring_operation_contracts(&evaluations),
    );

    assert_eq!(dry_run.status, GraphDryRunStatus::Blocked);
    assert_eq!(dry_run.blocked_node_count, 1);
    assert!(dry_run.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == GraphDiagnosticCode::OperationMutationDeclarationMismatch
    }));
    let write_step = dry_run
        .steps
        .iter()
        .find(|step| step.node_id.0 == "write_artifact")
        .expect("write step exists");
    assert!(!write_step.declared_mutation_capable);
    assert!(write_step.mutation_capable);
    assert_eq!(
        write_step.mutation_source,
        GraphMutationSource::OperationContract
    );
    assert_eq!(write_step.status, GraphDryRunStepStatus::Blocked);
    assert_eq!(write_step.blocked_by[0].0, "verify");
    assert_eq!(write_step.operation_runtime_ready, Some(true));
    assert_eq!(write_step.operation_plan_allowed, Some(true));
}

#[test]
fn claim_preflight_block_blocks_ready_mutation_step() {
    let graph = parse_workflow_graph_yaml(
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.claim-preflight"
nodes:
  - node_id: "write_artifact"
    node_kind: "operation"
    operation_ref: "contracts/operations/write-artifact.yaml"
    mutation_capable: false
edges: []
stop_conditions:
  - "validation_errors"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
"#,
    )
    .expect("graph parses");
    let mut evaluation = operation_evaluation(
        "contracts/operations/write-artifact.yaml",
        "op.write-artifact",
        true,
        true,
        GraphOperationStatus::Ready,
    );
    evaluation.claim_preflight = Some(GraphClaimPreflightEvaluation {
        status: GraphClaimPreflightStatus::Blocked,
        agent_id: Some(StableId("codex-main".to_string())),
        targets: vec![RepoPath(".forge-method/artifacts/story.yaml".to_string())],
        governed_by_self: Vec::new(),
        ungoverned: vec![RepoPath(".forge-method/artifacts/story.yaml".to_string())],
        blocks: Vec::new(),
        reasons: vec!["target is ungoverned".to_string()],
    });
    let dry_run = dry_run_graph_with_context(
        &graph,
        GraphDryRunContext::requiring_operation_contracts(&[evaluation]),
    );

    assert_eq!(dry_run.status, GraphDryRunStatus::Blocked);
    assert_eq!(dry_run.blocked_node_count, 1);
    assert!(dry_run
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == GraphDiagnosticCode::ClaimPreflightBlocked));
    let write_step = dry_run.steps.first().expect("write step exists");
    assert_eq!(write_step.status, GraphDryRunStepStatus::Blocked);
    assert!(write_step
        .reasons
        .contains(&GraphDryRunReason::ClaimPreflightBlocked));
    assert_eq!(
        write_step
            .claim_preflight
            .as_ref()
            .expect("claim preflight included")
            .status,
        GraphClaimPreflightStatus::Blocked
    );
}

fn operation_evaluation(
    operation_ref: &str,
    contract_id: &str,
    mutation_capable: bool,
    plan_allowed: bool,
    status: GraphOperationStatus,
) -> GraphOperationEvaluation {
    GraphOperationEvaluation {
        operation_ref: RepoPath(operation_ref.to_string()),
        contract_id: Some(StableId(contract_id.to_string())),
        mutation_capable,
        runtime_ready: plan_allowed,
        plan_allowed,
        status,
        preview_status: Some("ready".to_string()),
        ready_status: Some("ready".to_string()),
        blocking_reasons: Vec::new(),
        claim_preflight: None,
        touched_refs: Vec::new(),
    }
}
