use assert_cmd::Command;
use forge_core_contracts::WorkflowGovernanceReleaseManifestDocument;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST_PATH: &str = "contracts/migration/workflow-governance-release-foundation-v0.yaml";
const BATCH_PATH: &str = "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";

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
        "forge-p5d-rollout-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temporary directory");
    path
}

struct RolloutFixture {
    manifest_file: PathBuf,
    batch_file: PathBuf,
}

fn build_rollout_fixture() -> RolloutFixture {
    let root = repo_root();
    let manifest_file = root.join(MANIFEST_PATH);
    let batch_file = root.join(BATCH_PATH);
    assert!(manifest_file.is_file(), "canonical release manifest");
    assert!(batch_file.is_file(), "canonical migration batch");
    RolloutFixture {
        manifest_file,
        batch_file,
    }
}

fn run_rollout(manifest: &Path, batch: Option<&Path>) -> std::process::Output {
    let mut command = bin();
    command
        .args(["guide", "rollout-audit", "--manifest-file"])
        .arg(manifest);
    if let Some(batch) = batch {
        command.arg("--batch-file").arg(batch);
    }
    command
        .arg("--catalog-dir")
        .arg(repo_root().join("contracts/workflows"))
        .arg("--plan-file")
        .arg(repo_root().join("contracts/policies/workflow-migration-foundation-v0.yaml"))
        .arg("--json")
        .output()
        .expect("rollout audit command")
}

#[test]
fn agent_receives_complete_candidate_only_rollout_scorecard() {
    let fixture = build_rollout_fixture();
    let output = run_rollout(&fixture.manifest_file, Some(&fixture.batch_file));
    assert!(
        output.status.success(),
        "rollout audit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("rollout envelope");
    assert_eq!(envelope["command"], "guide.rollout-audit");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["status"], "structurally_valid");
    assert_eq!(envelope["data"]["authority"], "candidate_only");
    assert_eq!(
        envelope["data"]["evidence_assurance"],
        "content_integrity_only"
    );
    assert_eq!(
        envelope["data"]["counts"]["migration_candidate_structurally_valid"],
        15
    );
    assert_eq!(envelope["data"]["counts"]["compatibility_only"], 77);
    assert_eq!(envelope["data"]["counts"]["domain_pack_candidate"], 18);
    assert_eq!(envelope["data"]["counts"]["quarantined"], 0);
    assert_eq!(
        envelope["data"]["counts"]["retirement_pending_verification"],
        0
    );
    let assessments = envelope["data"]["assessments"]
        .as_array()
        .expect("assessments");
    assert_eq!(assessments.len(), 110);
    for assessment in assessments {
        assert_ne!(assessment["state"], "executable");
        assert_ne!(assessment["state"], "retired");
    }
}

#[test]
fn malformed_manifest_fails_as_invalid_decision_shape() {
    let root = temp_dir("malformed");
    let manifest_file = root.join("malformed.yaml");
    std::fs::write(
        &manifest_file,
        "schema_version: '0.1'\nworkflow_governance_release_manifest:\n  caller_authority: executable\n",
    )
    .expect("malformed manifest");
    let output = run_rollout(&manifest_file, None);
    assert_eq!(output.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("failure envelope");
    assert_eq!(envelope["command"], "guide.rollout-audit");
    assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    assert!(envelope.get("data").is_none());
}

#[test]
fn unreadable_manifest_is_an_environment_configuration_error() {
    let missing = temp_dir("missing-manifest").join("absent.yaml");
    let output = run_rollout(&missing, None);
    assert_eq!(output.status.code(), Some(5));
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("failure envelope");
    assert_eq!(envelope["command"], "guide.rollout-audit");
    assert_eq!(envelope["exit_reason"], "env_config");
    assert!(envelope.get("data").is_none());
}

#[test]
fn semantically_blocked_rollout_returns_typed_candidate_only_rejection() {
    let fixture = build_rollout_fixture();
    let manifest_text =
        std::fs::read_to_string(&fixture.manifest_file).expect("canonical manifest");
    let mut blocked: WorkflowGovernanceReleaseManifestDocument =
        yaml_serde::from_str(&manifest_text).expect("typed canonical manifest");
    blocked
        .workflow_governance_release_manifest
        .legacy_catalog_digest = format!("sha256:{}", "0".repeat(64));
    let root = temp_dir("blocked");
    let blocked_manifest = root.join("blocked-release.yaml");
    std::fs::write(
        &blocked_manifest,
        yaml_serde::to_string(&blocked).expect("blocked manifest YAML"),
    )
    .expect("blocked manifest");

    let output = run_rollout(&blocked_manifest, Some(&fixture.batch_file));
    assert_eq!(output.status.code(), Some(2));
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("rejection envelope");
    assert_eq!(envelope["exit_reason"], "rejected_by_gate");
    assert_eq!(envelope["data"]["status"], "blocked");
    assert_eq!(envelope["data"]["authority"], "candidate_only");
    assert!(envelope["data"]["issues"]
        .as_array()
        .is_some_and(|issues| !issues.is_empty()));
}

#[test]
fn help_unknown_flags_and_missing_manifest_use_canonical_command_surface() {
    let help = bin()
        .args(["guide", "rollout-audit", "--help"])
        .output()
        .expect("rollout help");
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("guide rollout-audit --manifest-file <yaml>"));
    assert!(stdout.contains("[--batch-file <yaml>]..."));

    for args in [
        vec!["guide", "rollout-audit"],
        vec!["guide", "rollout-audit", "--grant-authority"],
    ] {
        let output = bin().args(args).output().expect("usage rejection");
        assert_eq!(output.status.code(), Some(3));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("guide rollout-audit --manifest-file <yaml>"));
    }
}
