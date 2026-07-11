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
use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::tool_effect::EffectTargetKind;
use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument,
    OperationContractDocument, PrincipalId, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, authority_snapshot_token, command_contract_token, effect_contract_token,
    execution_intent_digest, operation_contract_token, ClaimRevisionObservation,
    ClaimSnapshotObservation, ContentAddressedBinding, ExecutionAdmissionRequest,
    GateSnapshotObservation, RevisionExpectation, SnapshotCompleteness,
};
use forge_core_protocol_mcp::{
    DormantTrustedMcpExecutor, ExplicitTrustedOperationWideOptIn, ExplicitTrustedSingleEffectOptIn,
    LocalMcpSnapshotSource, McpDeploymentPolicyDocument, McpLocalExecutionSnapshot,
    McpLocalExecutionSnapshotDocument, ReconciledTrustedMcpDeployment, TrustedMcpLoadError,
    TrustedMcpLoaderLimits, TrustedMcpMaterialLoader, TrustedOperationWideMcpExecutor,
    TrustedSingleEffectMcpExecutor, ValidatedMcpDeploymentPolicy,
    MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION,
};
use forge_core_store::replay_anchor::{advance_replay_anchor, provision_replay_anchor};
use forge_core_store::replay_wal::{initialize_replay_wal, replay_wal_path, reserve_replay_nonce};
use forge_core_store::sha256_content_hash;
use forge_core_store::{EffectWalRecord, EffectWalStage};
use serde_json::{Map, Value};

const AUDIENCE: &str = "forge-local";
const NOW: i64 = 1_800_000_000;
const NONCE: &str = "p4b3b-loader-nonce-0001";
const OPERATION_REF: &str = "contracts/operation.yaml";
const COMMAND_REF: &str = "contracts/command.yaml";
const EFFECT_REF: &str = "contracts/effect.yaml";
const EFFECT_REF_2: &str = "contracts/effect-2.yaml";
const RISK_REF: &str = "contracts/risk.yaml";
const SNAPSHOT_REF: &str = ".forge-method/runtime/mcp-snapshot.yaml";
const CLAIM_REF: &str = "contracts/claim.yaml";

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
    fs::create_dir_all(root.join("output")).expect("output");
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
  state_root_binding: "project_link_resolved"
  replay_rollback_protection: "external_monotonic_head"
  required_commit_protocol: "execution_provenance_commit_v0@0.1"
  same_user_boundary_acknowledged: true
"#;
    ValidatedMcpDeploymentPolicy::from_yaml(yaml).expect("trusted policy")
}

fn trusted_operation_wide_policy() -> ValidatedMcpDeploymentPolicy {
    let yaml = r#"
schema_version: "0.1"
mcp_deployment_policy:
  id: "trusted-local-operation-wide"
  mode: "trusted_operation_wide"
  required_audience: "forge-local"
  mutating_tools: ["execute-operation"]
  startup_reconciliation: "required_before_listen"
  material_loading: "canonical_project_bound"
  snapshot_loading: "bounded_local_read_only"
  effect_scope: "operation_wide"
  public_mutation: "explicit_opt_in"
  root_binding: "canonical_configured_root"
  state_root_binding: "project_link_resolved"
  replay_rollback_protection: "external_monotonic_head"
  required_commit_protocol: "execution_provenance_commit_v0@0.1"
  same_user_boundary_acknowledged: true
"#;
    ValidatedMcpDeploymentPolicy::from_yaml(yaml).expect("operation-wide policy")
}

fn provision_anchor(state_root: &std::path::Path, project_root: &std::path::Path) -> PathBuf {
    provision_anchor_for_policy(state_root, project_root, "trusted-local-single-effect")
}

fn provision_anchor_for_policy(
    state_root: &std::path::Path,
    project_root: &std::path::Path,
    policy_id: &str,
) -> PathBuf {
    let anchor = project_root.join("operator-replay-anchor.json");
    provision_replay_anchor(state_root, &anchor, policy_id)
        .expect("provision external replay anchor");
    anchor
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("write to string");
        output
    })
}

#[allow(clippy::too_many_lines)] // one linear cryptographic fixture keeps all signed bindings visible
fn fixture(label: &str, bind_payload_digests: bool, require_citation: bool) -> LoaderFixture {
    fixture_for_scope(label, bind_payload_digests, require_citation, false)
}

fn operation_wide_fixture(
    label: &str,
    bind_payload_digests: bool,
    require_citation: bool,
) -> LoaderFixture {
    fixture_for_scope(label, bind_payload_digests, require_citation, true)
}

