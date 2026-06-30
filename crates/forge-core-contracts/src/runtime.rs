use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeHandoffContractDocument {
    pub schema_version: String,
    pub runtime_handoff_contract: RuntimeHandoffContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRegistryEntryDocument {
    pub schema_version: String,
    pub runtime_registry_entry: RuntimeRegistryEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilityDocument {
    pub schema_version: String,
    pub runtime_capability: RuntimeCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeHandoffContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub status: RuntimeHandoffStatus,
    pub source_runtime: RuntimeEndpoint,
    pub target_runtime: RuntimeTargetEndpoint,
    pub task_boundary: RuntimeTaskBoundary,
    pub double_gate: RuntimeDoubleGate,
    pub handoff_payload: RuntimeHandoffPayload,
    pub blocked_reason: Option<RuntimeBlockedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEndpoint {
    pub kind: RuntimeKind,
    pub id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTargetEndpoint {
    pub kind: RuntimeKind,
    pub id: StableId,
    pub registry_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskBoundary {
    pub reason_code: StableId,
    pub current_runtime_gap: StableId,
    pub required_capability: RuntimeCapabilityKind,
    pub product_area: Option<StableId>,
    pub paths: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDoubleGate {
    pub requires_registry_evidence: bool,
    pub registry_evidence_ref: Option<RepoPath>,
    pub requires_capability_evidence: bool,
    pub capability_evidence_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeHandoffPayload {
    pub state_version: u64,
    pub context_refs: Vec<RepoPath>,
    pub forbidden_context_refs: Vec<StableId>,
    pub authority_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRegistryEntry {
    pub id: StableId,
    pub runtime_kind: RuntimeKind,
    pub runtime_id: StableId,
    pub status: RuntimeRegistryStatus,
    pub owner: StableId,
    pub surface: StableId,
    pub protocol_refs: Vec<StableId>,
    pub capability_refs: Vec<RepoPath>,
    pub trust_policy: RuntimeTrustPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTrustPolicy {
    pub may_receive_handoff: bool,
    pub may_mutate_forge_state: bool,
    pub must_use_requests_for_state_changes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapability {
    pub id: StableId,
    pub runtime_id: StableId,
    pub capability_kind: RuntimeCapabilityKind,
    pub evidence_kind: RuntimeEvidenceKind,
    pub protocol_surface: StableId,
    pub schema_ref: StableId,
    pub constraints: RuntimeCapabilityConstraints,
    pub safety: RuntimeCapabilitySafety,
}

// Each field is an independent capability that the runtime advertises.
// Modeling them as bitflags would obscure the JSON schema consumed by
// downstream agents and policy authors.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilityConstraints {
    pub requires_human_visible_browser: bool,
    pub may_access_network: bool,
    pub may_mutate_project_files: bool,
    pub output_must_be_evidence_ref: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilitySafety {
    pub unfiltered_agent_card_text_trusted: bool,
    pub prompt_injection_surface: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHandoffStatus {
    Blocked,
    Suggestible,
    Requested,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Codex,
    Cursor,
    Claude,
    Opencode,
    Vscode,
    Pidev,
    ForgeStandalone,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapabilityKind {
    CodeEdit,
    BrowserValidation,
    DesignSurface,
    LongRunningDaemon,
    RepoSplit,
    HumanReview,
    ExternalConnector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvidenceKind {
    RegistryEntry,
    CapabilityManifest,
    AgentCard,
    McpToolSchema,
    HumanApproval,
    StateRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBlockedReason {
    MissingRegistryEvidence,
    MissingCapabilityEvidence,
    ScopeNotOutsideCurrentRuntime,
    HandoffPayloadIncomplete,
    HumanOrDriverReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRegistryStatus {
    Available,
    Unavailable,
    Disabled,
}
