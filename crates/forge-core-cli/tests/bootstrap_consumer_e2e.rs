//! Bootstrap proof: a fresh consumer repo can adopt Forge end-to-end.
//!
//! This is the "bootstrap proof" the README admits was missing. It drives the
//! REAL `forge-core` binary as a subprocess through the complete onboarding
//! lifecycle a brand-new consumer repo would follow:
//!
//!   1. fresh repo (git init + README, no Forge state)
//!   2. `start` diagnoses `no_link` and recommends `project init`
//!   3. `project init` creates the sibling sidecar
//!   4. `project resolve` confirms the sidecar layout
//!   5. `claim acquire` opens an active claim
//!   6. `claim check-write` authorizes the owner
//!   7. `claim check-write` blocks an intruder (`rejected_by_gate`)
//!   8. `claim release` releases the claim
//!   9. `validate` passes clean — the KEY assertion that was failing before the
//!      embedded-contracts fix: a consumer validates clean with no local
//!      `contracts/` tree because the shared definitions are served from the
//!      binary.
//!
//! The whole sequence is one test because it is a single temporal narrative —
//! splitting it would hide the dependency chain (init must precede acquire must
//! precede release). Each numbered step has its own explicit assertions and a
//! `label` so a failure points straight at the broken step.
//!
//! Mirrors the harness in `project_init_e2e.rs` / `claim_cli_sidecar_e2e.rs`:
//! `assert_cmd::Command::cargo_bin` for subprocesses, `serde_json::Value` for
//! parsing, and a repo-relative `FreshParent` under `target/` (NOT `tempfile` —
//! see DD46: `tempfile`'s `/tmp/...` paths get mangled under WSL→Windows
//! translation, which is exactly this build host) with `Drop` cleanup.

#![allow(clippy::too_many_lines)]
// The bootstrap lifecycle is a single end-to-end narrative; splitting it across
// functions would obscure the step dependency chain (init → acquire → release).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed epoch so `acquired_at/expires_at` in the JSON are deterministic.
const NOW: i64 = 1_800_000_000;

/// The consumer app directory name. Also the derived `project_id`, so the
/// sibling sidecar lands at `<parent>/forge-<APP_DIR>`.
const APP_DIR: &str = "bootstrap-app";

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

/// A fresh temp parent under `target/`, cleaned up on drop. Mirrors the
/// `project_init_e2e` harness (DD46: repo-relative, not `tempfile`).
struct FreshParent {
    path: PathBuf,
}

