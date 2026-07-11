use forge_core_contracts::{
    claim::ActorRole,
    operation::{CallerRole, OperationGateScope, RequiredGate},
    tool_effect::EffectTargetKind,
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument, GateContractDocument,
    OperationContractDocument, PrincipalId, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_decisions::{
    assurance_case_token, authority_snapshot_token, command_contract_token, derive_assurance_case,
    effect_contract_token, evaluate_execution_admission, execution_intent_digest,
    operation_contract_token, unix_to_rfc3339, ClaimRevisionObservation, ClaimSnapshotObservation,
    CommitAssuranceObservation, CompensationCoverage, ContentAddressedBinding,
    EffectContractBinding, ExecutionAdmissionInput, ExecutionAdmissionInputDocument,
    ExecutionAdmissionIssueCode, ExecutionAdmissionRequest, ExecutionAdmissionStatus,
    ExecutionCommitScope, ExecutionCommitStrategy, ExecutionPrincipalObservation,
    ExecutionPrincipalTrust, GateRevisionObservation, GateSnapshotObservation, GuaranteeStatus,
    ObligationEngineInputDocument, ReplayProtectionObservation, ReplayReservationStatus,
    RevisionExpectation, SnapshotCompleteness, EXECUTION_ADMISSION_SCHEMA_VERSION,
    EXECUTION_AUTHORITY_SCOPE,
};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

const NOW: i64 = 1_800_000_000;
const CLAIM_REF: &str = "contracts/claims/story-v2-010-active-claim.yaml";
const GATE_REF: &str = "contracts/gates/story-ready-lane-gate.yaml";
const EFFECT_REF: &str = "contracts/effects/story-artifact-write-effect.yaml";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let path = repo_root().join(relative);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    yaml_serde::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn verified_assurance_case() -> AssuranceCaseDocument {
    let input: ObligationEngineInputDocument =
        parse_yaml("docs/fixtures/obligation-engine-v0/verified-release.yaml");
    derive_assurance_case(&input).expect("derive verified Assurance Case")
}

// The fixture intentionally assembles the complete authority bundle in one
// place so reviewers can audit every admitted field without chasing helpers.
#[allow(clippy::too_many_lines)]
fn admitted_input() -> ExecutionAdmissionInputDocument {
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
    effect.tool_effect_contract.write_set.truncate(1);
    effect.tool_effect_contract.write_set[0].target_kind = EffectTargetKind::FilePath;
    "crates/forge-contracts/p4a-fixture.yaml"
        .clone_into(&mut effect.tool_effect_contract.write_set[0].reference);

    let mut claim: ClaimContractDocument = parse_yaml(CLAIM_REF);
    claim.claim_contract.claim.claimant_principal_id =
        Some(PrincipalId("principal.codex-main".to_owned()));
    claim.claim_contract.claim.claimant_agent_id = StableId("codex-main".to_owned());
    claim.claim_contract.claim.claimant_role = ActorRole::Driver;
    claim.claim_contract.lease.expected_state_version = state_version;
    claim.claim_contract.lease.expires_at = unix_to_rfc3339(NOW + 600);

    let gate: GateContractDocument = parse_yaml(GATE_REF);
    let request = ExecutionAdmissionRequest {
        id: StableId("admission.request.001".to_owned()),
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
        authority_snapshot_token: String::new(),
        expected_replay_reservation_revision: 19,
        nonce: "nonce-96-bits-minimum-fixture-001".to_owned(),
        issued_at_unix: NOW - 10,
    };

    let mut document = ExecutionAdmissionInputDocument {
        schema_version: EXECUTION_ADMISSION_SCHEMA_VERSION.to_owned(),
        execution_admission: ExecutionAdmissionInput {
            request,
            assurance_case,
            operation,
            command_contracts: vec![command],
            effect_contracts: vec![EffectContractBinding {
                effect_ref: RepoPath(EFFECT_REF.to_owned()),
                document: effect,
            }],
            principal: ExecutionPrincipalObservation {
                principal_id: PrincipalId("principal.codex-main".to_owned()),
                agent_id: StableId("codex-main".to_owned()),
                role: CallerRole::Driver,
                trust: ExecutionPrincipalTrust::AuthorizedKeyRegistry,
                credential_id: "key.codex-main.2026-01".to_owned(),
                audience: "forge-core:mcp:stdio".to_owned(),
                required_audience: "forge-core:mcp:stdio".to_owned(),
                authority_grants: vec![StableId(EXECUTION_AUTHORITY_SCOPE.to_owned())],
                attested_intent_digest: String::new(),
            },
            replay: ReplayProtectionObservation {
                status: ReplayReservationStatus::FreshReserved,
                nonce: "nonce-96-bits-minimum-fixture-001".to_owned(),
                reserved_intent_digest: String::new(),
                reservation_revision: 19,
            },
            claim_snapshot: ClaimSnapshotObservation {
                revision: 11,
                completeness: SnapshotCompleteness::Complete,
                claims: vec![ClaimRevisionObservation {
                    claim_ref: RepoPath(CLAIM_REF.to_owned()),
                    revision: 7,
                    document: claim,
                }],
            },
            gate_snapshot: GateSnapshotObservation {
                revision: 5,
                completeness: SnapshotCompleteness::Complete,
                gates: vec![GateRevisionObservation {
                    gate_ref: RepoPath(GATE_REF.to_owned()),
                    revision: 3,
                    observed_state_version: state_version,
                    document: gate,
                }],
            },
            commit: CommitAssuranceObservation {
                strategy: ExecutionCommitStrategy::SingleEffectWal,
                scope: ExecutionCommitScope::SingleEffect,
                wal_lock: GuaranteeStatus::Verified,
                rollback_recovery: GuaranteeStatus::Verified,
                durable_commit_record: GuaranteeStatus::Verified,
                compensation: CompensationCoverage::NotApplicable,
            },
            current_state_version: state_version,
            now_unix: NOW,
            max_attestation_age_seconds: 300,
            max_future_skew_seconds: 30,
        },
    };
    rebind_authority_snapshot(&mut document);
    rebind_intent(&mut document);
    document
}

fn rebind_authority_snapshot(document: &mut ExecutionAdmissionInputDocument) {
    let input = &mut document.execution_admission;
    input.request.authority_snapshot_token = authority_snapshot_token(
        &input.claim_snapshot,
        &input.gate_snapshot,
        input.current_state_version,
        input.now_unix,
    )
    .expect("authority snapshot token");
}

fn rebind_intent(document: &mut ExecutionAdmissionInputDocument) {
    let input = &mut document.execution_admission;
    let digest = execution_intent_digest(&input.request).expect("intent digest");
    input.principal.attested_intent_digest.clone_from(&digest);
    input.replay.reserved_intent_digest = digest;
    input.replay.nonce.clone_from(&input.request.nonce);
}

fn rebind_effect_token(document: &mut ExecutionAdmissionInputDocument) {
    let input = &mut document.execution_admission;
    input.request.effect_bindings[0].token =
        effect_contract_token(&input.effect_contracts[0].document).expect("effect token");
    rebind_intent(document);
}

fn issue_codes(document: &ExecutionAdmissionInputDocument) -> Vec<ExecutionAdmissionIssueCode> {
    evaluate_execution_admission(document)
        .expect("typed input should evaluate")
        .issues
        .into_iter()
        .map(|issue| issue.code)
        .collect()
}

#[test]
fn complete_single_effect_wal_snapshot_is_admitted() {
    let decision = evaluate_execution_admission(&admitted_input()).expect("admission decision");

    assert_eq!(decision.status, ExecutionAdmissionStatus::Admitted);
    assert!(decision.issues.is_empty());
    assert_eq!(decision.validated_claim_revisions.len(), 1);
    assert_eq!(decision.validated_gate_revisions.len(), 1);
    assert!(decision.intent_digest.starts_with("sha256:"));
}

#[test]
fn agent_only_claim_cannot_stand_in_for_verified_execution_principal() {
    let mut input = admitted_input();
    input.execution_admission.claim_snapshot.claims[0]
        .document
        .claim_contract
        .claim
        .claimant_principal_id = None;
    rebind_authority_snapshot(&mut input);
    rebind_intent(&mut input);
    let decision = evaluate_execution_admission(&input).expect("deterministic decision");
    assert_eq!(decision.status, ExecutionAdmissionStatus::Blocked);
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == ExecutionAdmissionIssueCode::ClaimPrincipalMismatch));
}

