//! Pure, non-authoritative P5d.4b release-at-a-time admission review.
//!
//! This module recomputes every review-relevant binding and derives only
//! whether exactly one adjacent candidate is ready to be presented to the independent signature
//! authority. It cannot verify signatures, construct an opaque authorization
//! capability, mutate a registry, or upgrade a project.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use forge_core_contracts::workflow_release_review_v2::{
    WorkflowReleaseReviewArtifactBindingV2, WorkflowReleaseReviewIndexV2Document,
};
use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicyDocument, WorkflowBehavioralDisposition,
    WorkflowBehavioralReviewSubjectDocument, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralShadowReportDocument, WorkflowBehavioralVerdict,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowMigrationBatchDocument,
    WorkflowMigrationPlanDocument, WorkflowReceiptCarryover, WorkflowReleaseDispositionIntent,
    WorkflowReleaseRegistrySource, WorkflowReleaseReviewDecision,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::catalog::LoadedWorkflowDocument;
use crate::workflow_behavior::{
    evaluate_workflow_behavior, WorkflowBehavioralBundleInput, WorkflowBehavioralCorpusInput,
    WorkflowBehavioralReportIdentity,
};
use crate::workflow_migration::WorkflowMigrationAudit;
use crate::workflow_release::{
    evaluate_workflow_release, evaluate_workflow_release_registry_evolution,
    workflow_policy_set_digest, workflow_release_manifest_digest, workflow_release_registry_digest,
    workflow_runtime_bundle_digest, WorkflowReleaseEvaluationStatus,
    WorkflowReleaseRegistryEvaluationStatus, WorkflowReleaseRegistryEvolutionArtifact,
};

const EVALUATION_DIGEST_ALGORITHM: &str = "forge.workflow-release-admission.v2/jcs-sha256/1";

