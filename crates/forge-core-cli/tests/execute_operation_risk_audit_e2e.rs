//! F11.3 Risk Audit Gate enforcement in `execute-operation` — E2E.
//!
//! V3.A moved the risk-audit and citation gates from inline CLI steps into the
//! kernel (attached via `.with_gate(...)`). The `--require-risk-audit` flag is
//! now CONFIG (which gate to attach), not the gate's location. The kernel runs
//! the gate against the operation plan BEFORE any WAL append, so:
//!
//! - `blocked`: anti-pattern in source + flag → fail-closed, WAL untouched,
//!   trace emitted.
//! - `passes_when_clean`: clean source + flag → gate passes; the ready mutation
//!   plan reaches the runtime and stops before application because this focused
//!   fixture deliberately omits the effect input.
//! - `without_flag_skips_audit`: anti-pattern in source, NO flag → gate does
//!   not attach; no risk-audit error.
//! - `invalid_rules_yaml`: malformed rules YAML + flag → `ParseYaml` error
//!   propagated clearly (the CLI still loads/validates the rule set before
//!   constructing the gate).
//!
//! NOTE: mutation gates run only for a `ReadyToCallOperation` plan, after the
//! kernel has validated the fixed durable layout and retained one producer
//! boundary. The scaffold therefore carries the minimal cross-reference closure
//! for `execute-trivial-write.yaml`; the CLI intentionally omits `--effect` so a
//! clean audit cannot apply anything or append to the WAL.

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

/// Consumer project layout: `<parent>/app` is the project, `<parent>/forge-app`
/// is the sidecar carrying `<parent>/forge-app/.forge-method` as `state_root`.
/// Mirrors the layout used by `operation_sidecar_e2e.rs::consumer_fixture`,
/// but copies only the reference closure needed to produce one ready mutation
/// plan. The effect document is indexed but never supplied to the executor, so
/// the focused clean path cannot apply it.
struct ConsumerScaffold {
    app: PathBuf,
    state_root: PathBuf,
    /// Path (under `app`) of the copied ready operation contract.
    operation_path: PathBuf,
}

fn fresh_consumer(label: &str) -> ConsumerScaffold {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let parent = repo_root()
        .join("target")
        .join(format!("exec-op-risk-audit-e2e-{label}-{n}"));
    let _ = fs::remove_dir_all(&parent);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");
    fs::create_dir_all(&app).expect("create app dir");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\n\
         project_id: risk-audit-e2e\n\
         sidecar_root: ../forge-app\n\
         state_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    fs::write(app.join("README.md"), "# app\n").expect("write app readme");
    fs::write(
        state_root.join("state.yaml"),
        "schema_version: forge_project_state_v1\n\
         current_phase: \"4-build-verify\"\n\
         updated_at: null\n",
    )
    .expect("write authoritative phase");

    let operation_dir = app.join("docs/fixtures/operation-contract-v0");
    let effect_dir = app.join("contracts/effects");
    let claim_dir = app.join("contracts/claims");
    fs::create_dir_all(&operation_dir).expect("create operations dir");
    fs::create_dir_all(&effect_dir).expect("create effects dir");
    fs::create_dir_all(&claim_dir).expect("create claims dir");

    let operation_path = operation_dir.join("execute-trivial-write.yaml");
    fs::copy(
        repo_root().join("docs/fixtures/operation-contract-v0/execute-trivial-write.yaml"),
        &operation_path,
    )
    .expect("copy ready operation fixture");
    fs::copy(
        repo_root().join("contracts/effects/tool-effect-contract-v0.yaml"),
        effect_dir.join("tool-effect-contract-v0.yaml"),
    )
    .expect("copy effect contract definition");
    fs::copy(
        repo_root().join("contracts/effects/story-artifact-write-effect.yaml"),
        effect_dir.join("story-artifact-write-effect.yaml"),
    )
    .expect("copy effect fixture");
    fs::copy(
        repo_root().join("contracts/claims/story-v2-010-active-claim.yaml"),
        claim_dir.join("story-v2-010-active-claim.yaml"),
    )
    .expect("copy claim fixture");
    ConsumerScaffold {
        app,
        state_root,
        operation_path,
    }
}

fn fail_soft_rules() -> PathBuf {
    repo_root().join("contracts/risk-audits/fail-soft.yaml")
}

/// Write `src/lib.rs` under the project root so the risk-audit walker
/// (`collect_risk_audit_targets`) picks it up.
fn write_source(app: &Path, body: &str) {
    fs::create_dir_all(app.join("src")).expect("create src dir");
    fs::write(app.join("src/lib.rs"), body).expect("write src/lib.rs");
}

/// Run `execute-operation` against the scaffold with the given rules arg.
/// `rules_arg` controls whether `--require-risk-audit <path>` is appended.
/// The operation points at the scaffold's ready mutation fixture so the kernel
/// admits the retained producer boundary and evaluates the configured gate.
fn run_execute_operation(
    scaffold: &ConsumerScaffold,
    rules_arg: Option<&Path>,
) -> std::process::Output {
    let mut cmd = bin();
    cmd.args(["execute-operation", "--root"]).arg(&scaffold.app);
    cmd.args(["--operation"]).arg(&scaffold.operation_path);
    if let Some(rules) = rules_arg {
        cmd.args(["--require-risk-audit"]).arg(rules);
    }
    cmd.output().expect("run execute-operation")
}

