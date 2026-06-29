use assert_cmd::Command;
use serde_json::Value;
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

fn fresh_parent(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root()
        .join("target")
        .join(format!("operation-sidecar-e2e-{label}-{n}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create fresh parent");
    root
}

struct ConsumerFixture {
    app: PathBuf,
    state_root: PathBuf,
}

fn consumer_fixture(label: &str) -> ConsumerFixture {
    let parent = fresh_parent(label);
    let app = parent.join("app");
    let sidecar_root = parent.join("forge-app");
    let state_root = sidecar_root.join(".forge-method");
    fs::create_dir_all(app.join("docs/fixtures/operation-contract-v0"))
        .expect("create operation fixture dir");
    fs::create_dir_all(app.join("contracts/commands")).expect("create command contracts dir");
    fs::create_dir_all(app.join("contracts/effects")).expect("create effect contracts dir");
    fs::create_dir_all(app.join("contracts/claims")).expect("create claim contracts dir");
    fs::create_dir_all(app.join("payloads")).expect("create payload dir");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    fs::write(app.join("README.md"), "# app\n").expect("write app readme");
    fs::copy(
        repo_root().join("docs/fixtures/operation-contract-v0/execute-trivial-write.yaml"),
        app.join("docs/fixtures/operation-contract-v0/execute-trivial-write.yaml"),
    )
    .expect("copy operation fixture");
    fs::write(
        app.join("docs/fixtures/operation-contract-v0/read-only-command.yaml"),
        read_only_command_operation(),
    )
    .expect("write read-only command operation");
    fs::copy(
        repo_root().join("contracts/commands/command-contract-v0.yaml"),
        app.join("contracts/commands/command-contract-v0.yaml"),
    )
    .expect("copy command contract definition");
    fs::write(
        app.join("contracts/commands/git-version.yaml"),
        git_version_command(),
    )
    .expect("write git version command contract");
    fs::copy(
        repo_root().join("contracts/effects/tool-effect-contract-v0.yaml"),
        app.join("contracts/effects/tool-effect-contract-v0.yaml"),
    )
    .expect("copy effect contract definition");
    fs::copy(
        repo_root().join("contracts/claims/story-v2-010-active-claim.yaml"),
        app.join("contracts/claims/story-v2-010-active-claim.yaml"),
    )
    .expect("copy claim fixture");
    fs::write(
        app.join("contracts/effects/story-artifact-write-effect.yaml"),
        story_artifact_write_effect(),
    )
    .expect("write sidecar-safe effect fixture");
    fs::write(app.join("payloads/story.yaml"), "story: completed\n")
        .expect("write artifact payload");
    fs::write(app.join("payloads/evidence.json"), r#"{"status":"passed"}"#)
        .expect("write evidence payload");

    ConsumerFixture { app, state_root }
}

fn read_only_command_operation() -> &'static str {
    r#"operation_contract:
  schema_version: "0.1"
  contract_id: "op_fixture_read_only_command_sidecar"
  created_at: "2026-06-29T00:00:00Z"
  project_ref:
    root: "."
    project_id: "app"
    state_version: 1
  source:
    host: "codex"
    surface: "cli_json"
    operation: "guide"
    human_input_digest: "sha256:fixture"
  autonomy:
    mode: "execute"
    rationale: "fixture"
  recommendation:
    next_actor: "host_agent"
    next_operation: "record_artifact"
    host_action: "call_operation"
    phase: "4-build-verify"
    workflow: "smoke"
    action: "run_read_only_command"
  authority:
    mutation_policy: "allowed"
    side_effect_policy: "read_only"
    authority_sources:
      - "operation_contract"
    authority_evidence:
      - kind: "operation_contract"
        ref: "contracts/commands/git-version.yaml"
    missing_authority: []
  coordination_scope:
    target:
      kind: "project"
      id: "app"
      product_area: "runtime-core"
      paths:
        - "README.md"
    concurrency:
      expected_state_version: 1
      agent_id: null
      caller_role: "driver"
      fleet_mode: false
      registry_ref: null
    write_authority:
      requires_driver_claim: false
      requires_lane_claim: false
      claim_contract_ref: null
    completion:
      must_check_completion: false
      completion_contract_ref: null
  execution_policy:
    mode: "single_step"
    max_steps: 1
    retry_policy:
      max_attempts: 0
      on_failure: "stop"
    branch_policy:
      allowed_branches: []
      default_branch: "stop"
  stop_policy:
    stop_when:
      - "execution_step_limit_reached"
    on_stop:
      next_actor: "human"
      next_operation: null
      host_action: "show_status"
  request: null
  decision_close: null
  runtime_handoff: null
  allowed_actions:
    - "read_contract"
  forbidden_actions:
    - "change_state"
  human:
    input_requirement: "none"
    prompt:
      mode: "none"
      text: ""
      options: []
    tone_contract: "curious_direct"
  loads:
    required: []
    optional: []
  gates:
    required_before_mutation: []
    current_gate_status: "pass"
    gate_contract_refs: []
  stop_conditions:
    - "execution_step_limit_reached"
  command_refs:
    - id: "cmd.fixture.git_version"
      required: true
  effect_contract_refs: []
  diagnostics:
    warnings: []
    errors: []
"#
}

fn git_version_command() -> &'static str {
    r#"schema_version: "0.1"
command_contract:
  id: "cmd.fixture.git_version"
  contract_ref: "contracts/commands/command-contract-v0.yaml"
  kind: "smoke"
  executor: "git"
  args:
    - "--version"
  cwd_policy: "project_root"
  side_effect_policy: "read_only"
  platforms:
    - "windows"
    - "macos"
    - "linux"
  timeout_ms: 30000
  env_policy:
    inherit: "minimal"
    required: []
    forbidden: []
  network_policy: "disabled"
  output_policy:
    capture: "summary"
    max_bytes: 12000
  authority_required:
    - "operation_contract"
  safety:
    shell_string_allowed: false
    writes_files: false
    publishes: false
    installs_packages: false
"#
}

