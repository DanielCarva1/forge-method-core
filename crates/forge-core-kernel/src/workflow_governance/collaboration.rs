//! Read-only Collaboration Plan projection. Observation and stability checks stay in the adapter.

use super::adapter::{
    linked_claim_liveness_gap_is_blocking, ReplacementClaimLiveness, ReplacementClaimProjection,
    WorkflowReplacementContinuity, WorkflowReplacementPromotionStatus,
};
use forge_core_contracts::{
    IsolationStatus, StableId, WorkflowCollaborationPlan,
    WorkflowCurrentWorkCollaborationClaimState, WorkflowCurrentWorkCollaborationDetail,
    WorkflowCurrentWorkCollaborationIsolationValidation,
    WorkflowCurrentWorkCollaborationLaneDetail, WorkflowCurrentWorkCollaborationLaneState,
    WorkflowCurrentWorkCollaborationLaneSummary, WorkflowCurrentWorkCollaborationOwnerDetail,
    WorkflowCurrentWorkCollaborationPromotionState, WorkflowCurrentWorkCollaborationSummary,
};
use std::collections::BTreeSet;

pub(super) fn current_work_collaboration_summary(
    plan: &WorkflowCollaborationPlan,
    continuity: &WorkflowReplacementContinuity,
) -> WorkflowCurrentWorkCollaborationSummary {
    let owners = current_work_collaboration_owner_sets(continuity);
    current_work_collaboration_summary_from_owner_sets(
        plan,
        &owners.integrated,
        &owners.active,
        &owners.blocked,
    )
}

struct CurrentWorkCollaborationOwnerSets {
    integrated: BTreeSet<StableId>,
    active: BTreeSet<StableId>,
    blocked: BTreeSet<StableId>,
}

