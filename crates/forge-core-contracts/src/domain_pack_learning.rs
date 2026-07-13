//! Closed P6c contracts for non-authoritative learning, reviewed promotion,
//! and the append-only reviewed Domain Pack registry.
//!
//! Documents in this module are durable evidence and candidate requests. They
//! never grant authority merely by deserializing or passing structural
//! validation. Admission remains an opaque trusted-boundary operation.

use std::collections::BTreeSet;

use crate::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCoordinate,
    DomainPackResolutionProjectionDocument, DomainPackResolutionStatus, DomainPackVersionReference,
    PrincipalId, RepoPath, StableId, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN_PACK_LEARNING_SCHEMA_VERSION: &str = "0.3";
pub const MAX_LEARNING_EVIDENCE: usize = 256;
pub const MAX_LEARNING_FINDINGS: usize = 256;
pub const MAX_LEARNING_FIXTURES: usize = 128;
pub const MAX_LEARNING_REVIEWS: usize = 32;
pub const MAX_LEARNING_CONFLICTS: usize = 128;
pub const MAX_REVIEWED_REGISTRY_ENTRIES: usize = 4_096;

/// Semantic assurance is independent from P6b supply-chain assurance. Pure
/// resolution always emits `Unreviewed`; only the trusted promotion boundary
/// can mint `Reviewed` after exact registry verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSemanticAssurance {
    Unreviewed,
    ValidatedCandidate,
    Reviewed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackLearningContractIssue {
    pub code: DomainPackLearningContractIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningContractIssueCode {
    UnsupportedSchemaVersion,
    MissingRequiredValue,
    InvalidDigest,
    InvalidTimeWindow,
    InvalidStageTransition,
    AuthorityEscalation,
    MissingDurableEvidence,
    MissingIndependentReview,
    UnresolvedConflict,
    InvalidRegistryChain,
    InvalidRegistryEligibility,
    InvalidRevocation,
    InvalidSupersession,
    DuplicateRecord,
    ResourceLimitExceeded,
    CrossReferenceMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLocalLearningCandidateDocument {
    pub schema_version: String,
    pub domain_pack_local_learning_candidate: DomainPackLocalLearningCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLocalLearningCandidate {
    pub candidate_id: StableId,
    pub authority: DomainPackLocalLearningAuthority,
    pub target: DomainPackLearningTarget,
    pub assertion: String,
    pub provenance: DomainPackLearningProvenance,
    pub evidence: Vec<DomainPackLearningEvidenceBinding>,
    pub observed_at_unix: u64,
    pub candidate_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLocalLearningAuthority {
    NonAuthoritativeObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningTarget {
    pub pack: DomainPackCoordinate,
    pub base_version: Option<String>,
    pub contribution_ref: Option<StableId>,
    pub proposed_namespace: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningProvenance {
    pub source_kind: DomainPackLearningSourceKind,
    pub source_ref: String,
    pub source_digest: String,
    pub captured_by: PrincipalId,
    pub capture_run_id: StableId,
    /// A transcript can aid audit, but is never evidence of correctness by
    /// itself and never enters promotion authority.
    pub chat_transcript_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningSourceKind {
    RunEvidence,
    UserReport,
    EvaluatorObservation,
    ResearchArtifact,
    ImportedRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningEvidenceBinding {
    pub evidence_id: StableId,
    pub kind: DomainPackLearningEvidenceKind,
    pub artifact: DomainPackArtifactBinding,
    pub producer: PrincipalId,
    pub produced_at_unix: u64,
    pub provenance_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningEvidenceKind {
    Reproduction,
    TestRun,
    EvaluationRun,
    Ablation,
    StrongJudgeComparison,
    UserOutcome,
    Incident,
    ResearchSource,
    ImplementationDiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionDossierDocument {
    pub schema_version: String,
    pub domain_pack_promotion_dossier: DomainPackPromotionDossier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionDossier {
    pub dossier_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub pack: DomainPackVersionReference,
    pub package_digest: String,
    pub manifest_digest: String,
    pub content_digest: String,
    pub license_digest: String,
    pub transition: DomainPackPromotionTransition,
    pub candidate_digests: Vec<String>,
    pub prior_promotion_record_digest: Option<String>,
    pub evidence: Vec<DomainPackLearningEvidenceBinding>,
    pub evaluator_runs: Vec<DomainPackLearningEvaluatorRun>,
    pub fixture_bindings: Vec<DomainPackLearningFixtureBinding>,
    pub provenance: DomainPackPromotionProvenance,
    pub conflict_record_digests: Vec<String>,
    pub open_gap_refs: Vec<StableId>,
    pub dossier_digest: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionStage {
    Candidate,
    Trial,
    Validated,
    Reviewed,
    Deprecated,
    Revoked,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionTransition {
    pub from: DomainPackPromotionStage,
    pub to: DomainPackPromotionStage,
}

impl DomainPackPromotionTransition {
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(
            (self.from, self.to),
            (
                DomainPackPromotionStage::Candidate,
                DomainPackPromotionStage::Trial | DomainPackPromotionStage::Revoked
            ) | (
                DomainPackPromotionStage::Trial,
                DomainPackPromotionStage::Validated | DomainPackPromotionStage::Revoked
            ) | (
                DomainPackPromotionStage::Validated,
                DomainPackPromotionStage::Reviewed | DomainPackPromotionStage::Revoked
            ) | (
                DomainPackPromotionStage::Reviewed,
                DomainPackPromotionStage::Deprecated
                    | DomainPackPromotionStage::Revoked
                    | DomainPackPromotionStage::Superseded
            ) | (
                DomainPackPromotionStage::Deprecated,
                DomainPackPromotionStage::Revoked | DomainPackPromotionStage::Superseded
            )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionProvenance {
    pub authored_by: Vec<PrincipalId>,
    pub source_repository: String,
    pub source_revision: String,
    pub source_tree_digest: String,
    pub build_recipe_digest: String,
    pub generated_artifact_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningEvaluatorRun {
    pub run_id: StableId,
    pub evaluator_ref: StableId,
    pub evaluator_principal: PrincipalId,
    pub evaluator_digest: String,
    pub fixture_set_digest: String,
    pub protocol_version: String,
    pub comparison: DomainPackLearningComparison,
    pub strong_judge_proof: Option<DomainPackStrongJudgeProof>,
    pub evidence_ref: StableId,
    pub run_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackStrongJudgeProof {
    pub judge_principal: PrincipalId,
    pub independence_domain: StableId,
    pub blind_ab: bool,
    pub deterministic_order_digest: String,
    pub rubric_digest: String,
    pub model_digest: String,
    pub prompt_digest: String,
    pub input_digest: String,
    pub output_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningComparison {
    pub method: DomainPackLearningComparisonMethod,
    pub baseline_outcome_digest: String,
    pub candidate_outcome_digest: String,
    pub verdict: DomainPackLearningComparisonVerdict,
    pub regression_finding_refs: Vec<StableId>,
    pub unknown_gap_refs: Vec<StableId>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningComparisonMethod {
    Ablation,
    StrongJudge,
    ControlledReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningComparisonVerdict {
    Improved,
    Equivalent,
    Regressed,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningFixtureBinding {
    pub fixture_id: StableId,
    pub fixture_ref: RepoPath,
    pub producer: PrincipalId,
    pub raw_sha256: String,
    pub canonical_sha256: String,
    pub expected_outcome_digest: String,
    pub provenance_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackIndependentReviewDocument {
    pub schema_version: String,
    pub domain_pack_independent_review: DomainPackIndependentReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackIndependentReview {
    pub review_id: StableId,
    pub authority: DomainPackReviewAuthority,
    pub dossier_digest: String,
    pub reviewer_id: PrincipalId,
    pub reviewer_role: DomainPackReviewerRole,
    pub reviewer_registry_digest: String,
    pub credential_id: StableId,
    pub independence: DomainPackReviewerIndependence,
    pub decision: DomainPackReviewDecision,
    pub findings: Vec<DomainPackReviewFinding>,
    pub signed_subject_digest: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub review_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewAuthority {
    ReviewEvidenceOnly,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewerRole {
    DomainExpert,
    EvidenceReviewer,
    SafetyReviewer,
    CompatibilityReviewer,
    RegistryAuthorizer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackReviewerIndependence {
    Independent { attestation: String },
    ConflictDeclared { conflict_record_digest: String },
    NotIndependent { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewDecision {
    Approve,
    Reject,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewFinding {
    pub finding_id: StableId,
    pub severity: DomainPackReviewFindingSeverity,
    pub disposition: DomainPackReviewFindingDisposition,
    pub evidence_refs: Vec<StableId>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewFindingSeverity {
    Advisory,
    Required,
    Blocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewFindingDisposition {
    Open,
    Resolved,
    AcceptedRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningConflictDocument {
    pub schema_version: String,
    pub domain_pack_learning_conflict: DomainPackLearningConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningConflict {
    pub conflict_id: StableId,
    pub authority: DomainPackConflictAuthority,
    pub target: DomainPackLearningTarget,
    pub kind: DomainPackLearningConflictKind,
    pub subject_digests: Vec<String>,
    pub evidence_refs: Vec<StableId>,
    pub status: DomainPackLearningConflictStatus,
    pub review_request_digest: Option<String>,
    pub resolution: Option<DomainPackLearningConflictResolution>,
    pub conflict_digest: String,
}

/// Canonical content identity of a learning-conflict record. The authored
/// `conflict_digest` field is removed rather than blanked, so mutation of
/// status, evidence, or resolution necessarily changes this digest.
///
/// # Errors
///
/// Returns an error when the typed document cannot be converted to canonical
/// JSON or the authored digest field is unexpectedly absent.
pub fn domain_pack_learning_conflict_digest(
    document: &DomainPackLearningConflictDocument,
) -> Result<String, String> {
    let mut value = serde_json::to_value(document).map_err(|error| error.to_string())?;
    value
        .get_mut("domain_pack_learning_conflict")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|conflict| conflict.remove("conflict_digest"))
        .ok_or_else(|| "conflict digest field is absent".to_owned())?;
    let canonical = serde_json_canonicalizer::to_vec(&value).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackConflictAuthority {
    ConflictEvidenceOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningConflictKind {
    ContradictoryObservation,
    ReviewedKnowledgeDisagreement,
    FixtureOutcomeMismatch,
    EvaluatorDisagreement,
    ProvenanceDispute,
    CompatibilityDispute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningConflictStatus {
    Open,
    ReviewRequested,
    Resolved,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningConflictResolution {
    pub decision: DomainPackLearningConflictResolutionDecision,
    pub rationale: String,
    pub evidence_refs: Vec<StableId>,
    pub resolved_by_review_digests: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningConflictResolutionDecision {
    PreferExisting,
    PreferCandidate,
    MergeWithQualification,
    RejectBoth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningReviewRequestDocument {
    pub schema_version: String,
    pub domain_pack_learning_review_request: DomainPackLearningReviewRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningReviewRequest {
    pub request_id: StableId,
    pub authority: DomainPackReviewRequestAuthority,
    pub dossier_digest: String,
    pub conflict_digests: Vec<String>,
    pub required_roles: Vec<DomainPackReviewerRole>,
    pub minimum_independent_reviews: u16,
    pub reason: DomainPackReviewRequestReason,
    pub status: DomainPackReviewRequestStatus,
    pub resulting_review_digests: Vec<String>,
    pub requested_at_unix: u64,
    pub request_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewRequestAuthority {
    RequestOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewRequestReason {
    PromotionBoundary,
    LearningConflict,
    CompatibilityChange,
    SafetyCriticalChange,
    Deprecation,
    Revocation,
    Supersession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewRequestStatus {
    Pending,
    Satisfied,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionDecisionDocument {
    pub schema_version: String,
    pub domain_pack_promotion_decision: DomainPackPromotionDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionDecision {
    pub decision_id: StableId,
    pub authority: DomainPackPromotionDecisionAuthority,
    pub dossier_digest: String,
    pub transition: DomainPackPromotionTransition,
    pub decision: DomainPackPromotionDecisionKind,
    pub independent_review_digests: Vec<String>,
    pub resolved_conflict_digests: Vec<String>,
    pub registry_predecessor_digest: String,
    /// Canonical proposed-registry commitment with only circular provenance
    /// backlinks and derived entry digests removed. Package, transition,
    /// compatibility, and independent-review provenance remain committed.
    pub proposed_registry_digest: String,
    pub rationale: String,
    pub decided_at_unix: u64,
    pub decision_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionDecisionAuthority {
    CandidateDecisionOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionDecisionKind {
    Approve,
    Reject,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewerRegistryDocument {
    pub schema_version: String,
    pub domain_pack_reviewer_registry: DomainPackReviewerRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewerRegistry {
    pub registry_id: StableId,
    pub audience: String,
    pub generation: u64,
    pub previous_registry_digest: Option<String>,
    /// Operator trust policy used for genesis and for validating the anchored
    /// predecessor that authorizes every later rotation.
    pub trust_policy_digest: String,
    pub signature_threshold: u16,
    pub reviewers: Vec<DomainPackReviewerRegistryEntry>,
    pub rotation_signatures: Vec<DomainPackReviewerRegistrySignature>,
    pub registry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewerRegistryEntry {
    pub reviewer_id: PrincipalId,
    pub credential_id: StableId,
    pub public_key_hex: String,
    pub public_key_fingerprint: String,
    pub algorithm: DomainPackPromotionSignatureAlgorithm,
    pub roles: Vec<DomainPackReviewerRole>,
    pub independence_domains: Vec<StableId>,
    pub status: DomainPackReviewerStatus,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
}

/// A genesis registry is signed by the operator trust root. Every successor
/// is signed by credentials resolved only from the already anchored
/// predecessor registry, never from the proposed registry itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewerRegistrySignature {
    pub signer_id: PrincipalId,
    pub credential_id: StableId,
    pub predecessor_registry_digest: Option<String>,
    pub payload_digest: String,
    pub algorithm: DomainPackPromotionSignatureAlgorithm,
    pub signature: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewerStatus {
    Active,
    Suspended,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionAuthorizationDocument {
    pub schema_version: String,
    pub domain_pack_promotion_authorization: DomainPackPromotionAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionAuthorization {
    pub authority: DomainPackPromotionAuthorizationAuthority,
    pub payload: DomainPackPromotionAuthorizationPayload,
    pub signatures: Vec<DomainPackPromotionSignature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionAuthorizationAuthority {
    CandidateAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionAuthorizationPayload {
    pub authorization_id: StableId,
    pub dossier_digest: String,
    pub decision_digest: String,
    pub independent_review_digests: Vec<String>,
    pub reviewer_registry_digest: String,
    pub current_reviewed_registry_digest: String,
    /// Same non-circular proposed-registry commitment carried by the exact
    /// promotion decision. The final registry separately carries its own
    /// canonical digest including the resulting provenance backlinks.
    pub proposed_reviewed_registry_digest: String,
    pub transition: DomainPackPromotionTransition,
    pub audience: String,
    pub domain: String,
    pub nonce: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionSignature {
    pub reviewer_id: PrincipalId,
    pub credential_id: StableId,
    pub role: DomainPackReviewerRole,
    pub algorithm: DomainPackPromotionSignatureAlgorithm,
    pub payload_digest: String,
    pub signature: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionSignatureAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedRegistryDocument {
    pub schema_version: String,
    pub domain_pack_reviewed_registry: DomainPackReviewedRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedRegistry {
    pub registry_id: StableId,
    pub audience: String,
    pub generation: u64,
    pub previous_registry_digest: Option<String>,
    pub entries: Vec<DomainPackReviewedRegistryEntry>,
    pub snapshot_signatures: Vec<DomainPackReviewedRegistrySignature>,
    pub registry_digest: String,
}

/// Fresh-verifiable signatures over the exact reviewed-registry snapshot.
/// Exact anchored replay verifies these signatures without replaying the
/// complete historical promotion dossier graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedRegistrySignature {
    pub reviewer_id: PrincipalId,
    pub credential_id: StableId,
    pub role: DomainPackReviewerRole,
    pub algorithm: DomainPackPromotionSignatureAlgorithm,
    pub payload_digest: String,
    pub signature: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedRegistryEntry {
    pub pack: DomainPackVersionReference,
    pub package_digest: String,
    pub supply_chain_record_digest: String,
    pub manifest_digest: String,
    pub content_digest: String,
    pub license_digest: String,
    pub fixture_digests: Vec<String>,
    pub stage: DomainPackPromotionStage,
    pub eligibility: DomainPackReviewedEligibility,
    /// Exact canonical promotion-decision digest.
    pub promotion_decision_digest: String,
    /// Exact digest of the signed authorization payload (not of the envelope
    /// containing signatures), avoiding any signature or registry cycle.
    pub authorization_digest: String,
    pub independent_review_digests: Vec<String>,
    pub compatibility: DomainPackReviewedCompatibility,
    pub deprecation: Option<DomainPackDeprecationBinding>,
    pub revocation: Option<DomainPackRevocationBinding>,
    pub supersession: Option<DomainPackSupersessionBinding>,
    pub entry_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewedEligibility {
    EligibleReviewed,
    IneligibleDeprecated,
    IneligibleRevoked,
    IneligibleSuperseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedCompatibility {
    pub forge_core_requirement: String,
    pub pack_schema_requirement: String,
    pub evaluator_protocol_versions: Vec<String>,
    pub predecessor_content_digests: Vec<String>,
    pub breaking_change: bool,
    pub migration_evidence_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDeprecationBinding {
    pub reason: String,
    pub announced_at_unix: u64,
    pub removal_after_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRevocationBinding {
    pub reason: String,
    pub effective_at_unix: u64,
    pub authorization_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackSupersessionBinding {
    pub replacement_pack: DomainPackVersionReference,
    pub replacement_package_digest: String,
    pub authorization_digest: String,
}

impl DomainPackLocalLearningCandidateDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let candidate = &self.domain_pack_local_learning_candidate;
        required(
            &mut issues,
            "candidate.candidate_id",
            &candidate.candidate_id.0,
        );
        required(&mut issues, "candidate.assertion", &candidate.assertion);
        validate_target(&mut issues, "candidate.target", &candidate.target);
        validate_provenance(&mut issues, &candidate.provenance);
        bounded_nonempty(
            &mut issues,
            "candidate.evidence",
            &candidate.evidence,
            MAX_LEARNING_EVIDENCE,
        );
        validate_evidence(&mut issues, "candidate.evidence", &candidate.evidence);
        digest(
            &mut issues,
            "candidate.candidate_digest",
            &candidate.candidate_digest,
        );
        issues
    }
}

impl DomainPackResolutionProjectionDocument {
    /// Validate the structural P6b evidence required by the reviewed join.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::UnsupportedSchemaVersion,
                "resolution.schema_version",
                "unsupported Domain Pack lifecycle schema version",
            );
        }
        let projection = &self.domain_pack_resolution_projection;
        required(
            &mut issues,
            "resolution.request_id",
            &projection.request_id.0,
        );
        if projection.status != DomainPackResolutionStatus::Resolved {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::CrossReferenceMismatch,
                "resolution.status",
                "only a resolved projection can enter the reviewed-registry join",
            );
        }
        if projection.selected.len() > MAX_REVIEWED_REGISTRY_ENTRIES {
            limit(
                &mut issues,
                "resolution.selected",
                MAX_REVIEWED_REGISTRY_ENTRIES,
            );
        }
        let mut identities = BTreeSet::new();
        for (index, selected) in projection.selected.iter().enumerate() {
            let path = format!("resolution.selected[{index}]");
            required(
                &mut issues,
                &format!("{path}.identity.publisher"),
                &selected.identity.publisher.0,
            );
            required(
                &mut issues,
                &format!("{path}.identity.name"),
                &selected.identity.name.0,
            );
            required(
                &mut issues,
                &format!("{path}.identity.version"),
                &selected.identity.version,
            );
            for (field, value) in [
                ("package.package_digest", &selected.package.package_digest),
                ("registry_record_digest", &selected.registry_record_digest),
                (
                    "package.manifest.raw_sha256",
                    &selected.package.manifest.raw_sha256,
                ),
                (
                    "package.manifest.canonical_sha256",
                    &selected.package.manifest.canonical_sha256,
                ),
                (
                    "package.content.raw_sha256",
                    &selected.package.content.raw_sha256,
                ),
                (
                    "package.content.canonical_sha256",
                    &selected.package.content.canonical_sha256,
                ),
                (
                    "package.license.raw_sha256",
                    &selected.package.license.raw_sha256,
                ),
                (
                    "package.license.canonical_sha256",
                    &selected.package.license.canonical_sha256,
                ),
            ] {
                supply_chain_digest(&mut issues, &format!("{path}.{field}"), value);
            }
            for (fixture_index, fixture) in selected.package.fixtures.iter().enumerate() {
                supply_chain_digest(
                    &mut issues,
                    &format!("{path}.package.fixtures[{fixture_index}].raw_sha256"),
                    &fixture.raw_sha256,
                );
                supply_chain_digest(
                    &mut issues,
                    &format!("{path}.package.fixtures[{fixture_index}].canonical_sha256"),
                    &fixture.canonical_sha256,
                );
            }
            let identity = (
                &selected.identity.publisher.0,
                &selected.identity.name.0,
                &selected.identity.version,
                &selected.package.package_digest,
            );
            if !identities.insert(identity) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &path,
                    "duplicate exact selected package",
                );
            }
        }
        supply_chain_digest(
            &mut issues,
            "resolution.resolution_digest",
            &projection.resolution_digest,
        );
        issues
    }
}

impl DomainPackPromotionDossierDocument {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let dossier = &self.domain_pack_promotion_dossier;
        required(&mut issues, "dossier.dossier_id", &dossier.dossier_id.0);
        validate_version_ref(&mut issues, "dossier.pack", &dossier.pack);
        for (path, value) in [
            ("dossier.package_digest", &dossier.package_digest),
            ("dossier.manifest_digest", &dossier.manifest_digest),
            ("dossier.content_digest", &dossier.content_digest),
            ("dossier.license_digest", &dossier.license_digest),
        ] {
            supply_chain_digest(&mut issues, path, value);
        }
        digest(
            &mut issues,
            "dossier.dossier_digest",
            &dossier.dossier_digest,
        );
        if !dossier.transition.is_allowed() {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::InvalidStageTransition,
                "dossier.transition",
                "transition is not permitted by the closed promotion graph",
            );
        }
        bounded_nonempty(
            &mut issues,
            "dossier.candidate_digests",
            &dossier.candidate_digests,
            MAX_LEARNING_EVIDENCE,
        );
        digest_list(
            &mut issues,
            "dossier.candidate_digests",
            &dossier.candidate_digests,
        );
        bounded_nonempty(
            &mut issues,
            "dossier.evidence",
            &dossier.evidence,
            MAX_LEARNING_EVIDENCE,
        );
        validate_evidence(&mut issues, "dossier.evidence", &dossier.evidence);
        bounded_nonempty(
            &mut issues,
            "dossier.evaluator_runs",
            &dossier.evaluator_runs,
            MAX_LEARNING_EVIDENCE,
        );
        for (index, run) in dossier.evaluator_runs.iter().enumerate() {
            validate_evaluator_run(
                &mut issues,
                &format!("dossier.evaluator_runs[{index}]"),
                run,
            );
        }
        bounded_nonempty(
            &mut issues,
            "dossier.fixture_bindings",
            &dossier.fixture_bindings,
            MAX_LEARNING_FIXTURES,
        );
        for (index, fixture) in dossier.fixture_bindings.iter().enumerate() {
            validate_fixture(
                &mut issues,
                &format!("dossier.fixture_bindings[{index}]"),
                fixture,
            );
        }
        if dossier.provenance.authored_by.is_empty() {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingRequiredValue,
                "dossier.provenance.authored_by",
                "at least one author is required",
            );
        }
        for (path, value) in [
            (
                "dossier.provenance.source_repository",
                &dossier.provenance.source_repository,
            ),
            (
                "dossier.provenance.source_revision",
                &dossier.provenance.source_revision,
            ),
        ] {
            required(&mut issues, path, value);
        }
        for (path, value) in [
            (
                "dossier.provenance.source_tree_digest",
                &dossier.provenance.source_tree_digest,
            ),
            (
                "dossier.provenance.build_recipe_digest",
                &dossier.provenance.build_recipe_digest,
            ),
        ] {
            digest(&mut issues, path, value);
        }
        if matches!(dossier.transition.to, DomainPackPromotionStage::Reviewed)
            && !dossier.evaluator_runs.iter().any(|run| {
                matches!(
                    run.comparison.method,
                    DomainPackLearningComparisonMethod::Ablation
                        | DomainPackLearningComparisonMethod::StrongJudge
                )
            })
        {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingDurableEvidence,
                "dossier.evaluator_runs",
                "reviewed promotion requires an ablation or strong-judge comparison",
            );
        }
        issues
    }
}

impl DomainPackIndependentReviewDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let review = &self.domain_pack_independent_review;
        required(&mut issues, "review.review_id", &review.review_id.0);
        required(&mut issues, "review.reviewer_id", &review.reviewer_id.0);
        required(&mut issues, "review.credential_id", &review.credential_id.0);
        for (path, value) in [
            ("review.dossier_digest", &review.dossier_digest),
            (
                "review.reviewer_registry_digest",
                &review.reviewer_registry_digest,
            ),
            (
                "review.signed_subject_digest",
                &review.signed_subject_digest,
            ),
            ("review.review_digest", &review.review_digest),
        ] {
            digest(&mut issues, path, value);
        }
        if review.issued_at_unix >= review.expires_at_unix {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::InvalidTimeWindow,
                "review.expires_at_unix",
                "review validity window must be increasing",
            );
        }
        if matches!(review.decision, DomainPackReviewDecision::Approve)
            && !matches!(
                review.independence,
                DomainPackReviewerIndependence::Independent { .. }
            )
        {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "review.independence",
                "an approving review must be independently attested",
            );
        }
        if review.findings.len() > MAX_LEARNING_FINDINGS {
            limit(&mut issues, "review.findings", MAX_LEARNING_FINDINGS);
        }
        for (index, finding) in review.findings.iter().enumerate() {
            required(
                &mut issues,
                &format!("review.findings[{index}].finding_id"),
                &finding.finding_id.0,
            );
            required(
                &mut issues,
                &format!("review.findings[{index}].message"),
                &finding.message,
            );
            if matches!(review.decision, DomainPackReviewDecision::Approve)
                && matches!(finding.severity, DomainPackReviewFindingSeverity::Blocking)
                && matches!(
                    finding.disposition,
                    DomainPackReviewFindingDisposition::Open
                )
            {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::MissingIndependentReview,
                    &format!("review.findings[{index}]"),
                    "approving review cannot retain an open blocking finding",
                );
            }
        }
        issues
    }
}

impl DomainPackLearningConflictDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let conflict = &self.domain_pack_learning_conflict;
        required(&mut issues, "conflict.conflict_id", &conflict.conflict_id.0);
        validate_target(&mut issues, "conflict.target", &conflict.target);
        bounded_nonempty(
            &mut issues,
            "conflict.subject_digests",
            &conflict.subject_digests,
            MAX_LEARNING_CONFLICTS,
        );
        digest_list(
            &mut issues,
            "conflict.subject_digests",
            &conflict.subject_digests,
        );
        digest(
            &mut issues,
            "conflict.conflict_digest",
            &conflict.conflict_digest,
        );
        validate_conflict_content_digest(&mut issues, self);
        match conflict.status {
            DomainPackLearningConflictStatus::Open
            | DomainPackLearningConflictStatus::ReviewRequested => {
                if conflict.review_request_digest.is_none() {
                    add(
                        &mut issues,
                        DomainPackLearningContractIssueCode::UnresolvedConflict,
                        "conflict.review_request_digest",
                        "an unresolved conflict requires an explicit review request",
                    );
                }
                if conflict.resolution.is_some() {
                    add(
                        &mut issues,
                        DomainPackLearningContractIssueCode::CrossReferenceMismatch,
                        "conflict.resolution",
                        "an unresolved conflict cannot carry a final resolution",
                    );
                }
            }
            DomainPackLearningConflictStatus::Resolved => {
                if let Some(resolution) = &conflict.resolution {
                    required(
                        &mut issues,
                        "conflict.resolution.rationale",
                        &resolution.rationale,
                    );
                    bounded_nonempty(
                        &mut issues,
                        "conflict.resolution.evidence_refs",
                        &resolution.evidence_refs,
                        MAX_LEARNING_EVIDENCE,
                    );
                    for (index, evidence_ref) in resolution.evidence_refs.iter().enumerate() {
                        if !conflict.evidence_refs.contains(evidence_ref) {
                            add(
                                &mut issues,
                                DomainPackLearningContractIssueCode::CrossReferenceMismatch,
                                &format!("conflict.resolution.evidence_refs[{index}]"),
                                "resolution evidence must be bound by the conflict record",
                            );
                        }
                    }
                    bounded_nonempty(
                        &mut issues,
                        "conflict.resolution.resolved_by_review_digests",
                        &resolution.resolved_by_review_digests,
                        MAX_LEARNING_REVIEWS,
                    );
                    digest_list(
                        &mut issues,
                        "conflict.resolution.resolved_by_review_digests",
                        &resolution.resolved_by_review_digests,
                    );
                } else {
                    add(
                        &mut issues,
                        DomainPackLearningContractIssueCode::UnresolvedConflict,
                        "conflict.resolution",
                        "resolved conflict requires durable resolution evidence",
                    );
                }
                if conflict.review_request_digest.is_none() {
                    add(
                        &mut issues,
                        DomainPackLearningContractIssueCode::UnresolvedConflict,
                        "conflict.review_request_digest",
                        "resolved conflict must bind the review request it satisfies",
                    );
                }
            }
            DomainPackLearningConflictStatus::Withdrawn => {}
        }
        if let Some(value) = &conflict.review_request_digest {
            digest(&mut issues, "conflict.review_request_digest", value);
        }
        issues
    }
}

impl DomainPackLearningReviewRequestDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let request = &self.domain_pack_learning_review_request;
        required(
            &mut issues,
            "review_request.request_id",
            &request.request_id.0,
        );
        digest(
            &mut issues,
            "review_request.dossier_digest",
            &request.dossier_digest,
        );
        digest(
            &mut issues,
            "review_request.request_digest",
            &request.request_digest,
        );
        digest_list(
            &mut issues,
            "review_request.conflict_digests",
            &request.conflict_digests,
        );
        if request.required_roles.is_empty() || request.minimum_independent_reviews == 0 {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "review_request.required_roles",
                "at least one role and one independent review are required",
            );
        }
        if usize::from(request.minimum_independent_reviews) > MAX_LEARNING_REVIEWS
            || request.resulting_review_digests.len() > MAX_LEARNING_REVIEWS
        {
            limit(&mut issues, "review_request.reviews", MAX_LEARNING_REVIEWS);
        }
        digest_list(
            &mut issues,
            "review_request.resulting_review_digests",
            &request.resulting_review_digests,
        );
        if matches!(request.status, DomainPackReviewRequestStatus::Satisfied)
            && request.resulting_review_digests.len()
                < usize::from(request.minimum_independent_reviews)
        {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "review_request.resulting_review_digests",
                "satisfied request lacks its minimum independent review count",
            );
        }
        issues
    }
}

impl DomainPackPromotionDecisionDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let decision = &self.domain_pack_promotion_decision;
        required(
            &mut issues,
            "promotion_decision.decision_id",
            &decision.decision_id.0,
        );
        for (path, value) in [
            (
                "promotion_decision.dossier_digest",
                &decision.dossier_digest,
            ),
            (
                "promotion_decision.registry_predecessor_digest",
                &decision.registry_predecessor_digest,
            ),
            (
                "promotion_decision.proposed_registry_digest",
                &decision.proposed_registry_digest,
            ),
            (
                "promotion_decision.decision_digest",
                &decision.decision_digest,
            ),
        ] {
            digest(&mut issues, path, value);
        }
        if !decision.transition.is_allowed() {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::InvalidStageTransition,
                "promotion_decision.transition",
                "transition is not permitted by the closed promotion graph",
            );
        }
        digest_list(
            &mut issues,
            "promotion_decision.independent_review_digests",
            &decision.independent_review_digests,
        );
        digest_list(
            &mut issues,
            "promotion_decision.resolved_conflict_digests",
            &decision.resolved_conflict_digests,
        );
        if matches!(decision.decision, DomainPackPromotionDecisionKind::Approve)
            && decision.independent_review_digests.is_empty()
        {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "promotion_decision.independent_review_digests",
                "approved transition requires independent review evidence",
            );
        }
        issues
    }
}

impl DomainPackReviewerRegistryDocument {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let registry = &self.domain_pack_reviewer_registry;
        validate_registry_head(
            &mut issues,
            registry.generation,
            registry.previous_registry_digest.as_deref(),
            &registry.registry_id.0,
            &registry.audience,
            &registry.registry_digest,
        );
        digest(
            &mut issues,
            "reviewer_registry.trust_policy_digest",
            &registry.trust_policy_digest,
        );
        if registry.signature_threshold == 0
            || usize::from(registry.signature_threshold) > registry.rotation_signatures.len()
        {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "reviewer_registry.signature_threshold",
                "signature threshold must be nonzero and satisfied",
            );
        }
        bounded_nonempty(
            &mut issues,
            "reviewer_registry.reviewers",
            &registry.reviewers,
            MAX_LEARNING_REVIEWS,
        );
        let mut credentials = BTreeSet::new();
        for (index, reviewer) in registry.reviewers.iter().enumerate() {
            let path = format!("reviewer_registry.reviewers[{index}]");
            required(
                &mut issues,
                &format!("{path}.reviewer_id"),
                &reviewer.reviewer_id.0,
            );
            required(
                &mut issues,
                &format!("{path}.credential_id"),
                &reviewer.credential_id.0,
            );
            required(
                &mut issues,
                &format!("{path}.public_key_hex"),
                &reviewer.public_key_hex,
            );
            digest(
                &mut issues,
                &format!("{path}.public_key_fingerprint"),
                &reviewer.public_key_fingerprint,
            );
            if reviewer.roles.is_empty() {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::MissingRequiredValue,
                    &format!("{path}.roles"),
                    "at least one reviewer role is required",
                );
            }
            if reviewer.independence_domains.is_empty() {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::MissingRequiredValue,
                    &format!("{path}.independence_domains"),
                    "at least one independence domain is required",
                );
            }
            if reviewer.valid_from_unix >= reviewer.valid_until_unix {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::InvalidTimeWindow,
                    &format!("{path}.valid_until_unix"),
                    "credential validity window must be increasing",
                );
            }
            if !credentials.insert(&reviewer.credential_id.0) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &format!("{path}.credential_id"),
                    "duplicate reviewer credential",
                );
            }
        }
        let mut signers = BTreeSet::new();
        for (index, signature) in registry.rotation_signatures.iter().enumerate() {
            let path = format!("reviewer_registry.rotation_signatures[{index}]");
            required(
                &mut issues,
                &format!("{path}.signer_id"),
                &signature.signer_id.0,
            );
            required(
                &mut issues,
                &format!("{path}.credential_id"),
                &signature.credential_id.0,
            );
            digest(
                &mut issues,
                &format!("{path}.payload_digest"),
                &signature.payload_digest,
            );
            required(
                &mut issues,
                &format!("{path}.signature"),
                &signature.signature,
            );
            if signature.predecessor_registry_digest.as_deref()
                != registry.previous_registry_digest.as_deref()
            {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::CrossReferenceMismatch,
                    &format!("{path}.predecessor_registry_digest"),
                    "rotation signature must bind the exact registry predecessor",
                );
            }
            if let Some(value) = &signature.predecessor_registry_digest {
                digest(
                    &mut issues,
                    &format!("{path}.predecessor_registry_digest"),
                    value,
                );
            }
            if !signers.insert(&signature.credential_id.0) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &format!("{path}.credential_id"),
                    "rotation signer credentials must be distinct",
                );
            }
        }
        issues
    }
}

impl DomainPackPromotionAuthorizationDocument {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let authorization = &self.domain_pack_promotion_authorization;
        let payload = &authorization.payload;
        required(
            &mut issues,
            "authorization.payload.authorization_id",
            &payload.authorization_id.0,
        );
        for (path, value) in [
            (
                "authorization.payload.dossier_digest",
                &payload.dossier_digest,
            ),
            (
                "authorization.payload.decision_digest",
                &payload.decision_digest,
            ),
            (
                "authorization.payload.reviewer_registry_digest",
                &payload.reviewer_registry_digest,
            ),
            (
                "authorization.payload.current_reviewed_registry_digest",
                &payload.current_reviewed_registry_digest,
            ),
            (
                "authorization.payload.proposed_reviewed_registry_digest",
                &payload.proposed_reviewed_registry_digest,
            ),
        ] {
            digest(&mut issues, path, value);
        }
        digest_list(
            &mut issues,
            "authorization.payload.independent_review_digests",
            &payload.independent_review_digests,
        );
        for (path, value) in [
            ("authorization.payload.audience", &payload.audience),
            ("authorization.payload.domain", &payload.domain),
            ("authorization.payload.nonce", &payload.nonce),
        ] {
            required(&mut issues, path, value);
        }
        if !payload.transition.is_allowed() {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::InvalidStageTransition,
                "authorization.payload.transition",
                "transition is not permitted by the closed promotion graph",
            );
        }
        if payload.issued_at_unix >= payload.expires_at_unix {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::InvalidTimeWindow,
                "authorization.payload.expires_at_unix",
                "authorization validity window must be increasing",
            );
        }
        if authorization.signatures.len() < 2 {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "authorization.signatures",
                "at least two independent reviewer signatures are required",
            );
        }
        if authorization.signatures.len() > MAX_LEARNING_REVIEWS {
            limit(
                &mut issues,
                "authorization.signatures",
                MAX_LEARNING_REVIEWS,
            );
        }
        let mut reviewers = BTreeSet::new();
        let mut roles = BTreeSet::new();
        for (index, signature) in authorization.signatures.iter().enumerate() {
            let path = format!("authorization.signatures[{index}]");
            required(
                &mut issues,
                &format!("{path}.reviewer_id"),
                &signature.reviewer_id.0,
            );
            required(
                &mut issues,
                &format!("{path}.credential_id"),
                &signature.credential_id.0,
            );
            digest(
                &mut issues,
                &format!("{path}.payload_digest"),
                &signature.payload_digest,
            );
            required(
                &mut issues,
                &format!("{path}.signature"),
                &signature.signature,
            );
            if signature.signed_at_unix < payload.issued_at_unix
                || signature.signed_at_unix > payload.expires_at_unix
            {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::InvalidTimeWindow,
                    &format!("{path}.signed_at_unix"),
                    "signature time must be inside the authorization window",
                );
            }
            if !reviewers.insert(&signature.reviewer_id.0) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &format!("{path}.reviewer_id"),
                    "reviewers must be distinct",
                );
            }
            roles.insert(signature.role);
        }
        if roles.len() < 2 {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "authorization.signatures.role",
                "authorization requires at least two distinct reviewer roles",
            );
        }
        if !roles.contains(&DomainPackReviewerRole::RegistryAuthorizer) {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "authorization.signatures.role",
                "authorization requires an exact registry_authorizer signature",
            );
        }
        if !roles.iter().any(|role| {
            matches!(
                role,
                DomainPackReviewerRole::DomainExpert
                    | DomainPackReviewerRole::EvidenceReviewer
                    | DomainPackReviewerRole::SafetyReviewer
                    | DomainPackReviewerRole::CompatibilityReviewer
            )
        }) {
            add(
                &mut issues,
                DomainPackLearningContractIssueCode::MissingIndependentReview,
                "authorization.signatures.role",
                "authorization requires a semantic reviewer signature",
            );
        }
        issues
    }
}

impl DomainPackReviewedRegistryDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<DomainPackLearningContractIssue> {
        let mut issues = schema_issues(&self.schema_version);
        let registry = &self.domain_pack_reviewed_registry;
        validate_registry_head(
            &mut issues,
            registry.generation,
            registry.previous_registry_digest.as_deref(),
            &registry.registry_id.0,
            &registry.audience,
            &registry.registry_digest,
        );
        if registry.entries.len() > MAX_REVIEWED_REGISTRY_ENTRIES {
            limit(
                &mut issues,
                "reviewed_registry.entries",
                MAX_REVIEWED_REGISTRY_ENTRIES,
            );
        }
        validate_snapshot_signatures(&mut issues, &registry.snapshot_signatures);
        let mut identities = BTreeSet::new();
        let mut coordinate_versions = std::collections::BTreeMap::new();
        let mut entry_digests = BTreeSet::new();
        for (index, entry) in registry.entries.iter().enumerate() {
            let path = format!("reviewed_registry.entries[{index}]");
            validate_registry_entry(&mut issues, &path, entry);
            let identity = (
                &entry.pack.publisher.0,
                &entry.pack.name.0,
                &entry.pack.version,
                &entry.package_digest,
            );
            if !identities.insert(identity) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &path,
                    "duplicate reviewed package identity",
                );
            }
            let coordinate_version = (
                &entry.pack.publisher.0,
                &entry.pack.name.0,
                &entry.pack.version,
            );
            if let Some(existing) =
                coordinate_versions.insert(coordinate_version, &entry.package_digest)
            {
                if existing != &entry.package_digest {
                    add(
                        &mut issues,
                        DomainPackLearningContractIssueCode::DuplicateRecord,
                        &format!("{path}.package_digest"),
                        "one publisher/name/version cannot claim divergent package digests",
                    );
                }
            }
            if !entry_digests.insert(&entry.entry_digest) {
                add(
                    &mut issues,
                    DomainPackLearningContractIssueCode::DuplicateRecord,
                    &format!("{path}.entry_digest"),
                    "duplicate registry entry digest",
                );
            }
        }
        issues
    }
}

fn validate_snapshot_signatures(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    signatures: &[DomainPackReviewedRegistrySignature],
) {
    if signatures.len() < 2 {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingIndependentReview,
            "reviewed_registry.snapshot_signatures",
            "reviewed registry snapshot requires at least two signatures",
        );
    }
    if signatures.len() > MAX_LEARNING_REVIEWS {
        limit(
            issues,
            "reviewed_registry.snapshot_signatures",
            MAX_LEARNING_REVIEWS,
        );
    }
    let mut reviewers = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for (index, signature) in signatures.iter().enumerate() {
        let path = format!("reviewed_registry.snapshot_signatures[{index}]");
        required(
            issues,
            &format!("{path}.reviewer_id"),
            &signature.reviewer_id.0,
        );
        required(
            issues,
            &format!("{path}.credential_id"),
            &signature.credential_id.0,
        );
        digest(
            issues,
            &format!("{path}.payload_digest"),
            &signature.payload_digest,
        );
        required(issues, &format!("{path}.signature"), &signature.signature);
        if !reviewers.insert(&signature.reviewer_id.0) {
            add(
                issues,
                DomainPackLearningContractIssueCode::DuplicateRecord,
                &format!("{path}.reviewer_id"),
                "snapshot signers must be distinct",
            );
        }
        roles.insert(signature.role);
    }
    if !roles.contains(&DomainPackReviewerRole::RegistryAuthorizer) {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingIndependentReview,
            "reviewed_registry.snapshot_signatures.role",
            "snapshot requires a registry_authorizer signature",
        );
    }
    if !roles.iter().any(|role| {
        matches!(
            role,
            DomainPackReviewerRole::DomainExpert
                | DomainPackReviewerRole::EvidenceReviewer
                | DomainPackReviewerRole::SafetyReviewer
                | DomainPackReviewerRole::CompatibilityReviewer
        )
    }) {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingIndependentReview,
            "reviewed_registry.snapshot_signatures.role",
            "snapshot requires a semantic reviewer signature",
        );
    }
}

#[allow(clippy::too_many_lines)]
fn validate_registry_entry(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    entry: &DomainPackReviewedRegistryEntry,
) {
    validate_version_ref(issues, &format!("{path}.pack"), &entry.pack);
    for (field, value) in [
        ("package_digest", &entry.package_digest),
        (
            "supply_chain_record_digest",
            &entry.supply_chain_record_digest,
        ),
        ("manifest_digest", &entry.manifest_digest),
        ("content_digest", &entry.content_digest),
        ("license_digest", &entry.license_digest),
    ] {
        supply_chain_digest(issues, &format!("{path}.{field}"), value);
    }
    for (field, value) in [
        (
            "promotion_decision_digest",
            &entry.promotion_decision_digest,
        ),
        ("authorization_digest", &entry.authorization_digest),
        ("entry_digest", &entry.entry_digest),
    ] {
        digest(issues, &format!("{path}.{field}"), value);
    }
    supply_chain_digest_list(
        issues,
        &format!("{path}.fixture_digests"),
        &entry.fixture_digests,
    );
    if entry.fixture_digests.is_empty() {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingDurableEvidence,
            &format!("{path}.fixture_digests"),
            "reviewed registry entry requires exact fixture digests",
        );
    }
    digest_list(
        issues,
        &format!("{path}.independent_review_digests"),
        &entry.independent_review_digests,
    );
    if entry.independent_review_digests.is_empty() {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingIndependentReview,
            &format!("{path}.independent_review_digests"),
            "reviewed registry entry requires independent review evidence",
        );
    }
    required(
        issues,
        &format!("{path}.compatibility.forge_core_requirement"),
        &entry.compatibility.forge_core_requirement,
    );
    required(
        issues,
        &format!("{path}.compatibility.pack_schema_requirement"),
        &entry.compatibility.pack_schema_requirement,
    );
    supply_chain_digest_list(
        issues,
        &format!("{path}.compatibility.predecessor_content_digests"),
        &entry.compatibility.predecessor_content_digests,
    );
    let state_valid = matches!(
        (entry.stage, entry.eligibility),
        (
            DomainPackPromotionStage::Reviewed,
            DomainPackReviewedEligibility::EligibleReviewed
        ) | (
            DomainPackPromotionStage::Deprecated,
            DomainPackReviewedEligibility::IneligibleDeprecated
        ) | (
            DomainPackPromotionStage::Revoked,
            DomainPackReviewedEligibility::IneligibleRevoked
        ) | (
            DomainPackPromotionStage::Superseded,
            DomainPackReviewedEligibility::IneligibleSuperseded
        )
    );
    if !state_valid {
        add(
            issues,
            DomainPackLearningContractIssueCode::InvalidRegistryEligibility,
            &format!("{path}.eligibility"),
            "registry eligibility must exactly match reviewed lifecycle stage",
        );
    }
    if matches!(
        entry.stage,
        DomainPackPromotionStage::Candidate
            | DomainPackPromotionStage::Trial
            | DomainPackPromotionStage::Validated
    ) {
        add(
            issues,
            DomainPackLearningContractIssueCode::AuthorityEscalation,
            &format!("{path}.stage"),
            "unreviewed stage cannot enter the reviewed registry",
        );
    }
    if matches!(entry.stage, DomainPackPromotionStage::Deprecated) != entry.deprecation.is_some() {
        add(
            issues,
            DomainPackLearningContractIssueCode::CrossReferenceMismatch,
            &format!("{path}.deprecation"),
            "deprecation binding must exist exactly for deprecated entries",
        );
    }
    if matches!(entry.stage, DomainPackPromotionStage::Revoked) != entry.revocation.is_some() {
        add(
            issues,
            DomainPackLearningContractIssueCode::InvalidRevocation,
            &format!("{path}.revocation"),
            "revocation binding must exist exactly for revoked entries",
        );
    }
    if matches!(entry.stage, DomainPackPromotionStage::Superseded) != entry.supersession.is_some() {
        add(
            issues,
            DomainPackLearningContractIssueCode::InvalidSupersession,
            &format!("{path}.supersession"),
            "supersession binding must exist exactly for superseded entries",
        );
    }
    if let Some(binding) = &entry.revocation {
        required(
            issues,
            &format!("{path}.revocation.reason"),
            &binding.reason,
        );
        digest(
            issues,
            &format!("{path}.revocation.authorization_digest"),
            &binding.authorization_digest,
        );
    }
    if let Some(binding) = &entry.supersession {
        validate_version_ref(
            issues,
            &format!("{path}.supersession.replacement_pack"),
            &binding.replacement_pack,
        );
        supply_chain_digest(
            issues,
            &format!("{path}.supersession.replacement_package_digest"),
            &binding.replacement_package_digest,
        );
        digest(
            issues,
            &format!("{path}.supersession.authorization_digest"),
            &binding.authorization_digest,
        );
        if binding.replacement_package_digest == entry.package_digest {
            add(
                issues,
                DomainPackLearningContractIssueCode::InvalidSupersession,
                &format!("{path}.supersession.replacement_package_digest"),
                "supersession replacement must be a different exact package",
            );
        }
    }
}

fn validate_registry_head(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    generation: u64,
    previous: Option<&str>,
    id: &str,
    audience: &str,
    registry_digest: &str,
) {
    required(issues, "registry.registry_id", id);
    required(issues, "registry.audience", audience);
    digest(issues, "registry.registry_digest", registry_digest);
    match (generation, previous) {
        (0, None) => {}
        (0, Some(_)) | (_, None) => add(
            issues,
            DomainPackLearningContractIssueCode::InvalidRegistryChain,
            "registry.previous_registry_digest",
            "generation zero must omit predecessor; later generations must bind one",
        ),
        (_, Some(value)) => digest(issues, "registry.previous_registry_digest", value),
    }
}

fn validate_target(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    target: &DomainPackLearningTarget,
) {
    required(
        issues,
        &format!("{path}.pack.publisher"),
        &target.pack.publisher.0,
    );
    required(issues, &format!("{path}.pack.name"), &target.pack.name.0);
    required(
        issues,
        &format!("{path}.proposed_namespace"),
        &target.proposed_namespace.0,
    );
    if target
        .base_version
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingRequiredValue,
            &format!("{path}.base_version"),
            "base version cannot be blank",
        );
    }
}

fn validate_conflict_content_digest(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    document: &DomainPackLearningConflictDocument,
) {
    match domain_pack_learning_conflict_digest(document) {
        Ok(expected) if document.domain_pack_learning_conflict.conflict_digest == expected => {}
        Ok(_) => add(
            issues,
            DomainPackLearningContractIssueCode::CrossReferenceMismatch,
            "conflict.conflict_digest",
            "conflict digest does not match the canonical record content",
        ),
        Err(error) => add(
            issues,
            DomainPackLearningContractIssueCode::CrossReferenceMismatch,
            "conflict.conflict_digest",
            &format!("conflict digest canonicalization failed: {error}"),
        ),
    }
}

fn validate_version_ref(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    pack: &DomainPackVersionReference,
) {
    required(issues, &format!("{path}.publisher"), &pack.publisher.0);
    required(issues, &format!("{path}.name"), &pack.name.0);
    required(issues, &format!("{path}.version"), &pack.version);
}

fn validate_provenance(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    provenance: &DomainPackLearningProvenance,
) {
    required(
        issues,
        "candidate.provenance.source_ref",
        &provenance.source_ref,
    );
    digest(
        issues,
        "candidate.provenance.source_digest",
        &provenance.source_digest,
    );
    required(
        issues,
        "candidate.provenance.captured_by",
        &provenance.captured_by.0,
    );
    required(
        issues,
        "candidate.provenance.capture_run_id",
        &provenance.capture_run_id.0,
    );
}

fn validate_evidence(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    evidence: &[DomainPackLearningEvidenceBinding],
) {
    let mut ids = BTreeSet::new();
    for (index, item) in evidence.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        required(
            issues,
            &format!("{item_path}.evidence_id"),
            &item.evidence_id.0,
        );
        required(
            issues,
            &format!("{item_path}.artifact.artifact_ref"),
            &item.artifact.artifact_ref.0,
        );
        supply_chain_digest(
            issues,
            &format!("{item_path}.artifact.raw_sha256"),
            &item.artifact.raw_sha256,
        );
        supply_chain_digest(
            issues,
            &format!("{item_path}.artifact.canonical_sha256"),
            &item.artifact.canonical_sha256,
        );
        required(issues, &format!("{item_path}.producer"), &item.producer.0);
        digest(
            issues,
            &format!("{item_path}.provenance_digest"),
            &item.provenance_digest,
        );
        if !ids.insert(&item.evidence_id.0) {
            add(
                issues,
                DomainPackLearningContractIssueCode::DuplicateRecord,
                &format!("{item_path}.evidence_id"),
                "duplicate evidence id",
            );
        }
    }
}

fn validate_evaluator_run(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    run: &DomainPackLearningEvaluatorRun,
) {
    required(issues, &format!("{path}.run_id"), &run.run_id.0);
    required(
        issues,
        &format!("{path}.evaluator_ref"),
        &run.evaluator_ref.0,
    );
    required(
        issues,
        &format!("{path}.evaluator_principal"),
        &run.evaluator_principal.0,
    );
    required(
        issues,
        &format!("{path}.protocol_version"),
        &run.protocol_version,
    );
    required(issues, &format!("{path}.evidence_ref"), &run.evidence_ref.0);
    for (field, value) in [
        ("evaluator_digest", &run.evaluator_digest),
        ("fixture_set_digest", &run.fixture_set_digest),
        ("run_digest", &run.run_digest),
        (
            "comparison.baseline_outcome_digest",
            &run.comparison.baseline_outcome_digest,
        ),
        (
            "comparison.candidate_outcome_digest",
            &run.comparison.candidate_outcome_digest,
        ),
    ] {
        digest(issues, &format!("{path}.{field}"), value);
    }
    required(
        issues,
        &format!("{path}.comparison.rationale"),
        &run.comparison.rationale,
    );
    match (run.comparison.method, &run.strong_judge_proof) {
        (DomainPackLearningComparisonMethod::StrongJudge, Some(proof)) => {
            required(
                issues,
                &format!("{path}.strong_judge_proof.judge_principal"),
                &proof.judge_principal.0,
            );
            required(
                issues,
                &format!("{path}.strong_judge_proof.independence_domain"),
                &proof.independence_domain.0,
            );
            if !proof.blind_ab {
                add(
                    issues,
                    DomainPackLearningContractIssueCode::MissingIndependentReview,
                    &format!("{path}.strong_judge_proof.blind_ab"),
                    "strong-judge comparison must be blind A/B",
                );
            }
            for (field, value) in [
                (
                    "deterministic_order_digest",
                    &proof.deterministic_order_digest,
                ),
                ("rubric_digest", &proof.rubric_digest),
                ("model_digest", &proof.model_digest),
                ("prompt_digest", &proof.prompt_digest),
                ("input_digest", &proof.input_digest),
                ("output_digest", &proof.output_digest),
            ] {
                digest(issues, &format!("{path}.strong_judge_proof.{field}"), value);
            }
        }
        (DomainPackLearningComparisonMethod::StrongJudge, None) => add(
            issues,
            DomainPackLearningContractIssueCode::MissingDurableEvidence,
            &format!("{path}.strong_judge_proof"),
            "strong-judge method requires a machine-checkable proof",
        ),
        (_, Some(_)) => add(
            issues,
            DomainPackLearningContractIssueCode::CrossReferenceMismatch,
            &format!("{path}.strong_judge_proof"),
            "strong-judge proof is only valid for the strong-judge method",
        ),
        (_, None) => {}
    }
}

fn validate_fixture(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    fixture: &DomainPackLearningFixtureBinding,
) {
    required(issues, &format!("{path}.fixture_id"), &fixture.fixture_id.0);
    required(
        issues,
        &format!("{path}.fixture_ref"),
        &fixture.fixture_ref.0,
    );
    required(issues, &format!("{path}.producer"), &fixture.producer.0);
    for (field, value) in [
        ("raw_sha256", &fixture.raw_sha256),
        ("canonical_sha256", &fixture.canonical_sha256),
    ] {
        supply_chain_digest(issues, &format!("{path}.{field}"), value);
    }
    for (field, value) in [
        ("expected_outcome_digest", &fixture.expected_outcome_digest),
        ("provenance_digest", &fixture.provenance_digest),
    ] {
        digest(issues, &format!("{path}.{field}"), value);
    }
}

fn schema_issues(version: &str) -> Vec<DomainPackLearningContractIssue> {
    let mut issues = Vec::new();
    if version != DOMAIN_PACK_LEARNING_SCHEMA_VERSION {
        add(
            &mut issues,
            DomainPackLearningContractIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            "unsupported schema version",
        );
    }
    issues
}

fn bounded_nonempty<T>(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    values: &[T],
    max: usize,
) {
    if values.is_empty() {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingRequiredValue,
            path,
            "at least one value is required",
        );
    }
    if values.len() > max {
        limit(issues, path, max);
    }
}

fn digest_list(issues: &mut Vec<DomainPackLearningContractIssue>, path: &str, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        digest(issues, &format!("{path}[{index}]"), value);
    }
}

fn supply_chain_digest_list(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    path: &str,
    values: &[String],
) {
    for (index, value) in values.iter().enumerate() {
        supply_chain_digest(issues, &format!("{path}[{index}]"), value);
    }
}

fn required(issues: &mut Vec<DomainPackLearningContractIssue>, path: &str, value: &str) {
    if value.trim().is_empty() {
        add(
            issues,
            DomainPackLearningContractIssueCode::MissingRequiredValue,
            path,
            "value cannot be blank",
        );
    }
}

fn digest(issues: &mut Vec<DomainPackLearningContractIssue>, path: &str, value: &str) {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        add(
            issues,
            DomainPackLearningContractIssueCode::InvalidDigest,
            path,
            "expected 64 hexadecimal SHA-256 characters",
        );
    }
}

fn supply_chain_digest(issues: &mut Vec<DomainPackLearningContractIssue>, path: &str, value: &str) {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        add(
            issues,
            DomainPackLearningContractIssueCode::InvalidDigest,
            path,
            "expected sha256: plus 64 lowercase hexadecimal characters",
        );
    }
}

fn limit(issues: &mut Vec<DomainPackLearningContractIssue>, path: &str, max: usize) {
    add(
        issues,
        DomainPackLearningContractIssueCode::ResourceLimitExceeded,
        path,
        &format!("value count exceeds maximum {max}"),
    );
}

fn add(
    issues: &mut Vec<DomainPackLearningContractIssue>,
    code: DomainPackLearningContractIssueCode,
    path: &str,
    message: &str,
) {
    issues.push(DomainPackLearningContractIssue {
        code,
        path: path.to_owned(),
        message: message.to_owned(),
    });
}
