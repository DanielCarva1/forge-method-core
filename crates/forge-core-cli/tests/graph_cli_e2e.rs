use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command as ProcessCommand;
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

fn copy_repo_file(app: &Path, source_relative: &str, target_relative: &str) {
    let target = app.join(target_relative);
    fs::create_dir_all(target.parent().expect("target file has parent"))
        .expect("create target parent");
    fs::copy(repo_root().join(source_relative), target).expect("copy repo fixture");
}

fn install_read_operation(app: &Path, target_relative: &str) {
    copy_repo_file(
        app,
        "docs/fixtures/operation-contract-v0/observe-project-status.yaml",
        target_relative,
    );
}

fn install_write_operation(app: &Path, target_relative: &str) {
    copy_repo_file(
        app,
        "docs/fixtures/operation-contract-v0/execute-trivial-write.yaml",
        target_relative,
    );
    copy_repo_file(
        app,
        "contracts/effects/story-artifact-write-effect.yaml",
        "contracts/effects/story-artifact-write-effect.yaml",
    );
    copy_repo_file(
        app,
        "contracts/claims/story-v2-010-active-claim.yaml",
        "contracts/claims/story-v2-010-active-claim.yaml",
    );
}

fn install_review_operation(app: &Path, target_relative: &str) {
    copy_repo_file(
        app,
        "docs/fixtures/operation-contract-v0/plan-sprint-slice.yaml",
        target_relative,
    );
}

fn create_directory_link(link: &Path, target: &Path) {
    fs::create_dir_all(link.parent().expect("link parent")).expect("create link parent");
    create_directory_link_platform(link, target).unwrap_or_else(|message| panic!("{message}"));
}

#[cfg(windows)]
fn create_directory_link_platform(link: &Path, target: &Path) -> Result<(), String> {
    let output = ProcessCommand::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.display().to_string(),
            &target.display().to_string(),
        ])
        .output()
        .map_err(|source| format!("create junction failed to start: {source}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "create junction failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(not(windows))]
fn create_directory_link_platform(link: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link)
        .map_err(|source| format!("create symlink failed: {source}"))
}

fn step_by_id<'a>(json: &'a serde_json::Value, node_id: &str) -> &'a serde_json::Value {
    json["report"]["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .find(|step| step["node_id"] == node_id)
        .expect("step exists")
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

fn ready_mutation_graph() -> &'static str {
    r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.ready-mutation"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "write_artifact"
    node_kind: "operation"
    operation_ref: "contracts/operations/write-artifact.yaml"
    mutation_capable: false
edges: []
stop_conditions:
  - "validation_errors"
"#
}

fn write_effect_with_single_write(
    app: &Path,
    target_kind: &str,
    target_ref: &str,
    access_mode: &str,
) {
    fs::write(
        app.join("contracts")
            .join("effects")
            .join("story-artifact-write-effect.yaml"),
        format!(
            r#"schema_version: "0.1"
tool_effect_contract:
  id: "effect.fixture.story_artifact_write"
  contract_ref: "contracts/effects/tool-effect-contract-v0.yaml"
  effect_kind: "file_edit"
  operation_ref: "op_fixture_execute_trivial_write"
  actor:
    agent_id: "codex-main"
    role: "driver"
  read_set: []
  write_set:
    - target_kind: "{target_kind}"
      ref: "{target_ref}"
      access_mode: "{access_mode}"
      expected_hash: null
      expected_version: null
      destructive: false
  conflict_detection:
    check_against: "latest_projection"
    granularity: "path"
    conflict_codes:
      - "write_target_claimed"
      - "path_outside_scope"
    policy: "block"
  notification:
    required: false
    recipients: []
    request_contract_ref: null
  repair:
    strategy: "none"
    automatic_repair_allowed: false
    inverse_operation_ref: null
    stop_if_inverse_missing: false
    inverse:
      kind: "none"
      source: "unavailable"
      ref: null
      input_mapping_refs: []
      validation_gate_refs: []
      review_required: false
"#
        ),
    )
    .expect("write custom effect");
}

