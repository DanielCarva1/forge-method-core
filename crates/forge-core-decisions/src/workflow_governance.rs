//! P5b workflow-governance simulation engine.
//!
//! This module is pure and deterministic. It evaluates caller-provided
//! hypothetical observations for guidance, but its serializable output is
//! explicitly `simulation_only` and is never executable authority. The
//! mutation kernel owns the opaque verified lane.
//!
//! The engine evaluates phase/dependency
//! eligibility, capability and human-decision blockers, evidence-backed
//! claims, obligations, completion, and ranked next actions. Advisory
//! playbooks are copied to the result but are never read by authority logic.

use std::collections::{BTreeMap, BTreeSet};

use forge_core_contracts::{
    AdvisoryWorkflowPlaybook, CapabilityGap, CatalogEntry, DecisionRequest, NextAction,
    NextActionKind, ObligationCriticality, ObligationStatus, Phase, ReadinessTarget, StableId,
    UniversalAssuranceLens, WorkflowAssuranceClaimRole, WorkflowClaimWaiverObservation,
    WorkflowClaimWaiverPolicy, WorkflowCompletionAssertion, WorkflowDecisionActivation,
    WorkflowDisproofPolicy, WorkflowEvaluatorBinding, WorkflowEvaluatorProvider,
    WorkflowEvidenceFreshness, WorkflowEvidenceKind, WorkflowEvidenceObservation,
    WorkflowEvidenceOutcome, WorkflowFreshnessRequirement, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernancePolicy, WorkflowPolicyActivation,
    WorkflowPrerequisiteRequirement, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceSimulation {
    pub schema_version: String,
    pub authority: WorkflowGovernanceSimulationAuthority,
    pub bundle_id: String,
    pub policy_id: String,
    pub workflow_id: String,
    pub observation_set_id: String,
    pub state_version: u64,
    pub current_phase: String,
    pub target: ReadinessTarget,
    pub candidate_status: WorkflowGovernanceStatus,
    pub candidate_eligibility: WorkflowEligibilityVerdict,
    pub candidate_progression: WorkflowProgressionVerdict,
    pub candidate_completion: WorkflowCompletionVerdict,
    pub candidate_obligation_results: Vec<WorkflowObligationResult>,
    pub candidate_claim_results: Vec<WorkflowClaimResult>,
    pub candidate_decision_requests: Vec<DecisionRequest>,
    pub candidate_capability_gaps: Vec<CapabilityGap>,
    pub candidate_next_actions: Vec<NextAction>,
    pub advisory_playbook: AdvisoryWorkflowPlaybook,
    pub issues: Vec<WorkflowGovernanceIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceSimulationAuthority {
    SimulationOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceStatus {
    Ineligible,
    Blocked,
    Active,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEligibilityVerdict {
    Eligible,
    Ineligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowProgressionVerdict {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompletionVerdict {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationResult {
    pub obligation_id: String,
    pub description: String,
    pub criticality: ObligationCriticality,
    pub required_before: ReadinessTarget,
    pub status: ObligationStatus,
    pub claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimResult {
    pub claim_id: String,
    pub statement: String,
    pub status: WorkflowClaimResultStatus,
    pub accepted_evidence_refs: Vec<String>,
    pub rejected_evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowClaimResultStatus {
    Unknown,
    Supported,
    Verified,
    Waived,
    Disproven,
    Contradictory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceIssue {
    pub code: WorkflowGovernanceIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceIssueCode {
    UnsupportedSchemaVersion,
    BlankRequiredField,
    DuplicateIdentifier,
    DuplicateReference,
    DanglingReference,
    DependencyCycle,
    InvalidEvaluator,
    InvalidDecisionRule,
    InvalidPolicy,
    BundleMismatch,
    UnknownPolicy,
    InvalidPhase,
    EvidenceBindingMismatch,
    UnsupportedEvidenceKind,
    InsufficientEvidenceStrength,
    StaleEvidence,
    InconclusiveEvidence,
    ContradictoryEvidence,
    PhaseIneligible,
    MissingPrerequisite,
    UnknownApplicability,
    InvalidWaiver,
    ExpiredWaiver,
    InsufficientPrincipalDiversity,
    InventedCompletionClaim,
    LegacyProjectionMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceRejection {
    pub issues: Vec<WorkflowGovernanceIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyWorkflowProjectionAuthority {
    SimulationCompatibilityOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyWorkflowGovernanceProjection {
    pub catalog_entry: CatalogEntry,
    pub candidate_governance_status: WorkflowGovernanceStatus,
    pub authority: LegacyWorkflowProjectionAuthority,
    pub advisory_steps: Vec<String>,
    pub blocker_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyWorkflowProjectionError {
    pub issue: WorkflowGovernanceIssue,
}

/// Validate a complete policy bundle, including its dependency graph.
#[must_use]
pub fn validate_workflow_governance_bundle(
    document: &WorkflowGovernanceBundleDocument,
) -> Vec<WorkflowGovernanceIssue> {
    let mut issues = Vec::new();
    if document.schema_version != WORKFLOW_GOVERNANCE_SCHEMA_VERSION {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!("unsupported schema version {}", document.schema_version),
        );
    }
    let bundle = &document.workflow_governance_bundle;
    require_nonblank(&mut issues, "workflow_governance_bundle.id", &bundle.id.0);
    if bundle.policies.is_empty() {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            "workflow_governance_bundle.policies",
            "bundle must contain at least one policy",
        );
        return sorted_issues(issues);
    }

    let mut policy_ids = BTreeSet::new();
    let mut workflow_ids = BTreeSet::new();
    let mut routing_priorities = BTreeSet::new();
    for policy in &bundle.policies {
        let path = format!("workflow_governance_bundle.policies.{}", policy.id.0);
        require_nonblank(&mut issues, format!("{path}.id"), &policy.id.0);
        require_nonblank(
            &mut issues,
            format!("{path}.compatibility_workflow_id"),
            &policy.compatibility_workflow_id.0,
        );
        insert_unique(
            &mut issues,
            &mut policy_ids,
            &policy.id.0,
            format!("{path}.id"),
        );
        insert_unique(
            &mut issues,
            &mut workflow_ids,
            &policy.compatibility_workflow_id.0,
            format!("{path}.compatibility_workflow_id"),
        );
        if !routing_priorities.insert(policy.routing.priority) {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::DuplicateIdentifier,
                format!("{path}.routing.priority"),
                format!(
                    "routing priority {} occurs more than once",
                    policy.routing.priority
                ),
            );
        }
    }

    for policy in &bundle.policies {
        validate_policy(policy, &policy_ids, &mut issues);
    }
    let mut global_content_ids = BTreeSet::new();
    for policy in &bundle.policies {
        let policy_path = format!("workflow_governance_bundle.policies.{}", policy.id.0);
        for obligation in &policy.obligations {
            insert_unique(
                &mut issues,
                &mut global_content_ids,
                &obligation.id.0,
                format!("{policy_path}.obligations.{}.id", obligation.id.0),
            );
        }
        for claim in &policy.claims {
            insert_unique(
                &mut issues,
                &mut global_content_ids,
                &claim.id.0,
                format!("{policy_path}.claims.{}.id", claim.id.0),
            );
        }
        for evaluator in &policy.evaluators {
            insert_unique(
                &mut issues,
                &mut global_content_ids,
                &evaluator.id.0,
                format!("{policy_path}.evaluators.{}.id", evaluator.id.0),
            );
        }
        for capability in &policy.capability_requirements {
            insert_unique(
                &mut issues,
                &mut global_content_ids,
                &capability.id.0,
                format!(
                    "{policy_path}.capability_requirements.{}.id",
                    capability.id.0
                ),
            );
        }
        for decision in &policy.decision_rules {
            insert_unique(
                &mut issues,
                &mut global_content_ids,
                &decision.id.0,
                format!("{policy_path}.decision_rules.{}.id", decision.id.0),
            );
        }
    }
    validate_dependency_graph(&bundle.policies, &policy_ids, &mut issues);
    validate_lens_aware_bundle(bundle, &mut issues);
    sorted_issues(issues)
}

fn validate_lens_aware_bundle(
    bundle: &forge_core_contracts::WorkflowGovernanceBundle,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) {
    let lens_aware = bundle
        .policies
        .iter()
        .flat_map(|policy| &policy.claims)
        .any(|claim| !claim.assurance_lenses.is_empty() || claim.assurance_role.is_some());
    if !lens_aware {
        return;
    }

    let mut covered = BTreeSet::new();
    let mut verification_capable = BTreeSet::new();
    let mut definition_count = 0_usize;
    let mut execution_count = 0_usize;
    for policy in &bundle.policies {
        for claim in &policy.claims {
            let path = format!(
                "workflow_governance_bundle.policies.{}.claims.{}",
                policy.id.0, claim.id.0
            );
            let unique = claim
                .assurance_lenses
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if unique.len() != claim.assurance_lenses.len() {
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::DuplicateReference,
                    format!("{path}.assurance_lenses"),
                    "an assurance lens may occur only once on a claim",
                );
            }
            if claim.assurance_lenses.is_empty()
                && matches!(
                    claim.assurance_role,
                    Some(
                        WorkflowAssuranceClaimRole::LensEvidence
                            | WorkflowAssuranceClaimRole::RepresentativeSliceExecution
                    )
                )
            {
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::InvalidPolicy,
                    format!("{path}.assurance_role"),
                    "lens evidence and representative execution require mapped universal lenses",
                );
            }
            if !claim.assurance_lenses.is_empty() && claim.assurance_role.is_none() {
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::InvalidPolicy,
                    format!("{path}.assurance_role"),
                    "a lens-aware claim requires an explicit closed assurance role",
                );
            }
            covered.extend(unique.iter().copied());
            let referenced_by_required_obligation = policy.obligations.iter().any(|obligation| {
                obligation.criticality != ObligationCriticality::Advisory
                    && obligation.claim_refs.contains(&claim.id)
            });
            if (!claim.assurance_lenses.is_empty() || claim.assurance_role.is_some())
                && !referenced_by_required_obligation
            {
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::InvalidPolicy,
                    format!("{path}.assurance_lenses"),
                    "an assurance claim must be referenced by a non-advisory obligation",
                );
            }
            let evaluator = policy
                .evaluators
                .iter()
                .find(|candidate| candidate.id == claim.evaluator_ref);
            if evaluator.is_some_and(evaluator_can_verify_assurance) {
                verification_capable.extend(unique.iter().copied());
            }
            match claim.assurance_role {
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition) => {
                    definition_count = definition_count.saturating_add(1);
                    let valid = claim.assurance_lenses.is_empty()
                        && evaluator.is_some_and(|evaluator| {
                            evaluator.provider == WorkflowEvaluatorProvider::IndependentReviewer
                                && evaluator
                                    .accepted_evidence_kinds
                                    .contains(&WorkflowEvidenceKind::IndependentReview)
                                && evaluator.minimum_strength
                                    >= forge_core_contracts::WorkflowEvidenceStrength::IndependentConfirmation
                                && evaluator.minimum_passing_observations == 1
                                && evaluator.minimum_distinct_principals >= 1
                        });
                    if !valid {
                        issue(
                            issues,
                            WorkflowGovernanceIssueCode::InvalidEvaluator,
                            format!("{path}.assurance_role"),
                            "representative-slice definition must be unmapped and require exactly one independent-review approval",
                        );
                    }
                }
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution) => {
                    execution_count = execution_count.saturating_add(1);
                    let valid = claim
                        .assurance_lenses
                        .contains(&UniversalAssuranceLens::CriticalJourneys)
                        && claim
                            .assurance_lenses
                            .contains(&UniversalAssuranceLens::EvidenceRepresentativeness)
                        && evaluator.is_some_and(|evaluator| {
                            evaluator.provider == WorkflowEvaluatorProvider::RepresentativeRuntime
                                && evaluator
                                    .accepted_evidence_kinds
                                    .contains(&WorkflowEvidenceKind::RepresentativeExecution)
                                && evaluator.minimum_strength
                                    >= forge_core_contracts::WorkflowEvidenceStrength::RepresentativeExecution
                        });
                    if !valid {
                        issue(
                            issues,
                            WorkflowGovernanceIssueCode::InvalidEvaluator,
                            format!("{path}.assurance_role"),
                            "representative-slice execution requires representative runtime evidence for critical journeys and representativeness",
                        );
                    }
                }
                Some(WorkflowAssuranceClaimRole::LensEvidence) | None => {}
            }
        }
    }

    let expected = UniversalAssuranceLens::ALL
        .into_iter()
        .collect::<BTreeSet<_>>();
    if covered != expected {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            "workflow_governance_bundle.policies",
            "a lens-aware bundle must cover exactly all eight universal assurance lenses",
        );
    }
    for lens in UniversalAssuranceLens::ALL {
        if !verification_capable.contains(&lens) {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidEvaluator,
                "workflow_governance_bundle.policies",
                format!(
                    "lens {} requires at least one non-research verification-capable claim",
                    lens.id()
                ),
            );
        }
    }
    if definition_count != 1 || execution_count != 1 {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            "workflow_governance_bundle.policies",
            "a lens-aware bundle requires exactly one representative-slice definition and one execution claim",
        );
    }
}

fn evaluator_can_verify_assurance(evaluator: &WorkflowEvaluatorBinding) -> bool {
    match evaluator.provider {
        WorkflowEvaluatorProvider::ResearchSource => false,
        WorkflowEvaluatorProvider::AuthorizedHuman => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::HumanAcceptance)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::AuthoritativeAcceptance
        }
        WorkflowEvaluatorProvider::IndependentReviewer => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::IndependentReview)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::IndependentConfirmation
                && evaluator.minimum_distinct_principals >= 1
        }
        WorkflowEvaluatorProvider::RepositoryInspector => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::ArtifactInspection)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::InspectedArtifact
        }
        WorkflowEvaluatorProvider::DeterministicTool => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::DeterministicCheck)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::DeterministicVerification
        }
        WorkflowEvaluatorProvider::RepresentativeRuntime => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::RepresentativeExecution)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::RepresentativeExecution
        }
        WorkflowEvaluatorProvider::ExternalAuthority => {
            evaluator
                .accepted_evidence_kinds
                .contains(&WorkflowEvidenceKind::ExternalAuthority)
                && evaluator.minimum_strength
                    >= forge_core_contracts::WorkflowEvidenceStrength::AuthoritativeAcceptance
        }
    }
}