#[allow(clippy::too_many_lines)] // one linear cryptographic fixture keeps all signed bindings visible
fn fixture_for_scope(
    label: &str,
    bind_payload_digests: bool,
    require_citation: bool,
    operation_wide: bool,
) -> LoaderFixture {
    let root = fresh_root(label);
    let mut operation: OperationContractDocument =
        parse_repo_yaml("docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml");
    let command: CommandContractDocument =
        parse_repo_yaml("contracts/commands/story-validation-fast.yaml");
    let mut effect: ToolEffectContractDocument =
        parse_repo_yaml("contracts/effects/story-artifact-write-effect.yaml");
    let assurance: AssuranceCaseDocument =
        parse_repo_yaml("contracts/assurance/representative-slice-verified-assurance.yaml");
    let mut claim: ClaimContractDocument =
        parse_repo_yaml("contracts/claims/story-v2-010-active-claim.yaml");
    let state_version = assurance.assurance_case.project_snapshot.state_version;
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
        .agent_id = Some(StableId("codex-main".to_owned()));
    operation.operation_contract.coordination_scope.target.paths =
        vec![RepoPath("output/result.txt".to_owned())];
    operation
        .operation_contract
        .coordination_scope
        .write_authority
        .requires_lane_claim = true;
    operation
        .operation_contract
        .coordination_scope
        .write_authority
        .claim_contract_ref = Some(RepoPath(CLAIM_REF.to_owned()));
    effect
        .tool_effect_contract
        .read_set
        .retain(|read| read.target_kind != EffectTargetKind::FilePath);
    effect.tool_effect_contract.write_set.truncate(1);
    effect.tool_effect_contract.write_set[0].target_kind = EffectTargetKind::FilePath;
    "output/result.txt".clone_into(&mut effect.tool_effect_contract.write_set[0].reference);
    let mut effects = vec![(EFFECT_REF, effect)];
    if operation_wide {
        let mut second = effects[0].1.clone();
        second.tool_effect_contract.id = StableId("effect.story-artifact-write.second".to_owned());
        second.tool_effect_contract.contract_ref = RepoPath(EFFECT_REF_2.to_owned());
        "output/result-2.txt".clone_into(&mut second.tool_effect_contract.write_set[0].reference);
        operation
            .operation_contract
            .coordination_scope
            .target
            .paths
            .push(RepoPath("output/result-2.txt".to_owned()));
        effects.push((EFFECT_REF_2, second));
    }
    operation.operation_contract.effect_contract_refs = effects
        .iter()
        .map(|(reference, _)| RepoPath((*reference).to_owned()))
        .collect();
    claim.claim_contract.claim.claimant_principal_id =
        Some(PrincipalId("principal.codex-main".to_owned()));
    claim.claim_contract.claim.claimant_agent_id = StableId("codex-main".to_owned());
    claim.claim_contract.claim.claimant_role = ActorRole::Driver;
    claim
        .claim_contract
        .scope
        .paths
        .clone_from(&operation.operation_contract.coordination_scope.target.paths);
    claim.claim_contract.lease.expected_state_version = state_version;
    "2030-01-01T00:00:00Z".clone_into(&mut claim.claim_contract.lease.expires_at);

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
    for (reference, effect) in &effects {
        fs::write(
            root.join(reference),
            yaml_serde::to_string(effect).expect("effect yaml"),
        )
        .expect("write effect");
    }
    fs::copy(
        repo_root().join("contracts/risk-audits/fail-soft.yaml"),
        root.join(RISK_REF),
    )
    .expect("copy risk audit");
    fs::create_dir_all(root.join("contracts/research")).expect("research contracts");
    fs::copy(
        repo_root().join("contracts/research/field-evidence-20260625.yaml"),
        root.join("contracts/research/field-evidence-20260625.yaml"),
    )
    .expect("copy field evidence");

    let mut payload_bindings = Vec::new();
    let mut first_payload = None;
    for (index, write) in effects
        .iter()
        .flat_map(|(_, effect)| &effect.tool_effect_contract.write_set)
        .enumerate()
    {
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
    let claim_snapshot = ClaimSnapshotObservation {
        revision: 11,
        completeness: SnapshotCompleteness::Complete,
        claims: vec![ClaimRevisionObservation {
            claim_ref: RepoPath(CLAIM_REF.to_owned()),
            revision: 7,
            document: claim,
        }],
    };
    let gate_snapshot = GateSnapshotObservation {
        revision: 5,
        completeness: SnapshotCompleteness::Complete,
        gates: Vec::new(),
    };
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
        effect_bindings: effects
            .iter()
            .map(|(reference, effect)| ContentAddressedBinding {
                reference: (*reference).to_owned(),
                token: effect_contract_token(effect).expect("effect token"),
            })
            .collect(),
        expected_claim_snapshot_revision: 11,
        expected_claim_revisions: vec![RevisionExpectation {
            reference: CLAIM_REF.to_owned(),
            revision: 7,
        }],
        expected_gate_snapshot_revision: 5,
        expected_gate_revisions: Vec::new(),
        authority_snapshot_token: authority_snapshot_token(
            &claim_snapshot,
            &gate_snapshot,
            state_version,
            NOW,
        )
        .expect("authority snapshot token"),
        expected_replay_reservation_revision: 1,
        nonce: NONCE.to_owned(),
        issued_at_unix: NOW - 10,
    };
    let intent_digest = execution_intent_digest(&admission).expect("intent digest");
    let call = verified_call(
        principal_id,
        agent_id,
        &intent_digest,
        payload_bindings,
        require_citation,
        operation
            .operation_contract
            .effect_contract_refs
            .iter()
            .map(|reference| PathBuf::from(&reference.0))
            .collect(),
    );
    let snapshot = McpLocalExecutionSnapshotDocument {
        schema_version: MCP_LOCAL_SNAPSHOT_SCHEMA_VERSION.to_owned(),
        execution_snapshot: McpLocalExecutionSnapshot {
            admission_request: admission,
            assurance_case: assurance,
            claim_snapshot,
            gate_snapshot,
            current_state_version: state_version,
            now_unix: NOW,
        },
    };
    fs::write(
        root.join(SNAPSHOT_REF),
        yaml_serde::to_string(&snapshot).expect("snapshot yaml"),
    )
    .expect("write snapshot");

    let loader = TrustedMcpMaterialLoader::new(
        if operation_wide {
            trusted_operation_wide_policy()
        } else {
            trusted_policy()
        },
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
    require_citation: bool,
    effect_refs: Vec<PathBuf>,
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
        ExecutionRequest::new_operation_wide(
            PathBuf::from(OPERATION_REF),
            vec![PathBuf::from(COMMAND_REF)],
            effect_refs,
            payloads,
            Some(PathBuf::from(RISK_REF)),
            require_citation,
        ),
    )
}

