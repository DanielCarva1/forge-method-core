use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    AttestationInput, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry,
    CanonicalIntent, ExecutionExecutor, ExecutionPayloadBinding, ExecutionRequest,
    PrincipalCredentialStatus, PrincipalRegistryContract, PrincipalRegistryDocument,
    PrincipalRegistryEntry, VerifiedExecutionCall, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{
    AssuranceCaseDocument, CommandContractDocument, OperationContractDocument, PrincipalId,
    StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, command_contract_token, effect_contract_token, execution_intent_digest,
    operation_contract_token, ClaimSnapshotObservation, ContentAddressedBinding,
    ExecutionAdmissionRequest, GateSnapshotObservation, SnapshotCompleteness,
};
use forge_core_protocol_mcp::{
    DormantTrustedMcpExecutor, LocalMcpSnapshotSource, McpDeploymentPolicyDocument,
    McpLocalExecutionSnapshot, McpLocalExecutionSnapshotDocument, TrustedMcpLoadError,
    TrustedMcpLoaderLimits, TrustedMcpMaterialLoader, ValidatedMcpDeploymentPolicy,
    MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION,
};
use forge_core_store::sha256_content_hash;
use serde_json::{Map, Value};

const AUDIENCE: &str = "forge-local";
const NOW: i64 = 1_800_000_000;
const NONCE: &str = "p4b3b-loader-nonce-0001";
const OPERATION_REF: &str = "contracts/operation.yaml";
const COMMAND_REF: &str = "contracts/command.yaml";
const EFFECT_REF: &str = "contracts/effect.yaml";
const RISK_REF: &str = "contracts/risk.yaml";
const SNAPSHOT_REF: &str = ".forge-method/runtime/mcp-snapshot.yaml";

struct LoaderFixture {
    root: PathBuf,
    loader: TrustedMcpMaterialLoader,
    call: VerifiedExecutionCall,
    first_payload: PathBuf,
    operation: PathBuf,
    risk_audit: PathBuf,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_repo_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let path = repo_root().join(relative);
    let yaml = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    yaml_serde::from_str(&yaml).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn fresh_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "forge-mcp-trusted-loader-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("contracts")).expect("contracts");
    fs::create_dir_all(root.join("payloads")).expect("payloads");
    fs::create_dir_all(root.join(".forge-method/runtime")).expect("runtime");
    root
}

fn trusted_policy() -> ValidatedMcpDeploymentPolicy {
    let yaml = r#"
schema_version: "0.1"
mcp_deployment_policy:
  id: "trusted-local-single-effect"
  mode: "trusted_single_effect"
  required_audience: "forge-local"
  mutating_tools: ["execute-operation"]
  startup_reconciliation: "required_before_listen"
  material_loading: "canonical_project_bound"
  snapshot_loading: "bounded_local_read_only"
  effect_scope: "single_effect"
  public_mutation: "explicit_opt_in"
  root_binding: "canonical_configured_root"
  required_commit_protocol: "execution_provenance_commit_v0@0.1"
  same_user_boundary_acknowledged: true
"#;
    ValidatedMcpDeploymentPolicy::from_yaml(yaml).expect("trusted policy")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("write to string");
        output
    })
}

