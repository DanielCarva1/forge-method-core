use assert_cmd::Command;
use forge_core_contracts::{AssuranceCaseDocument, OperationContractDocument};
use forge_core_protocol_mcp::McpLocalExecutionSnapshotDocument;
use forge_core_protocol_mcp::{
    AttestationInput, AttestationPolicy, AttestationVerifier, CanonicalIntent,
    PrincipalCredentialStatus, PrincipalRegistryDocument,
};
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

#[test]
#[allow(clippy::too_many_lines)] // one linear lifecycle keeps authority transitions auditable
fn credential_lifecycle_keeps_private_key_out_of_output_and_project() {
    let parent = fresh_parent("credential-lifecycle");
    let (project, state_root) = prepare_project(&parent);
    let registry_path = parent.join("operator/registry.yaml");
    let secret_dir = parent.join("operator/secrets");
    let provision = bin()
        .args(["mcp", "credential", "provision", "--root"])
        .arg(&project)
        .args(["--registry"])
        .arg(&registry_path)
        .args(["--secret-dir"])
        .arg(&secret_dir)
        .args([
            "--credential-id",
            "key.agent.1",
            "--principal-id",
            "principal.agent",
            "--agent-id",
            "agent",
            "--role",
            "driver",
            "--audience",
            "forge-core:mcp:test",
            "--json",
        ])
        .output()
        .expect("provision credential");
    assert!(
        provision.status.success(),
        "provision failed: {}",
        String::from_utf8_lossy(&provision.stdout)
    );
    let provision_text = String::from_utf8(provision.stdout).expect("UTF-8 envelope");
    assert!(!provision_text.contains("private_key"));
    assert!(!provision_text.contains("secret_key"));
    assert!(!project.join("operator").exists());
    let secrets = fs::read_dir(&secret_dir)
        .expect("secret dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("secret entries");
    assert_eq!(secrets.len(), 1);
    assert_eq!(secrets[0].metadata().expect("secret metadata").len(), 32);

    let snapshot = bin()
        .args(["mcp", "snapshot", "--root"])
        .arg(&project)
        .args([
            "--operation",
            "operation.yaml",
            "--assurance",
            "contracts/assurance/case.yaml",
            "--principal-registry",
        ])
        .arg(&registry_path)
        .args([
            "--credential-id",
            "key.agent.1",
            "--nonce",
            "0123456789abcdef",
            "--json",
        ])
        .output()
        .expect("snapshot with provisioned credential");
    assert!(snapshot.status.success());
    let arguments_path = parent.join("arguments.json");
    fs::write(
        &arguments_path,
        r#"{"--operation":"operation.yaml","--effect":"contracts/effects/file-delete-restore-inverse-effect.yaml"}"#,
    )
    .expect("arguments JSON");
    let signed = bin()
        .args(["mcp", "credential", "sign", "--root"])
        .arg(&project)
        .args(["--registry"])
        .arg(&registry_path)
        .args(["--secret-dir"])
        .arg(&secret_dir)
        .args(["--credential-id", "key.agent.1", "--snapshot"])
        .arg("runtime/mcp-execution-snapshot.yaml")
        .args(["--arguments-json"])
        .arg(&arguments_path)
        .arg("--json")
        .output()
        .expect("sign intent");
    assert!(
        signed.status.success(),
        "sign failed: {}",
        String::from_utf8_lossy(&signed.stdout)
    );
    let signed_json: Value = serde_json::from_slice(&signed.stdout).expect("signed envelope");
    let attestation: AttestationInput =
        serde_json::from_value(signed_json["data"]["mcp_meta"]["attestation"].clone())
            .expect("attestation");
    let arguments: Value =
        serde_json::from_str(&fs::read_to_string(&arguments_path).expect("arguments"))
            .expect("arguments value");
    let intent = CanonicalIntent {
        tool: "execute-operation".to_owned(),
        arguments,
        credential_id: attestation.credential_id.clone(),
        audience: attestation.audience.clone(),
        execution_intent_digest: attestation.execution_intent_digest.clone(),
        nonce: attestation.nonce.clone(),
        ts: attestation.ts,
    };
    AttestationVerifier::new(AttestationPolicy::Default)
        .verify(&intent, &attestation)
        .expect("generated signature verifies");

    let allowlist_path = parent.join("operator/allowlist.yaml");
    fs::copy(
        repo_root().join("contracts/examples/mcp-trusted-single-effect-allowlist.yaml"),
        &allowlist_path,
    )
    .expect("allowlist");
    let policy_path = parent.join("operator/deployment-policy.yaml");
    fs::write(
        &policy_path,
        r#"schema_version: "0.1"
mcp_deployment_policy:
  id: "trusted-test"
  mode: "trusted_single_effect"
  required_audience: "forge-core:mcp:test"
  mutating_tools: ["execute-operation"]
  startup_reconciliation: "required_before_listen"
  material_loading: "canonical_project_bound"
  snapshot_loading: "bounded_local_read_only"
  effect_scope: "single_effect"
  public_mutation: "explicit_opt_in"
  root_binding: "canonical_configured_root"
  state_root_binding: "project_link_resolved"
  replay_rollback_protection: "external_monotonic_head"
  required_commit_protocol: "execution_provenance_commit_v0@0.1"
  same_user_boundary_acknowledged: true
"#,
    )
    .expect("policy");
    let replay_anchor_path = parent.join("operator/replay-anchor.json");
    let anchor = bin()
        .args(["mcp", "replay-anchor", "provision", "--root"])
        .arg(&project)
        .arg("--anchor")
        .arg(&replay_anchor_path)
        .args(["--deployment-id", "trusted-test", "--json"])
        .output()
        .expect("provision replay anchor");
    assert!(
        anchor.status.success(),
        "anchor failed: {}",
        String::from_utf8_lossy(&anchor.stdout)
    );
    let client_config = parent.join("operator/client-config.json");
    let readiness = || {
        bin()
            .args(["mcp", "readiness", "--root"])
            .arg(&project)
            .args(["--allowlist"])
            .arg(&allowlist_path)
            .args(["--principal-registry"])
            .arg(&registry_path)
            .args(["--deployment-policy"])
            .arg(&policy_path)
            .args([
                "--snapshot",
                "runtime/mcp-execution-snapshot.yaml",
                "--replay-anchor",
            ])
            .arg(&replay_anchor_path)
            .args(["--secret-dir"])
            .arg(&secret_dir)
            .args(["--credential-id", "key.agent.1", "--client-config-output"])
            .arg(&client_config)
            .arg("--json")
            .output()
            .expect("readiness")
    };
    let first_readiness = readiness();
    assert!(
        first_readiness.status.success(),
        "readiness failed: {}",
        String::from_utf8_lossy(&first_readiness.stdout)
    );
    let readiness_json: Value =
        serde_json::from_slice(&first_readiness.stdout).expect("readiness JSON");
    assert_eq!(readiness_json["data"]["verdict"], "ready");
    assert!(client_config.exists());
    let second_readiness = readiness();
    assert!(second_readiness.status.success());
    let second_json: Value =
        serde_json::from_slice(&second_readiness.stdout).expect("replacement readiness JSON");
    assert_eq!(
        readiness_json["data"]["checks"],
        second_json["data"]["checks"]
    );

    let rotate = bin()
        .args(["mcp", "credential", "rotate", "--root"])
        .arg(&project)
        .args(["--registry"])
        .arg(&registry_path)
        .args(["--secret-dir"])
        .arg(&secret_dir)
        .args([
            "--credential-id",
            "key.agent.2",
            "--replaces",
            "key.agent.1",
            "--principal-id",
            "principal.agent",
            "--agent-id",
            "agent",
            "--role",
            "driver",
            "--audience",
            "forge-core:mcp:test",
            "--json",
        ])
        .output()
        .expect("rotate credential");
    assert!(rotate.status.success());
    let registry: PrincipalRegistryDocument =
        yaml_serde::from_str(&fs::read_to_string(&registry_path).expect("registry"))
            .expect("typed registry");
    assert_eq!(registry.principal_registry.principals.len(), 2);
    assert_eq!(
        registry.principal_registry.principals[0].status,
        PrincipalCredentialStatus::Revoked
    );
    assert_eq!(
        registry.principal_registry.principals[1].status,
        PrincipalCredentialStatus::Active
    );
    assert_eq!(fs::read_dir(&secret_dir).expect("secrets").count(), 1);

    let revoke = bin()
        .args(["mcp", "credential", "revoke", "--root"])
        .arg(&project)
        .args(["--registry"])
        .arg(&registry_path)
        .args(["--secret-dir"])
        .arg(&secret_dir)
        .args(["--credential-id", "key.agent.2", "--json"])
        .output()
        .expect("revoke credential");
    assert!(revoke.status.success());
    assert_eq!(fs::read_dir(secret_dir).expect("secret dir").count(), 0);
    assert!(state_root
        .join("runtime/mcp-execution-snapshot.yaml")
        .exists());
}
