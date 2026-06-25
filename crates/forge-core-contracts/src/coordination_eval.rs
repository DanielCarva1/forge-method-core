use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationEvalContractDocument {
    pub schema_version: String,
    pub coordination_eval_contract: CoordinationEvalContract,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationEvalContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub status: CoordinationEvalStatus,
    pub dimensions: Vec<CoordinationEvalDimension>,
    pub pass_policy: CoordinationEvalPassPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationEvalDimension {
    pub dimension: CoordinationDimension,
    pub metric_kind: CoordinationMetricKind,
    pub required_level: CoordinationRequiredLevel,
    pub fixture_refs: Vec<RepoPath>,
    pub threshold: Option<f64>,
    pub failure_signal: String,
    pub evidence_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationEvalPassPolicy {
    pub required_level: CoordinationRequiredLevel,
    pub all_must_pass_dimensions_required: bool,
    pub manual_review_blocks_release: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationDimension {
    DuplicateWorkPrevention,
    LaneCollisionDetection,
    StaleClaimRecovery,
    ReadWriteConflictRepair,
    RequestLifecycle,
    IntegrationGateScope,
    RuntimeHandoffSafety,
    DestructiveEffectSafety,
    HumanGuidanceRecovery,
}

impl CoordinationDimension {
    pub const ALL: [Self; 9] = [
        Self::DuplicateWorkPrevention,
        Self::LaneCollisionDetection,
        Self::StaleClaimRecovery,
        Self::ReadWriteConflictRepair,
        Self::RequestLifecycle,
        Self::IntegrationGateScope,
        Self::RuntimeHandoffSafety,
        Self::DestructiveEffectSafety,
        Self::HumanGuidanceRecovery,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationMetricKind {
    FixturePass,
    Threshold,
    LatencyBudget,
    ManualReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationRequiredLevel {
    MustPass,
    ShouldPass,
    ManualReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationEvalStatus {
    Draft,
    Required,
    Passed,
    Failed,
}
