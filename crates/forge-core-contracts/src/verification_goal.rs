use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A machine-readable verification-goal contract document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationGoalContractDocument {
    pub schema_version: String,
    pub verification_goal_contract: VerificationGoalContract,
}

/// Explicit, machine-checkable completion criteria for an agent loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationGoalContract {
    pub id: StableId,
    pub scope: GoalScope,
    pub goals: Vec<VerificationGoal>,
    pub overall: OverallVerdict,
    pub evidence_refs: Vec<String>,
}

impl VerificationGoalContract {
    /// Returns true when the aggregate verdict and every goal agree that all
    /// declared verification goals passed.
    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        matches!(self.overall.value, OverallVerdictValue::AllSatisfied)
            && !self.goals.is_empty()
            && self.overall.satisfied <= self.overall.total
            && self.overall.satisfied == self.goals.len()
            && self.overall.total == self.goals.len()
            && self
                .goals
                .iter()
                .all(|goal| matches!(goal.status, GoalStatus::Passed))
    }

    /// Returns the subset of verification goals that failed.
    pub fn failed_goals(&self) -> impl Iterator<Item = &VerificationGoal> {
        self.goals
            .iter()
            .filter(|goal| matches!(goal.status, GoalStatus::Failed))
    }
}

/// Repository and story scope for the verification goal set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalScope {
    pub phase: String,
    pub story_ref: Option<StableId>,
    pub changed_paths: Vec<RepoPath>,
}

/// One machine-checkable verification target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationGoal {
    pub kind: GoalKind,
    pub target: String,
    pub status: GoalStatus,
    pub adapter: AdapterKind,
    pub detail: Option<String>,
}

