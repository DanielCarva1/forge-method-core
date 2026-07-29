//! Read-only contracts for previewing isolated work before governed promotion.
//!
//! A preview is caller-carried audit data. It grants no apply authority and
//! cannot be converted into a mutation capability without a later kernel
//! re-admission of every bound snapshot and governance coordinate.

use crate::{
    ClaimId, IsolationStatus, RepoPath, StableId, WorkflowCooperativeEvidenceCurrentStatus,
    WorkflowCooperativeEvidenceDisposition, WorkflowCooperativeEvidenceNonProof,
    WorkflowCooperativeEvidenceProof, WorkflowReadinessProfile,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const GOVERNED_PROMOTION_PREVIEW_SCHEMA_VERSION: &str = "governed_promotion_preview_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GovernedPromotionPreviewAuthority {
    ReadOnlyCandidateNoApplyAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GovernedPromotionPreviewStatus {
    NoChanges,
    Reviewable,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionSnapshotBinding {
    /// Canonical absolute path used only to bind this local preview.
    pub canonical_root: String,
    pub canonical_root_digest: String,
    /// Complete promotion observation including file bytes, modes, and directories.
    pub snapshot_digest: String,
    /// Store-owned retained namespace/content digest used for late revalidation.
    pub retained_tree_digest: String,
    /// Compatibility digest of sorted regular-file path/content pairs.
    pub regular_file_set_digest: String,
    pub file_count: usize,
    pub directory_count: usize,
    pub total_regular_file_bytes: u64,
    /// Root-level names deliberately outside the promotion snapshot.
    pub excluded_roots: Vec<PromotionExcludedRootBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionExcludedRootKind {
    GitControlMetadata,
    ForgeControlState,
    BuildOrDependencyCache,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionExcludedRootBinding {
    pub name: String,
    pub kind: PromotionExcludedRootKind,
    pub present: bool,
    /// Metadata-only digest. Content is explicitly outside the promotable tree.
    pub metadata_digest: Option<String>,
    pub promotable_content_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionGitWorktreeBinding {
    pub common_repository_git_dir_digest: String,
    pub worktree_git_dir_digest: String,
    pub branch_ref: String,
    pub head_oid: String,
    pub canonical_repository_head_ref: String,
    pub canonical_repository_head_oid: String,
    pub observation_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionSourceBinding {
    pub isolation_id: StableId,
    pub isolation_contract_digest: String,
    pub isolation_contract_relative_path: String,
    pub isolation_status: IsolationStatus,
    pub agent_id: StableId,
    pub linked_claim_id: Option<ClaimId>,
    pub declared_worktree_path: RepoPath,
    pub git_worktree: PromotionGitWorktreeBinding,
    pub snapshot: PromotionSnapshotBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionDestinationBinding {
    pub project_id: StableId,
    pub snapshot: PromotionSnapshotBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionObjectiveBinding {
    pub objective_id: StableId,
    pub revision: u64,
    pub objective_digest: String,
    pub assurance_epoch: u64,
    pub accepted_record_digest: String,
    pub accepted_record_sequence: u64,
    pub accepted_state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionGovernanceBinding {
    pub readiness_profile: WorkflowReadinessProfile,
    pub effective_bundle_digest: String,
    pub selected_policy_ref: StableId,
    pub ledger_head_digest: String,
    pub state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionEvidenceRecordBinding {
    pub record_digest: String,
    pub offer_digest: String,
    pub historical_disposition: WorkflowCooperativeEvidenceDisposition,
    pub current_status: WorkflowCooperativeEvidenceCurrentStatus,
    pub supports_cooperative_claim_ref: Option<StableId>,
    pub does_not_satisfy_source_claim_ref: Option<StableId>,
    pub proves: Vec<WorkflowCooperativeEvidenceProof>,
    pub does_not_prove: Vec<WorkflowCooperativeEvidenceNonProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionEvidenceSetBinding {
    /// All historically admitted cooperative records for the active objective.
    /// Stale/disproving status is retained rather than silently discarded.
    pub admitted_records: Vec<PromotionEvidenceRecordBinding>,
    pub evidence_set_digest: String,
    pub supporting_record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionClaimSetBinding {
    pub claim_projection_digest: String,
    pub last_applied_sequence: u64,
    pub linked_claim_id: Option<ClaimId>,
    pub active_claim_ids: Vec<ClaimId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDiffEffect {
    CreateRegularFile,
    WriteRegularFile,
    DeleteRegularFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionDiffEntry {
    pub path: RepoPath,
    pub effect: PromotionDiffEffect,
    pub before_content_digest: Option<String>,
    pub before_byte_length: Option<u64>,
    pub after_content_digest: Option<String>,
    pub after_byte_length: Option<u64>,
    pub before_metadata_fingerprint: Option<String>,
    pub after_metadata_fingerprint: Option<String>,
    pub destructive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionObjectiveCoverageStatus {
    BoundToAcceptedObjectiveSemanticCoverageNotInferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionObjectiveCoverage {
    pub status: PromotionObjectiveCoverageStatus,
    pub semantic_coverage_caller_assertion_accepted: bool,
    pub open_uncertainty_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionAssuranceClaimStatus {
    Unknown,
    Supported,
    Verified,
    Waived,
    Disproven,
    Contradictory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionAssuranceClaimCoverage {
    pub claim_ref: StableId,
    pub status: PromotionAssuranceClaimStatus,
    pub accepted_evidence_refs: Vec<String>,
    pub rejected_evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionWriteClaimCoverageStatus {
    FullyGovernedByIsolationOwner,
    PartiallyOrFullyUngoverned,
    ConflictingClaim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionWriteClaimCoverage {
    pub status: PromotionWriteClaimCoverageStatus,
    pub governed_paths: Vec<RepoPath>,
    pub ungoverned_paths: Vec<RepoPath>,
    pub path_attribution: Vec<PromotionPathClaimAttribution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionPathClaimAttribution {
    pub path: RepoPath,
    pub governing_linked_claim_id: Option<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionClaimConflict {
    pub path: RepoPath,
    pub blocking_claim_id: ClaimId,
    pub blocking_agent_id: StableId,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PromotionUnsupportedEffectKind {
    FileMetadataCreate,
    FileModeChange,
    DirectoryCreate,
    DirectoryDelete,
    ObjectTypeTransition,
    SymlinkOrSpecialObject,
    ExcludedSourceRootContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionUnsupportedEffect {
    pub path: RepoPath,
    pub kind: PromotionUnsupportedEffectKind,
    pub detail: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PromotionGapCode {
    NoChanges,
    OpenObjectiveUncertainties,
    MissingSupportingCooperativeEvidence,
    SourceAssuranceClaimUnsatisfied,
    MissingLinkedIsolationClaim,
    UngovernedWriteSet,
    ConflictingClaim,
    DestructiveDeleteRequiresSeparateAuthority,
    UnsupportedEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromotionGap {
    pub code: PromotionGapCode,
    pub subject_ref: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernedPromotionPreview {
    pub schema_version: String,
    /// Stable id derived from `preview_digest`.
    pub preview_id: StableId,
    /// Digest of the canonical document with both digest-derived fields blank.
    pub preview_digest: String,
    /// Final observation time after bounded filesystem capture and revalidation.
    pub observed_at_unix: u64,
    /// Earliest evidence/claim validity boundary known to this preview.
    pub valid_through_unix: Option<u64>,
    pub authority: GovernedPromotionPreviewAuthority,
    pub status: GovernedPromotionPreviewStatus,
    pub canonical_mutation_performed: bool,
    pub forge_state_mutation_performed: bool,
    pub source: PromotionSourceBinding,
    pub destination: PromotionDestinationBinding,
    pub objective: PromotionObjectiveBinding,
    pub governance: PromotionGovernanceBinding,
    pub evidence: PromotionEvidenceSetBinding,
    pub claims: PromotionClaimSetBinding,
    pub diff: Vec<PromotionDiffEntry>,
    pub write_set: Vec<RepoPath>,
    pub diff_digest: String,
    pub write_set_digest: String,
    pub predicted_result_regular_file_set_digest: String,
    pub objective_coverage: PromotionObjectiveCoverage,
    pub assurance_claim_coverage: Vec<PromotionAssuranceClaimCoverage>,
    pub write_claim_coverage: PromotionWriteClaimCoverage,
    pub conflicts: Vec<PromotionClaimConflict>,
    pub unsupported_effects: Vec<PromotionUnsupportedEffect>,
    pub unresolved_gaps: Vec<PromotionGap>,
}
