//! End-to-end command wiring for the strict public workflow-broker control plane.
//!
//! Genesis remains externally blocked until a selected-host adapter provides a
//! preconfigured operator trust anchor; command inputs cannot bootstrap trust.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn temp_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-workflow-broker-product-e2e-{}-{nonce}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    root
}

fn ok(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON output")
}

fn broker(app: &str, args: &[&str]) -> Value {
    let mut command = bin();
    command.args(["workflow", "broker"]);
    command.args(args);
    command.args(["--root", app, "--json"]);
    ok(&command.output().expect("workflow broker command"))
}

#[test]
fn product_genesis_is_typed_blocked_without_selected_host_anchor() {
    let parent = temp_root();
    let app = parent.join("app");
    fs::create_dir_all(&app).expect("app root");
    fs::write(app.join("README.md"), "# app\n").expect("README");
    let _ = std::process::Command::new("git")
        .arg("init")
        .arg(&app)
        .output();
    let app_arg = app.display().to_string();
    ok(&bin()
        .args(["start", "--root", &app_arg, "--json"])
        .output()
        .expect("start"));
    ok(&bin()
        .args(["workflow", "init", "--root", &app_arg, "--json"])
        .output()
        .expect("workflow init"));

    let registry_file = parent.join("agent-created-genesis.yaml");
    let authorization_file = parent.join("agent-created-genesis-authorization.json");
    fs::write(&registry_file, "self_trusting: true\n").expect("registry fixture");
    fs::write(&authorization_file, "{\"self_signed\":true}\n").expect("authorization fixture");
    let blocked = broker(
        &app_arg,
        &[
            "initialize",
            "--registry-file",
            &registry_file.display().to_string(),
            "--authorization-file",
            &authorization_file.display().to_string(),
        ],
    );
    assert_eq!(blocked["data"]["action"], "broker_genesis_blocked");
    assert_eq!(
        blocked["data"]["component_state"],
        "blocked_external_dependency"
    );
    assert_eq!(blocked["data"]["external_setup"]["state"], "blocked");
    assert_eq!(
        blocked["data"]["external_setup"]["reason"],
        "selected_host_unavailable"
    );
    assert!(blocked["data"]["receipt"].is_null());
    assert!(blocked["data"]["registry_digest"].is_null());
    let text = blocked.to_string();
    assert!(!text.contains("private_key"));
    assert!(!text.contains("selected-host custody complete"));

    let _ = fs::remove_dir_all(parent);
}

#[test]
fn broker_usage_and_oracle_shaped_inputs_fail_closed() {
    for args in [
        vec!["workflow", "broker", "register", "--json"],
        vec![
            "workflow",
            "broker",
            "status",
            "--root",
            ".",
            "--sign-file",
            "packet.json",
            "--json",
        ],
    ] {
        let output = bin().args(args).output().expect("invalid broker command");
        assert!(!output.status.success());
        let envelope: Value = serde_json::from_slice(&output.stdout).expect("failure envelope");
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    }
}
