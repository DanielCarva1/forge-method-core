//! Pure P5d.5 retirement-evidence and two-axis scorecard evaluation.
//!
//! The evaluator deliberately returns candidate-only output. It recomputes
//! evidence relationships and counts, but cannot verify reviewer credentials or
//! signatures and therefore cannot retire legacy authority.

use std::collections::{BTreeMap, BTreeSet};

use forge_core_contracts::{
    StableId, WorkflowConsumerCompatibilityMatrixDocument,
    WorkflowConsumerCompatibilityReportDocument, WorkflowDeletionProofDocument,
    WorkflowDeletionSurface, WorkflowFinalLegacyAuthorityCounts, WorkflowFinalLegacyAuthorityState,
    WorkflowFinalRuntimeDisposition, WorkflowFinalRuntimeDispositionCounts, WorkflowFinalScorecard,
    WorkflowFinalScorecardAssessment, WorkflowFinalScorecardAuthority,
    WorkflowFinalScorecardDocument, WorkflowGovernanceBundleDocument, WorkflowGovernancePolicy,
    WorkflowGovernanceReleaseIdentity, WorkflowGovernanceReleaseManifestDocument,
    WorkflowReleaseDispositionIntent, WorkflowRetirementArtifactBinding,
    WorkflowRetirementEvidenceIndexDocument, WorkflowRetirementTombstoneCatalogDocument,
    WorkflowRuntimeBundleIdentity, WORKFLOW_CONSUMER_COMPATIBILITY_MATRIX_SCHEMA_VERSION,
    WORKFLOW_CONSUMER_COMPATIBILITY_REPORT_SCHEMA_VERSION, WORKFLOW_DELETION_PROOF_SCHEMA_VERSION,
    WORKFLOW_FINAL_SCORECARD_SCHEMA_VERSION, WORKFLOW_RETIREMENT_EVIDENCE_INDEX_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_TOMBSTONE_CATALOG_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::workflow_release::{
    workflow_policy_set_digest, workflow_release_manifest_digest, workflow_release_policy_digest,
    workflow_runtime_bundle_digest,
};

#[derive(Debug, Clone)]
pub struct WorkflowRetirementCandidateInput {
    pub evidence_index: WorkflowRetirementEvidenceIndexDocument,
    pub evidence_index_binding: WorkflowRetirementArtifactBinding,
    pub deletion_proof: WorkflowDeletionProofDocument,
    pub consumer_matrix: WorkflowConsumerCompatibilityMatrixDocument,
    pub consumer_report: WorkflowConsumerCompatibilityReportDocument,
    pub tombstone_catalog: WorkflowRetirementTombstoneCatalogDocument,
    pub release_manifest: WorkflowGovernanceReleaseManifestDocument,
    pub runtime_bundle: WorkflowGovernanceBundleDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementEvaluationStatus {
    ReadyForIndependentAuthorization,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementEvaluationAuthority {
    CandidateOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementIssueCode {
    UnsupportedSchemaVersion,
    IdentityMismatch,
    ArtifactBindingMismatch,
    RetirementSetMismatch,
    DuplicateWorkflow,
    ReplacementMismatch,
    DeletionProofIncomplete,
    DeletionMismatch,
    ConsumerWindowInvalid,
    ConsumerDiagnosticMissing,
    UnsupportedConsumerPresent,
    TombstoneMismatch,
    ScorecardInvariantFailed,
    CanonicalizationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementIssue {
    pub code: WorkflowRetirementIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementEvaluation {
    pub status: WorkflowRetirementEvaluationStatus,
    pub authority: WorkflowRetirementEvaluationAuthority,
    pub retired_legacy_count: usize,
    pub scorecard: WorkflowFinalScorecardDocument,
    pub issues: Vec<WorkflowRetirementIssue>,
    pub evaluation_digest: String,
}

/// Recomputes the policy-owned projection for one deletion surface.
/// Legacy workflow content is deliberately absent from every projection.
///
/// # Errors
///
/// Returns an encoding error if JCS canonicalization fails.
pub fn workflow_deletion_surface_digest(
    policy: &WorkflowGovernancePolicy,
    surface: WorkflowDeletionSurface,
    release: &WorkflowGovernanceReleaseIdentity,
    runtime: &WorkflowRuntimeBundleIdentity,
    release_history: &WorkflowRetirementArtifactBinding,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct Context<'a, T> {
        domain: &'static str,
        surface: WorkflowDeletionSurface,
        release: &'a WorkflowGovernanceReleaseIdentity,
        runtime: &'a WorkflowRuntimeBundleIdentity,
        release_history: &'a WorkflowRetirementArtifactBinding,
        policy_id: &'a StableId,
        compatibility_workflow_id: &'a StableId,
        projection: T,
    }
    macro_rules! digest {
        ($projection:expr) => {
            canonical_digest(&Context {
                domain: "forge-method:workflow-deletion-surface:v1",
                surface,
                release,
                runtime,
                release_history,
                policy_id: &policy.id,
                compatibility_workflow_id: &policy.compatibility_workflow_id,
                projection: $projection,
            })
        };
    }
    match surface {
        WorkflowDeletionSurface::Routing => digest!((&policy.routing, &policy.eligible_phases)),
        WorkflowDeletionSurface::Readiness => digest!((
            &policy.prerequisites,
            &policy.obligations,
            &policy.capability_requirements,
            &policy.decision_rules,
        )),
        WorkflowDeletionSurface::Verdicts => digest!((&policy.claims, &policy.evaluators)),
        WorkflowDeletionSurface::Receipts => digest!(workflow_release_policy_digest(policy)?),
        WorkflowDeletionSurface::Continuation => digest!((
            &policy.prerequisites,
            &policy.routing.activation,
            &policy.eligible_phases,
            &policy.advisory_playbook.id,
        )),
    }
}

/// Evaluates one complete retirement checkpoint without IO or authority.
#[must_use]
pub fn evaluate_workflow_retirement(
    input: &WorkflowRetirementCandidateInput,
) -> WorkflowRetirementEvaluation {
    let mut issues = Vec::new();
    validate_schema_versions(input, &mut issues);

    let index = &input.evidence_index.workflow_retirement_evidence_index;
    let manifest = &input.release_manifest.workflow_governance_release_manifest;
    let bundle = &input.runtime_bundle.workflow_governance_bundle;
    let release_digest = workflow_release_manifest_digest(&input.release_manifest)
        .unwrap_or_else(|error| digest_error(&mut issues, "release_manifest", error));
    let bundle_digest = workflow_runtime_bundle_digest(&input.runtime_bundle)
        .unwrap_or_else(|error| digest_error(&mut issues, "runtime_bundle", error));
    let policy_set_digest = workflow_policy_set_digest(&bundle.policies)
        .unwrap_or_else(|error| digest_error(&mut issues, "runtime_bundle.policies", error));
    let expected_release = WorkflowGovernanceReleaseIdentity {
        lineage_id: manifest.lineage_id.clone(),
        release_id: manifest.release_id.clone(),
        release_version: manifest.release_version.clone(),
        release_digest,
    };
    let expected_runtime = WorkflowRuntimeBundleIdentity {
        bundle_id: bundle.id.clone(),
        bundle_digest,
        policy_set_digest,
    };
    if index.release != expected_release || index.runtime_bundle != expected_runtime {
        issue(
            &mut issues,
            WorkflowRetirementIssueCode::IdentityMismatch,
            "workflow_retirement_evidence_index",
            "evidence index release/runtime identity does not match the exact manifest and bundle",
        );
    }
    if index.legacy_catalog_digest != manifest.legacy_catalog_digest {
        issue(
            &mut issues,
            WorkflowRetirementIssueCode::IdentityMismatch,
            "workflow_retirement_evidence_index.legacy_catalog_digest",
            "legacy catalog digest does not match the reviewed release manifest",
        );
    }

    validate_artifact_binding(
        &index.deletion_proof,
        &input.deletion_proof.workflow_deletion_proof.id,
        &input.deletion_proof,
        "workflow_retirement_evidence_index.deletion_proof",
        &mut issues,
    );
    validate_artifact_binding(
        &index.consumer_report,
        &input
            .consumer_report
            .workflow_consumer_compatibility_report
            .id,
        &input.consumer_report,
        "workflow_retirement_evidence_index.consumer_report",
        &mut issues,
    );

    let policy_by_workflow = bundle
        .policies
        .iter()
        .map(|policy| (policy.compatibility_workflow_id.0.as_str(), policy))
        .collect::<BTreeMap<_, _>>();
    let migration_ids = manifest
        .workflow_entries
        .iter()
        .filter_map(|entry| {
            matches!(
                entry.disposition_intent,
                WorkflowReleaseDispositionIntent::MigrationCandidate { .. }
            )
            .then_some(entry.workflow_id.0.as_str())
        })
        .collect::<BTreeSet<_>>();
    let retirement_by_id = unique_retirements(index, &mut issues);
    let retirement_ids = retirement_by_id.keys().copied().collect::<BTreeSet<_>>();
    let policy_ids = policy_by_workflow.keys().copied().collect::<BTreeSet<_>>();
    if migration_ids != retirement_ids || policy_ids != retirement_ids {
        issue(
            &mut issues,
            WorkflowRetirementIssueCode::RetirementSetMismatch,
            "workflow_retirement_evidence_index.retirements",
            "retirement set must exactly equal both migration candidates and executable policy compatibility ids",
        );
    }
    for (workflow_id, retirement) in &retirement_by_id {
        let Some(policy) = policy_by_workflow.get(workflow_id) else {
            continue;
        };
        let digest = workflow_release_policy_digest(policy)
            .unwrap_or_else(|error| digest_error(&mut issues, "runtime_bundle.policy", error));
        if retirement.replacement_policy_ref != policy.id
            || retirement.replacement_policy_digest != digest
        {
            issue(
                &mut issues,
                WorkflowRetirementIssueCode::ReplacementMismatch,
                format!("retirements.{workflow_id}"),
                "replacement policy id/digest does not match the executable runtime policy",
            );
        }
    }

    validate_deletion_proof(input, &retirement_ids, &policy_by_workflow, &mut issues);
    validate_consumer_report(input, &retirement_ids, &mut issues);
    validate_tombstones(input, &retirement_by_id, &mut issues);

    let mut assessments = manifest
        .workflow_entries
        .iter()
        .map(|entry| {
            let retired = retirement_ids.contains(entry.workflow_id.0.as_str());
            let runtime_disposition = match entry.disposition_intent {
                WorkflowReleaseDispositionIntent::MigrationCandidate { .. } => {
                    WorkflowFinalRuntimeDisposition::Executable
                }
                WorkflowReleaseDispositionIntent::CompatibilityOnly { .. } => {
                    WorkflowFinalRuntimeDisposition::CompatibilityOnly
                }
                WorkflowReleaseDispositionIntent::Quarantined { .. } => {
                    WorkflowFinalRuntimeDisposition::Quarantined
                }
                WorkflowReleaseDispositionIntent::DomainPackCandidate { .. } => {
                    WorkflowFinalRuntimeDisposition::DomainPackCandidate
                }
                WorkflowReleaseDispositionIntent::RetirementCandidate { .. } => {
                    // A raw retirement intent is never an executable replacement.
                    WorkflowFinalRuntimeDisposition::CompatibilityOnly
                }
            };
            WorkflowFinalScorecardAssessment {
                workflow_id: entry.workflow_id.clone(),
                runtime_disposition,
                legacy_authority: if retired {
                    WorkflowFinalLegacyAuthorityState::Retired
                } else {
                    WorkflowFinalLegacyAuthorityState::Retained
                },
                retirement_evidence_digest: retired
                    .then(|| input.evidence_index_binding.canonical_digest.clone()),
            }
        })
        .collect::<Vec<_>>();
    assessments.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    let runtime_disposition_counts = count_runtime(&assessments);
    let legacy_authority_counts = count_legacy(&assessments);
    if assessments.len() != 110
        || runtime_disposition_counts
            != (WorkflowFinalRuntimeDispositionCounts {
                executable: 42,
                compatibility_only: 47,
                quarantined: 3,
                domain_pack_candidate: 18,
            })
        || legacy_authority_counts
            != (WorkflowFinalLegacyAuthorityCounts {
                retired: 42,
                retained: 68,
            })
    {
        issue(
            &mut issues,
            WorkflowRetirementIssueCode::ScorecardInvariantFailed,
            "workflow_final_scorecard",
            "final two-axis scorecard must be runtime 42/47/3/18 and legacy 42/68 over 110 workflows",
        );
    }

    let mut scorecard = WorkflowFinalScorecardDocument {
        schema_version: WORKFLOW_FINAL_SCORECARD_SCHEMA_VERSION.to_owned(),
        workflow_final_scorecard: WorkflowFinalScorecard {
            id: StableId("workflow-governance.final-scorecard.p5d-v0".to_owned()),
            scorecard_version: "0.1.0".to_owned(),
            authority: WorkflowFinalScorecardAuthority::DerivedCandidateOnly,
            release: index.release.clone(),
            runtime_bundle: index.runtime_bundle.clone(),
            legacy_catalog_digest: index.legacy_catalog_digest.clone(),
            evidence_index: input.evidence_index_binding.clone(),
            runtime_disposition_counts,
            legacy_authority_counts,
            assessments,
            evaluation_digest: String::new(),
        },
    };
    let scorecard_digest = canonical_digest(&scorecard)
        .unwrap_or_else(|error| digest_error(&mut issues, "workflow_final_scorecard", error));
    scorecard.workflow_final_scorecard.evaluation_digest = scorecard_digest;
    issues.sort_by(|left, right| {
        (left.code, left.path.as_str(), left.message.as_str()).cmp(&(
            right.code,
            right.path.as_str(),
            right.message.as_str(),
        ))
    });
    issues.dedup();
    let status = if issues.is_empty() {
        WorkflowRetirementEvaluationStatus::ReadyForIndependentAuthorization
    } else {
        WorkflowRetirementEvaluationStatus::Blocked
    };
    let evaluation_digest = canonical_digest(&(
        status,
        WorkflowRetirementEvaluationAuthority::CandidateOnly,
        &scorecard,
        &issues,
    ))
    .unwrap_or_default();
    WorkflowRetirementEvaluation {
        status,
        authority: WorkflowRetirementEvaluationAuthority::CandidateOnly,
        retired_legacy_count: retirement_ids.len(),
        scorecard,
        issues,
        evaluation_digest,
    }
}

fn validate_schema_versions(
    input: &WorkflowRetirementCandidateInput,
    issues: &mut Vec<WorkflowRetirementIssue>,
) {
    for (path, found, expected) in [
        (
            "evidence_index.schema_version",
            input.evidence_index.schema_version.as_str(),
            WORKFLOW_RETIREMENT_EVIDENCE_INDEX_SCHEMA_VERSION,
        ),
        (
            "deletion_proof.schema_version",
            input.deletion_proof.schema_version.as_str(),
            WORKFLOW_DELETION_PROOF_SCHEMA_VERSION,
        ),
        (
            "consumer_matrix.schema_version",
            input.consumer_matrix.schema_version.as_str(),
            WORKFLOW_CONSUMER_COMPATIBILITY_MATRIX_SCHEMA_VERSION,
        ),
        (
            "consumer_report.schema_version",
            input.consumer_report.schema_version.as_str(),
            WORKFLOW_CONSUMER_COMPATIBILITY_REPORT_SCHEMA_VERSION,
        ),
        (
            "tombstone_catalog.schema_version",
            input.tombstone_catalog.schema_version.as_str(),
            WORKFLOW_RETIREMENT_TOMBSTONE_CATALOG_SCHEMA_VERSION,
        ),
    ] {
        if found != expected {
            issue(
                issues,
                WorkflowRetirementIssueCode::UnsupportedSchemaVersion,
                path,
                format!("expected schema {expected}, found {found}"),
            );
        }
    }
}

fn unique_retirements<'a>(
    index: &'a forge_core_contracts::WorkflowRetirementEvidenceIndex,
    issues: &mut Vec<WorkflowRetirementIssue>,
) -> BTreeMap<&'a str, &'a forge_core_contracts::WorkflowRetirementWorkflowBinding> {
    let mut result = BTreeMap::new();
    for retirement in &index.retirements {
        if result
            .insert(retirement.workflow_id.0.as_str(), retirement)
            .is_some()
        {
            issue(
                issues,
                WorkflowRetirementIssueCode::DuplicateWorkflow,
                "workflow_retirement_evidence_index.retirements",
                format!("duplicate retirement {}", retirement.workflow_id.0),
            );
        }
    }
    result
}

fn validate_deletion_proof(
    input: &WorkflowRetirementCandidateInput,
    retirement_ids: &BTreeSet<&str>,
    policy_by_workflow: &BTreeMap<&str, &WorkflowGovernancePolicy>,
    issues: &mut Vec<WorkflowRetirementIssue>,
) {
    let proof = &input.deletion_proof.workflow_deletion_proof;
    let index = &input.evidence_index.workflow_retirement_evidence_index;
    if proof.release != index.release
        || proof.runtime_bundle != index.runtime_bundle
        || proof.legacy_catalog_digest != index.legacy_catalog_digest
        || proof.release_history != index.release_history
        || proof.mismatch_count != 0
        || proof.evaluation_error_count != 0
    {
        issue(
            issues,
            WorkflowRetirementIssueCode::DeletionMismatch,
            "workflow_deletion_proof",
            "deletion proof identity/counts do not match the evidence index or are not clean",
        );
    }
    let expected_surfaces = BTreeSet::from([
        WorkflowDeletionSurface::Routing,
        WorkflowDeletionSurface::Readiness,
        WorkflowDeletionSurface::Verdicts,
        WorkflowDeletionSurface::Receipts,
        WorkflowDeletionSurface::Continuation,
    ]);
    let mut found = BTreeSet::new();
    for entry in &proof.workflows {
        let id = entry.retirement.workflow_id.0.as_str();
        if !found.insert(id) {
            issue(
                issues,
                WorkflowRetirementIssueCode::DuplicateWorkflow,
                "workflow_deletion_proof.workflows",
                format!("duplicate deletion proof {id}"),
            );
        }
        let surfaces = entry
            .surfaces
            .iter()
            .map(|surface| surface.surface)
            .collect::<BTreeSet<_>>();
        let expected = entry
            .surfaces
            .iter()
            .map(|surface| {
                policy_by_workflow.get(id).map_or_else(
                    || Err("replacement policy missing".to_owned()),
                    |policy| {
                        workflow_deletion_surface_digest(
                            policy,
                            surface.surface,
                            &index.release,
                            &index.runtime_bundle,
                            &index.release_history,
                        )
                    },
                )
            })
            .collect::<Vec<_>>();
        if !entry.legacy_present_in_control
            || entry.legacy_present_after_ablation
            || surfaces != expected_surfaces
            || entry.surfaces.len() != expected_surfaces.len()
            || entry
                .surfaces
                .iter()
                .zip(expected)
                .any(|(surface, expected)| {
                    let expected = expected.as_deref().unwrap_or_default();
                    !surface.equivalent
                        || surface.control_digest != expected
                        || surface.legacy_ablated_digest != expected
                })
        {
            issue(
                issues,
                WorkflowRetirementIssueCode::DeletionProofIncomplete,
                format!("workflow_deletion_proof.workflows.{id}"),
                "proof must cover five unique surfaces with exact control/ablation equality",
            );
        }
    }
    if &found != retirement_ids {
        issue(
            issues,
            WorkflowRetirementIssueCode::RetirementSetMismatch,
            "workflow_deletion_proof.workflows",
            "deletion proof set does not equal the exact retirement set",
        );
    }
}

fn validate_consumer_report(
    input: &WorkflowRetirementCandidateInput,
    retirement_ids: &BTreeSet<&str>,
    issues: &mut Vec<WorkflowRetirementIssue>,
) {
    let report = &input.consumer_report.workflow_consumer_compatibility_report;
    let index = &input.evidence_index.workflow_retirement_evidence_index;
    let matrix = &input.consumer_matrix.workflow_consumer_compatibility_matrix;
    validate_artifact_binding(
        &report.compatibility_matrix,
        &matrix.id,
        &input.consumer_matrix,
        "workflow_consumer_compatibility_report.compatibility_matrix",
        issues,
    );
    if report.release != index.release
        || report.legacy_catalog_digest != index.legacy_catalog_digest
        || matrix.release != index.release
        || matrix.legacy_catalog_digest != index.legacy_catalog_digest
    {
        issue(
            issues,
            WorkflowRetirementIssueCode::IdentityMismatch,
            "workflow_consumer_compatibility_report",
            "consumer report identity does not match evidence index",
        );
    }
    if !(report.announced_at_unix < report.observed_from_unix
        && report.observed_from_unix <= report.observed_until_unix
        && report.retirement_not_before_unix <= report.observed_until_unix)
        || report.minimum_consumer_version.trim().is_empty()
        || !valid_digest(&report.consumer_population_digest)
        || report.consumer_population_digest != matrix.operational_catalog_digest
    {
        issue(
            issues,
            WorkflowRetirementIssueCode::ConsumerWindowInvalid,
            "workflow_consumer_compatibility_report",
            "consumer window ordering, minimum version, or population digest is invalid",
        );
    }
    let matrix_by_id = matrix
        .entries
        .iter()
        .map(|entry| (entry.workflow_id.0.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if matrix_by_id.len() != matrix.entries.len()
        || matrix_by_id.keys().copied().collect::<BTreeSet<_>>() != *retirement_ids
    {
        issue(
            issues,
            WorkflowRetirementIssueCode::RetirementSetMismatch,
            "workflow_consumer_compatibility_matrix.entries",
            "repository compatibility matrix must cover the exact retirement set",
        );
    }
    let mut found = BTreeSet::new();
    for entry in &report.workflows {
        let id = entry.workflow_id.0.as_str();
        if !found.insert(id) {
            issue(
                issues,
                WorkflowRetirementIssueCode::DuplicateWorkflow,
                "workflow_consumer_compatibility_report.workflows",
                format!("duplicate consumer observation {id}"),
            );
        }
        let matrix_entry = matrix_by_id.get(id);
        if entry.diagnostic_code.0.trim().is_empty()
            || entry.replacement_argv.is_empty()
            || entry
                .replacement_argv
                .iter()
                .any(|arg| arg.trim().is_empty())
            || entry.diagnostic_fixture_count == 0
            || matrix_entry.is_none_or(|matrix_entry| {
                matrix_entry.diagnostic_code != entry.diagnostic_code
                    || matrix_entry.replacement_policy_ref != entry.replacement_policy_ref
                    || matrix_entry.replacement_argv != entry.replacement_argv
                    || matrix_entry.repository_fixture_refs.is_empty()
            })
        {
            issue(
                issues,
                WorkflowRetirementIssueCode::ConsumerDiagnosticMissing,
                format!("workflow_consumer_compatibility_report.workflows.{id}"),
                "typed replacement diagnostic must match a real repository matrix fixture",
            );
        }
        if entry.unsupported_repository_consumer_count != 0 {
            issue(
                issues,
                WorkflowRetirementIssueCode::UnsupportedConsumerPresent,
                format!("workflow_consumer_compatibility_report.workflows.{id}"),
                "unsupported repository consumers remain at retirement boundary",
            );
        }
    }
    if &found != retirement_ids {
        issue(
            issues,
            WorkflowRetirementIssueCode::RetirementSetMismatch,
            "workflow_consumer_compatibility_report.workflows",
            "consumer report set does not equal the exact retirement set",
        );
    }
}

fn validate_tombstones(
    input: &WorkflowRetirementCandidateInput,
    retirements: &BTreeMap<&str, &forge_core_contracts::WorkflowRetirementWorkflowBinding>,
    issues: &mut Vec<WorkflowRetirementIssue>,
) {
    let catalog = &input
        .tombstone_catalog
        .workflow_retirement_tombstone_catalog;
    let index = &input.evidence_index.workflow_retirement_evidence_index;
    if catalog.release != index.release {
        issue(
            issues,
            WorkflowRetirementIssueCode::IdentityMismatch,
            "workflow_retirement_tombstone_catalog.release",
            "tombstone release does not match evidence index",
        );
    }
    let mut found = BTreeSet::new();
    for tombstone in &catalog.tombstones {
        let id = tombstone.workflow_id.0.as_str();
        let Some(retirement) = retirements.get(id) else {
            issue(
                issues,
                WorkflowRetirementIssueCode::TombstoneMismatch,
                format!("workflow_retirement_tombstone_catalog.tombstones.{id}"),
                "tombstone is not in the retirement set",
            );
            continue;
        };
        if !found.insert(id)
            || tombstone.legacy_workflow_digest != retirement.legacy_workflow_digest
            || tombstone.replacement_policy_ref != retirement.replacement_policy_ref
            || tombstone.replacement_release_id != index.release.release_id
            || tombstone.diagnostic_code.0.trim().is_empty()
            || tombstone.replacement_argv.is_empty()
            || tombstone
                .replacement_argv
                .iter()
                .any(|arg| arg.trim().is_empty())
        {
            issue(
                issues,
                WorkflowRetirementIssueCode::TombstoneMismatch,
                format!("workflow_retirement_tombstone_catalog.tombstones.{id}"),
                "tombstone must exactly bind legacy identity and typed replacement diagnostics",
            );
        }
    }
    if found.len() != retirements.len() {
        issue(
            issues,
            WorkflowRetirementIssueCode::RetirementSetMismatch,
            "workflow_retirement_tombstone_catalog.tombstones",
            "tombstone set does not equal the exact retirement set",
        );
    }
}

fn validate_artifact_binding<T: Serialize>(
    binding: &WorkflowRetirementArtifactBinding,
    expected_id: &StableId,
    document: &T,
    path: &str,
    issues: &mut Vec<WorkflowRetirementIssue>,
) {
    let canonical =
        canonical_digest(document).unwrap_or_else(|error| digest_error(issues, path, error));
    if &binding.artifact_id != expected_id
        || binding.embedded_ref.0.trim().is_empty()
        || !valid_digest(&binding.raw_digest)
        || binding.canonical_digest != canonical
    {
        issue(
            issues,
            WorkflowRetirementIssueCode::ArtifactBindingMismatch,
            path,
            "artifact id/ref/raw/canonical binding mismatch",
        );
    }
}

fn count_runtime(
    assessments: &[WorkflowFinalScorecardAssessment],
) -> WorkflowFinalRuntimeDispositionCounts {
    let mut counts = WorkflowFinalRuntimeDispositionCounts::default();
    for assessment in assessments {
        match assessment.runtime_disposition {
            WorkflowFinalRuntimeDisposition::Executable => counts.executable += 1,
            WorkflowFinalRuntimeDisposition::CompatibilityOnly => counts.compatibility_only += 1,
            WorkflowFinalRuntimeDisposition::Quarantined => counts.quarantined += 1,
            WorkflowFinalRuntimeDisposition::DomainPackCandidate => {
                counts.domain_pack_candidate += 1;
            }
        }
    }
    counts
}

fn count_legacy(
    assessments: &[WorkflowFinalScorecardAssessment],
) -> WorkflowFinalLegacyAuthorityCounts {
    let mut counts = WorkflowFinalLegacyAuthorityCounts::default();
    for assessment in assessments {
        match assessment.legacy_authority {
            WorkflowFinalLegacyAuthorityState::Retired => counts.retired += 1,
            WorkflowFinalLegacyAuthorityState::Retained => counts.retained += 1,
        }
    }
    counts
}

fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn digest_error(issues: &mut Vec<WorkflowRetirementIssue>, path: &str, error: String) -> String {
    issue(
        issues,
        WorkflowRetirementIssueCode::CanonicalizationFailed,
        path,
        error,
    );
    String::new()
}

fn issue(
    issues: &mut Vec<WorkflowRetirementIssue>,
    code: WorkflowRetirementIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowRetirementIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}
