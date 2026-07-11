use crate::claim::ActorRole;
use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolEffectContractDocument {
    pub schema_version: String,
    pub tool_effect_contract: ToolEffectContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolEffectContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub effect_kind: EffectKind,
    pub operation_ref: StableId,
    pub actor: EffectActor,
    pub read_set: Vec<EffectRead>,
    pub write_set: Vec<EffectWrite>,
    pub conflict_detection: ConflictDetection,
    pub notification: EffectNotification,
    pub repair: EffectRepair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectActor {
    pub agent_id: StableId,
    pub role: ActorRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectRead {
    pub target_kind: EffectTargetKind,
    #[serde(rename = "ref")]
    pub reference: String,
    pub expected_hash: Option<String>,
    pub expected_version: Option<u64>,
    pub required_for_plan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectWrite {
    pub target_kind: EffectTargetKind,
    #[serde(rename = "ref")]
    pub reference: String,
    pub access_mode: AccessMode,
    pub expected_hash: Option<String>,
    pub expected_version: Option<u64>,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConflictDetection {
    pub check_against: StableId,
    pub granularity: StableId,
    pub conflict_codes: Vec<ConflictCode>,
    pub policy: ConflictPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectNotification {
    pub required: bool,
    pub recipients: Vec<StableId>,
    pub request_contract_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectRepair {
    pub strategy: RepairStrategy,
    pub automatic_repair_allowed: bool,
    pub inverse_operation_ref: Option<String>,
    pub stop_if_inverse_missing: bool,
    pub inverse: InverseMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InverseMetadata {
    pub kind: InverseKind,
    pub source: InverseSource,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub input_mapping_refs: Vec<String>,
    pub validation_gate_refs: Vec<RepoPath>,
    pub review_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    OperationTransaction,
    FileEdit,
    StateWrite,
    ArtifactWrite,
    EvidenceAppend,
    RequestAppend,
    CommandRun,
    GitStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetKind {
    FilePath,
    Glob,
    StateKey,
    ArtifactId,
    EvidenceId,
    LedgerStream,
    RequestStream,
    CompletionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    Read,
    Write,
    Append,
    Create,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictCode {
    ReadTargetChanged,
    WriteTargetChanged,
    WriteTargetClaimed,
    ExpectedStateVersionMismatch,
    CompletionNowDone,
    PathOutsideScope,
    OverlappingWriteSet,
    MissingInverseForDestructiveWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    Allow,
    Block,
    NotifyAndRepair,
    DriverReview,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepairStrategy {
    None,
    RebasePlan,
    RefreshReads,
    RecordRequest,
    RunGate,
    AskHuman,
    CompensateThenRetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InverseKind {
    None,
    ExactRollback,
    LogicalCompensation,
    RestoreSnapshot,
    AppendReversal,
    ManualRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InverseSource {
    Unavailable,
    ToolContract,
    McpAnnotation,
    CommandContract,
    Snapshot,
    HumanApproval,
}
