//! Autonomy-lane routing.
//!
//! The router is the engine-side half of the dual-lane autonomy model: it maps a
//! proposed agent loop to either the fast lane (machine-checkable evidence,
//! near-silent execution) or the rigorous lane (human contact / hard gates). The
//! function is deterministic and intentionally fail-closed: manual policies,
//! exhausted retries, missing evidence, or high-risk failed verification all
//! route to [`LaneKind::Rigorous`].

use forge_core_contracts::autonomy_policy::{AutonomyMode, AutonomyPolicyContract};
use forge_core_contracts::verification_goal::VerificationGoalContract;

const HIGH_RISK_THRESHOLD: u8 = 70;

/// The lane the engine selected for the proposed work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    /// Machine-checkable evidence exists and the policy allows low-friction work.
    Fast,
    /// Human contact, hard gates, or explicit review are required.
    Rigorous,
}

/// Typed reason explaining why a lane was selected.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneRouteReason {
    /// The work has machine-checkable evidence and no fail-closed trigger fired.
    LowRiskVerified,
    /// Reserved for future policy reasons that explicitly force human review.
    HighRiskNeedsHuman,
    /// Fast lane is disallowed because no machine-checkable verification goal exists.
    NoVerificationGoal,
    /// The policy is explicitly manual.
    ManualMode,
    /// Failed verification touched a high-risk, non-YOLO tool class.
    HighRiskToolClass(u8),
    /// The agent exhausted retry budget and should escalate.
    RepeatedFailures(u8),
}

/// The router's lane verdict.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LaneDecision {
    pub lane: LaneKind,
    pub reason: LaneRouteReason,
}

/// Route a proposed change to the fast or rigorous lane.
///
/// Deterministic precedence:
/// 1. manual policy always routes rigorous;
/// 2. exhausted retries route rigorous;
/// 3. missing verification goal routes rigorous;
/// 4. failed verification plus a high-risk non-YOLO tool class routes rigorous;
/// 5. otherwise, the work can proceed on the fast lane.
#[must_use]
pub fn route_lane(
    policy: &AutonomyPolicyContract,
    goal: Option<&VerificationGoalContract>,
    failure_streak: u8,
) -> LaneDecision {
    if policy.default_mode == AutonomyMode::Manual {
        return rigorous(LaneRouteReason::ManualMode);
    }

    if failure_streak >= policy.escalation.max_retries_before_human {
        return rigorous(LaneRouteReason::RepeatedFailures(failure_streak));
    }

    let Some(goal) = goal else {
        return rigorous(LaneRouteReason::NoVerificationGoal);
    };

    if !goal.is_satisfied() && goal.failed_goals().next().is_some() {
        if let Some(risk) =
            highest_risk_non_yolo_class(policy).filter(|risk| *risk >= HIGH_RISK_THRESHOLD)
        {
            return rigorous(LaneRouteReason::HighRiskToolClass(risk));
        }
    }

    fast(LaneRouteReason::LowRiskVerified)
}

#[must_use]
fn fast(reason: LaneRouteReason) -> LaneDecision {
    LaneDecision {
        lane: LaneKind::Fast,
        reason,
    }
}

#[must_use]
fn rigorous(reason: LaneRouteReason) -> LaneDecision {
    LaneDecision {
        lane: LaneKind::Rigorous,
        reason,
    }
}