#[test]
fn signature_from_caller_selected_key_is_not_authority() {
    let mut input = admitted_input();
    input.execution_admission.principal.trust = ExecutionPrincipalTrust::SignatureVerified;

    let decision = evaluate_execution_admission(&input).expect("admission decision");

    assert_eq!(decision.status, ExecutionAdmissionStatus::Blocked);
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == ExecutionAdmissionIssueCode::PrincipalNotTrusted));
}

#[test]
fn principal_role_must_match_attested_request_and_operation() {
    let mut input = admitted_input();
    input.execution_admission.principal.role = CallerRole::Worker;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::PrincipalRoleMismatch));
}

#[test]
fn replayed_nonce_is_blocked_even_with_valid_binding() {
    let mut input = admitted_input();
    input.execution_admission.replay.status = ReplayReservationStatus::AlreadySeen;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::InvocationReplayRejected));
}

#[test]
fn stale_claim_revision_is_blocked_at_commit_snapshot() {
    let mut input = admitted_input();
    input.execution_admission.request.expected_claim_revisions[0].revision = 6;
    rebind_intent(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::ClaimRevisionMismatch));
}

#[test]
fn gate_from_old_project_state_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.gate_snapshot.gates[0].observed_state_version -= 1;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::GateStateVersionMismatch));
}

