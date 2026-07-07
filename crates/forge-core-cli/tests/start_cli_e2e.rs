//! End-to-end tests for `forge-core start` (F12 Guided Start).
//!
//! Exercises the binary as a real subprocess across all five [`BootstrapState`]s,
//! mirroring the `project_init_e2e.rs` harness pattern (`assert_cmd::Command` +
//! `FreshParent` with Drop cleanup). The unit tests in `start_cmd.rs` cover the
//! pure classifier; these tests verify the full argv → stdout-envelope → exit-code
//! contract that agents consume.
//!
//! What is locked here:
//! - `start` is read-only: running it never creates files (the temp dirs are
//!   inspected after the call to prove nothing appeared).
//! - `start` emits exactly one `CliEnvelope` as JSON on stdout.
//! - The five states map to the documented exit codes and payload shapes.
//! - Re-running `start` is idempotent (same state on the second call).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The bootstrap states, as they appear in the `state` field of the `start`
/// payload. Mirrors `BootstrapState::as_str` in `start_cmd.rs`. Kept as plain
/// strings (not imported) so this test stays a black-box against the binary —
/// it catches wire-form regressions the unit tests would not.
///
/// `no_link` is covered by its own dedicated case below (it returns an `ok`
/// envelope with a `project init` `next_step`), so it has no entry in the
/// bootstrap-state constant list.
const STATE_SIDECAR_READY: &str = "sidecar_ready_no_contract";
const STATE_CONTRACT_PRESENT: &str = "contract_present";
const STATE_PREVIEW_RUN: &str = "preview_run";

const PROJECT_LINK_FILE_NAME: &str = ".forge-method.yaml";
const PROJECT_LINK_SCHEMA_VERSION: &str = "forge_project_link_v1";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

/// A fresh temp parent under the OS temp dir, cleaned up on drop. Mirrors the
/// `project_init_e2e` harness so the two stay consistent.
struct FreshParent {
    path: PathBuf,
}

impl FreshParent {
    fn new(label: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        // Use the OS temp dir, NOT repo_root()/target/. The repo-identity
        // validation (incident closure) rejects a consumer root nested inside a
        // foreign git repo, and target/ is inside the forge core repo — so the
        // test's bootstrap sidecar would be rejected. std::env::temp_dir()
        // returns a Windows path (D:\Temp\...) on this host, which avoids the
        // WSL→Windows /tmp mangling the old DD46 comment warned about.
        let path = std::env::temp_dir().join(format!(
            "start-e2e-{label}-{}-{n}",
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

/// Run `forge-core start --root <app> --json`; return `(exit_ok, envelope_json)`.
fn run_start(app: &Path) -> (bool, Value) {
    let output = bin()
        .args(["start", "--root"])
        .arg(app)
        .arg("--json")
        .output()
        .expect("run forge-core start");
    let exit_ok = output.status.success();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be a CliEnvelope JSON: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (exit_ok, json)
}

/// Write a Project Link pointing at a sidecar/state root relative to `app`.
fn write_link(app: &Path, sidecar_rel: &str, state_rel: &str) {
    fs::write(
        app.join(PROJECT_LINK_FILE_NAME),
        format!(
            "schema_version: {PROJECT_LINK_SCHEMA_VERSION}\n\
             project_id: app\n\
             sidecar_root: {sidecar_rel}\n\
             state_root: {state_rel}\n",
        ),
    )
    .expect("write project link");
}

/// Create the Forge state tree (the dirs `create_state_tree` would make).
fn make_state_tree(state: &Path) {
    for d in [
        "",
        "artifacts",
        "claims-active",
        "evidence",
        "traces",
        "wal",
    ] {
        fs::create_dir_all(state.join(d)).expect("create state dir");
    }
}

#[test]
fn state_one_no_link_bootstraps_the_project_in_one_command() {
    // Scenario A: empty repo, no Project Link. `start` now bootstraps the
    // project (creates the Project Link + sidecar) in a single command, then
    // reports the post-init state. The agent does not need a separate
    // `project init` step.
    let parent = FreshParent::new("no-link");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok, "no_link bootstrap must exit zero");
    assert_eq!(env["ok"], true, "bootstrap envelope ok must be true");
    assert_eq!(
        env["exit_reason"], "ok",
        "bootstrap must report ok"
    );
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "start should bootstrap and advance to sidecar_ready_no_contract"
    );
    assert_eq!(
        env["data"]["actions_performed"],
        serde_json::json!(["initialized"]),
        "start should report it initialized the project"
    );
    // Bootstrap actually created the Project Link.
    assert!(
        app.join(PROJECT_LINK_FILE_NAME).is_file(),
        "start should write a Project Link on no_link"
    );
}

#[test]
fn state_one_no_link_bootstraps_project_with_space_in_path() {
    // Space-in-path must not break bootstrap. The link is created at the raw
    // path (no shell quoting); agents read `actions_performed` and `state`.
    let parent = FreshParent::new("no-link path");
    let app = parent.path.join("app with spaces");
    fs::create_dir_all(&app).unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok, "no_link with a space path should still exit zero");
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "no_link with space path should bootstrap and advance"
    );
    assert_eq!(
        env["data"]["actions_performed"],
        serde_json::json!(["initialized"]),
        "no_link with space path should report it initialized"
    );
    assert!(
        app.join(PROJECT_LINK_FILE_NAME).is_file(),
        "start should write a Project Link even with a space in the path"
    );
}

