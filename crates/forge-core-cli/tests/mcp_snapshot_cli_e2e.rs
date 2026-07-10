use assert_cmd::Command;
use forge_core_contracts::{AssuranceCaseDocument, OperationContractDocument};
use forge_core_protocol_mcp::McpLocalExecutionSnapshotDocument;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fresh_parent(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("mcp-snapshot-cli-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("fresh parent");
    path
}

fn prepare_project(parent: &Path) -> (PathBuf, PathBuf) {
    let project = parent.join("consumer");
    let state_root = parent.join("runtime/.forge-method");
    fs::create_dir_all(project.join("contracts/effects")).expect("effects dir");
    fs::create_dir_all(project.join("contracts/assurance")).expect("assurance dir");
    fs::write(project.join("README.md"), "# consumer\n").expect("readme");
    let source = repo_root();
    let assurance_text = fs::read_to_string(
        source.join("contracts/assurance/representative-slice-verified-assurance.yaml"),
    )
    .expect("assurance fixture");
    let assurance: AssuranceCaseDocument =
        yaml_serde::from_str(&assurance_text).expect("typed assurance");
    let mut operation: OperationContractDocument = yaml_serde::from_str(
        &fs::read_to_string(
            source.join("docs/fixtures/operation-contract-v0/destructive-effect-with-inverse.yaml"),
        )
        .expect("operation fixture"),
    )
    .expect("typed operation");
    let state_version = assurance.assurance_case.project_snapshot.state_version;
    operation.operation_contract.project_ref.state_version = state_version;
    operation
        .operation_contract
        .coordination_scope
        .concurrency
        .expected_state_version = state_version;
    fs::write(
        project.join("operation.yaml"),
        yaml_serde::to_string(&operation).expect("operation yaml"),
    )
    .expect("operation");
    fs::write(
        project.join("contracts/assurance/case.yaml"),
        assurance_text,
    )
    .expect("assurance");
    fs::copy(
        source.join("contracts/effects/file-delete-restore-inverse-effect.yaml"),
        project.join("contracts/effects/file-delete-restore-inverse-effect.yaml"),
    )
    .expect("effect");
    let init = bin()
        .args(["project", "init", "--root"])
        .arg(&project)
        .args(["--sidecar-root"])
        .arg(parent.join("runtime"))
        .args(["--state-root"])
        .arg(&state_root)
        .arg("--json")
        .output()
        .expect("project init");
    assert!(
        init.status.success(),
        "project init failed: {}",
        String::from_utf8_lossy(&init.stdout)
    );
    (project, state_root)
}

fn registry(parent: &Path) -> PathBuf {
    let path = parent.join("principal-registry.yaml");
    fs::write(
        &path,
        r#"schema_version: "0.1"
principal_registry:
  audience: "forge-core:mcp:test"
  principals:
    - credential_id: "key.agent.test"
      principal_id: "principal.agent"
      agent_id: "agent"
      role: "driver"
      public_key_hex: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
      allowed_tools: ["execute-operation"]
      authority_grants: ["operation.execute"]
      status: "active"
"#,
    )
    .expect("registry");
    path
}

#[test]
fn agent_command_generates_and_atomically_refreshes_content_bound_snapshot() {
    let parent = fresh_parent("generate");
    let (project, state_root) = prepare_project(&parent);
    let registry = registry(&parent);
    let run = || {
        bin()
            .args(["mcp", "snapshot", "--root"])
            .arg(&project)
            .args([
                "--operation",
                "operation.yaml",
                "--assurance",
                "contracts/assurance/case.yaml",
                "--principal-registry",
            ])
            .arg(&registry)
            .args([
                "--credential-id",
                "key.agent.test",
                "--nonce",
                "0123456789abcdef",
                "--now-unix",
                "1800000000",
                "--json",
            ])
            .output()
            .expect("mcp snapshot")
    };
    let first = run();
    assert!(
        first.status.success(),
        "snapshot failed: {}",
        String::from_utf8_lossy(&first.stdout)
    );
    let envelope: Value = serde_json::from_slice(&first.stdout).expect("JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert!(envelope["data"]["execution_intent_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    let snapshot_path = state_root.join("runtime/mcp-execution-snapshot.yaml");
    let first_bytes = fs::read(&snapshot_path).expect("generated snapshot");
    let snapshot: McpLocalExecutionSnapshotDocument =
        yaml_serde::from_slice(&first_bytes).expect("typed generated snapshot");
    assert!(!snapshot
        .execution_snapshot
        .admission_request
        .authority_snapshot_token
        .is_empty());
    let second = run();
    assert!(second.status.success());
    assert_eq!(
        first_bytes,
        fs::read(snapshot_path).expect("refreshed snapshot")
    );
}

#[test]
fn snapshot_output_escape_fails_closed() {
    let parent = fresh_parent("escape");
    let (project, _) = prepare_project(&parent);
    let registry = registry(&parent);
    let output = bin()
        .args(["mcp", "snapshot", "--root"])
        .arg(&project)
        .args([
            "--operation",
            "operation.yaml",
            "--assurance",
            "contracts/assurance/case.yaml",
            "--principal-registry",
        ])
        .arg(&registry)
        .args([
            "--credential-id",
            "key.agent.test",
            "--nonce",
            "0123456789abcdef",
            "--output",
            "../escape.yaml",
            "--now-unix",
            "1800000000",
            "--json",
        ])
        .output()
        .expect("escaped snapshot");
    assert!(!output.status.success());
    assert!(!parent.join("runtime/escape.yaml").exists());
}