/// Aggregate verdict across all verification goals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverallVerdict {
    pub value: OverallVerdictValue,
    pub evaluated_at: String,
    pub satisfied: usize,
    pub total: usize,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalKind {
    UnitTestsPass,
    LintClean,
    TypecheckPass,
    BuildPass,
    CiGreen,
    CustomCommand,
    CoverageThreshold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Flaky,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Cargo,
    Pytest,
    Jest,
    GoTest,
    Shell,
    Ci,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverallVerdictValue {
    AllSatisfied,
    PartiallySatisfied,
    NotSatisfied,
    NotEvaluated,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_contract() -> VerificationGoalContractDocument {
        VerificationGoalContractDocument {
            schema_version: "0.1".to_string(),
            verification_goal_contract: VerificationGoalContract {
                id: StableId("verification.goal.story-123".to_string()),
                scope: GoalScope {
                    phase: "4-build-verify".to_string(),
                    story_ref: Some(StableId("story-123".to_string())),
                    changed_paths: vec![
                        RepoPath("crates/forge-core-store/src/lib.rs".to_string()),
                        RepoPath("crates/forge-core-store/tests/wal_recovery.rs".to_string()),
                    ],
                },
                goals: vec![
                    VerificationGoal {
                        kind: GoalKind::UnitTestsPass,
                        target: "cargo test -p forge-core-store".to_string(),
                        status: GoalStatus::Passed,
                        adapter: AdapterKind::Cargo,
                        detail: Some("80 tests passed".to_string()),
                    },
                    VerificationGoal {
                        kind: GoalKind::LintClean,
                        target: "cargo clippy -p forge-core-store --all-targets".to_string(),
                        status: GoalStatus::Passed,
                        adapter: AdapterKind::Cargo,
                        detail: None,
                    },
                    VerificationGoal {
                        kind: GoalKind::CiGreen,
                        target: "CI check #123".to_string(),
                        status: GoalStatus::Pending,
                        adapter: AdapterKind::Ci,
                        detail: Some("queued".to_string()),
                    },
                ],
                overall: OverallVerdict {
                    value: OverallVerdictValue::PartiallySatisfied,
                    evaluated_at: "2026-06-27T00:00:00Z".to_string(),
                    satisfied: 2,
                    total: 3,
                    notes: Some("CI still pending".to_string()),
                },
                evidence_refs: vec!["contracts/evidence/story-123-verification.yaml".to_string()],
            },
        }
    }

    #[test]
    fn serde_round_trip_populated_multi_goal_contract() {
        let document = populated_contract();

        let serialized = yaml_serde::to_string(&document).expect("serializes");
        let deserialized: VerificationGoalContractDocument =
            yaml_serde::from_str(&serialized).expect("deserializes");

        assert_eq!(deserialized, document);
    }

    #[test]
    fn deny_unknown_fields_rejects_unknown_key() {
        let yaml = r#"
schema_version: "0.1"
verification_goal_contract:
  id: verification.goal.story-123
  unexpected_key: nope
  scope:
    phase: 4-build-verify
    story_ref: story-123
    changed_paths: []
  goals: []
  overall:
    value: not_evaluated
    evaluated_at: "2026-06-27T00:00:00Z"
    satisfied: 0
    total: 0
    notes: null
  evidence_refs: []
"#;

        let result = yaml_serde::from_str::<VerificationGoalContractDocument>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn is_satisfied_true_when_all_satisfied() {
        let mut document = populated_contract();
        document.verification_goal_contract.overall.value = OverallVerdictValue::AllSatisfied;
        document.verification_goal_contract.overall.satisfied = 3;
        document
            .verification_goal_contract
            .goals
            .iter_mut()
            .for_each(|goal| goal.status = GoalStatus::Passed);

        assert!(document.verification_goal_contract.is_satisfied());
    }

    #[test]
    fn is_satisfied_false_when_all_satisfied_has_no_goals() {
        let mut document = populated_contract();
        document.verification_goal_contract.goals.clear();
        document.verification_goal_contract.overall.value = OverallVerdictValue::AllSatisfied;
        document.verification_goal_contract.overall.satisfied = 0;
        document.verification_goal_contract.overall.total = 0;

        assert!(!document.verification_goal_contract.is_satisfied());
    }

    #[test]
    fn is_satisfied_false_when_all_satisfied_contains_failed_pending_or_flaky_goal() {
        for status in [
            GoalStatus::Failed,
            GoalStatus::Pending,
            GoalStatus::Running,
            GoalStatus::Flaky,
            GoalStatus::Skipped,
        ] {
            let mut document = populated_contract();
            document.verification_goal_contract.overall.value = OverallVerdictValue::AllSatisfied;
            document.verification_goal_contract.overall.satisfied = 3;
            document.verification_goal_contract.overall.total = 3;
            document
                .verification_goal_contract
                .goals
                .iter_mut()
                .for_each(|goal| goal.status = GoalStatus::Passed);
            document.verification_goal_contract.goals[1].status = status;

            assert!(
                !document.verification_goal_contract.is_satisfied(),
                "status {status:?} must not satisfy strict verification"
            );
        }
    }

    #[test]
    fn is_satisfied_false_when_all_satisfied_counts_are_inconsistent() {
        let inconsistent_counts = [(2, 3), (4, 3), (3, 4), (4, 4)];

        for (satisfied, total) in inconsistent_counts {
            let mut document = populated_contract();
            document.verification_goal_contract.overall.value = OverallVerdictValue::AllSatisfied;
            document.verification_goal_contract.overall.satisfied = satisfied;
            document.verification_goal_contract.overall.total = total;
            document
                .verification_goal_contract
                .goals
                .iter_mut()
                .for_each(|goal| goal.status = GoalStatus::Passed);

            assert!(
                !document.verification_goal_contract.is_satisfied(),
                "counts satisfied={satisfied} total={total} must not satisfy strict verification"
            );
        }
    }

    #[test]
    fn failed_goals_returns_failed_subset() {
        let mut document = populated_contract();
        document
            .verification_goal_contract
            .goals
            .push(VerificationGoal {
                kind: GoalKind::CustomCommand,
                target: "./scripts/e2e.sh".to_string(),
                status: GoalStatus::Failed,
                adapter: AdapterKind::Shell,
                detail: Some("exit code 1".to_string()),
            });

        let failed_targets = document
            .verification_goal_contract
            .failed_goals()
            .map(|goal| goal.target.as_str())
            .collect::<Vec<_>>();

        assert_eq!(failed_targets, vec!["./scripts/e2e.sh"]);
    }

    #[test]
    fn flaky_goal_does_not_count_as_satisfied() {
        let document = VerificationGoalContractDocument {
            schema_version: "0.1".to_string(),
            verification_goal_contract: VerificationGoalContract {
                id: StableId("verification.goal.flaky".to_string()),
                scope: GoalScope {
                    phase: "4-build-verify".to_string(),
                    story_ref: None,
                    changed_paths: vec![],
                },
                goals: vec![
                    VerificationGoal {
                        kind: GoalKind::UnitTestsPass,
                        target: "cargo test -p forge-core-kernel".to_string(),
                        status: GoalStatus::Passed,
                        adapter: AdapterKind::Cargo,
                        detail: None,
                    },
                    VerificationGoal {
                        kind: GoalKind::UnitTestsPass,
                        target: "cargo test -p forge-core-kernel --test operation_plan"
                            .to_string(),
                        status: GoalStatus::Flaky,
                        adapter: AdapterKind::Cargo,
                        detail: Some("passed on retry".to_string()),
                    },
                ],
                overall: OverallVerdict {
                    value: OverallVerdictValue::PartiallySatisfied,
                    evaluated_at: "2026-06-27T00:00:00Z".to_string(),
                    satisfied: 1,
                    total: 2,
                    notes: Some(
                        "flaky goal requires review before fast-lane acceptance".to_string(),
                    ),
                },
                evidence_refs: vec![],
            },
        };

        assert_eq!(document.verification_goal_contract.overall.satisfied, 1);
        assert!(!document.verification_goal_contract.is_satisfied());
    }
}