/// Closed typed inputs used to recompute the P5d.4 review projection.
///
/// Raw bytes remain separate from parsed documents. The evaluator recomputes
/// every raw and canonical digest and never accepts caller-authored counts.
#[derive(Debug, Clone)]
pub struct WorkflowReleaseAdmissionCandidateV2Input {
    pub review_index: WorkflowReleaseReviewIndexV2Document,
    pub report_identity: WorkflowBehavioralReportIdentity,
    pub coverage_policy: WorkflowBehavioralCoveragePolicyDocument,
    pub corpus_set: WorkflowBehavioralCorpusSetDocument,
    pub representative_corpus: WorkflowBehavioralScenarioCorpusDocument,
    pub adversarial_corpus: WorkflowBehavioralScenarioCorpusDocument,
    pub review_subject: WorkflowBehavioralReviewSubjectDocument,
    pub behavioral_bundles: BTreeMap<String, WorkflowBehavioralBundleInput>,
    pub authored_shadow_report: WorkflowBehavioralShadowReportDocument,
    pub migration_batches: Vec<WorkflowMigrationBatchDocument>,
    pub migration_plan: WorkflowMigrationPlanDocument,
    /// Trusted, deterministic P5a projection recomputed from the same catalog.
    pub migration_audit: WorkflowMigrationAudit,
    /// The complete typed legacy catalog. The release evaluator proves its
    /// identity, per-workflow digests, and exhaustive disposition set.
    pub legacy_workflows: Vec<LoadedWorkflowDocument>,
    pub candidate_manifest: WorkflowGovernanceReleaseManifestDocument,
    pub candidate_runtime_bundle: WorkflowGovernanceBundleDocument,
    pub promoted_runtime_bundle: WorkflowGovernanceBundleDocument,
    pub predecessor_registry: WorkflowGovernanceReleaseRegistryDocument,
    pub proposed_registry: WorkflowGovernanceReleaseRegistryDocument,
    pub registry_bundles: Vec<WorkflowGovernanceBundleDocument>,
    pub source_bytes: HashMap<RepoPath, Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionV2EvaluationStatus {
    Blocked,
    ReadyForIndependentAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionV2EvaluationAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionV2IssueCode {
    InvalidReviewIndex,
    MissingArtifactBytes,
    RawDigestMismatch,
    CanonicalDigestMismatch,
    BehavioralReportMismatch,
    BehavioralEvidenceIncomplete,
    ReviewDecisionBlocked,
    ReviewSetMismatch,
    PromotionBindingMismatch,
    PolicySetDrift,
    PolicyCountMismatch,
    CatalogDispositionMismatch,
    RegistryEvolutionInvalid,
    RegistryShapeMismatch,
    PredecessorMismatch,
    ReceiptCarryoverInvalid,
    FrozenHistoryIncompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionV2Issue {
    pub code: WorkflowReleaseAdmissionV2IssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionV2Evaluation {
    pub algorithm_version: String,
    pub status: WorkflowReleaseAdmissionV2EvaluationStatus,
    pub authority: WorkflowReleaseAdmissionV2EvaluationAuthority,
    pub review_index_id: StableId,
    pub review_index_digest: String,
    pub candidate_release_digest: String,
    pub candidate_policy_count: usize,
    pub predecessor_policy_count: usize,
    pub reviewed_workflow_count: usize,
    pub quarantine_count: usize,
    pub behavioral_mismatch_count: usize,
    pub behavioral_evaluation_error_count: usize,
    pub issues: Vec<WorkflowReleaseAdmissionV2Issue>,
    pub evaluation_digest: String,
}

/// Recompute one candidate's complete review surface without granting release
/// or runtime authority.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn evaluate_workflow_release_admission_candidate_v2(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
) -> WorkflowReleaseAdmissionV2Evaluation {
    let mut issues = Vec::new();
    let index = &input.review_index.workflow_release_review_index;
    for issue in input.review_index.validate() {
        push_issue(
            &mut issues,
            WorkflowReleaseAdmissionV2IssueCode::InvalidReviewIndex,
            issue.path,
            issue.message,
        );
    }

    verify_typed_binding(
        &mut issues,
        "review_index.release_manifest",
        &index.release_manifest,
        &input.candidate_manifest,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.coverage_policy",
        &index.coverage_policy,
        &input.coverage_policy,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.full_catalog",
        &index.full_catalog,
        &input.migration_plan,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.corpus_set",
        &index.corpus_set,
        &input.corpus_set,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.representative_corpus",
        &index.representative_corpus,
        &input.representative_corpus,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.adversarial_corpus",
        &index.adversarial_corpus,
        &input.adversarial_corpus,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.shadow_report",
        &index.shadow_report,
        &input.authored_shadow_report,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.candidate_runtime_bundle",
        &index.candidate_runtime_bundle,
        &input.candidate_runtime_bundle,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.promoted_runtime_bundle",
        &index.promoted_runtime_bundle,
        &input.promoted_runtime_bundle,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.predecessor_registry",
        &index.predecessor_registry,
        &input.predecessor_registry,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.proposed_registry",
        &index.proposed_registry,
        &input.proposed_registry,
        &input.source_bytes,
    );
    verify_typed_set(
        &mut issues,
        "review_index.migration_batches",
        &index.migration_batches,
        &input.migration_batches,
        &input.source_bytes,
    );
    verify_typed_binding(
        &mut issues,
        "review_index.review_subject",
        &index.review_subject,
        &input.review_subject,
        &input.source_bytes,
    );
    verify_source_binding(
        &mut issues,
        "review_index.evaluator_source",
        &index.evaluator_source,
        &input.source_bytes,
        SourceCanonicalDomain::Utf8Text,
    );
    verify_source_binding(
        &mut issues,
        "review_index.frozen_history",
        &index.frozen_history,
        &input.source_bytes,
        SourceCanonicalDomain::JsonLines,
    );

    let corpora = vec![
        WorkflowBehavioralCorpusInput {
            artifact: artifact_reference(&index.representative_corpus),
            document: input.representative_corpus.clone(),
        },
        WorkflowBehavioralCorpusInput {
            artifact: artifact_reference(&index.adversarial_corpus),
            document: input.adversarial_corpus.clone(),
        },
    ];
    let recomputed_report = evaluate_workflow_behavior(
        &input.report_identity,
        &input.coverage_policy,
        &input.corpus_set,
        &input.review_subject,
        &corpora,
        &input.behavioral_bundles,
        &input.source_bytes,
    );
    if recomputed_report != input.authored_shadow_report {
        push_issue(
            &mut issues,
            WorkflowReleaseAdmissionV2IssueCode::BehavioralReportMismatch,
            "shadow_report",
            "authored shadow report differs from deterministic recomputation",
        );
    }
    let report = &recomputed_report.workflow_behavioral_shadow_report;
    let (behavioral_mismatch_count, behavioral_evaluation_error_count) = report
        .workflow_reports
        .iter()
        .fold((0_usize, 0_usize), |(mismatches, errors), workflow| {
            (
                mismatches + usize::from(workflow.mismatch_count),
                errors + usize::from(workflow.evaluation_error_count),
            )
        });
    let expected_review_count = input
        .review_subject
        .workflow_behavioral_review_subject
        .candidate_workflows
        .len();
    if report.verdict != WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
        || report.disposition != WorkflowBehavioralDisposition::ReviewCandidate
        || behavioral_mismatch_count != 0
        || behavioral_evaluation_error_count != 0
        || expected_review_count == 0
        || report.workflow_reports.len() != expected_review_count
        || report
            .workflow_reports
            .iter()
            .any(|workflow| workflow.total_scenarios == 0)
    {
        push_issue(
            &mut issues,
            WorkflowReleaseAdmissionV2IssueCode::BehavioralEvidenceIncomplete,
            "shadow_report",
            "behavioral evidence must recompute every nonempty reviewed workflow with scenarios and zero mismatches or errors",
        );
    }

    validate_review_decisions(input, &recomputed_report, &mut issues);
    validate_promotion(input, &mut issues);
    let (predecessor_policy_count, candidate_policy_count) =
        validate_policy_composition(input, &mut issues);
    let quarantine_count = validate_dispositions(input, &mut issues);
    validate_registry_evolution(input, &mut issues);

    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    let status = if issues.is_empty() {
        WorkflowReleaseAdmissionV2EvaluationStatus::ReadyForIndependentAuthorization
    } else {
        WorkflowReleaseAdmissionV2EvaluationStatus::Blocked
    };
    let review_index_digest = canonical_digest(&input.review_index).unwrap_or_default();
    let candidate_release_digest =
        workflow_release_manifest_digest(&input.candidate_manifest).unwrap_or_default();
    let reviewed_workflow_count = report.workflow_reports.len();
    let evaluation_digest = canonical_digest(&(
        EVALUATION_DIGEST_ALGORITHM,
        status,
        &index.id,
        &review_index_digest,
        &candidate_release_digest,
        candidate_policy_count,
        predecessor_policy_count,
        reviewed_workflow_count,
        quarantine_count,
        behavioral_mismatch_count,
        behavioral_evaluation_error_count,
        &issues,
    ))
    .unwrap_or_default();
    WorkflowReleaseAdmissionV2Evaluation {
        algorithm_version: EVALUATION_DIGEST_ALGORITHM.to_owned(),
        status,
        authority: WorkflowReleaseAdmissionV2EvaluationAuthority::NonAuthoritative,
        review_index_id: index.id.clone(),
        review_index_digest,
        candidate_release_digest,
        candidate_policy_count,
        predecessor_policy_count,
        reviewed_workflow_count,
        quarantine_count,
        behavioral_mismatch_count,
        behavioral_evaluation_error_count,
        issues,
        evaluation_digest,
    }
}

fn validate_review_decisions(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    report: &WorkflowBehavioralShadowReportDocument,
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
) {
    let index = &input.review_index.workflow_release_review_index;
    if index
        .workflow_decisions
        .iter()
        .any(|decision| decision.decision != WorkflowReleaseReviewDecision::Approved)
        || index
            .quarantine_decisions
            .iter()
            .any(|decision| decision.decision != WorkflowReleaseReviewDecision::Approved)
        || index
            .dimension_decisions
            .iter()
            .any(|decision| decision.decision != WorkflowReleaseReviewDecision::Approved)
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::ReviewDecisionBlocked,
            "review_index.decisions",
            "every workflow, quarantine, and governed dimension requires explicit approval",
        );
    }
    let reviewed = index
        .workflow_decisions
        .iter()
        .map(|decision| &decision.workflow_id)
        .collect::<BTreeSet<_>>();
    let reported = report
        .workflow_behavioral_shadow_report
        .workflow_reports
        .iter()
        .map(|workflow| &workflow.bindings.workflow_id)
        .collect::<BTreeSet<_>>();
    let subject = input
        .review_subject
        .workflow_behavioral_review_subject
        .candidate_workflows
        .iter()
        .map(|workflow| &workflow.workflow_id)
        .collect::<BTreeSet<_>>();
    if reviewed != reported || reviewed != subject {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::ReviewSetMismatch,
            "review_index.workflow_decisions",
            "reviewed workflow ids must equal the recomputed report and review subject",
        );
    }
    let reviewed_quarantines = index
        .quarantine_decisions
        .iter()
        .map(|decision| &decision.workflow_id)
        .collect::<BTreeSet<_>>();
    let subject_quarantines = input
        .review_subject
        .workflow_behavioral_review_subject
        .quarantines
        .iter()
        .map(|quarantine| &quarantine.workflow_id)
        .collect::<BTreeSet<_>>();
    if reviewed_quarantines != subject_quarantines {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::ReviewSetMismatch,
            "review_index.quarantine_decisions",
            "reviewed quarantine ids must equal the review subject quarantines",
        );
    }
}

