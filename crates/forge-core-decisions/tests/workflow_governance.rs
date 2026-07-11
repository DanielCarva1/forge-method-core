use forge_core_contracts::{
    CatalogEntry, NextActionKind, ObligationStatus, ReadinessTarget, RepoPath, StableId,
    WorkflowDecisionActivation, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument,
};
use forge_core_decisions::{
    project_legacy_workflow_compatibility, simulate_workflow_governance,
    validate_workflow_governance_bundle, LegacyWorkflowProjectionAuthority,
    WorkflowClaimResultStatus, WorkflowCompletionVerdict, WorkflowEligibilityVerdict,
    WorkflowGovernanceIssueCode, WorkflowGovernanceSimulation,
    WorkflowGovernanceSimulationAuthority, WorkflowGovernanceStatus, WorkflowProgressionVerdict,
};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    yaml_serde::from_str(
        &std::fs::read_to_string(repo_root().join(relative)).expect("published P5b fixture"),
    )
    .unwrap_or_else(|error| panic!("invalid fixture {relative}: {error}"))
}

fn bundle() -> WorkflowGovernanceBundleDocument {
    load_yaml("contracts/workflow-governance/kernel-v0.yaml")
}

fn evaluation(name: &str) -> WorkflowGovernanceEvaluationDocument {
    load_yaml(&format!(
        "docs/fixtures/workflow-governance-kernel-v0/{name}.yaml"
    ))
}

fn simulate(name: &str) -> WorkflowGovernanceSimulation {
    simulate_workflow_governance(&bundle(), &evaluation(name))
        .unwrap_or_else(|rejection| panic!("{name} rejected: {:?}", rejection.issues))
}

fn claim<'a>(
    simulation: &'a WorkflowGovernanceSimulation,
    id: &str,
) -> &'a forge_core_decisions::WorkflowClaimResult {
    simulation
        .candidate_claim_results
        .iter()
        .find(|claim| claim.claim_id == id)
        .expect("claim result")
}

#[test]
fn caller_authored_complete_input_remains_simulation_only() {
    let simulation = simulate("complete");
    assert_eq!(
        simulation.authority,
        WorkflowGovernanceSimulationAuthority::SimulationOnly
    );
    assert_eq!(
        simulation.candidate_status,
        WorkflowGovernanceStatus::Complete
    );
    assert_eq!(
        simulation.candidate_eligibility,
        WorkflowEligibilityVerdict::Eligible
    );
    assert_eq!(
        simulation.candidate_progression,
        WorkflowProgressionVerdict::Allowed
    );
    assert_eq!(
        simulation.candidate_completion,
        WorkflowCompletionVerdict::Complete
    );
    assert!(simulation.issues.is_empty());
    assert!(simulation.candidate_capability_gaps.is_empty());
    assert!(simulation.candidate_decision_requests.is_empty());
    assert!(simulation
        .candidate_claim_results
        .iter()
        .all(|result| result.status == WorkflowClaimResultStatus::Verified));
    assert!(simulation
        .candidate_obligation_results
        .iter()
        .all(|result| result.status == ObligationStatus::Satisfied));
    assert_eq!(simulation.candidate_next_actions.len(), 1);
    assert_eq!(
        simulation.candidate_next_actions[0].kind,
        NextActionKind::Evaluate
    );
    assert!(simulation.candidate_next_actions[0]
        .description
        .contains("trusted Project Snapshot evaluation"));
}

