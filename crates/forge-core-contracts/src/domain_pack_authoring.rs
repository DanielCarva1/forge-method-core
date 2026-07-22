//! Closed C7.1 authoring contracts for candidate-only Domain Pack work.
//!
//! The types in this module carry draft bytes and deterministic diagnostic
//! evidence. They deliberately do not model acquisition, signing, publishing,
//! trust provisioning, lifecycle apply, commit, installation, or activation.

use crate::{
    DomainPackCandidateAuthority, DomainPackCandidateInput, DomainPackCompositionRequestDocument,
    DomainPackCoreBinding, DomainPackExactLockDocument, DomainPackIndependentReviewDocument,
    DomainPackLearningConflictDocument, DomainPackLocalLearningCandidateDocument,
    DomainPackProjectRequirementsDocument, DomainPackPromotionDossierDocument,
    DomainPackReviewedRegistryDocument, DomainPackVersionReference, RepoPath, StableId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN_PACK_AUTHORING_SCHEMA_VERSION: &str = "0.1";

/// Closed input for deterministic generic skeleton generation. The source,
/// authorship, license, requirements, and sealed Core binding are explicit so
/// a generated template never relies on ambient repository state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorSkeletonRequestDocument {
    pub schema_version: String,
    pub domain_pack_author_skeleton_request: DomainPackAuthorSkeletonRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorSkeletonRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub publisher: StableId,
    pub name: StableId,
    pub namespace: StableId,
    pub version: String,
    pub forge_core_version: String,
    pub core: DomainPackCoreBinding,
    pub requirements: DomainPackProjectRequirementsDocument,
    pub provenance: DomainPackAuthorProvenanceTemplate,
    pub artifact_refs: DomainPackAuthorArtifactRefs,
}

/// Explicit metadata used in a minimal generic manifest. These are authored
/// placeholders, not a signature, publisher assertion, or registry record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorProvenanceTemplate {
    pub source_kind: crate::DomainPackSourceKind,
    pub source_uri: String,
    pub source_revision: String,
    pub source_digest: String,
    pub authors: Vec<StableId>,
    pub license_spdx_expression: String,
}

/// Logical candidate locations only. A later CLI adapter may choose where to
/// write the returned bytes, but this pure layer cannot write them itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorArtifactRefs {
    pub manifest_ref: RepoPath,
    pub content_ref: RepoPath,
    pub license_ref: RepoPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorSkeletonStatus {
    Generated,
    Blocked,
}

/// Output of skeleton generation. The artifact byte fields are intended for a
/// CLI/file adapter; their bindings make the template immediately usable as a
/// candidate input without the generator performing I/O.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorSkeletonDocument {
    pub schema_version: String,
    pub domain_pack_author_skeleton: DomainPackAuthorSkeleton,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorSkeleton {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub status: DomainPackAuthorSkeletonStatus,
    pub template: Option<DomainPackAuthorPackTemplate>,
    pub issues: Vec<DomainPackAuthorIssue>,
    pub skeleton_digest: String,
}

/// Minimal, generic authoring material. Empty dependency/conflict/replacement
/// and contribution vectors are intentional editable placeholders; callers
/// must add meaningful domain content before a non-empty requirement can pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorPackTemplate {
    pub candidate: DomainPackCandidateInput,
    pub manifest: DomainPackAuthorRawArtifactTemplate,
    pub content: DomainPackAuthorRawContentTemplate,
    pub license: DomainPackAuthorRawArtifactTemplate,
    pub requirements: DomainPackProjectRequirementsDocument,
    pub composition_request: DomainPackCompositionRequestDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorRawArtifactTemplate {
    pub artifact_ref: RepoPath,
    pub raw_sha256: String,
    pub canonical_sha256: String,
    pub raw_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorRawContentTemplate {
    pub content_ref: RepoPath,
    pub raw_sha256: String,
    pub canonical_sha256: String,
    pub raw_bytes: Vec<u8>,
}

/// Caller-supplied exact byte sidecars for one typed candidate. They remain
/// untrusted input and are checked again by the existing candidate validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorRawSidecars {
    pub pack: DomainPackVersionReference,
    pub manifest_raw: Vec<u8>,
    pub content_raw: Vec<u8>,
    pub license_raw: Vec<u8>,
}

