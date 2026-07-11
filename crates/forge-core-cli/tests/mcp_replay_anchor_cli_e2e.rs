use assert_cmd::Command;
use forge_core_contracts::PrincipalId;
use forge_core_store::replay_wal::{replay_wal_path, reserve_replay_nonce};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn fixture() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "forge-mcp-replay-anchor-{}-{unique}",
        std::process::id()
    ));
    let project = parent.join("consumer");
    let state_root = parent.join("runtime/.forge-method");
    let operator = parent.join("operator");
    fs::create_dir_all(&project).expect("project");
    fs::create_dir_all(&operator).expect("operator");
    fs::write(project.join("README.md"), "# replay anchor fixture\n").expect("readme");
    let init = bin()
        .args(["project", "init", "--root"])
        .arg(&project)
        .arg("--sidecar-root")
        .arg(parent.join("runtime"))
        .arg("--state-root")
        .arg(&state_root)
        .arg("--json")
        .output()
        .expect("project init");
    assert!(
        init.status.success(),
        "project init failed: {}",
        String::from_utf8_lossy(&init.stdout)
    );
    let anchor = operator.join("replay-anchor.json");
    (parent, project, state_root, anchor)
}

fn command(project: &PathBuf, anchor: &PathBuf, action: &str) -> Command {
    let mut command = bin();
    command
        .args(["mcp", "replay-anchor", action, "--root"])
        .arg(project)
        .arg("--anchor")
        .arg(anchor)
        .arg("--json");
    command
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON output failed: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn agent_can_anchor_advance_and_detect_replay_pair_rollback() {
    let (_parent, project, state_root, anchor) = fixture();
    let provision = command(&project, &anchor, "provision")
        .args(["--deployment-id", "deployment.cli-test"])
        .output()
        .expect("provision");
    assert!(provision.status.success());
    let provisioned = output_json(&provision);
    assert_eq!(provisioned["data"]["anchor"]["head"]["last_seq"], 0);
    assert!(anchor.exists());
    assert!(!project.join(".forge-method").exists());

    let empty_wal = fs::read(replay_wal_path(&state_root)).expect("empty WAL");
    reserve_replay_nonce(
        &state_root,
        &PrincipalId("principal.cli-test".to_owned()),
        "forge://replay-anchor/cli-test",
        "replay-anchor-cli-nonce-0001",
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .expect("reserve");

    let pending = command(&project, &anchor, "verify")
        .output()
        .expect("verify pending");
    assert!(pending.status.success());
    assert_eq!(output_json(&pending)["data"]["status"], "advance_required");
    let advanced = command(&project, &anchor, "advance")
        .output()
        .expect("advance");
    assert!(advanced.status.success());
    assert_eq!(output_json(&advanced)["data"]["anchor"]["generation"], 2);

    fs::write(replay_wal_path(&state_root), empty_wal).expect("rollback WAL");
    let rejected = command(&project, &anchor, "verify")
        .output()
        .expect("verify rollback");
    assert!(!rejected.status.success());
    let rejection = output_json(&rejected);
    assert_eq!(rejection["ok"], false);
    assert!(rejection["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("rollback detected")));
}

#[test]
fn agent_cannot_place_external_anchor_inside_the_project() {
    let (_parent, project, _state_root, _anchor) = fixture();
    let project_anchor = project.join("replay-anchor.json");
    let rejected = command(&project, &project_anchor, "provision")
        .args(["--deployment-id", "deployment.invalid-location"])
        .output()
        .expect("reject project-local anchor");
    assert!(!rejected.status.success());
    let envelope = output_json(&rejected);
    assert!(envelope["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("outside both project and Forge state")));
    assert!(!project_anchor.exists());
}
