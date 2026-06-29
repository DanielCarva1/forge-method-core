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
        .join(format!("eval-cli-e2e-{label}-{n}"));
    let app = parent.join("app");
    let sidecar = parent.join("forge-app").join(".forge-method");
    let _ = fs::remove_dir_all(&parent);
    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&sidecar).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    (app, sidecar)
}

fn copy_eval_fixtures(app: &Path) {
    let source = repo_root()
        .join("docs")
        .join("fixtures")
        .join("eval-run-v0");
    let target = app.join("docs").join("fixtures").join("eval-run-v0");
    copy_dir(&source, &target);
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create target dir");
    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("source entry");
        let entry_source = entry.path();
        let entry_target = target.join(entry.file_name());
        if entry_source.is_dir() {
            copy_dir(&entry_source, &entry_target);
        } else {
            fs::copy(&entry_source, &entry_target).expect("copy fixture file");
        }
    }
}

fn prepend_utf8_bom(path: &Path) {
    let content = fs::read(path).expect("read file before BOM");
    let mut with_bom = b"\xEF\xBB\xBF".to_vec();
    with_bom.extend_from_slice(&content);
    fs::write(path, with_bom).expect("write UTF-8 BOM file");
}

#[test]
fn eval_compare_default_suite_outputs_deterministic_json() {
    let root = repo_root();

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &root.display().to_string(),
            "--allow-bootstrap-core",
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "eval compare should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema_version"], "0.1");
    assert_eq!(
        json["comparison_id"],
        "eval.compare.graph_vs_single_agent.smoke"
    );
    assert_eq!(json["baseline"], "single-agent");
    assert_eq!(json["candidate"], "graph");
    assert_eq!(json["source"], "precomputed_eval_runs");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["task_count"], 2);
    assert_eq!(json["baseline_summary"]["success_rate_bps"], 10_000);
    assert_eq!(json["candidate_summary"]["success_rate_bps"], 10_000);
    assert!(json["deltas"]["total_cost_usd_micros"].as_i64().unwrap() < 0);
    assert_eq!(json["recommendation"], "try_candidate");
    assert!(json["measurement_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| { gap.as_str().unwrap().contains("human_intervention_count") }));
}

#[test]
fn eval_compare_accepts_utf8_bom_suite_and_run_fixtures() {
    let (app, _sidecar) = fresh_project("bom-fixtures");
    copy_eval_fixtures(&app);
    prepend_utf8_bom(
        &app.join("docs")
            .join("fixtures")
            .join("eval-run-v0")
            .join("eval-compare-smoke-suite.yaml"),
    );
    prepend_utf8_bom(
        &app.join("docs")
            .join("fixtures")
            .join("eval-run-v0")
            .join("single-agent")
            .join("task-a.yaml"),
    );

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &app.display().to_string(),
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "eval compare should accept UTF-8 BOM fixtures: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "passed");
    assert_eq!(json["recommendation"], "try_candidate");
}

#[test]
fn eval_compare_human_output_uses_sidecar_project_without_mutating_state() {
    let (app, sidecar) = fresh_project("sidecar-human");
    copy_eval_fixtures(&app);
    let trace = sidecar.join("traces").join("events.ndjson");
    fs::create_dir_all(trace.parent().expect("trace parent")).expect("create trace dir");
    fs::write(&trace, "preexisting\n").expect("write trace sentinel");

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &app.display().to_string(),
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--no-json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "eval compare should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("forge_core_eval_compare"));
    assert!(stdout.contains("recommendation=TryCandidate"));
    assert_eq!(fs::read_to_string(&trace).unwrap(), "preexisting\n");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn eval_compare_missing_baseline_value_exits_3() {
    let root = repo_root();

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &root.display().to_string(),
            "--allow-bootstrap-core",
            "--baseline",
            "--candidate",
            "graph",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("eval compare: missing value for --baseline"));
}

#[test]
fn eval_compare_unsupported_label_exits_3() {
    let root = repo_root();

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &root.display().to_string(),
            "--allow-bootstrap-core",
            "--baseline",
            "single-agent",
            "--candidate",
            "magic",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported eval arm 'magic'"));
}

#[test]
fn eval_compare_missing_fixture_file_exits_env_config() {
    let (app, _sidecar) = fresh_project("missing-fixture");
    fs::create_dir_all(app.join("docs").join("fixtures").join("eval-run-v0"))
        .expect("create fixture dir");
    fs::write(
        app.join("docs")
            .join("fixtures")
            .join("eval-run-v0")
            .join("eval-compare-smoke-suite.yaml"),
        r#"schema_version: "0.1"
eval_compare_suite:
  id: "eval.suite.missing"
  comparison_id: "eval.compare.missing"
  baseline:
    label: "single-agent"
    run_refs:
      - "docs/fixtures/eval-run-v0/missing-single.yaml"
  candidate:
    label: "graph"
    run_refs:
      - "docs/fixtures/eval-run-v0/missing-graph.yaml"
  policy:
    require_matching_tasks: true
    require_evidence_refs: true
    require_trace_refs: true
    minimum_task_count: 1
"#,
    )
    .expect("write missing suite");

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &app.display().to_string(),
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("read eval run"));
}

#[test]
fn eval_compare_unsupported_run_schema_exits_env_config() {
    let (app, _sidecar) = fresh_project("bad-run-schema");
    copy_eval_fixtures(&app);
    let run_path = app
        .join("docs")
        .join("fixtures")
        .join("eval-run-v0")
        .join("single-agent")
        .join("task-a.yaml");
    let content = fs::read_to_string(&run_path).expect("read run fixture");
    fs::write(
        &run_path,
        content.replacen(r#"schema_version: "0.1""#, r#"schema_version: "9.9""#, 1),
    )
    .expect("write mutated run fixture");

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &app.display().to_string(),
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported schema_version '9.9'"));
}

#[test]
fn eval_compare_unsupported_suite_schema_exits_env_config() {
    let (app, _sidecar) = fresh_project("bad-suite-schema");
    copy_eval_fixtures(&app);
    let suite_path = app
        .join("docs")
        .join("fixtures")
        .join("eval-run-v0")
        .join("eval-compare-smoke-suite.yaml");
    let content = fs::read_to_string(&suite_path).expect("read suite fixture");
    fs::write(
        &suite_path,
        content.replacen(r#"schema_version: "0.1""#, r#"schema_version: "9.9""#, 1),
    )
    .expect("write mutated suite fixture");

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &app.display().to_string(),
            "--baseline",
            "single-agent",
            "--candidate",
            "graph",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported schema_version '9.9'"));
}

#[test]
fn eval_compare_blocked_report_exits_1() {
    let root = repo_root();

    let output = bin()
        .args([
            "eval",
            "compare",
            "--root",
            &root.display().to_string(),
            "--allow-bootstrap-core",
            "--baseline",
            "single-agent",
            "--candidate",
            "mas",
            "--json",
        ])
        .output()
        .expect("run eval compare");

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "blocked");
    assert!(json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| {
            diagnostic["code"] == "candidate_label_mismatch" && diagnostic["severity"] == "error"
        }));
}
