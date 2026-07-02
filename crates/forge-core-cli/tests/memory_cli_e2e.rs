//! F06.8 — end-to-end CLI test for `forge-core memory`.
//!
//! Drives the REAL `forge-core` binary through the full memory lifecycle
//! (ingest → list → promote → list → forget → list) using the permanent
//! fixtures in `contracts/examples/`. This is the E2E acceptance test the F06
//! spec names: "ingest → list → promote → list (autoridade muda)".
//!
//! No mocks: the binary writes to a fresh `--memory-dir`, the PEP appends to a
//! real JSONL log, the projection replays it, and the assertions check the
//! wire envelope the way an agent would consume it. Mirrors
//! `autonomy_route_e2e.rs` structure.

// E2E test files run the real binary through long lifecycle scenarios; the
// doc-comment / line-count pedantic lints are noise here, not signal (helpers
// reference unquoted identifiers like forge-core in doc-comments, and the
// lifecycle test is a single linear scenario by design). Matches the
// governance_cli_e2e.rs convention.
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

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

/// Fresh per-test memory dir under target/ (auto-cleaned; matches the
/// autonomy_route_e2e fresh_dir convention).
fn fresh_memory_dir(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = repo_root()
        .join("target")
        .join(format!("memory-cli-e2e-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fresh memory dir");
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

/// The canonical F06 E2E: ingest → list → promote → list → forget → list.
/// Each step asserts the wire envelope; the promote step proves "autoridade
/// muda" (the F06.8 acceptance criterion) and that review STAYS unreviewed.
#[test]
fn memory_lifecycle_ingest_list_promote_forget() {
    let dir = fresh_memory_dir("lifecycle");
    let policy = example("memory-policy.yaml");
    let admitted = example("memory-entry-admitted.yaml");
    let promoted = example("memory-entry-promoted.yaml");

    // 1. ingest the "admitted" entry → sequence 1, at the trust floor.
    let out = bin()
        .args(["memory", "ingest", "--entry-file"])
        .arg(&admitted)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run ingest");
    assert!(
        out.status.success(),
        "ingest should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "memory ingest");
    assert_eq!(json["data"]["entry_id"], "memory.entry.admitted");
    assert_eq!(json["data"]["sequence"], 1);

    // 2. ingest the "promoted" entry too → sequence 2.
    bin()
        .args(["memory", "ingest", "--entry-file"])
        .arg(&promoted)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run ingest 2");
    // 3. list → both entries present at authority=raw, review=unreviewed.
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .output()
        .expect("run list");
    assert!(out.status.success(), "list should pass");
    let json = output_json(&out);
    let entries = json["data"]["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 2, "both entries present");
    for entry in entries {
        assert_eq!(entry["authority"], "raw", "admitted entries start at raw");
        assert_eq!(entry["review"], "unreviewed");
    }

    // 4. promote memory.entry.promoted with raw evidence.
    let out = bin()
        .args(["memory", "promote", "--entry-id", "memory.entry.promoted"])
        .arg("--policy-file")
        .arg(&policy)
        .args(["--evidence", "run.f06-promote"])
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run promote");
    assert!(out.status.success(), "promote should pass");
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["before"], "raw");
    assert_eq!(json["data"]["after"], "authority");
    assert_eq!(json["data"]["sequence"], 3);

    // 5. list again → memory.entry.promoted is now authority; the other stays
    //    raw. Review must remain unreviewed for BOTH (the orthogonality NFR).
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .output()
        .expect("run list after promote");
    let json = output_json(&out);
    let entries = json["data"]["entries"].as_array().expect("entries array");
    let by_id: std::collections::HashMap<&str, &Value> = entries
        .iter()
        .map(|e| (e["entry_id"].as_str().expect("id"), e))
        .collect();
    assert_eq!(
        by_id["memory.entry.promoted"]["authority"], "authority",
        "promoted entry must reflect the new authority"
    );
    assert_eq!(
        by_id["memory.entry.promoted"]["review"], "unreviewed",
        "promote must not touch the review axis"
    );
    assert_eq!(
        by_id["memory.entry.admitted"]["authority"], "raw",
        "the non-promoted entry is unaffected"
    );

    // 6. forget memory.entry.promoted.
    let out = bin()
        .args(["memory", "forget", "--entry-id", "memory.entry.promoted"])
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run forget");
    assert!(out.status.success(), "forget should pass");
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["sequence"], 4);

    // 7. list → only memory.entry.admitted remains.
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .output()
        .expect("run list after forget");
    let json = output_json(&out);
    let entries = json["data"]["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 1, "forgotten entry is gone");
    assert_eq!(entries[0]["entry_id"], "memory.entry.admitted");
}

/// The admission gate denies an entry missing required evidence and appends
/// NOTHING to the log (the PEP never writes on a denial).
#[test]
fn memory_ingest_rejected_by_gate_appends_nothing() {
    let dir = fresh_memory_dir("reject");
    let policy = example("memory-policy.yaml");
    let rejected = example("memory-entry-rejected.yaml");

    let out = bin()
        .args(["memory", "ingest", "--entry-file"])
        .arg(&rejected)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run ingest rejected");

    // RejectedByGate ⇒ exit code 2.
    assert_eq!(out.status.code(), Some(2), "denied admission exits 2");
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "rejected_by_gate");
    let reasons = json["data"].as_array().expect("reasons in data");
    assert!(
        reasons.iter().any(|r| r
            .as_str()
            .is_some_and(|s| s.contains("missing_required_evidence"))),
        "denial reasons must include missing_required_evidence: {reasons:?}"
    );

    // The log must not exist — a denial appends nothing.
    let log = dir.join("events.ndjson");
    assert!(!log.exists(), "denied admission must not create the log");

    // And a subsequent list reports zero entries.
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .output()
        .expect("run list");
    let json = output_json(&out);
    assert_eq!(
        json["data"]["entries"].as_array().expect("entries").len(),
        0
    );
}

/// The lazy TTL sweep: an expired entry is flipped stale and excluded on read.
#[test]
fn memory_list_sweeps_expired_entry_lazily() {
    let dir = fresh_memory_dir("ttl");
    let policy = example("memory-policy.yaml");
    let expired = example("memory-entry-expired.yaml");

    // Ingest the expired-shape entry (captured_at 1780000000, ttl 3600).
    let out = bin()
        .args(["memory", "ingest", "--entry-file"])
        .arg(&expired)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run ingest");
    assert!(out.status.success(), "ingest should pass");

    // List BEFORE expiry → entry is live.
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .output()
        .expect("run list before expiry");
    let json = output_json(&out);
    assert_eq!(
        json["data"]["entries"].as_array().expect("entries").len(),
        1
    );
    assert_eq!(json["data"]["flipped"], 0);

    // List AFTER expiry (now = 1780003601, past 1780003600) → flipped, excluded.
    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780003601")
        .output()
        .expect("run list after expiry");
    let json = output_json(&out);
    assert_eq!(json["data"]["flipped"], 1, "the expired entry was swept");
    assert_eq!(
        json["data"]["entries"].as_array().expect("entries").len(),
        0
    );
}

/// `memory review` is deferred and emits a clear envelope (never silently
/// succeeds). An agent consuming the JSON sees `ok: false`.
#[test]
fn memory_review_is_deferred_and_reports_clearly() {
    let dir = fresh_memory_dir("review-deferred");
    let out = bin()
        .args(["memory", "review", "--memory-dir"])
        .arg(&dir)
        .output()
        .expect("run review");
    assert!(!out.status.success(), "deferred review must not succeed");
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "memory review");
    assert!(
        json["error"]["message"]
            .as_str()
            .expect("message")
            .contains("deferred"),
        "message must explain the deferral: {}",
        json["error"]["message"]
    );
}

/// `--no-json` produces human-readable text output (the dual-output NFR).
#[test]
fn memory_list_text_mode_is_human_readable() {
    let dir = fresh_memory_dir("text");
    let policy = example("memory-policy.yaml");
    let admitted = example("memory-entry-admitted.yaml");
    bin()
        .args(["memory", "ingest", "--entry-file"])
        .arg(&admitted)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--memory-dir")
        .arg(&dir)
        .output()
        .expect("run ingest");

    let out = bin()
        .args(["memory", "list", "--memory-dir"])
        .arg(&dir)
        .arg("--now-unix")
        .arg("1780000000")
        .arg("--no-json")
        .output()
        .expect("run list text");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("memory list") && stdout.contains("ok"),
        "text output should be a human one-liner: {stdout}"
    );
}

/// Unknown subcommand → structured usage error (exit 3).
#[test]
fn memory_unknown_subcommand_returns_usage_error() {
    let out = bin()
        .args(["memory", "frobnicate"])
        .output()
        .expect("run unknown subcommand");
    assert_eq!(out.status.code(), Some(3));
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "invalid_decision_shape");
    assert!(json["error"]["message"]
        .as_str()
        .expect("message")
        .contains("unknown subcommand 'frobnicate'"));
}
