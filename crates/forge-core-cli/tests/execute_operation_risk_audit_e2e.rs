//! F11.3 Risk Audit Gate enforcement in `execute-operation` — E2E.
//!
//! These tests exercise the `--require-risk-audit <path>` flag added by F11.3.
//! The gate runs BEFORE any contract parse or WAL write, so:
//!
//! - `blocked`: anti-pattern in source + flag → fail-closed, WAL untouched.
//! - `passes_when_clean`: clean source + flag → gate passes; the operation
//!   still fails afterwards (no real operation contract) but NOT with a
//!   risk-audit error, proving the gate did not block.
//! - `without_flag_skips_audit`: anti-pattern in source, NO flag → gate does
//!   not run; same downstream failure, no risk-audit error.
//! - `invalid_rules_yaml`: malformed rules YAML + flag → `ParseYaml` error
//!   propagated clearly.

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
/// kept minimal (no operation/command/effect contracts copied).
struct ConsumerScaffold {
    app: PathBuf,
    state_root: PathBuf,
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
    ConsumerScaffold { app, state_root }
}

fn fail_soft_rules() -> PathBuf {
    repo_root().join("contracts/risk-audits/fail-soft.yaml")
}

/// Write `src/lib.rs` under the project root so the risk-audit walker
/// (`collect_targets`) picks it up.
fn write_source(app: &Path, body: &str) {
    fs::create_dir_all(app.join("src")).expect("create src dir");
    fs::write(app.join("src/lib.rs"), body).expect("write src/lib.rs");
}

/// Run `execute-operation` against the scaffold with the given rules arg.
/// `rules_arg` controls whether `--require-risk-audit <path>` is appended.
fn run_execute_operation(app: &Path, rules_arg: Option<&Path>) -> std::process::Output {
    let mut cmd = bin();
    cmd.args(["execute-operation", "--root"]).arg(app);
    // Operation path points to a non-existent file under root. The gate
    // (when it runs) executes before this path is read, so blocked runs never
    // reach the read; clean runs fail here with a read error AFTER the gate.
    cmd.args(["--operation"])
        .arg(app.join("contracts/no-such-op.yaml"));
    if let Some(rules) = rules_arg {
        cmd.args(["--require-risk-audit"]).arg(rules);
    }
    cmd.output().expect("run execute-operation")
}

#[test]
fn execute_operation_blocked_by_risk_audit() {
    let ConsumerScaffold { app, state_root } = fresh_consumer("blocked");
    write_source(
        &app,
        "pub fn risky() -> u32 {\n    let x: Option<u32> = None;\n    x.unwrap()\n}\n",
    );
    let output = run_execute_operation(&app, Some(&fail_soft_rules()));
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
    let wal = state_root.join("wal/effects.ndjson");
    assert!(
        !wal.exists(),
        "WAL must not be written when the risk-audit gate fails"
    );
    // F11.4: the gate emits TraceEvents to the project trace log even on
    // failure, so `forge explain` can narrate the audit later.
    let trace = state_root.join("traces/events.ndjson");
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
    let ConsumerScaffold { app, state_root } = fresh_consumer("clean");
    // Clean source: no anti-patterns matched by fail-soft.yaml.
    write_source(&app, "pub fn answer() -> u32 {\n    42\n}\n");
    let output = run_execute_operation(&app, Some(&fail_soft_rules()));
    // Downstream parse fails because `no-such-op.yaml` does not exist, but the
    // gate must NOT have blocked: stderr must not mention risk-audit.
    assert!(
        !output.status.success(),
        "downstream read should still fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("risk-audit gate failed"),
        "clean source must pass the gate; stderr: {stderr}"
    );
    // WAL is still not written because the operation never reached the runtime.
    let wal = state_root.join("wal/effects.ndjson");
    assert!(
        !wal.exists(),
        "WAL must not be written on downstream failure"
    );
}

#[test]
fn execute_operation_without_flag_skips_audit() {
    let ConsumerScaffold { app, state_root: _ } = fresh_consumer("skip");
    write_source(
        &app,
        "pub fn risky() -> u32 {\n    let x: Option<u32> = None;\n    x.unwrap()\n}\n",
    );
    // No --require-risk-audit: the gate must not run, even with anti-patterns.
    let output = run_execute_operation(&app, None);
    assert!(
        !output.status.success(),
        "downstream read should still fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("risk-audit gate failed"),
        "gate must be skipped without the flag; stderr: {stderr}"
    );
}

#[test]
fn execute_operation_risk_audit_invalid_rules_yaml_fails_clearly() {
    let ConsumerScaffold { app, state_root } = fresh_consumer("bad-rules");
    let bad_rules = app.join("bad-rules.yaml");
    // Intentionally malformed YAML so `yaml_serde::from_str` rejects it.
    fs::write(
        &bad_rules,
        "schema_version: risk-audit-v0\nrules: [unclosed bracket\n",
    )
    .expect("write bad rules yaml");
    let output = run_execute_operation(&app, Some(&bad_rules));
    assert!(
        !output.status.success(),
        "invalid rules YAML must surface as a failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("parse") && stderr.contains("bad-rules.yaml"),
        "stderr should report a parse error on the rules file, got: {stderr}"
    );
    let wal = state_root.join("wal/effects.ndjson");
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