/// Simulate one selected policy from a valid bundle without IO or mutation.
///
/// # Errors
/// Returns every structural policy/input issue before producing a simulation.
pub fn simulate_workflow_governance(
    bundle_document: &WorkflowGovernanceBundleDocument,
    evaluation_document: &WorkflowGovernanceEvaluationDocument,
) -> Result<WorkflowGovernanceSimulation, WorkflowGovernanceRejection> {
    let mut structural_issues = validate_workflow_governance_bundle(bundle_document);
    structural_issues.extend(validate_evaluation_input(
        bundle_document,
        evaluation_document,
    ));
    structural_issues = sorted_issues(structural_issues);
    if !structural_issues.is_empty() {
        return Err(WorkflowGovernanceRejection {
            issues: structural_issues,
        });
    }

    let bundle = &bundle_document.workflow_governance_bundle;
    let input = &evaluation_document.workflow_governance_evaluation;
    let Some(policy) = bundle
        .policies
        .iter()
        .find(|policy| policy.id == input.policy_id)
    else {
        return Err(WorkflowGovernanceRejection {
            issues: vec![WorkflowGovernanceIssue {
                code: WorkflowGovernanceIssueCode::UnknownPolicy,
                path: "workflow_governance_evaluation.policy_id".to_owned(),
                message: format!("unknown policy {}", input.policy_id.0),
            }],
        });
    };

    let mut runtime_issues = Vec::new();
    let claim_results = evaluate_claims(
        policy,
        &input.evidence,
        &input.waivers,
        input.target,
        input.observed_at_unix,
        &mut runtime_issues,
    );
    let claim_statuses = claim_results
        .iter()
        .map(|result| (result.claim_id.as_str(), result.status))
        .collect::<BTreeMap<_, _>>();
    let obligation_results = evaluate_obligations(policy, &claim_statuses);
    let capability_gaps = capability_gaps(policy, input);
    let decision_requests = decision_requests(policy, input, &claim_statuses);
    let eligibility = evaluate_eligibility(policy, input, &mut runtime_issues);
    let blocking_capability_gap = capability_gaps.iter().any(|gap| gap.blocking);
    let blocking_human_decision = decision_requests.iter().any(|request| request.blocking);
    let progression = if eligibility == WorkflowEligibilityVerdict::Eligible
        && !blocking_capability_gap
        && !blocking_human_decision
    {
        WorkflowProgressionVerdict::Allowed
    } else {
        WorkflowProgressionVerdict::Blocked
    };
    let required_obligations_complete = obligation_results.iter().all(|obligation| {
        obligation.criticality == ObligationCriticality::Advisory
            || obligation.required_before.rank() > input.target.rank()
            || obligation.status == ObligationStatus::Satisfied
    });
    let completion =
        if required_obligations_complete && progression == WorkflowProgressionVerdict::Allowed {
            WorkflowCompletionVerdict::Complete
        } else {
            WorkflowCompletionVerdict::Incomplete
        };
    if input.completion_assertion == WorkflowCompletionAssertion::Asserted
        && completion == WorkflowCompletionVerdict::Incomplete
    {
        issue(
            &mut runtime_issues,
            WorkflowGovernanceIssueCode::InventedCompletionClaim,
            "workflow_governance_evaluation.completion_assertion",
            "completion was asserted without satisfied governed obligations and evidence",
        );
    }
    runtime_issues = sorted_issues(runtime_issues);

    let status = match (eligibility, progression, completion) {
        (WorkflowEligibilityVerdict::Ineligible, _, _) => WorkflowGovernanceStatus::Ineligible,
        (_, WorkflowProgressionVerdict::Blocked, _) => WorkflowGovernanceStatus::Blocked,
        (_, _, WorkflowCompletionVerdict::Complete) => WorkflowGovernanceStatus::Complete,
        _ => WorkflowGovernanceStatus::Active,
    };
    let next_actions = next_actions(
        policy,
        status,
        &claim_results,
        &capability_gaps,
        &decision_requests,
        &runtime_issues,
    );

    Ok(WorkflowGovernanceSimulation {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        authority: WorkflowGovernanceSimulationAuthority::SimulationOnly,
        bundle_id: bundle.id.0.clone(),
        policy_id: policy.id.0.clone(),
        workflow_id: policy.compatibility_workflow_id.0.clone(),
        observation_set_id: input.observation_set_id.0.clone(),
        state_version: input.state_version,
        current_phase: input.current_phase.0.clone(),
        target: input.target,
        candidate_status: status,
        candidate_eligibility: eligibility,
        candidate_progression: progression,
        candidate_completion: completion,
        candidate_obligation_results: obligation_results,
        candidate_claim_results: claim_results,
        candidate_decision_requests: decision_requests,
        candidate_capability_gaps: capability_gaps,
        candidate_next_actions: next_actions,
        advisory_playbook: policy.advisory_playbook.clone(),
        issues: runtime_issues,
    })
}

