use forge_core_contracts::{
    UniversalAssuranceLens, WorkflowCompletionAssertion, WorkflowDecisionActivation,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluationDocument,
    WorkflowGovernanceEvent, WorkflowGovernanceLedgerDocument, WorkflowPolicyActivation,
    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use std::{collections::BTreeSet, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(repo_root().join(relative)).expect("published P5b fixture")
}

#[test]
fn published_bundle_round_trips_as_a_closed_typed_contract() {
    let yaml = read("contracts/workflow-governance/kernel-v0.yaml");
    let document: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&yaml).expect("typed workflow governance bundle");
    assert_eq!(document.schema_version, WORKFLOW_GOVERNANCE_SCHEMA_VERSION);
    assert_eq!(document.workflow_governance_bundle.policies.len(), 2);
    let build = document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build-story policy");
    assert_eq!(build.obligations.len(), 2);
    assert_eq!(build.claims.len(), 2);
    assert!(build
        .claims
        .iter()
        .all(|claim| claim.assurance_lenses.is_empty()));
    assert_eq!(build.evaluators.len(), 2);
    assert_eq!(build.capability_requirements.len(), 1);
    assert_eq!(build.decision_rules.len(), 1);
    assert_eq!(
        build.decision_rules[0].activation,
        WorkflowDecisionActivation::ObservedNeed
    );

    let serialized = yaml_serde::to_string(&document).expect("serialize governance bundle");
    assert!(
        !serialized.contains("assurance_lenses:"),
        "legacy empty lens tags must retain the historical wire shape"
    );
    let round_trip: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&serialized).expect("round-trip governance bundle");
    assert_eq!(round_trip, document);
}

#[test]
fn claim_lens_tags_are_typed_while_historical_omission_defaults_empty() {
    let historical = r"
id: claim.workflow.historical
statement: Historical claim
evaluator_ref: evaluator.workflow.historical
waiver:
  kind: not_waivable
";
    let historical: forge_core_contracts::WorkflowClaimPolicy =
        yaml_serde::from_str(historical).expect("historical claim without lens tags");
    assert!(historical.assurance_lenses.is_empty());
    let historical_wire =
        yaml_serde::to_string(&historical).expect("serialize historical claim without lens tags");
    assert!(!historical_wire.contains("assurance_lenses:"));
    let historical_round_trip: forge_core_contracts::WorkflowClaimPolicy =
        yaml_serde::from_str(&historical_wire).expect("round-trip historical claim wire");
    assert_eq!(historical_round_trip, historical);

    let tagged_yaml = r"
id: claim.workflow.assurance.intended-outcome
statement: Accepted intent defines observable success.
evaluator_ref: evaluator.workflow.assurance.intended-outcome
assurance_lenses:
  - intended_outcome
waiver:
  kind: not_waivable
";
    let tagged: forge_core_contracts::WorkflowClaimPolicy =
        yaml_serde::from_str(tagged_yaml).expect("typed lens-aware claim");
    assert_eq!(
        tagged.assurance_lenses,
        vec![UniversalAssuranceLens::IntendedOutcome]
    );
    let tagged_wire = yaml_serde::to_string(&tagged).expect("serialize tagged claim");
    assert!(tagged_wire.contains("assurance_lenses:"));

    let unknown = tagged_yaml.replace("intended_outcome", "caller_invented_lens");
    assert!(yaml_serde::from_str::<forge_core_contracts::WorkflowClaimPolicy>(&unknown).is_err());
}

#[test]
fn published_evaluations_round_trip_as_closed_typed_contracts() {
    for name in [
        "complete",
        "active",
        "missing-evidence",
        "missing-capability",
        "human-decision",
        "contradictory-evidence",
        "invented-completion",
    ] {
        let yaml = read(&format!(
            "docs/fixtures/workflow-governance-kernel-v0/{name}.yaml"
        ));
        let document: WorkflowGovernanceEvaluationDocument =
            yaml_serde::from_str(&yaml).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(document.schema_version, WORKFLOW_GOVERNANCE_SCHEMA_VERSION);
        let serialized = yaml_serde::to_string(&document).expect("serialize evaluation");
        let round_trip: WorkflowGovernanceEvaluationDocument =
            yaml_serde::from_str(&serialized).expect("round-trip evaluation");
        assert_eq!(round_trip, document, "fixture {name}");
    }

    let invented: WorkflowGovernanceEvaluationDocument = yaml_serde::from_str(&read(
        "docs/fixtures/workflow-governance-kernel-v0/invented-completion.yaml",
    ))
    .expect("invented completion fixture");
    assert_eq!(
        invented.workflow_governance_evaluation.completion_assertion,
        WorkflowCompletionAssertion::Asserted
    );
}