/// Read-only exact-lock comparison input. `operation` selects the existing
/// compatibility evaluator's comparison policy only; it is not a lifecycle
/// request and grants no apply or commit authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorExactLockComparison {
    pub comparison_id: StableId,
    pub operation: crate::DomainPackLifecycleOperation,
    pub sealed_core: DomainPackCoreBinding,
    pub current_lock: Option<DomainPackExactLockDocument>,
    pub proposed_lock: DomainPackExactLockDocument,
}

/// Optional durable evidence supplied to the existing promotion-readiness
/// evaluator. Serialization preserves evidence only; it is never promotion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorLearningEvidence {
    pub dossier: DomainPackPromotionDossierDocument,
    pub candidates: Vec<DomainPackLocalLearningCandidateDocument>,
    pub independent_reviews: Vec<DomainPackIndependentReviewDocument>,
    pub conflicts: Vec<DomainPackLearningConflictDocument>,
}

/// Optional reviewed-registry material for semantic evolution readiness. The
/// current/proposed snapshots remain candidate evidence and are not anchored by
/// this author workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorReviewedRegistryEvidence {
    pub current: Option<DomainPackReviewedRegistryDocument>,
    pub proposed: DomainPackReviewedRegistryDocument,
    pub competing_heads: Vec<DomainPackReviewedRegistryDocument>,
}

/// One author-facing candidate test request. `candidate` is repeated outside
/// the composition request so the author can receive direct sidecar-binding and
/// adversarial diagnostics before reading the full multi-pack projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorTestRequestDocument {
    pub schema_version: String,
    pub domain_pack_author_test_request: DomainPackAuthorTestRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorTestRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub candidate: DomainPackCandidateInput,
    pub composition_request: DomainPackCompositionRequestDocument,
    pub raw_sidecars: Vec<DomainPackAuthorRawSidecars>,
    pub compatibility: Option<DomainPackAuthorExactLockComparison>,
    pub learning: Option<DomainPackAuthorLearningEvidence>,
    pub reviewed_registry: Option<DomainPackAuthorReviewedRegistryEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorCheckKind {
    Structural,
    ArtifactBinding,
    Composition,
    Compatibility,
    LearningReadiness,
    ReviewedRegistryReadiness,
    Adversarial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorCheckStatus {
    Passed,
    Failed,
    NotSupplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorCheck {
    pub kind: DomainPackAuthorCheckKind,
    pub status: DomainPackAuthorCheckStatus,
    pub issues: Vec<DomainPackAuthorIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorCompatibilityStatus {
    Compatible,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorCompatibilityReadiness {
    pub status: DomainPackAuthorCompatibilityStatus,
    pub report_digest: String,
    pub issues: Vec<DomainPackAuthorIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorPromotionReadiness {
    ReadyForReview,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorLearningReadiness {
    pub status: DomainPackAuthorPromotionReadiness,
    pub evaluation_digest: String,
    pub review_request_digest: Option<String>,
    pub issues: Vec<DomainPackAuthorIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorReviewedRegistryReadinessStatus {
    AdmissibleCandidate,
    GenesisCandidate,
    Replay,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorReviewedRegistryReadiness {
    pub status: DomainPackAuthorReviewedRegistryReadinessStatus,
    pub evaluation_digest: String,
    pub issues: Vec<DomainPackAuthorIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorTestStatus {
    CandidateReady,
    Blocked,
}

/// Stable author-facing diagnostic vocabulary. The workflow maps diagnostics
/// from the established P6a/P6b/P6c evaluators into these closed categories
/// without converting any of them into authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAuthorIssueCode {
    InvalidAuthorContract,
    MissingMaterial,
    RawCanonicalMismatch,
    CoordinateVersionMismatch,
    InvalidEvaluatorVariant,
    InvalidAdapterVariant,
    InvalidLifecycleVariant,
    InvalidCapabilityVariant,
    InvalidFixtureVariant,
    DanglingReference,
    DependencyConflict,
    DependencyCycle,
    CoreShadowing,
    PackShadowing,
    NamespaceCollision,
    UnsafePromptOrToolProse,
    IncompatibleCore,
    IncompatibleProjectRequirements,
    MissingDomain,
    MissingCapability,
    RevokedRecord,
    DeprecatedRecord,
    SupersededRecord,
    NoOpComparison,
    RegressingComparison,
    NonIndependentReview,
    MissingIndependentReview,
    RejectedReview,
    UnresolvedConflict,
    ExternalExecutableCapabilityClaim,
    CompatibilityBlocked,
    LearningBlocked,
    ReviewedRegistryBlocked,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorIssue {
    pub code: DomainPackAuthorIssueCode,
    pub path: String,
    pub message: String,
    pub authority: DomainPackCandidateAuthority,
}

impl DomainPackAuthorSkeletonRequestDocument {
    /// Validates the exact, independently supplied sealed Core binding required
    /// before a skeleton can be generated. This is intentionally limited to
    /// binding integrity; composition remains responsible for evaluating the
    /// Core together with candidates and persistent requirements.
    #[must_use]
    pub fn validate_sealed_core_binding(&self) -> Vec<DomainPackAuthorIssue> {
        let core = &self.domain_pack_author_skeleton_request.core;
        let mut issues = Vec::new();
        if core.bundle_id.0.trim().is_empty() || core.bundle.id.0.trim().is_empty() {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::InvalidAuthorContract,
                "core.bundle_id",
                "sealed Core bundle ids must not be blank",
            ));
        }
        if core.bundle_id != core.bundle.id {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::CoreShadowing,
                "core.bundle_id",
                "sealed Core bundle id does not match the embedded Core bundle id",
            ));
        }
        if !authoring_sha256_digest(&core.bundle_digest) {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::RawCanonicalMismatch,
                "core.bundle_digest",
                "sealed Core bundle digest must be sha256: followed by 64 lowercase hexadecimal characters",
            ));
        } else if authoring_canonical_digest(&core.bundle) != core.bundle_digest {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::RawCanonicalMismatch,
                "core.bundle_digest",
                "sealed Core bundle digest does not match the embedded Core bundle",
            ));
        }
        if !authoring_sha256_digest(&core.policy_set_digest) {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::RawCanonicalMismatch,
                "core.policy_set_digest",
                "sealed Core policy-set digest must be sha256: followed by 64 lowercase hexadecimal characters",
            ));
        } else if authoring_canonical_digest(&core.bundle.policies) != core.policy_set_digest {
            issues.push(authoring_core_issue(
                DomainPackAuthorIssueCode::RawCanonicalMismatch,
                "core.policy_set_digest",
                "sealed Core policy-set digest does not match the embedded Core policy set",
            ));
        }
        issues
    }
}

