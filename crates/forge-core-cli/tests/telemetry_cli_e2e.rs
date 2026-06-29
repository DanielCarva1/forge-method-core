use assert_cmd::Command;
use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

const RAW_AGENT_ID: &str = "agent.codex.raw.telemetry";
const RAW_FILE_PATH: &str = "src/domain/secret_plan.rs";

struct ProjectFixture {
    app: PathBuf,
    state_root: PathBuf,
}

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

fn fresh_project(label: &str) -> ProjectFixture {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let parent = repo_root()
        .join("target")
        .join(format!("telemetry-cli-e2e-{label}-{n}"));
    let app = parent.join("app");
    let state_root = parent.join("forge-app").join(".forge-method");
    let _ = fs::remove_dir_all(&parent);
    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    ProjectFixture { app, state_root }
}

fn write_contract(
    app: &Path,
    name: &str,
    enabled: bool,
    sink: &str,
    redact_paths: bool,
    hash_agent_ids: bool,
    fields: &[&str],
) -> PathBuf {
    let contract_dir = app.join("contracts");
    fs::create_dir_all(&contract_dir).expect("create contract dir");
    let mut fields_yaml = String::new();
    for field in fields {
        writeln!(&mut fields_yaml, "        - {field}").expect("write field YAML");
    }
    let contract = format!(
        r#"schema_version: "0.1"
telemetry_contract:
  id: telemetry.test.{name}
  enabled: {enabled}
  sink: {sink}
  events:
    - kind: gate_evaluated
      record: true
      fields:
{fields_yaml}  sampling:
    rate: 10000
    max_per_second: 100
    always_record_kinds:
      - gate_evaluated
  privacy:
    redact_secrets: true
    redact_paths: {redact_paths}
    hash_agent_ids: {hash_agent_ids}
    denylist_field_globs:
      - "*.secret"
      - "*.token"
  correlation:
    trace_parent: null
    run_id_ref: null
    span_id_seed: telemetry-e2e
"#
    );
    let path = contract_dir.join(format!("{name}.yaml"));
    fs::write(&path, contract).expect("write telemetry contract");
    path
}

fn write_trace_events(state_root: &Path, events: &[TraceEvent]) {
    let trace_path = state_root.join("traces").join("events.ndjson");
    fs::create_dir_all(trace_path.parent().expect("trace parent")).expect("create trace dir");
    let mut text = String::new();
    for event in events {
        text.push_str(&serde_json::to_string(event).expect("serialize trace event"));
        text.push('\n');
    }
    fs::write(trace_path, text).expect("write trace events");
}

fn trace_event(run_id: &str, event_id: &str, event_kind: TraceEventKind) -> TraceEvent {
    TraceEvent::new(
        format!("trace.{run_id}"),
        run_id,
        event_id,
        event_kind,
        "2026-06-29T12:00:00Z",
        "telemetry export e2e event",
    )
    .with_project_id("app")
    .with_actor(TraceActor::new(
        "principal.human.daniel",
        RAW_AGENT_ID,
        "worker",
    ))
    .with_authority(TraceAuthority {
        operation_id: Some("operation.telemetry.e2e".into()),
        capability_ids: vec!["capability.telemetry.export".into()],
    })
    .with_inputs(vec![TraceRef::new("file", RAW_FILE_PATH)])
    .with_outputs(vec![TraceRef::new(
        "artifact",
        "target/telemetry/report.json",
    )])
    .with_risk(TraceRisk::new(TraceRiskLevel::Low, false))
    .with_cost(TraceCost {
        model_calls: 1,
        tool_calls: 2,
        estimated_tokens: 345,
    })
}

