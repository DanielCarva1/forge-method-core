use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    AttestationInput, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry,
    CanonicalIntent, ExecutionPayloadBinding, ExecutionRequest, PrincipalCredentialStatus,
    PrincipalRegistryContract, PrincipalRegistryDocument, PrincipalRegistryEntry,
    VerifiedExecutionCall, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::{
    claim::ActorRole,
    operation::{CallerRole, OperationGateScope, RequiredGate},
    tool_effect::EffectTargetKind,
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument, GateContractDocument,
    OperationContractDocument, PrincipalId, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, authority_snapshot_token, command_contract_token, derive_assurance_case,
    effect_contract_token, execution_intent_digest, operation_contract_token, unix_to_rfc3339,
    ClaimRevisionObservation, ClaimSnapshotObservation, ContentAddressedBinding,
    ExecutionAdmissionIssueCode, ExecutionAdmissionRequest, ExecutionAdmissionStatus,
    GateRevisionObservation, GateSnapshotObservation, ObligationEngineInputDocument,
    RevisionExpectation, SnapshotCompleteness,
};
use forge_core_kernel::{
    prepare_execution_transaction, reconcile_prepared_execution_commits, ExecutionCommitError,
    ExecutionCommitOutcome, ExecutionCommitStatus, ExecutionReplayReconciliationStatus,
    LateAdmissionError, LateAdmissionOutcome, LateExecutionSnapshot, LateExecutionSnapshotSource,
    LateSnapshotError, PreparedExecutionMaterial, RuntimeEffectPayloadKind,
    RuntimeOperationEffectPayload, TrustedExecutionEnvironment, PREPARED_EFFECT_LOCK_RELATIVE_PATH,
    PREPARED_EFFECT_WAL_RELATIVE_PATH,
};
use forge_core_store::replay_wal::{
    initialize_replay_wal, recover_replay_wal, ReplayReservationState,
};
use forge_core_store::{
    sha256_content_hash, try_acquire_effect_store_lock, EffectStoreLockError, EffectWalRecord,
    EffectWalStage,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const NOW: i64 = 1_800_000_000;
const CLAIM_REF: &str = "contracts/claims/story-v2-010-active-claim.yaml";
const GATE_REF: &str = "contracts/gates/story-ready-lane-gate.yaml";
const EFFECT_REF: &str = "contracts/effects/story-artifact-write-effect.yaml";
const EFFECT_TARGET: &str = "crates/forge-contracts/p4b2b-fixture.yaml";
const AUDIENCE: &str = "forge://workspace/p4b2b-fixture";
const NONCE: &str = "nonce-p4b2b-fixture-000000000001";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_yaml<T: DeserializeOwned>(relative: &str) -> T {
    let path = repo_root().join(relative);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    yaml_serde::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn temp_project(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-p4b2b-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(root.join(".forge-method")).expect("state root");
    fs::create_dir_all(root.join("crates/forge-contracts")).expect("target parent");
    initialize_replay_wal(root.join(".forge-method")).expect("initialize replay WAL");
    root
}

fn verified_assurance_case() -> AssuranceCaseDocument {
    let input: ObligationEngineInputDocument =
        parse_yaml("docs/fixtures/obligation-engine-v0/verified-release.yaml");
    derive_assurance_case(&input).expect("derive verified Assurance Case")
}

#[derive(Debug, Clone)]
struct StaticSnapshotSource {
    snapshot: LateExecutionSnapshot,
}

impl LateExecutionSnapshotSource for StaticSnapshotSource {
    fn capture(&self) -> Result<LateExecutionSnapshot, LateSnapshotError> {
        Ok(self.snapshot.clone())
    }
}

#[derive(Debug)]
struct FailingSnapshotSource;

impl LateExecutionSnapshotSource for FailingSnapshotSource {
    fn capture(&self) -> Result<LateExecutionSnapshot, LateSnapshotError> {
        Err(LateSnapshotError::new("snapshot source unavailable"))
    }
}

struct Fixture {
    project_root: PathBuf,
    target_path: PathBuf,
    material: PreparedExecutionMaterial,
    source: StaticSnapshotSource,
}

fn fixture(label: &str) -> Fixture {
    fixture_with_request_mutation(label, |_| {})
}

#[allow(clippy::too_many_lines)]
fn fixture_with_request_mutation<F>(label: &str, mutate_request: F) -> Fixture
where
    F: FnOnce(&mut ExecutionAdmissionRequest),
{
    let project_root = temp_project(label);
    let assurance_case = verified_assurance_case();
    let state_version = assurance_case.assurance_case.project_snapshot.state_version;

    let mut operation: OperationContractDocument =
        parse_yaml("docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml");
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
    operation.operation_contract.gates.required_before_mutation = vec![RequiredGate {
        scope: OperationGateScope::Lane,
        gate_contract_ref: RepoPath(GATE_REF.to_owned()),
        reason: Some("commit-time representative evidence".to_owned()),
    }];
    operation.operation_contract.gates.gate_contract_refs = vec![RepoPath(GATE_REF.to_owned())];

    let command: CommandContractDocument =
        parse_yaml("contracts/commands/story-validation-fast.yaml");
    let mut effect: ToolEffectContractDocument = parse_yaml(EFFECT_REF);
    effect.tool_effect_contract.actor.agent_id = StableId("codex-main".to_owned());
    effect
        .tool_effect_contract
        .read_set
        .retain(|read| read.target_kind != EffectTargetKind::FilePath);
    effect.tool_effect_contract.write_set.truncate(1);
    effect.tool_effect_contract.write_set[0].target_kind = EffectTargetKind::FilePath;
    effect.tool_effect_contract.write_set[0]
        .reference
        .clone_from(&EFFECT_TARGET.to_owned());

    let mut claim: ClaimContractDocument = parse_yaml(CLAIM_REF);
    claim.claim_contract.claim.claimant_principal_id =
        Some(PrincipalId("principal.codex-main".to_owned()));
    claim.claim_contract.claim.claimant_agent_id = StableId("codex-main".to_owned());
    claim.claim_contract.claim.claimant_role = ActorRole::Driver;
    claim.claim_contract.lease.expected_state_version = state_version;
    claim.claim_contract.lease.expires_at = unix_to_rfc3339(NOW + 600);
    let gate: GateContractDocument = parse_yaml(GATE_REF);
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
        gates: vec![GateRevisionObservation {
            gate_ref: RepoPath(GATE_REF.to_owned()),
            revision: 3,
            observed_state_version: state_version,
            document: gate,
        }],
    };

    let mut request = ExecutionAdmissionRequest {
        id: StableId(format!("admission.request.{label}")),
        principal_id: PrincipalId("principal.codex-main".to_owned()),
        agent_id: StableId("codex-main".to_owned()),
        principal_role: CallerRole::Driver,
        operation_id: operation.operation_contract.contract_id.clone(),
        operation_token: operation_contract_token(&operation).expect("operation token"),
        assurance_case_id: assurance_case.assurance_case.id.clone(),
        assurance_case_token: assurance_case_token(&assurance_case).expect("case token"),
        command_bindings: vec![ContentAddressedBinding {
            reference: command.command_contract.id.0.clone(),
            token: command_contract_token(&command).expect("command token"),
        }],
        effect_bindings: vec![ContentAddressedBinding {
            reference: EFFECT_REF.to_owned(),
            token: effect_contract_token(&effect).expect("effect token"),
        }],
        expected_claim_snapshot_revision: 11,
        expected_claim_revisions: vec![RevisionExpectation {
            reference: CLAIM_REF.to_owned(),
            revision: 7,
        }],
        expected_gate_snapshot_revision: 5,
        expected_gate_revisions: vec![RevisionExpectation {
            reference: GATE_REF.to_owned(),
            revision: 3,
        }],
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
    let intent_digest = execution_intent_digest(&request).expect("intent digest");
    let call = verified_call(&request, &intent_digest);
    mutate_request(&mut request);

    let payload_content = b"p4b2b: prepared\n".to_vec();
    let payload = RuntimeOperationEffectPayload {
        target_ref: EFFECT_TARGET.to_owned(),
        payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
        content_hash: sha256_content_hash(&payload_content),
        content: payload_content,
    };
    let material = PreparedExecutionMaterial::new(
        call,
        request,
        operation,
        vec![command],
        effect,
        vec![payload],
    );
    let source = StaticSnapshotSource {
        snapshot: LateExecutionSnapshot {
            assurance_case,
            claim_snapshot,
            gate_snapshot,
            current_state_version: state_version,
            now_unix: NOW,
        },
    };
    Fixture {
        target_path: project_root.join(EFFECT_TARGET),
        project_root,
        material,
        source,
    }
}

fn verified_call(
    request: &ExecutionAdmissionRequest,
    intent_digest: &str,
) -> VerifiedExecutionCall {
    let signing_key = SigningKey::from_bytes(&[17; 32]);
    let public_key_hex = hex(signing_key.verifying_key().as_bytes());
    let registry = AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
        schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
        principal_registry: PrincipalRegistryContract {
            audience: AUDIENCE.to_owned(),
            principals: vec![PrincipalRegistryEntry {
                credential_id: "key.codex-main.p4b2b".to_owned(),
                principal_id: request.principal_id.clone(),
                agent_id: request.agent_id.clone(),
                role: request.principal_role,
                public_key_hex: public_key_hex.clone(),
                allowed_tools: vec![StableId("execute-operation".to_owned())],
                authority_grants: vec![StableId("operation.execute".to_owned())],
                status: PrincipalCredentialStatus::Active,
            }],
        },
    })
    .expect("registry");

    let arguments = execution_arguments();
    let intent = CanonicalIntent {
        tool: "execute-operation".to_owned(),
        arguments: Value::Object(arguments),
        credential_id: Some("key.codex-main.p4b2b".to_owned()),
        audience: Some(AUDIENCE.to_owned()),
        execution_intent_digest: Some(intent_digest.to_owned()),
        nonce: request.nonce.clone(),
        ts: request.issued_at_unix,
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
        .expect("verified authorization");
    let execution_request = ExecutionRequest::new(
        PathBuf::from("contracts/operations/p4b2b-operation.yaml"),
        vec![PathBuf::from(
            "contracts/commands/story-validation-fast.yaml",
        )],
        Some(PathBuf::from(EFFECT_REF)),
        vec![ExecutionPayloadBinding::new(
            EFFECT_TARGET.to_owned(),
            PathBuf::from("payloads/p4b2b-fixture.yaml"),
        )],
        None,
        false,
    );
    VerifiedExecutionCall::new(authorization, execution_request)
}

fn execution_arguments() -> Map<String, Value> {
    let mut arguments = Map::new();
    arguments.insert(
        "--operation".to_owned(),
        Value::String("contracts/operations/p4b2b-operation.yaml".to_owned()),
    );
    arguments.insert(
        "--command".to_owned(),
        Value::String("contracts/commands/story-validation-fast.yaml".to_owned()),
    );
    arguments.insert("--effect".to_owned(), Value::String(EFFECT_REF.to_owned()));
    arguments.insert(
        "--payload".to_owned(),
        Value::String(format!("{EFFECT_TARGET}=payloads/p4b2b-fixture.yaml")),
    );
    arguments
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn effect_wal(root: &Path) -> PathBuf {
    root.join(PREPARED_EFFECT_WAL_RELATIVE_PATH)
}

#[test]
fn valid_snapshot_is_late_admitted_without_effect_wal_or_write() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("admitted");
    assert!(!format!("{material:?}").contains(NONCE));
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");
    assert!(!format!("{prepared:?}").contains(NONCE));

    assert_eq!(prepared.replay_reservation().revision, 1);
    assert_eq!(
        prepared.replay_reservation().state,
        ReplayReservationState::Reserved
    );
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    assert!(matches!(
        try_acquire_effect_store_lock(&project_root, PREPARED_EFFECT_LOCK_RELATIVE_PATH),
        Err(EffectStoreLockError::WouldBlock { .. })
    ));

    let outcome = prepared.evaluate_late(&source).expect("late evaluation");
    let LateAdmissionOutcome::Admitted(admitted) = outcome else {
        panic!("complete snapshot must admit");
    };
    assert_eq!(
        admitted.decision().status,
        ExecutionAdmissionStatus::Admitted
    );
    assert!(admitted.decision().issues.is_empty());
    assert!(!format!("{admitted:?}").contains(NONCE));
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    assert!(matches!(
        try_acquire_effect_store_lock(&project_root, PREPARED_EFFECT_LOCK_RELATIVE_PATH),
        Err(EffectStoreLockError::WouldBlock { .. })
    ));

    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let replay_root = project_root.join(".forge-method");
    let worker = thread::spawn(move || {
        started_tx.send(()).expect("signal replay reader");
        done_tx
            .send(recover_replay_wal(replay_root, false))
            .expect("send replay recovery");
    });
    started_rx.recv().expect("replay reader started");
    assert!(
        done_rx.recv_timeout(Duration::from_millis(150)).is_err(),
        "late-admitted typestate must retain the replay lock"
    );
    drop(admitted);
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("replay reader unblocked")
        .expect("replay remains valid");
    worker.join().expect("replay reader joins");

    assert!(
        try_acquire_effect_store_lock(&project_root, PREPARED_EFFECT_LOCK_RELATIVE_PATH).is_ok()
    );
    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    assert_eq!(replay.valid_record_count, 1);
    assert_eq!(
        replay
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Reserved
    );
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn admitted_typestate_commits_one_provenance_bound_effect_and_consumes_replay() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("commit");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");
    let LateAdmissionOutcome::Admitted(admitted) =
        prepared.evaluate_late(&source).expect("late evaluation")
    else {
        panic!("complete snapshot must admit");
    };

    let outcome = admitted.commit(&source).expect("commit boundary");
    let ExecutionCommitOutcome::Committed { receipt } = outcome else {
        panic!("valid admitted execution must commit");
    };
    assert_eq!(receipt.status, ExecutionCommitStatus::Committed);
    assert_eq!(
        receipt.execution_principal.principal_id.0,
        "principal.codex-main"
    );
    assert_eq!(receipt.execution_principal.agent_id.0, "codex-main");
    assert!(receipt
        .principal_trace_event_id
        .ends_with(".principal.effect-staged"));
    assert_eq!(receipt.application.applied_refs, vec![EFFECT_TARGET]);
    assert!(receipt
        .replay
        .as_ref()
        .is_some_and(|result| result.appended));
    assert!(receipt.completion.is_some());
    assert_eq!(
        fs::read_to_string(&target_path).expect("committed target"),
        "p4b2b: prepared\n"
    );

    let wal_text = fs::read_to_string(effect_wal(&project_root)).expect("effect WAL");
    assert!(
        !wal_text.contains(NONCE),
        "raw nonce must never be persisted"
    );
    let records: Vec<EffectWalRecord> = wal_text
        .lines()
        .map(|line| serde_json::from_str(line).expect("effect WAL record"))
        .collect();
    assert_eq!(
        records.first().map(|record| record.stage),
        Some(EffectWalStage::Begin)
    );
    assert!(records
        .first()
        .and_then(|record| record.execution_provenance.as_ref())
        .is_some());
    assert_eq!(
        records[0]
            .execution_provenance
            .as_ref()
            .expect("provenance")
            .document["execution_principal"]["principal_id"],
        "principal.codex-main"
    );
    assert_eq!(
        records.last().map(|record| record.stage),
        Some(EffectWalStage::ReplayConsumed)
    );

    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    let reservation = replay.reservations.values().next().expect("reservation");
    assert_eq!(reservation.state, ReplayReservationState::Consumed);
    assert_eq!(replay.valid_record_count, 2);
    let trace_text = fs::read_to_string(project_root.join(".forge-method/traces/events.ndjson"))
        .expect("execution principal trace");
    let trace: serde_json::Value =
        serde_json::from_str(trace_text.lines().last().expect("trace line")).expect("trace JSON");
    assert_eq!(trace["event_id"], receipt.principal_trace_event_id);
    assert_eq!(trace["actor"]["principal_id"], "principal.codex-main");
    assert_eq!(trace["actor"]["agent_id"], "codex-main");
    assert_eq!(trace["authority"]["capability_ids"][0], "operation.execute");
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn mutable_claim_drift_at_commit_call_blocks_before_effect_wal() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("commit-claim-drift");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");
    let LateAdmissionOutcome::Admitted(admitted) =
        prepared.evaluate_late(&source).expect("late evaluation")
    else {
        panic!("initial late snapshot must admit");
    };
    let mut commit_source = source;
    commit_source.snapshot.claim_snapshot.revision += 1;

    let outcome = admitted.commit(&commit_source).expect("typed commit block");
    let ExecutionCommitOutcome::Blocked { decision, .. } = outcome else {
        panic!("commit-time claim drift must block");
    };
    assert_eq!(decision.status, ExecutionAdmissionStatus::Blocked);
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == ExecutionAdmissionIssueCode::ClaimSnapshotRevisionMismatch));
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    assert_eq!(
        replay
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Reserved
    );
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn filesystem_drift_after_late_admission_fails_before_effect_wal() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("commit-filesystem-drift");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");
    let LateAdmissionOutcome::Admitted(admitted) =
        prepared.evaluate_late(&source).expect("late evaluation")
    else {
        panic!("initial late snapshot must admit");
    };
    fs::write(&target_path, "same-user bypass\n").expect("simulate drift");

    let error = admitted
        .commit(&source)
        .expect_err("commit-time filesystem drift must fail closed");
    assert!(matches!(
        error,
        ExecutionCommitError::EffectPreflightChanged { .. }
    ));
    assert!(!effect_wal(&project_root).exists());
    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    assert_eq!(
        replay
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Reserved
    );
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn recovery_reconciles_crash_after_effect_commit_before_replay_consume() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("crash-window");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment.clone()).expect("prepare");
    let LateAdmissionOutcome::Admitted(admitted) =
        prepared.evaluate_late(&source).expect("late evaluation")
    else {
        panic!("complete snapshot must admit");
    };
    let ExecutionCommitOutcome::Committed { receipt } = admitted.commit(&source).expect("commit")
    else {
        panic!("fixture must commit");
    };
    assert_eq!(receipt.status, ExecutionCommitStatus::Committed);

    // Reconstruct the exact durable state a process crash could leave after
    // effect Commit and before replay Consume: remove the completion marker and
    // truncate the replay WAL to its still-valid Reserve frame.
    let wal_path = effect_wal(&project_root);
    let wal_text = fs::read_to_string(&wal_path).expect("effect WAL");
    let mut effect_lines: Vec<&str> = wal_text.lines().collect();
    let removed = effect_lines.pop().expect("completion record");
    let removed_record: EffectWalRecord =
        serde_json::from_str(removed).expect("completion record JSON");
    assert_eq!(removed_record.stage, EffectWalStage::ReplayConsumed);
    fs::write(&wal_path, format!("{}\n", effect_lines.join("\n")))
        .expect("simulate missing completion marker");

    let replay_before =
        recover_replay_wal(project_root.join(".forge-method"), false).expect("replay before");
    assert_eq!(replay_before.records.len(), 2);
    let reserve = &replay_before.records[0];
    fs::OpenOptions::new()
        .write(true)
        .open(&replay_before.wal_path)
        .expect("open replay WAL")
        .set_len(reserve.offset + reserve.record_len)
        .expect("truncate to reserve frame");
    let reserved =
        recover_replay_wal(project_root.join(".forge-method"), false).expect("reserved replay");
    assert_eq!(reserved.valid_record_count, 1);
    assert_eq!(
        reserved
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Reserved
    );

    let reconciled =
        reconcile_prepared_execution_commits(&environment).expect("reconcile crash window");
    assert_eq!(
        reconciled.status,
        ExecutionReplayReconciliationStatus::Reconciled
    );
    assert_eq!(reconciled.reconciled_transactions.len(), 1);
    assert!(reconciled.replay_results[0].appended);
    assert!(reconciled.completion_records[0].completion.recovered);
    assert!(
        target_path.exists(),
        "committed effect must not be rolled back"
    );

    let replay_after =
        recover_replay_wal(project_root.join(".forge-method"), false).expect("replay after");
    assert_eq!(replay_after.valid_record_count, 2);
    assert_eq!(
        replay_after
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Consumed
    );
    let second = reconcile_prepared_execution_commits(&environment).expect("idempotent reconcile");
    assert_eq!(second.status, ExecutionReplayReconciliationStatus::Noop);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn recovery_is_idempotent_after_replay_consume_before_completion_marker() {
    let Fixture {
        project_root,
        target_path: _,
        material,
        source,
    } = fixture("crash-after-replay");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment.clone()).expect("prepare");
    let LateAdmissionOutcome::Admitted(admitted) =
        prepared.evaluate_late(&source).expect("late evaluation")
    else {
        panic!("complete snapshot must admit");
    };
    let ExecutionCommitOutcome::Committed { receipt } = admitted.commit(&source).expect("commit")
    else {
        panic!("fixture must commit");
    };
    assert_eq!(receipt.status, ExecutionCommitStatus::Committed);

    let wal_path = effect_wal(&project_root);
    let wal_text = fs::read_to_string(&wal_path).expect("effect WAL");
    let mut lines: Vec<&str> = wal_text.lines().collect();
    let marker_line = lines.pop().expect("completion marker");
    let marker: EffectWalRecord = serde_json::from_str(marker_line).expect("marker JSON");
    assert_eq!(marker.stage, EffectWalStage::ReplayConsumed);
    let torn_marker = &marker_line[..marker_line.len() / 2];
    fs::write(&wal_path, format!("{}\n{torn_marker}", lines.join("\n")))
        .expect("simulate torn completion marker");

    let replay_before =
        recover_replay_wal(project_root.join(".forge-method"), false).expect("replay before");
    assert_eq!(
        replay_before
            .reservations
            .values()
            .next()
            .expect("reservation")
            .state,
        ReplayReservationState::Consumed
    );
    let reconciled =
        reconcile_prepared_execution_commits(&environment).expect("reconcile completion marker");
    assert_eq!(
        reconciled.status,
        ExecutionReplayReconciliationStatus::Reconciled
    );
    assert_eq!(reconciled.reconciled_transactions.len(), 1);
    assert!(!reconciled.replay_results[0].appended);
    assert!(reconciled.completion_records[0].completion.recovered);
    let replay_after =
        recover_replay_wal(project_root.join(".forge-method"), false).expect("replay after");
    assert_eq!(replay_after.valid_record_count, 2);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn stale_claim_snapshot_blocks_at_late_boundary_without_effect_wal() {
    let Fixture {
        project_root,
        target_path,
        material,
        mut source,
    } = fixture("stale-claim");
    source.snapshot.claim_snapshot.revision = 12;
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");

    let outcome = prepared.evaluate_late(&source).expect("late evaluation");
    let LateAdmissionOutcome::Blocked { decision, .. } = outcome else {
        panic!("stale claim snapshot must block");
    };
    assert_eq!(decision.status, ExecutionAdmissionStatus::Blocked);
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == ExecutionAdmissionIssueCode::ClaimSnapshotRevisionMismatch));
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn filesystem_drift_after_prepare_fails_closed_before_admission() {
    let Fixture {
        project_root,
        target_path,
        material,
        source,
    } = fixture("drift");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");
    fs::write(&target_path, "raced: true\n").expect("simulate same-user bypass drift");

    let error = prepared
        .evaluate_late(&source)
        .expect_err("preflight drift must fail closed");
    assert!(matches!(
        error,
        LateAdmissionError::EffectPreflightChanged { .. }
    ));
    assert!(!effect_wal(&project_root).exists());
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn snapshot_capture_failure_releases_locks_without_effect_wal() {
    let Fixture {
        project_root,
        target_path,
        material,
        source: _,
    } = fixture("snapshot-failure");
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");
    let prepared = prepare_execution_transaction(material, environment).expect("prepare");

    let error = prepared
        .evaluate_late(&FailingSnapshotSource)
        .expect_err("snapshot failure must fail closed");
    assert!(matches!(error, LateAdmissionError::SnapshotCapture(_)));
    assert!(
        try_acquire_effect_store_lock(&project_root, PREPARED_EFFECT_LOCK_RELATIVE_PATH).is_ok()
    );
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn signed_request_tampering_is_rejected_before_lock_or_replay_reservation() {
    let Fixture {
        project_root,
        target_path,
        material,
        source: _,
    } = fixture_with_request_mutation("tampered-intent", |request| {
        request.id = StableId("admission.request.tampered-after-signature".to_owned());
    });
    let environment = TrustedExecutionEnvironment::from_project_root(&project_root, AUDIENCE)
        .expect("environment");

    let error = prepare_execution_transaction(material, environment)
        .expect_err("signed request tampering must fail");
    assert!(matches!(
        error,
        forge_core_kernel::PrepareExecutionError::AuthorityIntentMismatch
    ));
    assert!(
        try_acquire_effect_store_lock(&project_root, PREPARED_EFFECT_LOCK_RELATIVE_PATH).is_ok()
    );
    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    assert_eq!(replay.valid_record_count, 0);
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn trusted_environment_audience_mismatch_is_rejected_before_replay() {
    let Fixture {
        project_root,
        target_path,
        material,
        source: _,
    } = fixture("audience-mismatch");
    let environment = TrustedExecutionEnvironment::from_project_root(
        &project_root,
        "forge://workspace/another-project",
    )
    .expect("environment");

    let error = prepare_execution_transaction(material, environment)
        .expect_err("cross-project audience must fail");
    assert!(matches!(
        error,
        forge_core_kernel::PrepareExecutionError::AudienceMismatch
    ));
    let replay = recover_replay_wal(project_root.join(".forge-method"), false).expect("replay");
    assert_eq!(replay.valid_record_count, 0);
    assert!(!target_path.exists());
    assert!(!effect_wal(&project_root).exists());
    fs::remove_dir_all(project_root).expect("cleanup");
}
