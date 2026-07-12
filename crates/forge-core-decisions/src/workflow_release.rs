//! Pure P5d.1 release-manifest and candidate-batch evaluation.
//!
//! This Module validates authored rollout intent and derives a deterministic
//! scorecard. It deliberately cannot admit executable or retirement authority:
//! a valid raw batch remains a non-authoritative migration candidate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use forge_core_contracts::{
    StableId, WorkflowCompatibilityField, WorkflowCompatibilityLifecycle, WorkflowGovernanceBundle,
    WorkflowGovernanceBundleDocument, WorkflowGovernancePolicy, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowMigrationBatchDocument, WorkflowMigrationDisposition,
    WorkflowMigrationEvidenceReference, WorkflowReceiptCarryover, WorkflowReleaseDispositionIntent,
    WorkflowReleaseRegistrySource, WorkflowRuntimeBundleIdentity,
    WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
    WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION, WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::catalog::LoadedWorkflowDocument;
use crate::workflow_governance::validate_workflow_governance_bundle;
use crate::workflow_migration::{WorkflowMigrationAudit, WorkflowMigrationAuditStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseEvaluationStatus {
    StructurallyValid,
    Blocked,
}

/// Raw documents can only produce this non-authoritative evaluation tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseEvaluationAuthority {
    CandidateOnly,
}

/// Embedded SHA-256 verification proves bytes and binding only. It does not
/// establish behavioral, semantic, or rollout-readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseEvidenceAssurance {
    ContentIntegrityOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseDerivedState {
    MigrationCandidateStructurallyValid,
    CompatibilityOnly,
    Quarantined,
    DomainPackCandidate,
    RetirementPendingVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseIssueCode {
    UnsupportedSchemaVersion,
    InvalidIdentifier,
    InvalidSemver,
    InvalidDigest,
    AuditNotReady,
    CatalogDigestMismatch,
    CatalogMismatch,
    DuplicateManifestEntry,
    MissingManifestEntry,
    UnknownManifestEntry,
    DuplicateBatchReference,
    MissingBatch,
    ExtraBatch,
    BatchReferenceMismatch,
    EmbeddedBatchReferenceMissing,
    EmbeddedBatchParseFailed,
    EmbeddedBatchDocumentMismatch,
    BatchDigestMismatch,
    BatchChainMismatch,
    BatchSourceMismatch,
    DuplicateBinding,
    BindingMismatch,
    DispositionBatchLeak,
    DomainCoreLeak,
    EvidenceReferenceMissing,
    EvidenceDigestMismatch,
    RetirementVerificationUnavailable,
    GlobalPolicyCompositionInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseIssue {
    pub code: WorkflowReleaseIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseGapCode {
    MigrationStructuralValidationFailed,
    CompatibilityRetained,
    QuarantineReviewRequired,
    DomainPackDeferred,
    RetirementVerificationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseGap {
    pub code: WorkflowReleaseGapCode,
    pub workflow_id: StableId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAssessment {
    pub workflow_id: StableId,
    pub state: WorkflowReleaseDerivedState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<StableId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_ref: Option<StableId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseScorecardCounts {
    pub migration_candidate_structurally_valid: usize,
    pub compatibility_only: usize,
    pub quarantined: usize,
    pub domain_pack_candidate: usize,
    pub retirement_pending_verification: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseEvaluation {
    pub schema_version: String,
    pub status: WorkflowReleaseEvaluationStatus,
    pub authority: WorkflowReleaseEvaluationAuthority,
    pub evidence_assurance: WorkflowReleaseEvidenceAssurance,
    pub release_id: StableId,
    pub release_digest: String,
    pub catalog_digest: String,
    pub counts: WorkflowReleaseScorecardCounts,
    pub assessments: Vec<WorkflowReleaseAssessment>,
    pub non_executable_gaps: Vec<WorkflowReleaseGap>,
    pub issues: Vec<WorkflowReleaseIssue>,
    pub evaluation_digest: String,
}

/// Canonical digest of one complete legacy workflow plus its stable reference.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_release_legacy_digest(workflow: &LoadedWorkflowDocument) -> Result<String, String> {
    canonical_digest(workflow)
}

/// Canonical digest of one candidate batch document.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_migration_batch_digest(
    batch: &WorkflowMigrationBatchDocument,
) -> Result<String, String> {
    canonical_digest(batch)
}

/// Canonical digest of one policy, useful for future retirement evidence.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_release_policy_digest(policy: &WorkflowGovernancePolicy) -> Result<String, String> {
    canonical_digest(policy)
}

/// Canonical digest of one release manifest document.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_release_manifest_digest(
    manifest: &WorkflowGovernanceReleaseManifestDocument,
) -> Result<String, String> {
    canonical_digest(manifest)
}

/// Canonical digest of one runtime governance bundle document.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_runtime_bundle_digest(
    bundle: &WorkflowGovernanceBundleDocument,
) -> Result<String, String> {
    canonical_digest(bundle)
}

/// Canonical digest of the ordered policy set, independent of enclosing
/// runtime bundle identity.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_policy_set_digest(policies: &[WorkflowGovernancePolicy]) -> Result<String, String> {
    canonical_digest(&policies)
}

/// Canonical digest for the implicit P5c genesis release subject. The release
/// digest is deliberately distinct from its runtime bundle digest.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_implicit_p5c_release_digest(
    lineage_id: &StableId,
    release_id: &StableId,
    release_version: &str,
    runtime_bundle: &WorkflowRuntimeBundleIdentity,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct Subject<'a> {
        kind: &'static str,
        lineage_id: &'a StableId,
        release_id: &'a StableId,
        release_version: &'a str,
        runtime_bundle: &'a WorkflowRuntimeBundleIdentity,
    }
    canonical_digest(&Subject {
        kind: "implicit_p5c_genesis",
        lineage_id,
        release_id,
        release_version,
        runtime_bundle,
    })
}

/// Canonical digest of registry provenance, separate from every release digest.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_release_registry_digest(
    registry: &WorkflowGovernanceReleaseRegistryDocument,
) -> Result<String, String> {
    canonical_digest(registry)
}

/// Evaluate a complete P5d.1 release without IO or mutation.
#[must_use]
pub fn evaluate_workflow_release(
    manifest_document: &WorkflowGovernanceReleaseManifestDocument,
    batches: &[WorkflowMigrationBatchDocument],
    migration_audit: &WorkflowMigrationAudit,
    workflows: &[LoadedWorkflowDocument],
) -> WorkflowReleaseEvaluation {
    let manifest = &manifest_document.workflow_governance_release_manifest;
    let mut issues = Vec::new();

    validate_manifest_shape(manifest_document, &mut issues);

    let mut ordered_workflows = workflows.iter().collect::<Vec<_>>();
    ordered_workflows.sort_by(|left, right| {
        left.document
            .workflow
            .id
            .0
            .cmp(&right.document.workflow.id.0)
    });
    let mut workflow_by_id = BTreeMap::new();
    let mut legacy_digest_by_id = BTreeMap::new();
    for (index, workflow) in ordered_workflows.iter().enumerate() {
        let id = workflow.document.workflow.id.0.as_str();
        if workflow_by_id.insert(id, *workflow).is_some() {
            issue(
                &mut issues,
                WorkflowReleaseIssueCode::CatalogMismatch,
                format!("catalog[{index}].workflow.id"),
                format!("duplicate catalog workflow id {id}"),
            );
        }
        match workflow_release_legacy_digest(workflow) {
            Ok(digest) => {
                legacy_digest_by_id.insert(id, digest);
            }
            Err(error) => issue(
                &mut issues,
                WorkflowReleaseIssueCode::InvalidDigest,
                format!("catalog[{index}]"),
                error,
            ),
        }
    }
    let catalog_digest = catalog_digest(&ordered_workflows).unwrap_or_else(|error| {
        issue(
            &mut issues,
            WorkflowReleaseIssueCode::InvalidDigest,
            "catalog",
            error,
        );
        String::new()
    });
    validate_audit_and_catalog(
        migration_audit,
        &catalog_digest,
        &workflow_by_id,
        &mut issues,
    );
    if manifest.legacy_catalog_digest != catalog_digest {
        issue(
            &mut issues,
            WorkflowReleaseIssueCode::CatalogDigestMismatch,
            "workflow_governance_release_manifest.legacy_catalog_digest",
            format!(
                "expected catalog digest {catalog_digest}, found {}",
                manifest.legacy_catalog_digest
            ),
        );
    }

    let entry_by_id = validate_manifest_entries(
        manifest_document,
        &workflow_by_id,
        &legacy_digest_by_id,
        migration_audit,
        &mut issues,
    );
    let referenced_batches = validate_batch_references(manifest_document, &mut issues);
    let input_batches = index_input_batches(batches, &mut issues);
    validate_batch_set(
        &referenced_batches,
        &input_batches,
        &entry_by_id,
        &workflow_by_id,
        &legacy_digest_by_id,
        &catalog_digest,
        &mut issues,
    );

    issues.sort_by(|left, right| {
        (left.code, left.path.as_str(), left.message.as_str()).cmp(&(
            right.code,
            right.path.as_str(),
            right.message.as_str(),
        ))
    });
    issues.dedup();
    let structurally_valid = issues.is_empty();
    let retirement_pending = entry_by_id.values().any(|entry| {
        matches!(
            entry.disposition_intent,
            WorkflowReleaseDispositionIntent::RetirementCandidate { .. }
        )
    });
    let (assessments, counts, mut non_executable_gaps) =
        derive_scorecard(&entry_by_id, structurally_valid);
    non_executable_gaps.sort_by(|left, right| {
        (
            left.workflow_id.0.as_str(),
            left.code,
            left.message.as_str(),
        )
            .cmp(&(
                right.workflow_id.0.as_str(),
                right.code,
                right.message.as_str(),
            ))
    });

    let status = if structurally_valid && !retirement_pending {
        WorkflowReleaseEvaluationStatus::StructurallyValid
    } else {
        WorkflowReleaseEvaluationStatus::Blocked
    };
    let release_digest = canonical_digest(manifest_document).unwrap_or_default();
    let digest_input = WorkflowReleaseEvaluationDigestInput {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION,
        status,
        authority: WorkflowReleaseEvaluationAuthority::CandidateOnly,
        evidence_assurance: WorkflowReleaseEvidenceAssurance::ContentIntegrityOnly,
        release_id: &manifest.release_id,
        release_digest: &release_digest,
        catalog_digest: &catalog_digest,
        counts: &counts,
        assessments: &assessments,
        non_executable_gaps: &non_executable_gaps,
        issues: &issues,
    };
    let evaluation_digest = canonical_digest(&digest_input).unwrap_or_default();

    WorkflowReleaseEvaluation {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION.to_owned(),
        status,
        authority: WorkflowReleaseEvaluationAuthority::CandidateOnly,
        evidence_assurance: WorkflowReleaseEvidenceAssurance::ContentIntegrityOnly,
        release_id: manifest.release_id.clone(),
        release_digest,
        catalog_digest,
        counts,
        assessments,
        non_executable_gaps,
        issues,
        evaluation_digest,
    }
}