/// Project a simulation onto the legacy routing entry.
///
/// The projection remains explicitly simulation-only; legacy text and
/// playbook steps can never authorize progression, completion, or mutation.
///
/// # Errors
/// Rejects a catalog entry for any other compatibility workflow id.
pub fn project_legacy_workflow_compatibility(
    simulation: &WorkflowGovernanceSimulation,
    legacy_entry: &CatalogEntry,
) -> Result<LegacyWorkflowGovernanceProjection, LegacyWorkflowProjectionError> {
    if simulation.workflow_id != legacy_entry.id.0 {
        return Err(LegacyWorkflowProjectionError {
            issue: WorkflowGovernanceIssue {
                code: WorkflowGovernanceIssueCode::LegacyProjectionMismatch,
                path: "legacy_catalog_entry.id".to_owned(),
                message: format!(
                    "governed workflow {} cannot project legacy workflow {}",
                    simulation.workflow_id, legacy_entry.id.0
                ),
            },
        });
    }
    let mut blocker_refs = simulation
        .candidate_capability_gaps
        .iter()
        .filter(|gap| gap.blocking)
        .map(|gap| gap.id.0.clone())
        .chain(
            simulation
                .candidate_decision_requests
                .iter()
                .filter(|request| request.blocking)
                .map(|request| request.id.0.clone()),
        )
        .chain(simulation.issues.iter().map(|issue| issue.path.clone()))
        .collect::<Vec<_>>();
    blocker_refs.sort();
    blocker_refs.dedup();
    Ok(LegacyWorkflowGovernanceProjection {
        catalog_entry: legacy_entry.clone(),
        candidate_governance_status: simulation.candidate_status,
        authority: LegacyWorkflowProjectionAuthority::SimulationCompatibilityOnly,
        advisory_steps: simulation.advisory_playbook.steps.clone(),
        blocker_refs,
    })
}

