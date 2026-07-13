use forge_core_contracts::{
    CatalogEntry, NextActionKind, ObligationStatus, PrincipalId, ReadinessTarget, RepoPath,
    StableId, WorkflowClaimWaiverObservation, WorkflowDecisionActivation,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluationDocument,
    WorkflowPrerequisiteRequirement,
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
fn verified_claim_can_trigger_an_irreducible_follow_up_without_caller_need_injection() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    let rule = policy.decision_rules.first_mut().expect("decision rule");
    rule.activation = WorkflowDecisionActivation::ClaimVerified;
    rule.claim_ref = Some(StableId("claim.representative-execution".to_owned()));

    let mut complete = evaluation("complete");
    complete
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    complete
        .workflow_governance_evaluation
        .resolved_decision_refs
        .clear();
    let verified = simulate_workflow_governance(&governed_bundle, &complete)
        .expect("verified claim activates the decision");
    assert_eq!(verified.candidate_decision_requests.len(), 1);
    assert_eq!(
        verified.candidate_decision_requests[0].id.0,
        "decision.product-direction"
    );
    assert!(verified
        .candidate_next_actions
        .iter()
        .any(|action| action.kind == NextActionKind::AskHuman));

    let mut missing = evaluation("missing-evidence");
    missing
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    missing
        .workflow_governance_evaluation
        .resolved_decision_refs
        .clear();
    let unresolved = simulate_workflow_governance(&governed_bundle, &missing)
        .expect("unresolved claim remains agent work");
    assert!(unresolved.candidate_decision_requests.is_empty());
    assert!(unresolved
        .candidate_next_actions
        .iter()
        .all(|action| action.kind != NextActionKind::AskHuman));
}

#[test]
fn all_claims_verified_waits_for_complete_agent_evidence_before_human_contact() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    let rule = policy.decision_rules.first_mut().expect("decision rule");
    rule.activation = WorkflowDecisionActivation::AllClaimsVerified;
    rule.claim_ref = None;

    let mut complete = evaluation("complete");
    complete
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    complete
        .workflow_governance_evaluation
        .resolved_decision_refs
        .clear();
    let ready_for_human = simulate_workflow_governance(&governed_bundle, &complete)
        .expect("all verified claims activate the decision");
    assert_eq!(ready_for_human.candidate_decision_requests.len(), 1);
    assert!(ready_for_human
        .candidate_next_actions
        .iter()
        .any(|action| action.kind == NextActionKind::AskHuman));

    let mut partial = evaluation("active");
    partial
        .workflow_governance_evaluation
        .decision_need_refs
        .clear();
    partial
        .workflow_governance_evaluation
        .resolved_decision_refs
        .clear();
    let agent_work = simulate_workflow_governance(&governed_bundle, &partial)
        .expect("partially supported claims remain agent work");
    assert!(agent_work.candidate_decision_requests.is_empty());
    assert!(agent_work
        .candidate_next_actions
        .iter()
        .all(|action| action.kind != NextActionKind::AskHuman));
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
fn obligations_block_completion_only_when_due_for_the_selected_target() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    policy
        .obligations
        .iter_mut()
        .find(|obligation| obligation.id.0 == "obligation.representative-execution")
        .expect("runtime obligation")
        .required_before = ReadinessTarget::Release;

    let mut input = evaluation("complete");
    input
        .workflow_governance_evaluation
        .evidence
        .retain(|evidence| evidence.claim_ref.0 != "claim.representative-execution");
    let simulation = simulate_workflow_governance(&governed_bundle, &input)
        .expect("valid target-aware simulation");

    assert_eq!(
        simulation.candidate_completion,
        WorkflowCompletionVerdict::Complete
    );
    let future_obligation = simulation
        .candidate_obligation_results
        .iter()
        .find(|obligation| obligation.obligation_id == "obligation.representative-execution")
        .expect("future obligation result");
    assert_eq!(future_obligation.required_before, ReadinessTarget::Release);
    assert_eq!(future_obligation.status, ObligationStatus::Pending);
}

#[test]
fn conditional_prerequisites_require_complete_or_not_applicable_receipts() {
    let mut governed_bundle = bundle();
    let policy = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    policy.prerequisites[0].requirement = WorkflowPrerequisiteRequirement::WhenApplicable;

    let mut unknown_input = evaluation("complete");
    unknown_input
        .workflow_governance_evaluation
        .completed_policy_refs
        .clear();
    let unknown = simulate_workflow_governance(&governed_bundle, &unknown_input)
        .expect("unknown applicability is a runtime blocker");
    assert_eq!(
        unknown.candidate_eligibility,
        WorkflowEligibilityVerdict::Ineligible
    );
    assert!(unknown
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::UnknownApplicability));

    unknown_input
        .workflow_governance_evaluation
        .not_applicable_policy_refs
        .push(StableId("policy.workflow.write-spec".to_owned()));
    let not_applicable = simulate_workflow_governance(&governed_bundle, &unknown_input)
        .expect("not-applicable receipt satisfies a conditional prerequisite");
    assert_eq!(
        not_applicable.candidate_eligibility,
        WorkflowEligibilityVerdict::Eligible
    );
}