#[test]
fn bundle_and_evaluation_reject_unknown_fields_and_enum_values() {
    let bundle = read("contracts/workflow-governance/kernel-v0.yaml");
    let unknown_bundle_field = bundle.replace(
        "  id: bundle.workflow-governance.kernel-v0",
        "  id: bundle.workflow-governance.kernel-v0\n  caller_can_override: true",
    );
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(&unknown_bundle_field).is_err()
    );
    let unsafe_activation =
        bundle.replace("activation: observed_need", "activation: agent_decides");
    assert!(yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(&unsafe_activation).is_err());
    let verified_activation: WorkflowDecisionActivation =
        yaml_serde::from_str("claim_verified\n").expect("claim-verified wire value");
    assert_eq!(
        verified_activation,
        WorkflowDecisionActivation::ClaimVerified
    );
    let all_verified_activation: WorkflowDecisionActivation =
        yaml_serde::from_str("all_claims_verified\n").expect("all-claims-verified wire value");
    assert_eq!(
        all_verified_activation,
        WorkflowDecisionActivation::AllClaimsVerified
    );

    let evaluation = read("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
    let unknown_evaluation_field = evaluation.replace(
        "  state_version: 7",
        "  state_version: 7\n  skip_governance: true",
    );
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceEvaluationDocument>(&unknown_evaluation_field)
            .is_err()
    );
    let invented_outcome = evaluation.replace("outcome: pass", "outcome: assumed");
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceEvaluationDocument>(&invented_outcome).is_err()
    );
}

#[test]
fn golden_path_has_exact_p5a_coverage_and_deterministic_routing() {
    let document: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&read("contracts/workflow-governance/golden-path-v0.yaml"))
            .expect("typed P5c golden-path bundle");
    let policies = &document.workflow_governance_bundle.policies;
    assert_eq!(policies.len(), 15);

    let actual = policies
        .iter()
        .map(|policy| policy.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let expected = [
        "policy.workflow.discover-intent",
        "policy.workflow.domain-scan",
        "policy.workflow.technical-feasibility-scan",
        "policy.workflow.product-requirements",
        "policy.workflow.write-spec",
        "policy.workflow.architecture",
        "policy.workflow.plan-sprint",
        "policy.workflow.story-creation",
        "policy.workflow.build-story",
        "policy.workflow.test-strategy",
        "policy.workflow.reality-evidence-gate",
        "policy.workflow.correct-course",
        "policy.workflow.readiness-check",
        "policy.workflow.ready-release",
        "policy.workflow.context-recovery",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    let priorities = policies
        .iter()
        .map(|policy| policy.routing.priority)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        priorities.len(),
        policies.len(),
        "priorities must be unique"
    );
    let known = policies
        .iter()
        .map(|policy| policy.id.0.as_str())
        .collect::<BTreeSet<_>>();
    for policy in policies {
        for prerequisite in &policy.prerequisites {
            assert!(known.contains(prerequisite.policy_ref.0.as_str()));
            assert_ne!(prerequisite.policy_ref, policy.id);
            let prerequisite_priority = policies
                .iter()
                .find(|candidate| candidate.id == prerequisite.policy_ref)
                .expect("known prerequisite")
                .routing
                .priority;
            assert!(
                prerequisite_priority < policy.routing.priority,
                "dependency priority must form a deterministic DAG"
            );
        }
    }

    for id in [
        "policy.workflow.domain-scan",
        "policy.workflow.technical-feasibility-scan",
        "policy.workflow.architecture",
    ] {
        assert_eq!(
            policies
                .iter()
                .find(|policy| policy.id.0 == id)
                .unwrap()
                .routing
                .activation,
            WorkflowPolicyActivation::WhenApplicable
        );
    }
    for id in [
        "policy.workflow.context-recovery",
        "policy.workflow.correct-course",
        "policy.workflow.ready-release",
    ] {
        let policy = policies.iter().find(|policy| policy.id.0 == id).unwrap();
        assert_eq!(
            policy.routing.activation,
            WorkflowPolicyActivation::OnSignal
        );
        assert!(!policy.routing.signals.is_empty());
    }
    assert!(policies
        .iter()
        .flat_map(|policy| &policy.obligations)
        .all(|obligation| obligation.required_before.rank() <= 2));

    let serialized = yaml_serde::to_string(&document).expect("serialize golden path");
    let round_trip: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&serialized).expect("round-trip golden path");
    assert_eq!(round_trip, document);
}