fn validate_policy(
    policy: &WorkflowGovernancePolicy,
    known_policy_ids: &BTreeSet<&str>,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) {
    let path = format!("workflow_governance_bundle.policies.{}", policy.id.0);
    match policy.routing.activation {
        WorkflowPolicyActivation::Required | WorkflowPolicyActivation::WhenApplicable
            if !policy.routing.signals.is_empty() =>
        {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidPolicy,
                format!("{path}.routing.signals"),
                "required and when-applicable policies cannot declare activation signals",
            );
        }
        WorkflowPolicyActivation::OnSignal if policy.routing.signals.is_empty() => {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidPolicy,
                format!("{path}.routing.signals"),
                "on-signal policies must declare at least one activation signal",
            );
        }
        _ => {}
    }
    let unique_signals = policy
        .routing
        .signals
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if unique_signals.len() != policy.routing.signals.len() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::DuplicateReference,
            format!("{path}.routing.signals"),
            "activation signal occurs more than once",
        );
    }
    if policy.eligible_phases.is_empty() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            format!("{path}.eligible_phases"),
            "policy must declare at least one eligible phase",
        );
    }
    validate_unique_refs(
        issues,
        &policy.eligible_phases,
        format!("{path}.eligible_phases"),
    );
    for (index, phase) in policy.eligible_phases.iter().enumerate() {
        if phase.0 != "anytime" && Phase::parse(&phase.0).is_none() {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidPhase,
                format!("{path}.eligible_phases[{index}]"),
                format!("unrecognized phase {}", phase.0),
            );
        }
    }
    let mut prerequisite_refs = BTreeSet::new();
    for dependency in &policy.prerequisites {
        if !prerequisite_refs.insert(dependency.policy_ref.0.as_str()) {
            issue(
                issues,
                WorkflowGovernanceIssueCode::DuplicateReference,
                format!("{path}.prerequisites.{}", dependency.policy_ref.0),
                "prerequisite policy occurs more than once",
            );
        }
        if !known_policy_ids.contains(dependency.policy_ref.0.as_str()) {
            issue(
                issues,
                WorkflowGovernanceIssueCode::DanglingReference,
                format!("{path}.prerequisites.{}", dependency.policy_ref.0),
                "prerequisite policy does not exist in bundle",
            );
        }
    }

    if policy.obligations.is_empty() || policy.claims.is_empty() || policy.evaluators.is_empty() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            &path,
            "policy requires obligations, claims, and evaluators",
        );
    }
    if !policy.obligations.iter().any(|obligation| {
        matches!(
            obligation.criticality,
            ObligationCriticality::Required | ObligationCriticality::Critical
        )
    }) {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidPolicy,
            format!("{path}.obligations"),
            "policy requires at least one non-advisory obligation",
        );
    }

    let mut local_ids = BTreeSet::new();
    for obligation in &policy.obligations {
        validate_local_id(
            issues,
            &mut local_ids,
            &obligation.id,
            format!("{path}.obligations"),
        );
        require_nonblank(
            issues,
            format!("{path}.obligations.{}.description", obligation.id.0),
            &obligation.description,
        );
        if obligation.claim_refs.is_empty() {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidPolicy,
                format!("{path}.obligations.{}.claim_refs", obligation.id.0),
                "obligation must reference at least one claim",
            );
        }
        validate_unique_refs(
            issues,
            &obligation.claim_refs,
            format!("{path}.obligations.{}.claim_refs", obligation.id.0),
        );
    }
    for claim in &policy.claims {
        validate_local_id(issues, &mut local_ids, &claim.id, format!("{path}.claims"));
        require_nonblank(
            issues,
            format!("{path}.claims.{}.statement", claim.id.0),
            &claim.statement,
        );
        if let WorkflowClaimWaiverPolicy::Authorized {
            authority_scope,
            max_age_seconds,
            ..
        } = &claim.waiver
        {
            require_nonblank(
                issues,
                format!("{path}.claims.{}.waiver.authority_scope", claim.id.0),
                &authority_scope.0,
            );
            if *max_age_seconds == 0 {
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::InvalidPolicy,
                    format!("{path}.claims.{}.waiver.max_age_seconds", claim.id.0),
                    "authorized waiver must have a positive maximum age",
                );
            }
        }
    }
    for evaluator in &policy.evaluators {
        validate_local_id(
            issues,
            &mut local_ids,
            &evaluator.id,
            format!("{path}.evaluators"),
        );
        let kinds = evaluator
            .accepted_evidence_kinds
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if evaluator.minimum_passing_observations == 0
            || kinds.is_empty()
            || kinds.len() != evaluator.accepted_evidence_kinds.len()
            || evaluator.max_age_seconds == 0
            || evaluator.minimum_distinct_principals > evaluator.minimum_passing_observations
        {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidEvaluator,
                format!("{path}.evaluators.{}", evaluator.id.0),
                "evaluator requires unique accepted kinds, positive thresholds and maximum age, and achievable principal diversity",
            );
        }
    }
    for requirement in &policy.capability_requirements {
        validate_local_id(
            issues,
            &mut local_ids,
            &requirement.id,
            format!("{path}.capability_requirements"),
        );
        require_nonblank(
            issues,
            format!(
                "{path}.capability_requirements.{}.description",
                requirement.id.0
            ),
            &requirement.description,
        );
        if requirement.affected_claim_refs.is_empty() || requirement.resolution_options.is_empty() {
            issue(
                issues,
                WorkflowGovernanceIssueCode::InvalidPolicy,
                format!("{path}.capability_requirements.{}", requirement.id.0),
                "capability requirement needs affected claims and resolution options",
            );
        }
        validate_unique_refs(
            issues,
            &requirement.affected_claim_refs,
            format!(
                "{path}.capability_requirements.{}.affected_claim_refs",
                requirement.id.0
            ),
        );
        validate_nonblank_unique_strings(
            issues,
            &requirement.resolution_options,
            format!(
                "{path}.capability_requirements.{}.resolution_options",
                requirement.id.0
            ),
        );
    }
    for decision in &policy.decision_rules {
        validate_local_id(
            issues,
            &mut local_ids,
            &decision.id,
            format!("{path}.decision_rules"),
        );
        validate_decision_rule(decision, &path, issues);
    }
    validate_local_id(
        issues,
        &mut local_ids,
        &policy.advisory_playbook.id,
        format!("{path}.advisory_playbook"),
    );
    for (index, step) in policy.advisory_playbook.steps.iter().enumerate() {
        require_nonblank(
            issues,
            format!("{path}.advisory_playbook.steps[{index}]"),
            step,
        );
    }

    let claim_ids = policy
        .claims
        .iter()
        .map(|claim| claim.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let evaluator_ids = policy
        .evaluators
        .iter()
        .map(|evaluator| evaluator.id.0.as_str())
        .collect::<BTreeSet<_>>();
    for obligation in &policy.obligations {
        for claim_ref in &obligation.claim_refs {
            require_known_ref(
                issues,
                &claim_ids,
                &claim_ref.0,
                format!("{path}.obligations.{}.claim_refs", obligation.id.0),
            );
        }
    }
    for claim in &policy.claims {
        require_known_ref(
            issues,
            &evaluator_ids,
            &claim.evaluator_ref.0,
            format!("{path}.claims.{}.evaluator_ref", claim.id.0),
        );
    }
    for requirement in &policy.capability_requirements {
        for claim_ref in &requirement.affected_claim_refs {
            require_known_ref(
                issues,
                &claim_ids,
                &claim_ref.0,
                format!(
                    "{path}.capability_requirements.{}.affected_claim_refs",
                    requirement.id.0
                ),
            );
        }
    }
    for decision in &policy.decision_rules {
        if let Some(claim_ref) = &decision.claim_ref {
            require_known_ref(
                issues,
                &claim_ids,
                &claim_ref.0,
                format!("{path}.decision_rules.{}.claim_ref", decision.id.0),
            );
        }
    }
}

fn validate_decision_rule(
    decision: &forge_core_contracts::WorkflowDecisionRule,
    policy_path: &str,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) {
    let path = format!("{policy_path}.decision_rules.{}", decision.id.0);
    let claim_shape_valid = match decision.activation {
        WorkflowDecisionActivation::ObservedNeed
        | WorkflowDecisionActivation::AllClaimsVerified => decision.claim_ref.is_none(),
        WorkflowDecisionActivation::ClaimVerified
        | WorkflowDecisionActivation::ClaimUnresolved
        | WorkflowDecisionActivation::ClaimDisproven => decision.claim_ref.is_some(),
    };
    let alternatives = decision
        .alternatives
        .iter()
        .map(|alternative| alternative.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let alternatives_valid = alternatives.len() == decision.alternatives.len()
        && alternatives.len() >= 2
        && !decision.recommended_alternative_ref.0.trim().is_empty()
        && alternatives.contains(decision.recommended_alternative_ref.0.as_str())
        && decision.alternatives.iter().all(|alternative| {
            !alternative.id.0.trim().is_empty()
                && !alternative.description.trim().is_empty()
                && !alternative.consequences.is_empty()
                && alternative
                    .consequences
                    .iter()
                    .all(|consequence| !consequence.trim().is_empty())
                && alternative
                    .consequences
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == alternative.consequences.len()
        });
    if !claim_shape_valid || !alternatives_valid || decision.question.trim().is_empty() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidDecisionRule,
            path,
            "decision activation/claim shape, question, alternatives, or recommendation is invalid",
        );
    }
}

