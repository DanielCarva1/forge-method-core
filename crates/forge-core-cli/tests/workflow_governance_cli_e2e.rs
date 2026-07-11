use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "forge-p5b-cli-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temporary directory");
    path
}

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("fixture write");
}

#[test]
fn agent_receives_explicit_simulation_and_simulation_only_compatibility_projection() {
    let bundle = repo_root().join("contracts/workflow-governance/kernel-v0.yaml");
    let input = repo_root().join("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");

    let output = bin()
        .args(["guide", "govern-simulate", "--bundle-file"])
        .arg(&bundle)
        .arg("--input-file")
        .arg(&input)
        .arg("--legacy-workflow-file")
        .arg(repo_root().join("contracts/workflows/build-story.yaml"))
        .arg("--json")
        .output()
        .expect("govern-simulate command");
    assert!(
        output.status.success(),
        "govern-simulate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("govern envelope");
    assert_eq!(envelope["command"], "guide.govern-simulate");
    assert_eq!(envelope["ok"], true);
    assert_eq!(
        envelope["data"]["simulation"]["authority"],
        "simulation_only"
    );
    assert_eq!(
        envelope["data"]["simulation"]["candidate_status"],
        "complete"
    );
    assert_eq!(
        envelope["data"]["simulation"]["candidate_completion"],
        "complete"
    );
    assert!(envelope["data"].get("decision").is_none());
    assert!(envelope["data"]["simulation"].get("status").is_none());
    assert!(envelope["data"]["simulation"].get("completion").is_none());
    assert!(envelope["data"]["simulation"].get("next_actions").is_none());
    assert_eq!(
        envelope["data"]["simulation"]["candidate_next_actions"][0]["kind"],
        "evaluate"
    );
    assert!(
        envelope["data"]["simulation"]["candidate_next_actions"][0]["description"]
            .as_str()
            .expect("candidate action description")
            .contains("trusted Project Snapshot evaluation")
    );
    assert_eq!(
        envelope["data"]["legacy_projection"]["authority"],
        "simulation_compatibility_only"
    );
    assert_eq!(
        envelope["data"]["legacy_projection"]["catalog_entry"]["id"],
        "build-story"
    );
    assert_eq!(
        envelope["data"]["legacy_projection"]["catalog_entry"]["workflow_ref"],
        "contracts/workflows/build-story.yaml"
    );
}

#[test]
fn incomplete_but_structurally_valid_work_is_candidate_guidance_only() {
    let bundle = repo_root().join("contracts/workflow-governance/kernel-v0.yaml");
    let input = repo_root().join("docs/fixtures/workflow-governance-kernel-v0/active.yaml");

    let output = bin()
        .args(["guide", "govern-simulate", "--bundle-file"])
        .arg(&bundle)
        .arg("--input-file")
        .arg(&input)
        .arg("--json")
        .output()
        .expect("govern-simulate command");
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("govern envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["simulation"]["candidate_status"], "active");
    assert!(envelope["data"].get("legacy_projection").is_none());
    assert_eq!(
        envelope["data"]["simulation"]["candidate_next_actions"][0]["kind"],
        "evaluate"
    );
}

#[test]
fn blocked_work_is_candidate_guidance_with_ranked_self_correction() {
    let bundle = repo_root().join("contracts/workflow-governance/kernel-v0.yaml");
    let input =
        repo_root().join("docs/fixtures/workflow-governance-kernel-v0/missing-capability.yaml");

    let output = bin()
        .args(["guide", "govern-simulate", "--bundle-file"])
        .arg(&bundle)
        .arg("--input-file")
        .arg(&input)
        .arg("--json")
        .output()
        .expect("govern-simulate command");
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("govern envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(
        envelope["data"]["simulation"]["candidate_status"],
        "blocked"
    );
    assert_eq!(
        envelope["data"]["simulation"]["candidate_capability_gaps"][0]["id"],
        "capability.representative-runtime"
    );
    assert_eq!(
        envelope["data"]["simulation"]["candidate_next_actions"][0]["kind"],
        "acquire_capability"
    );
}

#[test]
fn malformed_or_semantically_invalid_contracts_fail_closed_as_invalid_shape() {
    let root = temp_dir("invalid");
    let bundle = repo_root().join("contracts/workflow-governance/kernel-v0.yaml");
    let malformed_input = root.join("malformed-input.yaml");
    write(
        &malformed_input,
        "schema_version: '0.1'\nworkflow_governance_evaluation:\n  invented_authority: true\n",
    );

    let malformed = bin()
        .args(["guide", "govern-simulate", "--bundle-file"])
        .arg(&bundle)
        .arg("--input-file")
        .arg(&malformed_input)
        .arg("--json")
        .output()
        .expect("malformed command");
    assert_eq!(malformed.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&malformed.stdout).expect("failure envelope");
    assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    assert!(envelope["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("closed valid contract")));

    let mismatched_input = root.join("mismatched-input.yaml");
    let canonical_input = std::fs::read_to_string(
        repo_root().join("docs/fixtures/workflow-governance-kernel-v0/complete.yaml"),
    )
    .expect("canonical complete input");
    write(
        &mismatched_input,
        &canonical_input.replace(
            "bundle.workflow-governance.kernel-v0",
            "bundle.workflow-governance.unknown",
        ),
    );
    let mismatched = bin()
        .args(["guide", "govern-simulate", "--bundle-file"])
        .arg(&bundle)
        .arg("--input-file")
        .arg(&mismatched_input)
        .arg("--json")
        .output()
        .expect("structurally rejected command");
    assert_eq!(mismatched.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&mismatched.stdout).expect("failure envelope");
    assert!(envelope["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("BundleMismatch")));
}

#[test]
fn help_and_unknown_arguments_project_the_canonical_govern_simulate_surface() {
    let help = bin()
        .args(["guide", "govern-simulate", "--help"])
        .output()
        .expect("govern-simulate help");
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("guide govern-simulate --bundle-file <yaml> --input-file <yaml>"));

    let unknown = bin()
        .args(["guide", "govern-simulate", "--invent-completion"])
        .output()
        .expect("unknown argument");
    assert_eq!(unknown.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("unrecognized argument '--invent-completion'"));
    assert!(stderr.contains("guide govern-simulate --bundle-file <yaml> --input-file <yaml>"));
}