#[test]
// This intentionally audits every closed ledger event in one ordered fixture;
// splitting it would hide the cross-event hash/binding assertions it protects.
#[allow(clippy::too_many_lines)]
fn durable_ledger_round_trips_every_closed_receipt_event() {
    let yaml = read("docs/fixtures/workflow-governance-golden-path-v0/ledger-all-events.yaml");
    let document: WorkflowGovernanceLedgerDocument =
        yaml_serde::from_str(&yaml).expect("typed governance ledger");
    assert_eq!(
        document.schema_version,
        WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
    );
    let records = &document.workflow_governance_ledger.records;
    assert_eq!(records.len(), 12);
    assert_eq!(records[0].sequence, 1);
    assert!(records[0].previous_record_digest.is_none());
    for pair in records.windows(2) {
        assert_eq!(pair[1].sequence, pair[0].sequence + 1);
        assert_eq!(
            pair[1].previous_record_digest.as_deref(),
            Some(pair[0].record_digest.as_str())
        );
        assert_eq!(pair[1].project_id, pair[0].project_id);
        assert_eq!(pair[1].bundle_digest, pair[0].bundle_digest);
    }
    assert!(matches!(
        records[0].event,
        WorkflowGovernanceEvent::ProjectImported(_)
    ));
    assert!(matches!(
        records[1].event,
        WorkflowGovernanceEvent::PhaseAdvanced(_)
    ));
    assert!(matches!(
        records[2].event,
        WorkflowGovernanceEvent::ApplicabilityAssessed(_)
    ));
    assert!(matches!(
        records[3].event,
        WorkflowGovernanceEvent::SignalChanged(_)
    ));
    assert!(matches!(
        records[4].event,
        WorkflowGovernanceEvent::CapabilityProbed(_)
    ));
    assert!(matches!(
        records[5].event,
        WorkflowGovernanceEvent::DecisionNeedRaised(_)
    ));
    assert!(matches!(
        records[6].event,
        WorkflowGovernanceEvent::DecisionResolved(_)
    ));
    assert!(matches!(
        records[7].event,
        WorkflowGovernanceEvent::EvaluatorObserved(_)
    ));
    assert!(matches!(
        records[8].event,
        WorkflowGovernanceEvent::WaiverAuthorized(_)
    ));
    assert!(matches!(
        records[9].event,
        WorkflowGovernanceEvent::PolicyCompleted(_)
    ));
    assert!(matches!(
        records[10].event,
        WorkflowGovernanceEvent::ReceiptRevoked(_)
    ));
    assert!(matches!(
        records[11].event,
        WorkflowGovernanceEvent::ContinuityRecorded(_)
    ));
    let WorkflowGovernanceEvent::ApplicabilityAssessed(applicability) = &records[2].event else {
        unreachable!("checked applicability variant")
    };
    assert!(!applicability.basis.is_empty());
    assert!(applicability
        .basis
        .iter()
        .all(|basis| basis.subject_digest.starts_with("sha256:")));
    assert_eq!(
        applicability.ledger_head_digest,
        records[2].previous_record_digest.clone().unwrap()
    );
    let WorkflowGovernanceEvent::SignalChanged(signal) = &records[3].event else {
        unreachable!("checked signal variant")
    };
    assert_eq!(signal.generation, 1);
    assert_eq!(signal.episode_id.0, "episode.course-correction.1");
    assert!(!signal.basis.is_empty());
    assert_eq!(
        signal.ledger_head_digest,
        records[3].previous_record_digest.clone().unwrap()
    );
    assert!(signal.authorization_registry_digest.starts_with("sha256:"));
    let WorkflowGovernanceEvent::EvaluatorObserved(observation) = &records[7].event else {
        unreachable!("checked evaluator variant")
    };
    assert!(observation
        .provenance
        .scenario_digest
        .starts_with("sha256:"));
    assert_eq!(
        observation.provenance.semantic_identity.0,
        "scenario.build-story.representative-execution.v1"
    );
    assert!(observation
        .authorization_registry_digest
        .starts_with("sha256:"));
    let WorkflowGovernanceEvent::PolicyCompleted(completion) = &records[9].event else {
        unreachable!("checked completion variant")
    };
    assert!(!completion.dependency_receipt_digests.is_empty());
    assert!(!completion.evidence_receipt_digests.is_empty());
    assert_eq!(completion.unresolved_deferred_obligation_refs.len(), 1);
    assert_eq!(completion.unresolved_deferred_capability_refs.len(), 1);

    let serialized = yaml_serde::to_string(&document).expect("serialize ledger");
    let round_trip: WorkflowGovernanceLedgerDocument =
        yaml_serde::from_str(&serialized).expect("round-trip ledger");
    assert_eq!(round_trip, document);
}

#[test]
fn schemas_are_generatable_and_ledger_payloads_reject_unknown_fields() {
    let bundle_schema = schemars::schema_for!(WorkflowGovernanceBundleDocument);
    let ledger_schema = schemars::schema_for!(WorkflowGovernanceLedgerDocument);
    let bundle_json = serde_json::to_value(bundle_schema).expect("bundle JSON schema");
    let ledger_json = serde_json::to_value(ledger_schema).expect("ledger JSON schema");
    assert!(bundle_json.to_string().contains("WorkflowPolicyRouting"));
    assert!(ledger_json
        .to_string()
        .contains("WorkflowGovernanceLedgerRecord"));

    let ledger = read("docs/fixtures/workflow-governance-golden-path-v0/ledger-all-events.yaml");
    let unknown_payload = ledger.replacen(
        "        initial_phase: 1-discovery",
        "        initial_phase: 1-discovery\n        agent_says_valid: true",
        1,
    );
    assert!(yaml_serde::from_str::<WorkflowGovernanceLedgerDocument>(&unknown_payload).is_err());
    let unknown_envelope = ledger.replacen(
        "    sequence: 1",
        "    sequence: 1\n    skip_hash_check: true",
        1,
    );
    assert!(yaml_serde::from_str::<WorkflowGovernanceLedgerDocument>(&unknown_envelope).is_err());
}