fn validate_dependency_graph(
    policies: &[WorkflowGovernancePolicy],
    known_ids: &BTreeSet<&str>,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) {
    let mut remaining = policies
        .iter()
        .map(|policy| {
            let dependencies = policy
                .prerequisites
                .iter()
                .filter(|dependency| known_ids.contains(dependency.policy_ref.0.as_str()))
                .map(|dependency| dependency.policy_ref.0.as_str())
                .collect::<BTreeSet<_>>();
            (policy.id.0.as_str(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.is_empty())
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        if ready.is_empty() {
            break;
        }
        for id in &ready {
            remaining.remove(id);
        }
        for dependencies in remaining.values_mut() {
            for id in &ready {
                dependencies.remove(id);
            }
        }
    }
    if !remaining.is_empty() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::DependencyCycle,
            "workflow_governance_bundle.policies.prerequisites",
            format!(
                "cyclic policy dependencies involve {}",
                remaining.keys().copied().collect::<Vec<_>>().join(", ")
            ),
        );
    }
}

fn validate_evaluation_input(
    bundle_document: &WorkflowGovernanceBundleDocument,
    evaluation_document: &WorkflowGovernanceEvaluationDocument,
) -> Vec<WorkflowGovernanceIssue> {
    let mut issues = Vec::new();
    if evaluation_document.schema_version != WORKFLOW_GOVERNANCE_SCHEMA_VERSION {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!(
                "unsupported evaluation schema version {}",
                evaluation_document.schema_version
            ),
        );
    }
    let bundle = &bundle_document.workflow_governance_bundle;
    let input = &evaluation_document.workflow_governance_evaluation;
    require_nonblank(
        &mut issues,
        "workflow_governance_evaluation.observation_set_id",
        &input.observation_set_id.0,
    );
    if input.bundle_id != bundle.id {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::BundleMismatch,
            "workflow_governance_evaluation.bundle_id",
            format!("expected bundle id {}", bundle.id.0),
        );
    }
    let Some(policy) = bundle
        .policies
        .iter()
        .find(|policy| policy.id == input.policy_id)
    else {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::UnknownPolicy,
            "workflow_governance_evaluation.policy_id",
            format!("unknown policy {}", input.policy_id.0),
        );
        return sorted_issues(issues);
    };
    if Phase::parse(&input.current_phase.0).is_none() {
        issue(
            &mut issues,
            WorkflowGovernanceIssueCode::InvalidPhase,
            "workflow_governance_evaluation.current_phase",
            format!("unrecognized phase {}", input.current_phase.0),
        );
    }

    validate_unique_refs(
        &mut issues,
        &input.completed_policy_refs,
        "workflow_governance_evaluation.completed_policy_refs",
    );
    validate_unique_refs(
        &mut issues,
        &input.not_applicable_policy_refs,
        "workflow_governance_evaluation.not_applicable_policy_refs",
    );
    validate_unique_refs(
        &mut issues,
        &input.available_capability_refs,
        "workflow_governance_evaluation.available_capability_refs",
    );
    validate_unique_refs(
        &mut issues,
        &input.decision_need_refs,
        "workflow_governance_evaluation.decision_need_refs",
    );
    validate_unique_refs(
        &mut issues,
        &input.resolved_decision_refs,
        "workflow_governance_evaluation.resolved_decision_refs",
    );
    let known_policy_ids = bundle
        .policies
        .iter()
        .map(|item| item.id.0.as_str())
        .collect::<BTreeSet<_>>();
    for policy_ref in &input.completed_policy_refs {
        require_known_ref(
            &mut issues,
            &known_policy_ids,
            &policy_ref.0,
            "workflow_governance_evaluation.completed_policy_refs",
        );
    }
    let completed_policy_refs = input
        .completed_policy_refs
        .iter()
        .map(|policy_ref| policy_ref.0.as_str())
        .collect::<BTreeSet<_>>();
    for policy_ref in &input.not_applicable_policy_refs {
        require_known_ref(
            &mut issues,
            &known_policy_ids,
            &policy_ref.0,
            "workflow_governance_evaluation.not_applicable_policy_refs",
        );
        if completed_policy_refs.contains(policy_ref.0.as_str()) {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::InvalidPolicy,
                "workflow_governance_evaluation.not_applicable_policy_refs",
                format!(
                    "policy {} cannot be both complete and not applicable",
                    policy_ref.0
                ),
            );
        }
    }
    let known_capabilities = policy
        .capability_requirements
        .iter()
        .map(|requirement| requirement.id.0.as_str())
        .collect::<BTreeSet<_>>();
    for capability_ref in &input.available_capability_refs {
        require_known_ref(
            &mut issues,
            &known_capabilities,
            &capability_ref.0,
            "workflow_governance_evaluation.available_capability_refs",
        );
    }
    let known_decisions = policy
        .decision_rules
        .iter()
        .map(|decision| decision.id.0.as_str())
        .collect::<BTreeSet<_>>();
    for decision_ref in &input.decision_need_refs {
        require_known_ref(
            &mut issues,
            &known_decisions,
            &decision_ref.0,
            "workflow_governance_evaluation.decision_need_refs",
        );
    }
    for decision_ref in &input.resolved_decision_refs {
        require_known_ref(
            &mut issues,
            &known_decisions,
            &decision_ref.0,
            "workflow_governance_evaluation.resolved_decision_refs",
        );
    }

    let claims = policy
        .claims
        .iter()
        .map(|claim| (claim.id.0.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    let mut waiver_claim_refs = BTreeSet::new();
    for (index, waiver) in input.waivers.iter().enumerate() {
        let path = format!("workflow_governance_evaluation.waivers[{index}]");
        if !waiver_claim_refs.insert(waiver.claim_ref.0.as_str()) {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::DuplicateIdentifier,
                format!("{path}.claim_ref"),
                "claim waiver occurs more than once",
            );
        }
        if !claims.contains_key(waiver.claim_ref.0.as_str()) {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::DanglingReference,
                format!("{path}.claim_ref"),
                "waiver references an unknown claim",
            );
        }
        require_nonblank(
            &mut issues,
            format!("{path}.principal"),
            &waiver.principal.0,
        );
        require_nonblank(
            &mut issues,
            format!("{path}.authority_scope"),
            &waiver.authority_scope.0,
        );
        require_nonblank(
            &mut issues,
            format!("{path}.authorization_intent_digest"),
            &waiver.authorization_intent_digest,
        );
    }
    let evaluators = policy
        .evaluators
        .iter()
        .map(|evaluator| evaluator.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut evidence_refs = BTreeSet::new();
    for (index, observation) in input.evidence.iter().enumerate() {
        let path = format!("workflow_governance_evaluation.evidence[{index}]");
        require_nonblank(
            &mut issues,
            format!("{path}.evidence_ref"),
            &observation.evidence_ref,
        );
        if !evidence_refs.insert(observation.evidence_ref.as_str()) {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::DuplicateIdentifier,
                format!("{path}.evidence_ref"),
                "evidence ref occurs more than once",
            );
        }
        let Some(claim) = claims.get(observation.claim_ref.0.as_str()) else {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::DanglingReference,
                format!("{path}.claim_ref"),
                "evidence references an unknown claim",
            );
            continue;
        };
        if !evaluators.contains(observation.evaluator_ref.0.as_str())
            || claim.evaluator_ref != observation.evaluator_ref
        {
            issue(
                &mut issues,
                WorkflowGovernanceIssueCode::EvidenceBindingMismatch,
                format!("{path}.evaluator_ref"),
                format!(
                    "claim {} requires evaluator {}",
                    claim.id.0, claim.evaluator_ref.0
                ),
            );
        }
    }
    sorted_issues(issues)
}

