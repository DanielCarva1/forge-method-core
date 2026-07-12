//! Closed P5d.4 contracts for independent candidate-release review.
//!
//! These documents bind review inputs, reviewer credentials, and signed
//! authorization proposals. Deserialization and structural validation remain
//! candidate-only: trusted cryptographic and semantic verification is required
//! before a release can become runtime authority.

use std::collections::BTreeSet;

use crate::common::{PrincipalId, RepoPath, StableId};
use crate::workflow_behavior::WorkflowGovernedOutcomeDimension;
use crate::workflow_release::{
    WorkflowGovernanceReleaseIdentity, WorkflowReleasePredecessorReference,
    WorkflowRuntimeBundleIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RELEASE_REVIEWED_WORKFLOW_COUNT: usize = 5;
pub const WORKFLOW_RELEASE_REVIEWED_QUARANTINE_COUNT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewIndexDocument {
    pub schema_version: String,
    pub workflow_release_review_index: WorkflowReleaseReviewIndex,
}

/// A complete, acyclic index of the artifacts and semantic decisions reviewed
/// for one promotion. It is review input, never admission authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewIndex {
    pub id: StableId,
    pub index_version: String,
    pub authority: WorkflowReleaseReviewIndexAuthority,
    pub promotion: WorkflowReleasePromotionBinding,
    pub release_manifest: WorkflowReleaseReviewArtifactBinding,
    pub migration_batches: Vec<WorkflowReleaseReviewArtifactBinding>,
    pub review_subjects: Vec<WorkflowReleaseReviewArtifactBinding>,
    pub coverage_policy: WorkflowReleaseReviewArtifactBinding,
    pub corpus_set: WorkflowReleaseReviewArtifactBinding,
    pub representative_corpus: WorkflowReleaseReviewArtifactBinding,
    pub adversarial_corpus: WorkflowReleaseReviewArtifactBinding,
    pub shadow_report: WorkflowReleaseReviewArtifactBinding,
    pub candidate_runtime_bundle: WorkflowReleaseReviewArtifactBinding,
    pub promoted_runtime_bundle: WorkflowReleaseReviewArtifactBinding,
    pub predecessor_registry: WorkflowReleaseReviewArtifactBinding,
    pub proposed_registry: WorkflowReleaseReviewArtifactBinding,
    pub evaluator_source: WorkflowReleaseReviewArtifactBinding,
    pub frozen_history: WorkflowReleaseReviewArtifactBinding,
    pub workflow_decisions: Vec<WorkflowReleaseReviewWorkflowDecision>,
    pub quarantine_decisions: Vec<WorkflowReleaseReviewQuarantineDecision>,
    pub dimension_decisions: Vec<WorkflowReleaseReviewDimensionDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewIndexAuthority {
    CandidateOnly,
}

/// Both digests are explicit so exact repository bytes can never be confused
/// with the canonical typed semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewArtifactBinding {
    pub artifact_id: StableId,
    pub embedded_ref: RepoPath,
    pub raw_digest: String,
    pub canonical_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleasePromotionBinding {
    pub predecessor: WorkflowReleasePredecessorReference,
    /// Final proposed release identity. It remains a candidate until trusted
    /// authorization verification and atomic kernel admission both succeed.
    pub candidate_release: WorkflowGovernanceReleaseIdentity,
    /// P5d.3 shadow candidate used as independent-review input.
    pub candidate_runtime_bundle: WorkflowRuntimeBundleIdentity,
    /// Final composed runtime bundle proposed for admission.
    pub promoted_runtime_bundle: WorkflowRuntimeBundleIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewDecision {
    Approved,
    Rejected,
    ChangesRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewWorkflowDecision {
    pub workflow_id: StableId,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewQuarantineDecision {
    pub workflow_id: StableId,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewDimensionDecision {
    pub dimension: WorkflowGovernedOutcomeDimension,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewerRegistryDocument {
    pub schema_version: String,
    pub workflow_release_reviewer_registry: WorkflowReleaseReviewerRegistry,
}

/// Repository-owned credential discovery. Only the trusted authority layer may
/// interpret a currently active credential as verified signing authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewerRegistry {
    pub registry_id: StableId,
    pub registry_version: String,
    pub authority: WorkflowReleaseReviewerRegistryAuthority,
    pub credentials: Vec<WorkflowReleaseReviewerCredential>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewerRegistryAuthority {
    CandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewerCredential {
    pub credential_id: StableId,
    pub principal_id: PrincipalId,
    pub public_key_fingerprint: String,
    /// Hex-encoded Ed25519 verifying key. Trusted verification derives and
    /// compares the fingerprint instead of accepting a caller-supplied key.
    pub public_key_hex: String,
    pub algorithm: WorkflowReleaseAdmissionSignatureAlgorithm,
    pub roles: Vec<WorkflowReleaseReviewerRole>,
    pub status: WorkflowReleaseReviewerCredentialStatus,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
    pub independence_domain: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewerRole {
    SemanticReviewer,
    ReleaseAuthorizer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewerCredentialStatus {
    Active,
    Suspended,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorizationDocument {
    pub schema_version: String,
    pub workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorization {
    pub authority: WorkflowReleaseAdmissionAuthorizationAuthority,
    pub payload: WorkflowReleaseAdmissionAuthorizationPayload,
    pub signatures: Vec<WorkflowReleaseAdmissionSignature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionAuthorizationAuthority {
    CandidateAuthorization,
}

/// The signed payload repeats all promotion-critical identities and decisions;
/// a signature over an index path or a passing label alone is insufficient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorizationPayload {
    pub authorization_id: StableId,
    pub review_index_id: StableId,
    pub review_index_version: String,
    pub review_index_raw_digest: String,
    pub review_index_canonical_digest: String,
    pub evaluation_digest: String,
    pub reviewer_registry_id: StableId,
    pub reviewer_registry_version: String,
    pub reviewer_registry_raw_digest: String,
    pub reviewer_registry_canonical_digest: String,
    pub promotion: WorkflowReleasePromotionBinding,
    pub invalidate_all_receipts: bool,
    pub workflow_decisions: Vec<WorkflowReleaseReviewWorkflowDecision>,
    pub quarantine_decisions: Vec<WorkflowReleaseReviewQuarantineDecision>,
    pub dimension_decisions: Vec<WorkflowReleaseReviewDimensionDecision>,
    pub audience: String,
    pub domain: String,
    pub nonce: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionSignature {
    pub principal_id: PrincipalId,
    pub credential_id: StableId,
    pub role: WorkflowReleaseReviewerRole,
    pub algorithm: WorkflowReleaseAdmissionSignatureAlgorithm,
    pub payload_digest: String,
    pub signature: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionSignatureAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowReleaseReviewContractIssue {
    pub path: String,
    pub message: String,
}

impl WorkflowReleaseReviewIndexDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowReleaseReviewContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let index = &self.workflow_release_review_index;
        require_nonblank(&mut issues, "review_index.id", &index.id.0);
        require_nonblank(
            &mut issues,
            "review_index.index_version",
            &index.index_version,
        );
        validate_promotion(&mut issues, "review_index.promotion", &index.promotion);
        validate_artifact(
            &mut issues,
            "review_index.release_manifest",
            &index.release_manifest,
        );
        validate_artifact_set(
            &mut issues,
            "review_index.migration_batches",
            &index.migration_batches,
        );
        validate_artifact_set(
            &mut issues,
            "review_index.review_subjects",
            &index.review_subjects,
        );
        validate_artifact(
            &mut issues,
            "review_index.coverage_policy",
            &index.coverage_policy,
        );
        validate_artifact(&mut issues, "review_index.corpus_set", &index.corpus_set);
        validate_artifact(
            &mut issues,
            "review_index.representative_corpus",
            &index.representative_corpus,
        );
        validate_artifact(
            &mut issues,
            "review_index.adversarial_corpus",
            &index.adversarial_corpus,
        );
        validate_artifact(
            &mut issues,
            "review_index.shadow_report",
            &index.shadow_report,
        );
        for (path, artifact) in [
            (
                "review_index.candidate_runtime_bundle",
                &index.candidate_runtime_bundle,
            ),
            (
                "review_index.promoted_runtime_bundle",
                &index.promoted_runtime_bundle,
            ),
            (
                "review_index.predecessor_registry",
                &index.predecessor_registry,
            ),
            ("review_index.proposed_registry", &index.proposed_registry),
            ("review_index.evaluator_source", &index.evaluator_source),
            ("review_index.frozen_history", &index.frozen_history),
        ] {
            validate_artifact(&mut issues, path, artifact);
        }
        validate_decision_sets(
            &mut issues,
            &index.workflow_decisions,
            &index.quarantine_decisions,
            &index.dimension_decisions,
        );
        issues
    }
}

impl WorkflowReleaseReviewerRegistryDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowReleaseReviewContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let registry = &self.workflow_release_reviewer_registry;
        require_nonblank(
            &mut issues,
            "reviewer_registry.registry_id",
            &registry.registry_id.0,
        );
        require_nonblank(
            &mut issues,
            "reviewer_registry.registry_version",
            &registry.registry_version,
        );
        if registry.credentials.is_empty() {
            issue(
                &mut issues,
                "reviewer_registry.credentials",
                "at least one credential is required",
            );
        }
        let mut credentials = BTreeSet::new();
        for (index, credential) in registry.credentials.iter().enumerate() {
            let path = format!("reviewer_registry.credentials[{index}]");
            require_nonblank(
                &mut issues,
                &format!("{path}.credential_id"),
                &credential.credential_id.0,
            );
            require_nonblank(
                &mut issues,
                &format!("{path}.principal_id"),
                &credential.principal_id.0,
            );
            require_nonblank(
                &mut issues,
                &format!("{path}.public_key_fingerprint"),
                &credential.public_key_fingerprint,
            );
            require_digest(
                &mut issues,
                &format!("{path}.public_key_fingerprint"),
                &credential.public_key_fingerprint,
            );
            require_hex_bytes(
                &mut issues,
                &format!("{path}.public_key_hex"),
                &credential.public_key_hex,
                32,
            );
            require_nonblank(
                &mut issues,
                &format!("{path}.independence_domain"),
                &credential.independence_domain,
            );
            if !credentials.insert(&credential.credential_id.0) {
                issue(
                    &mut issues,
                    &format!("{path}.credential_id"),
                    "duplicate credential id",
                );
            }
            let roles = credential.roles.iter().copied().collect::<BTreeSet<_>>();
            if roles.is_empty() || roles.len() != credential.roles.len() {
                issue(
                    &mut issues,
                    &format!("{path}.roles"),
                    "roles must be nonempty and unique",
                );
            }
            if credential.valid_from_unix >= credential.valid_until_unix {
                issue(
                    &mut issues,
                    &format!("{path}.valid_until_unix"),
                    "credential validity window must be increasing",
                );
            }
        }
        issues
    }
}

impl WorkflowReleaseAdmissionAuthorizationDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowReleaseReviewContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let authorization = &self.workflow_release_admission_authorization;
        let payload = &authorization.payload;
        for (path, value) in [
            (
                "authorization.payload.authorization_id",
                &payload.authorization_id.0,
            ),
            (
                "authorization.payload.review_index_id",
                &payload.review_index_id.0,
            ),
            (
                "authorization.payload.review_index_version",
                &payload.review_index_version,
            ),
            (
                "authorization.payload.reviewer_registry_id",
                &payload.reviewer_registry_id.0,
            ),
            (
                "authorization.payload.reviewer_registry_version",
                &payload.reviewer_registry_version,
            ),
            ("authorization.payload.audience", &payload.audience),
            ("authorization.payload.domain", &payload.domain),
            ("authorization.payload.nonce", &payload.nonce),
        ] {
            require_nonblank(&mut issues, path, value);
        }
        for (path, digest) in [
            (
                "authorization.payload.review_index_raw_digest",
                &payload.review_index_raw_digest,
            ),
            (
                "authorization.payload.review_index_canonical_digest",
                &payload.review_index_canonical_digest,
            ),
            (
                "authorization.payload.evaluation_digest",
                &payload.evaluation_digest,
            ),
            (
                "authorization.payload.reviewer_registry_raw_digest",
                &payload.reviewer_registry_raw_digest,
            ),
            (
                "authorization.payload.reviewer_registry_canonical_digest",
                &payload.reviewer_registry_canonical_digest,
            ),
        ] {
            require_digest(&mut issues, path, digest);
        }
        validate_promotion(
            &mut issues,
            "authorization.payload.promotion",
            &payload.promotion,
        );
        if !payload.invalidate_all_receipts {
            issue(
                &mut issues,
                "authorization.payload.invalidate_all_receipts",
                "P5d.4 admission must invalidate all predecessor receipts",
            );
        }
        validate_decision_sets(
            &mut issues,
            &payload.workflow_decisions,
            &payload.quarantine_decisions,
            &payload.dimension_decisions,
        );
        if payload.issued_at_unix >= payload.expires_at_unix {
            issue(
                &mut issues,
                "authorization.payload.expires_at_unix",
                "authorization validity window must be increasing",
            );
        }
        validate_signatures(&mut issues, &authorization.signatures);
        for (index, signature) in authorization.signatures.iter().enumerate() {
            if signature.signed_at_unix < payload.issued_at_unix
                || signature.signed_at_unix > payload.expires_at_unix
            {
                issue(
                    &mut issues,
                    &format!("authorization.signatures[{index}].signed_at_unix"),
                    "signature time must fall inside the authorization window",
                );
            }
        }
        issues
    }
}

fn validate_artifact_set(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    path: &str,
    artifacts: &[WorkflowReleaseReviewArtifactBinding],
) {
    if artifacts.is_empty() {
        issue(issues, path, "at least one artifact binding is required");
    }
    let mut ids = BTreeSet::new();
    for (index, artifact) in artifacts.iter().enumerate() {
        validate_artifact(issues, &format!("{path}[{index}]"), artifact);
        if !ids.insert(&artifact.artifact_id.0) {
            issue(
                issues,
                &format!("{path}[{index}].artifact_id"),
                "duplicate artifact id",
            );
        }
    }
}

fn validate_artifact(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    path: &str,
    artifact: &WorkflowReleaseReviewArtifactBinding,
) {
    require_nonblank(
        issues,
        &format!("{path}.artifact_id"),
        &artifact.artifact_id.0,
    );
    require_nonblank(
        issues,
        &format!("{path}.embedded_ref"),
        &artifact.embedded_ref.0,
    );
    require_digest(issues, &format!("{path}.raw_digest"), &artifact.raw_digest);
    require_digest(
        issues,
        &format!("{path}.canonical_digest"),
        &artifact.canonical_digest,
    );
}

fn validate_promotion(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    path: &str,
    promotion: &WorkflowReleasePromotionBinding,
) {
    require_nonblank(
        issues,
        &format!("{path}.predecessor.release_id"),
        &promotion.predecessor.release_id.0,
    );
    require_digest(
        issues,
        &format!("{path}.predecessor.release_digest"),
        &promotion.predecessor.release_digest,
    );
    require_nonblank(
        issues,
        &format!("{path}.candidate_release.lineage_id"),
        &promotion.candidate_release.lineage_id.0,
    );
    require_nonblank(
        issues,
        &format!("{path}.candidate_release.release_id"),
        &promotion.candidate_release.release_id.0,
    );
    require_nonblank(
        issues,
        &format!("{path}.candidate_release.release_version"),
        &promotion.candidate_release.release_version,
    );
    require_digest(
        issues,
        &format!("{path}.candidate_release.release_digest"),
        &promotion.candidate_release.release_digest,
    );
    require_nonblank(
        issues,
        &format!("{path}.candidate_runtime_bundle.bundle_id"),
        &promotion.candidate_runtime_bundle.bundle_id.0,
    );
    require_digest(
        issues,
        &format!("{path}.candidate_runtime_bundle.bundle_digest"),
        &promotion.candidate_runtime_bundle.bundle_digest,
    );
    require_digest(
        issues,
        &format!("{path}.candidate_runtime_bundle.policy_set_digest"),
        &promotion.candidate_runtime_bundle.policy_set_digest,
    );
    require_nonblank(
        issues,
        &format!("{path}.promoted_runtime_bundle.bundle_id"),
        &promotion.promoted_runtime_bundle.bundle_id.0,
    );
    require_digest(
        issues,
        &format!("{path}.promoted_runtime_bundle.bundle_digest"),
        &promotion.promoted_runtime_bundle.bundle_digest,
    );
    require_digest(
        issues,
        &format!("{path}.promoted_runtime_bundle.policy_set_digest"),
        &promotion.promoted_runtime_bundle.policy_set_digest,
    );
    if promotion.candidate_runtime_bundle.bundle_id == promotion.promoted_runtime_bundle.bundle_id {
        issue(
            issues,
            &format!("{path}.promoted_runtime_bundle.bundle_id"),
            "promoted runtime bundle must differ from the shadow candidate bundle",
        );
    }
    if promotion.predecessor.release_id == promotion.candidate_release.release_id {
        issue(
            issues,
            &format!("{path}.candidate_release.release_id"),
            "candidate release must differ from predecessor",
        );
    }
}

fn validate_decision_sets(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    workflows: &[WorkflowReleaseReviewWorkflowDecision],
    quarantines: &[WorkflowReleaseReviewQuarantineDecision],
    dimensions: &[WorkflowReleaseReviewDimensionDecision],
) {
    validate_named_decisions(
        issues,
        "workflow_decisions",
        workflows.iter().map(|item| {
            (
                &item.workflow_id,
                item.rationale.as_str(),
                item.finding_refs.as_slice(),
            )
        }),
    );
    validate_named_decisions(
        issues,
        "quarantine_decisions",
        quarantines.iter().map(|item| {
            (
                &item.workflow_id,
                item.rationale.as_str(),
                item.finding_refs.as_slice(),
            )
        }),
    );
    if workflows.len() != WORKFLOW_RELEASE_REVIEWED_WORKFLOW_COUNT {
        issue(
            issues,
            "workflow_decisions",
            "exactly five reviewed workflow decisions are required",
        );
    }
    if quarantines.len() != WORKFLOW_RELEASE_REVIEWED_QUARANTINE_COUNT {
        issue(
            issues,
            "quarantine_decisions",
            "exactly three quarantine decisions are required",
        );
    }
    let workflow_ids = workflows
        .iter()
        .map(|decision| &decision.workflow_id.0)
        .collect::<BTreeSet<_>>();
    for (index, quarantine) in quarantines.iter().enumerate() {
        if workflow_ids.contains(&quarantine.workflow_id.0) {
            issue(
                issues,
                &format!("quarantine_decisions[{index}].workflow_id"),
                "workflow cannot be both a promotion candidate and quarantined",
            );
        }
    }
    let mut seen = BTreeSet::new();
    for (index, item) in dimensions.iter().enumerate() {
        let path = format!("dimension_decisions[{index}]");
        require_nonblank(issues, &format!("{path}.rationale"), &item.rationale);
        let unique_findings = item
            .finding_refs
            .iter()
            .map(|finding| &finding.0)
            .collect::<BTreeSet<_>>();
        if unique_findings.len() != item.finding_refs.len() {
            issue(
                issues,
                &format!("{path}.finding_refs"),
                "finding references must be unique",
            );
        }
        if !seen.insert(item.dimension) {
            issue(
                issues,
                &format!("{path}.dimension"),
                "duplicate governed dimension",
            );
        }
    }
    let expected = WorkflowGovernedOutcomeDimension::all()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if seen != expected {
        issue(
            issues,
            "dimension_decisions",
            "every governed outcome dimension is required exactly once",
        );
    }
}

fn validate_named_decisions<'a>(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    path: &str,
    decisions: impl Iterator<Item = (&'a StableId, &'a str, &'a [StableId])>,
) {
    let mut ids = BTreeSet::new();
    for (index, (id, rationale, findings)) in decisions.enumerate() {
        let item_path = format!("{path}[{index}]");
        require_nonblank(issues, &format!("{item_path}.workflow_id"), &id.0);
        require_nonblank(issues, &format!("{item_path}.rationale"), rationale);
        if !ids.insert(&id.0) {
            issue(
                issues,
                &format!("{item_path}.workflow_id"),
                "duplicate workflow decision",
            );
        }
        let unique_findings = findings
            .iter()
            .map(|finding| &finding.0)
            .collect::<BTreeSet<_>>();
        if unique_findings.len() != findings.len() {
            issue(
                issues,
                &format!("{item_path}.finding_refs"),
                "finding references must be unique",
            );
        }
    }
}

fn validate_signatures(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    signatures: &[WorkflowReleaseAdmissionSignature],
) {
    let mut credentials = BTreeSet::new();
    let mut principals = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for (index, signature) in signatures.iter().enumerate() {
        let path = format!("authorization.signatures[{index}]");
        require_nonblank(
            issues,
            &format!("{path}.principal_id"),
            &signature.principal_id.0,
        );
        require_nonblank(
            issues,
            &format!("{path}.credential_id"),
            &signature.credential_id.0,
        );
        require_digest(
            issues,
            &format!("{path}.payload_digest"),
            &signature.payload_digest,
        );
        require_nonblank(issues, &format!("{path}.signature"), &signature.signature);
        require_hex_bytes(
            issues,
            &format!("{path}.signature"),
            &signature.signature,
            64,
        );
        if !credentials.insert(&signature.credential_id.0) {
            issue(
                issues,
                &format!("{path}.credential_id"),
                "a credential may sign only once",
            );
        }
        if !roles.insert(signature.role) {
            issue(
                issues,
                &format!("{path}.role"),
                "each required role must use a distinct signature",
            );
        }
        if !principals.insert(&signature.principal_id.0) {
            issue(
                issues,
                &format!("{path}.principal_id"),
                "independent roles require distinct principals",
            );
        }
    }
    let expected = [
        WorkflowReleaseReviewerRole::SemanticReviewer,
        WorkflowReleaseReviewerRole::ReleaseAuthorizer,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if roles != expected {
        issue(
            issues,
            "authorization.signatures",
            "one semantic-reviewer and one release-authorizer signature are required",
        );
    }
}

fn require_nonblank(issues: &mut Vec<WorkflowReleaseReviewContractIssue>, path: &str, value: &str) {
    if value.trim().is_empty() {
        issue(issues, path, "value must not be blank");
    }
}

fn require_digest(issues: &mut Vec<WorkflowReleaseReviewContractIssue>, path: &str, value: &str) {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if !valid {
        issue(issues, path, "expected sha256:<64 hexadecimal characters>");
    }
}

fn require_hex_bytes(
    issues: &mut Vec<WorkflowReleaseReviewContractIssue>,
    path: &str,
    value: &str,
    byte_count: usize,
) {
    if value.len() != byte_count * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        issue(
            issues,
            path,
            &format!("expected {byte_count} bytes encoded as hexadecimal"),
        );
    }
}

fn issue(issues: &mut Vec<WorkflowReleaseReviewContractIssue>, path: &str, message: &str) {
    issues.push(WorkflowReleaseReviewContractIssue {
        path: path.to_owned(),
        message: message.to_owned(),
    });
}