#[derive(Serialize)]
struct WorkflowReleaseEvaluationDigestInput<'a> {
    schema_version: &'static str,
    status: WorkflowReleaseEvaluationStatus,
    authority: WorkflowReleaseEvaluationAuthority,
    evidence_assurance: WorkflowReleaseEvidenceAssurance,
    release_id: &'a StableId,
    release_digest: &'a str,
    catalog_digest: &'a str,
    counts: &'a WorkflowReleaseScorecardCounts,
    assessments: &'a [WorkflowReleaseAssessment],
    non_executable_gaps: &'a [WorkflowReleaseGap],
    issues: &'a [WorkflowReleaseIssue],
}

fn validate_manifest_shape(
    document: &WorkflowGovernanceReleaseManifestDocument,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    if document.schema_version != WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION {
        issue(
            issues,
            WorkflowReleaseIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!("unsupported release schema {}", document.schema_version),
        );
    }
    let manifest = &document.workflow_governance_release_manifest;
    for (path, value) in [
        ("lineage_id", manifest.lineage_id.0.as_str()),
        ("release_id", manifest.release_id.0.as_str()),
    ] {
        if value.trim().is_empty() {
            issue(
                issues,
                WorkflowReleaseIssueCode::InvalidIdentifier,
                format!("workflow_governance_release_manifest.{path}"),
                "identifier must be non-blank",
            );
        }
    }
    validate_semver(
        issues,
        "workflow_governance_release_manifest.release_version",
        &manifest.release_version,
    );
    validate_semver(
        issues,
        "workflow_governance_release_manifest.compatibility_policy.minimum_consumer_version",
        &manifest.compatibility_policy.minimum_consumer_version,
    );
    validate_semver(
        issues,
        "workflow_governance_release_manifest.compatibility_policy.policy_version",
        &manifest.compatibility_policy.policy_version,
    );
    require_id(
        issues,
        "workflow_governance_release_manifest.compatibility_policy.diagnostic_code",
        &manifest.compatibility_policy.diagnostic_code,
    );
    if manifest.compatibility_policy.replacement_argv.is_empty() {
        issue(
            issues,
            WorkflowReleaseIssueCode::InvalidIdentifier,
            "workflow_governance_release_manifest.compatibility_policy.replacement_argv",
            "replacement argv must contain at least one argument",
        );
    }
    for (index, argument) in manifest
        .compatibility_policy
        .replacement_argv
        .iter()
        .enumerate()
    {
        require_text(
            issues,
            &format!(
                "workflow_governance_release_manifest.compatibility_policy.replacement_argv[{index}]"
            ),
            argument,
        );
    }
    match &manifest.compatibility_policy.lifecycle {
        WorkflowCompatibilityLifecycle::Deprecated {
            announced_at_unix,
            removal_not_before_unix,
        } if announced_at_unix >= removal_not_before_unix => issue(
            issues,
            WorkflowReleaseIssueCode::CatalogMismatch,
            "workflow_governance_release_manifest.compatibility_policy.lifecycle",
            "deprecation removal boundary must be later than announcement",
        ),
        WorkflowCompatibilityLifecycle::Supported
        | WorkflowCompatibilityLifecycle::Deprecated { .. } => {}
        WorkflowCompatibilityLifecycle::Retired { authorization_ref } => {
            require_id(
                issues,
                "workflow_governance_release_manifest.compatibility_policy.lifecycle.authorization_ref.authorization_id",
                &authorization_ref.authorization_id,
            );
            require_text(
                issues,
                "workflow_governance_release_manifest.compatibility_policy.lifecycle.authorization_ref.embedded_ref",
                &authorization_ref.embedded_ref.0,
            );
            validate_digest(
                issues,
                "workflow_governance_release_manifest.compatibility_policy.lifecycle.authorization_ref.expected_digest",
                &authorization_ref.expected_digest,
            );
            issue(
                issues,
                WorkflowReleaseIssueCode::RetirementVerificationUnavailable,
                "workflow_governance_release_manifest.compatibility_policy.lifecycle",
                "raw retired lifecycle cannot pass P5d.1 without trusted retirement verification",
            );
        }
    }
    validate_digest(
        issues,
        "workflow_governance_release_manifest.legacy_catalog_digest",
        &manifest.legacy_catalog_digest,
    );
    if let Some(digest) = &manifest.previous_release_digest {
        validate_digest(
            issues,
            "workflow_governance_release_manifest.previous_release_digest",
            digest,
        );
    }
    let unique_fields = manifest
        .compatibility_policy
        .exact_fields
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if unique_fields.len() != manifest.compatibility_policy.exact_fields.len() {
        issue(
            issues,
            WorkflowReleaseIssueCode::CatalogMismatch,
            "workflow_governance_release_manifest.compatibility_policy.exact_fields",
            "compatibility projection field occurs more than once",
        );
    }
    if !matches!(
        manifest.compatibility_policy.lifecycle,
        WorkflowCompatibilityLifecycle::Retired { .. }
    ) {
        let required = BTreeSet::from([
            WorkflowCompatibilityField::Id,
            WorkflowCompatibilityField::Phases,
            WorkflowCompatibilityField::WorkflowRef,
            WorkflowCompatibilityField::Triggers,
            WorkflowCompatibilityField::Prerequisites,
            WorkflowCompatibilityField::Outputs,
        ]);
        if unique_fields != required {
            issue(
                issues,
                WorkflowReleaseIssueCode::CatalogMismatch,
                "workflow_governance_release_manifest.compatibility_policy.exact_fields",
                "supported/deprecated compatibility must preserve the exact six-field P5a projection",
            );
        }
    }
}