#[must_use]
fn highest_risk_non_yolo_class(policy: &AutonomyPolicyContract) -> Option<u8> {
    policy
        .tool_classes
        .iter()
        .filter(|class_policy| class_policy.mode != AutonomyMode::Yolo)
        .filter_map(|class_policy| class_policy.risk_score)
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::autonomy_policy::{
        EscalationPolicy, PolicyScope, PolicyScopeKind, ToolClass, ToolClassPolicy,
    };
    use forge_core_contracts::common::{RepoPath, StableId};
    use forge_core_contracts::verification_goal::{
        AdapterKind, GoalKind, GoalScope, GoalStatus, OverallVerdict, OverallVerdictValue,
        VerificationGoal,
    };

    fn sid(value: &str) -> StableId {
        StableId(value.to_owned())
    }

    fn policy(
        default_mode: AutonomyMode,
        tool_classes: Vec<ToolClassPolicy>,
    ) -> AutonomyPolicyContract {
        AutonomyPolicyContract {
            id: sid("autonomy.policy.test"),
            applies_to: PolicyScope {
                kind: PolicyScopeKind::Lane,
                ids: vec![sid("fast_lane")],
            },
            default_mode,
            tool_classes,
            escalation: EscalationPolicy {
                on_repeated_failure: 2,
                on_high_risk_path: true,
                on_semantic_uncertainty: true,
                max_retries_before_human: 3,
                cooldown_seconds: 60,
            },
            evidence_basis: None,
        }
    }

    fn tool_class(mode: AutonomyMode, risk_score: u8) -> ToolClassPolicy {
        ToolClassPolicy {
            class: ToolClass::FileEdit,
            mode,
            risk_score: Some(risk_score),
            requires_approval_above: Some(HIGH_RISK_THRESHOLD),
        }
    }

    fn goal(status: GoalStatus, verdict: OverallVerdictValue) -> VerificationGoalContract {
        VerificationGoalContract {
            id: sid("verification.goal.test"),
            scope: GoalScope {
                phase: "4-build-verify".to_owned(),
                story_ref: Some(sid("story-test")),
                changed_paths: vec![RepoPath(
                    "crates/forge-core-engine/src/autonomy_router.rs".to_owned(),
                )],
            },
            goals: vec![VerificationGoal {
                kind: GoalKind::UnitTestsPass,
                target: "cargo test -p forge-core-engine".to_owned(),
                status,
                adapter: AdapterKind::Cargo,
                detail: None,
            }],
            overall: OverallVerdict {
                value: verdict,
                evaluated_at: "2026-06-27T00:00:00Z".to_owned(),
                satisfied: usize::from(status == GoalStatus::Passed),
                total: 1,
                notes: None,
            },
            evidence_refs: vec!["contracts/evidence/autonomy-router.yaml".to_owned()],
        }
    }

    #[test]
    fn manual_policy_routes_to_rigorous_manual_mode() {
        let policy = policy(
            AutonomyMode::Manual,
            vec![tool_class(AutonomyMode::Yolo, 10)],
        );
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane(&policy, Some(&goal), 0),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::ManualMode,
            }
        );
    }

    #[test]
    fn repeated_failures_route_to_rigorous() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::Yolo, 10)],
        );
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane(&policy, Some(&goal), 3),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::RepeatedFailures(3),
            }
        );
    }

    #[test]
    fn missing_verification_goal_routes_to_rigorous() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::Yolo, 10)],
        );

        assert_eq!(
            route_lane(&policy, None, 0),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::NoVerificationGoal,
            }
        );
    }

    #[test]
    fn satisfied_goal_with_low_risk_routes_to_fast() {
        let policy = policy(
            AutonomyMode::Yolo,
            vec![tool_class(AutonomyMode::ConfidenceThreshold, 30)],
        );
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane(&policy, Some(&goal), 0),
            LaneDecision {
                lane: LaneKind::Fast,
                reason: LaneRouteReason::LowRiskVerified,
            }
        );
    }

    #[test]
    fn failed_goal_with_high_risk_non_yolo_tool_routes_to_rigorous() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![
                tool_class(AutonomyMode::Yolo, 99),
                tool_class(AutonomyMode::Manual, 80),
            ],
        );
        let goal = goal(GoalStatus::Failed, OverallVerdictValue::NotSatisfied);

        assert_eq!(
            route_lane(&policy, Some(&goal), 0),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::HighRiskToolClass(80),
            }
        );
    }

    #[test]
    fn failed_goal_with_low_risk_non_yolo_tool_stays_fast() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::ConfidenceThreshold, 69)],
        );
        let goal = goal(GoalStatus::Failed, OverallVerdictValue::NotSatisfied);

        assert_eq!(
            route_lane(&policy, Some(&goal), 0),
            LaneDecision {
                lane: LaneKind::Fast,
                reason: LaneRouteReason::LowRiskVerified,
            }
        );
    }
}