#[test]
fn trusted_loader_loads_exact_typed_material_and_signed_payloads() {
    let fixture = fixture("valid", true, true);
    let loaded = fixture.loader.load(fixture.call).expect("trusted material");
    let audit = loaded.audit();
    assert_eq!(audit.command_count, 1);
    assert_eq!(audit.payload_count, 1);
    assert!(audit.risk_audit_loaded);
    assert!(audit.citation_material_loaded);
    assert_eq!(audit.claim_snapshot_revision, 11);
    assert_eq!(audit.gate_snapshot_revision, 5);
    assert!(audit
        .payload_hashes
        .iter()
        .all(|hash| hash.starts_with("sha256:")));
}

#[test]
fn trusted_loader_rejects_request_count_that_does_not_match_policy_scope() {
    let single = fixture("operation-wide-policy-single-request", true, false);
    let operation_wide_loader = TrustedMcpMaterialLoader::new(
        trusted_operation_wide_policy(),
        &single.root,
        SNAPSHOT_REF,
        TrustedMcpLoaderLimits::default(),
    )
    .expect("operation-wide loader");
    assert!(matches!(
        operation_wide_loader.load(single.call),
        Err(TrustedMcpLoadError::OperationWideEffectSetRequired)
    ));

    let operation_wide = operation_wide_fixture("single-policy-wide-request", true, false);
    let single_loader = TrustedMcpMaterialLoader::new(
        trusted_policy(),
        &operation_wide.root,
        SNAPSHOT_REF,
        TrustedMcpLoaderLimits::default(),
    )
    .expect("single-effect loader");
    assert!(matches!(
        single_loader.load(operation_wide.call),
        Err(TrustedMcpLoadError::SingleEffectRequired)
    ));
}