fn validate_audit_and_catalog(
    audit: &WorkflowMigrationAudit,
    catalog_digest: &str,
    workflow_by_id: &BTreeMap<&str, &LoadedWorkflowDocument>,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    let catalog_count = workflow_by_id.len();
    if audit.status != WorkflowMigrationAuditStatus::ReadyForShadow || !audit.issues.is_empty() {
        issue(
            issues,
            WorkflowReleaseIssueCode::AuditNotReady,
            "workflow_migration_audit",
            "P5a audit must be ready_for_shadow with no issues",
        );
    }
    if audit.catalog_count != catalog_count
        || audit.classified_count != catalog_count
        || audit.unresolved_count != 0
    {
        issue(
            issues,
            WorkflowReleaseIssueCode::CatalogMismatch,
            "workflow_migration_audit.catalog_count",
            "P5a audit counts do not cover the complete release catalog",
        );
    }
    if audit.manifest.schema_version != WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION {
        issue(
            issues,
            WorkflowReleaseIssueCode::UnsupportedSchemaVersion,
            "workflow_migration_audit.manifest.schema_version",
            format!(
                "unsupported P5a manifest schema {}",
                audit.manifest.schema_version
            ),
        );
    }
    let mut audit_ids = BTreeSet::new();
    for (index, entry) in audit.manifest.entries.iter().enumerate() {
        if !audit_ids.insert(entry.workflow_id.as_str()) {
            issue(
                issues,
                WorkflowReleaseIssueCode::CatalogMismatch,
                format!("workflow_migration_audit.manifest.entries[{index}].workflow_id"),
                format!("duplicate P5a workflow {}", entry.workflow_id),
            );
        }
    }
    let catalog_ids = workflow_by_id.keys().copied().collect::<BTreeSet<_>>();
    if audit_ids != catalog_ids {
        issue(
            issues,
            WorkflowReleaseIssueCode::CatalogMismatch,
            "workflow_migration_audit.manifest.entries",
            "P5a manifest workflow ids do not exactly match the release catalog",
        );
    }
    let disposition_count = |disposition| {
        audit
            .manifest
            .entries
            .iter()
            .filter(|entry| entry.disposition == disposition)
            .count()
    };
    if audit.golden_path_count != disposition_count(WorkflowMigrationDisposition::GoldenPath)
        || audit.domain_pack_candidate_count
            != disposition_count(WorkflowMigrationDisposition::DomainPackCandidate)
        || audit.compatibility_playbook_count
            != disposition_count(WorkflowMigrationDisposition::CompatibilityPlaybook)
        || audit.quarantined_count != disposition_count(WorkflowMigrationDisposition::Quarantined)
    {
        issue(
            issues,
            WorkflowReleaseIssueCode::CatalogMismatch,
            "workflow_migration_audit.counts",
            "P5a summary counts do not match its content-addressed entries",
        );
    }
    if audit.shadow_parity.mutation_allowed
        || audit.shadow_parity.equivalent_count != catalog_count
        || audit.shadow_parity.drift_count != 0
        || audit.deletion_baseline.retirement_allowed
    {
        issue(
            issues,
            WorkflowReleaseIssueCode::AuditNotReady,
            "workflow_migration_audit.shadow_parity",
            "P5a audit must preserve exact non-mutating parity and forbid retirement",
        );
    }
    for (path, found) in [
        (
            "workflow_migration_audit.deletion_baseline.catalog_digest",
            audit.deletion_baseline.catalog_digest.as_str(),
        ),
        (
            "workflow_migration_audit.manifest.catalog_digest",
            audit.manifest.catalog_digest.as_str(),
        ),
    ] {
        if found != catalog_digest {
            issue(
                issues,
                WorkflowReleaseIssueCode::CatalogDigestMismatch,
                path,
                format!("expected {catalog_digest}, found {found}"),
            );
        }
    }
    match p5a_manifest_digest(audit) {
        Ok(expected) if expected == audit.manifest.manifest_digest => {}
        Ok(expected) => issue(
            issues,
            WorkflowReleaseIssueCode::InvalidDigest,
            "workflow_migration_audit.manifest.manifest_digest",
            format!("expected P5a manifest digest {expected}"),
        ),
        Err(error) => issue(
            issues,
            WorkflowReleaseIssueCode::InvalidDigest,
            "workflow_migration_audit.manifest",
            error,
        ),
    }
}

fn p5a_manifest_digest(audit: &WorkflowMigrationAudit) -> Result<String, String> {
    #[derive(Serialize)]
    struct DigestInput<'a> {
        schema_version: &'a str,
        plan_id: &'a str,
        catalog_digest: &'a str,
        entries: &'a [crate::workflow_migration::WorkflowMigrationAssessment],
    }
    canonical_digest(&DigestInput {
        schema_version: WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
        plan_id: &audit.manifest.plan_id,
        catalog_digest: &audit.manifest.catalog_digest,
        entries: &audit.manifest.entries,
    })
}

fn validate_manifest_entries<'a>(
    document: &'a WorkflowGovernanceReleaseManifestDocument,
    workflow_by_id: &BTreeMap<&str, &LoadedWorkflowDocument>,
    legacy_digest_by_id: &BTreeMap<&str, String>,
    audit: &WorkflowMigrationAudit,
    issues: &mut Vec<WorkflowReleaseIssue>,
) -> BTreeMap<&'a str, &'a forge_core_contracts::WorkflowReleaseWorkflowEntry> {
    let mut entries = BTreeMap::new();
    for (index, entry) in document
        .workflow_governance_release_manifest
        .workflow_entries
        .iter()
        .enumerate()
    {
        let id = entry.workflow_id.0.as_str();
        if id.trim().is_empty() {
            issue(
                issues,
                WorkflowReleaseIssueCode::InvalidIdentifier,
                format!("workflow_entries[{index}].workflow_id"),
                "workflow id must be non-blank",
            );
        }
        if entries.insert(id, entry).is_some() {
            issue(
                issues,
                WorkflowReleaseIssueCode::DuplicateManifestEntry,
                format!("workflow_entries[{index}].workflow_id"),
                format!("workflow {id} occurs more than once"),
            );
        }
        validate_digest(
            issues,
            &format!("workflow_entries[{index}].legacy_workflow_digest"),
            &entry.legacy_workflow_digest,
        );
        match legacy_digest_by_id.get(id) {
            Some(expected) if expected == &entry.legacy_workflow_digest => {}
            Some(expected) => issue(
                issues,
                WorkflowReleaseIssueCode::CatalogDigestMismatch,
                format!("workflow_entries[{index}].legacy_workflow_digest"),
                format!("workflow {id} expected {expected}"),
            ),
            None => issue(
                issues,
                WorkflowReleaseIssueCode::UnknownManifestEntry,
                format!("workflow_entries[{index}].workflow_id"),
                format!("workflow {id} is not in the release catalog"),
            ),
        }
        validate_disposition_fields(entry, index, issues);
    }
    for id in workflow_by_id.keys() {
        if !entries.contains_key(id) {
            issue(
                issues,
                WorkflowReleaseIssueCode::MissingManifestEntry,
                "workflow_governance_release_manifest.workflow_entries",
                format!("catalog workflow {id} has no explicit release disposition"),
            );
        }
    }

    let p5a_disposition = audit
        .manifest
        .entries
        .iter()
        .map(|entry| (entry.workflow_id.as_str(), entry.disposition))
        .collect::<BTreeMap<_, _>>();
    for (id, entry) in &entries {
        if p5a_disposition.get(id) == Some(&WorkflowMigrationDisposition::DomainPackCandidate)
            && !matches!(
                entry.disposition_intent,
                WorkflowReleaseDispositionIntent::DomainPackCandidate { .. }
            )
        {
            issue(
                issues,
                WorkflowReleaseIssueCode::DomainCoreLeak,
                format!("workflow_entries.{id}.disposition_intent"),
                "P5a domain-pack candidate cannot enter the core rollout",
            );
        }
    }
    entries
}

