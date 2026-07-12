//! Generic P5d.4b contracts for reviewing one appended governance release.
//!
//! V2 deliberately does not alter the frozen V1 wire contract. These values
//! are candidate inputs only; structural validation is not admission authority.

use std::collections::BTreeSet;

use crate::common::{PrincipalId, RepoPath, StableId};
use crate::workflow_behavior::WorkflowGovernedOutcomeDimension;
use crate::workflow_release::{
    WorkflowGovernanceReleaseIdentity, WorkflowReleasePredecessorReference,
    WorkflowRuntimeBundleIdentity,
};
use crate::workflow_release_review::{
    WorkflowReleaseAdmissionSignatureAlgorithm, WorkflowReleaseReviewDecision,
    WorkflowReleaseReviewerRole,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION: &str = "0.2";
pub const WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewIndexV2Document {
    pub schema_version: String,
    pub workflow_release_review_index: WorkflowReleaseReviewIndexV2,
}

/// Review input for exactly one successor release. A chain is represented by
/// multiple independently reviewed documents, never by widening this subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewIndexV2 {
    pub id: StableId,
    pub index_version: String,
    pub authority: WorkflowReleaseReviewIndexV2Authority,
    pub promotion: WorkflowReleasePromotionBindingV2,
    pub release_manifest: WorkflowReleaseReviewArtifactBindingV2,
    pub migration_batches: Vec<WorkflowReleaseReviewArtifactBindingV2>,
    pub review_subject: WorkflowReleaseReviewArtifactBindingV2,
    pub coverage_policy: WorkflowReleaseReviewArtifactBindingV2,
    /// Exhaustive legacy catalog inventory used to prove that classifications
    /// and source identities were neither dropped nor silently substituted.
    pub full_catalog: WorkflowReleaseReviewArtifactBindingV2,
    pub corpus_set: WorkflowReleaseReviewArtifactBindingV2,
    pub representative_corpus: WorkflowReleaseReviewArtifactBindingV2,
    pub adversarial_corpus: WorkflowReleaseReviewArtifactBindingV2,
    pub shadow_report: WorkflowReleaseReviewArtifactBindingV2,
    pub candidate_runtime_bundle: WorkflowReleaseReviewArtifactBindingV2,
    pub promoted_runtime_bundle: WorkflowReleaseReviewArtifactBindingV2,
    pub predecessor_registry: WorkflowReleaseReviewArtifactBindingV2,
    pub proposed_registry: WorkflowReleaseReviewArtifactBindingV2,
    pub evaluator_source: WorkflowReleaseReviewArtifactBindingV2,
    pub frozen_history: WorkflowReleaseReviewArtifactBindingV2,
    pub workflow_decisions: Vec<WorkflowReleaseReviewWorkflowDecisionV2>,
    pub quarantine_decisions: Vec<WorkflowReleaseReviewQuarantineDecisionV2>,
    pub dimension_decisions: Vec<WorkflowReleaseReviewDimensionDecisionV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseReviewIndexV2Authority {
    CandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewArtifactBindingV2 {
    pub artifact_id: StableId,
    pub embedded_ref: RepoPath,
    pub raw_digest: String,
    pub canonical_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleasePromotionBindingV2 {
    pub predecessor: WorkflowReleasePredecessorReference,
    pub candidate_release: WorkflowGovernanceReleaseIdentity,
    pub candidate_runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub promoted_runtime_bundle: WorkflowRuntimeBundleIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewWorkflowDecisionV2 {
    pub workflow_id: StableId,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewQuarantineDecisionV2 {
    pub workflow_id: StableId,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseReviewDimensionDecisionV2 {
    pub dimension: WorkflowGovernedOutcomeDimension,
    pub decision: WorkflowReleaseReviewDecision,
    pub rationale: String,
    pub finding_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorizationV2Document {
    pub schema_version: String,
    pub workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorizationV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorizationV2 {
    pub authority: WorkflowReleaseAdmissionAuthorizationV2Authority,
    pub payload: WorkflowReleaseAdmissionAuthorizationPayloadV2,
    pub signatures: Vec<WorkflowReleaseAdmissionSignatureV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionAuthorizationV2Authority {
    CandidateAuthorization,
}

/// Release-specific signed material. Critical artifact bindings are repeated
/// so an authorization cannot be replayed for another predecessor or subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionAuthorizationPayloadV2 {
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
    pub promotion: WorkflowReleasePromotionBindingV2,
    pub release_manifest: WorkflowReleaseReviewArtifactBindingV2,
    pub review_subject: WorkflowReleaseReviewArtifactBindingV2,
    pub full_catalog: WorkflowReleaseReviewArtifactBindingV2,
    pub predecessor_registry: WorkflowReleaseReviewArtifactBindingV2,
    pub proposed_registry: WorkflowReleaseReviewArtifactBindingV2,
    pub invalidate_all_receipts: bool,
    pub workflow_decisions: Vec<WorkflowReleaseReviewWorkflowDecisionV2>,
    pub quarantine_decisions: Vec<WorkflowReleaseReviewQuarantineDecisionV2>,
    pub dimension_decisions: Vec<WorkflowReleaseReviewDimensionDecisionV2>,
    pub audience: String,
    pub domain: String,
    pub nonce: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionSignatureV2 {
    pub principal_id: PrincipalId,
    pub credential_id: StableId,
    pub role: WorkflowReleaseReviewerRole,
    pub algorithm: WorkflowReleaseAdmissionSignatureAlgorithm,
    pub payload_digest: String,
    pub signature: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowReleaseReviewV2ContractIssue {
    pub path: String,
    pub message: String,
}

impl WorkflowReleaseReviewIndexV2Document {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowReleaseReviewV2ContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION {
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
        for (path, artifact) in [
            ("review_index.review_subject", &index.review_subject),
            ("review_index.coverage_policy", &index.coverage_policy),
            ("review_index.full_catalog", &index.full_catalog),
            ("review_index.corpus_set", &index.corpus_set),
            (
                "review_index.representative_corpus",
                &index.representative_corpus,
            ),
            ("review_index.adversarial_corpus", &index.adversarial_corpus),
            ("review_index.shadow_report", &index.shadow_report),
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
        validate_decisions(
            &mut issues,
            &index.workflow_decisions,
            &index.quarantine_decisions,
            &index.dimension_decisions,
        );
        issues
    }
}

impl WorkflowReleaseAdmissionAuthorizationV2Document {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowReleaseReviewV2ContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let authorization = &self.workflow_release_admission_authorization;
        let payload = &authorization.payload;
        validate_authorization_payload(&mut issues, payload);
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

fn validate_authorization_payload(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    payload: &WorkflowReleaseAdmissionAuthorizationPayloadV2,
) {
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
        require_nonblank(issues, path, value);
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
        require_digest(issues, path, digest);
    }
    validate_promotion(
        issues,
        "authorization.payload.promotion",
        &payload.promotion,
    );
    for (path, artifact) in [
        (
            "authorization.payload.release_manifest",
            &payload.release_manifest,
        ),
        (
            "authorization.payload.review_subject",
            &payload.review_subject,
        ),
        ("authorization.payload.full_catalog", &payload.full_catalog),
        (
            "authorization.payload.predecessor_registry",
            &payload.predecessor_registry,
        ),
        (
            "authorization.payload.proposed_registry",
            &payload.proposed_registry,
        ),
    ] {
        validate_artifact(issues, path, artifact);
    }
    if !payload.invalidate_all_receipts {
        issue(
            issues,
            "authorization.payload.invalidate_all_receipts",
            "admission must invalidate all predecessor receipts",
        );
    }
    validate_decisions(
        issues,
        &payload.workflow_decisions,
        &payload.quarantine_decisions,
        &payload.dimension_decisions,
    );
    if payload.issued_at_unix >= payload.expires_at_unix {
        issue(
            issues,
            "authorization.payload.expires_at_unix",
            "authorization validity window must be increasing",
        );
    }
}

fn validate_artifact_set(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    artifacts: &[WorkflowReleaseReviewArtifactBindingV2],
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
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    artifact: &WorkflowReleaseReviewArtifactBindingV2,
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
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    promotion: &WorkflowReleasePromotionBindingV2,
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
    validate_bundle(
        issues,
        &format!("{path}.candidate_runtime_bundle"),
        &promotion.candidate_runtime_bundle,
    );
    validate_bundle(
        issues,
        &format!("{path}.promoted_runtime_bundle"),
        &promotion.promoted_runtime_bundle,
    );
    if promotion.predecessor.release_id == promotion.candidate_release.release_id {
        issue(
            issues,
            &format!("{path}.candidate_release.release_id"),
            "candidate release must differ from predecessor",
        );
    }
    if promotion.candidate_runtime_bundle.bundle_id == promotion.promoted_runtime_bundle.bundle_id {
        issue(
            issues,
            &format!("{path}.promoted_runtime_bundle.bundle_id"),
            "promoted bundle must differ from shadow candidate bundle",
        );
    }
}

fn validate_bundle(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    bundle: &WorkflowRuntimeBundleIdentity,
) {
    require_nonblank(issues, &format!("{path}.bundle_id"), &bundle.bundle_id.0);
    require_digest(
        issues,
        &format!("{path}.bundle_digest"),
        &bundle.bundle_digest,
    );
    require_digest(
        issues,
        &format!("{path}.policy_set_digest"),
        &bundle.policy_set_digest,
    );
}

fn validate_decisions(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    workflows: &[WorkflowReleaseReviewWorkflowDecisionV2],
    quarantines: &[WorkflowReleaseReviewQuarantineDecisionV2],
    dimensions: &[WorkflowReleaseReviewDimensionDecisionV2],
) {
    if workflows.is_empty() {
        issue(
            issues,
            "workflow_decisions",
            "at least one workflow decision is required",
        );
    }
    // The quarantine set is release-derived and may legitimately become
    // empty after all prior ambiguities are resolved. Requiring a sentinel
    // quarantine here would turn historical catalog state into a constant.
    if dimensions.is_empty() {
        issue(
            issues,
            "dimension_decisions",
            "at least one dimension decision is required",
        );
    }
    let workflow_ids = validate_named_decisions(
        issues,
        "workflow_decisions",
        workflows.iter().map(|item| {
            (
                &item.workflow_id,
                item.decision,
                item.rationale.as_str(),
                item.finding_refs.as_slice(),
            )
        }),
    );
    let quarantine_ids = validate_named_decisions(
        issues,
        "quarantine_decisions",
        quarantines.iter().map(|item| {
            (
                &item.workflow_id,
                item.decision,
                item.rationale.as_str(),
                item.finding_refs.as_slice(),
            )
        }),
    );
    if !workflow_ids.is_disjoint(&quarantine_ids) {
        issue(
            issues,
            "quarantine_decisions",
            "workflow and quarantine decision ids must be disjoint",
        );
    }
    let mut seen_dimensions = BTreeSet::new();
    for (index, item) in dimensions.iter().enumerate() {
        let path = format!("dimension_decisions[{index}]");
        require_approved(issues, &format!("{path}.decision"), item.decision);
        require_nonblank(issues, &format!("{path}.rationale"), &item.rationale);
        validate_finding_refs(issues, &path, &item.finding_refs);
        if !seen_dimensions.insert(item.dimension) {
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
    if seen_dimensions != expected {
        issue(
            issues,
            "dimension_decisions",
            "every governed outcome dimension is required exactly once",
        );
    }
}

fn validate_named_decisions<'a>(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    decisions: impl Iterator<
        Item = (
            &'a StableId,
            WorkflowReleaseReviewDecision,
            &'a str,
            &'a [StableId],
        ),
    >,
) -> BTreeSet<&'a str> {
    let mut ids = BTreeSet::new();
    for (index, (id, decision, rationale, findings)) in decisions.enumerate() {
        let item_path = format!("{path}[{index}]");
        require_nonblank(issues, &format!("{item_path}.workflow_id"), &id.0);
        require_approved(issues, &format!("{item_path}.decision"), decision);
        require_nonblank(issues, &format!("{item_path}.rationale"), rationale);
        validate_finding_refs(issues, &item_path, findings);
        if !ids.insert(id.0.as_str()) {
            issue(
                issues,
                &format!("{item_path}.workflow_id"),
                "duplicate workflow decision",
            );
        }
    }
    ids
}

fn validate_finding_refs(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    findings: &[StableId],
) {
    // A clean approval legitimately has no finding. Requiring a reference in
    // that case would incentivize fabricated evidence identifiers. When a
    // reviewer does cite findings, every reference must still be exact and
    // unique; non-approved decisions are rejected by this admission contract.
    let unique = findings
        .iter()
        .map(|finding| finding.0.as_str())
        .collect::<BTreeSet<_>>();
    if unique.len() != findings.len() || unique.contains("") {
        issue(
            issues,
            &format!("{path}.finding_refs"),
            "finding references must be nonblank and unique",
        );
    }
}

fn require_approved(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    decision: WorkflowReleaseReviewDecision,
) {
    if decision != WorkflowReleaseReviewDecision::Approved {
        issue(issues, path, "review decision must be approved");
    }
}

fn validate_signatures(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    signatures: &[WorkflowReleaseAdmissionSignatureV2],
) {
    if signatures.is_empty() {
        issue(
            issues,
            "authorization.signatures",
            "signatures must not be empty",
        );
    }
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
        if !principals.insert(&signature.principal_id.0) {
            issue(
                issues,
                &format!("{path}.principal_id"),
                "independent roles require distinct principals",
            );
        }
        if !roles.insert(signature.role) {
            issue(
                issues,
                &format!("{path}.role"),
                "each required role must use a distinct signature",
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

fn require_nonblank(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
    path: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        issue(issues, path, "value must not be blank");
    }
}

fn require_digest(issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>, path: &str, value: &str) {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if !valid {
        issue(issues, path, "expected sha256:<64 hexadecimal characters>");
    }
}

fn require_hex_bytes(
    issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>,
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

fn issue(issues: &mut Vec<WorkflowReleaseReviewV2ContractIssue>, path: &str, message: &str) {
    issues.push(WorkflowReleaseReviewV2ContractIssue {
        path: path.to_owned(),
        message: message.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> String {
        format!("sha256:{}", "a".repeat(64))
    }
    fn id(value: &str) -> StableId {
        StableId(value.to_owned())
    }
    fn artifact(value: &str) -> WorkflowReleaseReviewArtifactBindingV2 {
        WorkflowReleaseReviewArtifactBindingV2 {
            artifact_id: id(value),
            embedded_ref: RepoPath(format!("contracts/{value}.yaml")),
            raw_digest: digest(),
            canonical_digest: digest(),
        }
    }
    fn promotion() -> WorkflowReleasePromotionBindingV2 {
        WorkflowReleasePromotionBindingV2 {
            predecessor: WorkflowReleasePredecessorReference {
                release_id: id("release.previous"),
                release_digest: digest(),
            },
            candidate_release: WorkflowGovernanceReleaseIdentity {
                lineage_id: id("lineage.core"),
                release_id: id("release.next"),
                release_version: "0.3.0".to_owned(),
                release_digest: digest(),
            },
            candidate_runtime_bundle: WorkflowRuntimeBundleIdentity {
                bundle_id: id("bundle.shadow"),
                bundle_digest: digest(),
                policy_set_digest: digest(),
            },
            promoted_runtime_bundle: WorkflowRuntimeBundleIdentity {
                bundle_id: id("bundle.promoted"),
                bundle_digest: digest(),
                policy_set_digest: digest(),
            },
        }
    }
    fn workflow_decision(value: &str) -> WorkflowReleaseReviewWorkflowDecisionV2 {
        WorkflowReleaseReviewWorkflowDecisionV2 {
            workflow_id: id(value),
            decision: WorkflowReleaseReviewDecision::Approved,
            rationale: "independently reviewed".to_owned(),
            finding_refs: vec![id("finding.workflow")],
        }
    }
    fn quarantine_decision(value: &str) -> WorkflowReleaseReviewQuarantineDecisionV2 {
        WorkflowReleaseReviewQuarantineDecisionV2 {
            workflow_id: id(value),
            decision: WorkflowReleaseReviewDecision::Approved,
            rationale: "quarantine remains required".to_owned(),
            finding_refs: vec![id("finding.quarantine")],
        }
    }
    fn dimensions() -> Vec<WorkflowReleaseReviewDimensionDecisionV2> {
        WorkflowGovernedOutcomeDimension::all()
            .into_iter()
            .map(|dimension| WorkflowReleaseReviewDimensionDecisionV2 {
                dimension,
                decision: WorkflowReleaseReviewDecision::Approved,
                rationale: "dimension passed".to_owned(),
                finding_refs: vec![id("finding.dimension")],
            })
            .collect()
    }
    fn index_document() -> WorkflowReleaseReviewIndexV2Document {
        WorkflowReleaseReviewIndexV2Document {
            schema_version: WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION.to_owned(),
            workflow_release_review_index: WorkflowReleaseReviewIndexV2 {
                id: id("review.next"),
                index_version: "0.2.0".to_owned(),
                authority: WorkflowReleaseReviewIndexV2Authority::CandidateOnly,
                promotion: promotion(),
                release_manifest: artifact("release"),
                migration_batches: vec![artifact("batch")],
                review_subject: artifact("subject"),
                coverage_policy: artifact("coverage"),
                full_catalog: artifact("catalog"),
                corpus_set: artifact("corpus-set"),
                representative_corpus: artifact("representative"),
                adversarial_corpus: artifact("adversarial"),
                shadow_report: artifact("report"),
                candidate_runtime_bundle: artifact("candidate-bundle"),
                promoted_runtime_bundle: artifact("promoted-bundle"),
                predecessor_registry: artifact("predecessor-registry"),
                proposed_registry: artifact("proposed-registry"),
                evaluator_source: artifact("evaluator"),
                frozen_history: artifact("history"),
                workflow_decisions: vec![workflow_decision("workflow.a")],
                quarantine_decisions: vec![quarantine_decision("workflow.q")],
                dimension_decisions: dimensions(),
            },
        }
    }

    #[test]
    fn valid_index_has_no_structural_issues() {
        assert!(index_document().validate().is_empty());
    }

    #[test]
    fn dynamic_workflow_count_is_accepted() {
        let mut document = index_document();
        document
            .workflow_release_review_index
            .workflow_decisions
            .push(workflow_decision("workflow.b"));
        assert!(document.validate().is_empty());
    }

    #[test]
    fn derived_quarantine_set_may_be_empty() {
        let mut document = index_document();
        document
            .workflow_release_review_index
            .quarantine_decisions
            .clear();
        assert!(document.validate().is_empty());
    }

    #[test]
    fn duplicate_or_unapproved_decisions_are_rejected() {
        let mut document = index_document();
        let mut duplicate = workflow_decision("workflow.a");
        duplicate.decision = WorkflowReleaseReviewDecision::ChangesRequired;
        document
            .workflow_release_review_index
            .workflow_decisions
            .push(duplicate);
        let issues = document.validate();
        assert!(issues
            .iter()
            .any(|issue| issue.message == "duplicate workflow decision"));
        assert!(issues
            .iter()
            .any(|issue| issue.message == "review decision must be approved"));
    }

    #[test]
    fn empty_batches_and_missing_catalog_digest_are_rejected() {
        let mut document = index_document();
        document
            .workflow_release_review_index
            .migration_batches
            .clear();
        document
            .workflow_release_review_index
            .full_catalog
            .canonical_digest
            .clear();
        let issues = document.validate();
        assert!(issues
            .iter()
            .any(|issue| issue.path == "review_index.migration_batches"));
        assert!(issues
            .iter()
            .any(|issue| issue.path == "review_index.full_catalog.canonical_digest"));
    }

    fn authorization_document() -> WorkflowReleaseAdmissionAuthorizationV2Document {
        let index = index_document().workflow_release_review_index;
        let signature =
            |principal: &str, credential: &str, role| WorkflowReleaseAdmissionSignatureV2 {
                principal_id: PrincipalId(principal.to_owned()),
                credential_id: id(credential),
                role,
                algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
                payload_digest: digest(),
                signature: "b".repeat(128),
                signed_at_unix: 150,
            };
        WorkflowReleaseAdmissionAuthorizationV2Document {
            schema_version: WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION.to_owned(),
            workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorizationV2 {
                authority: WorkflowReleaseAdmissionAuthorizationV2Authority::CandidateAuthorization,
                payload: WorkflowReleaseAdmissionAuthorizationPayloadV2 {
                    authorization_id: id("authorization.next"),
                    review_index_id: index.id,
                    review_index_version: index.index_version,
                    review_index_raw_digest: digest(),
                    review_index_canonical_digest: digest(),
                    evaluation_digest: digest(),
                    reviewer_registry_id: id("reviewers"),
                    reviewer_registry_version: "0.1.0".to_owned(),
                    reviewer_registry_raw_digest: digest(),
                    reviewer_registry_canonical_digest: digest(),
                    promotion: index.promotion,
                    release_manifest: index.release_manifest,
                    review_subject: index.review_subject,
                    full_catalog: index.full_catalog,
                    predecessor_registry: index.predecessor_registry,
                    proposed_registry: index.proposed_registry,
                    invalidate_all_receipts: true,
                    workflow_decisions: index.workflow_decisions,
                    quarantine_decisions: index.quarantine_decisions,
                    dimension_decisions: index.dimension_decisions,
                    audience: "forge-core-kernel".to_owned(),
                    domain: "workflow-release-admission-v2".to_owned(),
                    nonce: "release.next:unique".to_owned(),
                    issued_at_unix: 100,
                    expires_at_unix: 200,
                },
                signatures: vec![
                    signature(
                        "principal.semantic",
                        "credential.semantic",
                        WorkflowReleaseReviewerRole::SemanticReviewer,
                    ),
                    signature(
                        "principal.release",
                        "credential.release",
                        WorkflowReleaseReviewerRole::ReleaseAuthorizer,
                    ),
                ],
            },
        }
    }

    #[test]
    fn valid_authorization_has_no_structural_issues() {
        assert!(authorization_document().validate().is_empty());
    }

    #[test]
    fn authorization_rejects_replay_weakening_and_bad_signers() {
        let mut document = authorization_document();
        let authorization = &mut document.workflow_release_admission_authorization;
        authorization.payload.nonce.clear();
        authorization.payload.invalidate_all_receipts = false;
        authorization.signatures[1].principal_id = authorization.signatures[0].principal_id.clone();
        let issues = document.validate();
        assert!(issues
            .iter()
            .any(|issue| issue.path == "authorization.payload.nonce"));
        assert!(issues
            .iter()
            .any(|issue| issue.path == "authorization.payload.invalidate_all_receipts"));
        assert!(issues
            .iter()
            .any(|issue| issue.message == "independent roles require distinct principals"));
    }
}