impl CurrentWorkCollaborationOwnerSets {
    fn from_observations<'a>(
        isolations: impl Iterator<Item = (&'a StableId, IsolationStatus)>,
        promotions: impl Iterator<Item = (&'a StableId, WorkflowReplacementPromotionStatus)>,
        blocking_gaps: impl Iterator<Item = &'a StableId>,
    ) -> Self {
        let mut owners = Self {
            integrated: BTreeSet::new(),
            active: BTreeSet::new(),
            blocked: blocking_gaps.cloned().collect(),
        };
        for (id, status) in isolations {
            match status {
                IsolationStatus::Active | IsolationStatus::Merging => {
                    owners.active.insert(id.clone());
                }
                IsolationStatus::Merged | IsolationStatus::Abandoned => {
                    owners.blocked.insert(id.clone());
                }
                _ => {}
            }
        }
        for (id, status) in promotions {
            match status {
                WorkflowReplacementPromotionStatus::Completed => {
                    owners.integrated.insert(id.clone());
                }
                WorkflowReplacementPromotionStatus::Recoverable
                | WorkflowReplacementPromotionStatus::BlockedCorrupt => {
                    owners.blocked.insert(id.clone());
                }
                WorkflowReplacementPromotionStatus::NotStarted => {}
            }
        }
        owners
    }
}

fn current_work_collaboration_owner_sets(
    continuity: &WorkflowReplacementContinuity,
) -> CurrentWorkCollaborationOwnerSets {
    CurrentWorkCollaborationOwnerSets::from_observations(
        continuity
            .isolations
            .iter()
            .map(|i| (&i.contract.id, i.contract.status)),
        continuity
            .promotions
            .iter()
            .map(|p| (&p.isolation_id, p.status)),
        continuity
            .gaps
            .iter()
            .filter(|g| g.blocking)
            .filter_map(|g| g.isolation_id.as_ref()),
    )
}

pub(super) fn current_work_collaboration_summary_from_owner_sets(
    plan: &WorkflowCollaborationPlan,
    integrated_isolations: &BTreeSet<StableId>,
    active_isolations: &BTreeSet<StableId>,
    blocked_isolations: &BTreeSet<StableId>,
) -> WorkflowCurrentWorkCollaborationSummary {
    let integrated_lanes = plan
        .lanes
        .iter()
        .filter(|lane| {
            lane.isolation_id
                .as_ref()
                .is_some_and(|isolation_id| integrated_isolations.contains(isolation_id))
        })
        .map(|lane| lane.lane_id.clone())
        .collect::<BTreeSet<_>>();

    let mut ready_lane_count = 0usize;
    let mut active_lane_count = 0usize;
    let mut blocked_lane_count = 0usize;
    let mut integrated_lane_count = 0usize;
    let mut next_ready_lane = None;

    for lane in &plan.lanes {
        match current_work_collaboration_lane_state(
            lane,
            &integrated_lanes,
            active_isolations,
            blocked_isolations,
        ) {
            WorkflowCurrentWorkCollaborationLaneState::Ready => {
                ready_lane_count += 1;
                if next_ready_lane.is_none() {
                    next_ready_lane = Some(WorkflowCurrentWorkCollaborationLaneSummary {
                        lane_id: lane.lane_id.clone(),
                        outcome: lane.outcome.clone(),
                        isolation_id: lane.isolation_id.clone(),
                    });
                }
            }
            WorkflowCurrentWorkCollaborationLaneState::Active => active_lane_count += 1,
            WorkflowCurrentWorkCollaborationLaneState::Blocked => blocked_lane_count += 1,
            WorkflowCurrentWorkCollaborationLaneState::Integrated => integrated_lane_count += 1,
        }
    }

    WorkflowCurrentWorkCollaborationSummary {
        lane_count: plan.lanes.len(),
        ready_lane_count,
        active_lane_count,
        blocked_lane_count,
        integrated_lane_count,
        next_ready_lane,
    }
}

fn current_work_collaboration_lane_state(
    lane: &forge_core_contracts::WorkflowCollaborationLane,
    integrated_lanes: &BTreeSet<StableId>,
    active_isolations: &BTreeSet<StableId>,
    blocked_isolations: &BTreeSet<StableId>,
) -> WorkflowCurrentWorkCollaborationLaneState {
    if integrated_lanes.contains(&lane.lane_id) {
        WorkflowCurrentWorkCollaborationLaneState::Integrated
    } else if !lane
        .depends_on
        .iter()
        .all(|dependency| integrated_lanes.contains(dependency))
        || lane
            .isolation_id
            .as_ref()
            .is_some_and(|isolation_id| blocked_isolations.contains(isolation_id))
    {
        WorkflowCurrentWorkCollaborationLaneState::Blocked
    } else if lane
        .isolation_id
        .as_ref()
        .is_some_and(|isolation_id| active_isolations.contains(isolation_id))
    {
        WorkflowCurrentWorkCollaborationLaneState::Active
    } else {
        WorkflowCurrentWorkCollaborationLaneState::Ready
    }
}

pub(super) fn current_work_collaboration_detail_from_existing_state(
    plan: &WorkflowCollaborationPlan,
    claims: &[ReplacementClaimProjection],
    workspace: &super::promotion::ReplacementWorkspaceInspection,
) -> WorkflowCurrentWorkCollaborationDetail {
    let CurrentWorkCollaborationOwnerSets {
        integrated: integrated_isolations,
        active: active_isolations,
        blocked: mut blocked_isolations,
    } = CurrentWorkCollaborationOwnerSets::from_observations(
        workspace
            .isolations
            .iter()
            .map(|i| (&i.contract.id, i.contract.status)),
        workspace.promotions.iter().map(|p| {
            (
                &p.isolation_id,
                match p.status {
                    super::promotion::ReplacementPromotionStatus::NotStarted => {
                        WorkflowReplacementPromotionStatus::NotStarted
                    }
                    super::promotion::ReplacementPromotionStatus::Recoverable => {
                        WorkflowReplacementPromotionStatus::Recoverable
                    }
                    super::promotion::ReplacementPromotionStatus::Completed => {
                        WorkflowReplacementPromotionStatus::Completed
                    }
                    super::promotion::ReplacementPromotionStatus::BlockedCorrupt => {
                        WorkflowReplacementPromotionStatus::BlockedCorrupt
                    }
                },
            )
        }),
        workspace
            .gaps
            .iter()
            .filter(|g| g.blocking)
            .filter_map(|g| g.isolation_id.as_ref()),
    );
    for isolation in &workspace.isolations {
        if matches!(
            isolation.contract.status,
            IsolationStatus::Merged | IsolationStatus::Abandoned
        ) {
            continue;
        }
        let Some(claim_id) = isolation.contract.claim_id.as_ref() else {
            continue;
        };
        let claim = claims.iter().find(|claim| claim.claim.id.0 == claim_id.0);
        let claim_blocks = match claim {
            None => true,
            Some(claim) if claim.claim.claim.claimant_agent_id != isolation.contract.agent_id => {
                true
            }
            Some(claim) if claim.liveness == ReplacementClaimLiveness::Live => false,
            Some(_) => linked_claim_liveness_gap_is_blocking(
                isolation.contract.status,
                workspace
                    .promotions
                    .iter()
                    .find(|promotion| promotion.isolation_id == isolation.contract.id)
                    .map(|promotion| promotion.status),
            ),
        };
        if claim_blocks {
            blocked_isolations.insert(isolation.contract.id.clone());
        }
    }
    let integrated_lanes = plan
        .lanes
        .iter()
        .filter(|lane| {
            lane.isolation_id
                .as_ref()
                .is_some_and(|id| integrated_isolations.contains(id))
        })
        .map(|lane| lane.lane_id.clone())
        .collect::<BTreeSet<_>>();
    let lanes = plan
        .lanes
        .iter()
        .map(|lane| {
            let isolation = lane.isolation_id.as_ref().and_then(|id| {
                workspace
                    .isolations
                    .iter()
                    .find(|isolation| isolation.contract.id == *id)
            });
            let promotion = lane.isolation_id.as_ref().and_then(|id| {
                workspace
                    .promotions
                    .iter()
                    .find(|promotion| promotion.isolation_id == *id)
            });
            WorkflowCurrentWorkCollaborationLaneDetail {
                lane_id: lane.lane_id.clone(),
                state: current_work_collaboration_lane_state(
                    lane,
                    &integrated_lanes,
                    &active_isolations,
                    &blocked_isolations,
                ),
                owner: isolation.map(|isolation| {
                    let claim_state = isolation.contract.claim_id.as_ref().map(|claim_id| {
                        claims
                            .iter()
                            .find(|claim| claim.claim.id.0 == claim_id.0)
                            .map_or(
                                WorkflowCurrentWorkCollaborationClaimState::Missing,
                                |claim| match claim.liveness {
                                    ReplacementClaimLiveness::Live => {
                                        WorkflowCurrentWorkCollaborationClaimState::Live
                                    }
                                    ReplacementClaimLiveness::Expired => {
                                        WorkflowCurrentWorkCollaborationClaimState::Expired
                                    }
                                    ReplacementClaimLiveness::NonActive => {
                                        WorkflowCurrentWorkCollaborationClaimState::NonActive
                                    }
                                },
                            )
                    });
                    WorkflowCurrentWorkCollaborationOwnerDetail {
                        isolation_id: isolation.contract.id.clone(),
                        agent_id: isolation.contract.agent_id.clone(),
                        branch_name: isolation.contract.branch_name.clone(),
                        worktree_path: isolation.contract.worktree_path.clone(),
                        isolation_status: isolation.contract.status,
                        isolation_validation: match isolation.validation {
                            super::promotion::ReplacementIsolationValidation::Valid => {
                                WorkflowCurrentWorkCollaborationIsolationValidation::Valid
                            }
                            super::promotion::ReplacementIsolationValidation::ProposedNotCreated => {
                                WorkflowCurrentWorkCollaborationIsolationValidation::ProposedNotCreated
                            }
                            super::promotion::ReplacementIsolationValidation::RetiredWorktreeAbsent => {
                                WorkflowCurrentWorkCollaborationIsolationValidation::RetiredWorktreeAbsent
                            }
                            super::promotion::ReplacementIsolationValidation::Missing => {
                                WorkflowCurrentWorkCollaborationIsolationValidation::Missing
                            }
                            super::promotion::ReplacementIsolationValidation::Mismatched => {
                                WorkflowCurrentWorkCollaborationIsolationValidation::Mismatched
                            }
                        },
                        claim_id: isolation.contract.claim_id.clone(),
                        claim_state,
                    }
                }),
                promotion_status: promotion.map(|promotion| match promotion.status {
                    super::promotion::ReplacementPromotionStatus::NotStarted => {
                        WorkflowCurrentWorkCollaborationPromotionState::NotStarted
                    }
                    super::promotion::ReplacementPromotionStatus::Recoverable => {
                        WorkflowCurrentWorkCollaborationPromotionState::Recoverable
                    }
                    super::promotion::ReplacementPromotionStatus::Completed => {
                        WorkflowCurrentWorkCollaborationPromotionState::Completed
                    }
                    super::promotion::ReplacementPromotionStatus::BlockedCorrupt => {
                        WorkflowCurrentWorkCollaborationPromotionState::BlockedCorrupt
                    }
                }),
                promotion_receipt_digest: promotion.and_then(|promotion| {
                    (promotion.status == super::promotion::ReplacementPromotionStatus::Completed)
                        .then(|| promotion.receipt_digest.clone())
                        .flatten()
                }),
            }
        })
        .collect();
    WorkflowCurrentWorkCollaborationDetail {
        plan: plan.clone(),
        lanes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collaboration_owner_classification_preserves_overlapping_facts() {
        let active = StableId("active".to_owned());
        let merged = StableId("merged".to_owned());
        let abandoned = StableId("abandoned".to_owned());
        let corrupt = StableId("corrupt".to_owned());
        let owners = CurrentWorkCollaborationOwnerSets::from_observations(
            [
                (&active, IsolationStatus::Merging),
                (&merged, IsolationStatus::Merged),
                (&abandoned, IsolationStatus::Abandoned),
            ]
            .into_iter(),
            [
                (&active, WorkflowReplacementPromotionStatus::Completed),
                (&merged, WorkflowReplacementPromotionStatus::Recoverable),
                (&corrupt, WorkflowReplacementPromotionStatus::BlockedCorrupt),
            ]
            .into_iter(),
            [&active, &active].into_iter(),
        );
        assert_eq!(owners.integrated, BTreeSet::from([active.clone()]));
        assert_eq!(owners.active, BTreeSet::from([active.clone()]));
        assert_eq!(
            owners.blocked,
            BTreeSet::from([active, merged, abandoned, corrupt])
        );
    }
}