fn authoring_core_issue(
    code: DomainPackAuthorIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> DomainPackAuthorIssue {
    DomainPackAuthorIssue {
        code,
        path: path.into(),
        message: message.into(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    }
}

fn authoring_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn authoring_canonical_digest<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_vec(value).map_or_else(
        |_| {
            format!(
                "sha256:{:x}",
                Sha256::digest(b"forge-domain-pack-authoring-canonical-encoding-failed")
            )
        },
        |bytes| format!("sha256:{:x}", Sha256::digest(bytes)),
    )
}

/// Deterministic, serializable diagnostic evidence. It intentionally contains
/// no command, endpoint, authority grant, active pointer, or lifecycle receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorTestReportDocument {
    pub schema_version: String,
    pub domain_pack_author_test_report: DomainPackAuthorTestReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAuthorTestReport {
    pub report_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub candidate: DomainPackVersionReference,
    pub status: DomainPackAuthorTestStatus,
    pub structural: DomainPackAuthorCheck,
    pub artifact_binding: DomainPackAuthorCheck,
    pub composition: DomainPackAuthorCheck,
    pub compatibility: Option<DomainPackAuthorCompatibilityReadiness>,
    pub learning: Option<DomainPackAuthorLearningReadiness>,
    pub reviewed_registry: Option<DomainPackAuthorReviewedRegistryReadiness>,
    pub adversarial: DomainPackAuthorCheck,
    pub gaps: Vec<DomainPackAuthorIssue>,
    pub issues: Vec<DomainPackAuthorIssue>,
    pub report_digest: String,
}
