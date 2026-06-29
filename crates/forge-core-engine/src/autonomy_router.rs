//! Autonomy-lane routing.
//!
//! The router is the engine-side half of the dual-lane autonomy model: it maps a
//! proposed agent loop to either the fast lane (machine-checkable evidence,
//! near-silent execution) or the rigorous lane (human contact / hard gates). The
//! function is deterministic and intentionally fail-closed: manual policies,
//! exhausted retries, missing evidence, or high-risk path policies all
//! route to [`LaneKind::Rigorous`].

use forge_core_contracts::autonomy_policy::{AutonomyMode, AutonomyPolicyContract, ToolClass};
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
    /// The proposal touched a tool class whose risk score requires approval.
    HighRiskToolClass(u8),
    /// The agent exhausted retry budget and should escalate.
    RepeatedFailures(u8),
}

/// The router's lane verdict.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneDecision {
    pub lane: LaneKind,
    pub reason: LaneRouteReason,
}

/// Route a proposed change to the fast or rigorous lane.
///
/// This backward-compatible entry point has no proposed tool-class parameter, so
/// it treats every tool class declared by the policy as in scope. That is
/// intentionally conservative: an unknown proposal must not bypass manual or
/// high-risk per-tool overrides.
#[must_use]
pub fn route_lane(
    policy: &AutonomyPolicyContract,
    goal: Option<&VerificationGoalContract>,
    failure_streak: u8,
) -> LaneDecision {
    let declared_tool_classes = policy
        .tool_classes
        .iter()
        .map(|class_policy| class_policy.class)
        .collect::<Vec<_>>();

    route_lane_for_tool_classes(policy, goal, failure_streak, &declared_tool_classes)
}

