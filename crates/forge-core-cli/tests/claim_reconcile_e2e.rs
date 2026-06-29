use assert_cmd::Command;
use forge_core_store::claim_wal::{recover_claim_wal, ClaimWalOperation};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_800_000_000;

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

fn fresh_state(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root().join("target").join(format!(
        "claim-reconcile-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("claims-active")).expect("create claims dir");
    root
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok");
    json
}

fn acquire_short_claim(claims_arg: &str, scope_id: &str, path: &str, now_unix: i64) -> Value {
    let now_arg = now_unix.to_string();
    let output = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            claims_arg,
            "--scope",
            "story",
            "--id",
            scope_id,
            "--agent",
            "alice",
            "--path",
            path,
            "--ttl",
            "10",
            "--heartbeat-interval",
            "5",
            "--now-unix",
            &now_arg,
        ])
        .output()
        .expect("run acquire");
    assert_success(&output, "acquire")
}

fn reconcile_at(claims_arg: &str, now_unix: i64) -> Value {
    let now_arg = now_unix.to_string();
    let output = bin()
        .args([
            "claim",
            "reconcile",
            "--claims-dir",
            claims_arg,
            "--now-unix",
            &now_arg,
        ])
        .output()
        .expect("run reconcile");
    assert_success(&output, "reconcile")
}

fn status_at(claims_arg: &str, now_unix: i64) -> Value {
    let now_arg = now_unix.to_string();
    let output = bin()
        .args([
            "claim",
            "status",
            "--claims-dir",
            claims_arg,
            "--now-unix",
            &now_arg,
        ])
        .output()
        .expect("run status");
    assert_success(&output, "status")
}

#[test]
fn claim_reconcile_command_one_shot_materializes_stale_and_handoff_required() {
    let state_root = fresh_state("one-shot");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();

    acquire_short_claim(&claims_arg, "P23-CLI", "src/p23.rs", NOW);

    let stale_json = reconcile_at(&claims_arg, NOW + 5);
    assert_eq!(stale_json["command"], "claim.reconcile");
    assert_eq!(stale_json["data"]["changed"], 1);
    assert_eq!(stale_json["data"]["transitions"][0]["to"], "stale");

    let no_op_json = reconcile_at(&claims_arg, NOW + 6);
    assert_eq!(
        no_op_json["data"]["changed"], 0,
        "second reconcile before expiry must be idempotent"
    );

    let handoff_json = reconcile_at(&claims_arg, NOW + 10);
    assert_eq!(handoff_json["data"]["changed"], 1);
    assert_eq!(
        handoff_json["data"]["transitions"][0]["to"],
        "handoff_required"
    );

    let status_json = status_at(&claims_arg, NOW + 10);
    assert_eq!(
        status_json["data"]["expired_handoff_required"][0]["blocker_reason"],
        "handoff_required"
    );

    let recovery = recover_claim_wal(&state_root, false).expect("recover claim WAL");
    let operations: Vec<_> = recovery
        .records
        .iter()
        .map(|record| record.operation)
        .collect();
    assert_eq!(
        operations,
        vec![
            ClaimWalOperation::Acquire,
            ClaimWalOperation::ReconcileStatus,
            ClaimWalOperation::ReconcileStatus,
        ]
    );
}

#[test]
fn claim_reconcile_loop_is_bounded_and_emits_ndjson_ticks() {
    let state_root = fresh_state("loop");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();

    acquire_short_claim(&claims_arg, "P23-LOOP", "src/loop.rs", NOW - 10);

    let loop_output = bin()
        .args([
            "claim",
            "reconcile",
            "--claims-dir",
            &claims_arg,
            "--loop",
            "--interval-ms",
            "1",
            "--max-ticks",
            "1",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run reconcile loop");

    assert!(
        loop_output.status.success(),
        "bounded loop should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&loop_output.stdout),
        String::from_utf8_lossy(&loop_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&loop_output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "max-ticks=1 should emit one NDJSON line");
    let json: Value = serde_json::from_str(lines[0]).expect("loop line json");
    assert_eq!(json["command"], "claim.reconcile");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["changed"], 1);
    assert_eq!(json["data"]["transitions"][0]["to"], "handoff_required");
}
