use crate::common::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A machine-readable eval-run outcome contract document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalRunContractDocument {
    pub schema_version: String,
    pub eval_run_contract: EvalRunContract,
}

/// Outcome evidence for one model/router attempt on one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalRunContract {
    pub run_id: StableId,
    pub task_id: StableId,
    pub model_ref: String,
    pub router_decision: Option<String>,
    pub outcome: EvalOutcome,
    pub cost: EvalCost,
    pub quality_signals: QualitySignals,
    pub evidence_refs: Vec<String>,
}

impl EvalRunContract {
    /// Returns true when this run reached the strict passing verdict.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.outcome.value, EvalVerdict::Passed)
    }

    /// Returns the run cost when the run succeeded.
    ///
    /// The value is represented as USD micros (`USD * 1_000_000`) so downstream
    /// ROI math can stay integer and canonical on disk.
    #[must_use]
    pub fn cost_per_success_micros(&self) -> Option<u64> {
        self.is_success()
            .then_some(self.cost.estimated_cost_usd_micros)
    }

    /// Returns tool-calls-per-1k-tokens for successful runs.
    ///
    /// This is `num_tool_calls * 1000 / total_tokens`. Failed runs and zero-token
    /// runs return `None` because they are not comparable efficiency samples.
    #[must_use]
    pub fn token_efficiency(&self) -> Option<u64> {
        if self.is_success() && self.cost.total_tokens > 0 {
            Some(u64::from(self.cost.num_tool_calls) * 1_000 / self.cost.total_tokens)
        } else {
            None
        }
    }
}

/// Verdict and failure metadata produced by an eval run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalOutcome {
    pub value: EvalVerdict,
    pub evaluated_at: String,
    pub failure_cluster: Option<EvalFailureCluster>,
    pub notes: Option<String>,
}

/// Top-level eval verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvalVerdict {
    Passed,
    Failed,
    Partial,
    Flaky,
    Error,
}

/// Stable failure buckets used by the eval bank and risk-router calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvalFailureCluster {
    BuildFailure,
    TestFailure,
    LintFailure,
    Timeout,
    WrongLocation,
    OverfitPatch,
    SemanticMismatch,
    ToolError,
    None,
}

/// Token, cost, wall-time, and loop-shape measurements for one eval run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalCost {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd_micros: u64,
    pub wall_time_ms: u64,
    pub num_tool_calls: u32,
    pub num_turns: u32,
}

/// Quality signals that separate "tests passed" from "credible fix".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualitySignals {
    pub correct_location: Option<bool>,
    pub semantic_correct: Option<bool>,
    pub overfit_suspected: Option<bool>,
    pub confidence: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing_document() -> EvalRunContractDocument {
        EvalRunContractDocument {
            schema_version: "0.1".to_string(),
            eval_run_contract: EvalRunContract {
                run_id: StableId("eval.run.story-123.gpt55.001".to_string()),
                task_id: StableId("story-123".to_string()),
                model_ref: "openai-codex/gpt-5.5".to_string(),
                router_decision: Some("fast_lane:low_blast_radius:test_backed".to_string()),
                outcome: EvalOutcome {
                    value: EvalVerdict::Passed,
                    evaluated_at: "2026-06-27T00:00:00Z".to_string(),
                    failure_cluster: None,
                    notes: Some("cargo test and clippy passed".to_string()),
                },
                cost: EvalCost {
                    prompt_tokens: 8_000,
                    completion_tokens: 2_000,
                    total_tokens: 10_000,
                    estimated_cost_usd_micros: 125_000,
                    wall_time_ms: 95_000,
                    num_tool_calls: 12,
                    num_turns: 3,
                },
                quality_signals: QualitySignals {
                    correct_location: Some(true),
                    semantic_correct: Some(true),
                    overfit_suspected: Some(false),
                    confidence: Some(91),
                },
                evidence_refs: vec![
                    "contracts/evidence/story-123-eval.yaml".to_string(),
                    "target/eval/story-123/cargo-test.txt".to_string(),
                ],
            },
        }
    }

    fn failing_contract() -> EvalRunContract {
        let mut contract = passing_document().eval_run_contract;
        contract.outcome = EvalOutcome {
            value: EvalVerdict::Failed,
            evaluated_at: "2026-06-27T00:05:00Z".to_string(),
            failure_cluster: Some(EvalFailureCluster::TestFailure),
            notes: Some("unit test regression".to_string()),
        };
        contract.cost.total_tokens = 6_000;
        contract.cost.estimated_cost_usd_micros = 75_000;
        contract.cost.num_tool_calls = 9;
        contract
    }

    #[test]
    fn serde_round_trip_populated_eval_run_contract() {
        let document = passing_document();

        let serialized = serde_yaml::to_string(&document).expect("serializes");
        let deserialized: EvalRunContractDocument =
            serde_yaml::from_str(&serialized).expect("deserializes");

        assert_eq!(deserialized, document);
    }

    #[test]
    fn deny_unknown_fields_rejects_unknown_key() {
        let yaml = r#"
schema_version: "0.1"
eval_run_contract:
  run_id: eval.run.story-123.gpt55.001
  task_id: story-123
  model_ref: openai-codex/gpt-5.5
  router_decision: fast_lane:low_blast_radius:test_backed
  outcome:
    value: passed
    evaluated_at: "2026-06-27T00:00:00Z"
    failure_cluster: null
    notes: ok
  cost:
    prompt_tokens: 8000
    completion_tokens: 2000
    total_tokens: 10000
    estimated_cost_usd_micros: 125000
    wall_time_ms: 95000
    num_tool_calls: 12
    num_turns: 3
  quality_signals:
    correct_location: true
    semantic_correct: true
    overfit_suspected: false
    confidence: 91
  evidence_refs: []
  unexpected_key: nope
"#;

        let result = serde_yaml::from_str::<EvalRunContractDocument>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn passing_run_reports_success_cost_and_token_efficiency() {
        let contract = passing_document().eval_run_contract;

        assert!(contract.is_success());
        assert_eq!(contract.cost_per_success_micros(), Some(125_000));
        assert_eq!(contract.token_efficiency(), Some(1));
    }

    #[test]
    fn failing_run_has_no_success_cost_or_token_efficiency() {
        let contract = failing_contract();

        assert!(!contract.is_success());
        assert_eq!(contract.cost_per_success_micros(), None);
        assert_eq!(contract.token_efficiency(), None);
    }

    #[test]
    fn zero_token_passing_run_has_no_token_efficiency() {
        let mut contract = passing_document().eval_run_contract;
        contract.cost.total_tokens = 0;

        assert!(contract.is_success());
        assert_eq!(contract.cost_per_success_micros(), Some(125_000));
        assert_eq!(contract.token_efficiency(), None);
    }
}