fn evaluate_claims(
    policy: &WorkflowGovernancePolicy,
    evidence: &[WorkflowEvidenceObservation],
    waivers: &[WorkflowClaimWaiverObservation],
    target: ReadinessTarget,
    observed_at_unix: u64,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) -> Vec<WorkflowClaimResult> {
    let evaluators = policy
        .evaluators
        .iter()
        .map(|evaluator| (evaluator.id.0.as_str(), evaluator))
        .collect::<BTreeMap<_, _>>();
    let mut claims = policy.claims.iter().collect::<Vec<_>>();
    claims.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    claims
        .into_iter()
        .map(|claim| {
            let evaluator = evaluators
                .get(claim.evaluator_ref.0.as_str())
                .expect("bundle validation resolves evaluator");
            let waiver = waivers.iter().find(|waiver| waiver.claim_ref == claim.id);
            evaluate_claim(
                claim,
                evaluator,
                evidence,
                waiver,
                target,
                observed_at_unix,
                issues,
            )
        })
        .collect()
}

fn evaluate_claim(
    claim: &forge_core_contracts::WorkflowClaimPolicy,
    evaluator: &WorkflowEvaluatorBinding,
    evidence: &[WorkflowEvidenceObservation],
    waiver: Option<&WorkflowClaimWaiverObservation>,
    target: ReadinessTarget,
    observed_at_unix: u64,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) -> WorkflowClaimResult {
    let mut accepted = Vec::new();
    let mut accepted_principals = BTreeSet::new();
    let mut rejected = Vec::new();
    let mut disproofs = Vec::new();
    for observation in evidence
        .iter()
        .filter(|observation| observation.claim_ref == claim.id)
    {
        let path = format!(
            "workflow_governance_evaluation.evidence.{}",
            observation.evidence_ref
        );
        if !evaluator
            .accepted_evidence_kinds
            .contains(&observation.kind)
        {
            rejected.push(observation.evidence_ref.clone());
            issue(
                issues,
                WorkflowGovernanceIssueCode::UnsupportedEvidenceKind,
                &path,
                "evidence kind is not accepted by the bound evaluator",
            );
            continue;
        }
        if evaluator.freshness == WorkflowFreshnessRequirement::CurrentOnly
            && observation.freshness == WorkflowEvidenceFreshness::Stale
        {
            rejected.push(observation.evidence_ref.clone());
            issue(
                issues,
                WorkflowGovernanceIssueCode::StaleEvidence,
                &path,
                "stale evidence cannot satisfy a current-only evaluator",
            );
            continue;
        }
        match observation.outcome {
            WorkflowEvidenceOutcome::Fail => {
                disproofs.push(observation.evidence_ref.clone());
            }
            WorkflowEvidenceOutcome::Inconclusive => {
                rejected.push(observation.evidence_ref.clone());
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::InconclusiveEvidence,
                    &path,
                    "inconclusive evidence cannot satisfy the claim",
                );
            }
            WorkflowEvidenceOutcome::Pass => {
                if observation.strength < evaluator.minimum_strength {
                    rejected.push(observation.evidence_ref.clone());
                    issue(
                        issues,
                        WorkflowGovernanceIssueCode::InsufficientEvidenceStrength,
                        &path,
                        "passing evidence is below the evaluator strength floor",
                    );
                } else {
                    accepted.push(observation.evidence_ref.clone());
                    if let Some(principal) = &observation.principal {
                        accepted_principals.insert(principal.0.as_str());
                    }
                }
            }
        }
    }
    accepted.sort();
    accepted.dedup();
    rejected.extend(disproofs.iter().cloned());
    rejected.sort();
    rejected.dedup();

    let enough_observations = accepted.len() >= evaluator.minimum_passing_observations;
    let enough_principal_diversity =
        accepted_principals.len() >= evaluator.minimum_distinct_principals;
    if enough_observations && !enough_principal_diversity {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InsufficientPrincipalDiversity,
            format!("workflow_governance_policy.claims.{}", claim.id.0),
            format!(
                "claim requires {} distinct evidence principals but has {}",
                evaluator.minimum_distinct_principals,
                accepted_principals.len()
            ),
        );
    }
    let enough_support = enough_observations && enough_principal_diversity;
    let mut status = match (
        enough_support,
        disproofs.is_empty(),
        evaluator.disproof_policy,
    ) {
        (true, false, WorkflowDisproofPolicy::RequireUncontestedSupport) => {
            issue(
                issues,
                WorkflowGovernanceIssueCode::ContradictoryEvidence,
                format!("workflow_governance_policy.claims.{}", claim.id.0),
                "claim has both qualifying support and current disproof evidence",
            );
            WorkflowClaimResultStatus::Contradictory
        }
        (_, false, WorkflowDisproofPolicy::RejectAnyDisproof)
        | (false, false, WorkflowDisproofPolicy::RequireUncontestedSupport) => {
            WorkflowClaimResultStatus::Disproven
        }
        (true, true, _) => WorkflowClaimResultStatus::Verified,
        (false, true, _) if accepted.is_empty() => WorkflowClaimResultStatus::Unknown,
        (false, true, _) => WorkflowClaimResultStatus::Supported,
    };
    if waiver.is_some_and(|waiver| validate_waiver(claim, waiver, target, observed_at_unix, issues))
    {
        status = WorkflowClaimResultStatus::Waived;
    }
    WorkflowClaimResult {
        claim_id: claim.id.0.clone(),
        statement: claim.statement.clone(),
        status,
        accepted_evidence_refs: accepted,
        rejected_evidence_refs: rejected,
    }
}

fn validate_waiver(
    claim: &forge_core_contracts::WorkflowClaimPolicy,
    waiver: &WorkflowClaimWaiverObservation,
    target: ReadinessTarget,
    observed_at_unix: u64,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) -> bool {
    let path = format!(
        "workflow_governance_evaluation.waivers.{}",
        waiver.claim_ref.0
    );
    let WorkflowClaimWaiverPolicy::Authorized {
        max_target: policy_max_target,
        authority_scope,
        max_age_seconds,
    } = &claim.waiver
    else {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidWaiver,
            &path,
            "claim policy does not permit a waiver",
        );
        return false;
    };

    let mut valid = true;
    if waiver.authority_scope != *authority_scope {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidWaiver,
            format!("{path}.authority_scope"),
            format!("waiver requires authority scope {}", authority_scope.0),
        );
        valid = false;
    }
    if waiver.max_target.rank() > policy_max_target.rank()
        || target.rank() > waiver.max_target.rank()
    {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidWaiver,
            format!("{path}.max_target"),
            "waiver target exceeds either its authorization or the admitted claim policy",
        );
        valid = false;
    }
    if waiver.authorized_at_unix > observed_at_unix
        || waiver.expires_at_unix < waiver.authorized_at_unix
    {
        issue(
            issues,
            WorkflowGovernanceIssueCode::InvalidWaiver,
            &path,
            "waiver authorization interval is invalid for the evaluation clock",
        );
        valid = false;
    } else if observed_at_unix > waiver.expires_at_unix
        || observed_at_unix.saturating_sub(waiver.authorized_at_unix) > *max_age_seconds
    {
        issue(
            issues,
            WorkflowGovernanceIssueCode::ExpiredWaiver,
            &path,
            "waiver is expired or older than the admitted claim policy permits",
        );
        valid = false;
    }
    valid
}