fn validate_promotion(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
) {
    let index = &input.review_index.workflow_release_review_index;
    let subject = &input.review_subject.workflow_behavioral_review_subject;
    let manifest = &input
        .candidate_manifest
        .workflow_governance_release_manifest;
    let manifest_digest = workflow_release_manifest_digest(&input.candidate_manifest).ok();
    let promoted_bundle_digest =
        workflow_runtime_bundle_digest(&input.promoted_runtime_bundle).ok();
    let promoted_policy_digest = workflow_policy_set_digest(
        &input
            .promoted_runtime_bundle
            .workflow_governance_bundle
            .policies,
    )
    .ok();
    let promoted_identity = &index.promotion.promoted_runtime_bundle;
    if index.promotion.candidate_release.lineage_id != manifest.lineage_id
        || index.promotion.candidate_release.release_id != manifest.release_id
        || index.promotion.candidate_release.release_version != manifest.release_version
        || Some(&index.promotion.candidate_release.release_digest) != manifest_digest.as_ref()
        || promoted_identity.bundle_id
            != input.promoted_runtime_bundle.workflow_governance_bundle.id
        || Some(&promoted_identity.bundle_digest) != promoted_bundle_digest.as_ref()
        || Some(&promoted_identity.policy_set_digest) != promoted_policy_digest.as_ref()
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PromotionBindingMismatch,
            "review_index.promotion",
            "promotion identities do not match the recomputed manifest and promoted bundle",
        );
    }
    if index.promotion.predecessor.release_id != subject.baseline_release.release_id
        || index.promotion.predecessor.release_digest != subject.baseline_release.release_digest
        || manifest.previous_release_digest.as_ref()
            != Some(&index.promotion.predecessor.release_digest)
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PredecessorMismatch,
            "review_index.promotion.predecessor",
            "promotion must be adjacent to the exact frozen foundation release",
        );
    }
    let candidate_identity = &subject.runtime_bundle;
    let candidate_bundle_digest =
        workflow_runtime_bundle_digest(&input.candidate_runtime_bundle).ok();
    let candidate_policy_digest = workflow_policy_set_digest(
        &input
            .candidate_runtime_bundle
            .workflow_governance_bundle
            .policies,
    )
    .ok();
    if candidate_identity.bundle_id != input.candidate_runtime_bundle.workflow_governance_bundle.id
        || Some(&candidate_identity.bundle_digest) != candidate_bundle_digest.as_ref()
        || Some(&candidate_identity.policy_set_digest) != candidate_policy_digest.as_ref()
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PromotionBindingMismatch,
            "review_subject.runtime_bundle",
            "review subject does not identify the exact candidate bundle",
        );
    }
}