fn story_artifact_write_effect() -> &'static str {
    r#"schema_version: "0.1"
tool_effect_contract:
  id: "effect.fixture.story_artifact_write"
  contract_ref: "contracts/effects/tool-effect-contract-v0.yaml"
  effect_kind: "artifact_write"
  operation_ref: "op_fixture_execute_trivial_write"
  actor:
    agent_id: "codex-main"
    role: "driver"
  read_set:
    - target_kind: "file_path"
      ref: "README.md"
      expected_hash: null
      expected_version: null
      required_for_plan: true
  write_set:
    - target_kind: "artifact_id"
      ref: ".forge-method/artifacts/story-current-result.yaml"
      access_mode: "create"
      expected_hash: null
      expected_version: null
      destructive: false
    - target_kind: "evidence_id"
      ref: ".forge-method/evidence/story-validation.json"
      access_mode: "append"
      expected_hash: null
      expected_version: null
      destructive: false
  conflict_detection:
    check_against: "latest_projection"
    granularity: "path"
    conflict_codes:
      - "read_target_changed"
      - "path_outside_scope"
    policy: "notify_and_repair"
  notification:
    required: true
    recipients:
      - "driver"
    request_contract_ref: null
  repair:
    strategy: "refresh_reads"
    automatic_repair_allowed: true
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
}

