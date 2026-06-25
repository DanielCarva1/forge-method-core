use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthRecoveryContractDocument {
    pub schema_version: String,
    pub health_recovery_contract: HealthRecoveryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthRecoveryContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub status: HealthStatus,
    pub runtime: RecoveryRuntime,
    pub heartbeat: RecoveryHeartbeat,
    pub anomaly: RecoveryAnomaly,
    pub recovery: RecoveryPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRuntime {
    pub agent_id: StableId,
    pub role: RecoveryRole,
    pub host: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryHeartbeat {
    pub last_seen_at: Option<String>,
    pub ttl_seconds: u64,
    pub missed_heartbeats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAnomaly {
    pub kind: AnomalyKind,
    pub scope: StableId,
    pub evidence_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPlan {
    pub action: RecoveryAction,
    pub automatic_allowed: bool,
    pub requires_review: bool,
    pub request_ref: Option<RepoPath>,
    pub claim_ref: Option<RepoPath>,
    pub handoff_context_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Stalled,
    Crashed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyKind {
    HeartbeatMissed,
    ToolFailure,
    ContextDegraded,
    StaleState,
    Unresponsive,
    Crash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    None,
    RecordRequest,
    HandoffToDriver,
    ReclaimAfterReview,
    RestartRuntime,
    OpenNewThread,
    QuarantineRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryRole {
    Driver,
    Worker,
    Runtime,
    ExternalRuntime,
}
