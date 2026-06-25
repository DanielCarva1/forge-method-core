use crate::claim::ActorRole;
use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionContractDocument {
    pub schema_version: String,
    pub completion_contract: CompletionContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub task: CompletionTask,
    pub status: CompletionStatusRecord,
    pub claim: CompletionClaim,
    pub proof_policy: ProofPolicy,
    pub proof_refs: Vec<String>,
    pub invalidation: Invalidation,
    pub storage: CompletionStorage,
    #[serde(default)]
    pub rust_compatibility: Option<CompletionRustCompatibility>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionTask {
    pub task_id: StableId,
    pub kind: TaskKind,
    pub product_area: Option<StableId>,
    pub lane_id: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionStatusRecord {
    pub value: CompletionStatus,
    pub changed_at: String,
    pub changed_by: StableId,
    pub changed_by_role: ActorRole,
    pub checked_at_state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionClaim {
    pub claim_contract_ref: Option<RepoPath>,
    pub claim_expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofPolicy {
    pub required_for_done: bool,
    pub accepted_proof_kinds: Vec<ProofKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Invalidation {
    pub invalidated_by: Option<StableId>,
    pub invalidated_at: Option<String>,
    pub reason_code: Option<StableId>,
    pub supersedes_completion_id: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionStorage {
    pub canonical_id: StableId,
    pub backend: StorageBackend,
    pub event_log_ref: Option<RepoPath>,
    pub projection_key: StableId,
    pub db_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionRustCompatibility {
    pub serde_enum_safe: bool,
    pub free_text_status_allowed: bool,
    pub db_migration_requires_shape_change: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Story,
    Gate,
    Artifact,
    ProductArea,
    Integration,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    NotStarted,
    Claimed,
    InProgress,
    Blocked,
    Done,
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProofKind {
    TestReport,
    GateResult,
    ArtifactRef,
    HumanApproval,
    ReleaseEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    FileProjection,
    Sqlite,
    EmbeddedKv,
    DaemonProjection,
    McpService,
}
