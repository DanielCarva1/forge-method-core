//! Validation for the operation-contract-first guide protocol.

use forge_core_contracts::operation::{
    AutonomyMode, ExecutionMode, ForgeOperation, HostAction, HumanInputRequirement, MutationPolicy,
    NextActor, OperationSideEffectPolicy, PromptMode,
};
use forge_core_contracts::{
    Catalog, GuideProtocolDocument, OperationContract, GUIDE_PROTOCOL_SCHEMA_VERSION,
};
use forge_core_decisions::{
    validate_guide_decision, GuideRejection, GuideValidation, ProvidedGateResult,
};
use forge_core_validate::validate_operation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideRoute {
    Facilitation,
    Research,
    VisualAlignment,
    CorrectCourse,
    AlreadyDone,
    MechanicalExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideProtocolRejectionCode {
    UnsupportedSchemaVersion,
    UnrecognizedCurrentPhase,
    UnknownWorkflow,
    NotEligibleInPhase,
    IllegalTransition,
    UnrecognizedProposedPhase,
    OperationContractInvalid,
    OperationSourceNotGuide,
    WorkflowMismatch,
    PhaseMismatch,
    AllowedActionsMismatch,
    StateVersionMismatch,
    OperationDiagnosticsErrors,
    UnsupportedRoute,
    RoutePolicyMismatch,
}

impl GuideProtocolRejectionCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSchemaVersion => "unsupported_schema_version",
            Self::UnrecognizedCurrentPhase => "unrecognized_current_phase",
            Self::UnknownWorkflow => "unknown_workflow",
            Self::NotEligibleInPhase => "not_eligible_in_phase",
            Self::IllegalTransition => "illegal_transition",
            Self::UnrecognizedProposedPhase => "unrecognized_proposed_phase",
            Self::OperationContractInvalid => "operation_contract_invalid",
            Self::OperationSourceNotGuide => "operation_source_not_guide",
            Self::WorkflowMismatch => "workflow_mismatch",
            Self::PhaseMismatch => "phase_mismatch",
            Self::AllowedActionsMismatch => "allowed_actions_mismatch",
            Self::StateVersionMismatch => "state_version_mismatch",
            Self::OperationDiagnosticsErrors => "operation_diagnostics_errors",
            Self::UnsupportedRoute => "unsupported_route",
            Self::RoutePolicyMismatch => "route_policy_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuideProtocolRejection {
    pub code: GuideProtocolRejectionCode,
    pub detail: String,
}

impl GuideProtocolRejection {
    fn new(code: GuideProtocolRejectionCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Validate one guide recommendation and its exact next `OperationContract` as a
/// single route. Acceptance grants no mutation authority; it only proves that
/// the host received one internally consistent, structurally valid operation
/// response that the existing planner/executor may evaluate next.
///
/// # Errors
///
/// Returns a typed rejection when the guide decision is illegal, the operation
/// is invalid, the two documents disagree, or the operation does not satisfy
/// one of the closed guide-route policies.
pub fn validate_guide_protocol(
    document: &GuideProtocolDocument,
    catalog: &Catalog,
    gates: &[ProvidedGateResult],
) -> Result<GuideRoute, GuideProtocolRejection> {
    if document.schema_version != GUIDE_PROTOCOL_SCHEMA_VERSION {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::UnsupportedSchemaVersion,
            format!(
                "guide protocol schema {} is unsupported; expected {}",
                document.schema_version, GUIDE_PROTOCOL_SCHEMA_VERSION
            ),
        ));
    }

    let protocol = &document.guide_protocol;
    match validate_guide_decision(&protocol.decision, catalog, gates) {
        GuideValidation::Accepted => {}
        GuideValidation::Rejected(rejection) => {
            return Err(guide_decision_rejection(&rejection));
        }
    }

    let validation = validate_operation(&protocol.next_operation);
    if validation.has_errors() {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::OperationContractInvalid,
            format!(
                "next operation failed structural validation with {} error(s)",
                validation.error_count()
            ),
        ));
    }

    let operation = &protocol.next_operation.operation_contract;
    if operation.schema_version != "0.1" {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::OperationContractInvalid,
            format!(
                "operation contract schema {} is unsupported; expected 0.1",
                operation.schema_version
            ),
        ));
    }
    if operation.source.operation != ForgeOperation::Guide {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::OperationSourceNotGuide,
            "the next operation was not derived from the guide surface",
        ));
    }
    if operation.recommendation.workflow != protocol.decision.recommended_workflow {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::WorkflowMismatch,
            "guide decision and next operation name different workflows",
        ));
    }

    let expected_phase = protocol
        .decision
        .proposed_next_phase
        .as_ref()
        .unwrap_or(&protocol.decision.current_phase);
    if operation.recommendation.phase != *expected_phase {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::PhaseMismatch,
            format!(
                "next operation phase {} does not match validated guide phase {}",
                operation.recommendation.phase.0, expected_phase.0
            ),
        ));
    }
    if operation.allowed_actions != protocol.decision.allowed_actions {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::AllowedActionsMismatch,
            "guide decision and next operation carry different allowed actions",
        ));
    }
    if operation.project_ref.state_version
        != operation
            .coordination_scope
            .concurrency
            .expected_state_version
    {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::StateVersionMismatch,
            "next operation does not bind one exact observed state version",
        ));
    }
    if !operation.diagnostics.errors.is_empty() {
        return Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::OperationDiagnosticsErrors,
            "next operation carries unresolved error diagnostics",
        ));
    }

    let route = classify_route(operation)?;
    validate_route_policy(route, operation)?;
    Ok(route)
}