#[test]
fn trusted_loader_rejects_second_effect_drift_in_operation_wide_set() {
    let fixture = operation_wide_fixture("second-effect-drift", true, false);
    let path = fixture.root.join(EFFECT_REF_2);
    let mut effect: ToolEffectContractDocument =
        yaml_serde::from_str(&fs::read_to_string(&path).expect("second effect"))
            .expect("typed second effect");
    effect.tool_effect_contract.id = StableId("effect.tampered.second".to_owned());
    fs::write(
        path,
        yaml_serde::to_string(&effect).expect("tampered effect YAML"),
    )
    .expect("tamper second effect");
    assert!(matches!(
        fixture.loader.load(fixture.call),
        Err(TrustedMcpLoadError::EffectBindingMismatch)
    ));
}

#[test]
fn trusted_loader_rejects_unsigned_and_changed_payload_bytes() {
    let unsigned = fixture("unsigned", false, false);
    assert!(matches!(
        unsigned.loader.load(unsigned.call),
        Err(TrustedMcpLoadError::PayloadDigestRequired { .. })
    ));

    let changed = fixture("changed", true, false);
    fs::write(&changed.first_payload, b"changed after signing\n").expect("tamper payload");
    assert!(matches!(
        changed.loader.load(changed.call),
        Err(TrustedMcpLoadError::PayloadDigestMismatch { .. })
    ));
}

#[test]
fn trusted_loader_rejects_mutable_snapshot_changed_after_signing() {
    let fixture = fixture("snapshot-token-tamper", true, false);
    let snapshot_path = fixture.root.join(SNAPSHOT_REF);
    let text = fs::read_to_string(&snapshot_path).expect("snapshot");
    let mut document: McpLocalExecutionSnapshotDocument =
        yaml_serde::from_str(&text).expect("typed snapshot");
    document.execution_snapshot.now_unix += 1;
    fs::write(
        &snapshot_path,
        yaml_serde::to_string(&document).expect("tampered snapshot yaml"),
    )
    .expect("tamper snapshot");

    assert!(matches!(
        fixture.loader.load(fixture.call),
        Err(TrustedMcpLoadError::AuthoritySnapshotBindingMismatch)
    ));
}

#[test]
fn trusted_loader_rejects_unsafe_paths_oversize_and_open_yaml() {
    let path_fixture = fixture("path-bounds", true, false);
    assert!(matches!(
        LocalMcpSnapshotSource::new(&path_fixture.root, "../outside.yaml", 1024),
        Err(TrustedMcpLoadError::InvalidReference(_))
    ));

    let open_fixture = fixture("open-yaml", true, false);
    let mut yaml = fs::read_to_string(&open_fixture.operation).expect("operation");
    yaml.push_str("unknown_field: true\n");
    fs::write(&open_fixture.operation, yaml).expect("open operation");
    assert!(matches!(
        open_fixture.loader.load(open_fixture.call),
        Err(TrustedMcpLoadError::Parse { .. })
    ));

    let risk_fixture = fixture("open-risk-yaml", true, false);
    let mut risk_yaml = fs::read_to_string(&risk_fixture.risk_audit).expect("risk audit");
    risk_yaml.push_str("unknown_field: true\n");
    fs::write(&risk_fixture.risk_audit, risk_yaml).expect("open risk audit");
    assert!(matches!(
        risk_fixture.loader.load(risk_fixture.call),
        Err(TrustedMcpLoadError::Parse { .. })
    ));

    let size_fixture = fixture("oversize", true, false);
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
    let fixture = fixture("dormant", true, false);
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
    let fixture = fixture("read-only-policy", true, false);
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

#[test]
fn reconciled_executor_commits_one_effect_and_consumes_replay() {
    let fixture = fixture("active-single-effect", true, true);
    let state_root = fixture.root.join(".forge-method");
    initialize_replay_wal(&state_root).expect("initialize replay");
    let replay_anchor = provision_anchor(&state_root, &fixture.root);
    let activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile(
            trusted_policy(),
            &fixture.root,
            &state_root,
            &replay_anchor,
            ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
        )
        .expect("startup reconciliation"),
    );
    let executor =
        TrustedSingleEffectMcpExecutor::new(fixture.loader, activation).expect("active executor");
    let result = executor.execute(fixture.call).expect("execute one effect");
    assert_eq!(
        result.status(),
        forge_core_authority::ExecutionStatus::Applied
    );
    assert_eq!(
        fs::read(fixture.root.join("output/result.txt")).expect("committed output"),
        b"trusted payload 0\n"
    );
    assert!(fixture
        .root
        .join(".forge-method/wal/effects.ndjson")
        .exists());
}