fn validate_disposition_fields(
    entry: &forge_core_contracts::WorkflowReleaseWorkflowEntry,
    index: usize,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    let path = format!("workflow_entries[{index}].disposition_intent");
    match &entry.disposition_intent {
        WorkflowReleaseDispositionIntent::MigrationCandidate {
            batch_id,
            policy_ref,
        } => {
            require_id(issues, &format!("{path}.batch_id"), batch_id);
            require_id(issues, &format!("{path}.policy_ref"), policy_ref);
        }
        WorkflowReleaseDispositionIntent::CompatibilityOnly { reason } => {
            require_text(
                issues,
                &format!("{path}.reason.explanation"),
                &reason.explanation,
            );
        }
        WorkflowReleaseDispositionIntent::Quarantined { quarantine } => {
            require_text(
                issues,
                &format!("{path}.quarantine.explanation"),
                &quarantine.explanation,
            );
            require_id(
                issues,
                &format!("{path}.quarantine.review_owner"),
                &quarantine.review_owner,
            );
            validate_semver(
                issues,
                &format!("{path}.quarantine.review_due_release_version"),
                &quarantine.review_due_release_version,
            );
            if quarantine.blocking_refs.is_empty() {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::InvalidIdentifier,
                    format!("{path}.quarantine.blocking_refs"),
                    "quarantine must declare at least one explicit blocker",
                );
            }
            let unique = quarantine.blocking_refs.iter().collect::<BTreeSet<_>>();
            if unique.len() != quarantine.blocking_refs.len() {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::InvalidIdentifier,
                    format!("{path}.quarantine.blocking_refs"),
                    "blocking ref occurs more than once",
                );
            }
            for (blocker_index, blocker) in quarantine.blocking_refs.iter().enumerate() {
                require_id(
                    issues,
                    &format!("{path}.quarantine.blocking_refs[{blocker_index}]"),
                    blocker,
                );
            }
            if quarantine.affected_consumer_refs.is_empty() {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::InvalidIdentifier,
                    format!("{path}.quarantine.affected_consumer_refs"),
                    "quarantine must identify at least one affected consumer",
                );
            }
            let affected = quarantine
                .affected_consumer_refs
                .iter()
                .collect::<BTreeSet<_>>();
            if affected.len() != quarantine.affected_consumer_refs.len() {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::InvalidIdentifier,
                    format!("{path}.quarantine.affected_consumer_refs"),
                    "affected consumer ref occurs more than once",
                );
            }
            for (consumer_index, consumer) in quarantine.affected_consumer_refs.iter().enumerate() {
                require_id(
                    issues,
                    &format!("{path}.quarantine.affected_consumer_refs[{consumer_index}]"),
                    consumer,
                );
            }
        }
        WorkflowReleaseDispositionIntent::DomainPackCandidate { candidate } => {
            require_id(
                issues,
                &format!("{path}.candidate.domain_id"),
                &candidate.domain_id,
            );
            require_id(
                issues,
                &format!("{path}.candidate.proposed_pack_id"),
                &candidate.proposed_pack_id,
            );
            require_text(
                issues,
                &format!("{path}.candidate.explanation"),
                &candidate.explanation,
            );
        }
        WorkflowReleaseDispositionIntent::RetirementCandidate {
            replacement_policy_ref,
            authorization,
        } => {
            require_id(
                issues,
                &format!("{path}.replacement_policy_ref"),
                replacement_policy_ref,
            );
            require_id(
                issues,
                &format!("{path}.authorization.authorization_id"),
                &authorization.authorization_id,
            );
            require_text(
                issues,
                &format!("{path}.authorization.embedded_ref"),
                &authorization.embedded_ref.0,
            );
            validate_digest(
                issues,
                &format!("{path}.authorization.expected_digest"),
                &authorization.expected_digest,
            );
        }
    }
}

fn validate_batch_references<'a>(
    document: &'a WorkflowGovernanceReleaseManifestDocument,
    issues: &mut Vec<WorkflowReleaseIssue>,
) -> Vec<&'a forge_core_contracts::WorkflowReleaseBatchReference> {
    let mut refs = document
        .workflow_governance_release_manifest
        .batches
        .iter()
        .collect::<Vec<_>>();
    refs.sort_by_key(|reference| (reference.deterministic_order, reference.batch_id.0.as_str()));
    let mut ids = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let mut embedded = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for (index, reference) in refs.iter().enumerate() {
        if reference.deterministic_order != u32::try_from(index).unwrap_or(u32::MAX) {
            issue(
                issues,
                WorkflowReleaseIssueCode::DuplicateBatchReference,
                format!("batches[{index}].deterministic_order"),
                format!("batch order must be contiguous and zero-based; expected {index}"),
            );
        }
        require_id(
            issues,
            &format!("batches[{index}].batch_id"),
            &reference.batch_id,
        );
        require_text(
            issues,
            &format!("batches[{index}].embedded_ref"),
            &reference.embedded_ref.0,
        );
        validate_semver(
            issues,
            &format!("batches[{index}].batch_version"),
            &reference.batch_version,
        );
        validate_digest(
            issues,
            &format!("batches[{index}].expected_digest"),
            &reference.expected_digest,
        );
        for (inserted, field) in [
            (ids.insert(reference.batch_id.0.as_str()), "batch_id"),
            (
                orders.insert(reference.deterministic_order.to_string()),
                "deterministic_order",
            ),
            (
                embedded.insert(reference.embedded_ref.0.as_str()),
                "embedded_ref",
            ),
            (
                digests.insert(reference.expected_digest.as_str()),
                "expected_digest",
            ),
        ] {
            if !inserted {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::DuplicateBatchReference,
                    format!("batches[{index}].{field}"),
                    format!("batch {field} must be unique"),
                );
            }
        }
    }
    refs
}

fn index_input_batches<'a>(
    batches: &'a [WorkflowMigrationBatchDocument],
    issues: &mut Vec<WorkflowReleaseIssue>,
) -> BTreeMap<&'a str, &'a WorkflowMigrationBatchDocument> {
    let mut indexed = BTreeMap::new();
    for (index, batch) in batches.iter().enumerate() {
        let id = batch.workflow_migration_batch.id.0.as_str();
        if indexed.insert(id, batch).is_some() {
            issue(
                issues,
                WorkflowReleaseIssueCode::DuplicateBatchReference,
                format!("input_batches[{index}].workflow_migration_batch.id"),
                format!("input batch {id} occurs more than once"),
            );
        }
    }
    indexed
}