fn validate_policy_composition(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
) -> (usize, usize) {
    let candidate = &input
        .candidate_runtime_bundle
        .workflow_governance_bundle
        .policies;
    let promoted = &input
        .promoted_runtime_bundle
        .workflow_governance_bundle
        .policies;
    if candidate != promoted {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PolicySetDrift,
            "promoted_runtime_bundle.policies",
            "candidate and promoted ordered policy sets must be byte-semantically equal",
        );
    }
    // A V2 review is always relative to the current registry tail. Looking up
    // an older matching release would permit a valid review to skip history.
    let predecessor_entry = input
        .predecessor_registry
        .workflow_governance_release_registry
        .releases
        .last()
        .filter(|entry| {
            let reviewed = &input
                .review_index
                .workflow_release_review_index
                .promotion
                .predecessor;
            entry.release.release_id == reviewed.release_id
                && entry.release.release_digest == reviewed.release_digest
        });
    let predecessor_bundle = predecessor_entry.and_then(|entry| {
        input.registry_bundles.iter().find(|bundle| {
            bundle.workflow_governance_bundle.id == entry.runtime_bundle.identity.bundle_id
        })
    });
    let predecessor_count =
        predecessor_bundle.map_or(0, |bundle| bundle.workflow_governance_bundle.policies.len());
    let predecessor_policies = predecessor_bundle
        .map(|bundle| bundle.workflow_governance_bundle.policies.as_slice())
        .unwrap_or_default();
    let predecessor_ids = predecessor_policies
        .iter()
        .map(|policy| &policy.id)
        .collect::<BTreeSet<_>>();
    let candidate_ids = candidate
        .iter()
        .map(|policy| &policy.id)
        .collect::<BTreeSet<_>>();
    let delta_policies = exact_nonempty_delta(predecessor_policies, candidate).unwrap_or_default();
    let ordered_prefix_preserved = !delta_policies.is_empty();
    let delta = delta_policies.len();
    let delta_workflow_ids = candidate
        .iter()
        .filter(|policy| !predecessor_ids.contains(&policy.id))
        .map(|policy| &policy.compatibility_workflow_id)
        .collect::<BTreeSet<_>>();
    let reviewed_workflow_ids = input
        .review_index
        .workflow_release_review_index
        .workflow_decisions
        .iter()
        .map(|decision| &decision.workflow_id)
        .collect::<BTreeSet<_>>();
    let manifest = &input
        .candidate_manifest
        .workflow_governance_release_manifest;
    let mut composed_policies = predecessor_policies.to_vec();
    let mut batch_bindings_valid = manifest.batches.len() == input.migration_batches.len();
    for batch_ref in &manifest.batches {
        let batch = input
            .migration_batches
            .iter()
            .find(|document| document.workflow_migration_batch.id == batch_ref.batch_id);
        let Some(batch) = batch else {
            batch_bindings_valid = false;
            continue;
        };
        let batch_data = &batch.workflow_migration_batch;
        batch_bindings_valid &= batch_data.batch_version == batch_ref.batch_version;
        batch_bindings_valid &= input.source_bytes.contains_key(&batch_ref.embedded_ref);
        batch_bindings_valid &= canonical_digest(batch).as_ref() == Ok(&batch_ref.expected_digest);
        composed_policies.extend(
            batch_data
                .policies
                .iter()
                .filter(|policy| !predecessor_ids.contains(&policy.id))
                .cloned(),
        );
    }
    let all_ids_unique = candidate_ids.len() == candidate.len()
        && predecessor_ids.len() == predecessor_policies.len();
    if predecessor_entry.is_none()
        || predecessor_bundle.is_none()
        || delta == 0
        || candidate.len() != predecessor_count + delta
        || promoted.len() != candidate.len()
        || !ordered_prefix_preserved
        || !all_ids_unique
        || delta_workflow_ids.len() != delta
        || reviewed_workflow_ids.len()
            != input
                .review_index
                .workflow_release_review_index
                .workflow_decisions
                .len()
        || delta_workflow_ids != reviewed_workflow_ids
        || !batch_bindings_valid
        || composed_policies != *candidate
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PolicyCountMismatch,
            "promoted_runtime_bundle.policies",
            "successor must preserve the predecessor policy ids and order exactly, then append a nonempty delta equal to the reviewed workflow set",
        );
    }
    (predecessor_count, candidate.len())
}

