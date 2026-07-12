//! Pure, deterministic P5d.3 workflow-behavior shadow evaluation.
//!
//! This module deliberately derives only non-authoritative review evidence.
//! Authored expected outcomes are compared with fresh governance simulations;
//! authored aggregate reports, pass labels, and confidence are never inputs.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference, WorkflowBehavioralCorpusClass,
    WorkflowBehavioralCorpusSetDocument, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralDisposition, WorkflowBehavioralEvidenceAuthority,
    WorkflowBehavioralEvidenceBindings, WorkflowBehavioralExecutionResult,
    WorkflowBehavioralGovernanceInput, WorkflowBehavioralOutcomeComparison,
    WorkflowBehavioralReviewSubject, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralScenario, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralScenarioExecution, WorkflowBehavioralScenarioKind,
    WorkflowBehavioralScenarioKindCount, WorkflowBehavioralScenarioResult,
    WorkflowBehavioralShadowReport, WorkflowBehavioralShadowReportDocument,
    WorkflowBehavioralVerdict, WorkflowBehavioralWorkflowReport, WorkflowCompletionAssertion,
    WorkflowDocument, WorkflowEvidenceFreshness, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvent, WorkflowGovernancePolicy, WorkflowGovernancePolicyOverlayDocument,
    WorkflowGovernanceReceiptDocument, WorkflowGovernedClaim, WorkflowGovernedClaimStatus,
    WorkflowGovernedCompletion, WorkflowGovernedEligibility, WorkflowGovernedIssue,
    WorkflowGovernedIssueCode, WorkflowGovernedNextAction, WorkflowGovernedObligation,
    WorkflowGovernedOutcome, WorkflowGovernedOutcomeDimension, WorkflowGovernedProgression,
    WorkflowGovernedStatus, WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS,
    WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::workflow_governance::{
    simulate_workflow_governance, validate_workflow_governance_bundle, WorkflowClaimResultStatus,
    WorkflowCompletionVerdict, WorkflowEligibilityVerdict, WorkflowGovernanceIssue,
    WorkflowGovernanceIssueCode, WorkflowGovernanceRejection, WorkflowGovernanceSimulation,
    WorkflowGovernanceStatus, WorkflowProgressionVerdict,
};
use crate::workflow_release::{
    workflow_policy_set_digest, workflow_release_legacy_digest, workflow_release_policy_digest,
    workflow_runtime_bundle_digest,
};
use crate::LoadedWorkflowDocument;

/// Content-addressed corpus input. The aggregate corpus-set document binds
/// these exact artifact references; callers cannot substitute an equivalent
/// looking document at another digest.
#[derive(Debug, Clone)]
pub struct WorkflowBehavioralCorpusInput {
    pub artifact: WorkflowBehavioralArtifactReference,
    pub document: WorkflowBehavioralScenarioCorpusDocument,
}

/// Parsed bundle paired with the exact raw source artifact it came from. The
/// enclosing map is keyed by the independently recomputed canonical digest.
#[derive(Debug, Clone)]
pub struct WorkflowBehavioralBundleInput {
    pub artifact: WorkflowBehavioralArtifactReference,
    pub document: WorkflowGovernanceBundleDocument,
}

/// Immutable report identity and the exact aggregate corpus-set identity.
#[derive(Debug, Clone)]
pub struct WorkflowBehavioralReportIdentity {
    pub report_id: StableId,
    pub report_version: String,
    pub corpus_set: WorkflowBehavioralArtifactReference,
    pub coverage_policy: WorkflowBehavioralArtifactReference,
}