/// Route a proposed change when the caller knows which tool classes are in
/// scope for the proposed operation.
///
/// Deterministic precedence:
/// 1. manual policy or in-scope manual tool override always routes rigorous;
/// 2. exhausted retries route rigorous;
/// 3. missing verification goal routes rigorous;
/// 4. non-satisfied verification (any pending/flaky/incomplete state, not only
///    explicit failure) routes rigorous - the fast lane requires ALL evidence
///    to be satisfied (fail-closed); an in-scope high-risk tool class surfaces a
///    specific risk reason;
/// 5. high-risk path escalation routes rigorous even when verification passed;
/// 6. otherwise, the work can proceed on the fast lane.
#[must_use]
pub fn route_lane_for_tool_classes(
    policy: &AutonomyPolicyContract,
    goal: Option<&VerificationGoalContract>,
    failure_streak: u8,
    tool_classes: &[ToolClass],
) -> LaneDecision {
    if policy.default_mode == AutonomyMode::Manual {
        return rigorous(LaneRouteReason::ManualMode);
    }

    if has_manual_tool_override(policy, tool_classes) {
        return rigorous(LaneRouteReason::ManualMode);
    }

    if failure_streak >= policy.escalation.max_retries_before_human {
        return rigorous(LaneRouteReason::RepeatedFailures(failure_streak));
    }

    let Some(goal) = goal else {
        return rigorous(LaneRouteReason::NoVerificationGoal);
    };

    // Fail-closed: the fast lane REQUIRES machine-checkable evidence to be
    // fully satisfied. A goal that is pending/running/flaky/skipped (not failed,
    // not satisfied) must NOT unlock the fast lane - incomplete evidence is
    // treated as no evidence (W4-001/W4-002).
    if !goal.is_satisfied() {
        // If a high-risk tool class is in scope AND evidence is not fully
        // satisfied, surface the specific risk so the host can act.
        if let Some(risk) = highest_approval_required_risk(policy, tool_classes) {
            return rigorous(LaneRouteReason::HighRiskToolClass(risk));
        }
        return rigorous(LaneRouteReason::HighRiskNeedsHuman);
    }

    if let Some(risk) = highest_approval_required_risk(policy, tool_classes) {
        return rigorous(LaneRouteReason::HighRiskToolClass(risk));
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
fn has_manual_tool_override(policy: &AutonomyPolicyContract, tool_classes: &[ToolClass]) -> bool {
    tool_classes.iter().any(|tool_class| {
        policy.tool_classes.iter().any(|class_policy| {
            class_policy.class == *tool_class && class_policy.mode == AutonomyMode::Manual
        })
    })
}

#[must_use]
fn highest_approval_required_risk(
    policy: &AutonomyPolicyContract,
    tool_classes: &[ToolClass],
) -> Option<u8> {
    if !policy.escalation.on_high_risk_path {
        return None;
    }

    tool_classes
        .iter()
        .flat_map(|tool_class| {
            policy
                .tool_classes
                .iter()
                .filter(|class_policy| class_policy.class == *tool_class)
        })
        .filter_map(|class_policy| {
            let risk_score = class_policy.risk_score?;
            let approval_threshold = class_policy
                .requires_approval_above
                .unwrap_or(HIGH_RISK_THRESHOLD);
            (risk_score >= approval_threshold).then_some(risk_score)
        })
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
        tool_class_policy(
            ToolClass::FileEdit,
            mode,
            risk_score,
            Some(HIGH_RISK_THRESHOLD),
        )
    }

    fn tool_class_policy(
        class: ToolClass,
        mode: AutonomyMode,
        risk_score: u8,
        requires_approval_above: Option<u8>,
    ) -> ToolClassPolicy {
        ToolClassPolicy {
            class,
            mode,
            risk_score: Some(risk_score),
            requires_approval_above,
        }
    }

    fn set_high_risk_path(policy: &mut AutonomyPolicyContract, enabled: bool) {
        policy.escalation.on_high_risk_path = enabled;
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
    fn failed_goal_with_high_risk_tool_routes_to_rigorous() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::ConfidenceThreshold, 80)],
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

    // W4-001: fail-closed. A non-satisfied goal (failed/pending/flaky) must
    // NEVER unlock the fast lane, regardless of tool-class risk. Previously a
    // failed goal with a low-risk non-YOLO class incorrectly stayed Fast.
    #[test]
    fn failed_goal_routes_rigorous_even_with_low_risk_non_yolo_tool() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::ConfidenceThreshold, 69)],
        );
        let goal = goal(GoalStatus::Failed, OverallVerdictValue::NotSatisfied);

        let decision = route_lane(&policy, Some(&goal), 0);
        assert_eq!(decision.lane, LaneKind::Rigorous);
        // High-risk-class reason only fires at risk >= HIGH_RISK_THRESHOLD (70);
        // with risk 69 the generic human-review reason is used.
        assert_eq!(decision.reason, LaneRouteReason::HighRiskNeedsHuman);
    }

    #[test]
    fn pending_goal_claiming_all_satisfied_routes_rigorous() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class(AutonomyMode::ConfidenceThreshold, 30)],
        );
        let mut goal = goal(GoalStatus::Pending, OverallVerdictValue::AllSatisfied);
        goal.overall.satisfied = 1;

        assert_eq!(
            route_lane(&policy, Some(&goal), 0),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::HighRiskNeedsHuman,
            }
        );
    }

    #[test]
    fn route_lane_is_conservative_for_manual_tool_override_without_tool_scope() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![
                tool_class_policy(ToolClass::FileEdit, AutonomyMode::SandboxAuto, 25, Some(80)),
                tool_class_policy(ToolClass::SecretAccess, AutonomyMode::Manual, 100, Some(1)),
            ],
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
    fn route_lane_for_tool_classes_only_blocks_in_scope_manual_override() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![
                tool_class_policy(ToolClass::FileEdit, AutonomyMode::SandboxAuto, 25, Some(80)),
                tool_class_policy(ToolClass::SecretAccess, AutonomyMode::Manual, 100, Some(1)),
            ],
        );
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane_for_tool_classes(&policy, Some(&goal), 0, &[ToolClass::FileEdit]),
            LaneDecision {
                lane: LaneKind::Fast,
                reason: LaneRouteReason::LowRiskVerified,
            }
        );
        assert_eq!(
            route_lane_for_tool_classes(&policy, Some(&goal), 0, &[ToolClass::SecretAccess]),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::ManualMode,
            }
        );
    }

    #[test]
    fn high_risk_path_uses_per_class_approval_threshold() {
        let policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class_policy(
                ToolClass::CodeExec,
                AutonomyMode::Yolo,
                60,
                Some(50),
            )],
        );
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane_for_tool_classes(&policy, Some(&goal), 0, &[ToolClass::CodeExec]),
            LaneDecision {
                lane: LaneKind::Rigorous,
                reason: LaneRouteReason::HighRiskToolClass(60),
            }
        );
    }

    #[test]
    fn high_risk_path_disabled_allows_satisfied_goal_to_route_fast() {
        let mut policy = policy(
            AutonomyMode::SandboxAuto,
            vec![tool_class_policy(
                ToolClass::CodeExec,
                AutonomyMode::Yolo,
                90,
                Some(50),
            )],
        );
        set_high_risk_path(&mut policy, false);
        let goal = goal(GoalStatus::Passed, OverallVerdictValue::AllSatisfied);

        assert_eq!(
            route_lane_for_tool_classes(&policy, Some(&goal), 0, &[ToolClass::CodeExec]),
            LaneDecision {
                lane: LaneKind::Fast,
                reason: LaneRouteReason::LowRiskVerified,
            }
        );
    }

    #[test]
    fn lane_decision_denies_unknown_fields() {
        let json = r#"{"lane":"fast","reason":"low_risk_verified","extra":"nope"}"#;
        let result = serde_json::from_str::<LaneDecision>(json);

        assert!(result.is_err());
    }
}