fn validate_dispositions(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
) -> usize {
    let entries = &input
        .candidate_manifest
        .workflow_governance_release_manifest
        .workflow_entries;
    let migration = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.disposition_intent,
                WorkflowReleaseDispositionIntent::MigrationCandidate { .. }
            )
        })
        .count();
    let quarantined = entries
        .iter()
        .filter_map(|entry| {
            matches!(
                entry.disposition_intent,
                WorkflowReleaseDispositionIntent::Quarantined { .. }
            )
            .then_some(&entry.workflow_id)
        })
        .collect::<BTreeSet<_>>();
    let reviewed_quarantines = input
        .review_index
        .workflow_release_review_index
        .quarantine_decisions
        .iter()
        .map(|decision| &decision.workflow_id)
        .collect::<BTreeSet<_>>();
    let subject_quarantines = &input
        .review_subject
        .workflow_behavioral_review_subject
        .quarantines;
    let quarantine_details_match = entries.iter().all(|entry| {
        let WorkflowReleaseDispositionIntent::Quarantined { quarantine } =
            &entry.disposition_intent
        else {
            return true;
        };
        subject_quarantines.iter().any(|subject| {
            subject.workflow_id == entry.workflow_id && subject.quarantine == *quarantine
        })
    });
    let release_evaluation = evaluate_workflow_release(
        &input.candidate_manifest,
        &input.migration_batches,
        &input.migration_audit,
        &input.legacy_workflows,
    );
    let plan = &input.migration_plan.workflow_migration_plan;
    let manifest = &input
        .candidate_manifest
        .workflow_governance_release_manifest;
    let catalog_identity_valid = release_evaluation.status
        == WorkflowReleaseEvaluationStatus::StructurallyValid
        && release_evaluation.catalog_digest == plan.expected_catalog_digest
        && manifest.legacy_catalog_digest == plan.expected_catalog_digest
        && entries.len() == input.legacy_workflows.len()
        && entries.len() == plan.expected_catalog_count;
    let predecessor_catalog_unchanged = predecessor_manifest(input).is_some_and(|predecessor| {
        let previous = &predecessor.workflow_governance_release_manifest;
        let previous_identities = previous
            .workflow_entries
            .iter()
            .map(|entry| (&entry.workflow_id, &entry.legacy_workflow_digest))
            .collect::<Vec<_>>();
        let candidate_identities = entries
            .iter()
            .map(|entry| (&entry.workflow_id, &entry.legacy_workflow_digest))
            .collect::<Vec<_>>();
        previous.legacy_catalog_digest == manifest.legacy_catalog_digest
            && previous_identities == candidate_identities
    });
    if !catalog_identity_valid
        || !predecessor_catalog_unchanged
        || migration
            != input
                .candidate_runtime_bundle
                .workflow_governance_bundle
                .policies
                .len()
        || quarantined.len()
            != input
                .review_subject
                .workflow_behavioral_review_subject
                .quarantines
                .len()
        || quarantined != reviewed_quarantines
        || !quarantine_details_match
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::CatalogDispositionMismatch,
            "candidate_manifest.workflow_entries",
            "release evaluation must prove an unchanged exhaustive catalog identity, migrated policy parity, and the exact derived quarantine set",
        );
    }
    quarantined.len()
}

