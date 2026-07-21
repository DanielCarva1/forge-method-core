use forge_core_contracts::autonomy_policy::{AutonomyMode as RoutingAutonomyMode, ToolClass};
use forge_core_contracts::funnel_autonomy::{
    FunnelAutonomyPolicy, FunnelAutonomyPolicyDocument, FunnelPhaseProfile,
    FunnelProtectedBoundaryPolicy, FUNNEL_AUTONOMY_POLICY_REF, FUNNEL_AUTONOMY_SCHEMA_VERSION,
};
use forge_core_contracts::operation::{
    AutonomyMode, HumanInputRequirement, MutationPolicy, OperationContract, OperationGateScope,
    OperationGateStatus, OperationRiskBoundary, OperationSideEffectPolicy,
};
use forge_core_contracts::tool_effect::AccessMode;
use forge_core_contracts::{Phase, ToolEffectContractDocument};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static ACCEPTED_POLICY: OnceLock<
    Result<FunnelAutonomyPolicyDocument, FunnelAutonomyPolicyRejection>,
> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelAutonomyPolicyRejection {
    pub issues: Vec<FunnelAutonomyPolicyIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelAutonomyPolicyIssue {
    pub code: FunnelAutonomyPolicyIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunnelAutonomyPolicyIssueCode {
    ParseFailed,
    UnsupportedSchemaVersion,
    MissingPhaseProfile,
    DuplicatePhaseProfile,
    MissingProtectedBoundary,
    DuplicateProtectedBoundary,
    ProtectedBoundaryScopeMismatch,
    MechanicalPolicyIncomplete,
    SemanticUncertaintyEscalationDisabled,
    ProtectedToolClassNotManual,
    AuthorityLimitWouldGrantAuthority,
    SelectedHostMustRemainNone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelPhaseDecision {
    pub profile: FunnelPhaseProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunnelOperationDisposition {
    Proceed,
    ReviewRequired,
    GateRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelOperationDecision {
    pub disposition: FunnelOperationDisposition,
    pub phase_profile: Option<FunnelPhaseProfile>,
    pub protected_boundaries: Vec<OperationRiskBoundary>,
    pub reasons: Vec<FunnelOperationReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunnelOperationReason {
    UnknownPhase,
    MechanicalPhaseNotEligible,
    MechanicalLaneClaimMissing,
    MechanicalGateNotPassed,
    MechanicalAuthorityEvidenceMissing,
    MechanicalEffectContractsMissing,
    UndeclaredDestructiveBoundary,
    ProtectedBoundaryPolicyMissing(OperationRiskBoundary),
    ProtectedBoundaryGateMissing(OperationRiskBoundary),
    ProtectedBoundaryGateNotPassed(OperationRiskBoundary),
    ProtectedBoundaryReviewMissing(OperationRiskBoundary),
}

/// Load and validate the exact accepted policy compiled into this crate.
///
/// The returned document is policy data only. It cannot mint a capability,
/// mutate state, advance phase, publish a release, select a host, or gain
/// private-key authority.
///
/// # Errors
///
/// Returns every structural policy issue when the embedded document is invalid.
pub fn load_accepted_funnel_autonomy_policy(
) -> Result<&'static FunnelAutonomyPolicyDocument, FunnelAutonomyPolicyRejection> {
    match ACCEPTED_POLICY.get_or_init(|| {
        let text = include_str!("../../../contracts/policies/funnel-autonomy.yaml");
        let document =
            yaml_serde::from_str::<FunnelAutonomyPolicyDocument>(text).map_err(|error| {
                FunnelAutonomyPolicyRejection {
                    issues: vec![FunnelAutonomyPolicyIssue {
                        code: FunnelAutonomyPolicyIssueCode::ParseFailed,
                        path: FUNNEL_AUTONOMY_POLICY_REF.to_owned(),
                        message: error.to_string(),
                    }],
                }
            })?;
        validate_funnel_autonomy_policy(&document)?;
        Ok(document)
    }) {
        Ok(document) => Ok(document),
        Err(rejection) => Err(rejection.clone()),
    }
}

/// Validate one funnel-autonomy policy without granting it authority.
///
/// # Errors
///
/// Returns a complete issue list for malformed or authority-expanding policy.
pub fn validate_funnel_autonomy_policy(
    document: &FunnelAutonomyPolicyDocument,
) -> Result<(), FunnelAutonomyPolicyRejection> {
    let mut issues = Vec::new();
    if document.schema_version != FUNNEL_AUTONOMY_SCHEMA_VERSION {
        push_issue(
            &mut issues,
            FunnelAutonomyPolicyIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!(
                "unsupported funnel policy schema {}; expected {}",
                document.schema_version, FUNNEL_AUTONOMY_SCHEMA_VERSION
            ),
        );
    }

    let policy = &document.funnel_autonomy_policy;
    for phase in Phase::ALL {
        let count = policy
            .phase_profiles
            .iter()
            .filter(|profile| profile.phase == phase)
            .count();
        if count == 0 {
            push_issue(
                &mut issues,
                FunnelAutonomyPolicyIssueCode::MissingPhaseProfile,
                "funnel_autonomy_policy.phase_profiles",
                format!("missing profile for phase {phase}"),
            );
        } else if count > 1 {
            push_issue(
                &mut issues,
                FunnelAutonomyPolicyIssueCode::DuplicatePhaseProfile,
                "funnel_autonomy_policy.phase_profiles",
                format!("phase {phase} appears {count} times"),
            );
        }
    }

    for boundary in [
        OperationRiskBoundary::Destructive,
        OperationRiskBoundary::Release,
        OperationRiskBoundary::Authority,
    ] {
        let matches = policy
            .protected_boundaries
            .iter()
            .filter(|rule| rule.boundary == boundary)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            push_issue(
                &mut issues,
                FunnelAutonomyPolicyIssueCode::MissingProtectedBoundary,
                "funnel_autonomy_policy.protected_boundaries",
                format!("missing policy for {boundary:?} boundary"),
            );
        } else if matches.len() > 1 {
            push_issue(
                &mut issues,
                FunnelAutonomyPolicyIssueCode::DuplicateProtectedBoundary,
                "funnel_autonomy_policy.protected_boundaries",
                format!("{boundary:?} boundary appears {} times", matches.len()),
            );
        }
        for rule in matches {
            if rule.required_gate_scope != gate_scope_for(boundary) {
                push_issue(
                    &mut issues,
                    FunnelAutonomyPolicyIssueCode::ProtectedBoundaryScopeMismatch,
                    "funnel_autonomy_policy.protected_boundaries.required_gate_scope",
                    format!(
                        "{boundary:?} must use {:?} gate scope",
                        gate_scope_for(boundary)
                    ),
                );
            }
        }
    }

    let mechanical = &policy.mechanical_loop;
    if mechanical.eligible_phases.is_empty()
        || mechanical.autonomy_modes.is_empty()
        || !mechanical.require_lane_claim
        || !mechanical.require_gate_pass
        || !mechanical.require_authority_evidence
        || !mechanical.require_effect_contracts
    {
        push_issue(
            &mut issues,
            FunnelAutonomyPolicyIssueCode::MechanicalPolicyIncomplete,
            "funnel_autonomy_policy.mechanical_loop",
            "mechanical loops must require an eligible phase, lane claim, passing gates, authority evidence, and effect contracts",
        );
    }

    if !policy.routing_policy.escalation.on_semantic_uncertainty {
        push_issue(
            &mut issues,
            FunnelAutonomyPolicyIssueCode::SemanticUncertaintyEscalationDisabled,
            "funnel_autonomy_policy.routing_policy.escalation.on_semantic_uncertainty",
            "semantic uncertainty must restore human guidance or research pressure",
        );
    }

    for class in [
        ToolClass::NetworkEgress,
        ToolClass::PackageInstall,
        ToolClass::SecretAccess,
        ToolClass::GitMutation,
    ] {
        if policy.routing_policy.effective_mode_for(class) != RoutingAutonomyMode::Manual {
            push_issue(
                &mut issues,
                FunnelAutonomyPolicyIssueCode::ProtectedToolClassNotManual,
                "funnel_autonomy_policy.routing_policy.tool_classes",
                format!("protected tool class {class:?} must remain manual"),
            );
        }
    }

    let limits = &policy.authority_limits;
    if limits.grants_mutation_authority
        || limits.grants_phase_authority
        || limits.grants_release_authority
        || limits.grants_signing_or_private_key_authority
    {
        push_issue(
            &mut issues,
            FunnelAutonomyPolicyIssueCode::AuthorityLimitWouldGrantAuthority,
            "funnel_autonomy_policy.authority_limits",
            "the funnel policy must not grant mutation, phase, release, signing, or private-key authority",
        );
    }
    if limits.selected_host.is_some() {
        push_issue(
            &mut issues,
            FunnelAutonomyPolicyIssueCode::SelectedHostMustRemainNone,
            "funnel_autonomy_policy.authority_limits.selected_host",
            "selected_host remains none",
        );
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(FunnelAutonomyPolicyRejection { issues })
    }
}

/// Project one exact phase profile from the accepted typed policy.
///
/// # Errors
///
/// Returns a policy rejection if the policy is malformed or omits the phase.
pub fn evaluate_funnel_phase(
    policy: &FunnelAutonomyPolicyDocument,
    phase: Phase,
) -> Result<FunnelPhaseDecision, FunnelAutonomyPolicyRejection> {
    validate_funnel_autonomy_policy(policy)?;
    let profile = policy
        .funnel_autonomy_policy
        .phase_profiles
        .iter()
        .find(|profile| profile.phase == phase)
        .cloned()
        .ok_or_else(|| FunnelAutonomyPolicyRejection {
            issues: vec![FunnelAutonomyPolicyIssue {
                code: FunnelAutonomyPolicyIssueCode::MissingPhaseProfile,
                path: "funnel_autonomy_policy.phase_profiles".to_owned(),
                message: format!("missing profile for phase {phase}"),
            }],
        })?;
    Ok(FunnelPhaseDecision { profile })
}

/// Evaluate an `OperationContract` against the same policy used by Guide and the
/// runtime gate. Effect documents are optional for planning and mandatory for
/// detecting an undeclared destructive write at the execution boundary.
///
/// # Errors
///
/// Returns a policy rejection before evaluating an operation when the policy is
/// malformed.
pub fn evaluate_funnel_operation(
    policy: &FunnelAutonomyPolicyDocument,
    operation: &OperationContract,
    effects: &[ToolEffectContractDocument],
) -> Result<FunnelOperationDecision, FunnelAutonomyPolicyRejection> {
    validate_funnel_autonomy_policy(policy)?;
    let Some(phase) = Phase::parse(&operation.recommendation.phase.0) else {
        return Ok(FunnelOperationDecision {
            disposition: FunnelOperationDisposition::Blocked,
            phase_profile: None,
            protected_boundaries: operation.risk_boundaries.clone(),
            reasons: vec![FunnelOperationReason::UnknownPhase],
        });
    };
    let phase_profile = evaluate_funnel_phase(policy, phase)?.profile;
    let funnel = &policy.funnel_autonomy_policy;
    let mut reasons = Vec::new();
    let mut boundaries = operation.risk_boundaries.clone();
    let mut blocked = false;

    let destructive_effect = effects.iter().any(|document| {
        document
            .tool_effect_contract
            .write_set
            .iter()
            .any(|write| write.destructive || write.access_mode == AccessMode::Delete)
    });
    if destructive_effect && !boundaries.contains(&OperationRiskBoundary::Destructive) {
        boundaries.push(OperationRiskBoundary::Destructive);
        reasons.push(FunnelOperationReason::UndeclaredDestructiveBoundary);
        blocked = true;
    }
    if operation.authority.side_effect_policy == OperationSideEffectPolicy::Publish
        && !boundaries.contains(&OperationRiskBoundary::Release)
    {
        boundaries.push(OperationRiskBoundary::Release);
    }

    let mut gate_required = false;
    for boundary in &boundaries {
        let Some(rule) = boundary_policy(funnel, *boundary) else {
            reasons.push(FunnelOperationReason::ProtectedBoundaryPolicyMissing(
                *boundary,
            ));
            gate_required = true;
            continue;
        };
        if !has_required_gate_scope(operation, rule.required_gate_scope) {
            reasons.push(FunnelOperationReason::ProtectedBoundaryGateMissing(
                *boundary,
            ));
            gate_required = true;
        }
        if operation.gates.current_gate_status != OperationGateStatus::Pass {
            reasons.push(FunnelOperationReason::ProtectedBoundaryGateNotPassed(
                *boundary,
            ));
            gate_required = true;
        }
        if rule.human_checkpoint_required
            && operation.gates.current_gate_status != OperationGateStatus::Pass
            && operation.authority.mutation_policy != MutationPolicy::RequiresReview
            && operation.human.input_requirement == HumanInputRequirement::None
        {
            reasons.push(FunnelOperationReason::ProtectedBoundaryReviewMissing(
                *boundary,
            ));
            gate_required = true;
        }
    }

    let mut review_required = false;
    if mechanical_operation(funnel, operation) {
        let mechanical = &funnel.mechanical_loop;
        if !mechanical.eligible_phases.contains(&phase) {
            reasons.push(FunnelOperationReason::MechanicalPhaseNotEligible);
            review_required = true;
        }
        if mechanical.require_lane_claim
            && (!operation
                .coordination_scope
                .write_authority
                .requires_lane_claim
                || operation
                    .coordination_scope
                    .write_authority
                    .claim_contract_ref
                    .is_none())
        {
            reasons.push(FunnelOperationReason::MechanicalLaneClaimMissing);
            review_required = true;
        }
        if mechanical.require_gate_pass
            && operation.gates.current_gate_status != OperationGateStatus::Pass
        {
            reasons.push(FunnelOperationReason::MechanicalGateNotPassed);
            gate_required = true;
        }
        if mechanical.require_authority_evidence
            && operation.authority.authority_evidence.is_empty()
        {
            reasons.push(FunnelOperationReason::MechanicalAuthorityEvidenceMissing);
            review_required = true;
        }
        if mechanical.require_effect_contracts && operation.effect_contract_refs.is_empty() {
            reasons.push(FunnelOperationReason::MechanicalEffectContractsMissing);
            review_required = true;
        }
    }

    let disposition = if blocked {
        FunnelOperationDisposition::Blocked
    } else if gate_required {
        FunnelOperationDisposition::GateRequired
    } else if review_required {
        FunnelOperationDisposition::ReviewRequired
    } else {
        FunnelOperationDisposition::Proceed
    };
    Ok(FunnelOperationDecision {
        disposition,
        phase_profile: Some(phase_profile),
        protected_boundaries: boundaries,
        reasons,
    })
}

fn boundary_policy(
    policy: &FunnelAutonomyPolicy,
    boundary: OperationRiskBoundary,
) -> Option<&FunnelProtectedBoundaryPolicy> {
    policy
        .protected_boundaries
        .iter()
        .find(|rule| rule.boundary == boundary)
}

fn mechanical_operation(policy: &FunnelAutonomyPolicy, operation: &OperationContract) -> bool {
    operation.authority.side_effect_policy != OperationSideEffectPolicy::ReadOnly
        && policy
            .mechanical_loop
            .autonomy_modes
            .contains(&operation.autonomy.mode)
        && matches!(
            operation.autonomy.mode,
            AutonomyMode::Plan | AutonomyMode::Execute | AutonomyMode::Repair
        )
}

fn has_required_gate_scope(operation: &OperationContract, scope: OperationGateScope) -> bool {
    operation
        .gates
        .required_before_mutation
        .iter()
        .any(|gate| gate.scope == scope)
}

const fn gate_scope_for(boundary: OperationRiskBoundary) -> OperationGateScope {
    match boundary {
        OperationRiskBoundary::Destructive => OperationGateScope::Destructive,
        OperationRiskBoundary::Release => OperationGateScope::Release,
        OperationRiskBoundary::Authority => OperationGateScope::Authority,
    }
}

fn push_issue(
    issues: &mut Vec<FunnelAutonomyPolicyIssue>,
    code: FunnelAutonomyPolicyIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(FunnelAutonomyPolicyIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::funnel_autonomy::{
        FunnelAmbiguityPressure, FunnelContactDensity, FunnelProceduralConfirmation,
    };

    fn accepted() -> FunnelAutonomyPolicyDocument {
        yaml_serde::from_str(include_str!(
            "../../../contracts/policies/funnel-autonomy.yaml"
        ))
        .expect("accepted funnel policy parses")
    }

    fn operation_fixture(name: &str) -> forge_core_contracts::OperationContractDocument {
        let text = match name {
            "mechanical" => include_str!(
                "../../../docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml"
            ),
            "release" => include_str!(
                "../../../docs/fixtures/operation-contract-v0/release-gate-required.yaml"
            ),
            "authority" => include_str!(
                "../../../docs/fixtures/operation-contract-v0/authority-transition-gate-required.yaml"
            ),
            _ => panic!("unknown fixture"),
        };
        yaml_serde::from_str(text).expect("operation fixture parses")
    }

    #[test]
    fn accepted_policy_is_complete_and_host_neutral() {
        let policy = load_accepted_funnel_autonomy_policy().expect("accepted policy");
        assert!(validate_funnel_autonomy_policy(policy).is_ok());
        assert!(policy
            .funnel_autonomy_policy
            .authority_limits
            .selected_host
            .is_none());
    }

    #[test]
    fn early_ambiguity_restores_human_guidance_and_research_pressure() {
        let policy = accepted();
        for phase in [Phase::Discovery, Phase::Specification] {
            let decision = evaluate_funnel_phase(&policy, phase).expect("phase profile");
            assert_eq!(decision.profile.contact_density, FunnelContactDensity::High);
            assert_eq!(
                decision.profile.ambiguity_pressure,
                FunnelAmbiguityPressure::HumanGuidanceAndResearch
            );
            assert_eq!(
                decision.profile.procedural_confirmation,
                FunnelProceduralConfirmation::Expected
            );
        }
    }

    #[test]
    fn settled_mechanical_loop_proceeds_only_with_claim_gate_and_evidence() {
        let policy = accepted();
        let mut operation = operation_fixture("mechanical").operation_contract;
        let decision =
            evaluate_funnel_operation(&policy, &operation, &[]).expect("mechanical decision");
        assert_eq!(decision.disposition, FunnelOperationDisposition::Proceed);

        operation
            .coordination_scope
            .write_authority
            .claim_contract_ref = None;
        let rejected =
            evaluate_funnel_operation(&policy, &operation, &[]).expect("mechanical rejection");
        assert_eq!(
            rejected.disposition,
            FunnelOperationDisposition::ReviewRequired
        );
        assert!(rejected
            .reasons
            .contains(&FunnelOperationReason::MechanicalLaneClaimMissing));
    }

    #[test]
    fn release_boundary_restores_release_gate() {
        let policy = accepted();
        let mut operation = operation_fixture("release").operation_contract;
        operation.risk_boundaries = vec![OperationRiskBoundary::Release];
        let decision =
            evaluate_funnel_operation(&policy, &operation, &[]).expect("release decision");
        assert_eq!(
            decision.disposition,
            FunnelOperationDisposition::GateRequired
        );
        assert!(decision
            .reasons
            .contains(&FunnelOperationReason::ProtectedBoundaryGateNotPassed(
                OperationRiskBoundary::Release
            )));
    }

    #[test]
    fn undeclared_destructive_effect_blocks_even_with_inverse() {
        let policy = accepted();
        let operation = operation_fixture("mechanical").operation_contract;
        let effect: ToolEffectContractDocument = yaml_serde::from_str(include_str!(
            "../../../contracts/effects/destructive-file-delete-with-inverse-effect.yaml"
        ))
        .expect("destructive effect fixture");
        let decision = evaluate_funnel_operation(&policy, &operation, &[effect])
            .expect("destructive decision");
        assert_eq!(decision.disposition, FunnelOperationDisposition::Blocked);
        assert!(decision
            .reasons
            .contains(&FunnelOperationReason::UndeclaredDestructiveBoundary));
    }

    #[test]
    fn authority_boundary_restores_typed_gate() {
        let policy = accepted();
        let operation = operation_fixture("authority").operation_contract;
        let decision =
            evaluate_funnel_operation(&policy, &operation, &[]).expect("authority decision");
        assert_eq!(
            decision.disposition,
            FunnelOperationDisposition::GateRequired
        );
        assert!(decision
            .reasons
            .contains(&FunnelOperationReason::ProtectedBoundaryGateNotPassed(
                OperationRiskBoundary::Authority
            )));
    }

    #[test]
    fn malformed_policy_that_selects_host_fails_closed() {
        let mut policy = accepted();
        policy.funnel_autonomy_policy.authority_limits.selected_host =
            Some(forge_core_contracts::StableId("host-x".to_owned()));
        let rejection = validate_funnel_autonomy_policy(&policy).expect_err("must reject host");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == FunnelAutonomyPolicyIssueCode::SelectedHostMustRemainNone
        }));
    }
}
