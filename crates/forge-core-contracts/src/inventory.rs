use crate::common::{RepoPath, SourceId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractFamilyInventoryDocument {
    pub schema_version: String,
    pub contract_family_inventory: ContractFamilyInventory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractFamilyInventory {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub created_at: String,
    pub status: InventoryStatus,
    pub lock_scope: LockScope,
    pub family_discovery_policy: FamilyDiscoveryPolicy,
    pub supporting_policy_refs: Vec<RepoPath>,
    pub supporting_research_refs: Vec<SourceId>,
    pub families: Vec<ContractFamily>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LockScope {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FamilyDiscoveryPolicy {
    pub before_schema_generation: String,
    pub after_schema_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractFamily {
    pub id: StableId,
    pub family_kind: FamilyKind,
    pub schema_ref: RepoPath,
    pub instance_globs: Vec<String>,
    pub validator_function: String,
    pub validation_surface: ValidationSurface,
    pub status: FamilyStatus,
    pub evidence_refs: Vec<SourceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    Draft,
    LockedForSchemaGeneration,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FamilyStatus {
    Active,
    LockedForSchemaGeneration,
    Reserved,
    SupportOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSurface {
    RustContractValidator,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FamilyKind {
    AuthorityResponse,
    AssuranceCase,
    ReferencePolicy,
    CommandSurface,
    CoordinationClaim,
    CompletionState,
    DecisionClose,
    QualityGate,
    ToolEffect,
    CoordinationRequest,
    RuntimeHandoff,
    CoordinationEval,
    HealthRecovery,
    MigrationManifest,
    WorkflowGovernancePolicy,
    WorkflowGovernanceRelease,
    WorkflowMigrationBatch,
    WorkflowRetirementAuthorization,
}