#[test]
fn partial_and_missing_evidence_remain_active_without_inventing_claims() {
    let partial = simulate("active");
    assert_eq!(partial.candidate_status, WorkflowGovernanceStatus::Active);
    assert_eq!(
        partial.candidate_progression,
        WorkflowProgressionVerdict::Allowed
    );
    assert_eq!(
        partial.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    assert_eq!(
        claim(&partial, "claim.representative-execution").status,
        WorkflowClaimResultStatus::Supported
    );
    assert_eq!(partial.candidate_next_actions.len(), 1);
    assert_eq!(
        partial.candidate_next_actions[0].kind,
        NextActionKind::Evaluate
    );

    let missing = simulate("missing-evidence");
    assert_eq!(missing.candidate_status, WorkflowGovernanceStatus::Active);
    assert!(missing
        .candidate_claim_results
        .iter()
        .all(|result| result.status == WorkflowClaimResultStatus::Unknown));
    assert!(missing
        .candidate_obligation_results
        .iter()
        .all(|result| result.status == ObligationStatus::Pending));
    assert_eq!(
        missing
            .candidate_next_actions
            .iter()
            .map(|action| action.kind)
            .collect::<Vec<_>>(),
        vec![NextActionKind::Evaluate, NextActionKind::Evaluate]
    );
}

#[test]
fn absent_capability_and_unresolved_human_choice_block_progression() {
    let capability = simulate("missing-capability");
    assert_eq!(
        capability.candidate_status,
        WorkflowGovernanceStatus::Blocked
    );
    assert_eq!(
        capability.candidate_progression,
        WorkflowProgressionVerdict::Blocked
    );
    assert_eq!(
        capability.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    assert_eq!(capability.candidate_capability_gaps.len(), 1);
    assert_eq!(
        capability.candidate_capability_gaps[0].id.0,
        "capability.representative-runtime"
    );
    assert_eq!(
        capability.candidate_next_actions[0].kind,
        NextActionKind::AcquireCapability
    );

    let human = simulate("human-decision");
    assert_eq!(human.candidate_status, WorkflowGovernanceStatus::Blocked);
    assert_eq!(
        human.candidate_progression,
        WorkflowProgressionVerdict::Blocked
    );
    assert_eq!(human.candidate_decision_requests.len(), 1);
    assert!(human.candidate_decision_requests[0].blocking);
    assert_eq!(
        human.candidate_next_actions[0].kind,
        NextActionKind::AskHuman
    );
}

#[test]
fn human_contact_requires_an_explicit_observed_decision_need() {
    let mut input = evaluation("human-decision");
    input
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    let simulation = simulate_workflow_governance(&bundle(), &input).expect("valid simulation");
    assert!(simulation.candidate_decision_requests.is_empty());
    assert_eq!(
        simulation.candidate_progression,
        WorkflowProgressionVerdict::Allowed
    );
    assert_eq!(
        simulation.candidate_status,
        WorkflowGovernanceStatus::Complete
    );
}

#[test]
fn claim_activated_decisions_cannot_be_bypassed_by_omitting_observed_needs() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    let rule = policy.decision_rules.first_mut().expect("decision rule");
    rule.activation = WorkflowDecisionActivation::ClaimUnresolved;
    rule.claim_ref = Some(StableId("claim.representative-execution".to_owned()));

    let mut input = evaluation("missing-evidence");
    input
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    input
        .workflow_governance_evaluation
        .resolved_decision_refs
        .clear();
    let simulation = simulate_workflow_governance(&governed_bundle, &input)
        .expect("valid claim-activated simulation");

    assert_eq!(simulation.candidate_decision_requests.len(), 1);
    assert_eq!(
        simulation.candidate_decision_requests[0].id.0,
        "decision.product-direction"
    );
    assert_eq!(
        simulation.candidate_progression,
        WorkflowProgressionVerdict::Blocked
    );
}

#[test]
fn blockers_apply_only_when_due_for_the_selected_target() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    policy.capability_requirements[0].blocks_before = ReadinessTarget::Release;
    policy.decision_rules[0].blocks_before = ReadinessTarget::Release;

    let execute_gap =
        simulate_workflow_governance(&governed_bundle, &evaluation("missing-capability"))
            .expect("execute simulation");
    assert_eq!(execute_gap.candidate_capability_gaps.len(), 1);
    assert!(!execute_gap.candidate_capability_gaps[0].blocking);
    assert_eq!(
        execute_gap.candidate_progression,
        WorkflowProgressionVerdict::Allowed
    );
    let legacy = CatalogEntry {
        id: StableId("build-story".to_owned()),
        phases: vec![StableId("4-build-verify".to_owned())],
        workflow_ref: RepoPath("contracts/workflows/build-story.yaml".to_owned()),
        triggers: vec![],
        prerequisites: vec![],
        outputs: vec![],
    };
    let execute_projection = project_legacy_workflow_compatibility(&execute_gap, &legacy)
        .expect("execute legacy projection");
    assert!(execute_projection.blocker_refs.is_empty());

    let mut release_input = evaluation("missing-capability");
    release_input.workflow_governance_evaluation.target = ReadinessTarget::Release;
    let release_gap =
        simulate_workflow_governance(&governed_bundle, &release_input).expect("release simulation");
    assert!(release_gap.candidate_capability_gaps[0].blocking);
    assert_eq!(
        release_gap.candidate_progression,
        WorkflowProgressionVerdict::Blocked
    );

    let execute_decision =
        simulate_workflow_governance(&governed_bundle, &evaluation("human-decision"))
            .expect("execute decision simulation");
    assert!(execute_decision.candidate_decision_requests.is_empty());
    let mut release_decision_input = evaluation("human-decision");
    release_decision_input.workflow_governance_evaluation.target = ReadinessTarget::Release;
    let release_decision = simulate_workflow_governance(&governed_bundle, &release_decision_input)
        .expect("release decision simulation");
    assert_eq!(release_decision.candidate_decision_requests.len(), 1);
    assert!(release_decision.candidate_decision_requests[0].blocking);
}