/// Recompute a complete non-authoritative behavioral shadow report.
///
/// `bundles_by_digest` is keyed by the canonical JCS SHA-256 of each bundle.
/// `source_bytes` is keyed by repository path and contains exact source-byte
/// digests. Missing or drifted content fails closed as `invalid_bindings`.
#[must_use]
// The source-byte map is a concrete repository audit input shared by CLI and
// generators; custom hashers add no semantic value at this public boundary.
#[allow(clippy::implicit_hasher)]
pub fn evaluate_workflow_behavior(
    identity: &WorkflowBehavioralReportIdentity,
    coverage_policy: &WorkflowBehavioralCoveragePolicyDocument,
    corpus_set: &WorkflowBehavioralCorpusSetDocument,
    review_subject: &WorkflowBehavioralReviewSubjectDocument,
    corpora: &[WorkflowBehavioralCorpusInput],
    bundles_by_digest: &BTreeMap<String, WorkflowBehavioralBundleInput>,
    source_bytes: &HashMap<RepoPath, Vec<u8>>,
) -> WorkflowBehavioralShadowReportDocument {
    let mut invalid_bindings = false;
    invalid_bindings |= identity.report_id.0.trim().is_empty();
    invalid_bindings |= identity.report_version.trim().is_empty();
    invalid_bindings |= !coverage_policy.validate().is_empty();
    invalid_bindings |=
        !typed_source_matches(&identity.coverage_policy, coverage_policy, source_bytes);
    invalid_bindings |=
        coverage_policy.workflow_behavioral_coverage_policy.id != identity.coverage_policy.id;
    invalid_bindings |= !typed_source_matches(&identity.corpus_set, corpus_set, source_bytes);
    invalid_bindings |= !corpus_set.validate().is_empty();
    invalid_bindings |= corpus_set.workflow_behavioral_corpus_set.id != identity.corpus_set.id;
    invalid_bindings |= corpora.is_empty();
    invalid_bindings |= !review_subject.validate().is_empty();

    let authored_corpora = corpora
        .iter()
        .map(|corpus| corpus.artifact.clone())
        .collect::<Vec<_>>();
    invalid_bindings |= corpus_set.workflow_behavioral_corpus_set.corpora != authored_corpora;

    let mut grouped: BTreeMap<
        String,
        (
            WorkflowBehavioralEvidenceBindings,
            Vec<WorkflowBehavioralScenario>,
        ),
    > = BTreeMap::new();
    let mut scenario_ids = BTreeSet::new();
    for corpus in corpora {
        invalid_bindings |= !corpus.document.validate().is_empty();
        invalid_bindings |= !typed_source_matches(&corpus.artifact, &corpus.document, source_bytes);
        let document = &corpus.document.workflow_behavioral_scenario_corpus;
        invalid_bindings |= document.coverage_policy != identity.coverage_policy;
        for workflow in &document.workflow_evidence {
            let key = workflow.bindings.workflow_id.0.clone();
            if let Some((existing, scenarios)) = grouped.get_mut(&key) {
                invalid_bindings |= existing != &workflow.bindings;
                scenarios.extend(workflow.scenarios.clone());
            } else {
                grouped.insert(key, (workflow.bindings.clone(), workflow.scenarios.clone()));
            }
            for scenario in &workflow.scenarios {
                invalid_bindings |= !scenario_ids.insert(scenario.scenario_id.0.clone());
            }
        }
    }
    let grouped_ids = grouped.keys().cloned().collect::<BTreeSet<_>>();
    let candidate_ids = review_subject
        .workflow_behavioral_review_subject
        .candidate_workflows
        .iter()
        .map(|candidate| candidate.workflow_id.0.clone())
        .collect::<BTreeSet<_>>();
    invalid_bindings |= grouped_ids != candidate_ids;

    let mut workflow_reports = Vec::new();
    let mut any_mismatch = false;
    let mut any_insufficient = grouped.is_empty();
    for (_, (bindings, mut scenarios)) in grouped {
        scenarios.sort_by(|left, right| left.scenario_id.0.cmp(&right.scenario_id.0));
        let (report, workflow_invalid, workflow_insufficient) = evaluate_workflow(
            &bindings,
            &scenarios,
            identity,
            coverage_policy,
            review_subject,
            bundles_by_digest,
            source_bytes,
        );
        invalid_bindings |= workflow_invalid;
        any_mismatch |= report.mismatch_count != 0;
        any_insufficient |= workflow_insufficient || report.evaluation_error_count != 0;
        workflow_reports.push(report);
    }

    let verdict = if invalid_bindings {
        WorkflowBehavioralVerdict::InvalidBindings
    } else if any_mismatch {
        WorkflowBehavioralVerdict::MismatchDetected
    } else if any_insufficient {
        WorkflowBehavioralVerdict::InsufficientEvidence
    } else {
        WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
    };
    let disposition = if verdict == WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate {
        WorkflowBehavioralDisposition::ReviewCandidate
    } else {
        WorkflowBehavioralDisposition::QuarantineRequired
    };

    WorkflowBehavioralShadowReportDocument {
        schema_version: WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_shadow_report: WorkflowBehavioralShadowReport {
            id: identity.report_id.clone(),
            report_version: identity.report_version.clone(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            corpus: identity.corpus_set.clone(),
            coverage_policy: identity.coverage_policy.clone(),
            workflow_reports,
            verdict,
            disposition,
        },
    }
}

fn evaluate_workflow(
    bindings: &WorkflowBehavioralEvidenceBindings,
    scenarios: &[WorkflowBehavioralScenario],
    identity: &WorkflowBehavioralReportIdentity,
    coverage_policy: &WorkflowBehavioralCoveragePolicyDocument,
    review_subject: &WorkflowBehavioralReviewSubjectDocument,
    bundles: &BTreeMap<String, WorkflowBehavioralBundleInput>,
    source_bytes: &HashMap<RepoPath, Vec<u8>>,
) -> (WorkflowBehavioralWorkflowReport, bool, bool) {
    let mut invalid = !bindings_valid(
        bindings,
        identity,
        coverage_policy,
        review_subject,
        bundles,
        source_bytes,
    );
    let mut mismatch_count = 0_u16;
    let mut evaluation_error_count = 0_u16;
    let mut kind_counts = BTreeMap::<WorkflowBehavioralScenarioKind, u16>::new();
    let mut representative = 0_u16;
    let mut adversarial = 0_u16;
    let mut results = Vec::new();
    let mut resume_seen = 0_u16;
    let mut resume_all_passed = true;
    let mut ablation_seen = 0_u16;
    let mut ablation_all_passed = true;
    let mut semantic_evidence_ok = true;

    for scenario in scenarios {
        *kind_counts.entry(scenario.scenario_kind).or_default() = kind_counts
            .get(&scenario.scenario_kind)
            .copied()
            .unwrap_or_default()
            .saturating_add(1);
        match scenario.corpus_class {
            WorkflowBehavioralCorpusClass::Representative => {
                representative = representative.saturating_add(1);
            }
            WorkflowBehavioralCorpusClass::Adversarial => {
                adversarial = adversarial.saturating_add(1);
            }
        }
        invalid |= workflow_behavior_execution_input_digest(&scenario.execution).as_deref()
            != Some(scenario.execution_input_digest.as_str());

        let execution = match &scenario.execution {
            WorkflowBehavioralScenarioExecution::Single { input, expected } => {
                let (comparison, errors, input_invalid) =
                    compare(input, expected, bundles, source_bytes);
                invalid |= input_invalid || !input_bound_to_candidate(input, bindings);
                evaluation_error_count = evaluation_error_count.saturating_add(errors);
                if !comparison.matches() {
                    mismatch_count = mismatch_count.saturating_add(1);
                }
                WorkflowBehavioralExecutionResult::Single {
                    comparison: Box::new(comparison),
                }
            }
            WorkflowBehavioralScenarioExecution::Resume {
                continuation,
                checkpoint_source,
                checkpoint_digest,
                checkpoint_input,
                checkpoint_expected,
                resumed_input,
                resumed_expected,
                equivalence_dimensions,
            } => {
                resume_seen = resume_seen.saturating_add(1);
                let checkpoint_source_valid = typed_source_matches(
                    checkpoint_source,
                    checkpoint_input.as_ref(),
                    source_bytes,
                );
                let history_valid = baseline_history_matches(
                    &review_subject.workflow_behavioral_review_subject,
                    continuation,
                    source_bytes,
                );
                let (checkpoint, checkpoint_errors, checkpoint_invalid) =
                    compare(checkpoint_input, checkpoint_expected, bundles, source_bytes);
                let (resumed, resumed_errors, resumed_invalid) =
                    compare(resumed_input, resumed_expected, bundles, source_bytes);
                invalid |= !checkpoint_source_valid
                    || !history_valid
                    || checkpoint_invalid
                    || resumed_invalid
                    || !input_bound_to_candidate(checkpoint_input, bindings)
                    || !input_bound_to_candidate(resumed_input, bindings)
                    || canonical_digest(checkpoint_input).as_deref()
                        != Ok(checkpoint_digest.as_str())
                    || checkpoint_input.evaluation != resumed_input.evaluation;
                evaluation_error_count = evaluation_error_count
                    .saturating_add(checkpoint_errors)
                    .saturating_add(resumed_errors);
                let dimensions = normalized_dimensions(equivalence_dimensions);
                invalid |= dimensions != WorkflowGovernedOutcomeDimension::all().to_vec();
                let equivalent =
                    outcomes_equal_on(&checkpoint.actual, &resumed.actual, &dimensions);
                resume_all_passed &= equivalent && checkpoint.matches() && resumed.matches();
                if !checkpoint.matches() || !resumed.matches() {
                    mismatch_count = mismatch_count.saturating_add(1);
                }
                WorkflowBehavioralExecutionResult::Resume {
                    checkpoint: Box::new(checkpoint),
                    resumed: Box::new(resumed),
                    equivalent,
                }
            }
            WorkflowBehavioralScenarioExecution::Ablation {
                control_input,
                control_expected,
                ablated_input,
                ablated_expected,
                removed_semantic_refs,
                required_difference_dimensions,
            } => {
                ablation_seen = ablation_seen.saturating_add(1);
                let (control, control_errors, control_invalid) =
                    compare(control_input, control_expected, bundles, source_bytes);
                let (ablated, ablated_errors, ablated_invalid) =
                    compare(ablated_input, ablated_expected, bundles, source_bytes);
                let ablation_binding_valid = valid_ablation(
                    control_input,
                    ablated_input,
                    &bindings.policy_ref,
                    removed_semantic_refs,
                    bundles,
                );
                invalid |= control_invalid
                    || ablated_invalid
                    || !input_bound_to_candidate(control_input, bindings)
                    || control_input.evaluation != ablated_input.evaluation
                    || !ablation_binding_valid;
                evaluation_error_count = evaluation_error_count
                    .saturating_add(control_errors)
                    .saturating_add(ablated_errors);
                let differing_dimensions = differing_dimensions(&control.actual, &ablated.actual);
                let required = normalized_dimensions(required_difference_dimensions);
                let semantic_delta = !differing_dimensions.is_empty()
                    && !required.is_empty()
                    && differing_dimensions == required
                    && required.contains(&WorkflowGovernedOutcomeDimension::Completion)
                    && control.actual.completion == WorkflowGovernedCompletion::Incomplete
                    && ablated.actual.completion == WorkflowGovernedCompletion::Complete
                    && ablated.actual.status == WorkflowGovernedStatus::Complete
                    && ablated.actual.progression == WorkflowGovernedProgression::Allowed;
                ablation_all_passed &= semantic_delta
                    && control.matches()
                    && ablated.matches()
                    && ablation_binding_valid;
                if !control.matches() || !ablated.matches() {
                    mismatch_count = mismatch_count.saturating_add(1);
                }
                WorkflowBehavioralExecutionResult::Ablation {
                    control: Box::new(control),
                    ablated: Box::new(ablated),
                    semantic_delta,
                    differing_dimensions,
                }
            }
        };
        semantic_evidence_ok &= scenario_semantics_satisfied(scenario, &execution);
        results.push(WorkflowBehavioralScenarioResult {
            scenario_id: scenario.scenario_id.clone(),
            scenario_kind: scenario.scenario_kind,
            corpus_class: scenario.corpus_class,
            execution,
        });
    }

    let policy = &coverage_policy.workflow_behavioral_coverage_policy;
    let kinds_met = WorkflowBehavioralScenarioKind::all()
        .into_iter()
        .filter(|kind| {
            kind_counts.get(kind).copied().unwrap_or_default() >= policy.minimum_scenarios_per_kind
        })
        .count();
    let coverage_basis_points = u16::try_from(
        kinds_met * usize::from(WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS) / 7,
    )
    .unwrap_or_default();
    let scenario_kind_counts = WorkflowBehavioralScenarioKind::all()
        .into_iter()
        .map(|scenario_kind| WorkflowBehavioralScenarioKindCount {
            scenario_kind,
            count: kind_counts.get(&scenario_kind).copied().unwrap_or_default(),
        })
        .collect();
    let total_scenarios = u16::try_from(scenarios.len()).unwrap_or(u16::MAX);
    let insufficient = total_scenarios < policy.minimum_scenarios_per_workflow
        || coverage_basis_points < policy.required_coverage_basis_points
        || representative == 0
        || adversarial == 0
        || resume_seen == 0
        || !resume_all_passed
        || ablation_seen == 0
        || !ablation_all_passed
        || !semantic_evidence_ok;

    (
        WorkflowBehavioralWorkflowReport {
            bindings: bindings.clone(),
            total_scenarios,
            scenario_kind_counts,
            representative_scenarios: representative,
            adversarial_scenarios: adversarial,
            coverage_basis_points,
            mismatch_count,
            evaluation_error_count,
            scenario_results: results,
        },
        invalid,
        insufficient,
    )
}

fn bindings_valid(
    bindings: &WorkflowBehavioralEvidenceBindings,
    identity: &WorkflowBehavioralReportIdentity,
    coverage_policy: &WorkflowBehavioralCoveragePolicyDocument,
    review_subject: &WorkflowBehavioralReviewSubjectDocument,
    bundles: &BTreeMap<String, WorkflowBehavioralBundleInput>,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool {
    if bindings.coverage_policy_id != coverage_policy.workflow_behavioral_coverage_policy.id
        || canonical_digest(coverage_policy).as_deref()
            != Ok(bindings.coverage_policy_digest.as_str())
        || bindings.coverage_policy_source_digest != identity.coverage_policy.expected_digest
        || !typed_source_matches(&bindings.review_subject, review_subject, sources)
        || canonical_digest(review_subject).as_deref()
            != Ok(bindings.review_subject_digest.as_str())
        || review_subject.workflow_behavioral_review_subject.id != bindings.review_subject.id
        || bindings.raw_sources.is_empty()
        || bindings.raw_sources.iter().any(|source| {
            !raw_source_matches(&source.embedded_ref, &source.expected_digest, sources)
        })
    {
        return false;
    }
    let subject = &review_subject.workflow_behavioral_review_subject;
    let Some(candidate) = subject
        .candidate_workflows
        .iter()
        .find(|candidate| candidate.workflow_id == bindings.workflow_id)
    else {
        return false;
    };
    if candidate.legacy_workflow_digest != bindings.legacy_workflow_digest
        || candidate.policy_ref != bindings.policy_ref
        || candidate.policy_digest != bindings.policy_digest
        || subject.runtime_bundle.bundle_id != bindings.candidate_bundle_id
        || subject.runtime_bundle.bundle_digest != bindings.candidate_bundle_digest
        || subject.runtime_bundle.policy_set_digest != bindings.candidate_policy_set_digest
        || subject.proposed_batch.batch_id != bindings.migration_batch_id
        || subject.proposed_batch.batch_version != bindings.migration_batch_version
        || subject.proposed_release.release_id != bindings.governance_release_id
        || subject.proposed_release.release_version != bindings.governance_release_version
        || subject.proposed_release.previous_release_digest != bindings.predecessor_release_digest
        || subject.evaluator != bindings.evaluator
        || !artifact_source_matches(&subject.overlay, sources)
    {
        return false;
    }
    let Some(bundle_input) = bundles.get(&bindings.candidate_bundle_digest) else {
        return false;
    };
    let bundle = &bundle_input.document;
    if workflow_runtime_bundle_digest(bundle).as_deref()
        != Ok(bindings.candidate_bundle_digest.as_str())
        || bundle_input.artifact.id != bindings.candidate_bundle_id
        || bundle_input.artifact.expected_digest != bindings.candidate_bundle_source_digest
        || !typed_source_matches(&bundle_input.artifact, bundle, sources)
        || bundle.workflow_governance_bundle.id != bindings.candidate_bundle_id
        || workflow_policy_set_digest(&bundle.workflow_governance_bundle.policies).as_deref()
            != Ok(bindings.candidate_policy_set_digest.as_str())
    {
        return false;
    }
    let Some(policy) = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == bindings.policy_ref)
    else {
        return false;
    };
    policy.compatibility_workflow_id == bindings.workflow_id
        && workflow_release_policy_digest(policy).as_deref() == Ok(bindings.policy_digest.as_str())
        && required_raw_bindings_match(bindings)
        && source_semantics_match(
            bindings,
            &subject.overlay,
            &bundle_input.artifact,
            &identity.coverage_policy,
            policy,
            sources,
        )
}

fn baseline_history_matches(
    subject: &WorkflowBehavioralReviewSubject,
    continuation: &forge_core_contracts::WorkflowBehavioralContinuationIdentity,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool {
    let Some(bytes) = sources.get(&subject.baseline_history.embedded_ref) else {
        return false;
    };
    let history_digest = sha256(bytes);
    if history_digest != subject.baseline_history.expected_digest
        || continuation.ledger_digest != history_digest
    {
        return false;
    }
    let mut documents = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(text) = std::str::from_utf8(line) else {
            return false;
        };
        let Ok(document) = yaml_serde::from_str::<WorkflowGovernanceReceiptDocument>(text) else {
            return false;
        };
        documents.push(document);
    }
    if documents.is_empty() {
        return false;
    }
    let mut previous_digest: Option<&str> = None;
    let mut project_id: Option<&StableId> = None;
    let mut imported_snapshot = None;
    let mut current_phase = None;
    for (index, document) in documents.iter().enumerate() {
        let record = &document.workflow_governance_receipt;
        if document.schema_version
            != forge_core_contracts::WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
            || record.sequence != u64::try_from(index + 1).unwrap_or(u64::MAX)
            || record.previous_record_digest.as_deref() != previous_digest
            || workflow_history_record_digest(record).as_deref()
                != Ok(record.record_digest.as_str())
            || project_id.is_some_and(|expected| expected != &record.project_id)
        {
            return false;
        }
        project_id = Some(&record.project_id);
        previous_digest = Some(&record.record_digest);
        match &record.event {
            WorkflowGovernanceEvent::ProjectImported(imported) => {
                if index != 0 || imported_snapshot.is_some() {
                    return false;
                }
                imported_snapshot = Some(imported.snapshot_digest.as_str());
                current_phase = Some(&imported.initial_phase);
            }
            WorkflowGovernanceEvent::PhaseAdvanced(advanced) => {
                current_phase = Some(&advanced.to_phase);
            }
            _ => {}
        }
    }
    let Some(snapshot_digest) = imported_snapshot else {
        return false;
    };
    let Some(last) = documents
        .last()
        .map(|document| &document.workflow_governance_receipt)
    else {
        return false;
    };
    let WorkflowGovernanceEvent::ReleaseUpgraded(upgrade) = &last.event else {
        return false;
    };
    upgrade.to_release == subject.baseline_release
        && upgrade.to_runtime_bundle == subject.baseline_runtime_bundle
        && upgrade.admission_proof.snapshot_digest == snapshot_digest
        && continuation.ledger_head_digest == last.record_digest
        && continuation.snapshot_digest == snapshot_digest
        && continuation.active_release_id == subject.baseline_release.release_id
        && continuation.active_release_digest == subject.baseline_release.release_digest
        && continuation.runtime_bundle_id == subject.baseline_runtime_bundle.bundle_id
        && continuation.runtime_bundle_digest == subject.baseline_runtime_bundle.bundle_digest
        && continuation.state_version == last.state_version
        && current_phase.is_some_and(|phase| phase == &continuation.current_phase)
        && continuation.observed_at_unix == last.recorded_at_unix
}

fn workflow_history_record_digest(
    record: &forge_core_contracts::WorkflowGovernanceLedgerRecord,
) -> Result<String, String> {
    let mut digest_input = record.clone();
    digest_input.record_digest.clear();
    canonical_digest(&digest_input)
}

fn source_semantics_match(
    bindings: &WorkflowBehavioralEvidenceBindings,
    overlay_artifact: &WorkflowBehavioralArtifactReference,
    bundle_artifact: &WorkflowBehavioralArtifactReference,
    coverage_artifact: &WorkflowBehavioralArtifactReference,
    candidate_policy: &WorkflowGovernancePolicy,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool {
    use forge_core_contracts::WorkflowBehavioralRawSourceKind;

    let Some(overlay) =
        parse_typed_source::<WorkflowGovernancePolicyOverlayDocument>(overlay_artifact, sources)
    else {
        return false;
    };
    if overlay.workflow_governance_policy_overlay.id != overlay_artifact.id
        || !overlay
            .workflow_governance_policy_overlay
            .policies
            .iter()
            .any(|policy| policy == candidate_policy)
    {
        return false;
    }
    let exact_raw_artifact = |kind, artifact: &WorkflowBehavioralArtifactReference| {
        bindings.raw_sources.iter().any(|source| {
            source.kind == kind
                && source.embedded_ref == artifact.embedded_ref
                && source.expected_digest == artifact.expected_digest
        })
    };
    if !exact_raw_artifact(
        WorkflowBehavioralRawSourceKind::GovernancePolicy,
        overlay_artifact,
    ) || !exact_raw_artifact(
        WorkflowBehavioralRawSourceKind::CandidateBundle,
        bundle_artifact,
    ) || !exact_raw_artifact(
        WorkflowBehavioralRawSourceKind::CoveragePolicy,
        coverage_artifact,
    ) {
        return false;
    }

    bindings.raw_sources.iter().any(|source| {
        if source.kind != WorkflowBehavioralRawSourceKind::LegacyWorkflow
            || !raw_source_matches(&source.embedded_ref, &source.expected_digest, sources)
        {
            return false;
        }
        let artifact = WorkflowBehavioralArtifactReference {
            id: bindings.workflow_id.clone(),
            embedded_ref: source.embedded_ref.clone(),
            expected_digest: source.expected_digest.clone(),
        };
        let Some(document) = parse_typed_source::<WorkflowDocument>(&artifact, sources) else {
            return false;
        };
        document.workflow.id == bindings.workflow_id
            && workflow_release_legacy_digest(&LoadedWorkflowDocument {
                workflow_ref: source.embedded_ref.clone(),
                document,
            })
            .as_deref()
                == Ok(bindings.legacy_workflow_digest.as_str())
    })
}

fn required_raw_bindings_match(bindings: &WorkflowBehavioralEvidenceBindings) -> bool {
    use forge_core_contracts::WorkflowBehavioralRawSourceKind;
    let mut kinds = [false; 5];
    let mut candidate_bundle = false;
    let mut coverage_policy = false;
    let mut evaluator = false;
    for source in &bindings.raw_sources {
        match source.kind {
            WorkflowBehavioralRawSourceKind::LegacyWorkflow => {
                kinds[0] = true;
            }
            WorkflowBehavioralRawSourceKind::GovernancePolicy => {
                kinds[1] = true;
            }
            WorkflowBehavioralRawSourceKind::CandidateBundle => {
                kinds[2] = true;
                candidate_bundle |=
                    source.expected_digest == bindings.candidate_bundle_source_digest;
            }
            WorkflowBehavioralRawSourceKind::CoveragePolicy => {
                kinds[3] = true;
                coverage_policy |= source.expected_digest == bindings.coverage_policy_source_digest;
            }
            WorkflowBehavioralRawSourceKind::Evaluator => {
                kinds[4] = true;
                evaluator |= source.expected_digest == bindings.evaluator.evaluator_source_digest;
            }
        }
    }
    kinds.into_iter().all(|present| present) && candidate_bundle && coverage_policy && evaluator
}

fn compare(
    input: &WorkflowBehavioralGovernanceInput,
    expected: &WorkflowGovernedOutcome,
    bundles: &BTreeMap<String, WorkflowBehavioralBundleInput>,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> (WorkflowBehavioralOutcomeComparison, u16, bool) {
    let Some((canonical_key, bundle_input)) = resolve_bundle(&input.bundle, bundles) else {
        return (
            WorkflowBehavioralOutcomeComparison {
                expected: normalize_outcome(expected.clone()),
                actual: invalid_outcome(Vec::new()),
                differing_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
            },
            1,
            true,
        );
    };
    let bundle = &bundle_input.document;
    let digest_valid = workflow_runtime_bundle_digest(bundle).as_deref() == Ok(canonical_key)
        && bundle.workflow_governance_bundle.id == input.bundle.id
        && typed_source_matches(&input.bundle, bundle, sources)
        && validate_workflow_governance_bundle(bundle).is_empty();
    let (actual, errors) = match simulate_workflow_governance(bundle, &input.evaluation) {
        Ok(simulation) => (project_simulation(simulation), 0),
        Err(rejection) => (project_rejection(rejection), 1),
    };
    let expected = normalize_outcome(expected.clone());
    let actual = normalize_outcome(actual);
    let differing_dimensions = differing_dimensions(&expected, &actual);
    (
        WorkflowBehavioralOutcomeComparison {
            expected,
            actual,
            differing_dimensions,
        },
        errors,
        !digest_valid,
    )
}

/// Derive the normalized, description-free governed projection for one exact
/// bundle and evaluation. Artifact generators can therefore author expected
/// fixtures without copying the evaluator's authority logic.
///
/// # Errors
/// Returns the governance engine's structural rejection unchanged.
pub fn derive_workflow_governed_outcome(
    bundle: &WorkflowGovernanceBundleDocument,
    evaluation: &forge_core_contracts::WorkflowGovernanceEvaluationDocument,
) -> Result<WorkflowGovernedOutcome, WorkflowGovernanceRejection> {
    simulate_workflow_governance(bundle, evaluation)
        .map(project_simulation)
        .map(normalize_outcome)
}

fn input_bound_to_candidate(
    input: &WorkflowBehavioralGovernanceInput,
    bindings: &WorkflowBehavioralEvidenceBindings,
) -> bool {
    input.bundle.id == bindings.candidate_bundle_id
        && input.bundle.expected_digest == bindings.candidate_bundle_source_digest
        && input.evaluation.workflow_governance_evaluation.bundle_id == bindings.candidate_bundle_id
        && input.evaluation.workflow_governance_evaluation.policy_id == bindings.policy_ref
}

fn valid_ablation(
    control: &WorkflowBehavioralGovernanceInput,
    ablated: &WorkflowBehavioralGovernanceInput,
    target_policy: &StableId,
    removed: &[StableId],
    bundles: &BTreeMap<String, WorkflowBehavioralBundleInput>,
) -> bool {
    let removed_set = removed.iter().collect::<BTreeSet<_>>();
    if removed.is_empty() || removed_set.len() != removed.len() || control.bundle == ablated.bundle
    {
        return false;
    }
    let Some((_, control_input)) = resolve_bundle(&control.bundle, bundles) else {
        return false;
    };
    let Some((ablated_key, ablated_input)) = resolve_bundle(&ablated.bundle, bundles) else {
        return false;
    };
    let control_bundle = &control_input.document;
    let ablated_bundle = &ablated_input.document;
    if workflow_runtime_bundle_digest(ablated_bundle).as_deref() != Ok(ablated_key)
        || ablated.bundle.id != ablated_bundle.workflow_governance_bundle.id
    {
        return false;
    }
    let mut expected = control_bundle.clone();
    let Some(policy) = expected
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id == *target_policy)
    else {
        return false;
    };
    remove_declared_semantics(policy, &removed_set) && expected == *ablated_bundle
}

fn scenario_semantics_satisfied(
    scenario: &WorkflowBehavioralScenario,
    result: &WorkflowBehavioralExecutionResult,
) -> bool {
    let WorkflowBehavioralScenarioExecution::Single { input, .. } = &scenario.execution else {
        return matches!(
            (scenario.scenario_kind, result),
            (
                WorkflowBehavioralScenarioKind::Resume,
                WorkflowBehavioralExecutionResult::Resume { .. }
            ) | (
                WorkflowBehavioralScenarioKind::Ablation,
                WorkflowBehavioralExecutionResult::Ablation { .. }
            )
        );
    };
    let WorkflowBehavioralExecutionResult::Single { comparison } = result else {
        return false;
    };
    match scenario.scenario_kind {
        WorkflowBehavioralScenarioKind::Positive => {
            comparison.actual.status == WorkflowGovernedStatus::Complete
                && comparison.actual.eligibility == WorkflowGovernedEligibility::Eligible
                && comparison.actual.progression == WorkflowGovernedProgression::Allowed
                && comparison.actual.completion == WorkflowGovernedCompletion::Complete
                && comparison.actual.issues.is_empty()
        }
        WorkflowBehavioralScenarioKind::Negative => {
            comparison.actual.completion == WorkflowGovernedCompletion::Incomplete
                && (comparison.actual.progression == WorkflowGovernedProgression::Blocked
                    || !comparison.actual.issues.is_empty()
                    || comparison.actual.obligations.iter().any(|obligation| {
                        obligation.status != forge_core_contracts::ObligationStatus::Satisfied
                    })
                    || comparison
                        .actual
                        .claims
                        .iter()
                        .any(|claim| claim.status != WorkflowGovernedClaimStatus::Verified))
        }
        WorkflowBehavioralScenarioKind::Ambiguity => {
            comparison.actual.progression == WorkflowGovernedProgression::Blocked
                && comparison.actual.completion == WorkflowGovernedCompletion::Incomplete
                && !comparison.actual.decision_refs.is_empty()
        }
        WorkflowBehavioralScenarioKind::FalseCompletion => {
            input
                .evaluation
                .workflow_governance_evaluation
                .completion_assertion
                == WorkflowCompletionAssertion::Asserted
                && comparison.actual.completion == WorkflowGovernedCompletion::Incomplete
                && comparison
                    .actual
                    .issues
                    .iter()
                    .any(|issue| issue.code == WorkflowGovernedIssueCode::InventedCompletionClaim)
        }
        WorkflowBehavioralScenarioKind::StaleEvidence => {
            input
                .evaluation
                .workflow_governance_evaluation
                .evidence
                .iter()
                .any(|evidence| evidence.freshness == WorkflowEvidenceFreshness::Stale)
                && comparison
                    .actual
                    .issues
                    .iter()
                    .any(|issue| issue.code == WorkflowGovernedIssueCode::StaleEvidence)
        }
        WorkflowBehavioralScenarioKind::Resume | WorkflowBehavioralScenarioKind::Ablation => false,
    }
}

fn resolve_bundle<'a>(
    artifact: &WorkflowBehavioralArtifactReference,
    bundles: &'a BTreeMap<String, WorkflowBehavioralBundleInput>,
) -> Option<(&'a str, &'a WorkflowBehavioralBundleInput)> {
    bundles
        .iter()
        .find(|(_, input)| input.artifact == *artifact)
        .map(|(digest, input)| (digest.as_str(), input))
}

fn remove_declared_semantics(
    policy: &mut WorkflowGovernancePolicy,
    removed: &BTreeSet<&StableId>,
) -> bool {
    let mut matched = BTreeSet::<String>::new();
    let removed_claims = policy
        .claims
        .iter()
        .filter(|claim| removed.contains(&claim.id))
        .map(|claim| claim.id.clone())
        .collect::<BTreeSet<_>>();
    policy.prerequisites.retain(|item| {
        let remove = removed.contains(&item.policy_ref);
        if remove {
            matched.insert(item.policy_ref.0.clone());
        }
        !remove
    });
    policy.obligations.retain_mut(|item| {
        if removed.contains(&item.id) {
            matched.insert(item.id.0.clone());
            return false;
        }
        let before = item.claim_refs.len();
        item.claim_refs
            .retain(|claim| !removed_claims.contains(claim));
        before == item.claim_refs.len() || !item.claim_refs.is_empty()
    });
    policy.claims.retain(|item| {
        let remove = removed.contains(&item.id);
        if remove {
            matched.insert(item.id.0.clone());
        }
        !remove
    });
    policy.evaluators.retain(|item| {
        let remove = removed.contains(&item.id);
        if remove {
            matched.insert(item.id.0.clone());
        }
        !remove
    });
    policy.capability_requirements.retain_mut(|item| {
        if removed.contains(&item.id) {
            matched.insert(item.id.0.clone());
            return false;
        }
        let before = item.affected_claim_refs.len();
        item.affected_claim_refs
            .retain(|claim| !removed_claims.contains(claim));
        before == item.affected_claim_refs.len() || !item.affected_claim_refs.is_empty()
    });
    policy.decision_rules.retain(|item| {
        let remove = removed.contains(&item.id)
            || item
                .claim_ref
                .as_ref()
                .is_some_and(|claim| removed_claims.contains(claim));
        if removed.contains(&item.id) {
            matched.insert(item.id.0.clone());
        }
        !remove
    });
    matched.len() == removed.len()
}

fn project_simulation(simulation: WorkflowGovernanceSimulation) -> WorkflowGovernedOutcome {
    WorkflowGovernedOutcome {
        status: match simulation.candidate_status {
            WorkflowGovernanceStatus::Ineligible => WorkflowGovernedStatus::Ineligible,
            WorkflowGovernanceStatus::Blocked => WorkflowGovernedStatus::Blocked,
            WorkflowGovernanceStatus::Active => WorkflowGovernedStatus::Active,
            WorkflowGovernanceStatus::Complete => WorkflowGovernedStatus::Complete,
        },
        eligibility: match simulation.candidate_eligibility {
            WorkflowEligibilityVerdict::Eligible => WorkflowGovernedEligibility::Eligible,
            WorkflowEligibilityVerdict::Ineligible => WorkflowGovernedEligibility::Ineligible,
        },
        progression: match simulation.candidate_progression {
            WorkflowProgressionVerdict::Allowed => WorkflowGovernedProgression::Allowed,
            WorkflowProgressionVerdict::Blocked => WorkflowGovernedProgression::Blocked,
        },
        completion: match simulation.candidate_completion {
            WorkflowCompletionVerdict::Complete => WorkflowGovernedCompletion::Complete,
            WorkflowCompletionVerdict::Incomplete => WorkflowGovernedCompletion::Incomplete,
        },
        obligations: simulation
            .candidate_obligation_results
            .into_iter()
            .map(|item| WorkflowGovernedObligation {
                obligation_id: StableId(item.obligation_id),
                status: item.status,
            })
            .collect(),
        claims: simulation
            .candidate_claim_results
            .into_iter()
            .map(|item| WorkflowGovernedClaim {
                claim_id: StableId(item.claim_id),
                status: match item.status {
                    WorkflowClaimResultStatus::Unknown => WorkflowGovernedClaimStatus::Unknown,
                    WorkflowClaimResultStatus::Supported => WorkflowGovernedClaimStatus::Supported,
                    WorkflowClaimResultStatus::Verified => WorkflowGovernedClaimStatus::Verified,
                    WorkflowClaimResultStatus::Waived => WorkflowGovernedClaimStatus::Waived,
                    WorkflowClaimResultStatus::Disproven => WorkflowGovernedClaimStatus::Disproven,
                    WorkflowClaimResultStatus::Contradictory => {
                        WorkflowGovernedClaimStatus::Contradictory
                    }
                },
            })
            .collect(),
        decision_refs: simulation
            .candidate_decision_requests
            .into_iter()
            .map(|item| item.id)
            .collect(),
        capability_refs: simulation
            .candidate_capability_gaps
            .into_iter()
            .map(|item| item.id)
            .collect(),
        issues: simulation.issues.into_iter().map(project_issue).collect(),
        next_actions: simulation
            .candidate_next_actions
            .into_iter()
            .map(|item| WorkflowGovernedNextAction {
                id: item.id,
                kind: item.kind,
                addresses_claim_refs: item.addresses_claim_refs,
                rank: item.rank,
            })
            .collect(),
    }
}

fn project_rejection(rejection: WorkflowGovernanceRejection) -> WorkflowGovernedOutcome {
    invalid_outcome(rejection.issues.into_iter().map(project_issue).collect())
}

fn invalid_outcome(issues: Vec<WorkflowGovernedIssue>) -> WorkflowGovernedOutcome {
    WorkflowGovernedOutcome {
        status: WorkflowGovernedStatus::Ineligible,
        eligibility: WorkflowGovernedEligibility::Ineligible,
        progression: WorkflowGovernedProgression::Blocked,
        completion: WorkflowGovernedCompletion::Incomplete,
        obligations: Vec::new(),
        claims: Vec::new(),
        decision_refs: Vec::new(),
        capability_refs: Vec::new(),
        issues,
        next_actions: Vec::new(),
    }
}

fn project_issue(issue: WorkflowGovernanceIssue) -> WorkflowGovernedIssue {
    WorkflowGovernedIssue {
        code: match issue.code {
            WorkflowGovernanceIssueCode::UnsupportedSchemaVersion => {
                WorkflowGovernedIssueCode::UnsupportedSchemaVersion
            }
            WorkflowGovernanceIssueCode::BlankRequiredField => {
                WorkflowGovernedIssueCode::BlankRequiredField
            }
            WorkflowGovernanceIssueCode::DuplicateIdentifier => {
                WorkflowGovernedIssueCode::DuplicateIdentifier
            }
            WorkflowGovernanceIssueCode::DuplicateReference => {
                WorkflowGovernedIssueCode::DuplicateReference
            }
            WorkflowGovernanceIssueCode::DanglingReference => {
                WorkflowGovernedIssueCode::DanglingReference
            }
            WorkflowGovernanceIssueCode::DependencyCycle => {
                WorkflowGovernedIssueCode::DependencyCycle
            }
            WorkflowGovernanceIssueCode::InvalidEvaluator => {
                WorkflowGovernedIssueCode::InvalidEvaluator
            }
            WorkflowGovernanceIssueCode::InvalidDecisionRule => {
                WorkflowGovernedIssueCode::InvalidDecisionRule
            }
            WorkflowGovernanceIssueCode::InvalidPolicy => WorkflowGovernedIssueCode::InvalidPolicy,
            WorkflowGovernanceIssueCode::BundleMismatch => {
                WorkflowGovernedIssueCode::BundleMismatch
            }
            WorkflowGovernanceIssueCode::UnknownPolicy => WorkflowGovernedIssueCode::UnknownPolicy,
            WorkflowGovernanceIssueCode::InvalidPhase => WorkflowGovernedIssueCode::InvalidPhase,
            WorkflowGovernanceIssueCode::EvidenceBindingMismatch => {
                WorkflowGovernedIssueCode::EvidenceBindingMismatch
            }
            WorkflowGovernanceIssueCode::UnsupportedEvidenceKind => {
                WorkflowGovernedIssueCode::UnsupportedEvidenceKind
            }
            WorkflowGovernanceIssueCode::InsufficientEvidenceStrength => {
                WorkflowGovernedIssueCode::InsufficientEvidenceStrength
            }
            WorkflowGovernanceIssueCode::StaleEvidence => WorkflowGovernedIssueCode::StaleEvidence,
            WorkflowGovernanceIssueCode::InconclusiveEvidence => {
                WorkflowGovernedIssueCode::InconclusiveEvidence
            }
            WorkflowGovernanceIssueCode::ContradictoryEvidence => {
                WorkflowGovernedIssueCode::ContradictoryEvidence
            }
            WorkflowGovernanceIssueCode::PhaseIneligible => {
                WorkflowGovernedIssueCode::PhaseIneligible
            }
            WorkflowGovernanceIssueCode::MissingPrerequisite => {
                WorkflowGovernedIssueCode::MissingPrerequisite
            }
            WorkflowGovernanceIssueCode::UnknownApplicability => {
                WorkflowGovernedIssueCode::UnknownApplicability
            }
            WorkflowGovernanceIssueCode::InvalidWaiver => WorkflowGovernedIssueCode::InvalidWaiver,
            WorkflowGovernanceIssueCode::ExpiredWaiver => WorkflowGovernedIssueCode::ExpiredWaiver,
            WorkflowGovernanceIssueCode::InsufficientPrincipalDiversity => {
                WorkflowGovernedIssueCode::InsufficientPrincipalDiversity
            }
            WorkflowGovernanceIssueCode::InventedCompletionClaim => {
                WorkflowGovernedIssueCode::InventedCompletionClaim
            }
            WorkflowGovernanceIssueCode::LegacyProjectionMismatch => {
                WorkflowGovernedIssueCode::LegacyProjectionMismatch
            }
        },
        path: issue.path,
    }
}

fn normalize_outcome(mut outcome: WorkflowGovernedOutcome) -> WorkflowGovernedOutcome {
    outcome
        .obligations
        .sort_by(|left, right| left.obligation_id.0.cmp(&right.obligation_id.0));
    outcome
        .claims
        .sort_by(|left, right| left.claim_id.0.cmp(&right.claim_id.0));
    outcome
        .decision_refs
        .sort_by(|left, right| left.0.cmp(&right.0));
    outcome
        .capability_refs
        .sort_by(|left, right| left.0.cmp(&right.0));
    outcome.issues.sort_by(|left, right| {
        (left.code, left.path.as_str()).cmp(&(right.code, right.path.as_str()))
    });
    for action in &mut outcome.next_actions {
        action
            .addresses_claim_refs
            .sort_by(|left, right| left.0.cmp(&right.0));
    }
    outcome.next_actions.sort_by(|left, right| {
        (left.rank, left.id.0.as_str()).cmp(&(right.rank, right.id.0.as_str()))
    });
    outcome
}

fn differing_dimensions(
    left: &WorkflowGovernedOutcome,
    right: &WorkflowGovernedOutcome,
) -> Vec<WorkflowGovernedOutcomeDimension> {
    let mut differing = Vec::new();
    for dimension in WorkflowGovernedOutcomeDimension::all() {
        let differs = match dimension {
            WorkflowGovernedOutcomeDimension::Status => left.status != right.status,
            WorkflowGovernedOutcomeDimension::Eligibility => left.eligibility != right.eligibility,
            WorkflowGovernedOutcomeDimension::Progression => left.progression != right.progression,
            WorkflowGovernedOutcomeDimension::Completion => left.completion != right.completion,
            WorkflowGovernedOutcomeDimension::Obligations => left.obligations != right.obligations,
            WorkflowGovernedOutcomeDimension::Claims => left.claims != right.claims,
            WorkflowGovernedOutcomeDimension::Decisions => {
                left.decision_refs != right.decision_refs
            }
            WorkflowGovernedOutcomeDimension::Capabilities => {
                left.capability_refs != right.capability_refs
            }
            WorkflowGovernedOutcomeDimension::Issues => left.issues != right.issues,
            WorkflowGovernedOutcomeDimension::NextActions => {
                left.next_actions != right.next_actions
            }
        };
        if differs {
            differing.push(dimension);
        }
    }
    differing
}

fn outcomes_equal_on(
    left: &WorkflowGovernedOutcome,
    right: &WorkflowGovernedOutcome,
    dimensions: &[WorkflowGovernedOutcomeDimension],
) -> bool {
    let differing = differing_dimensions(left, right);
    !dimensions
        .iter()
        .any(|dimension| differing.contains(dimension))
}

fn normalized_dimensions(
    dimensions: &[WorkflowGovernedOutcomeDimension],
) -> Vec<WorkflowGovernedOutcomeDimension> {
    let mut result = dimensions.to_vec();
    result.sort();
    result.dedup();
    result
}

fn artifact_source_matches(
    artifact: &WorkflowBehavioralArtifactReference,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool {
    raw_source_matches(&artifact.embedded_ref, &artifact.expected_digest, sources)
}

fn raw_source_matches(
    path: &RepoPath,
    expected_digest: &str,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool {
    sources
        .get(path)
        .is_some_and(|bytes| sha256(bytes) == expected_digest)
}

fn typed_source_matches<T>(
    artifact: &WorkflowBehavioralArtifactReference,
    document: &T,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> bool
where
    T: DeserializeOwned + PartialEq,
{
    parse_typed_source::<T>(artifact, sources).is_some_and(|parsed| parsed == *document)
}

fn parse_typed_source<T>(
    artifact: &WorkflowBehavioralArtifactReference,
    sources: &HashMap<RepoPath, Vec<u8>>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    let bytes = sources.get(&artifact.embedded_ref)?;
    if sha256(bytes) != artifact.expected_digest {
        return None;
    }
    yaml_serde::from_str(std::str::from_utf8(bytes).ok()?).ok()
}

/// Canonical digest of one execution input with every authored expected
/// outcome omitted.
#[must_use]
pub fn workflow_behavior_execution_input_digest(
    execution: &WorkflowBehavioralScenarioExecution,
) -> Option<String> {
    #[derive(Serialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Input<'a> {
        Single {
            input: &'a WorkflowBehavioralGovernanceInput,
        },
        Resume {
            continuation: &'a forge_core_contracts::WorkflowBehavioralContinuationIdentity,
            checkpoint_source: &'a WorkflowBehavioralArtifactReference,
            checkpoint_digest: &'a str,
            checkpoint_input: &'a WorkflowBehavioralGovernanceInput,
            resumed_input: &'a WorkflowBehavioralGovernanceInput,
            equivalence_dimensions: &'a [WorkflowGovernedOutcomeDimension],
        },
        Ablation {
            control_input: &'a WorkflowBehavioralGovernanceInput,
            ablated_input: &'a WorkflowBehavioralGovernanceInput,
            removed_semantic_refs: &'a [StableId],
            required_difference_dimensions: &'a [WorkflowGovernedOutcomeDimension],
        },
    }
    let input = match execution {
        WorkflowBehavioralScenarioExecution::Single { input, .. } => Input::Single { input },
        WorkflowBehavioralScenarioExecution::Resume {
            continuation,
            checkpoint_source,
            checkpoint_digest,
            checkpoint_input,
            resumed_input,
            equivalence_dimensions,
            ..
        } => Input::Resume {
            continuation,
            checkpoint_source,
            checkpoint_digest,
            checkpoint_input,
            resumed_input,
            equivalence_dimensions,
        },
        WorkflowBehavioralScenarioExecution::Ablation {
            control_input,
            ablated_input,
            removed_semantic_refs,
            required_difference_dimensions,
            ..
        } => Input::Ablation {
            control_input,
            ablated_input,
            removed_semantic_refs,
            required_difference_dimensions,
        },
    };
    canonical_digest(&input).ok()
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical);
    Ok(format!("sha256:{digest:x}"))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}