#[test]
fn reconciled_operation_wide_executor_commits_every_effect_in_one_transaction() {
    let fixture = operation_wide_fixture("active-operation-wide", true, true);
    let loaded = fixture
        .loader
        .load(fixture.call)
        .expect("load operation-wide material");
    assert_eq!(loaded.audit().effect_count, 2);
    assert_eq!(loaded.audit().effect_ids.len(), 2);

    let state_root = fixture.root.join(".forge-method");
    initialize_replay_wal(&state_root).expect("initialize replay");
    let replay_anchor =
        provision_anchor_for_policy(&state_root, &fixture.root, "trusted-local-operation-wide");
    let activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile_operation_wide(
            trusted_operation_wide_policy(),
            &fixture.root,
            &state_root,
            &replay_anchor,
            ExplicitTrustedOperationWideOptIn::from_operator_flag(),
        )
        .expect("operation-wide startup reconciliation"),
    );
    assert_eq!(
        activation.audit().effect_scope,
        forge_core_protocol_mcp::EffectScopePolicy::OperationWide
    );

    // Recreate the signed fixture because loading consumes the opaque call.
    let fixture = operation_wide_fixture("active-operation-wide-commit", true, true);
    let state_root = fixture.root.join(".forge-method");
    initialize_replay_wal(&state_root).expect("initialize replay");
    let replay_anchor =
        provision_anchor_for_policy(&state_root, &fixture.root, "trusted-local-operation-wide");
    let activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile_operation_wide(
            trusted_operation_wide_policy(),
            &fixture.root,
            &state_root,
            &replay_anchor,
            ExplicitTrustedOperationWideOptIn::from_operator_flag(),
        )
        .expect("operation-wide startup reconciliation"),
    );
    let executor = TrustedOperationWideMcpExecutor::new(fixture.loader, activation)
        .expect("operation-wide executor");
    let result = executor.execute(fixture.call).expect("execute all effects");
    assert_eq!(
        result.status(),
        forge_core_authority::ExecutionStatus::Applied
    );
    assert_eq!(
        fs::read(fixture.root.join("output/result.txt")).expect("first output"),
        b"trusted payload 0\n"
    );
    assert_eq!(
        fs::read(fixture.root.join("output/result-2.txt")).expect("second output"),
        b"trusted payload 1\n"
    );
    let records = fs::read_to_string(fixture.root.join(".forge-method/wal/effects.ndjson"))
        .expect("effect WAL")
        .lines()
        .map(|line| serde_json::from_str::<EffectWalRecord>(line).expect("effect WAL record"))
        .collect::<Vec<_>>();
    assert_eq!(
        records
            .iter()
            .filter(|record| record.stage == EffectWalStage::Begin)
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.stage == EffectWalStage::Commit)
            .count(),
        1
    );
}

#[test]
fn activation_requires_preinitialized_replay_authority() {
    let fixture = fixture("activation-missing-replay", true, false);
    let error = ReconciledTrustedMcpDeployment::reconcile(
        trusted_policy(),
        &fixture.root,
        fixture.root.join(".forge-method"),
        fixture.root.join("operator-replay-anchor.json"),
        ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
    )
    .expect_err("missing replay pair must fail startup");
    assert!(error.to_string().contains("replay"));
}

#[test]
fn activation_rejects_anchor_from_another_deployment_policy() {
    let fixture = fixture("activation-wrong-anchor-deployment", true, false);
    let state_root = fixture.root.join(".forge-method");
    initialize_replay_wal(&state_root).expect("initialize replay");
    let anchor = fixture.root.join("operator-replay-anchor.json");
    provision_replay_anchor(&state_root, &anchor, "different-policy").expect("provision anchor");
    let error = ReconciledTrustedMcpDeployment::reconcile(
        trusted_policy(),
        &fixture.root,
        &state_root,
        &anchor,
        ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
    )
    .expect_err("cross-deployment anchor must fail startup");
    assert!(error
        .to_string()
        .contains("does not match expected deployment"));
}

