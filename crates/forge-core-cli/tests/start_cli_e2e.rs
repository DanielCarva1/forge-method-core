//! End-to-end tests for `forge-core start` (F12 Guided Start).
//!
//! Exercises the binary as a real subprocess across all five [`BootstrapState`]s,
//! mirroring the `project_init_e2e.rs` harness pattern (`assert_cmd::Command` +
//! `FreshParent` with Drop cleanup). The unit tests in `start_cmd.rs` cover the
//! pure classifier; these tests verify the full argv → stdout-envelope → exit-code
//! contract that agents consume.
//!
//! What is locked here:
//! - clean, never-initialized projects bootstrap exactly once;
//! - linked missing/incomplete state fails closed with byte-identical filesystem state;
//! - `start` emits exactly one `CliEnvelope` as JSON on stdout;
//! - state loss is distinct from malformed-link corruption and clean bootstrap;
//! - healthy-state routing is idempotent and nonmutating.

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
        let path =
            std::env::temp_dir().join(format!("start-e2e-{label}-{}-{n}", std::process::id()));
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

fn run_start_text(app: &Path) -> std::process::Output {
    bin()
        .args(["start", "--root"])
        .arg(app)
        .arg("--text")
        .output()
        .expect("run forge-core start in text mode")
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

/// Create the minimum authoritative Forge state shape used by `start`.
fn make_state_tree(state: &Path) {
    for d in [
        "",
        "artifacts",
        "claims-active",
        "evidence",
        "handoffs/expired-claims",
        "index",
        "locks",
        "traces",
        "wal",
    ] {
        fs::create_dir_all(state.join(d)).expect("create state dir");
    }
    for f in [
        "ledger.ndjson",
        "wal/replay.fmr1",
        "replay-wal.manifest.json",
    ] {
        fs::write(state.join(f), b"").expect("create authority marker");
    }
}

fn tree_snapshot(root: &Path) -> Vec<(String, String, Vec<u8>)> {
    fn visit(base: &Path, path: &Path, entries: &mut Vec<(String, String, Vec<u8>)>) {
        let mut children = fs::read_dir(path)
            .expect("read snapshot directory")
            .map(|entry| entry.expect("read snapshot entry").path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            let relative = child
                .strip_prefix(base)
                .expect("snapshot path below base")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = fs::symlink_metadata(&child).expect("snapshot metadata");
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&child)
                    .expect("snapshot symlink target")
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes();
                entries.push((relative, "symlink".to_string(), target));
            } else if metadata.is_dir() {
                entries.push((relative, "dir".to_string(), Vec::new()));
                visit(base, &child, entries);
            } else {
                entries.push((
                    relative,
                    "file".to_string(),
                    fs::read(&child).expect("snapshot file bytes"),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_agent_native_workflow_handoff(env: &Value, app: &Path, state: &str) {
    let root = app.display().to_string();
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!(["forge-core", "workflow", "init", "--root", root]),
        "{state} should expose typed workflow init argv"
    );
    assert!(
        env["data"]["next_step"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("forge-core workflow init --root ")),
        "{state} should hand off to workflow init"
    );

    let references = env["data"]["next_step"]["references"]
        .as_array()
        .unwrap_or_else(|| panic!("{state} references should be an array"));
    assert!(
        references
            .first()
            .and_then(Value::as_str)
            .is_some_and(|reference| {
                reference.contains("next: forge-core workflow next --root")
                    && reference.contains(&app.display().to_string())
            }),
        "{state} should make workflow next for the same root the first reference"
    );
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
    assert_eq!(env["exit_reason"], "ok", "bootstrap must report ok");
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "start should bootstrap and advance to sidecar_ready_no_contract"
    );
    assert!(
        env["data"].get("state_loss").is_none(),
        "clean bootstrap must not carry state-loss status"
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
fn state_two_link_without_sidecar_fails_closed_without_mutation() {
    let parent = FreshParent::new("no-sidecar");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    let operator_root = parent.path.join("operator-anchors");
    fs::create_dir_all(&operator_root).unwrap();
    fs::write(operator_root.join("anchor.json"), b"{\"generation\":7}\n").unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok, "linked missing state must fail closed");
    assert_eq!(env["ok"], false);
    assert_eq!(env["exit_reason"], "env_config");
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["project"]["project_id"], "app");
    assert_eq!(
        env["data"]["state_loss"]["kind"],
        "linked_state_unavailable"
    );
    assert_eq!(env["data"]["state_loss"]["cause"], "missing_sidecar");
    assert_eq!(env["data"]["state_loss"]["project_id"], "app");
    assert_eq!(
        env["data"]["state_loss"]["project_link_schema_version"],
        PROJECT_LINK_SCHEMA_VERSION
    );
    let link_digest = env["data"]["state_loss"]["project_link_sha256"]
        .as_str()
        .expect("valid Project Link has an exact byte digest");
    assert_eq!(link_digest.len(), 64);
    assert!(link_digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        env["data"]["state_loss"]["workflow_release_status"],
        "unavailable_untrusted_state"
    );
    assert!(env["data"]["state_loss"]["workflow_release_id"].is_null());
    let state_loss_keys = env["data"]["state_loss"]
        .as_object()
        .expect("state_loss is an object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        state_loss_keys
            .iter()
            .all(|key| !key.contains("path") && !key.contains("root") && !key.contains("secret")),
        "typed state-loss identity must not expose secret paths: {state_loss_keys:?}"
    );
    assert!(
        env["data"].get("actions_performed").is_none(),
        "state-loss rejection must report no mutation actions"
    );
    assert!(
        env["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("possible durable-state loss")
                && message.contains("Automatic recreation is forbidden")),
        "state loss must have a distinct actionable diagnostic"
    );
    assert_eq!(
        tree_snapshot(&parent.path),
        before,
        "link, project, operator roots, and sidecar namespace must remain byte-identical"
    );
}

#[test]
fn state_two_human_output_names_state_loss_and_forbidden_recreation() {
    let parent = FreshParent::new("no-sidecar-text");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let output = run_start_text(&app);

    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("possible durable-state loss"));
    assert!(stderr.contains("Automatic recreation is forbidden"));
}

#[test]
fn linked_empty_and_partial_state_fail_closed_without_normalization() {
    for label in ["empty", "partial"] {
        let parent = FreshParent::new(label);
        let app = parent.path.join("app");
        let state = parent.path.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        if label == "empty" {
            fs::create_dir_all(&state).unwrap();
        } else {
            make_state_tree(&state);
            fs::remove_dir(state.join("evidence")).unwrap();
        }
        write_link(&app, "../forge-app", "../forge-app/.forge-method");
        let before = tree_snapshot(&parent.path);

        let (exit_ok, env) = run_start(&app);

        assert!(!exit_ok, "{label} linked state must fail closed");
        assert_eq!(env["data"]["state"], "link_present_no_sidecar");
        assert_eq!(env["data"]["state_loss"]["cause"], "incomplete_state");
        assert_eq!(tree_snapshot(&parent.path), before);
    }
}

#[cfg(unix)]
#[test]
fn linked_sidecar_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("sidecar-symlink");
    let app = parent.path.join("app");
    let foreign_sidecar = parent.path.join("foreign-sidecar");
    make_state_tree(&foreign_sidecar.join(".forge-method"));
    fs::create_dir_all(&app).unwrap();
    symlink(&foreign_sidecar, parent.path.join("forge-app")).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert!(env["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("symbolic link")));
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}
#[cfg(unix)]
#[test]
fn linked_ancestor_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("ancestor-symlink");
    let app = parent.path.join("app");
    let real_sidecar = parent.path.join("real").join("forge-app");
    make_state_tree(&real_sidecar.join(".forge-method"));
    fs::create_dir_all(&app).unwrap();
    symlink(parent.path.join("real"), parent.path.join("alias")).unwrap();
    write_link(
        &app,
        "../alias/forge-app",
        "../alias/forge-app/.forge-method",
    );
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[cfg(unix)]
#[test]
fn linked_ledger_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("ledger-symlink");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    make_state_tree(&state);
    fs::create_dir_all(&app).unwrap();
    fs::remove_file(state.join("ledger.ndjson")).unwrap();
    fs::write(parent.path.join("foreign-ledger.ndjson"), b"").unwrap();
    symlink(
        parent.path.join("foreign-ledger.ndjson"),
        state.join("ledger.ndjson"),
    )
    .unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert!(env["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("symbolic link")));
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}
#[cfg(unix)]
#[test]
fn linked_permission_denial_fails_closed_without_normalization() {
    use std::os::unix::fs::PermissionsExt;

    let parent = FreshParent::new("permission-denied");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    let ledger = state.join("ledger.ndjson");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);
    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&ledger).is_ok() {
        // Elevated principals can bypass mode bits, so no denial exists to test.
        fs::set_permissions(&ledger, fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (exit_ok, env) = run_start(&app);

    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["state_loss"]["cause"], "permission_denied");
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[test]
fn malformed_link_corruption_is_distinct_from_state_loss() {
    let parent = FreshParent::new("malformed-link");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join(PROJECT_LINK_FILE_NAME), "schema_version: [\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["ok"], false);
    assert!(
        env.get("data").is_none(),
        "corruption has no state-loss data"
    );
    assert!(!env["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("possible durable-state loss"));
}
#[test]
fn human_output_distinguishes_corruption_and_clean_bootstrap() {
    let clean_parent = FreshParent::new("clean-text");
    let clean_app = clean_parent.path.join("app");
    fs::create_dir_all(&clean_app).unwrap();
    let clean = run_start_text(&clean_app);
    assert!(clean.status.success());
    assert_eq!(String::from_utf8_lossy(&clean.stdout).trim(), "start: ok");
    assert!(clean.stderr.is_empty());

    let corrupt_parent = FreshParent::new("corrupt-text");
    let corrupt_app = corrupt_parent.path.join("app");
    fs::create_dir_all(&corrupt_app).unwrap();
    fs::write(
        corrupt_app.join(PROJECT_LINK_FILE_NAME),
        "schema_version: [\n",
    )
    .unwrap();
    let corrupt = run_start_text(&corrupt_app);
    assert!(!corrupt.status.success());
    assert!(corrupt.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&corrupt.stderr);
    assert!(stderr.contains("failed"));
    assert!(!stderr.contains("possible durable-state loss"));
}

#[test]
fn state_two_cross_project_link_fails_closed_not_silent_overwrite() {
    let parent = FreshParent::new("no-sidecar-cross-project");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(
        app.join(PROJECT_LINK_FILE_NAME),
        "schema_version: forge_project_link_v1\n\
         project_id: other-project\n\
         sidecar_root: ../forge-other-project\n\
         state_root: ../forge-other-project/.forge-method\n",
    )
    .unwrap();
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(
        !exit_ok,
        "cross-project link plus missing state must fail closed"
    );
    assert_eq!(env["ok"], false);
    assert_eq!(env["exit_reason"], "env_config");
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["state_loss"]["project_id"], "other-project");
    assert_eq!(tree_snapshot(&parent.path), before);
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
    assert_agent_native_workflow_handoff(&env, &app, "state 3");
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
}

#[test]
fn state_four_contract_present_hands_off_to_workflow() {
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
    assert_agent_native_workflow_handoff(&env, &app, "state 4");
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 4 references is an array");
    let refs_joined = refs
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        refs_joined.contains("compatibility: forge-core preview --operation"),
        "state 4 should retain legacy operation validation only as compatibility context"
    );
}

#[test]
fn state_five_preview_run_keeps_workflow_authority() {
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
    assert_agent_native_workflow_handoff(&env, &app, "state 5");
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 5 references is an array");
    assert!(
        refs.iter().filter_map(Value::as_str).any(|reference| {
            reference.contains("preview trace") && reference.contains("not workflow authority")
        }),
        "state 5 should retain preview evidence only as compatibility material"
    );
}

#[test]
fn clean_bootstrap_second_start_is_idempotent_and_nonmutating() {
    let parent = FreshParent::new("clean-bootstrap-twice");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();

    let (first_ok, first) = run_start(&app);
    let after_bootstrap = tree_snapshot(&parent.path);
    let (second_ok, second) = run_start(&app);

    assert!(first_ok && second_ok);
    assert_eq!(first["data"]["state"], second["data"]["state"]);
    assert_eq!(
        first["data"]["actions_performed"],
        serde_json::json!(["initialized"])
    );
    assert!(second["data"].get("actions_performed").is_none());
    assert!(second["data"].get("state_loss").is_none());
    assert_eq!(tree_snapshot(&parent.path), after_bootstrap);
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