#[allow(clippy::too_many_arguments)]
fn validate_batch_set(
    refs: &[&forge_core_contracts::WorkflowReleaseBatchReference],
    input_batches: &BTreeMap<&str, &WorkflowMigrationBatchDocument>,
    entries: &BTreeMap<&str, &forge_core_contracts::WorkflowReleaseWorkflowEntry>,
    workflow_by_id: &BTreeMap<&str, &LoadedWorkflowDocument>,
    legacy_digest_by_id: &BTreeMap<&str, String>,
    catalog_digest: &str,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    let referenced_ids = refs
        .iter()
        .map(|reference| reference.batch_id.0.as_str())
        .collect::<BTreeSet<_>>();
    for id in input_batches.keys() {
        if !referenced_ids.contains(id) {
            issue(
                issues,
                WorkflowReleaseIssueCode::ExtraBatch,
                "input_batches",
                format!("input batch {id} has no manifest reference"),
            );
        }
    }

    let mut previous_digest: Option<&str> = None;
    let mut all_policies = Vec::new();
    let mut binding_by_workflow = BTreeMap::new();
    let mut binding_by_policy = BTreeMap::new();
    for (order_index, reference) in refs.iter().enumerate() {
        let id = reference.batch_id.0.as_str();
        let embedded_document = resolve_embedded_batch(reference, order_index, issues);
        let Some(document) = input_batches.get(id).copied() else {
            issue(
                issues,
                WorkflowReleaseIssueCode::MissingBatch,
                format!("batches[{order_index}]"),
                format!("referenced batch {id} is missing"),
            );
            previous_digest = Some(reference.expected_digest.as_str());
            continue;
        };
        if embedded_document
            .as_ref()
            .is_some_and(|embedded| embedded != document)
        {
            issue(
                issues,
                WorkflowReleaseIssueCode::EmbeddedBatchDocumentMismatch,
                format!("batches[{order_index}].embedded_ref"),
                "provided batch document does not exactly equal the repository-owned embedded batch",
            );
        }
        let batch = &document.workflow_migration_batch;
        if document.schema_version != WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION {
            issue(
                issues,
                WorkflowReleaseIssueCode::UnsupportedSchemaVersion,
                format!("batch.{id}.schema_version"),
                format!("unsupported batch schema {}", document.schema_version),
            );
        }
        if batch.id != reference.batch_id || batch.batch_version != reference.batch_version {
            issue(
                issues,
                WorkflowReleaseIssueCode::BatchReferenceMismatch,
                format!("batch.{id}"),
                "batch id/version does not match its manifest reference",
            );
        }
        validate_semver(
            issues,
            &format!("batch.{id}.batch_version"),
            &batch.batch_version,
        );
        if batch.source_catalog_digest != catalog_digest {
            issue(
                issues,
                WorkflowReleaseIssueCode::BatchSourceMismatch,
                format!("batch.{id}.source_catalog_digest"),
                format!("expected source catalog digest {catalog_digest}"),
            );
        }
        validate_batch_evidence(id, &batch.evidence, issues);
        if batch.previous_batch_digest.as_deref() != previous_digest {
            issue(
                issues,
                WorkflowReleaseIssueCode::BatchChainMismatch,
                format!("batch.{id}.previous_batch_digest"),
                format!(
                    "expected previous digest {previous_digest:?}, found {:?}",
                    batch.previous_batch_digest
                ),
            );
        }
        if let Some(digest) = &batch.previous_batch_digest {
            validate_digest(issues, &format!("batch.{id}.previous_batch_digest"), digest);
        }
        match workflow_migration_batch_digest(document) {
            Ok(found) if found == reference.expected_digest => {}
            Ok(found) => issue(
                issues,
                WorkflowReleaseIssueCode::BatchDigestMismatch,
                format!("batch.{id}"),
                format!("expected {}, found {found}", reference.expected_digest),
            ),
            Err(error) => issue(
                issues,
                WorkflowReleaseIssueCode::InvalidDigest,
                format!("batch.{id}"),
                error,
            ),
        }

        let policies = batch
            .policies
            .iter()
            .map(|policy| (policy.id.0.as_str(), policy))
            .collect::<BTreeMap<_, _>>();
        if policies.len() != batch.policies.len() {
            issue(
                issues,
                WorkflowReleaseIssueCode::GlobalPolicyCompositionInvalid,
                format!("batch.{id}.policies"),
                "policy id occurs more than once inside batch",
            );
        }
        let mut batch_workflows = BTreeSet::new();
        let mut batch_policy_refs = BTreeSet::new();
        for (binding_index, binding) in batch.workflow_bindings.iter().enumerate() {
            let workflow_id = binding.workflow_id.0.as_str();
            let policy_id = binding.policy_ref.0.as_str();
            if !batch_workflows.insert(workflow_id)
                || !batch_policy_refs.insert(policy_id)
                || binding_by_workflow
                    .insert(workflow_id, (id, binding))
                    .is_some()
                || binding_by_policy.insert(policy_id, (id, binding)).is_some()
            {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::DuplicateBinding,
                    format!("batch.{id}.workflow_bindings[{binding_index}]"),
                    "workflow and policy bindings must be globally unique",
                );
            }
            if !workflow_by_id.contains_key(workflow_id) {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("batch.{id}.workflow_bindings[{binding_index}].workflow_id"),
                    format!("unknown workflow {workflow_id}"),
                );
            }
            if legacy_digest_by_id.get(workflow_id) != Some(&binding.legacy_workflow_digest) {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("batch.{id}.workflow_bindings[{binding_index}].legacy_workflow_digest"),
                    format!("legacy digest does not bind workflow {workflow_id}"),
                );
            }
            match policies.get(policy_id) {
                Some(policy) if policy.compatibility_workflow_id == binding.workflow_id => {}
                Some(_) => issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("batch.{id}.workflow_bindings[{binding_index}].policy_ref"),
                    "policy compatibility_workflow_id does not match binding workflow",
                ),
                None => issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("batch.{id}.workflow_bindings[{binding_index}].policy_ref"),
                    format!("policy {policy_id} is not present in batch"),
                ),
            }
        }
        for policy in &batch.policies {
            if !batch_policy_refs.contains(policy.id.0.as_str()) {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("batch.{id}.policies.{}", policy.id.0),
                    "batch policy has no workflow binding",
                );
            }
            all_policies.push(policy.clone());
        }
        previous_digest = Some(reference.expected_digest.as_str());
    }

    for (workflow_id, entry) in entries {
        match &entry.disposition_intent {
            WorkflowReleaseDispositionIntent::MigrationCandidate {
                batch_id,
                policy_ref,
            } => match binding_by_workflow.get(workflow_id).copied() {
                Some((found_batch, binding))
                    if found_batch == batch_id.0
                        && binding.policy_ref == *policy_ref
                        && binding.legacy_workflow_digest == entry.legacy_workflow_digest => {}
                _ => issue(
                    issues,
                    WorkflowReleaseIssueCode::BindingMismatch,
                    format!("workflow_entries.{workflow_id}.disposition_intent"),
                    "migration entry is not exactly bound by its declared batch and policy",
                ),
            },
            _ if binding_by_workflow.contains_key(workflow_id) => issue(
                issues,
                WorkflowReleaseIssueCode::DispositionBatchLeak,
                format!("workflow_entries.{workflow_id}.disposition_intent"),
                "non-migration disposition must not enter a candidate batch",
            ),
            _ => {}
        }
    }

    all_policies.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    if !all_policies.is_empty() {
        let composed = WorkflowGovernanceBundleDocument {
            schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
            workflow_governance_bundle: WorkflowGovernanceBundle {
                id: StableId("bundle.workflow-release.composed-candidate".to_owned()),
                policies: all_policies,
            },
        };
        for policy_issue in validate_workflow_governance_bundle(&composed) {
            issue(
                issues,
                WorkflowReleaseIssueCode::GlobalPolicyCompositionInvalid,
                format!("composed.{}", policy_issue.path),
                policy_issue.message,
            );
        }
    }
}

fn resolve_embedded_batch(
    reference: &forge_core_contracts::WorkflowReleaseBatchReference,
    order_index: usize,
    issues: &mut Vec<WorkflowReleaseIssue>,
) -> Option<WorkflowMigrationBatchDocument> {
    let path = reference.embedded_ref.0.as_str();
    let Some(text) = crate::embedded_text(path) else {
        issue(
            issues,
            WorkflowReleaseIssueCode::EmbeddedBatchReferenceMissing,
            format!("batches[{order_index}].embedded_ref"),
            format!("repository-owned embedded batch {path} is missing"),
        );
        return None;
    };
    let document = match yaml_serde::from_str::<WorkflowMigrationBatchDocument>(text) {
        Ok(document) => document,
        Err(error) => {
            issue(
                issues,
                WorkflowReleaseIssueCode::EmbeddedBatchParseFailed,
                format!("batches[{order_index}].embedded_ref"),
                format!("embedded batch {path} is invalid: {error}"),
            );
            return None;
        }
    };
    match workflow_migration_batch_digest(&document) {
        Ok(found) if found == reference.expected_digest => {}
        Ok(found) => issue(
            issues,
            WorkflowReleaseIssueCode::BatchDigestMismatch,
            format!("batches[{order_index}].expected_digest"),
            format!(
                "embedded batch digest expected {}, found {found}",
                reference.expected_digest
            ),
        ),
        Err(error) => issue(
            issues,
            WorkflowReleaseIssueCode::InvalidDigest,
            format!("batches[{order_index}].embedded_ref"),
            error,
        ),
    }
    Some(document)
}

fn validate_batch_evidence(
    batch_id: &str,
    evidence: &forge_core_contracts::WorkflowMigrationBatchEvidence,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    let groups = [
        (
            "representative_fixtures",
            evidence.representative_fixtures.as_slice(),
        ),
        (
            "adversarial_fixtures",
            evidence.adversarial_fixtures.as_slice(),
        ),
        ("shadow_reports", evidence.shadow_reports.as_slice()),
    ];
    let mut refs = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for (group, values) in groups {
        if values.is_empty() {
            issue(
                issues,
                WorkflowReleaseIssueCode::BindingMismatch,
                format!("batch.{batch_id}.evidence.{group}"),
                "candidate batch evidence group must not be empty",
            );
        }
        for (index, value) in values.iter().enumerate() {
            validate_evidence_reference(batch_id, group, index, value, issues);
            if !refs.insert(value.embedded_ref.0.as_str()) {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::DuplicateBinding,
                    format!("batch.{batch_id}.evidence.{group}[{index}].embedded_ref"),
                    "evidence ref must be unique across the batch",
                );
            }
            if !digests.insert(value.expected_digest.as_str()) {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::DuplicateBinding,
                    format!("batch.{batch_id}.evidence.{group}[{index}].expected_digest"),
                    "evidence digest must be unique across the batch",
                );
            }
        }
    }
}

fn validate_evidence_reference(
    batch_id: &str,
    group: &str,
    index: usize,
    reference: &WorkflowMigrationEvidenceReference,
    issues: &mut Vec<WorkflowReleaseIssue>,
) {
    require_text(
        issues,
        &format!("batch.{batch_id}.evidence.{group}[{index}].embedded_ref"),
        &reference.embedded_ref.0,
    );
    validate_digest(
        issues,
        &format!("batch.{batch_id}.evidence.{group}[{index}].expected_digest"),
        &reference.expected_digest,
    );
    match crate::embedded_text(&reference.embedded_ref.0) {
        Some(text) => {
            let expected = sha256_bytes(text.as_bytes());
            if reference.expected_digest != expected {
                issue(
                    issues,
                    WorkflowReleaseIssueCode::EvidenceDigestMismatch,
                    format!("batch.{batch_id}.evidence.{group}[{index}].expected_digest"),
                    format!("embedded evidence digest expected {expected}"),
                );
            }
        }
        None => issue(
            issues,
            WorkflowReleaseIssueCode::EvidenceReferenceMissing,
            format!("batch.{batch_id}.evidence.{group}[{index}].embedded_ref"),
            "evidence reference is not present in the repository-owned embedded contracts",
        ),
    }
}