#[test]
fn mutable_authority_snapshot_cannot_change_after_intent_is_signed() {
    let mut input = admitted_input();
    input.execution_admission.now_unix += 1;

    assert!(
        issue_codes(&input).contains(&ExecutionAdmissionIssueCode::AuthoritySnapshotTokenMismatch)
    );
}

#[test]
fn passing_gate_without_evidence_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.gate_snapshot.gates[0]
        .document
        .gate_contract
        .evidence_refs
        .clear();

    let decision = evaluate_execution_admission(&input).expect("admission decision");
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == ExecutionAdmissionIssueCode::GateEvidenceMissing));
    assert!(decision.validated_gate_revisions.is_empty());
}

#[test]
fn tampered_assurance_case_cannot_reuse_old_resume_token() {
    let mut input = admitted_input();
    input
        .execution_admission
        .assurance_case
        .assurance_case
        .intent
        .desired_outcome
        .push_str(" tampered");
    rebind_intent(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::AssuranceTokenMismatch));
}

#[test]
fn tampered_operation_cannot_reuse_old_contract_token() {
    let mut input = admitted_input();
    input
        .execution_admission
        .operation
        .operation_contract
        .created_at
        .push_str(".tampered");
    rebind_intent(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::OperationTokenMismatch));
}

#[test]
fn tampered_command_cannot_reuse_old_contract_token() {
    let mut input = admitted_input();
    input.execution_admission.command_contracts[0]
        .command_contract
        .args
        .push("--all-targets".to_owned());

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::CommandTokenMismatch));
}

#[test]
fn tampered_effect_cannot_reuse_old_contract_token() {
    let mut input = admitted_input();
    input.execution_admission.effect_contracts[0]
        .document
        .tool_effect_contract
        .write_set[0]
        .reference
        .push_str(".tampered");

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::EffectTokenMismatch));
}

#[test]
fn replay_reservation_revision_must_match_attested_request() {
    let mut input = admitted_input();
    input.execution_admission.replay.reservation_revision += 1;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::ReplayRevisionMismatch));
}

#[test]
fn duplicate_content_addressed_binding_is_blocked() {
    let mut input = admitted_input();
    let duplicate = input.execution_admission.request.command_bindings[0].clone();
    input
        .execution_admission
        .request
        .command_bindings
        .push(duplicate);
    rebind_intent(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::DuplicateBinding));
}

#[test]
fn duplicate_claim_snapshot_observation_is_blocked() {
    let mut input = admitted_input();
    let duplicate = input.execution_admission.claim_snapshot.claims[0].clone();
    input
        .execution_admission
        .claim_snapshot
        .claims
        .push(duplicate);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::DuplicateBinding));
}

#[test]
fn duplicate_gate_snapshot_observation_is_blocked() {
    let mut input = admitted_input();
    let duplicate = input.execution_admission.gate_snapshot.gates[0].clone();
    input
        .execution_admission
        .gate_snapshot
        .gates
        .push(duplicate);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::DuplicateBinding));
}

#[test]
fn file_backed_artifact_write_outside_claim_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.effect_contracts[0]
        .document
        .tool_effect_contract
        .write_set[0]
        .target_kind = EffectTargetKind::ArtifactId;
    ".forge-method/artifacts/outside-claim.yaml".clone_into(
        &mut input.execution_admission.effect_contracts[0]
            .document
            .tool_effect_contract
            .write_set[0]
            .reference,
    );
    rebind_effect_token(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::ClaimCoverageMissing));
}

#[test]
fn non_transactional_scope_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.commit.scope = ExecutionCommitScope::PerEffectOnly;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::CommitScopeInsufficient));
}

