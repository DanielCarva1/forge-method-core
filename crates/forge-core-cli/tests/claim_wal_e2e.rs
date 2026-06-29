use assert_cmd::Command;
use forge_core_store::claim_wal::{claim_wal_path, recover_claim_wal, ClaimWalOperation};
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
    let root = repo_root()
        .join("target")
        .join(format!("claim-wal-e2e-{label}-{}-{n}", std::process::id()));
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

#[test]
fn claim_acquire_heartbeat_release_append_claim_wal_records() {
    let state_root = fresh_state("lifecycle");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "CLAIM-WAL-LIFECYCLE",
            "--agent",
            "alice",
            "--path",
            "src/lib.rs",
            "--ttl",
            "600",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run acquire");
    let acquire_json = assert_success(&acquire, "acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let heartbeat = bin()
        .args([
            "claim",
            "heartbeat",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--now-unix",
            &(NOW + 60).to_string(),
        ])
        .output()
        .expect("run heartbeat");
    assert_success(&heartbeat, "heartbeat");

    let release = bin()
        .args([
            "claim",
            "release",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--now-unix",
            &(NOW + 120).to_string(),
        ])
        .output()
        .expect("run release");
    assert_success(&release, "release");

    let recovery = recover_claim_wal(&state_root, false).expect("recover claim WAL");
    let operations: Vec<ClaimWalOperation> = recovery
        .records
        .iter()
        .map(|record| record.operation)
        .collect();
    assert_eq!(
        operations,
        vec![
            ClaimWalOperation::Acquire,
            ClaimWalOperation::Heartbeat,
            ClaimWalOperation::Release,
        ]
    );
    assert!(claim_wal_path(&state_root).exists());
}

#[test]
fn claim_handoff_appends_handoff_record_after_artifact() {
    let state_root = fresh_state("handoff");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let evidence = state_root.join("handoff-evidence.txt");
    std::fs::write(&evidence, "handoff evidence").expect("write evidence");
    let evidence_arg = evidence.display().to_string();

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "CLAIM-WAL-HANDOFF",
            "--agent",
            "alice",
            "--path",
            "src/handoff.rs",
            "--ttl",
            "1",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run acquire");
    let acquire_json = assert_success(&acquire, "acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let handoff = bin()
        .args([
            "claim",
            "handoff",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--summary",
            "expired claim recovered",
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &(NOW + 2).to_string(),
        ])
        .output()
        .expect("run handoff");
    let handoff_json = assert_success(&handoff, "handoff");
    let handoff_path = handoff_json["data"]["handoff_path"]
        .as_str()
        .expect("handoff path");
    assert!(
        PathBuf::from(handoff_path).exists(),
        "handoff artifact should be written before command succeeds"
    );

    let recovery = recover_claim_wal(&state_root, false).expect("recover claim WAL");
    let operations: Vec<ClaimWalOperation> = recovery
        .records
        .iter()
        .map(|record| record.operation)
        .collect();
    assert_eq!(
        operations,
        vec![
            ClaimWalOperation::Acquire,
            ClaimWalOperation::HandoffRecorded,
        ]
    );
}