/// Resolve the manifest of the registry tail from the exact bytes already
/// bound by the predecessor registry. Failure is deliberately fail-closed.
fn predecessor_manifest(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
) -> Option<WorkflowGovernanceReleaseManifestDocument> {
    let tail = input
        .predecessor_registry
        .workflow_governance_release_registry
        .releases
        .last()?;
    let WorkflowReleaseRegistrySource::EmbeddedManifest { embedded_ref, .. } = &tail.source else {
        return None;
    };
    let bytes = input.source_bytes.get(embedded_ref)?;
    yaml_serde::from_slice(bytes).ok()
}

fn validate_registry_evolution(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
) {
    let registry = &input.proposed_registry.workflow_governance_release_registry;
    let artifacts = registry
        .releases
        .iter()
        .flat_map(|entry| {
            let mut paths = vec![entry.runtime_bundle.embedded_ref.clone()];
            if let WorkflowReleaseRegistrySource::EmbeddedManifest { embedded_ref, .. } =
                &entry.source
            {
                paths.push(embedded_ref.clone());
            }
            paths
        })
        .fold(Vec::new(), |mut unique, path| {
            if !unique.iter().any(|seen: &RepoPath| seen == &path) {
                unique.push(path);
            }
            unique
        })
        .into_iter()
        .filter_map(|embedded_ref| {
            input.source_bytes.get(&embedded_ref).map(|bytes| {
                WorkflowReleaseRegistryEvolutionArtifact {
                    embedded_ref,
                    bytes: bytes.clone(),
                }
            })
        })
        .collect::<Vec<_>>();
    let evolution = evaluate_workflow_release_registry_evolution(
        &input.predecessor_registry,
        &input.proposed_registry,
        &input.registry_bundles,
        &artifacts,
    );
    let expected_previous_count = input
        .predecessor_registry
        .workflow_governance_release_registry
        .releases
        .len();
    let exact_single_append = has_exact_single_append_prefix(
        &input
            .predecessor_registry
            .workflow_governance_release_registry
            .releases,
        &registry.releases,
    );
    if evolution.status != WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
        || expected_previous_count == 0
        || !exact_single_append
        || evolution.previous_release_count != expected_previous_count
        || evolution.current_release_count != expected_previous_count + 1
        || evolution.appended_release_count != 1
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::RegistryEvolutionInvalid,
            "proposed_registry",
            "registry must append exactly one release to the complete predecessor history",
        );
    }
    let Some(appended) = registry.releases.last() else {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::RegistryShapeMismatch,
            "proposed_registry.releases",
            "proposed registry requires one appended release",
        );
        return;
    };
    let promotion = &input.review_index.workflow_release_review_index.promotion;
    if appended.release != promotion.candidate_release
        || appended.runtime_bundle.identity != promotion.promoted_runtime_bundle
        || appended.predecessor.as_ref() != Some(&promotion.predecessor)
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::PredecessorMismatch,
            format!("proposed_registry.releases[{expected_previous_count}]"),
            "appended registry entry must equal the reviewed adjacent promotion",
        );
    }
    if appended.receipt_carryover != WorkflowReceiptCarryover::InvalidateAll {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::ReceiptCarryoverInvalid,
            format!("proposed_registry.releases[{expected_previous_count}].receipt_carryover"),
            "a changed policy set must invalidate all predecessor receipts",
        );
    }
    let predecessor_digest = workflow_release_registry_digest(&input.predecessor_registry).ok();
    if predecessor_digest.as_ref()
        != Some(
            &input
                .review_index
                .workflow_release_review_index
                .predecessor_registry
                .canonical_digest,
        )
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::FrozenHistoryIncompatible,
            "review_index.predecessor_registry",
            "frozen predecessor registry canonical identity changed",
        );
    }
    let history_binding = &input
        .review_index
        .workflow_release_review_index
        .frozen_history;
    let baseline_history = &input
        .review_subject
        .workflow_behavioral_review_subject
        .baseline_history;
    if history_binding.embedded_ref != baseline_history.embedded_ref
        || history_binding.raw_digest != baseline_history.expected_digest
    {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::FrozenHistoryIncompatible,
            "review_index.frozen_history",
            "review index must bind the exact frozen history consumed by behavioral resume evaluation",
        );
    }
}