fn evaluate_obligations(
    policy: &WorkflowGovernancePolicy,
    claim_statuses: &BTreeMap<&str, WorkflowClaimResultStatus>,
) -> Vec<WorkflowObligationResult> {
    let mut obligations = policy.obligations.iter().collect::<Vec<_>>();
    obligations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    obligations
        .into_iter()
        .map(|obligation| {
            let statuses = obligation
                .claim_refs
                .iter()
                .map(|claim_ref| claim_statuses[claim_ref.0.as_str()])
                .collect::<Vec<_>>();
            let status = if statuses.iter().all(|status| {
                matches!(
                    status,
                    WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                )
            }) {
                ObligationStatus::Satisfied
            } else if statuses.iter().any(|status| {
                matches!(
                    status,
                    WorkflowClaimResultStatus::Disproven | WorkflowClaimResultStatus::Contradictory
                )
            }) {
                ObligationStatus::Blocked
            } else {
                ObligationStatus::Pending
            };
            WorkflowObligationResult {
                obligation_id: obligation.id.0.clone(),
                description: obligation.description.clone(),
                criticality: obligation.criticality,
                required_before: obligation.required_before,
                status,
                claim_refs: obligation.claim_refs.clone(),
            }
        })
        .collect()
}

fn capability_gaps(
    policy: &WorkflowGovernancePolicy,
    input: &forge_core_contracts::WorkflowGovernanceEvaluation,
) -> Vec<CapabilityGap> {
    let available = input
        .available_capability_refs
        .iter()
        .map(|capability| capability.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut gaps = policy
        .capability_requirements
        .iter()
        .filter(|requirement| !available.contains(requirement.id.0.as_str()))
        .map(|requirement| CapabilityGap {
            id: requirement.id.clone(),
            kind: requirement.kind,
            description: requirement.description.clone(),
            affected_claim_refs: requirement.affected_claim_refs.clone(),
            resolution_options: requirement.resolution_options.clone(),
            blocking: requirement.blocks_before.rank() <= input.target.rank(),
            blocks_before: requirement.blocks_before,
        })
        .collect::<Vec<_>>();
    gaps.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    gaps
}

fn decision_requests(
    policy: &WorkflowGovernancePolicy,
    input: &forge_core_contracts::WorkflowGovernanceEvaluation,
    claim_statuses: &BTreeMap<&str, WorkflowClaimResultStatus>,
) -> Vec<DecisionRequest> {
    let resolved = input
        .resolved_decision_refs
        .iter()
        .map(|decision| decision.0.as_str())
        .collect::<BTreeSet<_>>();
    let observed_needs = input
        .decision_need_refs
        .iter()
        .map(|decision| decision.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut requests = policy
        .decision_rules
        .iter()
        .filter(|decision| !resolved.contains(decision.id.0.as_str()))
        .filter(|decision| decision.blocks_before.rank() <= input.target.rank())
        .filter(|decision| match decision.activation {
            WorkflowDecisionActivation::ObservedNeed => {
                observed_needs.contains(decision.id.0.as_str())
            }
            WorkflowDecisionActivation::ClaimVerified => {
                decision.claim_ref.as_ref().is_some_and(|claim_ref| {
                    matches!(
                        claim_statuses[claim_ref.0.as_str()],
                        WorkflowClaimResultStatus::Verified
                    )
                })
            }
            WorkflowDecisionActivation::AllClaimsVerified => {
                !claim_statuses.is_empty()
                    && claim_statuses
                        .values()
                        .all(|status| matches!(status, WorkflowClaimResultStatus::Verified))
            }
            WorkflowDecisionActivation::ClaimUnresolved => {
                decision.claim_ref.as_ref().is_some_and(|claim_ref| {
                    !matches!(
                        claim_statuses[claim_ref.0.as_str()],
                        WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                    )
                })
            }
            WorkflowDecisionActivation::ClaimDisproven => {
                decision.claim_ref.as_ref().is_some_and(|claim_ref| {
                    matches!(
                        claim_statuses[claim_ref.0.as_str()],
                        WorkflowClaimResultStatus::Disproven
                            | WorkflowClaimResultStatus::Contradictory
                    )
                })
            }
        })
        .map(|decision| DecisionRequest {
            id: decision.id.clone(),
            question: decision.question.clone(),
            reason: decision.reason,
            alternatives: decision.alternatives.clone(),
            recommended_alternative_ref: decision.recommended_alternative_ref.clone(),
            blocking: decision.blocking,
            blocks_before: decision.blocks_before,
        })
        .collect::<Vec<_>>();
    requests.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    requests
}

fn evaluate_eligibility(
    policy: &WorkflowGovernancePolicy,
    input: &forge_core_contracts::WorkflowGovernanceEvaluation,
    issues: &mut Vec<WorkflowGovernanceIssue>,
) -> WorkflowEligibilityVerdict {
    let current = Phase::parse(&input.current_phase.0).expect("input validation parses phase");
    let phase_eligible = policy
        .eligible_phases
        .iter()
        .any(|phase| Phase::tag_eligible(&phase.0, current));
    if !phase_eligible {
        issue(
            issues,
            WorkflowGovernanceIssueCode::PhaseIneligible,
            "workflow_governance_evaluation.current_phase",
            format!(
                "phase {} is not eligible for policy {}",
                input.current_phase.0, policy.id.0
            ),
        );
    }
    let completed = input
        .completed_policy_refs
        .iter()
        .map(|policy_ref| policy_ref.0.as_str())
        .collect::<BTreeSet<_>>();
    let not_applicable = input
        .not_applicable_policy_refs
        .iter()
        .map(|policy_ref| policy_ref.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut prerequisite_blockers = 0_u32;
    for prerequisite in &policy.prerequisites {
        if completed.contains(prerequisite.policy_ref.0.as_str()) {
            continue;
        }
        match prerequisite.requirement {
            WorkflowPrerequisiteRequirement::Always => {
                prerequisite_blockers = prerequisite_blockers.saturating_add(1);
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::MissingPrerequisite,
                    "workflow_governance_evaluation.completed_policy_refs",
                    format!(
                        "required policy {} is not complete",
                        prerequisite.policy_ref.0
                    ),
                );
            }
            WorkflowPrerequisiteRequirement::WhenApplicable
                if !not_applicable.contains(prerequisite.policy_ref.0.as_str()) =>
            {
                prerequisite_blockers = prerequisite_blockers.saturating_add(1);
                issue(
                    issues,
                    WorkflowGovernanceIssueCode::UnknownApplicability,
                    "workflow_governance_evaluation.not_applicable_policy_refs",
                    format!(
                        "conditional prerequisite {} lacks a complete or not-applicable receipt",
                        prerequisite.policy_ref.0
                    ),
                );
            }
            WorkflowPrerequisiteRequirement::WhenApplicable => {}
        }
    }
    if phase_eligible && prerequisite_blockers == 0 {
        WorkflowEligibilityVerdict::Eligible
    } else {
        WorkflowEligibilityVerdict::Ineligible
    }
}

fn next_actions(
    policy: &WorkflowGovernancePolicy,
    status: WorkflowGovernanceStatus,
    claims: &[WorkflowClaimResult],
    gaps: &[CapabilityGap],
    decisions: &[DecisionRequest],
    issues: &[WorkflowGovernanceIssue],
) -> Vec<NextAction> {
    #[derive(Debug)]
    struct Draft {
        priority: u8,
        key: String,
        action: NextAction,
    }
    let mut drafts = Vec::new();
    for gap in gaps {
        drafts.push(Draft {
            priority: if gap.blocking { 10 } else { 50 },
            key: gap.id.0.clone(),
            action: NextAction {
                id: StableId(format!("action.acquire.{}", gap.id.0)),
                kind: NextActionKind::AcquireCapability,
                description: format!("Resolve capability gap: {}", gap.description),
                addresses_claim_refs: gap.affected_claim_refs.clone(),
                rationale: "required capability is absent; guessing is forbidden".to_owned(),
                rank: 0,
            },
        });
    }
    for decision in decisions {
        drafts.push(Draft {
            priority: if decision.blocking { 20 } else { 60 },
            key: decision.id.0.clone(),
            action: NextAction {
                id: StableId(format!("action.ask.{}", decision.id.0)),
                kind: NextActionKind::AskHuman,
                description: decision.question.clone(),
                addresses_claim_refs: Vec::new(),
                rationale: "policy identifies an irreducible human judgment".to_owned(),
                rank: 0,
            },
        });
    }
    for claim in claims.iter().filter(|claim| {
        matches!(
            claim.status,
            WorkflowClaimResultStatus::Disproven | WorkflowClaimResultStatus::Contradictory
        )
    }) {
        drafts.push(Draft {
            priority: 30,
            key: claim.claim_id.clone(),
            action: NextAction {
                id: StableId(format!("action.correct.{}", claim.claim_id)),
                kind: NextActionKind::Challenge,
                description: format!(
                    "Correct or re-evaluate disproven claim: {}",
                    claim.statement
                ),
                addresses_claim_refs: vec![StableId(claim.claim_id.clone())],
                rationale: "current evidence contradicts the governed claim".to_owned(),
                rank: 0,
            },
        });
    }
    for claim in claims.iter().filter(|claim| {
        matches!(
            claim.status,
            WorkflowClaimResultStatus::Unknown | WorkflowClaimResultStatus::Supported
        )
    }) {
        drafts.push(Draft {
            priority: 40,
            key: claim.claim_id.clone(),
            action: NextAction {
                id: StableId(format!("action.evaluate.{}", claim.claim_id)),
                kind: NextActionKind::Evaluate,
                description: format!("Collect qualifying evidence for: {}", claim.statement),
                addresses_claim_refs: vec![StableId(claim.claim_id.clone())],
                rationale: "claim has not met its bound evaluator rule".to_owned(),
                rank: 0,
            },
        });
    }
    if status == WorkflowGovernanceStatus::Ineligible {
        for governance_issue in issues.iter().filter(|governance_issue| {
            matches!(
                governance_issue.code,
                WorkflowGovernanceIssueCode::PhaseIneligible
                    | WorkflowGovernanceIssueCode::MissingPrerequisite
                    | WorkflowGovernanceIssueCode::UnknownApplicability
            )
        }) {
            drafts.push(Draft {
                priority: 5,
                key: governance_issue.message.clone(),
                action: NextAction {
                    id: StableId(format!(
                        "action.eligibility.{}",
                        stable_fragment(&governance_issue.message)
                    )),
                    kind: NextActionKind::Implement,
                    description: governance_issue.message.clone(),
                    addresses_claim_refs: Vec::new(),
                    rationale: "workflow eligibility is governed before execution".to_owned(),
                    rank: 0,
                },
            });
        }
    }
    if status == WorkflowGovernanceStatus::Complete {
        let mut claim_refs = policy
            .claims
            .iter()
            .map(|claim| claim.id.clone())
            .collect::<Vec<_>>();
        claim_refs.sort_by(|left, right| left.0.cmp(&right.0));
        claim_refs.dedup();
        drafts.push(Draft {
            priority: 100,
            key: policy.id.0.clone(),
            action: NextAction {
                id: StableId(format!("action.request-trusted-evaluation.{}", policy.id.0)),
                kind: NextActionKind::Evaluate,
                description: format!(
                    "Candidate conditions for {} appear complete; obtain a trusted Project Snapshot evaluation before progression or completion",
                    policy.id.0
                ),
                addresses_claim_refs: claim_refs,
                rationale: "caller-authored observations are simulation-only and cannot authorize action"
                    .to_owned(),
                rank: 0,
            },
        });
    }
    drafts.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.key.cmp(&right.key))
    });
    drafts
        .into_iter()
        .enumerate()
        .map(|(index, mut draft)| {
            draft.action.rank = u32::try_from(index + 1).unwrap_or(u32::MAX);
            draft.action
        })
        .collect()
}