#[allow(clippy::too_many_lines)] // one linear cryptographic fixture keeps all signed bindings visible
fn fixture(label: &str, bind_payload_digests: bool) -> LoaderFixture {
    let root = fresh_root(label);
    let operation: OperationContractDocument =
        parse_repo_yaml("docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml");
    let command: CommandContractDocument =
        parse_repo_yaml("contracts/commands/story-validation-fast.yaml");
    let effect: ToolEffectContractDocument =
        parse_repo_yaml("contracts/effects/story-artifact-write-effect.yaml");
    let assurance: AssuranceCaseDocument =
        parse_repo_yaml("contracts/assurance/representative-slice-verified-assurance.yaml");

    fs::write(
        root.join(OPERATION_REF),
        yaml_serde::to_string(&operation).expect("operation yaml"),
    )
    .expect("write operation");
    fs::write(
        root.join(COMMAND_REF),
        yaml_serde::to_string(&command).expect("command yaml"),
    )
    .expect("write command");
    fs::write(
        root.join(EFFECT_REF),
        yaml_serde::to_string(&effect).expect("effect yaml"),
    )
    .expect("write effect");
    fs::copy(
        repo_root().join("contracts/risk-audits/fail-soft.yaml"),
        root.join(RISK_REF),
    )
    .expect("copy risk audit");

    let mut payload_bindings = Vec::new();
    let mut first_payload = None;
    for (index, write) in effect.tool_effect_contract.write_set.iter().enumerate() {
        let relative = PathBuf::from(format!("payloads/payload-{index}.bin"));
        let content = format!("trusted payload {index}\n").into_bytes();
        fs::write(root.join(&relative), &content).expect("write payload");
        first_payload.get_or_insert_with(|| root.join(&relative));
        let binding = if bind_payload_digests {
            ExecutionPayloadBinding::new_verified(
                write.reference.clone(),
                relative,
                sha256_content_hash(&content),
            )
        } else {
            ExecutionPayloadBinding::new(write.reference.clone(), relative)
        };
        payload_bindings.push(binding);
    }

    let principal_id = PrincipalId("principal.codex-main".to_owned());
    let agent_id = StableId("codex-main".to_owned());
    let admission = ExecutionAdmissionRequest {
        id: StableId("admission.p4b3b.loader".to_owned()),
        principal_id: principal_id.clone(),
        agent_id: agent_id.clone(),
        principal_role: CallerRole::Driver,
        operation_id: operation.operation_contract.contract_id.clone(),
        operation_token: operation_contract_token(&operation).expect("operation token"),
        assurance_case_id: assurance.assurance_case.id.clone(),
        assurance_case_token: assurance_case_token(&assurance).expect("assurance token"),
        command_bindings: vec![ContentAddressedBinding {
            reference: command.command_contract.id.0.clone(),
            token: command_contract_token(&command).expect("command token"),
        }],
        effect_bindings: vec![ContentAddressedBinding {
            reference: EFFECT_REF.to_owned(),
            token: effect_contract_token(&effect).expect("effect token"),
        }],
        expected_claim_snapshot_revision: 11,
        expected_claim_revisions: Vec::new(),
        expected_gate_snapshot_revision: 5,
        expected_gate_revisions: Vec::new(),
        expected_replay_reservation_revision: 1,
        nonce: NONCE.to_owned(),
        issued_at_unix: NOW - 10,
    };
    let intent_digest = execution_intent_digest(&admission).expect("intent digest");
    let call = verified_call(principal_id, agent_id, &intent_digest, payload_bindings);
    let snapshot = McpLocalExecutionSnapshotDocument {
        schema_version: MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION.to_owned(),
        execution_snapshot: McpLocalExecutionSnapshot {
            admission_request: admission,
            assurance_case: assurance,
            claim_snapshot: ClaimSnapshotObservation {
                revision: 11,
                completeness: SnapshotCompleteness::Complete,
                claims: Vec::new(),
            },
            gate_snapshot: GateSnapshotObservation {
                revision: 5,
                completeness: SnapshotCompleteness::Complete,
                gates: Vec::new(),
            },
            current_state_version: 1,
            now_unix: NOW,
        },
    };
    fs::write(
        root.join(SNAPSHOT_REF),
        yaml_serde::to_string(&snapshot).expect("snapshot yaml"),
    )
    .expect("write snapshot");

    let loader = TrustedMcpMaterialLoader::new(
        trusted_policy(),
        &root,
        SNAPSHOT_REF,
        TrustedMcpLoaderLimits::default(),
    )
    .expect("loader");
    LoaderFixture {
        operation: root.join(OPERATION_REF),
        risk_audit: root.join(RISK_REF),
        first_payload: first_payload.expect("payload"),
        root,
        loader,
        call,
    }
}

fn verified_call(
    principal_id: PrincipalId,
    agent_id: StableId,
    intent_digest: &str,
    payloads: Vec<ExecutionPayloadBinding>,
) -> VerifiedExecutionCall {
    let signing_key = SigningKey::from_bytes(&[23; 32]);
    let public_key_hex = hex(signing_key.verifying_key().as_bytes());
    let registry = AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
        schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
        principal_registry: PrincipalRegistryContract {
            audience: AUDIENCE.to_owned(),
            principals: vec![PrincipalRegistryEntry {
                credential_id: "key.codex-main.p4b3b".to_owned(),
                principal_id,
                agent_id,
                role: CallerRole::Driver,
                public_key_hex: public_key_hex.clone(),
                allowed_tools: vec![StableId("execute-operation".to_owned())],
                authority_grants: vec![StableId("operation.execute".to_owned())],
                status: PrincipalCredentialStatus::Active,
            }],
        },
    })
    .expect("registry");
    let intent = CanonicalIntent {
        tool: "execute-operation".to_owned(),
        arguments: Value::Object(Map::new()),
        credential_id: Some("key.codex-main.p4b3b".to_owned()),
        audience: Some(AUDIENCE.to_owned()),
        execution_intent_digest: Some(intent_digest.to_owned()),
        nonce: NONCE.to_owned(),
        ts: NOW - 10,
    };
    let signature = signing_key.sign(&intent.canonical_bytes().expect("canonical intent"));
    let attestation = AttestationInput {
        credential_id: intent.credential_id.clone(),
        audience: intent.audience.clone(),
        execution_intent_digest: intent.execution_intent_digest.clone(),
        nonce: intent.nonce.clone(),
        ts: intent.ts,
        signature: hex(&signature.to_bytes()),
        public_key_hex,
    };
    let authorization = registry
        .authorize_execution(
            &AttestationVerifier::new(AttestationPolicy::Default),
            &intent,
            &attestation,
            NOW,
            300,
            30,
        )
        .expect("authorization");
    VerifiedExecutionCall::new(
        authorization,
        ExecutionRequest::new(
            PathBuf::from(OPERATION_REF),
            vec![PathBuf::from(COMMAND_REF)],
            Some(PathBuf::from(EFFECT_REF)),
            payloads,
            Some(PathBuf::from(RISK_REF)),
            false,
        ),
    )
}