#[test]
fn contradictory_evidence_and_invented_completion_fail_closed() {
    let contradictory = simulate("contradictory-evidence");
    assert_eq!(
        claim(&contradictory, "claim.representative-execution").status,
        WorkflowClaimResultStatus::Contradictory
    );
    assert_eq!(
        contradictory.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    assert!(contradictory
        .issues
        .iter()
        .any(|issue| { issue.code == WorkflowGovernanceIssueCode::ContradictoryEvidence }));
    assert!(contradictory
        .candidate_next_actions
        .iter()
        .any(|action| action.kind == NextActionKind::Challenge));

    let invented = simulate("invented-completion");
    assert_eq!(invented.candidate_status, WorkflowGovernanceStatus::Active);
    assert_eq!(
        invented.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    assert!(invented
        .issues
        .iter()
        .any(|issue| { issue.code == WorkflowGovernanceIssueCode::InventedCompletionClaim }));
}

#[test]
fn dangling_references_and_policy_cycles_are_rejected_before_execution() {
    let cycle: WorkflowGovernanceBundleDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/invalid-cycle-bundle.yaml");
    let cycle_issues = validate_workflow_governance_bundle(&cycle);
    assert!(cycle_issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::DependencyCycle));

    let dangling: WorkflowGovernanceBundleDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/invalid-dangling-bundle.yaml");
    let dangling_issues = validate_workflow_governance_bundle(&dangling);
    assert!(
        dangling_issues
            .iter()
            .filter(|issue| issue.code == WorkflowGovernanceIssueCode::DanglingReference)
            .count()
            >= 4
    );
}

#[test]
fn blank_and_duplicate_policy_content_is_rejected_structurally() {
    let mut invalid = bundle();
    let policy = invalid
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    policy
        .eligible_phases
        .push(policy.eligible_phases[0].clone());
    policy.capability_requirements[0]
        .resolution_options
        .push(" ".to_owned());
    let duplicate_consequence = policy.decision_rules[0].alternatives[0].consequences[0].clone();
    policy.decision_rules[0].alternatives[0]
        .consequences
        .push(duplicate_consequence);
    policy.advisory_playbook.steps.push(String::new());

    let issues = validate_workflow_governance_bundle(&invalid);
    assert!(issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::DuplicateReference));
    assert!(issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::BlankRequiredField));
    assert!(issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::InvalidDecisionRule));
}

