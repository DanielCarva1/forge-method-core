//! End-to-end CLI tests for layer-1 worktree isolation (S4.6).
//!
//! These exercise the run_* entry points (load->validate->persist->envelope)
//! against a real temp directory, mirroring `claim_e2e.rs`. They prove the
//! multi-agent isolation promise: two agents get disjoint worktrees/branches;
//! a duplicate is blocked; merge-back produces a deterministic git step list.

use assert_cmd::Command;
use forge_core_cli::isolation::{run_merge_plan, run_propose, run_status, run_transition};
use forge_core_contracts::common::StableId;
use forge_core_contracts::isolation::{IsolationStatus, MergePolicy};
use std::fs;
use std::path::{Path, PathBuf};

fn dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("iso-e2e-{label}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

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

fn fresh_parent(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let d = repo_root().join("target").join(format!(
        "isolation-cli-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

struct ConsumerApp {
    app: PathBuf,
    state_root: PathBuf,
}

fn consumer_app(label: &str) -> ConsumerApp {
    let parent = fresh_parent(label);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");

    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");

    ConsumerApp { app, state_root }
}

fn output_json(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_cli_success(output: &std::process::Output, label: &str) -> serde_json::Value {
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

fn assert_cli_failure(output: &std::process::Output, label: &str) -> serde_json::Value {
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

fn assert_env_config_failure(output: &std::process::Output, label: &str) -> serde_json::Value {
    let json = assert_cli_failure(output, label);
    assert_eq!(
        json["exit_reason"], "env_config",
        "{label} should fail with env_config exit_reason: {json:#}"
    );
    assert_eq!(
        json["error"]["code"], "env_config",
        "{label} should fail with env_config error code: {json:#}"
    );
    json
}

fn yaml_file_count(dir: &Path) -> usize {
    fs::read_dir(dir)
        .expect("read isolation dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
        .count()
}

fn propose(
    d: &Path,
    agent: &str,
    branch: &str,
    path: &str,
) -> forge_core_contracts::CliEnvelope<forge_core_cli::isolation::IsolationProposePayload> {
    run_propose(
        d,
        &StableId(agent.into()),
        branch,
        path,
        "main",
        MergePolicy::Rebase,
        None,
        &format!("iso-{agent}-{}", branch.replace('/', "-")),
        NOW,
    )
}

#[test]
fn isolation_propose_defaults_to_resolved_sidecar_isolations_dir() {
    let fixture = consumer_app("default-sidecar");
    let app_arg = fixture.app.display().to_string();

    let output = bin()
        .args([
            "isolation",
            "propose",
            "--root",
            &app_arg,
            "--agent",
            "sidecar-agent",
            "--branch",
            "sidecar/feature",
            "--worktree-path",
            "../wt/sidecar",
            "--base-ref",
            "main",
            "--id",
            "iso-sidecar-feature",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run isolation propose");

    let json = assert_cli_success(&output, "isolation propose with default sidecar dir");
    assert_eq!(json["command"], "isolation propose");
    let expected_dir = fixture.state_root.join("contracts").join("isolations");
    let contract_path = PathBuf::from(
        json["data"]["contract_path"]
            .as_str()
            .expect("contract path"),
    );
    assert!(
        contract_path.starts_with(&expected_dir),
        "contract should be written under sidecar isolation dir\nexpected: {}\nactual: {}",
        expected_dir.display(),
        contract_path.display()
    );
    assert_eq!(yaml_file_count(&expected_dir), 1);
    assert!(
        !fixture.app.join("contracts").join("isolations").exists(),
        "default isolation propose must not create consumer-local contracts/isolations"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "default isolation propose must not create consumer-local .forge-method"
    );
}

#[test]
fn isolation_status_requires_project_link_without_explicit_isolation_dir() {
    let parent = fresh_parent("missing-link-status");
    let app = parent.join("app");
    fs::create_dir_all(&app).expect("create unlinked app");

    let output = bin()
        .args(["isolation", "status", "--root"])
        .arg(&app)
        .output()
        .expect("run isolation status without project link");

    let json = assert_env_config_failure(&output, "isolation status without project link");
    let message = json["error"]["message"].as_str().expect("error message");
    assert!(
        message.contains(".forge-method.yaml"),
        "error should explain missing Project Link: {message}"
    );
    assert!(
        !app.join(".forge-method").exists(),
        "failed isolation status must not create consumer-local .forge-method"
    );
    assert!(
        !app.join("contracts").join("isolations").exists(),
        "failed isolation status must not create consumer-local contracts/isolations"
    );
}

#[test]
fn isolation_status_rejects_project_link_missing_state_root() {
    let parent = fresh_parent("missing-state-root");
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");
    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&sidecar).expect("create sidecar root parent");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");

    let output = bin()
        .args(["isolation", "status", "--root"])
        .arg(&app)
        .output()
        .expect("run isolation status with missing state_root");

    let json = assert_env_config_failure(&output, "isolation status with missing state_root");
    let message = json["error"]["message"].as_str().expect("error message");
    assert!(
        message.contains("state_root"),
        "error should mention state_root: {message}"
    );
    assert!(
        message.contains("does not exist"),
        "error should distinguish missing state_root: {message}"
    );
    assert!(
        !app.join(".forge-method").exists(),
        "failed isolation status must not create consumer-local .forge-method"
    );
    assert!(
        !app.join("contracts").join("isolations").exists(),
        "failed isolation status must not create consumer-local contracts/isolations"
    );
    assert!(
        !state_root.exists(),
        "isolation status must not create the missing sidecar state root"
    );
}

#[test]
fn isolation_status_rejects_project_link_state_root_file() {
    let parent = fresh_parent("state-root-file");
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");
    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&sidecar).expect("create sidecar root parent");
    fs::write(&state_root, "not a directory").expect("write corrupt state root file");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");

    let output = bin()
        .args(["isolation", "status", "--root"])
        .arg(&app)
        .output()
        .expect("run isolation status with state_root file");

    let json = assert_env_config_failure(&output, "isolation status with state_root file");
    let message = json["error"]["message"].as_str().expect("error message");
    assert!(
        message.contains("state_root"),
        "error should mention state_root: {message}"
    );
    assert!(
        message.contains("not a directory"),
        "error should fail closed on file state_root: {message}"
    );
    assert!(
        !app.join(".forge-method").exists(),
        "failed isolation status must not create consumer-local .forge-method"
    );
    assert!(
        !app.join("contracts").join("isolations").exists(),
        "failed isolation status must not create consumer-local contracts/isolations"
    );
    assert!(
        state_root.is_file(),
        "isolation status must not replace corrupt state_root file"
    );
}

#[test]
fn explicit_isolation_dir_preserves_existing_behavior() {
    let parent = fresh_parent("explicit-dir");
    let app = parent.join("app");
    let isolation_dir = parent.join("explicit-isolations");
    let app_arg = app.display().to_string();
    let isolation_dir_arg = isolation_dir.display().to_string();
    fs::create_dir_all(&app).expect("create unlinked app");

    let propose = bin()
        .args([
            "isolation",
            "propose",
            "--root",
            &app_arg,
            "--isolation-dir",
            &isolation_dir_arg,
            "--agent",
            "explicit-agent",
            "--branch",
            "explicit/feature",
            "--worktree-path",
            "../wt/explicit",
            "--base-ref",
            "main",
            "--id",
            "iso-explicit-feature",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run isolation propose with explicit dir");
    assert_cli_success(&propose, "isolation propose with explicit dir");
    assert_eq!(yaml_file_count(&isolation_dir), 1);

    let status = bin()
        .args([
            "isolation",
            "status",
            "--root",
            &app_arg,
            "--isolation-dir",
            &isolation_dir_arg,
        ])
        .output()
        .expect("run isolation status with explicit dir");
    let status_json = assert_cli_success(&status, "isolation status with explicit dir");
    assert_eq!(status_json["data"]["total"], 1);
    assert!(
        !app.join(".forge-method").exists(),
        "explicit --isolation-dir must not create consumer-local .forge-method"
    );
    assert!(
        !app.join("contracts").join("isolations").exists(),
        "explicit --isolation-dir should not use the old implicit consumer-local default"
    );
}

#[test]
fn two_agents_get_disjoint_isolations() {
    let d = dir("disjoint");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let b = propose(&d, "bob", "bob/s6", "../wt/b");
    assert!(a.ok, "alice propose must succeed");
    assert!(b.ok, "bob propose must succeed");

    let status = run_status(&d, None);
    assert!(status.ok);
    assert_eq!(status.data.as_ref().unwrap().total, 2);
}

#[test]
fn duplicate_branch_is_blocked() {
    let d = dir("dupbranch");
    let _ = propose(&d, "alice", "shared/feature", "../wt/a");
    let b = propose(&d, "bob", "shared/feature", "../wt/b");
    assert!(!b.ok, "duplicate branch must be blocked");
    assert_eq!(b.exit_code(), 2);
    assert!(b
        .error
        .as_ref()
        .unwrap()
        .message
        .contains("duplicate_branch"));
}

#[test]
fn duplicate_worktree_path_is_blocked() {
    let d = dir("duppath");
    let _ = propose(&d, "alice", "alice/s5", "../wt/shared");
    let b = propose(&d, "bob", "bob/s6", "../wt/shared");
    assert!(!b.ok, "duplicate worktree path must be blocked");
    assert!(b
        .error
        .as_ref()
        .unwrap()
        .message
        .contains("duplicate_worktree_path"));
}

#[test]
fn propose_returns_suggested_git_commands() {
    let d = dir("suggest");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let cmds = &a.data.as_ref().unwrap().suggested_git_commands;
    assert!(
        cmds.iter().any(|c| c.contains("worktree add")),
        "must suggest worktree add"
    );
    // worktree_path is now shell-quoted (review S4.6 C1): cd '../wt/a'
    assert!(
        cmds.iter().any(|c| c.contains("cd '../wt/a'")),
        "must suggest cd into worktree"
    );
}

#[test]
fn merge_plan_produces_deterministic_rebase_steps() {
    let d = dir("mergeplan");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);
    let plan = run_merge_plan(&d, &id, NOW);
    assert!(plan.ok);
    let steps = &plan.data.as_ref().unwrap().steps;
    assert_eq!(steps.len(), 4, "rebase policy => 4 steps");
    // Rebase step must reference both base ref and branch.
    assert!(steps.iter().any(|s| {
        use forge_core_contracts::isolation::GitAction::*;
        matches!(s.action, Rebase) && s.args.contains(&"main".to_string())
    }));
}

#[test]
fn full_lifecycle_active_to_merged_releases_branch() {
    let d = dir("lifecycle");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);

    // Proposed -> Active
    let t = run_transition(&d, &id, IsolationStatus::Active, NOW);
    assert!(t.ok);
    // Active -> Merging (emits merge commands)
    let t = run_transition(&d, &id, IsolationStatus::Merging, NOW);
    assert!(t.ok);
    assert!(!t.data.as_ref().unwrap().suggested_git_commands.is_empty());
    // Merging -> Merged (terminal)
    let t = run_transition(&d, &id, IsolationStatus::Merged, NOW);
    assert!(t.ok);

    // Branch + path are now free for another agent.
    let b = propose(&d, "bob", "alice/s5", "../wt/a");
    assert!(
        b.ok,
        "merged isolation releases its branch and worktree path"
    );
}

#[test]
fn illegal_transition_blocked() {
    let d = dir("illegal");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);
    // Proposed -> Merged is illegal (skip Active/Merging).
    let t = run_transition(&d, &id, IsolationStatus::Merged, NOW);
    assert!(!t.ok);
    assert_eq!(t.exit_code(), 2);
}

#[test]
fn merge_plan_unknown_id_is_invalid_shape() {
    let d = dir("unknown");
    let plan = run_merge_plan(&d, &StableId("does-not-exist".into()), NOW);
    assert!(!plan.ok);
    assert_eq!(plan.exit_code(), 3); // InvalidDecisionShape
}

#[test]
fn status_persists_across_invocations() {
    // The contract YAML is the materialized state (DD22): a fresh process
    // loading the same dir sees the same isolations.
    let d = dir("persist");
    let _ = propose(&d, "alice", "alice/s5", "../wt/a");
    let _ = propose(&d, "bob", "bob/s6", "../wt/b");

    let status = run_status(&d, None);
    assert_eq!(status.data.as_ref().unwrap().total, 2);

    let ids: Vec<_> = status
        .data
        .unwrap()
        .active
        .into_iter()
        .map(|s| s.agent_id)
        .collect();
    assert!(ids.contains(&"alice".to_string()));
    assert!(ids.contains(&"bob".to_string()));
}
