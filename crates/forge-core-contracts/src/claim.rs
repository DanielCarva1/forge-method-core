use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimContractDocument {
    pub schema_version: String,
    pub claim_contract: ClaimContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub claim: ClaimIdentity,
    pub scope: ClaimScope,
    pub lease: ClaimLease,
    pub status: ClaimStatusRecord,
    pub expiry_policy: ExpiryPolicy,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimIdentity {
    pub kind: ClaimKind,
    pub claimant_agent_id: StableId,
    pub claimant_role: ActorRole,
    pub registry_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimScope {
    pub kind: ClaimScopeKind,
    pub id: StableId,
    pub product_area: Option<StableId>,
    pub paths: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimLease {
    pub acquired_at: String,
    pub last_heartbeat_at: String,
    pub expires_at: String,
    pub ttl_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub expected_state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimStatusRecord {
    pub value: ClaimStatus,
    pub evaluated_at: String,
    pub reason_code: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExpiryPolicy {
    pub on_expiry: ExpiryAction,
    pub handoff_required: bool,
    pub release_without_handoff_allowed: bool,
    pub reclaim_policy: ReclaimPolicy,
    pub handoff_request_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Driver,
    Lane,
    Story,
    ProductArea,
    Integration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorRole {
    Driver,
    Worker,
    Human,
    Runtime,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimScopeKind {
    Project,
    ProductArea,
    Story,
    Lane,
    Integration,
    IntegrationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Active,
    Stale,
    Expired,
    HandoffRequired,
    HandoffRecorded,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpiryAction {
    RecordHandoffRequest,
    DriverReview,
    HumanReview,
    BlockUntilRecovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReclaimPolicy {
    SameAgentOnly,
    DriverReview,
    OwnerReview,
    HumanReview,
    Forbidden,
}
