//! Effect staging: deciding which commands/effects are eligible for application.
//!
//! [`stage_operation_effects`] inspects a [`RuntimePlan`] and produces a
//! [`RuntimeEffectStagingPlan`] that the orchestrator and evidence layer consume.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEffectStagingPlan {
    pub status: RuntimeEffectStagingStatus,
    pub contract_id: StableId,
    pub side_effect_policy: OperationSideEffectPolicy,
    pub command_refs: Vec<CommandRef>,
    pub effect_contract_refs: Vec<RepoPath>,
    pub commit_allowed: bool,
    pub reasons: Vec<RuntimeEffectStagingReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectStagingStatus {
    Blocked,
    NotStageable,
    NoEffects,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectStagingReason {
    RuntimePlanBlocked,
    RuntimePlanNotReady,
    MissingEffectContractsForMutatingPlan,
    NoCommandsOrEffects,
    StagedCommands,
    StagedEffects,
    CommitRequiresLaterBoundary,
}

#[must_use]
pub fn stage_operation_effects(plan: &RuntimePlan) -> RuntimeEffectStagingPlan {
    let mut reasons = Vec::new();

    let status = if plan.status == RuntimePlanStatus::Blocked {
        reasons.push(RuntimeEffectStagingReason::RuntimePlanBlocked);
        RuntimeEffectStagingStatus::Blocked
    } else if plan.status != RuntimePlanStatus::ReadyToCallOperation {
        reasons.push(RuntimeEffectStagingReason::RuntimePlanNotReady);
        RuntimeEffectStagingStatus::NotStageable
    } else if mutating_side_effect(plan.side_effect_policy) && plan.effect_contract_refs.is_empty()
    {
        reasons.push(RuntimeEffectStagingReason::MissingEffectContractsForMutatingPlan);
        RuntimeEffectStagingStatus::Blocked
    } else if plan.command_refs.is_empty() && plan.effect_contract_refs.is_empty() {
        reasons.push(RuntimeEffectStagingReason::NoCommandsOrEffects);
        RuntimeEffectStagingStatus::NoEffects
    } else {
        if !plan.command_refs.is_empty() {
            reasons.push(RuntimeEffectStagingReason::StagedCommands);
        }
        if !plan.effect_contract_refs.is_empty() {
            reasons.push(RuntimeEffectStagingReason::StagedEffects);
        }
        reasons.push(RuntimeEffectStagingReason::CommitRequiresLaterBoundary);
        RuntimeEffectStagingStatus::Staged
    };

    RuntimeEffectStagingPlan {
        status,
        contract_id: plan.contract_id.clone(),
        side_effect_policy: plan.side_effect_policy,
        command_refs: plan.command_refs.clone(),
        effect_contract_refs: plan.effect_contract_refs.clone(),
        commit_allowed: false,
        reasons,
    }
}
