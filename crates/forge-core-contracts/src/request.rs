use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestContractDocument {
    pub schema_version: String,
    pub request_contract: RequestContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub kind: RequestKind,
    pub communication: Communication,
    pub sender_agent_id: StableId,
    pub sender_role: RequestRole,
    pub target_driver: StableId,
    pub requested_operation: StableId,
    pub target: RequestTarget,
    pub reason_code: StableId,
    pub evidence_refs: Vec<String>,
    pub payload: RequestPayload,
    pub response: RequestResponse,
    pub response_required: bool,
    pub deadline: Option<String>,
    pub status: RequestStatus,
    pub append_only: AppendOnlyPolicy,
    pub safety: RequestSafety,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Communication {
    pub performative: Performative,
    pub sender: Participant,
    pub receiver: Participant,
    pub thread_id: StableId,
    pub correlation_id: StableId,
    pub reply_to: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub agent_id: StableId,
    pub role: RequestRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestTarget {
    pub kind: RequestTargetKind,
    pub id: StableId,
    pub product_area: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestPayload {
    pub kind: PayloadKind,
    pub evidence_refs: Vec<String>,
    pub dependency_refs: Vec<DependencyRef>,
    pub summary_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyRef {
    pub kind: DependencyKind,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestResponse {
    pub required: bool,
    pub expected_kind: ResponseKind,
    pub deadline: Option<String>,
    pub allowed_statuses: Vec<RequestStatus>,
    pub required_evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppendOnlyPolicy {
    pub path: RepoPath,
    pub mutation_allowed: MutationAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestSafety {
    pub may_mutate_integration_state: bool,
    pub driver_must_apply: bool,
    #[serde(default)]
    pub release_without_handoff_allowed: Option<bool>,
    #[serde(default)]
    pub reclaim_without_review_allowed: Option<bool>,
    #[serde(default)]
    pub target_runtime_must_follow_authority_refs: Option<bool>,
    #[serde(default)]
    pub overwrite_conflicting_target_allowed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    StateTransition,
    ConflictNotification,
    ClaimExpiryHandoff,
    RuntimeHandoff,
    DestructiveEffectReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestRole {
    Driver,
    Worker,
    Runtime,
    Human,
    ExternalRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Performative {
    Request,
    Notify,
    Handoff,
    ReviewRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestTargetKind {
    IntegrationState,
    FilePath,
    Story,
    Runtime,
    ToolEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PayloadKind {
    TypedRefs,
    StatusOnly,
    HandoffPayload,
    ReviewPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    Ack,
    ApproveReject,
    ApplyOrReject,
    AcceptReject,
    ReviewDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Accepted,
    Rejected,
    Applied,
    Superseded,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MutationAllowed {
    AppendOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Request,
    Claim,
    Gate,
    Effect,
    RuntimeHandoff,
    Decision,
}