fn derive_scorecard(
    entries: &BTreeMap<&str, &forge_core_contracts::WorkflowReleaseWorkflowEntry>,
    structurally_valid: bool,
) -> (
    Vec<WorkflowReleaseAssessment>,
    WorkflowReleaseScorecardCounts,
    Vec<WorkflowReleaseGap>,
) {
    let mut assessments = Vec::with_capacity(entries.len());
    let mut counts = WorkflowReleaseScorecardCounts::default();
    let mut gaps = Vec::new();
    for entry in entries.values() {
        let (state, batch_id, policy_ref) = match &entry.disposition_intent {
            WorkflowReleaseDispositionIntent::MigrationCandidate {
                batch_id,
                policy_ref,
            } if structurally_valid => (
                WorkflowReleaseDerivedState::MigrationCandidateStructurallyValid,
                Some(batch_id.clone()),
                Some(policy_ref.clone()),
            ),
            WorkflowReleaseDispositionIntent::MigrationCandidate { .. } => {
                gaps.push(WorkflowReleaseGap {
                    code: WorkflowReleaseGapCode::MigrationStructuralValidationFailed,
                    workflow_id: entry.workflow_id.clone(),
                    message:
                        "candidate remains legacy compatibility until every release check passes"
                            .to_owned(),
                });
                (WorkflowReleaseDerivedState::CompatibilityOnly, None, None)
            }
            WorkflowReleaseDispositionIntent::CompatibilityOnly { reason } => {
                gaps.push(WorkflowReleaseGap {
                    code: WorkflowReleaseGapCode::CompatibilityRetained,
                    workflow_id: entry.workflow_id.clone(),
                    message: reason.explanation.clone(),
                });
                (WorkflowReleaseDerivedState::CompatibilityOnly, None, None)
            }
            WorkflowReleaseDispositionIntent::Quarantined { quarantine } => {
                gaps.push(WorkflowReleaseGap {
                    code: WorkflowReleaseGapCode::QuarantineReviewRequired,
                    workflow_id: entry.workflow_id.clone(),
                    message: quarantine.explanation.clone(),
                });
                (WorkflowReleaseDerivedState::Quarantined, None, None)
            }
            WorkflowReleaseDispositionIntent::DomainPackCandidate { candidate } => {
                gaps.push(WorkflowReleaseGap {
                    code: WorkflowReleaseGapCode::DomainPackDeferred,
                    workflow_id: entry.workflow_id.clone(),
                    message: candidate.explanation.clone(),
                });
                (WorkflowReleaseDerivedState::DomainPackCandidate, None, None)
            }
            WorkflowReleaseDispositionIntent::RetirementCandidate {
                replacement_policy_ref,
                ..
            } => {
                gaps.push(WorkflowReleaseGap {
                    code: WorkflowReleaseGapCode::RetirementVerificationRequired,
                    workflow_id: entry.workflow_id.clone(),
                    message: "retirement remains pending trusted authorization, compatibility, and deletion verification"
                        .to_owned(),
                });
                (
                    WorkflowReleaseDerivedState::RetirementPendingVerification,
                    None,
                    Some(replacement_policy_ref.clone()),
                )
            }
        };
        match state {
            WorkflowReleaseDerivedState::MigrationCandidateStructurallyValid => {
                counts.migration_candidate_structurally_valid += 1;
            }
            WorkflowReleaseDerivedState::CompatibilityOnly => counts.compatibility_only += 1,
            WorkflowReleaseDerivedState::Quarantined => counts.quarantined += 1,
            WorkflowReleaseDerivedState::DomainPackCandidate => {
                counts.domain_pack_candidate += 1;
            }
            WorkflowReleaseDerivedState::RetirementPendingVerification => {
                counts.retirement_pending_verification += 1;
            }
        }
        assessments.push(WorkflowReleaseAssessment {
            workflow_id: entry.workflow_id.clone(),
            state,
            batch_id,
            policy_ref,
        });
    }
    (assessments, counts, gaps)
}

fn catalog_digest(workflows: &[&LoadedWorkflowDocument]) -> Result<String, String> {
    canonical_digest(&workflows)
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| format!("canonical encoding failed: {error}"))?;
    Ok(sha256_bytes(&canonical))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn validate_digest(issues: &mut Vec<WorkflowReleaseIssue>, path: &str, value: &str) {
    if !is_sha256_digest(value) {
        issue(
            issues,
            WorkflowReleaseIssueCode::InvalidDigest,
            path,
            "digest must be sha256 followed by 64 lowercase hexadecimal characters",
        );
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn validate_semver(issues: &mut Vec<WorkflowReleaseIssue>, path: &str, value: &str) {
    if !is_semver(value) {
        issue(
            issues,
            WorkflowReleaseIssueCode::InvalidSemver,
            path,
            format!("{value:?} is not a valid semantic version"),
        );
    }
}

fn is_semver(value: &str) -> bool {
    let (without_build, build) = if let Some((core, metadata)) = value.split_once('+') {
        if metadata.is_empty() || metadata.contains('+') {
            return false;
        }
        (core, Some(metadata))
    } else {
        (value, None)
    };
    if build.is_some_and(|part| !valid_identifiers(part, false)) {
        return false;
    }
    let (core, prerelease) = if let Some((core, prerelease)) = without_build.split_once('-') {
        if prerelease.is_empty() {
            return false;
        }
        (core, Some(prerelease))
    } else {
        (without_build, None)
    };
    if prerelease.is_some_and(|part| !valid_identifiers(part, true)) {
        return false;
    }
    let components = core.split('.').collect::<Vec<_>>();
    components.len() == 3 && components.iter().all(|part| valid_numeric_identifier(part))
}

fn valid_identifiers(value: &str, forbid_numeric_leading_zero: bool) -> bool {
    value.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && (!forbid_numeric_leading_zero
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || valid_numeric_identifier(part))
    })
}

fn valid_numeric_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
        && value.parse::<u64>().is_ok()
}

fn require_id(issues: &mut Vec<WorkflowReleaseIssue>, path: &str, id: &StableId) {
    require_text(issues, path, &id.0);
}

fn require_text(issues: &mut Vec<WorkflowReleaseIssue>, path: &str, value: &str) {
    if value.trim().is_empty() {
        issue(
            issues,
            WorkflowReleaseIssueCode::InvalidIdentifier,
            path,
            "value must be non-blank",
        );
    }
}