fn validate_local_id<'a>(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    ids: &mut BTreeSet<&'a str>,
    id: &'a StableId,
    path: impl Into<String>,
) {
    let path = path.into();
    require_nonblank(issues, format!("{path}.id"), &id.0);
    insert_unique(issues, ids, &id.0, format!("{path}.{}", id.0));
}

fn insert_unique<'a>(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    ids: &mut BTreeSet<&'a str>,
    id: &'a str,
    path: impl Into<String>,
) {
    if !ids.insert(id) {
        issue(
            issues,
            WorkflowGovernanceIssueCode::DuplicateIdentifier,
            path,
            format!("identifier {id} occurs more than once"),
        );
    }
}

fn validate_unique_refs(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    refs: &[StableId],
    path: impl Into<String>,
) {
    let path = path.into();
    let mut seen = BTreeSet::new();
    for item in refs {
        if !seen.insert(item.0.as_str()) {
            issue(
                issues,
                WorkflowGovernanceIssueCode::DuplicateReference,
                format!("{path}.{}", item.0),
                "reference occurs more than once",
            );
        }
    }
}

fn validate_nonblank_unique_strings(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    values: &[String],
    path: impl Into<String>,
) {
    let path = path.into();
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        require_nonblank(issues, format!("{path}[{index}]"), value);
        if !seen.insert(value.as_str()) {
            issue(
                issues,
                WorkflowGovernanceIssueCode::DuplicateReference,
                format!("{path}[{index}]"),
                "value occurs more than once",
            );
        }
    }
}

fn require_known_ref(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    known: &BTreeSet<&str>,
    value: &str,
    path: impl Into<String>,
) {
    if !known.contains(value) {
        issue(
            issues,
            WorkflowGovernanceIssueCode::DanglingReference,
            path,
            format!("unknown reference {value}"),
        );
    }
}

fn require_nonblank(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    path: impl Into<String>,
    value: &str,
) {
    if value.trim().is_empty() {
        issue(
            issues,
            WorkflowGovernanceIssueCode::BlankRequiredField,
            path,
            "required value must be non-blank",
        );
    }
}

fn issue(
    issues: &mut Vec<WorkflowGovernanceIssue>,
    code: WorkflowGovernanceIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowGovernanceIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

fn sorted_issues(mut issues: Vec<WorkflowGovernanceIssue>) -> Vec<WorkflowGovernanceIssue> {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.message.cmp(&right.message))
    });
    issues.dedup();
    issues
}

fn stable_fragment(value: &str) -> String {
    let fragment = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    fragment
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-")
}
