use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn agent_receives_complete_p5a_manifest_from_one_read_only_command() {
    let output = bin()
        .args(["guide", "migration-audit", "--catalog-dir"])
        .arg(repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog"))
        .arg("--plan-file")
        .arg(repo_root().join("contracts/policies/workflow-migration-foundation-v0.yaml"))
        .arg("--json")
        .output()
        .expect("migration audit command");
    assert!(
        output.status.success(),
        "migration audit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("migration audit envelope");
    assert_eq!(envelope["command"], "guide.migration-audit");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["status"], "ready_for_shadow");
    assert_eq!(envelope["data"]["catalog_count"], 110);
    assert_eq!(envelope["data"]["classified_count"], 110);
    assert_eq!(envelope["data"]["shadow_parity"]["equivalent_count"], 110);
    assert_eq!(envelope["data"]["shadow_parity"]["drift_count"], 0);
    assert_eq!(envelope["data"]["shadow_parity"]["mutation_allowed"], false);
    assert_eq!(
        envelope["data"]["deletion_baseline"]["retirement_allowed"],
        false
    );
    assert_eq!(
        envelope["data"]["manifest"]["entries"]
            .as_array()
            .expect("manifest entries")
            .len(),
        110
    );
    assert!(envelope["data"]["manifest"]["manifest_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
}

#[test]
fn malformed_plan_fails_closed_before_manifest_projection() {
    let root = std::env::temp_dir().join(format!("forge-p5a-invalid-plan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    let plan = root.join("invalid.yaml");
    std::fs::write(
        &plan,
        "schema_version: '0.1'\nworkflow_migration_plan:\n  caller_authority: true\n",
    )
    .expect("invalid plan");
    let output = bin()
        .args(["guide", "migration-audit", "--catalog-dir"])
        .arg(repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog"))
        .arg("--plan-file")
        .arg(&plan)
        .arg("--json")
        .output()
        .expect("invalid migration audit command");
    assert_eq!(output.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("failure envelope");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    assert!(envelope.get("data").is_none());
}

#[test]
fn installed_agent_can_audit_from_embedded_contracts_without_repo_files() {
    let root = std::env::temp_dir().join(format!("forge-p5a-zero-config-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    let output = bin()
        .current_dir(&root)
        .args(["guide", "migration-audit", "--json"])
        .output()
        .expect("zero-config migration audit command");
    assert!(
        output.status.success(),
        "zero-config audit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("migration audit envelope");
    assert_eq!(envelope["data"]["status"], "ready_for_shadow");
    assert_eq!(envelope["data"]["catalog_count"], 110);
}
