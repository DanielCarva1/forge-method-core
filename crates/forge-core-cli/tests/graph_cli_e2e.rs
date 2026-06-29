use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn fresh_project(label: &str) -> (PathBuf, PathBuf) {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let parent = repo_root()
        .join("target")
        .join(format!("graph-cli-e2e-{label}-{n}"));
    let app = parent.join("app");
    let sidecar = parent.join("forge-app").join(".forge-method");
    let _ = fs::remove_dir_all(&parent);
    fs::create_dir_all(app.join("graphs")).expect("create app graph dir");
    fs::create_dir_all(&sidecar).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    (app, sidecar)
}

fn write_graph(app: &Path, name: &str, contents: &str) {
    fs::write(app.join("graphs").join(name), contents).expect("write graph fixture");
}

fn valid_graph() -> &'static str {
    r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.valid"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "read_a"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-a.yaml"
    budget:
      max_steps: 1
      max_tool_calls: 0
  - node_id: "read_b"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-b.yaml"
    budget:
      max_steps: 1
      max_tool_calls: 0
edges: []
stop_conditions:
  - "validation_errors"
  - "budget_exceeded"
"#
}

fn duplicate_node_graph() -> &'static str {
    r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.duplicate"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "same"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-a.yaml"
  - node_id: "same"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-b.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#
}

fn verifier_blocks_mutation_graph() -> &'static str {
    r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.blocked"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "read_current_state"
    node_kind: "operation"
    operation_ref: "contracts/operations/read-current-state.yaml"
    mutation_capable: false
  - node_id: "verify_write_authority"
    node_kind: "verifier"
    verifies:
      - "read_current_state"
    pass_condition: "all_required_evidence_present"
    verifier_result: "failed"
  - node_id: "write_artifact"
    node_kind: "operation"
    operation_ref: "contracts/operations/write-artifact.yaml"
    mutation_capable: true
edges:
  - from: "read_current_state"
    to: "verify_write_authority"
    edge_kind: "requires_success"
  - from: "verify_write_authority"
    to: "write_artifact"
    edge_kind: "requires_success"
stop_conditions:
  - "validation_errors"
  - "verifier_failed"
"#
}

#[test]
fn graph_validate_resolves_project_before_relative_graph_path() {
    let (app, sidecar) = fresh_project("validate");
    write_graph(&app, "valid.yaml", valid_graph());

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/valid.yaml",
            "--json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "graph validate should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["project_id"], "app");
    assert_eq!(json["status"], "passed");
    assert!(Path::new(json["graph_path"].as_str().unwrap()).ends_with("graphs/valid.yaml"));
    assert_eq!(json["state_root"], sidecar.display().to_string());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_uses_sidecar_resolution_without_creating_local_state() {
    let (app, sidecar) = fresh_project("run");
    write_graph(&app, "valid.yaml", valid_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/valid.yaml",
            "--dry-run",
            "--json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "graph dry-run should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["project_id"], "app");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["dry_run_executed"], true);
    assert!(json["report"].is_object());
    assert_eq!(json["state_root"], sidecar.display().to_string());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_validate_exits_nonzero_when_validation_blocks() {
    let (app, _sidecar) = fresh_project("invalid");
    write_graph(&app, "duplicate.yaml", duplicate_node_graph());

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/duplicate.yaml",
            "--json",
        ])
        .output()
        .expect("run graph validate");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["project_id"], "app");
    assert_eq!(json["status"], "blocked");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_exits_nonzero_when_verifier_blocks_mutation() {
    let (app, _sidecar) = fresh_project("blocked");
    write_graph(&app, "blocked.yaml", verifier_blocks_mutation_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/blocked.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["project_id"], "app");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["dry_run_executed"], true);
    assert_eq!(json["report"]["status"], "blocked");
    assert_eq!(json["report"]["blocked_node_count"], 1);
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn published_verifier_block_fixture_exits_nonzero_with_bootstrap_exception() {
    let root = repo_root();
    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &root.display().to_string(),
            "--graph",
            "docs/fixtures/workflow-graph-v0/verifier-blocks-mutation.yaml",
            "--dry-run",
            "--allow-bootstrap-core",
            "--json",
        ])
        .output()
        .expect("run published verifier-block fixture");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["dry_run_executed"], true);
    assert_eq!(json["report"]["status"], "blocked");
    assert_eq!(json["report"]["blocked_node_count"], 1);
}
