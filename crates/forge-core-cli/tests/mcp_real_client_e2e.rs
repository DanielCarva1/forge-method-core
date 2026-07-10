use assert_cmd::Command as AssertCommand;
use forge_core_contracts::{
    claim::ActorRole, tool_effect::EffectTargetKind, AssuranceCaseDocument, ClaimContractDocument,
    OperationContractDocument, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::unix_to_rfc3339;
use forge_core_store::claim_wal::{append_claim_wal_record, ClaimWalOperation};
use forge_core_store::sha256_content_hash;
use rmcp::model::{CallToolRequestParams, Meta};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CREDENTIAL_ID: &str = "key.agent.real-client";
const AUDIENCE: &str = "forge-core:mcp:real-client";
const OPERATION_REF: &str = "operation.yaml";
const ASSURANCE_REF: &str = "contracts/assurance/case.yaml";
const EFFECT_REF: &str = "contracts/effects/story-artifact-write-effect.yaml";
const CLAIM_REF: &str = "contracts/claims/story-v2-010-active-claim.yaml";
const EFFECT_TARGET: &str = ".forge-method/artifacts/p4b4d-real-client.yaml";

fn bin() -> AssertCommand {
    AssertCommand::cargo_bin("forge-core").expect("forge-core binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fresh_parent(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "forge-mcp-real-client-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("fresh parent");
    path
}

struct TrustedFixture {
    project: PathBuf,
    state_root: PathBuf,
    registry: PathBuf,
    secret_dir: PathBuf,
    allowlist: PathBuf,
    policy: PathBuf,
    client_config: PathBuf,
    arguments: serde_json::Map<String, Value>,
    attestation: Value,
    target: PathBuf,
}

#[allow(clippy::too_many_lines)] // one end-to-end setup keeps every trust input visible
fn trusted_fixture() -> TrustedFixture {
    let parent = fresh_parent("mutation");
    let project = parent.join("consumer");
    let state_root = parent.join("runtime/.forge-method");
    for dir in [
        "contracts/assurance",
        "contracts/effects",
        "contracts/claims",
        "payloads",
    ] {
        fs::create_dir_all(project.join(dir)).expect("project directory");
    }
    fs::write(project.join("README.md"), "# real MCP client fixture\n").expect("readme");

    let source = repo_root();
    let assurance_text = fs::read_to_string(
        source.join("contracts/assurance/representative-slice-verified-assurance.yaml"),
    )
    .expect("assurance fixture");
    let assurance: AssuranceCaseDocument =
        yaml_serde::from_str(&assurance_text).expect("typed assurance");
    let state_version = assurance.assurance_case.project_snapshot.state_version;
    fs::write(project.join(ASSURANCE_REF), assurance_text).expect("assurance");
    let mut operation: OperationContractDocument = yaml_serde::from_str(
        &fs::read_to_string(
            source.join("docs/fixtures/operation-contract-v0/execute-trivial-write.yaml"),
        )
        .expect("operation fixture"),
    )
    .expect("typed operation");
    operation.operation_contract.project_ref.state_version = state_version;
    operation
        .operation_contract
        .coordination_scope
        .concurrency
        .expected_state_version = state_version;
    operation
        .operation_contract
        .coordination_scope
        .concurrency
        .agent_id = Some(StableId("agent".to_owned()));
    fs::write(
        project.join(OPERATION_REF),
        yaml_serde::to_string(&operation).expect("operation YAML"),
    )
    .expect("operation");

    let mut effect: ToolEffectContractDocument =
        yaml_serde::from_str(&fs::read_to_string(source.join(EFFECT_REF)).expect("effect fixture"))
            .expect("typed effect");
    effect.tool_effect_contract.actor.agent_id = StableId("agent".to_owned());
    effect
        .tool_effect_contract
        .operation_ref
        .clone_from(&operation.operation_contract.contract_id);
    effect
        .tool_effect_contract
        .read_set
        .retain(|read| read.target_kind != EffectTargetKind::FilePath);
    effect.tool_effect_contract.write_set.truncate(1);
    effect.tool_effect_contract.write_set[0].target_kind = EffectTargetKind::FilePath;
    effect.tool_effect_contract.write_set[0]
        .reference
        .clone_from(&EFFECT_TARGET.to_owned());
    fs::write(
        project.join(EFFECT_REF),
        yaml_serde::to_string(&effect).expect("effect YAML"),
    )
    .expect("effect");

    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_secs(),
    )
    .expect("unix seconds fit i64");
    let mut claim: ClaimContractDocument =
        yaml_serde::from_str(&fs::read_to_string(source.join(CLAIM_REF)).expect("claim fixture"))
            .expect("typed claim");
    claim.claim_contract.claim.claimant_agent_id = StableId("agent".to_owned());
    claim.claim_contract.claim.claimant_role = ActorRole::Driver;
    claim.claim_contract.scope.paths = vec![forge_core_contracts::RepoPath(
        ".forge-method/artifacts".to_owned(),
    )];
    claim.claim_contract.lease.acquired_at = unix_to_rfc3339(now - 10);
    claim.claim_contract.lease.last_heartbeat_at = unix_to_rfc3339(now - 5);
    claim.claim_contract.lease.expires_at = unix_to_rfc3339(now + 600);
    claim.claim_contract.lease.expected_state_version = state_version;
    claim.claim_contract.status.evaluated_at = unix_to_rfc3339(now);
    fs::write(
        project.join(CLAIM_REF),
        yaml_serde::to_string(&claim).expect("claim YAML"),
    )
    .expect("claim");

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
    assert_success(&init, "project init");
    append_claim_wal_record(
        &state_root,
        ClaimWalOperation::Acquire,
        &claim.claim_contract,
        &unix_to_rfc3339(now),
    )
    .expect("claim WAL acquire");

    let registry = parent.join("operator/principal-registry.yaml");
    let secret_dir = parent.join("operator/secrets");
    let provision = bin()
        .args(["mcp", "credential", "provision", "--root"])
        .arg(&project)
        .arg("--registry")
        .arg(&registry)
        .arg("--secret-dir")
        .arg(&secret_dir)
        .args([
            "--credential-id",
            CREDENTIAL_ID,
            "--principal-id",
            "principal.agent",
            "--agent-id",
            "agent",
            "--role",
            "driver",
            "--audience",
            AUDIENCE,
            "--json",
        ])
        .output()
        .expect("provision credential");
    assert_success(&provision, "credential provision");

    let snapshot = bin()
        .args(["mcp", "snapshot", "--root"])
        .arg(&project)
        .args(["--operation", OPERATION_REF, "--assurance", ASSURANCE_REF])
        .arg("--principal-registry")
        .arg(&registry)
        .args([
            "--credential-id",
            CREDENTIAL_ID,
            "--nonce",
            "real-client-nonce-00000001",
            "--json",
        ])
        .output()
        .expect("snapshot");
    assert_success(&snapshot, "snapshot generation");

    let payload_path = project.join("payloads/result.yaml");
    let payload = b"p4b4d: official client applied\n";
    fs::write(&payload_path, payload).expect("payload");
    let payload_binding = format!(
        "{EFFECT_TARGET}=payloads/result.yaml#{}",
        sha256_content_hash(payload)
    );
    let arguments = serde_json::json!({
        "--operation": OPERATION_REF,
        "--effect": EFFECT_REF,
        "--payload": payload_binding
    })
    .as_object()
    .expect("argument object")
    .clone();
    let arguments_path = parent.join("operator/arguments.json");
    fs::write(
        &arguments_path,
        serde_json::to_vec(&arguments).expect("arguments JSON"),
    )
    .expect("arguments file");
    let signed = bin()
        .args(["mcp", "credential", "sign", "--root"])
        .arg(&project)
        .arg("--registry")
        .arg(&registry)
        .arg("--secret-dir")
        .arg(&secret_dir)
        .args(["--credential-id", CREDENTIAL_ID, "--snapshot"])
        .arg("runtime/mcp-execution-snapshot.yaml")
        .arg("--arguments-json")
        .arg(&arguments_path)
        .arg("--json")
        .output()
        .expect("sign call");
    assert_success(&signed, "call signing");
    let signed_json: Value = serde_json::from_slice(&signed.stdout).expect("signed envelope");
    let attestation = signed_json["data"]["mcp_meta"]["attestation"].clone();

    let allowlist = parent.join("operator/allowlist.yaml");
    fs::copy(
        source.join("contracts/examples/mcp-trusted-single-effect-allowlist.yaml"),
        &allowlist,
    )
    .expect("allowlist");
    let policy = parent.join("operator/deployment-policy.yaml");
    fs::write(
        &policy,
        format!(
            r#"schema_version: "0.1"
mcp_deployment_policy:
  id: "trusted-real-client"
  mode: "trusted_single_effect"
  required_audience: "{AUDIENCE}"
  mutating_tools: ["execute-operation"]
  startup_reconciliation: "required_before_listen"
  material_loading: "canonical_project_bound"
  snapshot_loading: "bounded_local_read_only"
  effect_scope: "single_effect"
  public_mutation: "explicit_opt_in"
  root_binding: "canonical_configured_root"
  state_root_binding: "project_link_resolved"
  required_commit_protocol: "execution_provenance_commit_v0@0.1"
  same_user_boundary_acknowledged: true
"#
        ),
    )
    .expect("policy");
    let client_config = parent.join("operator/client-config.json");
    let readiness = bin()
        .args(["mcp", "readiness", "--root"])
        .arg(&project)
        .arg("--allowlist")
        .arg(&allowlist)
        .arg("--principal-registry")
        .arg(&registry)
        .arg("--deployment-policy")
        .arg(&policy)
        .args([
            "--snapshot",
            "runtime/mcp-execution-snapshot.yaml",
            "--secret-dir",
        ])
        .arg(&secret_dir)
        .args(["--credential-id", CREDENTIAL_ID, "--client-config-output"])
        .arg(&client_config)
        .arg("--json")
        .output()
        .expect("readiness");
    assert_success(&readiness, "readiness");

    let target = state_root.join("artifacts/p4b4d-real-client.yaml");
    TrustedFixture {
        project,
        state_root,
        registry,
        secret_dir,
        allowlist,
        policy,
        client_config,
        arguments,
        attestation,
        target,
    }
}

fn assert_success(output: &std::process::Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn official_rmcp_client_initializes_lists_and_calls_real_stdio_server() {
    let binary = assert_cmd::cargo::cargo_bin("forge-core");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = tokio::process::Command::new(binary);
    command
        .args(["mcp", "serve", "--root"])
        .arg(&root)
        .kill_on_drop(true);
    let transport = TokioChildProcess::new(command).expect("child-process transport");
    let client = ().serve(transport).await.expect("initialize MCP client");
    let tools = client.list_all_tools().await.expect("tools/list");
    assert!(tools.iter().any(|tool| tool.name == "assurance"));
    let arguments = serde_json::json!({
        "--input-file": "docs/fixtures/obligation-engine-v0/verified-release.yaml"
    })
    .as_object()
    .expect("arguments object")
    .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("assurance").with_arguments(arguments))
        .await
        .expect("tools/call");
    assert_ne!(result.is_error, Some(true), "assurance result: {result:?}");
    let text = result
        .content
        .first()
        .and_then(|content| content.as_text())
        .expect("text result");
    assert!(text.text.contains("\"assurance_case\""));
    client.cancel().await.expect("close client");
}

#[tokio::test(flavor = "multi_thread")]
async fn generated_config_drives_signed_mutation_through_official_rmcp_client() {
    let fixture = trusted_fixture();
    let config: Value =
        serde_json::from_slice(&fs::read(&fixture.client_config).expect("generated client config"))
            .expect("client config JSON");
    let server = &config["mcpServers"]["forge-method"];
    let command_path = server["command"].as_str().expect("config command");
    let args = server["args"]
        .as_array()
        .expect("config args")
        .iter()
        .map(|arg| arg.as_str().expect("string arg"))
        .collect::<Vec<_>>();
    let mut command = tokio::process::Command::new(command_path);
    command.args(args).kill_on_drop(true);
    let transport = TokioChildProcess::new(command).expect("generated child-process transport");
    let client = ().serve(transport).await.expect("initialize trusted MCP client");
    let tools = client.list_all_tools().await.expect("trusted tools/list");
    assert_eq!(
        tools
            .iter()
            .filter(|tool| tool.name == "execute-operation")
            .count(),
        1
    );

    let mut meta = serde_json::Map::new();
    meta.insert("attestation".to_owned(), fixture.attestation.clone());
    let mut request =
        CallToolRequestParams::new("execute-operation").with_arguments(fixture.arguments.clone());
    request.meta = Some(Meta(meta));
    let result = client.call_tool(request).await.expect("trusted tools/call");
    assert_ne!(result.is_error, Some(true), "mutation result: {result:?}");
    let text = result
        .content
        .first()
        .and_then(|content| content.as_text())
        .expect("mutation text result");
    let envelope: Value = serde_json::from_str(&text.text).expect("mutation envelope JSON");
    assert_eq!(envelope["status"], "applied");
    assert_eq!(
        fs::read_to_string(&fixture.target).expect("committed target"),
        "p4b4d: official client applied\n"
    );
    assert!(fixture.state_root.join("wal/replay.fmr1").exists());
    assert!(fixture.state_root.join("wal/effects.ndjson").exists());
    assert!(
        !fixture.project.join(".forge-method").exists(),
        "trusted mutation must preserve the Project Link sidecar boundary"
    );
    assert!(fixture.registry.exists());
    assert!(fixture.secret_dir.exists());
    assert!(fixture.allowlist.exists());
    assert!(fixture.policy.exists());
    assert!(fixture.project.exists());
    client.cancel().await.expect("close trusted client");
}