#[test]
fn trusted_loader_loads_exact_typed_material_and_signed_payloads() {
    let fixture = fixture("valid", true);
    let loaded = fixture.loader.load(fixture.call).expect("trusted material");
    let audit = loaded.audit();
    assert_eq!(audit.command_count, 1);
    assert_eq!(audit.payload_count, 2);
    assert!(audit.risk_audit_loaded);
    assert_eq!(audit.claim_snapshot_revision, 11);
    assert_eq!(audit.gate_snapshot_revision, 5);
    assert!(audit
        .payload_hashes
        .iter()
        .all(|hash| hash.starts_with("sha256:")));
}

#[test]
fn trusted_loader_rejects_unsigned_and_changed_payload_bytes() {
    let unsigned = fixture("unsigned", false);
    assert!(matches!(
        unsigned.loader.load(unsigned.call),
        Err(TrustedMcpLoadError::PayloadDigestRequired { .. })
    ));

    let changed = fixture("changed", true);
    fs::write(&changed.first_payload, b"changed after signing\n").expect("tamper payload");
    assert!(matches!(
        changed.loader.load(changed.call),
        Err(TrustedMcpLoadError::PayloadDigestMismatch { .. })
    ));
}

#[test]
fn trusted_loader_rejects_unsafe_paths_oversize_and_open_yaml() {
    let path_fixture = fixture("path-bounds", true);
    assert!(matches!(
        LocalMcpSnapshotSource::new(&path_fixture.root, "../outside.yaml", 1024),
        Err(TrustedMcpLoadError::InvalidReference(_))
    ));

    let open_fixture = fixture("open-yaml", true);
    let mut yaml = fs::read_to_string(&open_fixture.operation).expect("operation");
    yaml.push_str("unknown_field: true\n");
    fs::write(&open_fixture.operation, yaml).expect("open operation");
    assert!(matches!(
        open_fixture.loader.load(open_fixture.call),
        Err(TrustedMcpLoadError::Parse { .. })
    ));

    let risk_fixture = fixture("open-risk-yaml", true);
    let mut risk_yaml = fs::read_to_string(&risk_fixture.risk_audit).expect("risk audit");
    risk_yaml.push_str("unknown_field: true\n");
    fs::write(&risk_fixture.risk_audit, risk_yaml).expect("open risk audit");
    assert!(matches!(
        risk_fixture.loader.load(risk_fixture.call),
        Err(TrustedMcpLoadError::Parse { .. })
    ));

    let size_fixture = fixture("oversize", true);
    let limits = TrustedMcpLoaderLimits {
        max_payload_bytes: 4,
        max_total_payload_bytes: 8,
        ..TrustedMcpLoaderLimits::default()
    };
    let loader =
        TrustedMcpMaterialLoader::new(trusted_policy(), &size_fixture.root, SNAPSHOT_REF, limits)
            .expect("small loader");
    assert!(matches!(
        loader.load(size_fixture.call),
        Err(TrustedMcpLoadError::SizeLimit { .. })
    ));
}

#[test]
fn dormant_executor_loads_but_never_activates_mutation() {
    let fixture = fixture("dormant", true);
    let executor = DormantTrustedMcpExecutor::new(fixture.loader);
    let rejection = executor
        .execute(fixture.call)
        .expect_err("must stay dormant");
    assert!(rejection.to_string().contains("remains dormant"));
    assert!(!fixture
        .root
        .join(".forge-method/wal/effects.ndjson")
        .exists());
}

#[test]
fn loader_requires_validated_trusted_policy() {
    let fixture = fixture("read-only-policy", true);
    let yaml =
        fs::read_to_string(repo_root().join("contracts/examples/mcp-deployment-policy.yaml"))
            .expect("read-only example");
    let document: McpDeploymentPolicyDocument = yaml_serde::from_str(&yaml).expect("policy yaml");
    let policy = ValidatedMcpDeploymentPolicy::from_document(document).expect("read-only policy");
    assert!(matches!(
        TrustedMcpMaterialLoader::new(
            policy,
            &fixture.root,
            SNAPSHOT_REF,
            TrustedMcpLoaderLimits::default()
        ),
        Err(TrustedMcpLoadError::TrustedPolicyRequired)
    ));
}