fn guide_decision_rejection(rejection: &GuideRejection) -> GuideProtocolRejection {
    let code = match rejection {
        GuideRejection::UnrecognizedCurrentPhase { .. } => {
            GuideProtocolRejectionCode::UnrecognizedCurrentPhase
        }
        GuideRejection::UnknownWorkflow { .. } => GuideProtocolRejectionCode::UnknownWorkflow,
        GuideRejection::NotEligibleInPhase { .. } => GuideProtocolRejectionCode::NotEligibleInPhase,
        GuideRejection::IllegalTransition(_) => GuideProtocolRejectionCode::IllegalTransition,
        GuideRejection::UnrecognizedProposedPhase { .. } => {
            GuideProtocolRejectionCode::UnrecognizedProposedPhase
        }
    };
    GuideProtocolRejection::new(code, format!("{rejection:?}"))
}

fn classify_route(operation: &OperationContract) -> Result<GuideRoute, GuideProtocolRejection> {
    let workflow = operation.recommendation.workflow.0.as_str();
    let action = operation.recommendation.action.0.as_str();
    let route = if action == "story_already_done" {
        GuideRoute::AlreadyDone
    } else if workflow == "guidance-engine" && action == "address_user_frustration" {
        GuideRoute::CorrectCourse
    } else if workflow == "visual-alignment-prototype" {
        GuideRoute::VisualAlignment
    } else {
        match operation.autonomy.mode {
            AutonomyMode::Facilitate => GuideRoute::Facilitation,
            AutonomyMode::Research => GuideRoute::Research,
            AutonomyMode::Execute => GuideRoute::MechanicalExecution,
            _ => {
                return Err(GuideProtocolRejection::new(
                    GuideProtocolRejectionCode::UnsupportedRoute,
                    format!(
                        "workflow {} does not map to a closed guide route",
                        operation.recommendation.workflow.0
                    ),
                ));
            }
        }
    };
    Ok(route)
}

fn validate_route_policy(
    route: GuideRoute,
    operation: &OperationContract,
) -> Result<(), GuideProtocolRejection> {
    let valid = match route {
        GuideRoute::Facilitation | GuideRoute::VisualAlignment | GuideRoute::CorrectCourse => {
            operation.autonomy.mode == AutonomyMode::Facilitate
                && nonmutating_route(operation)
                && operation.recommendation.next_actor == NextActor::Human
                && operation.recommendation.next_operation.is_none()
                && operation.recommendation.host_action == HostAction::RequestConfirmation
                && operation.human.input_requirement == HumanInputRequirement::Required
                && operation.human.prompt.mode == PromptMode::Question
        }
        GuideRoute::Research => {
            operation.autonomy.mode == AutonomyMode::Research
                && nonmutating_route(operation)
                && operation.recommendation.next_actor == NextActor::HostAgent
                && operation.recommendation.next_operation.is_none()
                && operation.recommendation.host_action == HostAction::CallOperation
                && operation.human.input_requirement == HumanInputRequirement::None
        }
        GuideRoute::AlreadyDone => {
            operation.autonomy.mode == AutonomyMode::Observe
                && nonmutating_route(operation)
                && operation.recommendation.next_actor == NextActor::Human
                && operation.recommendation.next_operation.is_none()
                && operation.recommendation.host_action == HostAction::ShowStatus
                && operation.human.input_requirement == HumanInputRequirement::None
        }
        GuideRoute::MechanicalExecution => {
            operation.autonomy.mode == AutonomyMode::Execute
                && operation.recommendation.next_actor == NextActor::HostAgent
                && operation.recommendation.next_operation.is_some()
                && operation.recommendation.host_action == HostAction::CallOperation
                && operation.authority.mutation_policy != MutationPolicy::Forbidden
                && operation.authority.side_effect_policy != OperationSideEffectPolicy::ReadOnly
                && operation.execution_policy.mode != ExecutionMode::ObserveOnly
                && operation.human.input_requirement == HumanInputRequirement::None
        }
    };

    if valid {
        Ok(())
    } else {
        Err(GuideProtocolRejection::new(
            GuideProtocolRejectionCode::RoutePolicyMismatch,
            format!(
                "next operation {} violates the {:?} guide route policy",
                operation.contract_id.0, route
            ),
        ))
    }
}

fn nonmutating_route(operation: &OperationContract) -> bool {
    operation.authority.mutation_policy == MutationPolicy::Forbidden
        && operation.authority.side_effect_policy == OperationSideEffectPolicy::ReadOnly
        && operation.execution_policy.mode == ExecutionMode::ObserveOnly
        && operation.command_refs.is_empty()
        && operation.effect_contract_refs.is_empty()
}