fn verify_typed_set<T: Serialize + DeserializeOwned + PartialEq>(
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
    path: &str,
    bindings: &[WorkflowReleaseReviewArtifactBindingV2],
    documents: &[T],
    sources: &HashMap<RepoPath, Vec<u8>>,
) {
    if bindings.len() != documents.len() {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::ReviewSetMismatch,
            path,
            "artifact binding and typed document counts differ",
        );
    }
    for (index, (binding, document)) in bindings.iter().zip(documents).enumerate() {
        verify_typed_binding(
            issues,
            &format!("{path}[{index}]"),
            binding,
            document,
            sources,
        );
    }
}

fn verify_typed_binding<T: Serialize + DeserializeOwned + PartialEq>(
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
    path: &str,
    binding: &WorkflowReleaseReviewArtifactBindingV2,
    document: &T,
    sources: &HashMap<RepoPath, Vec<u8>>,
) {
    verify_raw_binding(issues, path, binding, sources);
    let raw_matches_typed = sources
        .get(&binding.embedded_ref)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| yaml_serde::from_str::<T>(text).ok())
        .is_some_and(|parsed| parsed == *document);
    if !raw_matches_typed {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::CanonicalDigestMismatch,
            format!("{path}.canonical_digest"),
            "parsed source bytes do not equal the supplied typed document",
        );
    }
    if canonical_digest(document).as_deref() != Ok(binding.canonical_digest.as_str()) {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::CanonicalDigestMismatch,
            format!("{path}.canonical_digest"),
            "canonical typed digest does not match the review binding",
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum SourceCanonicalDomain {
    Utf8Text,
    JsonLines,
}

fn verify_source_binding(
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
    path: &str,
    binding: &WorkflowReleaseReviewArtifactBindingV2,
    sources: &HashMap<RepoPath, Vec<u8>>,
    domain: SourceCanonicalDomain,
) {
    verify_raw_binding(issues, path, binding, sources);
    let Some(bytes) = sources.get(&binding.embedded_ref) else {
        return;
    };
    let digest = match domain {
        SourceCanonicalDomain::Utf8Text => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| error.to_string())
            .and_then(|text| canonical_digest(&text)),
        SourceCanonicalDomain::JsonLines => canonical_json_lines_digest(bytes),
    };
    if digest.as_deref() != Ok(binding.canonical_digest.as_str()) {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::CanonicalDigestMismatch,
            format!("{path}.canonical_digest"),
            "canonical source digest does not match the review binding",
        );
    }
}