#[test]
fn active_executor_rejects_whole_replay_pair_rollback_before_loading_call() {
    let fixture = fixture("active-replay-rollback", true, true);
    let state_root = fixture.root.join(".forge-method");
    initialize_replay_wal(&state_root).expect("initialize replay");
    let replay_anchor = provision_anchor(&state_root, &fixture.root);
    let activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile(
            trusted_policy(),
            &fixture.root,
            &state_root,
            &replay_anchor,
            ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
        )
        .expect("startup reconciliation"),
    );
    let empty_wal = fs::read(replay_wal_path(&state_root)).expect("empty replay WAL");
    reserve_replay_nonce(
        &state_root,
        &PrincipalId("principal.rollback-probe".to_owned()),
        AUDIENCE,
        "trusted-runtime-rollback-probe",
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .expect("append rollback probe");
    advance_replay_anchor(&state_root, &replay_anchor).expect("advance external head");
    fs::write(replay_wal_path(&state_root), empty_wal).expect("restore older replay WAL");

    let executor =
        TrustedSingleEffectMcpExecutor::new(fixture.loader, activation).expect("active executor");
    let error = executor
        .execute(fixture.call)
        .expect_err("rollback must reject before loading the call");
    assert!(error.to_string().contains("rollback detected"));
}

#[test]
fn activation_accepts_project_link_resolved_external_sidecar_state() {
    let fixture = fixture("activation-external-sidecar", true, false);
    let external_state = fresh_root("external-sidecar-state").join("state/.forge-method");
    fs::create_dir_all(&external_state).expect("external state root");
    initialize_replay_wal(&external_state).expect("external replay initialization");
    let replay_anchor = provision_anchor(&external_state, &fixture.root);
    let activation = ReconciledTrustedMcpDeployment::reconcile(
        trusted_policy(),
        &fixture.root,
        &external_state,
        &replay_anchor,
        ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
    )
    .expect("external sidecar activation");
    assert_eq!(
        activation.environment().state_root(),
        external_state
            .canonicalize()
            .expect("canonical external state")
    );
}

#[test]
fn active_executor_runs_risk_and_citation_gates_before_replay_reservation() {
    let risk_fixture = fixture("active-risk-block", true, false);
    fs::write(
        &risk_fixture.risk_audit,
        r#"schema_version: risk-audit-v0
rules:
  - id: block-operation-yaml
    description: test rejection
    severity: error
    detector:
      kind: regex
      pattern: operation_contract
    evidence_required: false
    fix_hint: remove test marker
    applies_to: ["**/*.yaml"]
"#,
    )
    .expect("blocking risk rule");
    let risk_state = risk_fixture.root.join(".forge-method");
    initialize_replay_wal(&risk_state).expect("initialize replay");
    let risk_anchor = provision_anchor(&risk_state, &risk_fixture.root);
    let risk_activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile(
            trusted_policy(),
            &risk_fixture.root,
            &risk_state,
            &risk_anchor,
            ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
        )
        .expect("risk startup"),
    );
    let risk_executor = TrustedSingleEffectMcpExecutor::new(risk_fixture.loader, risk_activation)
        .expect("risk executor");
    let risk_error = risk_executor
        .execute(risk_fixture.call)
        .expect_err("risk gate must reject");
    assert!(risk_error.to_string().contains("risk_audit"));
    let risk_recovery = forge_core_store::replay_wal::recover_replay_wal(
        risk_fixture.root.join(".forge-method"),
        false,
    )
    .expect("risk replay state");
    assert_eq!(risk_recovery.valid_record_count, 0);

    let citation_fixture = fixture("active-citation-block", true, true);
    fs::write(
        citation_fixture
            .root
            .join("contracts/unresolved-citation.yaml"),
        "schema_version: '0.1'\nsource_id: definitely_missing_source\n",
    )
    .expect("unresolved citation");
    let citation_state = citation_fixture.root.join(".forge-method");
    initialize_replay_wal(&citation_state).expect("initialize replay");
    let citation_anchor = provision_anchor(&citation_state, &citation_fixture.root);
    let citation_activation = std::sync::Arc::new(
        ReconciledTrustedMcpDeployment::reconcile(
            trusted_policy(),
            &citation_fixture.root,
            &citation_state,
            &citation_anchor,
            ExplicitTrustedSingleEffectOptIn::from_operator_flag(),
        )
        .expect("citation startup"),
    );
    let citation_executor =
        TrustedSingleEffectMcpExecutor::new(citation_fixture.loader, citation_activation)
            .expect("citation executor");
    let citation_error = citation_executor
        .execute(citation_fixture.call)
        .expect_err("citation gate must reject");
    assert!(citation_error.to_string().contains("citation"));
    let citation_recovery = forge_core_store::replay_wal::recover_replay_wal(
        citation_fixture.root.join(".forge-method"),
        false,
    )
    .expect("citation replay state");
    assert_eq!(citation_recovery.valid_record_count, 0);
}
