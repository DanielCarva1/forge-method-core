use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionCloseContractDocument {
    pub schema_version: String,
    pub decision_close_contract: DecisionCloseContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionCloseContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub decision: DecisionClose,
    pub requires_grill_evidence: bool,
    pub grill_contract_ref: Option<RepoPath>,
    pub authority_evidence: Vec<DecisionEvidenceRef>,
    pub closed_by: StableId,
    pub closed_at: Option<String>,
    pub reopen_policy: ReopenPolicy,
    pub blocked_reason: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionClose {
    pub kind: DecisionKind,
    pub status: DecisionStatus,
    pub target_phase: Option<StableId>,
    pub target_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionEvidenceRef {
    pub kind: DecisionEvidenceKind,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    PhaseTransition,
    Handoff,
    SpecLock,
    RouteChange,
    ReleaseReadiness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Open,
    Blocked,
    Closed,
    Reopened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReopenPolicy {
    HumanCorrectCourse,
    GrillRequired,
    DriverReview,
    ReleaseReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEvidenceKind {
    GrillResult,
    HumanApproval,
    GateResult,
    StateRule,
    HandoffRequest,
}
