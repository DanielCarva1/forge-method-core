//! F07.7 — end-to-end CLI test for `forge-core governance`.
//!
//! Drives the REAL `forge-core` binary through the arbitration lifecycle
//! (record → conflicts[pending] → arbitrate → conflicts[resolved]) using the
//! permanent F07 fixtures in `contracts/examples/`. This is the E2E acceptance
//! test the F07 spec names: "2 principals disputing the same ref → `ConflictContract`
//! emitted → manual resolution → ledger updated".
//!
//! No mocks: the binary writes to a fresh `--governance-dir`, the PEP appends to
//! a real JSONL log, the projection replays it, and assertions check the wire
//! envelope the way an agent would consume it. Mirrors `memory_cli_e2e.rs`.

#![allow(clippy::too_many_lines)]
#![allow(clippy::doc_markdown)]

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
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

fn example(name: &str) -> PathBuf {
    repo_root().join("contracts").join("examples").join(name)
}

/// Fresh per-test governance dir under target/ (auto-cleaned; matches the
/// memory_cli_e2e fresh_dir convention).
fn fresh_governance_dir(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = repo_root().join("target").join(format!(
        "governance-cli-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fresh governance dir");
    dir
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// The canonical F07 E2E: record → conflicts(pending) → arbitrate →
/// conflicts(resolved). Each step asserts the wire envelope; the arbitrate
/// step proves "manual resolution → ledger updated" (the F07.7 acceptance
/// criterion).
#[test]
fn governance_lifecycle_record_conflicts_arbitrate() {
    let dir = fresh_governance_dir("lifecycle");
    let conflict = example("conflict-contract.yaml");
    let policy = example("governance-policy.yaml");

    // 1. record the conflict → sequence 1, resolution pending.
    let out = bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("run record");
    assert!(
        out.status.success(),
        "record should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "governance record");
    assert_eq!(json["data"]["conflict_id"], "conflict.alice-bob.stories");
    assert_eq!(json["data"]["sequence"], 1);

    // 2. conflicts (no filter) → one conflict, pending.
    let out = bin()
        .args(["governance", "conflicts", "--governance-dir"])
        .arg(&dir)
        .output()
        .expect("run conflicts");
    assert!(out.status.success(), "conflicts should pass");
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "governance conflicts");
    assert_eq!(json["data"]["count"], 1);
    assert_eq!(
        json["data"]["conflicts"][0]["conflict_id"],
        "conflict.alice-bob.stories"
    );
    assert_eq!(json["data"]["conflicts"][0]["resolution"], "pending");

    // 3. conflicts --status pending → still one; --status resolved → zero.
    let out = bin()
        .args([
            "governance",
            "conflicts",
            "--status",
            "pending",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run conflicts pending");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 1, "one pending conflict");
    let out = bin()
        .args([
            "governance",
            "conflicts",
            "--status",
            "resolved",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run conflicts resolved");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 0, "no resolved conflicts yet");

    // 4. arbitrate: principal.daniel (authorized reviewer) awards to alice.
    let out = bin()
        .args([
            "governance",
            "arbitrate",
            "--conflict-id",
            "conflict.alice-bob.stories",
            "--policy-file",
        ])
        .arg(&policy)
        .args([
            "--arbiter",
            "principal.daniel",
            "--awarded-to",
            "principal.alice",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run arbitrate");
    assert!(
        out.status.success(),
        "arbitrate should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "governance arbitrate");
    assert_eq!(json["data"]["conflict_id"], "conflict.alice-bob.stories");
    assert_eq!(json["data"]["sequence"], 2, "record=1, arbitrate=2");

    // 5. conflicts (no filter) → still one conflict, now resolved.
    let out = bin()
        .args(["governance", "conflicts", "--governance-dir"])
        .arg(&dir)
        .output()
        .expect("run conflicts after arbitrate");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 1);
    assert_eq!(
        json["data"]["conflicts"][0]["resolution"], "resolved",
        "resolution must be updated after arbitrate"
    );
    // And the --status resolved filter now finds it.
    let out = bin()
        .args([
            "governance",
            "conflicts",
            "--status",
            "resolved",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run conflicts resolved after arbitrate");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 1, "now one resolved conflict");
}

/// record is idempotent on conflict_id: a second record of the same conflict
/// is AlreadyRecorded (sequence 0, no new event).
#[test]
fn governance_record_is_idempotent() {
    let dir = fresh_governance_dir("idempotent");
    let conflict = example("conflict-contract.yaml");

    let out = bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("run record 1");
    assert!(out.status.success());
    let json = output_json(&out);
    assert_eq!(json["data"]["sequence"], 1);

    let out = bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("run record 2");
    assert!(out.status.success(), "second record should still exit 0");
    let json = output_json(&out);
    assert_eq!(
        json["data"]["sequence"], 0,
        "AlreadyRecorded returns sequence 0 (no event appended)"
    );
}

/// An unauthorized arbiter is denied by the gate (non-zero exit, envelope ok=false).
#[test]
fn governance_arbitrate_unauthorized_is_denied() {
    let dir = fresh_governance_dir("denied");
    let conflict = example("conflict-contract.yaml");
    let policy = example("governance-policy.yaml");

    // record first.
    let out = bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("run record");
    assert!(out.status.success());

    // principal.eve is NOT in authorized_reviewers → denied.
    let out = bin()
        .args([
            "governance",
            "arbitrate",
            "--conflict-id",
            "conflict.alice-bob.stories",
            "--policy-file",
        ])
        .arg(&policy)
        .args([
            "--arbiter",
            "principal.eve",
            "--both-released",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run arbitrate denied");
    assert!(
        !out.status.success(),
        "unauthorized arbitrate must fail (non-zero exit)"
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "rejected_by_gate");
}

/// escalate moves Pending→Escalated for an authorized principal.
#[test]
fn governance_escalate_transitions_to_escalated() {
    let dir = fresh_governance_dir("escalate");
    let conflict = example("conflict-contract.yaml");
    let policy = example("governance-policy.yaml");

    // record.
    let out = bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("run record");
    assert!(out.status.success());

    // escalate by principal.daniel (authorized).
    let out = bin()
        .args([
            "governance",
            "escalate",
            "--conflict-id",
            "conflict.alice-bob.stories",
            "--policy-file",
        ])
        .arg(&policy)
        .args(["--principal", "principal.daniel", "--governance-dir"])
        .arg(&dir)
        .output()
        .expect("run escalate");
    assert!(out.status.success());
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "governance escalate");
    assert_eq!(json["data"]["sequence"], 2);

    // conflicts --status escalated finds it.
    let out = bin()
        .args([
            "governance",
            "conflicts",
            "--status",
            "escalated",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run conflicts escalated");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 1);
}

/// A double-resolve is barred: arbitrating an already-resolved conflict returns
/// the NotPending error (non-zero exit).
#[test]
fn governance_double_arbitrate_is_not_pending() {
    let dir = fresh_governance_dir("double");
    let conflict = example("conflict-contract.yaml");
    let policy = example("governance-policy.yaml");

    // record + arbitrate once.
    bin()
        .args(["governance", "record", "--conflict-file"])
        .arg(&conflict)
        .arg("--governance-dir")
        .arg(&dir)
        .output()
        .expect("record");
    bin()
        .args([
            "governance",
            "arbitrate",
            "--conflict-id",
            "conflict.alice-bob.stories",
            "--policy-file",
        ])
        .arg(&policy)
        .args([
            "--arbiter",
            "principal.daniel",
            "--both-released",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("first arbitrate");

    // second arbitrate → NotPending.
    let out = bin()
        .args([
            "governance",
            "arbitrate",
            "--conflict-id",
            "conflict.alice-bob.stories",
            "--policy-file",
        ])
        .arg(&policy)
        .args([
            "--arbiter",
            "principal.daniel",
            "--split-scope",
            "--governance-dir",
        ])
        .arg(&dir)
        .output()
        .expect("second arbitrate");
    assert!(
        !out.status.success(),
        "double-resolve must fail (non-zero exit)"
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
}