impl FreshParent {
    fn new(label: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let path = repo_root().join("target").join(format!(
            "bootstrap-consumer-e2e-{label}-{}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create fresh parent");
        Self { path }
    }
}

impl Drop for FreshParent {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Parse stdout as a JSON value, panicking with both streams on failure.
fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be JSON: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// Assert the subprocess exited zero and the envelope reports `ok: true`;
/// return the parsed envelope.
fn assert_ok(output: &Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok: {json:#}");
    json
}

/// Assert the subprocess exited non-zero and the envelope reports `ok: false`;
/// return the parsed envelope.
fn assert_rejected(output: &Output, label: &str) -> Value {
    assert!(
        !output.status.success(),
        "{label} should fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], false, "{label} should report not ok: {json:#}");
    json
}

/// Total error count across every check in a `validate` summary. `validate`
/// emits the raw `ValidateSummary` (not a `CliEnvelope`), so there is no
/// top-level `errors` field — errors live per-check under `checks[].errors`.
fn total_errors(summary: &Value) -> i64 {
    summary["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|check| check["errors"].as_i64().unwrap_or(0))
        .sum()
}

#[test]
fn fresh_consumer_repo_bootstraps_and_validates_clean_end_to_end() {
    let parent = FreshParent::new("full-lifecycle");
    let app = parent.path.join(APP_DIR);
    let app_arg = app.display().to_string();
    let sidecar_state_root = parent
        .path
        .join(format!("forge-{APP_DIR}"))
        .join(".forge-method");

    // ── Step 1: fresh consumer repo ───────────────────────────────────────
    // A real consumer starts with a git repo and a README, nothing Forge.
    fs::create_dir_all(&app).expect("create app root");
    fs::write(app.join("README.md"), "# bootstrap-app\n").expect("write README");
    let git_ok = std::process::Command::new("git")
        .arg("init")
        .arg(&app)
        .output()
        .is_ok_and(|o| o.status.success());
    // `git init` is best-effort: the proof must still hold on minimal CI
    // images that don't ship git (the bootstrap logic keys off Forge state,
    // not git metadata).
    if git_ok {
        assert!(
            app.join(".git").is_dir(),
            "git init should create a .git dir"
        );
    }
    assert!(
        !app.join(".forge-method.yaml").exists(),
        "fresh repo must have no Forge Project Link"
    );
    assert!(
        !app.join("contracts").exists(),
        "fresh repo must have no contracts/ tree"
    );

    // ── Step 2: start bootstraps the project in one command ───────────────
    // `start` on a fresh repo now creates the Project Link + sidecar (rather
    // than recommending a separate `project init`), so the agent gets a ready
    // project in a single command.
    let start = bin()
        .args(["start", "--root", &app_arg, "--json"])
        .output()
        .expect("run forge-core start");
    let start_json = assert_ok(&start, "start on fresh repo");
    assert_eq!(
        start_json["data"]["state"], "sidecar_ready_no_contract",
        "start should bootstrap and advance to sidecar_ready_no_contract"
    );
    assert_eq!(
        start_json["data"]["actions_performed"],
        serde_json::json!(["initialized"]),
        "start should report it initialized the project"
    );
    assert!(
        app.join(".forge-method.yaml").is_file(),
        "start should create the Project Link in the consumer"
    );
    assert!(
        sidecar_state_root.is_dir(),
        "start should create the sibling sidecar state root on disk"
    );
    // The consumer must stay clean: no local `.forge-method` dir.
    assert!(
        !app.join(".forge-method").exists(),
        "consumer must not carry local .forge-method state"
    );

    // ── Step 3: project init is idempotent (already_initialized) ──────────
    let init = bin()
        .args(["project", "init", "--root", &app_arg, "--json"])
        .output()
        .expect("run forge-core project init");
    let init_json = assert_ok(&init, "project init after start");
    assert_eq!(
        init_json["data"]["status"], "already_initialized",
        "project init after start should report already_initialized"
    );

    // ── Step 4: project resolve confirms sidecar layout ───────────────────
    let resolve = bin()
        .args(["project", "resolve", "--root", &app_arg, "--json"])
        .output()
        .expect("run forge-core project resolve");
    let resolve_json = assert_ok(&resolve, "project resolve");
    assert_eq!(
        resolve_json["data"]["layout"], "sidecar",
        "resolve should report the sidecar layout"
    );

    // ── Step 5: claim acquire opens an active claim ───────────────────────
    let scope_id = "bootstrap-feature";
    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app_arg,
            "--scope",
            "story",
            "--id",
            scope_id,
            "--agent",
            "test-worker",
            "--path",
            "src/main.rs",
            "--now-unix",
            &NOW.to_string(),
            "--json",
        ])
        .output()
        .expect("run claim acquire");
    let acquire_json = assert_ok(&acquire, "claim acquire");
    assert_eq!(
        acquire_json["data"]["status"], "active",
        "acquired claim should be active"
    );
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim_id in acquire response")
        .to_string();
    let expected_claim_id = format!("claim.story.{scope_id}.{scope_id}");
    assert_eq!(
        claim_id, expected_claim_id,
        "claim id should follow the claim.<scope>.<id>.<id> pattern"
    );

    // ── Step 6: check-write authorizes the owner ─────────────────────────
    let owner_check = bin()
        .args([
            "claim",
            "check-write",
            "--root",
            &app_arg,
            "--agent",
            "test-worker",
            "--target",
            "src/main.rs",
            "--now-unix",
            &(NOW + 1).to_string(),
            "--json",
        ])
        .output()
        .expect("run check-write as owner");
    let owner_json = assert_ok(&owner_check, "check-write (owner)");
    assert_eq!(
        owner_json["data"]["allowed"], true,
        "owner writing its own claimed path must be allowed"
    );

    // ── Step 7: check-write blocks an intruder ───────────────────────────
    let intruder_check = bin()
        .args([
            "claim",
            "check-write",
            "--root",
            &app_arg,
            "--agent",
            "intruder",
            "--target",
            "src/main.rs",
            "--now-unix",
            &(NOW + 2).to_string(),
            "--json",
        ])
        .output()
        .expect("run check-write as intruder");
    let intruder_json = assert_rejected(&intruder_check, "check-write (intruder)");
    assert_eq!(
        intruder_json["exit_reason"], "rejected_by_gate",
        "intruder must be rejected by the write gate"
    );
    assert_eq!(
        intruder_json["data"]["allowed"], false,
        "intruder must not be allowed to write"
    );

    // ── Step 8: claim release ─────────────────────────────────────────────
    let release = bin()
        .args([
            "claim",
            "release",
            "--root",
            &app_arg,
            "--id",
            &claim_id,
            "--agent",
            "test-worker",
            "--now-unix",
            &(NOW + 3).to_string(),
            "--json",
        ])
        .output()
        .expect("run claim release");
    let release_json = assert_ok(&release, "claim release");
    assert_eq!(
        release_json["data"]["status"], "released",
        "released claim should be marked released"
    );

    // ── Step 9: validate passes clean ─────────────────────────────────────
    // THE key assertion. Before the embedded-contracts fix this failed for any
    // consumer without a local contracts/ tree, because validate could not
    // resolve the shared definitions. The fix serves them from the binary, so
    // a fresh consumer now validates clean. `validate --json` emits the raw
    // ValidateSummary (not a CliEnvelope): { status, root, checks, diagnostics }.
    let validate = bin()
        .args(["validate", "--root", &app_arg, "--json"])
        .output()
        .expect("run forge-core validate");
    assert!(
        validate.status.success(),
        "validate should pass clean\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validate.stdout),
        String::from_utf8_lossy(&validate.stderr)
    );
    let validate_json = output_json(&validate);
    assert_eq!(
        validate_json["status"], "passed",
        "validate should report status=passed: {validate_json:#}"
    );
    assert_eq!(
        total_errors(&validate_json),
        0,
        "validate should report zero errors across all checks: {validate_json:#}"
    );
    assert!(
        validate_json["diagnostics"]
            .as_array()
            .is_some_and(std::vec::Vec::is_empty),
        "validate should emit no diagnostics: {validate_json:#}"
    );
}
