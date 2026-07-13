//! Public workflow-credential lifecycle proof. The test never constructs a
//! signing key and never writes the trusted registry itself.

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
        "forge-workflow-credential-public-e2e-{}-{nonce}",
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

fn credential(app: &str, args: &[&str]) -> Value {
    let mut command = bin();
    command.args(["workflow", "credential"]);
    command.args(args);
    command.args(["--root", app, "--json"]);
    ok(&command.output().expect("workflow credential command"))
}

#[test]
#[allow(clippy::too_many_lines)] // one auditable public lifecycle, kept chronological
fn public_cli_provisions_signs_rotates_and_revokes_without_consumer_state() {
    let parent = temp_root();
    let app = parent.join("app");
    fs::create_dir_all(&app).expect("app root");
    fs::write(app.join("README.md"), "# app\n").expect("app README");
    let _ = std::process::Command::new("git")
        .arg("init")
        .arg(&app)
        .output();
    let app_arg = app.display().to_string();

    ok(&bin()
        .args(["start", "--root", &app_arg, "--json"])
        .output()
        .expect("start"));

    let provisioned = credential(
        &app_arg,
        &[
            "provision",
            "--credential-id",
            "credential.workflow.human",
            "--principal-id",
            "principal.workflow.human",
            "--agent-id",
            "agent.workflow.human-console",
            "--profile",
            "human",
        ],
    );
    assert_eq!(provisioned["data"]["action"], "provisioned");
    let registry = PathBuf::from(
        provisioned["data"]["registry_path"]
            .as_str()
            .expect("registry path"),
    );
    assert!(registry.is_file());
    assert!(!registry.starts_with(&app));
    assert!(!app.join(".forge-method").exists());
    assert!(!provisioned.to_string().contains("private_key"));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let request_path = parent.join("applicability.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "project_id": "app",
            "policy_bundle_digest": format!("sha256:{}", "1".repeat(64)),
            "policy_ref": "policy.test",
            "state_version": 1,
            "current_phase": "1-discovery",
            "snapshot_digest": format!("sha256:{}", "2".repeat(64)),
            "ledger_head_digest": format!("sha256:{}", "3".repeat(64)),
            "applicable": true,
            "evaluator_ref": "evaluator.workflow.applicability.human",
            "authority_scope": "workflow.applicability.assess",
            "basis_refs": ["chat://decision/1"],
            "basis_digest": format!("sha256:{}", "4".repeat(64)),
            "observed_at_unix": now,
            "expires_at_unix": now + 300
        }))
        .expect("request JSON"),
    )
    .expect("request file");
    let attestation_path = parent.join("attestation.json");
    let signed = credential(
        &app_arg,
        &[
            "sign",
            "--credential-id",
            "credential.workflow.human",
            "--kind",
            "applicability",
            "--request-file",
            &request_path.display().to_string(),
            "--output-file",
            &attestation_path.display().to_string(),
        ],
    );
    assert_eq!(signed["data"]["action"], "signed_applicability_assess");
    let attestation: Value =
        serde_json::from_slice(&fs::read(attestation_path).expect("attestation bytes"))
            .expect("attestation JSON");
    assert_eq!(attestation["credential_id"], "credential.workflow.human");
    assert_eq!(attestation["signature"].as_str().map(str::len), Some(128));

    let rotated = credential(
        &app_arg,
        &[
            "rotate",
            "--replaces",
            "credential.workflow.human",
            "--credential-id",
            "credential.workflow.human.v2",
            "--principal-id",
            "principal.workflow.human",
            "--agent-id",
            "agent.workflow.human-console",
            "--profile",
            "human",
        ],
    );
    assert_eq!(rotated["data"]["action"], "rotated");
    let status = credential(&app_arg, &["status"]);
    let principals = status["data"]["principals"].as_array().expect("principals");
    assert_eq!(principals.len(), 2);
    assert_eq!(principals[0]["status"], "revoked");
    assert_eq!(principals[1]["status"], "active");

    let revoked = credential(
        &app_arg,
        &["revoke", "--credential-id", "credential.workflow.human.v2"],
    );
    assert_eq!(revoked["data"]["action"], "revoked");
    let rejected = bin()
        .args([
            "workflow",
            "credential",
            "sign",
            "--root",
            &app_arg,
            "--credential-id",
            "credential.workflow.human.v2",
            "--kind",
            "applicability",
            "--request-file",
            &request_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("revoked signing attempt");
    assert!(!rejected.status.success());
    let rejected: Value = serde_json::from_slice(&rejected.stdout).expect("failure envelope");
    assert_eq!(rejected["ok"], false);
    assert_eq!(rejected["exit_reason"], "env_config");
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn credential_usage_errors_remain_machine_readable_and_unknown_flags_fail_closed() {
    for args in [
        vec!["workflow", "credential", "provision", "--json"],
        vec![
            "workflow",
            "credential",
            "status",
            "--root",
            ".",
            "--imagined-flag",
            "value",
            "--json",
        ],
    ] {
        let output = bin()
            .args(args)
            .output()
            .expect("invalid credential command");
        assert!(!output.status.success());
        let envelope: Value = serde_json::from_slice(&output.stdout).expect("failure envelope");
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    }
}