#[test]
fn execute_operation_blocked_by_risk_audit() {
    let scaffold = fresh_consumer("blocked");
    write_source(
        &scaffold.app,
        "pub fn risky() -> u32 {\n    let x: Option<u32> = None;\n    x.unwrap()\n}\n",
    );
    let output = run_execute_operation(&scaffold, Some(&fail_soft_rules()));
    assert!(
        !output.status.success(),
        "gate must fail-closed when anti-patterns are present"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("risk-audit gate failed"),
        "stderr should report risk-audit gate failure, got: {stderr}"
    );
    // Fail-closed contract: nothing is written to the WAL.
    let wal = scaffold.state_root.join("wal/effects.ndjson");
    assert!(
        !wal.exists(),
        "WAL must not be written when the risk-audit gate fails"
    );
    // F11.4: the gate emits TraceEvents to the project trace log even on
    // failure, so `forge explain` can narrate the audit later.
    let trace = scaffold.state_root.join("traces/events.ndjson");
    assert!(
        trace.exists(),
        "trace log must be written so the audit is visible to forge explain"
    );
    let trace_body = fs::read_to_string(&trace).expect("read trace log");
    assert!(
        trace_body.contains("risk_audit_failed"),
        "trace should record risk_audit_failed, got: {trace_body}"
    );
    assert!(
        trace_body.contains("risk_audit_started"),
        "trace should record risk_audit_started, got: {trace_body}"
    );
}

#[test]
fn execute_operation_passes_when_risk_audit_clean() {
    let scaffold = fresh_consumer("clean");
    // Clean source: no anti-patterns matched by fail-soft.yaml.
    write_source(&scaffold.app, "pub fn answer() -> u32 {\n    42\n}\n");
    let output = run_execute_operation(&scaffold, Some(&fail_soft_rules()));
    // The ready plan passes the audit and then stops because this focused test
    // does not supply the declared effect input. Crucially the gate must NOT
    // have blocked: stderr must not mention a risk-audit failure.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("risk-audit gate failed"),
        "clean source must pass the gate; stderr: {stderr}"
    );
    // WAL is not written because no effect input was supplied.
    let wal = scaffold.state_root.join("wal/effects.ndjson");
    assert!(
        !wal.exists(),
        "WAL must not be written when the plan awaits a human"
    );
}

#[test]
fn execute_operation_without_flag_skips_audit() {
    let scaffold = fresh_consumer("skip");
    write_source(
        &scaffold.app,
        "pub fn risky() -> u32 {\n    let x: Option<u32> = None;\n    x.unwrap()\n}\n",
    );
    // No --require-risk-audit: the gate must not attach, even with anti-patterns.
    let output = run_execute_operation(&scaffold, None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("risk-audit gate failed"),
        "gate must be skipped without the flag; stderr: {stderr}"
    );
}

#[test]
fn execute_operation_risk_audit_invalid_rules_yaml_fails_clearly() {
    let scaffold = fresh_consumer("bad-rules");
    let bad_rules = scaffold.app.join("bad-rules.yaml");
    // Intentionally malformed YAML so `yaml_serde::from_str` rejects it.
    fs::write(
        &bad_rules,
        "schema_version: risk-audit-v0\nrules: [unclosed bracket\n",
    )
    .expect("write bad rules yaml");
    let output = run_execute_operation(&scaffold, Some(&bad_rules));
    assert!(
        !output.status.success(),
        "invalid rules YAML must surface as a failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("parse") && stderr.contains("bad-rules.yaml"),
        "stderr should report a parse error on the rules file, got: {stderr}"
    );
    let wal = scaffold.state_root.join("wal/effects.ndjson");
    assert!(
        !wal.exists(),
        "WAL must not be written when the rules YAML is invalid"
    );
}

#[test]
fn standalone_risk_audit_emits_trace_in_forge_project() {
    // The standalone CLI emits trace events only when the audited root
    // already carries a `.forge-method` dir (so it never pollutes a
    // non-Forge tree). Unlike the sidecar layout used by execute-operation,
    // the standalone resolves the trace root as `<root>/.forge-method`
    // directly, so this scaffold plants `.forge-method` under the audit root.
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root()
        .join("target")
        .join(format!("standalone-risk-audit-trace-{n}"));
    let _ = fs::remove_dir_all(&root);
    let state_root = root.join(".forge-method");
    fs::create_dir_all(&state_root).expect("create .forge-method");
    fs::write(root.join("README.md"), "# app\n").expect("write readme");
    write_source(
        &root,
        "pub fn risky() -> u32 {\n    let x: Option<u32> = None;\n    x.unwrap()\n}\n",
    );
    let output = bin()
        .args(["risk-audit", "--root"])
        .arg(&root)
        .args(["--rules"])
        .arg(fail_soft_rules())
        .arg("--json")
        .output()
        .expect("run risk-audit");
    assert!(
        !output.status.success(),
        "standalone must fail-closed on anti-patterns"
    );
    let trace = state_root.join("traces/events.ndjson");
    assert!(
        trace.exists(),
        "standalone trace must be written under .forge-method in a Forge project"
    );
    let trace_body = fs::read_to_string(&trace).expect("read trace log");
    assert!(
        trace_body.contains("risk_audit_failed"),
        "standalone trace should record risk_audit_failed, got: {trace_body}"
    );
    // Sanity: each line is a JSON object with an event_kind field.
    for line in trace_body.lines() {
        let parsed: Value = serde_json::from_str(line).expect("trace line is valid JSON");
        assert!(
            parsed.get("event_kind").is_some(),
            "trace event must carry event_kind"
        );
    }
}
