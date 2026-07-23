use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateContractDocument {
    pub schema_version: String,
    pub gate_contract: GateContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub gate: GateResult,
    pub target: GateTarget,
    pub evidence_refs: Vec<GateEvidenceRef>,
    pub promotion: GatePromotion,
    pub blocks: GateBlocks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub scope: GateScope,
    pub status: GateStatus,
    pub checked_at: String,
    pub checked_by: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateTarget {
    pub kind: GateTargetKind,
    pub id: StableId,
    pub product_area: Option<StableId>,
    pub paths: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateEvidenceRef {
    pub kind: EvidenceKind,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatePromotion {
    pub policy: PromotionPolicy,
    pub parent_gate_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateBlocks {
    pub operations: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateScope {
    Lane,
    ProductArea,
    Integration,
    Destructive,
    Release,
    Authority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Pass,
    Fail,
    Concerns,
    Missing,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateTargetKind {
    Lane,
    ProductArea,
    Project,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    TestReport,
    ArtifactCheck,
    IntegrationCheck,
    ReleaseCheck,
    HumanApproval,
    SecurityReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionPolicy {
    NoPromotion,
    MayFeedParentGate,
    RequiresParentGate,
}
