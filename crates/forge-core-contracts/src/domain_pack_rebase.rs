//! Closed P7D read-only coordinated Core/Domain-Pack rebase plan types.
//!
//! A serialized plan is exact-CAS evidence only. It cannot authorize either
//! lifecycle or workflow-ledger mutation.

use crate::{
    DomainPackCandidateAuthority, DomainPackCompositionGap, DomainPackCoreBinding,
    DomainPackLifecycleOperation, DomainPackReceiptMigrationPolicy, StableId,
    WorkflowEffectiveBundleIdentity, WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_REBASE_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebasePlanDocument {
    pub schema_version: String,
    pub domain_pack_rebase_plan: DomainPackRebasePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebasePlan {
    pub plan_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub mutation_allowed: bool,
    pub apply_status: DomainPackRebaseApplyStatus,
    pub project_id: StableId,
    pub source_release: WorkflowGovernanceReleaseIdentity,
    pub target_release: WorkflowGovernanceReleaseIdentity,
    pub source_core: DomainPackCoreBinding,
    pub target_core: DomainPackCoreBinding,
    pub active_generation: DomainPackRebaseActiveGeneration,
    pub exact_cas: DomainPackRebaseExactCas,
    pub compatibility: DomainPackRebaseCompatibilityProjection,
    pub semantic_changes: Vec<DomainPackRebaseSemanticChange>,
    pub actionable_gaps: Vec<DomainPackRebaseGap>,
    pub plan_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRebaseApplyStatus {
    ReadyForTcbRevalidation,
    BlockedActionableGaps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebaseActiveGeneration {
    pub generation: u64,
    pub lifecycle_operation: DomainPackLifecycleOperation,
    pub degraded_empty: bool,
    pub active_package_count: usize,
    pub active_composition_gaps: Vec<DomainPackCompositionGap>,
}

/// Every independently durable input captured by the read-only plan.
/// `plan_digest` commits this complete structure plus the plan conclusions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebaseExactCas {
    pub expected_current_release_digest: String,
    pub expected_workflow_ledger_head_digest: String,
    pub expected_project_snapshot_digest: String,
    pub expected_effective_bundle_digest: String,
    pub expected_receipt_context_digest: String,
    pub expected_generation: u64,
    pub expected_lifecycle_pointer_digest: String,
    pub expected_lifecycle_head_digest: String,
    pub expected_active_lock_digest: String,
    pub expected_composition_digest: String,
    pub expected_supply_chain_registry_digest: String,
    pub expected_reviewer_registry_digest: String,
    pub expected_reviewed_registry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebaseCompatibilityProjection {
    pub adjacent_core_release_admitted: bool,
    pub core_policy_set_changed: bool,
    pub package_set_retained_as_candidate: bool,
    pub target_core_pack_compatibility: DomainPackRebaseCheckStatus,
    pub policy_recomposition: DomainPackRebaseCheckStatus,
    pub capability_revalidation: DomainPackRebaseCheckStatus,
    pub requirement_revalidation: DomainPackRebaseCheckStatus,
    pub supply_chain_revalidation: DomainPackRebaseCheckStatus,
    pub semantic_review_revalidation: DomainPackRebaseCheckStatus,
    pub workflow_receipt_carryover: WorkflowReceiptCarryover,
    pub domain_pack_receipt_carryover: DomainPackReceiptMigrationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRebaseCheckStatus {
    ExactCurrentBindingObserved,
    RequiresTargetRevalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebaseSemanticChange {
    pub kind: DomainPackRebaseSemanticChangeKind,
    pub subject_ref: StableId,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub explanation: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRebaseSemanticChangeKind {
    CoreReleaseChanged,
    CoreRuntimeBundleChanged,
    CorePolicySetChanged,
    PackCompatibilityPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebaseGap {
    pub code: DomainPackRebaseGapCode,
    pub subject_ref: StableId,
    pub message: String,
    pub next_action: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRebaseGapCode {
    TargetCoreCompositionRequired,
    SupplyChainReverificationRequired,
    ReviewedRegistryRevalidationRequired,
    CapabilityRevalidationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRebasePlanInput {
    pub project_id: StableId,
    pub source_release: WorkflowGovernanceReleaseIdentity,
    pub target_release: WorkflowGovernanceReleaseIdentity,
    pub source_core: DomainPackCoreBinding,
    pub target_core: DomainPackCoreBinding,
    pub target_workflow_receipt_carryover: WorkflowReceiptCarryover,
    pub effective_identity: WorkflowEffectiveBundleIdentity,
    pub lifecycle_operation: DomainPackLifecycleOperation,
    pub generation: u64,
    pub lifecycle_pointer_digest: String,
    pub lifecycle_head_digest: String,
    pub active_lock_digest: String,
    pub composition_digest: String,
    pub supply_chain_registry_digest: String,
    pub reviewer_registry_digest: String,
    pub reviewed_registry_digest: String,
    pub active_package_count: usize,
    pub active_composition_gaps: Vec<DomainPackCompositionGap>,
    pub workflow_ledger_head_digest: String,
    pub project_snapshot_digest: String,
}