fn issue(
    issues: &mut Vec<WorkflowReleaseIssue>,
    code: WorkflowReleaseIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowReleaseIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseRegistryEvaluationStatus {
    StructurallyValid,
    Blocked,
}

/// Raw registry evaluation is intentionally never runtime admission authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseRegistryEvaluationAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseRegistryIssueCode {
    UnsupportedSchemaVersion,
    InvalidIdentifier,
    InvalidSemver,
    InvalidDigest,
    WrongReleaseCount,
    DuplicateReleaseIdentity,
    DuplicateRuntimeBundleIdentity,
    LineageMismatch,
    DefaultSuccessorMismatch,
    GenesisMappingMismatch,
    PredecessorMismatch,
    EmbeddedReferenceMissing,
    EmbeddedDocumentInvalid,
    EmbeddedDigestMismatch,
    SuppliedBundleMissing,
    SuppliedBundleMismatch,
    RuntimeBundleIdentityMismatch,
    ReleaseManifestIdentityMismatch,
    PolicySetDrift,
    CandidateSetElevation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseRegistryIssue {
    pub code: WorkflowReleaseRegistryIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseRegistryEvaluation {
    pub schema_version: String,
    pub status: WorkflowReleaseRegistryEvaluationStatus,
    pub authority: WorkflowReleaseRegistryEvaluationAuthority,
    pub registry_id: StableId,
    pub registry_digest: String,
    pub lineage_id: StableId,
    pub genesis_release: Option<WorkflowGovernanceReleaseIdentity>,
    pub genesis_runtime_bundle: Option<WorkflowRuntimeBundleIdentity>,
    pub default_successor_release: Option<WorkflowGovernanceReleaseIdentity>,
    pub default_successor_runtime_bundle: Option<WorkflowRuntimeBundleIdentity>,
    pub successor_policy_count: usize,
    pub issues: Vec<WorkflowReleaseRegistryIssue>,
    pub evaluation_digest: String,
}

struct ResolvedRegistryEntry<'a> {
    entry: &'a forge_core_contracts::WorkflowGovernanceReleaseRegistryEntry,
    bundle: Option<WorkflowGovernanceBundleDocument>,
    manifest: Option<WorkflowGovernanceReleaseManifestDocument>,
}

#[derive(Default)]
struct RegistryIdentitySets {
    release_ids: BTreeSet<String>,
    release_digests: BTreeSet<String>,
    bundle_ids: BTreeSet<String>,
    bundle_digests: BTreeSet<String>,
}

/// Validate the closed P5d.2 foundation registry and caller-supplied bundle
/// projections against repository-owned embedded bytes. A successful result is
/// still non-authoritative and cannot pin or upgrade a project.
#[must_use]
pub fn evaluate_workflow_release_registry(
    registry_document: &WorkflowGovernanceReleaseRegistryDocument,
    supplied_bundles: &[WorkflowGovernanceBundleDocument],
) -> WorkflowReleaseRegistryEvaluation {
    let registry = &registry_document.workflow_governance_release_registry;
    let mut issues = Vec::new();
    validate_registry_header(registry_document, &mut issues);

    let supplied = index_supplied_bundles(supplied_bundles, &mut issues);
    let mut identities = RegistryIdentitySets::default();
    let mut resolved = Vec::new();
    for (index, entry) in registry.releases.iter().enumerate() {
        validate_registry_entry_identity(registry, entry, index, &mut identities, &mut issues);
        resolved.push(resolve_registry_entry(entry, index, &supplied, &mut issues));
    }

    let (genesis, successor) = validate_registry_chain(registry, &resolved, &mut issues);
    let successor_policy_count =
        validate_registry_policy_equivalence(genesis.as_ref(), successor.as_ref(), &mut issues);
    let status = if issues.is_empty() {
        WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
    } else {
        WorkflowReleaseRegistryEvaluationStatus::Blocked
    };
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    let registry_digest = workflow_release_registry_digest(registry_document).unwrap_or_default();
    let genesis_release = genesis.map(|value| value.entry.release.clone());
    let genesis_runtime_bundle = genesis.map(|value| value.entry.runtime_bundle.identity.clone());
    let default_successor_release = successor.map(|value| value.entry.release.clone());
    let default_successor_runtime_bundle =
        successor.map(|value| value.entry.runtime_bundle.identity.clone());
    let evaluation_digest = canonical_digest(&(
        status,
        &registry_digest,
        &genesis_release,
        &genesis_runtime_bundle,
        &default_successor_release,
        &default_successor_runtime_bundle,
        successor_policy_count,
        &issues,
    ))
    .unwrap_or_default();
    WorkflowReleaseRegistryEvaluation {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION.to_owned(),
        status,
        authority: WorkflowReleaseRegistryEvaluationAuthority::NonAuthoritative,
        registry_id: registry.registry_id.clone(),
        registry_digest,
        lineage_id: registry.lineage_id.clone(),
        genesis_release,
        genesis_runtime_bundle,
        default_successor_release,
        default_successor_runtime_bundle,
        successor_policy_count,
        issues,
        evaluation_digest,
    }
}

fn validate_registry_header(
    document: &WorkflowGovernanceReleaseRegistryDocument,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) {
    let registry = &document.workflow_governance_release_registry;
    if document.schema_version != WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            "unsupported workflow release registry schema version",
        );
    }
    registry_require_text(issues, "registry_id", &registry.registry_id.0);
    registry_require_text(issues, "lineage_id", &registry.lineage_id.0);
    registry_require_text(
        issues,
        "default_successor_release_id",
        &registry.default_successor_release_id.0,
    );
    registry_validate_semver(issues, "registry_version", &registry.registry_version);
    if registry.releases.len() != 2 {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::WrongReleaseCount,
            "releases",
            "foundation registry must contain exactly the implicit P5c genesis and one successor",
        );
    }
}

fn index_supplied_bundles<'a>(
    bundles: &'a [WorkflowGovernanceBundleDocument],
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> BTreeMap<&'a str, &'a WorkflowGovernanceBundleDocument> {
    let mut supplied = BTreeMap::new();
    for (index, bundle) in bundles.iter().enumerate() {
        let id = bundle.workflow_governance_bundle.id.0.as_str();
        if supplied.insert(id, bundle).is_some() {
            registry_issue(
                issues,
                WorkflowReleaseRegistryIssueCode::DuplicateRuntimeBundleIdentity,
                format!("supplied_bundles[{index}].workflow_governance_bundle.id"),
                format!("duplicate supplied runtime bundle id {id}"),
            );
        }
    }
    supplied
}

fn validate_registry_entry_identity(
    registry: &forge_core_contracts::WorkflowGovernanceReleaseRegistry,
    entry: &forge_core_contracts::WorkflowGovernanceReleaseRegistryEntry,
    index: usize,
    identities: &mut RegistryIdentitySets,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) {
    let base = format!("releases[{index}]");
    registry_require_text(
        issues,
        &format!("{base}.release.release_id"),
        &entry.release.release_id.0,
    );
    registry_validate_semver(
        issues,
        &format!("{base}.release.release_version"),
        &entry.release.release_version,
    );
    registry_validate_digest(
        issues,
        &format!("{base}.release.release_digest"),
        &entry.release.release_digest,
    );
    registry_validate_digest(
        issues,
        &format!("{base}.runtime_bundle.identity.bundle_digest"),
        &entry.runtime_bundle.identity.bundle_digest,
    );
    registry_validate_digest(
        issues,
        &format!("{base}.runtime_bundle.identity.policy_set_digest"),
        &entry.runtime_bundle.identity.policy_set_digest,
    );
    registry_validate_digest(
        issues,
        &format!("{base}.runtime_bundle.expected_digest"),
        &entry.runtime_bundle.expected_digest,
    );
    if entry.release.lineage_id != registry.lineage_id {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::LineageMismatch,
            format!("{base}.release.lineage_id"),
            "release lineage must equal registry lineage",
        );
    }
    unique_registry_value(
        &mut identities.release_ids,
        &entry.release.release_id.0,
        issues,
        WorkflowReleaseRegistryIssueCode::DuplicateReleaseIdentity,
        format!("{base}.release.release_id"),
    );
    unique_registry_value(
        &mut identities.release_digests,
        &entry.release.release_digest,
        issues,
        WorkflowReleaseRegistryIssueCode::DuplicateReleaseIdentity,
        format!("{base}.release.release_digest"),
    );
    unique_registry_value(
        &mut identities.bundle_ids,
        &entry.runtime_bundle.identity.bundle_id.0,
        issues,
        WorkflowReleaseRegistryIssueCode::DuplicateRuntimeBundleIdentity,
        format!("{base}.runtime_bundle.identity.bundle_id"),
    );
    unique_registry_value(
        &mut identities.bundle_digests,
        &entry.runtime_bundle.identity.bundle_digest,
        issues,
        WorkflowReleaseRegistryIssueCode::DuplicateRuntimeBundleIdentity,
        format!("{base}.runtime_bundle.identity.bundle_digest"),
    );
}

fn unique_registry_value(
    seen: &mut BTreeSet<String>,
    value: &str,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
    code: WorkflowReleaseRegistryIssueCode,
    path: String,
) {
    if !seen.insert(value.to_owned()) {
        registry_issue(issues, code, path, format!("duplicate identity {value}"));
    }
}

fn resolve_registry_entry<'a>(
    entry: &'a forge_core_contracts::WorkflowGovernanceReleaseRegistryEntry,
    index: usize,
    supplied: &BTreeMap<&str, &WorkflowGovernanceBundleDocument>,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> ResolvedRegistryEntry<'a> {
    let bundle = resolve_registry_bundle(entry, index, supplied, issues);
    let manifest = match &entry.source {
        WorkflowReleaseRegistrySource::ImplicitP5cGenesis => None,
        WorkflowReleaseRegistrySource::EmbeddedManifest {
            embedded_ref,
            expected_digest,
        } => resolve_registry_manifest(entry, index, &embedded_ref.0, expected_digest, issues),
    };
    ResolvedRegistryEntry {
        entry,
        bundle,
        manifest,
    }
}

fn resolve_registry_bundle(
    entry: &forge_core_contracts::WorkflowGovernanceReleaseRegistryEntry,
    index: usize,
    supplied: &BTreeMap<&str, &WorkflowGovernanceBundleDocument>,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> Option<WorkflowGovernanceBundleDocument> {
    let path = entry.runtime_bundle.embedded_ref.0.as_str();
    let embedded = resolve_embedded_registry_yaml::<WorkflowGovernanceBundleDocument>(
        path,
        &entry.runtime_bundle.expected_digest,
        &format!("releases[{index}].runtime_bundle"),
        issues,
    )?;
    if embedded.workflow_governance_bundle.id != entry.runtime_bundle.identity.bundle_id {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::RuntimeBundleIdentityMismatch,
            format!("releases[{index}].runtime_bundle.identity.bundle_id"),
            "embedded runtime bundle id does not match registry identity",
        );
    }
    let found_bundle_digest = workflow_runtime_bundle_digest(&embedded).unwrap_or_default();
    if found_bundle_digest != entry.runtime_bundle.identity.bundle_digest {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::RuntimeBundleIdentityMismatch,
            format!("releases[{index}].runtime_bundle.identity.bundle_digest"),
            "canonical runtime bundle digest does not match registry identity",
        );
    }
    let found_policy_set_digest =
        workflow_policy_set_digest(&embedded.workflow_governance_bundle.policies)
            .unwrap_or_default();
    if found_policy_set_digest != entry.runtime_bundle.identity.policy_set_digest {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::PolicySetDrift,
            format!("releases[{index}].runtime_bundle.identity.policy_set_digest"),
            "embedded runtime policy-set digest does not match registry identity",
        );
    }
    match supplied.get(entry.runtime_bundle.identity.bundle_id.0.as_str()) {
        Some(candidate) if **candidate == embedded => {}
        Some(_) => registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::SuppliedBundleMismatch,
            format!(
                "supplied_bundles.{}",
                entry.runtime_bundle.identity.bundle_id.0
            ),
            "supplied runtime bundle differs from repository-owned embedded bytes",
        ),
        None => registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::SuppliedBundleMissing,
            format!(
                "supplied_bundles.{}",
                entry.runtime_bundle.identity.bundle_id.0
            ),
            "registry runtime bundle was not supplied for audit",
        ),
    }
    Some(embedded)
}