fn run_export_json(
    app: &Path,
    contract: &Path,
    output_path: &Path,
    selector_flag: &str,
    selector_value: Option<&str>,
) -> Output {
    let mut command = bin();
    command.args([
        "telemetry",
        "export",
        "--root",
        &app.display().to_string(),
        "--contract",
        &contract.display().to_string(),
        "--output",
        &output_path.display().to_string(),
        "--format",
        "jsonl",
    ]);
    if let Some(value) = selector_value {
        command.args([selector_flag, value]);
    } else {
        command.arg(selector_flag);
    }
    command
        .arg("--json")
        .output()
        .expect("run telemetry export")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_state_root(summary: &Value, expected_state_root: &Path) {
    let actual = summary["state_root"]
        .as_str()
        .expect("summary should report state_root");
    let actual = fs::canonicalize(PathBuf::from(actual)).expect("canonicalize actual state_root");
    let expected = fs::canonicalize(expected_state_root).expect("canonicalize expected state_root");
    assert_eq!(actual, expected);
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    let text = fs::read_to_string(path).expect("read JSONL output");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse JSONL record"))
        .collect()
}

#[test]
fn telemetry_export_jsonl_reads_sidecar_trace_and_reports_gaps() {
    let fixture = fresh_project("jsonl-gaps");
    let contract = write_contract(
        &fixture.app,
        "jsonl-gaps",
        true,
        "jsonl_file",
        false,
        false,
        &["gate_status", "duration_ms", "failure_cluster"],
    );
    write_trace_events(
        &fixture.state_root,
        &[
            trace_event("run.old", "evt.old.0001", TraceEventKind::GatePassed),
            trace_event("run.latest", "evt.latest.0001", TraceEventKind::GateBlocked),
            trace_event(
                "run.latest",
                "evt.latest.0002",
                TraceEventKind::RunCompleted,
            ),
        ],
    );
    let output_path = fixture.app.join("telemetry-out.ndjson");

    let output = run_export_json(&fixture.app, &contract, &output_path, "--latest-run", None);

    assert_success(&output);
    let summary = stdout_json(&output);
    assert_eq!(summary["status"], "exported");
    assert!(summary["exported_event_count"].as_u64().unwrap_or(0) > 0);
    assert_state_root(&summary, &fixture.state_root);
    assert!(summary["missing_field_count"].as_u64().unwrap_or(0) > 0);
    assert!(summary["field_gaps"]
        .as_array()
        .expect("summary should report field_gaps")
        .iter()
        .any(|gap| gap.to_string().contains("duration_ms")));

    let records = read_jsonl(&output_path);
    assert!(!records.is_empty(), "JSONL output should contain records");
    assert!(records
        .iter()
        .all(|record| record["run_id"] == "run.latest"));
    assert!(!fs::read_to_string(&output_path)
        .expect("read output")
        .contains("run.old"));
    assert!(!fixture.app.join(".forge-method").exists());
}

#[test]
fn telemetry_export_disabled_contract_noops_without_inventing_records() {
    let fixture = fresh_project("disabled-noop");
    let contract = write_contract(
        &fixture.app,
        "disabled-noop",
        false,
        "disabled",
        false,
        false,
        &["gate_status"],
    );
    write_trace_events(
        &fixture.state_root,
        &[trace_event(
            "run.disabled",
            "evt.disabled.0001",
            TraceEventKind::GatePassed,
        )],
    );
    let output_path = fixture.app.join("disabled-out.ndjson");

    let output = run_export_json(
        &fixture.app,
        &contract,
        &output_path,
        "--run-id",
        Some("run.disabled"),
    );

    assert_success(&output);
    let summary = stdout_json(&output);
    assert_eq!(summary["status"], "noop");
    assert_eq!(summary["exported_event_count"], 0);
    assert_state_root(&summary, &fixture.state_root);
    if output_path.exists() {
        assert!(
            fs::read_to_string(&output_path)
                .expect("read disabled output")
                .trim()
                .is_empty(),
            "disabled telemetry must not invent output records"
        );
    }
}

#[test]
fn telemetry_export_redacts_paths_and_hashes_agent_ids() {
    let fixture = fresh_project("privacy");
    let contract = write_contract(
        &fixture.app,
        "privacy",
        true,
        "jsonl_file",
        true,
        true,
        &["gate_status", "agent_id", "input_refs"],
    );
    write_trace_events(
        &fixture.state_root,
        &[trace_event(
            "run.privacy",
            "evt.privacy.0001",
            TraceEventKind::GatePassed,
        )],
    );
    let output_path = fixture.app.join("privacy-out.ndjson");

    let output = run_export_json(
        &fixture.app,
        &contract,
        &output_path,
        "--run-id",
        Some("run.privacy"),
    );

    assert_success(&output);
    let summary = stdout_json(&output);
    assert_eq!(summary["status"], "exported");
    let text = fs::read_to_string(&output_path).expect("read privacy output");
    assert!(
        text.contains("sha256:"),
        "redacted records should contain sha256 refs: {text}"
    );
    assert!(
        !text.contains(RAW_AGENT_ID),
        "raw agent id leaked into telemetry output: {text}"
    );
    assert!(
        !text.contains(RAW_FILE_PATH),
        "raw file path leaked into telemetry output: {text}"
    );
}

#[test]
fn telemetry_export_uses_sidecar_trace_not_consumer_local_state() {
    let fixture = fresh_project("sidecar-isolation");
    let contract = write_contract(
        &fixture.app,
        "sidecar-isolation",
        true,
        "jsonl_file",
        false,
        false,
        &["gate_status"],
    );
    write_trace_events(
        &fixture.state_root,
        &[trace_event(
            "run.sidecar",
            "evt.sidecar.0001",
            TraceEventKind::GatePassed,
        )],
    );
    write_trace_events(
        &fixture.app.join(".forge-method"),
        &[trace_event(
            "run.local",
            "evt.local.0001",
            TraceEventKind::GateBlocked,
        )],
    );
    let output_path = fixture.app.join("sidecar-out.ndjson");

    let output = run_export_json(
        &fixture.app,
        &contract,
        &output_path,
        "--run-id",
        Some("run.sidecar"),
    );

    assert_success(&output);
    let summary = stdout_json(&output);
    assert_state_root(&summary, &fixture.state_root);
    let text = fs::read_to_string(&output_path).expect("read sidecar output");
    assert!(text.contains("run.sidecar"));
    assert!(!text.contains("run.local"));
}

#[test]
fn telemetry_export_rejects_contract_path_escaping_project_root() {
    let fixture = fresh_project("contract-escape");
    let outside_contract = fixture
        .app
        .parent()
        .expect("app parent")
        .join("outside-telemetry.yaml");
    fs::write(&outside_contract, "not used\n").expect("write outside contract");
    write_trace_events(
        &fixture.state_root,
        &[trace_event(
            "run.escape",
            "evt.escape.0001",
            TraceEventKind::GatePassed,
        )],
    );
    let output_path = fixture.app.join("escape-out.ndjson");

    let output = run_export_json(
        &fixture.app,
        &outside_contract,
        &output_path,
        "--run-id",
        Some("run.escape"),
    );

    assert!(
        !output.status.success(),
        "contract path escape should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("contract"), "{stderr}");
    assert!(
        stderr.contains("project root") || stderr.contains("under"),
        "{stderr}"
    );
    assert!(!output_path.exists());
}

#[test]
fn telemetry_export_rejects_output_path_escaping_project_root() {
    let fixture = fresh_project("output-escape");
    let contract = write_contract(
        &fixture.app,
        "output-escape",
        true,
        "jsonl_file",
        false,
        false,
        &["gate_status"],
    );
    write_trace_events(
        &fixture.state_root,
        &[trace_event(
            "run.output.escape",
            "evt.output.escape.0001",
            TraceEventKind::GatePassed,
        )],
    );
    let outside_output = fixture
        .app
        .parent()
        .expect("app parent")
        .join("outside-telemetry-output.ndjson");
    fs::write(&outside_output, "must-not-be-replaced\n").expect("write sentinel output");

    let output = run_export_json(
        &fixture.app,
        &contract,
        &outside_output,
        "--run-id",
        Some("run.output.escape"),
    );

    assert!(
        !output.status.success(),
        "output path escape should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("output"), "{stderr}");
    assert!(
        stderr.contains("project root") || stderr.contains("under"),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(&outside_output).expect("read sentinel output"),
        "must-not-be-replaced\n"
    );
}