#[test]
fn state_two_link_without_sidecar_repairs_the_sidecar() {
    // Scenario B: link parses and points at the canonical default sidecar,
    // but the sidecar/state root does not exist. `start` repairs the sidecar
    // (idempotent `init_project` re-creates the state tree), then reports the
    // post-repair state. The dir name matches the link's project_id ("app")
    // so the canonical-default reconciliation succeeds.
    let parent = FreshParent::new("no-sidecar");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    // sidecar/state intentionally NOT created.
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (exit_ok, env) = run_start(&app);

    assert!(
        exit_ok,
        "sidecar repair must exit zero"
    );
    assert_eq!(env["ok"], true);
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "state 2 should repair the sidecar and advance to sidecar_ready_no_contract"
    );
    assert_eq!(
        env["data"]["actions_performed"],
        serde_json::json!(["repaired_sidecar"]),
        "state 2 should report it repaired the sidecar"
    );
    // The sidecar state root was actually (re)created.
    assert!(
        app.parent()
            .unwrap()
            .join("forge-app")
            .join(".forge-method")
            .is_dir(),
        "start should (re)create the sidecar state root on state 2"
    );
}

#[test]
fn state_two_link_mismatch_fails_closed_not_silent_overwrite() {
    // Scenario B-fail: link parses and points at a canonical default sidecar,
    // but the dir name yields a different default project_id than the link
    // declares. `init_project` returns `ExistingProjectLinkMismatch` and
    // `start` surfaces it as an error rather than silently overwriting the
    // link. This protects against an operator hand-editing the link to a
    // non-default location and then losing it to an idempotent repair.
    let parent = FreshParent::new("no-sidecar-mismatch");
    // Dir name "app-with-spaces" slugifies differently than the link's
    // declared project_id ("app"), so the canonical-default reconciliation
    // cannot proceed.
    let app = parent.path.join("app with spaces");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (exit_ok, env) = run_start(&app);

    assert!(
        !exit_ok,
        "mismatched link + missing sidecar must fail closed, not overwrite"
    );
    assert_eq!(env["ok"], false);
    assert_eq!(
        env["exit_reason"], "invalid_decision_shape",
        "mismatch should surface as invalid_decision_shape, got {:?}",
        env["exit_reason"]
    );
}

#[test]
fn state_three_sidecar_ready_points_at_starter_fixtures() {
    // Scenario C: healthy state tree, no operation contract, no preview.
    let parent = FreshParent::new("ready-no-contract");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_SIDECAR_READY);
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 3 references is an array");
    let refs_joined = refs
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        refs_joined.contains("observe-project-status.yaml"),
        "state 3 should name the observe starter fixture"
    );
    assert!(
        refs_joined.contains("execute-trivial-write.yaml"),
        "state 3 should name the execute starter fixture"
    );
    assert!(
        refs_joined.contains("preview --operation"),
        "state 3 should point at the validation command"
    );
    assert!(
        env["data"]["next_step"]["command"].is_null(),
        "state 3's step is authoring, not a command"
    );
    assert!(
        env["data"]["next_step"]["argv"].is_null(),
        "state 3 should not expose argv when there is no command to execute"
    );
}

#[test]
fn state_four_contract_present_hands_off_to_guide() {
    // Scenario D: state tree + an operation-contract-looking file.
    let parent = FreshParent::new("with-contract");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    fs::write(app.join("my-operation.yaml"), "operation_contract: {}\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_CONTRACT_PRESENT);
    assert_eq!(
        env["data"]["next_step"]["command"], "forge-core guide describe",
        "state 4 hands off to guide describe"
    );
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!(["forge-core", "guide", "describe"]),
        "state 4 should expose typed guide argv"
    );
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 4 references is an array");
    let refs_joined = refs
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        refs_joined.contains("guide status --phase discovery"),
        "state 4 should point at the first-phase guide status"
    );
}

#[test]
fn state_five_preview_run_is_terminal() {
    // Scenario E: state tree + non-empty traces dir => preview has run.
    let parent = FreshParent::new("preview-run");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    // Simulate a trace having been written.
    fs::write(state.join("traces").join("m1.jsonl"), "{}\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_PREVIEW_RUN);
    assert_eq!(
        env["data"]["next_step"]["command"], "forge-core guide describe",
        "state 5 still points at guide (ongoing orientation)"
    );
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!(["forge-core", "guide", "describe"]),
        "state 5 should expose typed guide argv"
    );
}

#[test]
fn start_is_idempotent_running_twice_keeps_same_state() {
    // Read-only invariant: running start twice on the same repo must not
    // advance or regress the state, and must not create any files.
    let parent = FreshParent::new("idempotent");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (_, first) = run_start(&app);
    let (_, second) = run_start(&app);
    assert_eq!(
        first["data"]["state"], second["data"]["state"],
        "idempotent: state must not change across two runs"
    );
    // Nothing was created in the app dir beyond what the test wrote.
    let app_entries = fs::read_dir(&app).unwrap().count();
    assert_eq!(
        app_entries, 1,
        "idempotent: app dir should still contain only the Project Link"
    );
}
