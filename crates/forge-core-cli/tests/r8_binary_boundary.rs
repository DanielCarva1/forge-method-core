//! Binary-boundary test pinning R8 (slice-6 Frente A).
//!
//! This is Defense #1 from `contracts/research/rust-testing-defenses-v1.yaml`
//! (F4, F7): the single highest-ROI catch because it runs the REAL compiled
//! binary via `CARGO_BIN_EXE_` with operator-shaped argv.
//!
//! R8 (slice-5 live demo): `release`/`heartbeat` resolved `--id` only against
//! the full derived claim id, so real CLI usage with the operator-typed scope id
//! returned "claim not found". Every unit/integration test passed because they
//! extracted the canonical id from acquire and fed it back (circular oracle).
//! Only a 2h live demo exposed it.
//!
//! This test reproduces the EXACT failure mode at the binary boundary: acquire
//! by scope id argv, then release/heartbeat by the SAME scope id argv against
//! the real `forge-core` exe. If R8 ever regresses, this fails — and unlike the
//! property test (`r8_property.rs`), it exercises the argv parsing + process
//! spawn layer, so a future bug in flag handling is caught too.

use assert_cmd::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed epoch so `acquired_at`/`expires_at` in the JSON are deterministic.
const NOW: &str = "1800000000";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

/// A fresh, unique, repo-relative claims bus dir under `target/`.
///
/// We use relative paths (not `tempfile`'s `/tmp/...`) because forge-core is a
/// Windows binary under WSL and `/mnt/c`-style or `/tmp` paths get mangled
/// (DD46). A repo-relative path resolves against the child's CWD (the package
/// dir, set by `assert_cmd`) and survives WSL→Windows path translation.
fn fresh_bus(label: &str) -> String {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let p = format!("target/r8-bin-bus-{label}-{n}");
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Read the `data.claim_id` field out of a JSON envelope.
fn claim_id_from(stdout: &[u8]) -> String {
    let v: serde_json::Value = serde_json::from_slice(stdout).expect("valid JSON envelope");
    v["data"]["claim_id"]
        .as_str()
        .expect("data.claim_id present")
        .to_string()
}

/// Acquire a claim by scope id argv; returns `(bus_dir, scope_id, canonical_id)`.
fn acquire(label: &str, scope_id: &str) -> (String, String, String) {
    let bus = fresh_bus(label);
    let output = bin()
        .args([
            "claim",
            "acquire",
            "--scope",
            "lane",
            "--id",
            scope_id,
            "--agent",
            "agentA",
            "--claims-dir",
            &bus,
            "--now-unix",
            NOW,
        ])
        .unwrap();
    assert!(
        output.status.success(),
        "acquire must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let full = claim_id_from(&output.stdout);
    assert_ne!(
        full, scope_id,
        "R8 setup guard: canonical id must differ from scope id"
    );
    (bus, scope_id.to_string(), full)
}

#[test]
fn release_by_scope_id_argv_on_real_binary() {
    // THE R8 PIN. The operator acquires with `--id s1` and releases with the
    // SAME `--id s1` scope id. Before R8's fix this returned exit 3
    // ("claim not found"); now it must succeed.
    let (bus, scope, _full) = acquire("release", "s1");
    let output = bin()
        .args([
            "claim",
            "release",
            "--id",
            &scope,
            "--agent",
            "agentA",
            "--claims-dir",
            &bus,
        ])
        .unwrap();
    assert!(
        output.status.success(),
        "release by scope id '{}' must succeed (R8): {}",
        scope,
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["data"]["status"], "released");
}

#[test]
fn heartbeat_by_scope_id_argv_on_real_binary() {
    let (bus, scope, _full) = acquire("hb", "h1");
    let output = bin()
        .args([
            "claim",
            "heartbeat",
            "--id",
            &scope,
            "--agent",
            "agentA",
            "--claims-dir",
            &bus,
        ])
        .unwrap();
    assert!(
        output.status.success(),
        "heartbeat by scope id must succeed (R8): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["data"]["status"], "active");
}

#[test]
fn release_by_full_canonical_id_argv_still_works() {
    // Backwards-compat: a consumer holding the canonical id must still resolve.
    let (bus, _scope, full) = acquire("full", "c1");
    let output = bin()
        .args([
            "claim",
            "release",
            "--id",
            &full,
            "--agent",
            "agentA",
            "--claims-dir",
            &bus,
        ])
        .unwrap();
    assert!(
        output.status.success(),
        "release by full canonical id must still work: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn release_nonexistent_scope_returns_nonzero() {
    // Negative: a scope id with no claim must fail, not silently succeed. Pins
    // that the lookup is genuine, not a no-op.
    let bus = fresh_bus("neg");
    let result = bin()
        .args([
            "claim",
            "release",
            "--id",
            "does-not-exist",
            "--agent",
            "agentA",
            "--claims-dir",
            &bus,
        ])
        .ok();
    assert!(
        result.is_err(),
        "release of unknown scope must fail (non-zero exit), not silently succeed"
    );
}