#[test]
fn operation_wide_wal_is_fail_closed_until_runtime_support_exists() {
    let mut input = admitted_input();
    input.execution_admission.commit.strategy = ExecutionCommitStrategy::OperationWideWal;
    input.execution_admission.commit.scope = ExecutionCommitScope::WholeOperation;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::CommitStrategyUnsupported));
}

#[test]
fn saga_is_fail_closed_until_runtime_support_exists() {
    let mut input = admitted_input();
    input.execution_admission.commit.strategy = ExecutionCommitStrategy::Saga;
    input.execution_admission.commit.scope = ExecutionCommitScope::WholeOperation;
    input.execution_admission.commit.compensation = CompensationCoverage::Complete;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::CommitStrategyUnsupported));
}

#[test]
fn command_with_network_side_effect_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.command_contracts[0]
        .command_contract
        .network_policy = forge_core_contracts::command::NetworkPolicy::Allowed;

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::CommandNotSafelyReadOnly));
}

#[test]
fn expired_attestation_is_blocked() {
    let mut input = admitted_input();
    input.execution_admission.request.issued_at_unix = NOW - 301;
    rebind_intent(&mut input);

    assert!(issue_codes(&input).contains(&ExecutionAdmissionIssueCode::InvocationExpired));
}

#[test]
fn decisions_are_byte_deterministic() {
    let input = admitted_input();
    let first = evaluate_execution_admission(&input).expect("first decision");
    let second = evaluate_execution_admission(&input).expect("second decision");

    assert_eq!(first, second);
    assert_eq!(
        yaml_serde::to_string(&first).expect("serialize first"),
        yaml_serde::to_string(&second).expect("serialize second")
    );
}