fn run_execute_fixture(fixture: &ConsumerFixture) -> Value {
    let output = bin()
        .args(["execute-operation", "--root"])
        .arg(&fixture.app)
        .args([
            "--operation",
            "docs/fixtures/operation-contract-v0/execute-trivial-write.yaml",
            "--effect",
            "contracts/effects/story-artifact-write-effect.yaml",
            "--payload",
            ".forge-method/artifacts/story-current-result.yaml=payloads/story.yaml",
            "--payload",
            ".forge-method/evidence/story-validation.json=payloads/evidence.json",
            "--recorded-at",
            "2026-06-29T00:00:00Z",
            "--tx-id-prefix",
            "operation-sidecar-e2e",
            "--json",
        ])
        .output()
        .expect("run execute-operation");

    assert!(
        output.status.success(),
        "execute-operation should complete\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("execute-operation json")
}

#[test]
fn execute_operation_writes_command_evidence_to_sidecar_not_consumer_repo() {
    let fixture = consumer_fixture("command-evidence");
    let output = bin()
        .args(["execute-operation", "--root"])
        .arg(&fixture.app)
        .args([
            "--operation",
            "docs/fixtures/operation-contract-v0/read-only-command.yaml",
            "--command",
            "contracts/commands/git-version.yaml",
            "--recorded-at",
            "2026-06-29T00:00:02Z",
            "--tx-id-prefix",
            "operation-sidecar-command-e2e",
            "--json",
        ])
        .output()
        .expect("run execute-operation with command ref");

    assert!(
        output.status.success(),
        "execute-operation command evidence run should complete\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("execute-operation json");
    assert_eq!(json["status"], "completed");
    assert_eq!(json["command_evidence_appended"], 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "command evidence run must not create consumer-local .forge-method"
    );
    let evidence_log = fixture
        .state_root
        .join("evidence")
        .join("command-execution.ndjson");
    let evidence = fs::read_to_string(&evidence_log).expect("read sidecar command evidence log");
    assert!(evidence.contains("cmd.fixture.git_version"));
    assert!(
        !fixture
            .app
            .join(".forge-method/evidence/command-execution.ndjson")
            .exists(),
        "consumer repo must not receive command evidence log"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn execute_operation_writes_forge_state_to_sidecar_not_consumer_repo() {
    let fixture = consumer_fixture("execute");
    let json = run_execute_fixture(&fixture);

    assert_eq!(json["status"], "completed");
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "execute-operation must not create consumer-local .forge-method"
    );
    assert!(
        fixture
            .state_root
            .join("artifacts/story-current-result.yaml")
            .exists(),
        "artifact write should land under sidecar state"
    );
    assert!(
        fixture
            .state_root
            .join("evidence/story-validation.json")
            .exists(),
        "evidence effect write should land under sidecar state"
    );
    assert!(
        fixture.state_root.join("wal/effects.ndjson").exists(),
        "effect WAL should land under sidecar state"
    );
    assert!(
        fixture
            .state_root
            .join("index/effect-targets.ndjson")
            .exists(),
        "effect metadata index should land under sidecar state"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn rebuild_and_query_effect_index_default_to_resolved_sidecar_state() {
    let fixture = consumer_fixture("rebuild-query");
    run_execute_fixture(&fixture);
    fs::remove_file(fixture.state_root.join("index/effect-targets.ndjson"))
        .expect("remove generated index before rebuild");

    let rebuild = bin()
        .args(["rebuild-effect-index", "--root"])
        .arg(&fixture.app)
        .args(["--recorded-at", "2026-06-29T00:00:01Z", "--json"])
        .output()
        .expect("run rebuild-effect-index");
    assert!(
        rebuild.status.success(),
        "rebuild-effect-index should use sidecar state\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rebuild.stdout),
        String::from_utf8_lossy(&rebuild.stderr)
    );

    let query = bin()
        .args(["query-effect-index", "--root"])
        .arg(&fixture.app)
        .args([
            "--logical-ref",
            ".forge-method/artifacts/story-current-result.yaml",
            "--latest",
            "--json",
        ])
        .output()
        .expect("run query-effect-index");
    assert!(
        query.status.success(),
        "query-effect-index should use sidecar state\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&query.stdout),
        String::from_utf8_lossy(&query.stderr)
    );
    let json: Value = serde_json::from_slice(&query.stdout).expect("query json");
    assert_eq!(json["status"], "queried");
    assert_eq!(json["matched_records"], 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "rebuild/query must not create consumer-local .forge-method"
    );
    assert!(
        fixture
            .state_root
            .join("index/effect-targets.ndjson")
            .exists(),
        "rebuilt index should land under sidecar state"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn state_bearing_commands_fail_closed_without_project_link() {
    let app = fresh_parent("missing-link").join("app");
    fs::create_dir_all(&app).expect("create unlinked app");

    let output = bin()
        .args(["query-effect-index", "--root"])
        .arg(&app)
        .args(["--logical-ref", "story.result", "--json"])
        .output()
        .expect("run query-effect-index without project link");

    assert!(
        !output.status.success(),
        "missing Project Link should fail closed"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(".forge-method.yaml"),
        "failure should explain missing Project Link\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !app.join(".forge-method").exists(),
        "failed command must not create consumer-local .forge-method"
    );
    let _ = fs::remove_dir_all(app.parent().expect("fixture app has parent directory"));
}