fn verify_raw_binding(
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
    path: &str,
    binding: &WorkflowReleaseReviewArtifactBindingV2,
    sources: &HashMap<RepoPath, Vec<u8>>,
) {
    let Some(bytes) = sources.get(&binding.embedded_ref) else {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::MissingArtifactBytes,
            format!("{path}.embedded_ref"),
            "exact source bytes are missing",
        );
        return;
    };
    if sha256(bytes) != binding.raw_digest {
        push_issue(
            issues,
            WorkflowReleaseAdmissionV2IssueCode::RawDigestMismatch,
            format!("{path}.raw_digest"),
            "raw SHA-256 does not match exact source bytes",
        );
    }
}

fn artifact_reference(
    binding: &WorkflowReleaseReviewArtifactBindingV2,
) -> forge_core_contracts::WorkflowBehavioralArtifactReference {
    forge_core_contracts::WorkflowBehavioralArtifactReference {
        id: binding.artifact_id.clone(),
        embedded_ref: binding.embedded_ref.clone(),
        expected_digest: binding.raw_digest.clone(),
    }
}

fn canonical_json_lines_digest(bytes: &[u8]) -> Result<String, String> {
    let mut values = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        let text = std::str::from_utf8(line).map_err(|error| error.to_string())?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        values.push(
            yaml_serde::from_str::<yaml_serde::Value>(trimmed)
                .map_err(|error| error.to_string())?,
        );
    }
    canonical_digest(&values)
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(sha256(&bytes))
}

fn exact_nonempty_delta<'a, T: PartialEq>(
    predecessor: &[T],
    candidate: &'a [T],
) -> Option<&'a [T]> {
    if candidate.len() <= predecessor.len()
        || candidate.get(..predecessor.len()) != Some(predecessor)
    {
        return None;
    }
    candidate.get(predecessor.len()..)
}

fn has_exact_single_append_prefix<T: PartialEq>(predecessor: &[T], proposed: &[T]) -> bool {
    proposed.len() == predecessor.len() + 1
        && proposed.get(..predecessor.len()) == Some(predecessor)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn push_issue(
    issues: &mut Vec<WorkflowReleaseAdmissionV2Issue>,
    code: WorkflowReleaseAdmissionV2IssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowReleaseAdmissionV2Issue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::{exact_nonempty_delta, has_exact_single_append_prefix};
    use std::collections::BTreeSet;

    #[test]
    fn tampered_predecessor_prefix_is_not_a_valid_delta() {
        assert_eq!(
            exact_nonempty_delta(&["a", "b"], &["a", "tampered", "c"]),
            None
        );
    }

    #[test]
    fn unauthorized_registry_tail_is_not_one_reviewed_append() {
        assert!(!has_exact_single_append_prefix(
            &["r1", "r2"],
            &["r1", "r2", "r3", "r4"]
        ));
        assert!(has_exact_single_append_prefix(
            &["r1", "r2"],
            &["r1", "r2", "r3"]
        ));
    }

    #[test]
    fn delta_and_review_sets_must_match_exactly() {
        let delta = ["investigation", "atdd-plan"]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing = ["investigation"].into_iter().collect::<BTreeSet<_>>();
        let substituted = ["investigation", "eval-design"]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_ne!(delta, missing);
        assert_ne!(delta, substituted);
    }
}