#[test]
fn unsupported_input_schema_is_rejected() {
    let mut input = admitted_input();
    input.schema_version = "99".to_owned();

    let rejection = evaluate_execution_admission(&input).expect_err("schema must reject");

    assert!(rejection
        .to_string()
        .contains("unsupported execution admission schema"));
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionScenarioMatrix {
    schema_version: String,
    scenarios: Vec<AdmissionScenario>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionScenario {
    id: String,
    mutation: AdmissionFixtureMutation,
    expected_status: ExecutionAdmissionStatus,
    expected_issue: Option<ExecutionAdmissionIssueCode>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdmissionFixtureMutation {
    None,
    SignatureOnlyPrincipal,
    PrincipalRoleMismatch,
    ReplayedNonce,
    StaleClaimRevision,
    StaleGateState,
    MissingGateEvidence,
    TamperedAssuranceCase,
    TamperedOperation,
    TamperedCommand,
    TamperedEffect,
    ReplayRevisionMismatch,
    DuplicateCommandBinding,
    DuplicateClaimObservation,
    DuplicateGateObservation,
    UngovernedArtifactWrite,
    PerEffectCommit,
    OperationWideCommit,
    SagaCommit,
    NetworkCommand,
    ExpiredInvocation,
}

#[test]
fn published_p4a_scenario_matrix_matches_engine_verdicts() {
    let matrix: AdmissionScenarioMatrix =
        parse_yaml("docs/fixtures/execution-admission-v0/scenario-matrix.yaml");
    assert_eq!(matrix.schema_version, EXECUTION_ADMISSION_SCHEMA_VERSION);

    for scenario in matrix.scenarios {
        let mut input = admitted_input();
        apply_fixture_mutation(&mut input, scenario.mutation);
        let decision = evaluate_execution_admission(&input)
            .unwrap_or_else(|error| panic!("scenario {} rejected input: {error}", scenario.id));
        assert_eq!(
            decision.status, scenario.expected_status,
            "scenario {}: {decision:#?}",
            scenario.id
        );
        if let Some(expected) = scenario.expected_issue {
            assert!(
                decision.issues.iter().any(|issue| issue.code == expected),
                "scenario {} missing {expected:?}: {decision:#?}",
                scenario.id
            );
        } else {
            assert!(
                decision.issues.is_empty(),
                "scenario {} expected no issues: {decision:#?}",
                scenario.id
            );
        }
    }
}

// Keeping the published mutation vocabulary in one exhaustive match makes the
// YAML scenario matrix auditable and guarantees new enum cases update it.
#[allow(clippy::too_many_lines)]
fn apply_fixture_mutation(
    input: &mut ExecutionAdmissionInputDocument,
    mutation: AdmissionFixtureMutation,
) {
    match mutation {
        AdmissionFixtureMutation::None => {}
        AdmissionFixtureMutation::SignatureOnlyPrincipal => {
            input.execution_admission.principal.trust = ExecutionPrincipalTrust::SignatureVerified;
        }
        AdmissionFixtureMutation::PrincipalRoleMismatch => {
            input.execution_admission.principal.role = CallerRole::Worker;
        }
        AdmissionFixtureMutation::ReplayedNonce => {
            input.execution_admission.replay.status = ReplayReservationStatus::AlreadySeen;
        }
        AdmissionFixtureMutation::StaleClaimRevision => {
            input.execution_admission.request.expected_claim_revisions[0].revision -= 1;
            rebind_intent(input);
        }
        AdmissionFixtureMutation::StaleGateState => {
            input.execution_admission.gate_snapshot.gates[0].observed_state_version -= 1;
        }
        AdmissionFixtureMutation::MissingGateEvidence => {
            input.execution_admission.gate_snapshot.gates[0]
                .document
                .gate_contract
                .evidence_refs
                .clear();
        }
        AdmissionFixtureMutation::TamperedAssuranceCase => {
            input
                .execution_admission
                .assurance_case
                .assurance_case
                .intent
                .desired_outcome
                .push_str(" tampered");
            rebind_intent(input);
        }
        AdmissionFixtureMutation::TamperedOperation => {
            input
                .execution_admission
                .operation
                .operation_contract
                .created_at
                .push_str(".tampered");
            rebind_intent(input);
        }
        AdmissionFixtureMutation::TamperedCommand => {
            input.execution_admission.command_contracts[0]
                .command_contract
                .args
                .push("--all-targets".to_owned());
        }
        AdmissionFixtureMutation::TamperedEffect => {
            input.execution_admission.effect_contracts[0]
                .document
                .tool_effect_contract
                .write_set[0]
                .reference
                .push_str(".tampered");
        }
        AdmissionFixtureMutation::ReplayRevisionMismatch => {
            input.execution_admission.replay.reservation_revision += 1;
        }
        AdmissionFixtureMutation::DuplicateCommandBinding => {
            let duplicate = input.execution_admission.request.command_bindings[0].clone();
            input
                .execution_admission
                .request
                .command_bindings
                .push(duplicate);
            rebind_intent(input);
        }
        AdmissionFixtureMutation::DuplicateClaimObservation => {
            let duplicate = input.execution_admission.claim_snapshot.claims[0].clone();
            input
                .execution_admission
                .claim_snapshot
                .claims
                .push(duplicate);
        }
        AdmissionFixtureMutation::DuplicateGateObservation => {
            let duplicate = input.execution_admission.gate_snapshot.gates[0].clone();
            input
                .execution_admission
                .gate_snapshot
                .gates
                .push(duplicate);
        }
        AdmissionFixtureMutation::UngovernedArtifactWrite => {
            input.execution_admission.effect_contracts[0]
                .document
                .tool_effect_contract
                .write_set[0]
                .target_kind = EffectTargetKind::ArtifactId;
            ".forge-method/artifacts/outside-claim.yaml".clone_into(
                &mut input.execution_admission.effect_contracts[0]
                    .document
                    .tool_effect_contract
                    .write_set[0]
                    .reference,
            );
            rebind_effect_token(input);
        }
        AdmissionFixtureMutation::PerEffectCommit => {
            input.execution_admission.commit.scope = ExecutionCommitScope::PerEffectOnly;
        }
        AdmissionFixtureMutation::OperationWideCommit => {
            input.execution_admission.commit.strategy = ExecutionCommitStrategy::OperationWideWal;
            input.execution_admission.commit.scope = ExecutionCommitScope::WholeOperation;
        }
        AdmissionFixtureMutation::SagaCommit => {
            input.execution_admission.commit.strategy = ExecutionCommitStrategy::Saga;
            input.execution_admission.commit.scope = ExecutionCommitScope::WholeOperation;
            input.execution_admission.commit.compensation = CompensationCoverage::Complete;
        }
        AdmissionFixtureMutation::NetworkCommand => {
            input.execution_admission.command_contracts[0]
                .command_contract
                .network_policy = forge_core_contracts::command::NetworkPolicy::Allowed;
        }
        AdmissionFixtureMutation::ExpiredInvocation => {
            input.execution_admission.request.issued_at_unix = NOW - 301;
            rebind_intent(input);
        }
    }
}