fn resolve_registry_manifest(
    entry: &forge_core_contracts::WorkflowGovernanceReleaseRegistryEntry,
    index: usize,
    path: &str,
    expected_digest: &str,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> Option<WorkflowGovernanceReleaseManifestDocument> {
    let manifest = resolve_embedded_registry_yaml::<WorkflowGovernanceReleaseManifestDocument>(
        path,
        expected_digest,
        &format!("releases[{index}].source"),
        issues,
    )?;
    let identity = &entry.release;
    let subject = &manifest.workflow_governance_release_manifest;
    let canonical_release_digest = workflow_release_manifest_digest(&manifest).unwrap_or_default();
    if subject.lineage_id != identity.lineage_id
        || subject.release_id != identity.release_id
        || subject.release_version != identity.release_version
        || identity.release_digest != canonical_release_digest
    {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::ReleaseManifestIdentityMismatch,
            format!("releases[{index}].release"),
            "release identity must exactly bind its embedded release manifest",
        );
    }
    Some(manifest)
}

fn resolve_embedded_registry_yaml<T: serde::de::DeserializeOwned>(
    path: &str,
    expected_digest: &str,
    issue_path: &str,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> Option<T> {
    let Some(text) = crate::embedded_text(path) else {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::EmbeddedReferenceMissing,
            format!("{issue_path}.embedded_ref"),
            format!("repository-owned embedded document {path} is missing"),
        );
        return None;
    };
    let found = sha256_bytes(text.as_bytes());
    if found != expected_digest {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::EmbeddedDigestMismatch,
            format!("{issue_path}.expected_digest"),
            format!("embedded digest expected {expected_digest}, found {found}"),
        );
    }
    match yaml_serde::from_str(text) {
        Ok(document) => Some(document),
        Err(error) => {
            registry_issue(
                issues,
                WorkflowReleaseRegistryIssueCode::EmbeddedDocumentInvalid,
                format!("{issue_path}.embedded_ref"),
                format!("embedded document is invalid: {error}"),
            );
            None
        }
    }
}

fn validate_registry_chain<'a>(
    registry: &forge_core_contracts::WorkflowGovernanceReleaseRegistry,
    entries: &'a [ResolvedRegistryEntry<'a>],
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> (
    Option<&'a ResolvedRegistryEntry<'a>>,
    Option<&'a ResolvedRegistryEntry<'a>>,
) {
    let genesis_entries = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.entry.source,
                WorkflowReleaseRegistrySource::ImplicitP5cGenesis
            )
        })
        .collect::<Vec<_>>();
    if genesis_entries.len() != 1 {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::GenesisMappingMismatch,
            "releases",
            "registry must contain exactly one implicit P5c genesis mapping",
        );
    }
    let genesis = genesis_entries.first().copied();
    if let Some(genesis) = genesis {
        validate_genesis_mapping(genesis, issues);
    }
    let successor = entries
        .iter()
        .find(|entry| entry.entry.release.release_id == registry.default_successor_release_id);
    if successor.is_none_or(|value| {
        matches!(
            value.entry.source,
            WorkflowReleaseRegistrySource::ImplicitP5cGenesis
        )
    }) {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::DefaultSuccessorMismatch,
            "default_successor_release_id",
            "default successor must identify the single non-genesis release",
        );
    }
    if let (Some(genesis), Some(successor)) = (genesis, successor) {
        let expected = forge_core_contracts::WorkflowReleasePredecessorReference {
            release_id: genesis.entry.release.release_id.clone(),
            release_digest: genesis.entry.release.release_digest.clone(),
        };
        if successor.entry.predecessor.as_ref() != Some(&expected) {
            registry_issue(
                issues,
                WorkflowReleaseRegistryIssueCode::PredecessorMismatch,
                "default_successor.predecessor",
                "successor predecessor must bind the exact genesis release id and digest",
            );
        }
        if successor.entry.receipt_carryover != WorkflowReceiptCarryover::PreservePolicyEquivalent {
            registry_issue(
                issues,
                WorkflowReleaseRegistryIssueCode::PolicySetDrift,
                "default_successor.receipt_carryover",
                "foundation successor must request policy-equivalent receipt carryover",
            );
        }
    }
    (genesis, successor)
}

fn validate_genesis_mapping(
    genesis: &ResolvedRegistryEntry<'_>,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) {
    let entry = genesis.entry;
    if entry.predecessor.is_some()
        || entry.receipt_carryover != WorkflowReceiptCarryover::NotApplicable
        || entry.runtime_bundle.embedded_ref.0
            != "contracts/workflow-governance/golden-path-v0.yaml"
    {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::GenesisMappingMismatch,
            "implicit_p5c_genesis",
            "implicit P5c genesis must map the legacy bundle, have no predecessor, and use not_applicable carryover",
        );
    }
    let expected = workflow_implicit_p5c_release_digest(
        &entry.release.lineage_id,
        &entry.release.release_id,
        &entry.release.release_version,
        &entry.runtime_bundle.identity,
    );
    if expected.as_deref() != Ok(entry.release.release_digest.as_str()) {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::GenesisMappingMismatch,
            "implicit_p5c_genesis.release.release_digest",
            "implicit P5c genesis release digest does not match its canonical mapping subject",
        );
    }
}

fn validate_registry_policy_equivalence(
    genesis: Option<&&ResolvedRegistryEntry<'_>>,
    successor: Option<&&ResolvedRegistryEntry<'_>>,
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
) -> usize {
    let (Some(genesis), Some(successor)) = (genesis, successor) else {
        return 0;
    };
    let (Some(genesis_bundle), Some(successor_bundle), Some(manifest)) = (
        genesis.bundle.as_ref(),
        successor.bundle.as_ref(),
        successor.manifest.as_ref(),
    ) else {
        return 0;
    };
    let genesis_policies = &genesis_bundle.workflow_governance_bundle.policies;
    let successor_policies = &successor_bundle.workflow_governance_bundle.policies;
    if genesis_policies.len() != 15 || successor_policies != genesis_policies {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::PolicySetDrift,
            "default_successor.runtime_bundle.policies",
            "foundation successor must contain the exact fifteen P5c policy objects in canonical order",
        );
    }
    let candidate_ids = manifest
        .workflow_governance_release_manifest
        .workflow_entries
        .iter()
        .filter_map(|entry| match entry.disposition_intent {
            WorkflowReleaseDispositionIntent::MigrationCandidate { .. } => {
                Some(entry.workflow_id.0.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let policy_workflow_ids = successor_policies
        .iter()
        .map(|policy| policy.compatibility_workflow_id.0.as_str())
        .collect::<BTreeSet<_>>();
    if candidate_ids.len() != 15 || policy_workflow_ids != candidate_ids {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::CandidateSetElevation,
            "default_successor.runtime_bundle.policies",
            "runtime bundle must admit exactly the fifteen manifest migration candidates and none of the other 95 workflows",
        );
    }
    successor_policies.len()
}

fn registry_validate_semver(
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
    path: &str,
    value: &str,
) {
    if !is_semver(value) {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::InvalidSemver,
            path,
            "value must be a valid semantic version",
        );
    }
}

fn registry_validate_digest(
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
    path: &str,
    value: &str,
) {
    if !is_sha256_digest(value) {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::InvalidDigest,
            path,
            "value must be a lowercase sha256 digest",
        );
    }
}

fn registry_require_text(issues: &mut Vec<WorkflowReleaseRegistryIssue>, path: &str, value: &str) {
    if value.trim().is_empty() {
        registry_issue(
            issues,
            WorkflowReleaseRegistryIssueCode::InvalidIdentifier,
            path,
            "value must be non-blank",
        );
    }
}

fn registry_issue(
    issues: &mut Vec<WorkflowReleaseRegistryIssue>,
    code: WorkflowReleaseRegistryIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowReleaseRegistryIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}
