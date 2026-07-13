//! Public operator trust lifecycle for an external workflow origin broker.
//! Forge receives public keys only; this test never constructs a signing key.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PUBLIC_KEY_V1: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const PUBLIC_KEY_V2: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn temp_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-workflow-broker-public-e2e-{}-{nonce}",
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
#[allow(clippy::too_many_lines)] // One chronological trust ceremony keeps every public lifecycle transition visible.
fn operator_trusts_rotates_and_revokes_public_broker_keys_outside_project_state() {
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

    let key_v1 = parent.join("broker-v1.pub");
    let key_v2 = parent.join("broker-v2.pub");
    let ceremony_v1 = parent.join("broker-v1-ceremony.md");
    let ceremony_v2 = parent.join("broker-v2-ceremony.md");
    fs::write(&key_v1, PUBLIC_KEY_V1).expect("public key v1");
    fs::write(&key_v2, PUBLIC_KEY_V2).expect("public key v2");
    fs::write(&ceremony_v1, "operator enrolled broker v1 outside agent\n").expect("ceremony v1");
    fs::write(
        &ceremony_v2,
        "operator rotated broker to v2 outside agent\n",
    )
    .expect("ceremony v2");

    let trusted = broker(
        &app_arg,
        &[
            "trust",
            "--issuer-id",
            "broker.host.human.v1",
            "--profile",
            "human",
            "--public-key-file",
            &key_v1.display().to_string(),
            "--ceremony-ref",
            "operator://ceremony/human/v1",
            "--ceremony-file",
            &ceremony_v1.display().to_string(),
        ],
    );
    assert_eq!(trusted["data"]["action"], "added_broker_trust");
    let registry = PathBuf::from(
        trusted["data"]["registry_path"]
            .as_str()
            .expect("registry path"),
    );
    assert!(registry.is_file());
    assert!(!registry.starts_with(&app));
    assert!(!trusted.to_string().contains("private_key"));

    let admitted_registry = fs::read_to_string(&registry).expect("admitted registry");
    let expected_audience = trusted["data"]["audience"]
        .as_str()
        .expect("project audience");
    fs::write(
        &registry,
        admitted_registry.replacen(
            expected_audience,
            "forge-core:workflow:project.copied-from-elsewhere",
            1,
        ),
    )
    .expect("foreign-audience registry");
    let foreign_status = bin()
        .args(["workflow", "broker", "status", "--root", &app_arg, "--json"])
        .output()
        .expect("foreign registry status");
    assert!(!foreign_status.status.success());
    let foreign_error: Value =
        serde_json::from_slice(&foreign_status.stdout).expect("foreign registry failure JSON");
    assert!(foreign_error["error"]["message"]
        .as_str()
        .expect("foreign registry message")
        .contains("audience mismatch"));
    fs::write(&registry, admitted_registry).expect("restore project registry");

    let packets = ok(&bin()
        .args(["workflow", "action-packets", "--root", &app_arg, "--json"])
        .output()
        .expect("action packets"));
    assert_eq!(
        packets["data"]["registry_setup"]["broker_registry"],
        "ready"
    );

    let rotated = broker(
        &app_arg,
        &[
            "rotate",
            "--replaces",
            "broker.host.human.v1",
            "--issuer-id",
            "broker.host.human.v2",
            "--profile",
            "human",
            "--public-key-file",
            &key_v2.display().to_string(),
            "--ceremony-ref",
            "operator://ceremony/human/v2",
            "--ceremony-file",
            &ceremony_v2.display().to_string(),
        ],
    );
    assert_eq!(rotated["data"]["action"], "rotated_broker_trust");
    let issuers = rotated["data"]["issuers"].as_array().expect("issuers");
    assert_eq!(issuers.len(), 2);
    assert_eq!(issuers[0]["status"], "revoked");
    assert_eq!(issuers[1]["status"], "active");

    let revoked = broker(&app_arg, &["revoke", "--issuer-id", "broker.host.human.v2"]);
    assert_eq!(revoked["data"]["action"], "revoked_broker_trust");
    assert_eq!(revoked["data"]["issuers"][1]["status"], "revoked");

    let all_revoked = broker(&app_arg, &["status"]);
    assert!(all_revoked["data"]["issuers"]
        .as_array()
        .expect("revoked issuers")
        .iter()
        .all(|issuer| issuer["status"] == "revoked"));
    let packets = ok(&bin()
        .args(["workflow", "action-packets", "--root", &app_arg, "--json"])
        .output()
        .expect("action packets after last revoke"));
    assert_eq!(
        packets["data"]["registry_setup"]["broker_registry"],
        "no_active_issuer"
    );
    let next = ok(&bin()
        .args(["workflow", "next", "--root", &app_arg, "--json"])
        .output()
        .expect("next after last revoke"));
    assert_eq!(
        next["data"]["authorization"]["registry_setup"]["broker_registry"],
        "no_active_issuer"
    );
    assert!(!next["data"]["authorization"]["action_packets"]
        .as_array()
        .expect("next action packets")
        .is_empty());
    let setup_gaps = next["data"]["authorization"]["setup_gaps"]
        .as_array()
        .expect("setup gaps");
    assert!(!setup_gaps.is_empty());
    assert!(setup_gaps.iter().all(|gap| {
        gap["code"] == "broker_registry_no_active_issuer"
            && gap["setup_argv"][0] == "forge-core"
            && !gap.to_string().contains("private_key")
    }));
}

#[test]
fn broker_usage_errors_are_machine_readable_and_unknown_flags_fail_closed() {
    for args in [
        vec!["workflow", "broker", "trust", "--json"],
        vec![
            "workflow",
            "broker",
            "status",
            "--root",
            ".",
            "--invented",
            "value",
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
