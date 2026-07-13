//! Closed P5d.5 contracts for evidence-backed legacy workflow retirement.
//!
//! Every document in this module is non-authoritative input. In particular,
//! authored equality flags, scorecard counts, tombstones, and detached
//! signatures cannot retire legacy authority without deterministic
//! recomputation and trusted signature verification.

use crate::{
    RepoPath, StableId, WorkflowGovernanceReleaseIdentity, WorkflowReleaseAdmissionSignatureV2,
    WorkflowRuntimeBundleIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_RETIREMENT_EVIDENCE_INDEX_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RETIREMENT_SNAPSHOT_MANIFEST_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_DELETION_PROOF_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_CONSUMER_COMPATIBILITY_REPORT_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_CONSUMER_COMPATIBILITY_MATRIX_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RETIREMENT_TOMBSTONE_CATALOG_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_FINAL_SCORECARD_SCHEMA_VERSION: &str = "0.1";
/// Version 0.1 in `workflow_release` remains the original single-workflow
/// proposal. This aggregate authorization is an append-only V2 contract.
pub const WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementArtifactBinding {
    pub artifact_id: StableId,
    pub embedded_ref: RepoPath,
    pub raw_digest: String,
    pub canonical_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementSnapshotEntry {
    pub logical_ref: RepoPath,
    pub archive_ref: RepoPath,
    pub raw_digest: String,
    pub canonical_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementSnapshotManifest {
    pub id: StableId,
    pub snapshot_version: String,
    pub entries: Vec<WorkflowRetirementSnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementSnapshotManifestDocument {
    pub schema_version: String,
    pub workflow_retirement_snapshot_manifest: WorkflowRetirementSnapshotManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementWorkflowBinding {
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub replacement_policy_ref: StableId,
    pub replacement_policy_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementCandidateAuthority {
    CandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementEvidenceIndexDocument {
    pub schema_version: String,
    pub workflow_retirement_evidence_index: WorkflowRetirementEvidenceIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementEvidenceIndex {
    pub id: StableId,
    pub index_version: String,
    pub authority: WorkflowRetirementCandidateAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub legacy_catalog_digest: String,
    pub release_manifest: WorkflowRetirementArtifactBinding,
    pub runtime_bundle_artifact: WorkflowRetirementArtifactBinding,
    pub snapshot_manifest: WorkflowRetirementArtifactBinding,
    pub runtime_evidence: WorkflowRetirementArtifactBinding,
    pub release_history: WorkflowRetirementArtifactBinding,
    pub retirements: Vec<WorkflowRetirementWorkflowBinding>,
    pub deletion_proof: WorkflowRetirementArtifactBinding,
    pub consumer_report: WorkflowRetirementArtifactBinding,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDeletionSurface {
    Routing,
    Readiness,
    Verdicts,
    Receipts,
    Continuation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDeletionSurfaceProof {
    pub surface: WorkflowDeletionSurface,
    pub control_digest: String,
    pub legacy_ablated_digest: String,
    pub equivalent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDeletionProofEntry {
    pub retirement: WorkflowRetirementWorkflowBinding,
    pub legacy_present_in_control: bool,
    pub legacy_present_after_ablation: bool,
    pub surfaces: Vec<WorkflowDeletionSurfaceProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDeletionProofDocument {
    pub schema_version: String,
    pub workflow_deletion_proof: WorkflowDeletionProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDeletionProof {
    pub id: StableId,
    pub proof_version: String,
    pub authority: WorkflowRetirementCandidateAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub legacy_catalog_digest: String,
    pub release_history: WorkflowRetirementArtifactBinding,
    pub workflows: Vec<WorkflowDeletionProofEntry>,
    pub mismatch_count: usize,
    pub evaluation_error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityEntry {
    pub workflow_id: StableId,
    pub diagnostic_code: StableId,
    pub replacement_policy_ref: StableId,
    pub replacement_argv: Vec<String>,
    pub diagnostic_fixture_count: usize,
    pub unsupported_repository_consumer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityMatrixEntry {
    pub workflow_id: StableId,
    pub diagnostic_code: StableId,
    pub replacement_policy_ref: StableId,
    pub replacement_argv: Vec<String>,
    pub repository_fixture_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityMatrixDocument {
    pub schema_version: String,
    pub workflow_consumer_compatibility_matrix: WorkflowConsumerCompatibilityMatrix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityMatrix {
    pub id: StableId,
    pub matrix_version: String,
    pub authority: WorkflowRetirementCandidateAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub legacy_catalog_digest: String,
    pub operational_catalog_digest: String,
    pub minimum_consumer_version: String,
    pub entries: Vec<WorkflowConsumerCompatibilityMatrixEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConsumerObservationSource {
    RepositoryCompatibilityMatrix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityReportDocument {
    pub schema_version: String,
    pub workflow_consumer_compatibility_report: WorkflowConsumerCompatibilityReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConsumerCompatibilityReport {
    pub id: StableId,
    pub report_version: String,
    pub authority: WorkflowRetirementCandidateAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub legacy_catalog_digest: String,
    pub announced_at_unix: u64,
    pub retirement_not_before_unix: u64,
    pub observed_from_unix: u64,
    pub observed_until_unix: u64,
    pub minimum_consumer_version: String,
    pub consumer_population_digest: String,
    pub observation_source: WorkflowConsumerObservationSource,
    pub compatibility_matrix: WorkflowRetirementArtifactBinding,
    pub workflows: Vec<WorkflowConsumerCompatibilityEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementTombstoneAuthority {
    NonAuthoritativeDiagnosticsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementTombstone {
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub diagnostic_code: StableId,
    pub replacement_policy_ref: StableId,
    pub replacement_release_id: StableId,
    pub replacement_argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementTombstoneCatalogDocument {
    pub schema_version: String,
    pub workflow_retirement_tombstone_catalog: WorkflowRetirementTombstoneCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementTombstoneCatalog {
    pub id: StableId,
    pub catalog_version: String,
    pub authority: WorkflowRetirementTombstoneAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub tombstones: Vec<WorkflowRetirementTombstone>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFinalRuntimeDisposition {
    Executable,
    CompatibilityOnly,
    Quarantined,
    DomainPackCandidate,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFinalLegacyAuthorityState {
    Retired,
    Retained,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinalRuntimeDispositionCounts {
    pub executable: usize,
    pub compatibility_only: usize,
    pub quarantined: usize,
    pub domain_pack_candidate: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinalLegacyAuthorityCounts {
    pub retired: usize,
    pub retained: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinalScorecardAssessment {
    pub workflow_id: StableId,
    pub runtime_disposition: WorkflowFinalRuntimeDisposition,
    pub legacy_authority: WorkflowFinalLegacyAuthorityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement_evidence_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFinalScorecardAuthority {
    DerivedCandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinalScorecardDocument {
    pub schema_version: String,
    pub workflow_final_scorecard: WorkflowFinalScorecard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinalScorecard {
    pub id: StableId,
    pub scorecard_version: String,
    pub authority: WorkflowFinalScorecardAuthority,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub legacy_catalog_digest: String,
    pub evidence_index: WorkflowRetirementArtifactBinding,
    pub runtime_disposition_counts: WorkflowFinalRuntimeDispositionCounts,
    pub legacy_authority_counts: WorkflowFinalLegacyAuthorityCounts,
    pub assessments: Vec<WorkflowFinalScorecardAssessment>,
    pub evaluation_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementAuthorizationV2Authority {
    CandidateAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorizationV2Payload {
    pub authorization_id: StableId,
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub legacy_catalog_digest: String,
    pub release_manifest: WorkflowRetirementArtifactBinding,
    pub runtime_bundle_artifact: WorkflowRetirementArtifactBinding,
    pub snapshot_manifest: WorkflowRetirementArtifactBinding,
    pub runtime_evidence: WorkflowRetirementArtifactBinding,
    pub release_history: WorkflowRetirementArtifactBinding,
    pub retirements: Vec<WorkflowRetirementWorkflowBinding>,
    pub evidence_index: WorkflowRetirementArtifactBinding,
    pub deletion_proof: WorkflowRetirementArtifactBinding,
    pub consumer_report: WorkflowRetirementArtifactBinding,
    pub tombstone_catalog: WorkflowRetirementArtifactBinding,
    pub final_scorecard: WorkflowRetirementArtifactBinding,
    pub reviewer_registry: WorkflowRetirementArtifactBinding,
    pub audience: String,
    pub domain: String,
    pub nonce: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorizationV2 {
    pub authority: WorkflowRetirementAuthorizationV2Authority,
    pub payload: WorkflowRetirementAuthorizationV2Payload,
    /// Candidate signatures only. An authority crate must verify their keys,
    /// roles, independence, validity windows, payload digest, and bytes.
    pub signatures: Vec<WorkflowReleaseAdmissionSignatureV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorizationV2Document {
    pub schema_version: String,
    pub workflow_retirement_authorization_v2: WorkflowRetirementAuthorizationV2,
}
