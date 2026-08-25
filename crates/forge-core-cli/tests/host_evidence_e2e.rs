use assert_cmd::Command;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

#[test]
fn host_evidence_is_registered_with_the_public_verify_surface() {
    let output = bin()
        .args(["host-evidence", "--help"])
        .output()
        .expect("run host-evidence help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("host-evidence verify --bundle-file"));
}

#[test]
fn missing_bundle_is_a_bounded_json_error_from_the_real_command() {
    let output = bin()
        .args([
            "host-evidence",
            "verify",
            "--bundle-file",
            "missing-real-host-evidence.yaml",
            "--json",
        ])
        .output()
        .expect("run host-evidence verify");
    assert_eq!(output.status.code(), Some(5));
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON error envelope");
    assert_eq!(envelope["command"], "host-evidence.verify");
    assert_eq!(envelope["ok"], false);
    let message = envelope["error"]["message"].as_str().expect("message");
    assert!(message.contains("real-host evidence verification failed"));
    assert!(message.contains("does not certify a production host"));
}