#[test]
fn authorized_waivers_are_target_scope_and_time_bounded() {
    let governed_bundle = bundle();
    let mut input = evaluation("complete");
    let observations = &mut input.workflow_governance_evaluation;
    observations
        .evidence
        .retain(|evidence| evidence.claim_ref.0 != "claim.acceptance-defined");
    observations.waivers.push(WorkflowClaimWaiverObservation {
        claim_ref: StableId("claim.acceptance-defined".to_owned()),
        principal: PrincipalId("principal.product-owner".to_owned()),
        authority_scope: StableId("project.delivery".to_owned()),
        max_target: ReadinessTarget::Execute,
        authorization_intent_digest: "sha256:authorized-waiver".to_owned(),
        authorized_at_unix: observations.observed_at_unix - 10,
        expires_at_unix: observations.observed_at_unix + 10,
    });

    let waived =
        simulate_workflow_governance(&governed_bundle, &input).expect("valid waiver simulation");
    assert_eq!(
        claim(&waived, "claim.acceptance-defined").status,
        WorkflowClaimResultStatus::Waived
    );
    assert_eq!(
        waived.candidate_completion,
        WorkflowCompletionVerdict::Complete
    );

    let mut wrong_scope_input = input.clone();
    wrong_scope_input.workflow_governance_evaluation.waivers[0].authority_scope =
        StableId("project.unrelated".to_owned());
    let wrong_scope = simulate_workflow_governance(&governed_bundle, &wrong_scope_input)
        .expect("wrong-scope waiver is a runtime result");
    assert_eq!(
        claim(&wrong_scope, "claim.acceptance-defined").status,
        WorkflowClaimResultStatus::Unknown
    );
    assert!(wrong_scope
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::InvalidWaiver));

    let mut expired_input = input;
    expired_input
        .workflow_governance_evaluation
        .observed_at_unix += 30;
    let expired = simulate_workflow_governance(&governed_bundle, &expired_input)
        .expect("expired waiver is a runtime result");
    assert_eq!(
        claim(&expired, "claim.acceptance-defined").status,
        WorkflowClaimResultStatus::Unknown
    );
    assert!(expired
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowGovernanceIssueCode::ExpiredWaiver));
}

#[test]
fn evaluator_principal_diversity_is_enforced_independently_of_evidence_count() {
    let mut governed_bundle = bundle();
    let evaluator = governed_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy")
        .evaluators
        .iter_mut()
        .find(|evaluator| evaluator.id.0 == "evaluator.runtime-verification")
        .expect("runtime evaluator");
    evaluator.minimum_distinct_principals = 2;

    let mut same_principal_input = evaluation("complete");
    for observation in &mut same_principal_input.workflow_governance_evaluation.evidence {
        if observation.claim_ref.0 == "claim.representative-execution" {
            observation.principal = Some(PrincipalId("principal.runtime-a".to_owned()));
        }
    }
    let same_principal = simulate_workflow_governance(&governed_bundle, &same_principal_input)
        .expect("insufficient diversity is a runtime result");
    assert_eq!(
        claim(&same_principal, "claim.representative-execution").status,
        WorkflowClaimResultStatus::Supported
    );
    assert!(same_principal.issues.iter().any(|issue| {
        issue.code == WorkflowGovernanceIssueCode::InsufficientPrincipalDiversity
    }));

    let mut runtime_observations = same_principal_input
        .workflow_governance_evaluation
        .evidence
        .iter_mut()
        .filter(|observation| observation.claim_ref.0 == "claim.representative-execution")
        .collect::<Vec<_>>();
    runtime_observations[1].principal = Some(PrincipalId("principal.runtime-b".to_owned()));
    let diverse = simulate_workflow_governance(&governed_bundle, &same_principal_input)
        .expect("distinct principals satisfy evaluator diversity");
    assert_eq!(
        claim(&diverse, "claim.representative-execution").status,
        WorkflowClaimResultStatus::Verified
    );
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
fn content_identifiers_must_be_globally_unique_across_policies() {
    let mut document: WorkflowGovernanceBundleDocument =
        load_yaml("contracts/workflow-governance/golden-path-v0.yaml");
    let duplicate = document.workflow_governance_bundle.policies[0].claims[0]
        .id
        .clone();
    document.workflow_governance_bundle.policies[1].claims[0].id = duplicate;
    let issues = validate_workflow_governance_bundle(&document);
    assert!(issues.iter().any(|issue| {
        issue.code == WorkflowGovernanceIssueCode::DuplicateIdentifier
            && issue.path.contains(".claims.")
    }));
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
        policy.prerequisites.reverse();
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
    observations.not_applicable_policy_refs.reverse();
    observations.available_capability_refs.reverse();
    observations.decision_need_refs.reverse();
    observations.resolved_decision_refs.reverse();
    observations.waivers.reverse();
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
