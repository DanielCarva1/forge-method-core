use forge_core_graph::{
    dry_run_graph, parse_workflow_graph_yaml, validate_graph, GraphDiagnosticCode,
    GraphDryRunReason, GraphDryRunStatus, GraphDryRunStepStatus, ParseWorkflowGraphError,
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
