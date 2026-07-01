//! F13 Budget/Cost Accounting — `forge-core cost` E2E.
//!
//! Builds a Forge-shaped consumer project, seeds its trace log with synthetic
//! cost-bearing `TraceEvents` via `append_trace_event`, then runs the
//! `forge-core cost` binary and asserts the aggregated report matches.

use assert_cmd::Command;
use forge_core_store::append_trace_event;
use forge_core_trace::{TraceActor, TraceCost, TraceEvent, TraceEventKind};
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

struct ConsumerScaffold {
    app: PathBuf,
    state_root: PathBuf,
}

fn fresh_consumer(label: &str) -> ConsumerScaffold {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let parent = repo_root()
        .join("target")
        .join(format!("cost-e2e-{label}-{n}"));
    let _ = fs::remove_dir_all(&parent);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");
    fs::create_dir_all(&app).expect("create app dir");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\n\
         project_id: cost-e2e\n\
         sidecar_root: ../forge-app\n\
         state_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    fs::write(app.join("README.md"), "# app\n").expect("write app readme");
    ConsumerScaffold { app, state_root }
}

fn cost_event(
    run_id: &str,
    agent_id: &str,
    model_calls: u64,
    tool_calls: u64,
    tokens: u64,
) -> TraceEvent {
    TraceEvent::new(
        format!("trace.{run_id}"),
        run_id,
        format!("{run_id}.evt.{}", model_calls + tool_calls + tokens),
        TraceEventKind::EffectApplied,
        "2026-06-30T00:00:00Z",
        "effect applied",
    )
    .with_actor(TraceActor::new("principal-1", agent_id, "driver"))
    .with_cost(TraceCost {
        model_calls,
        tool_calls,
        estimated_tokens: tokens,
    })
}

#[test]
fn cost_aggregates_all_events_when_no_scope_given() {
    let ConsumerScaffold { app, state_root } = fresh_consumer("all");
    append_trace_event(&state_root, &cost_event("run.a", "agent-1", 10, 5, 1_000))
        .expect("append event a1");
    append_trace_event(&state_root, &cost_event("run.a", "agent-2", 3, 2, 500))
        .expect("append event a2");
    append_trace_event(&state_root, &cost_event("run.b", "agent-1", 7, 1, 2_000))
        .expect("append event b1");

    let output = bin()
        .args(["cost", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run cost");
    assert!(output.status.success(), "cost should succeed");
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("parse cost envelope");
    let data = envelope.get("data").expect("envelope has data");
    let totals = data.get("totals").expect("report has totals");
    assert_eq!(totals["model_calls"], 20);
    assert_eq!(totals["tool_calls"], 8);
    assert_eq!(totals["estimated_tokens"], 3_500);
    assert_eq!(totals["event_count"], 3);
    // scope is All when no filter is given.
    assert_eq!(data["scope"], "all");
    // by_run sorted by descending tokens: run.b first.
    let by_run = data["by_run"].as_array().expect("by_run is array");
    assert_eq!(by_run[0]["key"], "run.b");
    assert_eq!(by_run[0]["totals"]["estimated_tokens"], 2_000);
}

#[test]
fn cost_scopes_to_a_single_run() {
    let ConsumerScaffold { app, state_root } = fresh_consumer("run-scope");
    append_trace_event(&state_root, &cost_event("run.a", "agent-1", 10, 0, 1_000))
        .expect("append event a");
    append_trace_event(&state_root, &cost_event("run.b", "agent-1", 4, 0, 4_000))
        .expect("append event b");

    let output = bin()
        .args(["cost", "--root"])
        .arg(&app)
        .args(["--run-id", "run.a"])
        .arg("--json")
        .output()
        .expect("run cost");
    assert!(output.status.success(), "cost should succeed");
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("parse cost envelope");
    let data = envelope.get("data").expect("envelope has data");
    assert_eq!(data["scope"], "run");
    assert_eq!(data["scope_id"], "run.a");
    assert_eq!(data["totals"]["model_calls"], 10);
    assert_eq!(data["totals"]["estimated_tokens"], 1_000);
}

#[test]
fn cost_reports_empty_when_trace_log_absent() {
    // A Forge project with no trace log yet: cost must succeed with a zero
    // report rather than failing, so a fresh project does not crash callers.
    let ConsumerScaffold { app, state_root: _ } = fresh_consumer("empty");
    let output = bin()
        .args(["cost", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run cost");
    assert!(
        output.status.success(),
        "cost should succeed on a project with no trace log"
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("parse cost envelope");
    let data = envelope.get("data").expect("envelope has data");
    assert_eq!(data["totals"]["event_count"], 0);
    assert_eq!(data["totals"]["estimated_tokens"], 0);
}