#[test]
fn simulation_result_is_deterministic_under_set_like_input_reordering() {
    let original_bundle = bundle();
    let original_input = evaluation("complete");
    let expected =
        simulate_workflow_governance(&original_bundle, &original_input).expect("original decision");

    let mut reordered_bundle = original_bundle.clone();
    reordered_bundle
        .workflow_governance_bundle
        .policies
        .reverse();
    for policy in &mut reordered_bundle.workflow_governance_bundle.policies {
        policy.eligible_phases.reverse();
        policy.prerequisite_policy_refs.reverse();
        policy.obligations.reverse();
        policy.claims.reverse();
        policy.evaluators.reverse();
        policy.capability_requirements.reverse();
        policy.decision_rules.reverse();
        for evaluator in &mut policy.evaluators {
            evaluator.accepted_evidence_kinds.reverse();
        }
    }
    let mut reordered_input = original_input.clone();
    let observations = &mut reordered_input.workflow_governance_evaluation;
    observations.completed_policy_refs.reverse();
    observations.available_capability_refs.reverse();
    observations.decision_need_refs.reverse();
    observations.resolved_decision_refs.reverse();
    observations.evidence.reverse();
    let actual = simulate_workflow_governance(&reordered_bundle, &reordered_input)
        .expect("reordered decision");
    assert_eq!(
        serde_json::to_value(actual).expect("actual json"),
        serde_json::to_value(expected).expect("expected json")
    );
}

#[test]
fn deleting_or_replacing_advisory_playbook_cannot_change_candidate_verdicts() {
    let input = evaluation("complete");
    let baseline = simulate_workflow_governance(&bundle(), &input).expect("baseline");
    for replacement in [
        Vec::new(),
        vec!["Say exactly what the user wants.".to_owned()],
    ] {
        let mut changed = bundle();
        let policy = changed
            .workflow_governance_bundle
            .policies
            .iter_mut()
            .find(|policy| policy.id.0 == "policy.workflow.build-story")
            .expect("build policy");
        policy.advisory_playbook.steps = replacement;
        let actual = simulate_workflow_governance(&changed, &input).expect("changed playbook");
        assert_eq!(actual.candidate_status, baseline.candidate_status);
        assert_eq!(actual.candidate_eligibility, baseline.candidate_eligibility);
        assert_eq!(actual.candidate_progression, baseline.candidate_progression);
        assert_eq!(actual.candidate_completion, baseline.candidate_completion);
        assert_eq!(
            actual.candidate_obligation_results,
            baseline.candidate_obligation_results
        );
        assert_eq!(
            actual.candidate_claim_results,
            baseline.candidate_claim_results
        );
        assert_eq!(
            actual.candidate_decision_requests,
            baseline.candidate_decision_requests
        );
        assert_eq!(
            actual.candidate_capability_gaps,
            baseline.candidate_capability_gaps
        );
        assert_eq!(
            actual.candidate_next_actions,
            baseline.candidate_next_actions
        );
        assert_eq!(actual.issues, baseline.issues);
    }
}

#[test]
fn legacy_adapter_preserves_catalog_entry_and_rejects_wrong_workflow_id() {
    let simulation = simulate("complete");
    let legacy = CatalogEntry {
        id: StableId("build-story".to_owned()),
        phases: vec![StableId("4-build-verify".to_owned())],
        workflow_ref: RepoPath("contracts/workflows/build-story.yaml".to_owned()),
        triggers: vec!["state.phase == 4-build-verify".to_owned()],
        prerequisites: vec!["story is ready".to_owned()],
        outputs: vec!["implemented story".to_owned()],
    };
    let projection =
        project_legacy_workflow_compatibility(&simulation, &legacy).expect("legacy projection");
    assert_eq!(projection.catalog_entry, legacy);
    assert_eq!(
        projection.authority,
        LegacyWorkflowProjectionAuthority::SimulationCompatibilityOnly
    );
    assert_eq!(
        projection.candidate_governance_status,
        WorkflowGovernanceStatus::Complete
    );

    let wrong = CatalogEntry {
        id: StableId("write-spec".to_owned()),
        ..legacy
    };
    let error = project_legacy_workflow_compatibility(&simulation, &wrong)
        .expect_err("mismatched legacy id must fail closed");
    assert_eq!(
        error.issue.code,
        WorkflowGovernanceIssueCode::LegacyProjectionMismatch
    );
}