fn acquire_claim(app: &Path, agent: &str, claim_paths: &[&str]) {
    let mut command = bin();
    command.args([
        "claim",
        "acquire",
        "--root",
        &app.display().to_string(),
        "--scope",
        "story",
        "--id",
        "graph-claim-preflight",
        "--agent",
        agent,
    ]);
    for path in claim_paths {
        command.args(["--path", *path]);
    }
    let output = command.unwrap();
    assert!(
        output.status.success(),
        "claim acquire should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    install_read_operation(&app, "contracts/operations/read-a.yaml");
    install_read_operation(&app, "contracts/operations/read-b.yaml");
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
fn graph_validate_rejects_graph_file_outside_project_root() {
    let (app, _sidecar) = fresh_project("validate-graph-outside-root");
    let outside_graph = app
        .parent()
        .expect("fresh project parent")
        .join("outside-graphs")
        .join("valid.yaml");
    fs::create_dir_all(outside_graph.parent().expect("outside graph parent"))
        .expect("create outside graph parent");
    fs::write(&outside_graph, valid_graph()).expect("write outside graph");

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            &outside_graph.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run graph validate");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("graph file path"));
    assert!(stderr.contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_rejects_graph_file_outside_project_root() {
    let (app, _sidecar) = fresh_project("run-graph-outside-root");
    install_read_operation(&app, "contracts/operations/read-a.yaml");
    install_read_operation(&app, "contracts/operations/read-b.yaml");
    let outside_graph = app
        .parent()
        .expect("fresh project parent")
        .join("outside-graphs")
        .join("valid.yaml");
    fs::create_dir_all(outside_graph.parent().expect("outside graph parent"))
        .expect("create outside graph parent");
    fs::write(&outside_graph, valid_graph()).expect("write outside graph");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            &outside_graph.display().to_string(),
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("graph file path"));
    assert!(stderr.contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_validate_rejects_graph_file_symlink_escape() {
    let (app, _sidecar) = fresh_project("validate-graph-symlink-escape");
    let outside_dir = app
        .parent()
        .expect("fresh project parent")
        .join("outside-graphs");
    fs::create_dir_all(&outside_dir).expect("create outside graph dir");
    fs::write(outside_dir.join("valid.yaml"), valid_graph()).expect("write outside graph");
    create_directory_link(&app.join("graphs").join("linked"), &outside_dir);

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/linked/valid.yaml",
            "--json",
        ])
        .output()
        .expect("run graph validate");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("graph file path"));
    assert!(stderr.contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_validate_rejects_nonexistent_graph_file_under_symlink_escape() {
    let (app, _sidecar) = fresh_project("validate-graph-symlink-missing");
    let outside_dir = app
        .parent()
        .expect("fresh project parent")
        .join("outside-graphs");
    fs::create_dir_all(&outside_dir).expect("create outside graph dir");
    create_directory_link(&app.join("graphs").join("linked"), &outside_dir);

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/linked/missing.yaml",
            "--json",
        ])
        .output()
        .expect("run graph validate");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("graph file path"));
    assert!(stderr.contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_validate_allows_nonexistent_graph_file_inside_project_until_read() {
    let (app, _sidecar) = fresh_project("validate-graph-missing-inside");

    let output = bin()
        .args([
            "graph",
            "validate",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/missing.yaml",
            "--json",
        ])
        .output()
        .expect("run graph validate");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("read graph"),
        "inside missing graph should reach read failure, got: {stderr}"
    );
    assert!(
        !stderr.contains("escapes project root"),
        "inside missing graph must not be misclassified as a boundary escape: {stderr}"
    );
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_rejects_nonexistent_graph_file_under_symlink_escape() {
    let (app, _sidecar) = fresh_project("run-graph-symlink-missing");
    let outside_dir = app
        .parent()
        .expect("fresh project parent")
        .join("outside-graphs");
    fs::create_dir_all(&outside_dir).expect("create outside graph dir");
    create_directory_link(&app.join("graphs").join("linked"), &outside_dir);

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/linked/missing.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("graph file path"));
    assert!(stderr.contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_allows_nonexistent_graph_file_inside_project_until_read() {
    let (app, _sidecar) = fresh_project("run-graph-missing-inside");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/missing.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("read graph"),
        "inside missing graph should reach read failure, got: {stderr}"
    );
    assert!(
        !stderr.contains("escapes project root"),
        "inside missing graph must not be misclassified as a boundary escape: {stderr}"
    );
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_with_missing_state_root_reports_env_config() {
    let (app, sidecar) = fresh_project("missing-state-root");
    install_read_operation(&app, "contracts/operations/read-a.yaml");
    install_read_operation(&app, "contracts/operations/read-b.yaml");
    write_graph(&app, "valid.yaml", valid_graph());
    let sidecar_root = sidecar
        .parent()
        .expect("sidecar state root has sidecar parent")
        .to_path_buf();
    fs::remove_dir_all(&sidecar_root).expect("remove sidecar root");

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
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("env_config"));
    assert!(stderr.contains("state_root"));
    assert!(stderr.contains(&sidecar.display().to_string()));
    assert!(!app.join(".forge-method").exists());
    assert!(!sidecar.exists());
    assert!(!sidecar_root.exists());
}

#[test]
fn graph_run_rejects_flag_looking_agent_value() {
    let (app, _sidecar) = fresh_project("flag-agent");
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
            "--agent",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("graph: missing value for --agent"));
}

#[test]
fn graph_run_invalid_now_unix_reports_graph_prefix() {
    let (app, _sidecar) = fresh_project("bad-now-unix");
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
            "--now-unix",
            "not-a-number",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("graph: invalid value for --now-unix"));
}

#[test]
fn graph_run_dry_run_resolves_operation_refs_from_project_root() {
    let (app, _sidecar) = fresh_project("operation-ref");
    install_read_operation(&app, "contracts/operations/observe.yaml");
    write_graph(
        &app,
        "operation-resolution.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.operation-resolution"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "read_status"
    node_kind: "operation"
    operation_ref: "contracts/operations/observe.yaml"
    mutation_capable: false
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/operation-resolution.yaml",
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
    let step = step_by_id(&json, "read_status");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["report"]["status"], "planned");
    assert_eq!(
        step["operation_contract_id"],
        "op_fixture_observe_project_status"
    );
    assert_eq!(step["operation_status"], "safe_read_only");
    assert_eq!(step["operation_plan_allowed"], true);
    assert_eq!(step["mutation_source"], "operation_contract");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_when_operation_ref_file_is_missing() {
    let (app, _sidecar) = fresh_project("missing-operation");
    write_graph(
        &app,
        "missing-operation.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.missing-operation"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "missing_operation"
    node_kind: "operation"
    operation_ref: "contracts/operations/does-not-exist.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/missing-operation.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "missing_operation");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["dry_run_executed"], true);
    assert_eq!(json["report"]["status"], "blocked");
    assert_eq!(step["status"], "blocked");
    assert_eq!(step["operation_status"], "missing");
    assert_eq!(step["reasons"][0], "operation_contract_missing");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_rejects_operation_ref_parent_escape() {
    let (app, _sidecar) = fresh_project("operation-ref-escape");
    write_graph(
        &app,
        "operation-ref-escape.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.operation-ref-escape"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "escaped_operation"
    node_kind: "operation"
    operation_ref: "../outside-operation.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/operation-ref-escape.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "escaped_operation");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["operation_status"], "invalid");
    assert_eq!(step["reasons"][0], "operation_contract_invalid");
    assert!(step["operation_blocking_reasons"][0]
        .as_str()
        .expect("blocking reason")
        .contains("unsafe operation_ref"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_rejects_absolute_operation_ref() {
    let (app, _sidecar) = fresh_project("operation-ref-absolute");
    let absolute_ref = repo_root()
        .join("docs")
        .join("fixtures")
        .join("operation-contract-v0")
        .join("observe-project-status.yaml");
    write_graph(
        &app,
        "operation-ref-absolute.yaml",
        &format!(
            r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.operation-ref-absolute"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "absolute_operation"
    node_kind: "operation"
    operation_ref: '{}'
edges: []
stop_conditions:
  - "validation_errors"
"#,
            absolute_ref.display()
        ),
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/operation-ref-absolute.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "absolute_operation");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["operation_status"], "invalid");
    assert_eq!(step["reasons"][0], "operation_contract_invalid");
    assert!(step["operation_blocking_reasons"][0]
        .as_str()
        .expect("blocking reason")
        .contains("unsafe operation_ref"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_rejects_operation_ref_symlink_escape() {
    let (app, _sidecar) = fresh_project("operation-ref-symlink");
    let outside_dir = app
        .parent()
        .expect("fresh project parent")
        .join("outside-ops");
    fs::create_dir_all(&outside_dir).expect("create outside operation dir");
    fs::copy(
        repo_root()
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0")
            .join("observe-project-status.yaml"),
        outside_dir.join("observe.yaml"),
    )
    .expect("copy outside operation");
    create_directory_link(
        &app.join("contracts").join("operations").join("linked"),
        &outside_dir,
    );
    write_graph(
        &app,
        "operation-ref-symlink.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.operation-ref-symlink"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "linked_operation"
    node_kind: "operation"
    operation_ref: "contracts/operations/linked/observe.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/operation-ref-symlink.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "linked_operation");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["operation_status"], "invalid");
    assert_eq!(step["reasons"][0], "operation_contract_invalid");
    assert!(step["operation_blocking_reasons"][0]
        .as_str()
        .expect("blocking reason")
        .contains("escapes project root"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_invalid_operation_contract_yaml() {
    let (app, _sidecar) = fresh_project("invalid-operation");
    let operation_path = app
        .join("contracts")
        .join("operations")
        .join("invalid.yaml");
    fs::create_dir_all(operation_path.parent().expect("operation parent"))
        .expect("create operation parent");
    fs::write(&operation_path, "operation_contract: [").expect("write invalid operation");
    write_graph(
        &app,
        "invalid-operation.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.invalid-operation"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "invalid_operation"
    node_kind: "operation"
    operation_ref: "contracts/operations/invalid.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/invalid-operation.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "invalid_operation");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["operation_status"], "invalid");
    assert_eq!(step["reasons"][0], "operation_contract_invalid");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_review_required_operation_contract() {
    let (app, _sidecar) = fresh_project("review-required-operation");
    install_review_operation(&app, "contracts/operations/plan-sprint.yaml");
    write_graph(
        &app,
        "review-required-operation.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.review-required-operation"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "plan_sprint"
    node_kind: "operation"
    operation_ref: "contracts/operations/plan-sprint.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/review-required-operation.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "plan_sprint");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["operation_status"], "not_ready");
    assert_eq!(step["operation_preview_status"], "review_required");
    assert!(step["operation_blocking_reasons"]
        .as_array()
        .expect("blocking reasons")
        .iter()
        .any(|reason| reason == "mutation_requires_review"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_does_not_append_sidecar_trace_or_ledger() {
    let (app, sidecar) = fresh_project("no-trace-mutation");
    install_read_operation(&app, "contracts/operations/observe.yaml");
    let trace = sidecar.join("traces").join("events.ndjson");
    fs::create_dir_all(trace.parent().expect("trace parent")).expect("create trace parent");
    fs::write(&trace, "preexisting-trace\n").expect("write trace sentinel");
    let ledger = sidecar.join("ledger.ndjson");
    fs::write(&ledger, "preexisting-ledger\n").expect("write ledger sentinel");
    write_graph(
        &app,
        "no-trace-mutation.yaml",
        r#"
schema_version: "0.1"
kind: "workflow_graph"
graph_id: "graph.e2e.no-trace-mutation"
created_at: "2026-06-29T00:00:00Z"
authority_boundary:
  source_of_truth: "forge-core-runtime"
  adapters_may_suggest: true
  adapters_may_mutate: false
nodes:
  - node_id: "read_status"
    node_kind: "operation"
    operation_ref: "contracts/operations/observe.yaml"
edges: []
stop_conditions:
  - "validation_errors"
"#,
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/no-trace-mutation.yaml",
            "--dry-run",
            "--json",
        ])
        .unwrap();

    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&trace).unwrap(), "preexisting-trace\n");
    assert_eq!(fs::read_to_string(&ledger).unwrap(), "preexisting-ledger\n");
    assert!(!sidecar.join("effects").exists());
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
    install_read_operation(&app, "contracts/operations/read-current-state.yaml");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
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
fn graph_run_dry_run_blocks_ready_mutation_without_agent_for_claim_preflight() {
    let (app, _sidecar) = fresh_project("claim-preflight-no-agent");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["status"], "blocked");
    assert!(step["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .any(|reason| reason == "claim_preflight_blocked"));
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert!(step["claim_preflight"]["reasons"][0]
        .as_str()
        .expect("claim preflight reason")
        .contains("--agent"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_ready_mutation_without_covering_claim() {
    let (app, _sidecar) = fresh_project("claim-preflight-ungoverned");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert_eq!(step["claim_preflight"]["agent_id"], "codex-main");
    assert_eq!(
        step["claim_preflight"]["ungoverned"]
            .as_array()
            .expect("ungoverned")
            .len(),
        2
    );
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_preflights_file_path_effect_targets() {
    let (app, _sidecar) = fresh_project("claim-preflight-file-path");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_effect_with_single_write(&app, "file_path", "src/generated.txt", "write");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());
    acquire_claim(&app, "codex-main", &["src/"]);

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert!(
        output.status.success(),
        "graph dry-run should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(step["claim_preflight"]["status"], "passed");
    assert_eq!(step["claim_preflight"]["targets"][0], "src/generated.txt");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_glob_effect_targets_fail_closed() {
    let (app, _sidecar) = fresh_project("claim-preflight-glob");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_effect_with_single_write(&app, "glob", "src/**/*.rs", "write");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert!(step["claim_preflight"]["reasons"][0]
        .as_str()
        .expect("claim preflight reason")
        .contains("glob write targets"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_state_key_effect_targets_fail_closed() {
    let (app, _sidecar) = fresh_project("claim-preflight-state-key");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_effect_with_single_write(&app, "state_key", ".forge-method/state.yaml#phase", "write");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert!(step["claim_preflight"]["reasons"][0]
        .as_str()
        .expect("claim preflight reason")
        .contains("resolve effect target"));
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_passes_ready_mutation_with_covering_sidecar_claim() {
    let (app, sidecar) = fresh_project("claim-preflight-owned");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());
    acquire_claim(
        &app,
        "codex-main",
        &[".forge-method/artifacts/", ".forge-method/evidence/"],
    );
    let trace = sidecar.join("traces").join("events.ndjson");
    fs::create_dir_all(trace.parent().expect("trace parent")).expect("create trace parent");
    fs::write(&trace, "preexisting-trace\n").expect("write trace sentinel");
    let ledger = sidecar.join("ledger.ndjson");
    fs::write(&ledger, "preexisting-ledger\n").expect("write ledger sentinel");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert!(
        output.status.success(),
        "graph dry-run should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "passed");
    assert_eq!(step["claim_preflight"]["status"], "passed");
    assert_eq!(
        step["claim_preflight"]["governed_by_self"]
            .as_array()
            .expect("governed")
            .len(),
        2
    );
    assert_eq!(fs::read_to_string(&trace).unwrap(), "preexisting-trace\n");
    assert_eq!(fs::read_to_string(&ledger).unwrap(), "preexisting-ledger\n");
    assert!(!sidecar.join("effects").exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_ready_mutation_with_expired_self_claim() {
    let (app, _sidecar) = fresh_project("claim-preflight-expired");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());
    acquire_claim(
        &app,
        "codex-main",
        &[".forge-method/artifacts/", ".forge-method/evidence/"],
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--now-unix",
            "4102444800",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert_eq!(
        step["claim_preflight"]["ungoverned"][0],
        ".forge-method/artifacts/story-current-result.yaml"
    );
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_honors_claims_dir_override() {
    let (app, sidecar) = fresh_project("claim-preflight-override");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());
    acquire_claim(&app, "codex-main", &[".forge-method/artifacts/"]);
    let override_claims = app
        .parent()
        .expect("fresh project parent")
        .join("override-claims");
    fs::create_dir_all(&override_claims).expect("create override claims dir");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--claims-dir",
            &override_claims.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["claims_dir"], override_claims.display().to_string());
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert!(step["claim_preflight"]["ungoverned"].is_array());
    assert!(sidecar.join("claims-active").exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_ready_mutation_claimed_by_peer_agent() {
    let (app, _sidecar) = fresh_project("claim-preflight-peer");
    install_write_operation(&app, "contracts/operations/write-artifact.yaml");
    write_graph(&app, "ready-mutation.yaml", ready_mutation_graph());
    acquire_claim(
        &app,
        "peer-agent",
        &[".forge-method/artifacts/", ".forge-method/evidence/"],
    );

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &app.display().to_string(),
            "--graph",
            "graphs/ready-mutation.yaml",
            "--dry-run",
            "--agent",
            "codex-main",
            "--json",
        ])
        .output()
        .expect("run graph dry-run");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(step["claim_preflight"]["status"], "blocked");
    assert_eq!(
        step["claim_preflight"]["blocks"][0]["claimant"],
        "peer-agent"
    );
    assert_eq!(
        step["claim_preflight"]["blocks"][0]["conflict_code"],
        "write_target_claimed"
    );
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn graph_run_dry_run_blocks_operation_contract_that_is_not_ready() {
    let root = repo_root();
    let graph = root
        .join("docs")
        .join("fixtures")
        .join("workflow-graph-v0")
        .join("operation-aware-blocked.yaml");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &root.display().to_string(),
            "--graph",
            &graph.display().to_string(),
            "--dry-run",
            "--allow-bootstrap-core",
            "--json",
        ])
        .output()
        .expect("run operation-aware blocked graph");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "release_gate");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["report"]["status"], "blocked");
    assert_eq!(step["status"], "blocked");
    assert_eq!(step["operation_status"], "not_ready");
    assert_eq!(step["operation_preview_status"], "gate_required");
    assert_eq!(step["operation_runtime_ready"], false);
    assert!(step["operation_blocking_reasons"]
        .as_array()
        .expect("blocking reasons")
        .iter()
        .any(|reason| reason == "gate_missing_or_pending" || reason == "gate_pending"));
}

#[test]
fn graph_run_dry_run_derives_mutation_from_operation_contract_over_graph_false() {
    let root = repo_root();
    let graph = root
        .join("docs")
        .join("fixtures")
        .join("workflow-graph-v0")
        .join("operation-aware-valid.yaml");

    let output = bin()
        .args([
            "graph",
            "run",
            "--root",
            &root.display().to_string(),
            "--graph",
            &graph.display().to_string(),
            "--dry-run",
            "--allow-bootstrap-core",
            "--json",
        ])
        .output()
        .expect("run operation-aware mutation graph");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = step_by_id(&json, "write_artifact");
    assert_eq!(json["status"], "blocked");
    assert_eq!(json["report"]["blocked_node_count"], 1);
    assert_eq!(step["status"], "blocked");
    assert_eq!(step["declared_mutation_capable"], false);
    assert_eq!(step["mutation_capable"], true);
    assert_eq!(step["mutation_source"], "operation_contract");
    assert_eq!(step["blocked_by"][0], "verify_write_authority");
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
