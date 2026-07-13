//! Pure P6c learning-promotion and reviewed-registry decisions.
//!
//! This module deliberately has no authority. It can explain whether an
//! evidence graph is ready to be presented to the trusted boundary, but it
//! cannot verify signatures, advance an anchor, or make learned material
//! executable.

use std::collections::{BTreeMap, BTreeSet};

use forge_core_contracts::{
    DomainPackIndependentReviewDocument, DomainPackLearningComparisonMethod,
    DomainPackLearningComparisonVerdict, DomainPackLearningConflictDocument,
    DomainPackLearningConflictStatus, DomainPackLocalLearningCandidateDocument,
    DomainPackPromotionDossierDocument, DomainPackPromotionStage,
    DomainPackResolutionProjectionDocument, DomainPackReviewDecision,
    DomainPackReviewedEligibility, DomainPackReviewedRegistryDocument,
    DomainPackReviewedRegistryEntry, DomainPackReviewerIndependence, DomainPackReviewerRole,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Hard bound for diagnostics returned from one pure evaluation.
pub const MAX_DOMAIN_PACK_LEARNING_DIAGNOSTICS: usize = 256;

/// Inputs to the non-authoritative promotion-readiness evaluator.
#[derive(Debug, Clone, Copy)]
pub struct DomainPackPromotionEvaluationInput<'a> {
    pub dossier: &'a DomainPackPromotionDossierDocument,
    pub candidates: &'a [DomainPackLocalLearningCandidateDocument],
    pub independent_reviews: &'a [DomainPackIndependentReviewDocument],
    pub conflicts: &'a [DomainPackLearningConflictDocument],
}

/// Inputs to reviewed-registry semantic evolution.
#[derive(Debug, Clone, Copy)]
pub struct DomainPackReviewedRegistryEvolutionInput<'a> {
    pub current: Option<&'a DomainPackReviewedRegistryDocument>,
    pub proposed: &'a DomainPackReviewedRegistryDocument,
    /// Other observed heads for the proposed registry id. A different digest
    /// at the same generation is deterministic equivocation evidence.
    pub competing_heads: &'a [DomainPackReviewedRegistryDocument],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningDecisionAuthority {
    NonAuthoritativeEvaluation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionReadinessStatus {
    ReadyForTrustedReview,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLearningIssueCode {
    InvalidContract,
    InvalidStageTransition,
    MissingCandidate,
    CandidateTargetMismatch,
    NonAuthoritativeBypass,
    MissingEvaluationEvidence,
    NoOpComparison,
    RegressionDetected,
    UnknownGap,
    NonIndependentJudge,
    MissingIndependentReview,
    ReviewRejected,
    ReviewBindingMismatch,
    UnresolvedConflict,
    SemanticConflict,
    RegistryChainMismatch,
    RegistryEquivocation,
    RegistryEntryRemoved,
    RegistryEntryRewritten,
    InvalidRegistryStage,
    TerminalReactivation,
    SupersessionTargetNotReviewed,
    NoOpRegistrySuccessor,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLearningIssue {
    pub code: DomainPackLearningIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDetectedSemanticConflict {
    pub conflict_digest: String,
    pub candidate_digests: Vec<String>,
    pub reason: String,
}

/// Explicit deterministic request. It is a request only, not a signed review
/// document and not promotion authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExplicitReviewRequest {
    pub authority: DomainPackLearningDecisionAuthority,
    pub dossier_digest: String,
    pub conflict_digests: Vec<String>,
    pub required_roles: Vec<DomainPackReviewerRole>,
    pub minimum_independent_reviews: u16,
    pub request_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPromotionEvaluation {
    pub authority: DomainPackLearningDecisionAuthority,
    pub status: DomainPackPromotionReadinessStatus,
    pub transition_from: DomainPackPromotionStage,
    pub transition_to: DomainPackPromotionStage,
    pub detected_conflicts: Vec<DomainPackDetectedSemanticConflict>,
    pub review_request: Option<DomainPackExplicitReviewRequest>,
    pub issues: Vec<DomainPackLearningIssue>,
    pub evaluation_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewedRegistryEvolutionStatus {
    AdmissibleCandidate,
    GenesisCandidate,
    Replay,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackActivationCandidate {
    pub publisher: String,
    pub name: String,
    pub version: String,
    pub package_digest: String,
    pub supply_chain_record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedRegistryEvolution {
    pub authority: DomainPackLearningDecisionAuthority,
    pub status: DomainPackReviewedRegistryEvolutionStatus,
    pub from_generation: Option<u64>,
    pub to_generation: u64,
    /// Only reviewed and eligible exact records appear here. Deprecated,
    /// revoked, and superseded history is deliberately excluded.
    pub eligible_for_new_activation: Vec<DomainPackActivationCandidate>,
    pub issues: Vec<DomainPackLearningIssue>,
    pub evaluation_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReviewedResolutionJoinStatus {
    InvalidContract,
    EligibleReviewed,
    MissingReviewedRecord,
    AmbiguousReviewedRecord,
    SupplyChainRecordMismatch,
    ArtifactBindingMismatch,
    IneligibleDeprecated,
    IneligibleRevoked,
    IneligibleSuperseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedResolutionJoin {
    pub publisher: String,
    pub name: String,
    pub version: String,
    pub package_digest: String,
    pub registry_record_digest: String,
    pub status: DomainPackReviewedResolutionJoinStatus,
    pub reviewed_entry_digest: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewedResolutionProjection {
    pub authority: DomainPackLearningDecisionAuthority,
    pub resolution_digest: String,
    pub reviewed_registry_digest: String,
    pub all_selected_eligible: bool,
    pub joins: Vec<DomainPackReviewedResolutionJoin>,
    pub projection_digest: String,
}

/// Exact, non-authoritative join between a P6b resolution projection and the
/// reviewed P6c registry. Every supply-chain and artifact binding must match;
/// a coordinate/version match alone is insufficient.
#[must_use]
pub fn evaluate_domain_pack_resolution_reviewed_eligibility(
    resolution: &DomainPackResolutionProjectionDocument,
    registry: &DomainPackReviewedRegistryDocument,
) -> DomainPackReviewedResolutionProjection {
    let contract_valid = resolution.validate().is_empty() && registry.validate().is_empty();
    let projection = &resolution.domain_pack_resolution_projection;
    let reviewed = &registry.domain_pack_reviewed_registry;
    let mut joins = projection
        .selected
        .iter()
        .map(|selected| {
            let matching = reviewed
                .entries
                .iter()
                .filter(|entry| {
                    entry.pack.publisher == selected.identity.publisher
                        && entry.pack.name == selected.identity.name
                        && entry.pack.version == selected.identity.version
                        && entry.package_digest == selected.package.package_digest
                })
                .collect::<Vec<_>>();
            let (status, reviewed_entry_digest, reason) = match matching.as_slice() {
                [] => (
                    DomainPackReviewedResolutionJoinStatus::MissingReviewedRecord,
                    None,
                    "no exact reviewed package record".to_owned(),
                ),
                [entry] => {
                    let fixture_digests = selected
                        .package
                        .fixtures
                        .iter()
                        .map(|fixture| fixture.canonical_sha256.clone())
                        .collect::<Vec<_>>();
                    let artifact_match = entry.manifest_digest
                        == selected.package.manifest.canonical_sha256
                        && entry.content_digest == selected.package.content.canonical_sha256
                        && entry.license_digest == selected.package.license.canonical_sha256
                        && entry.fixture_digests == fixture_digests;
                    let status =
                        if entry.supply_chain_record_digest != selected.registry_record_digest {
                            DomainPackReviewedResolutionJoinStatus::SupplyChainRecordMismatch
                        } else if !artifact_match {
                            DomainPackReviewedResolutionJoinStatus::ArtifactBindingMismatch
                        } else {
                            match entry.eligibility {
                                DomainPackReviewedEligibility::EligibleReviewed
                                    if entry.stage == DomainPackPromotionStage::Reviewed =>
                                {
                                    DomainPackReviewedResolutionJoinStatus::EligibleReviewed
                                }
                                DomainPackReviewedEligibility::IneligibleDeprecated => {
                                    DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated
                                }
                                DomainPackReviewedEligibility::IneligibleRevoked => {
                                    DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
                                }
                                DomainPackReviewedEligibility::IneligibleSuperseded => {
                                    DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded
                                }
                                DomainPackReviewedEligibility::EligibleReviewed => {
                                    DomainPackReviewedResolutionJoinStatus::ArtifactBindingMismatch
                                }
                            }
                        };
                    (
                        status,
                        Some(entry.entry_digest.clone()),
                        reviewed_join_reason(status).to_owned(),
                    )
                }
                _ => (
                    DomainPackReviewedResolutionJoinStatus::AmbiguousReviewedRecord,
                    None,
                    "multiple reviewed records claim the same exact package identity".to_owned(),
                ),
            };
            DomainPackReviewedResolutionJoin {
                publisher: selected.identity.publisher.0.clone(),
                name: selected.identity.name.0.clone(),
                version: selected.identity.version.clone(),
                package_digest: selected.package.package_digest.clone(),
                registry_record_digest: selected.registry_record_digest.clone(),
                status,
                reviewed_entry_digest,
                reason,
            }
        })
        .collect::<Vec<_>>();
    if !contract_valid {
        for join in &mut joins {
            join.status = DomainPackReviewedResolutionJoinStatus::InvalidContract;
            join.reviewed_entry_digest = None;
            "resolution or reviewed-registry contract is invalid".clone_into(&mut join.reason);
        }
    }
    joins.sort_by(|left, right| {
        (
            &left.publisher,
            &left.name,
            &left.version,
            &left.package_digest,
        )
            .cmp(&(
                &right.publisher,
                &right.name,
                &right.version,
                &right.package_digest,
            ))
    });
    let all_selected_eligible = !joins.is_empty()
        && joins
            .iter()
            .all(|join| join.status == DomainPackReviewedResolutionJoinStatus::EligibleReviewed);
    let mut result = DomainPackReviewedResolutionProjection {
        authority: DomainPackLearningDecisionAuthority::NonAuthoritativeEvaluation,
        resolution_digest: projection.resolution_digest.clone(),
        reviewed_registry_digest: reviewed.registry_digest.clone(),
        all_selected_eligible,
        joins,
        projection_digest: String::new(),
    };
    result.projection_digest = canonical_digest(&result);
    result
}

/// Concise integration alias for the exact P6b-to-P6c reviewed join.
#[must_use]
pub fn join_reviewed_registry_to_resolution(
    resolution: &DomainPackResolutionProjectionDocument,
    registry: &DomainPackReviewedRegistryDocument,
) -> DomainPackReviewedResolutionProjection {
    evaluate_domain_pack_resolution_reviewed_eligibility(resolution, registry)
}

const fn reviewed_join_reason(status: DomainPackReviewedResolutionJoinStatus) -> &'static str {
    match status {
        DomainPackReviewedResolutionJoinStatus::InvalidContract => {
            "resolution or reviewed-registry contract is invalid"
        }
        DomainPackReviewedResolutionJoinStatus::EligibleReviewed => {
            "exact supply-chain and artifact bindings are reviewed and eligible"
        }
        DomainPackReviewedResolutionJoinStatus::MissingReviewedRecord => {
            "no exact reviewed package record"
        }
        DomainPackReviewedResolutionJoinStatus::AmbiguousReviewedRecord => {
            "multiple exact reviewed package records"
        }
        DomainPackReviewedResolutionJoinStatus::SupplyChainRecordMismatch => {
            "supply-chain record digest does not match the reviewed record"
        }
        DomainPackReviewedResolutionJoinStatus::ArtifactBindingMismatch => {
            "manifest, content, license, fixture, or stage binding differs"
        }
        DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated => {
            "deprecated reviewed history is excluded from new activation"
        }
        DomainPackReviewedResolutionJoinStatus::IneligibleRevoked => {
            "revoked reviewed history is excluded from new activation"
        }
        DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded => {
            "superseded reviewed history is excluded from new activation"
        }
    }
}

/// Evaluates a promotion dossier without granting promotion authority.
#[must_use]
pub fn evaluate_domain_pack_promotion(
    input: &DomainPackPromotionEvaluationInput<'_>,
) -> DomainPackPromotionEvaluation {
    let dossier = &input.dossier.domain_pack_promotion_dossier;
    let mut issues = Vec::new();

    append_contract_issues(&mut issues, "dossier", input.dossier.validate());
    for (index, candidate) in input.candidates.iter().enumerate() {
        append_contract_issues(
            &mut issues,
            &format!("candidates[{index}]"),
            candidate.validate(),
        );
    }
    for (index, review) in input.independent_reviews.iter().enumerate() {
        append_contract_issues(
            &mut issues,
            &format!("independent_reviews[{index}]"),
            review.validate(),
        );
    }
    for (index, conflict) in input.conflicts.iter().enumerate() {
        append_contract_issues(
            &mut issues,
            &format!("conflicts[{index}]"),
            conflict.validate(),
        );
    }

    if !dossier.transition.is_allowed() {
        issue(
            &mut issues,
            DomainPackLearningIssueCode::InvalidStageTransition,
            "dossier.transition",
            "the transition is outside the closed promotion graph",
        );
    }

    let candidates_by_digest = input
        .candidates
        .iter()
        .map(|document| {
            (
                document
                    .domain_pack_local_learning_candidate
                    .candidate_digest
                    .as_str(),
                &document.domain_pack_local_learning_candidate,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (index, digest) in dossier.candidate_digests.iter().enumerate() {
        let Some(candidate) = candidates_by_digest.get(digest.as_str()) else {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::MissingCandidate,
                format!("dossier.candidate_digests[{index}]"),
                "the dossier references a candidate not present in the durable input graph",
            );
            continue;
        };
        if candidate.target.pack.publisher != dossier.pack.publisher
            || candidate.target.pack.name != dossier.pack.name
            || candidate.target.base_version.as_deref() != Some(dossier.pack.version.as_str())
        {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::CandidateTargetMismatch,
                format!("candidates[{index}].target.pack"),
                "candidate target and base version do not match the exact dossier package",
            );
        }
    }

    // Merely authoring a dossier, mentioning it in chat, or placing it in
    // memory never substitutes for exact candidate and evaluation evidence.
    if dossier.candidate_digests.is_empty() || input.candidates.is_empty() {
        issue(
            &mut issues,
            DomainPackLearningIssueCode::NonAuthoritativeBypass,
            "dossier.candidate_digests",
            "chat, memory, authorship, and document presence cannot create reviewed authority",
        );
    }

    evaluate_comparisons(dossier, &mut issues);
    evaluate_reviews(dossier, input.independent_reviews, &mut issues);

    let mut detected_conflicts = detect_semantic_conflicts(input.candidates);
    let referenced_conflicts = dossier
        .conflict_record_digests
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut conflicts_by_digest = BTreeMap::new();
    for (index, document) in input.conflicts.iter().enumerate() {
        let conflict = &document.domain_pack_learning_conflict;
        if conflicts_by_digest
            .insert(conflict.conflict_digest.as_str(), conflict)
            .is_some()
        {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::ReviewBindingMismatch,
                format!("conflicts[{index}].conflict_digest"),
                "duplicate conflict digest makes resolution evidence ambiguous",
            );
        }
    }
    for detected in &mut detected_conflicts {
        let matching_records = input
            .conflicts
            .iter()
            .filter(|document| {
                let record = &document.domain_pack_learning_conflict;
                let mut recorded_subjects = record.subject_digests.clone();
                recorded_subjects.sort();
                recorded_subjects.dedup();
                recorded_subjects == detected.candidate_digests
                    && detected.candidate_digests.iter().all(|digest| {
                        candidates_by_digest
                            .get(digest.as_str())
                            .is_some_and(|candidate| candidate.target == record.target)
                    })
            })
            .collect::<Vec<_>>();
        if matching_records.len() == 1 {
            detected.conflict_digest = matching_records[0]
                .domain_pack_learning_conflict
                .conflict_digest
                .clone();
        } else if matching_records.len() > 1 {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::ReviewBindingMismatch,
                "conflicts",
                "multiple conflict records claim the same exact candidate conflict",
            );
        }
    }
    let supplied_review_digests = input
        .independent_reviews
        .iter()
        .map(|document| {
            document
                .domain_pack_independent_review
                .review_digest
                .as_str()
        })
        .collect::<BTreeSet<_>>();
    for (index, digest) in dossier.conflict_record_digests.iter().enumerate() {
        if !conflicts_by_digest.contains_key(digest.as_str()) {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::ReviewBindingMismatch,
                format!("dossier.conflict_record_digests[{index}]"),
                "dossier conflict binding is absent from the durable input graph",
            );
        }
    }
    for (index, conflict) in input.conflicts.iter().enumerate() {
        let conflict = &conflict.domain_pack_learning_conflict;
        if !referenced_conflicts.contains(conflict.conflict_digest.as_str()) {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::ReviewBindingMismatch,
                format!("conflicts[{index}].conflict_digest"),
                "conflict evidence is not bound by the promotion dossier",
            );
        }
        if matches!(
            conflict.status,
            DomainPackLearningConflictStatus::Open
                | DomainPackLearningConflictStatus::ReviewRequested
        ) {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::UnresolvedConflict,
                format!("conflicts[{index}].status"),
                "unresolved learning cannot be silently promoted",
            );
        }
        if let Some(resolution) = &conflict.resolution {
            for (review_index, review_digest) in
                resolution.resolved_by_review_digests.iter().enumerate()
            {
                if !supplied_review_digests.contains(review_digest.as_str()) {
                    issue(
                        &mut issues,
                        DomainPackLearningIssueCode::ReviewBindingMismatch,
                        format!(
                            "conflicts[{index}].resolution.resolved_by_review_digests[{review_index}]"
                        ),
                        "conflict resolution cites a review absent from the durable input graph",
                    );
                }
            }
        }
    }
    detected_conflicts.sort_by(|left, right| left.conflict_digest.cmp(&right.conflict_digest));

    let unresolved_detected = detected_conflicts
        .iter()
        .filter(|detected| {
            let Some(record) = conflicts_by_digest.get(detected.conflict_digest.as_str()) else {
                return true;
            };
            let mut recorded_subjects = record.subject_digests.clone();
            recorded_subjects.sort();
            recorded_subjects.dedup();
            record.status != DomainPackLearningConflictStatus::Resolved
                || !referenced_conflicts.contains(detected.conflict_digest.as_str())
                || recorded_subjects != detected.candidate_digests
        })
        .collect::<Vec<_>>();
    for conflict in &unresolved_detected {
        issue(
            &mut issues,
            DomainPackLearningIssueCode::SemanticConflict,
            "candidates",
            format!(
                "contradictory candidate assertions require review ({})",
                conflict.conflict_digest
            ),
        );
    }

    let mut conflict_digests = unresolved_detected
        .iter()
        .map(|conflict| conflict.conflict_digest.clone())
        .chain(
            input
                .conflicts
                .iter()
                .filter(|document| {
                    matches!(
                        document.domain_pack_learning_conflict.status,
                        DomainPackLearningConflictStatus::Open
                            | DomainPackLearningConflictStatus::ReviewRequested
                    )
                })
                .map(|document| {
                    document
                        .domain_pack_learning_conflict
                        .conflict_digest
                        .clone()
                }),
        )
        .collect::<Vec<_>>();
    conflict_digests.sort();
    conflict_digests.dedup();
    let review_request = (!conflict_digests.is_empty())
        .then(|| explicit_review_request(&dossier.dossier_digest, conflict_digests));

    sort_and_bound(&mut issues);
    let status = if issues.iter().any(|entry| {
        !matches!(
            entry.code,
            DomainPackLearningIssueCode::SemanticConflict
                | DomainPackLearningIssueCode::UnresolvedConflict
        )
    }) {
        DomainPackPromotionReadinessStatus::Blocked
    } else if review_request.is_some() {
        DomainPackPromotionReadinessStatus::ReviewRequired
    } else {
        DomainPackPromotionReadinessStatus::ReadyForTrustedReview
    };

    let mut evaluation = DomainPackPromotionEvaluation {
        authority: DomainPackLearningDecisionAuthority::NonAuthoritativeEvaluation,
        status,
        transition_from: dossier.transition.from,
        transition_to: dossier.transition.to,
        detected_conflicts,
        review_request,
        issues,
        evaluation_digest: String::new(),
    };
    evaluation.evaluation_digest = canonical_digest(&evaluation);
    evaluation
}

/// Evaluates append-only reviewed-registry semantics. Cryptographic snapshot
/// verification and monotonic anchor mutation remain outside this crate.
#[must_use]
pub fn evaluate_domain_pack_reviewed_registry_evolution(
    input: &DomainPackReviewedRegistryEvolutionInput<'_>,
) -> DomainPackReviewedRegistryEvolution {
    let proposed = &input.proposed.domain_pack_reviewed_registry;
    let mut issues = Vec::new();
    append_contract_issues(&mut issues, "proposed", input.proposed.validate());
    detect_coordinate_version_equivocation(proposed.entries.as_slice(), &mut issues);
    if let Some(current) = input.current {
        append_contract_issues(&mut issues, "current", current.validate());
    }
    for (index, head) in input.competing_heads.iter().enumerate() {
        append_contract_issues(
            &mut issues,
            &format!("competing_heads[{index}]"),
            head.validate(),
        );
        let head = &head.domain_pack_reviewed_registry;
        if head.registry_id == proposed.registry_id
            && head.generation == proposed.generation
            && !reviewed_registry_body_equal(head, proposed)
        {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::RegistryEquivocation,
                format!("competing_heads[{index}].registry_digest"),
                "different reviewed-registry digests were observed for the same id and generation",
            );
        }
    }

    let status = if let Some(current_document) = input.current {
        let current = &current_document.domain_pack_reviewed_registry;
        if proposed.registry_id != current.registry_id || proposed.audience != current.audience {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::RegistryChainMismatch,
                "proposed.registry_id",
                "reviewed registry identity or audience changed",
            );
        }
        if proposed.generation == current.generation
            && proposed.registry_digest == current.registry_digest
        {
            if !reviewed_registry_body_equal(proposed, current) {
                issue(
                    &mut issues,
                    DomainPackLearningIssueCode::RegistryEquivocation,
                    "proposed.registry_digest",
                    "same reviewed-registry digest and generation carry a different body",
                );
            }
            DomainPackReviewedRegistryEvolutionStatus::Replay
        } else {
            if proposed.registry_digest == current.registry_digest {
                issue(
                    &mut issues,
                    DomainPackLearningIssueCode::RegistryEquivocation,
                    "proposed.registry_digest",
                    "a successor generation cannot reuse its predecessor digest",
                );
            }
            if proposed.generation != current.generation.saturating_add(1)
                || proposed.previous_registry_digest.as_deref()
                    != Some(current.registry_digest.as_str())
            {
                issue(
                    &mut issues,
                    DomainPackLearningIssueCode::RegistryChainMismatch,
                    "proposed.previous_registry_digest",
                    "proposed registry is not the exact direct successor",
                );
            }
            compare_registry_entries(
                current.entries.as_slice(),
                proposed.entries.as_slice(),
                &mut issues,
            );
            if current.entries == proposed.entries {
                issue(
                    &mut issues,
                    DomainPackLearningIssueCode::NoOpRegistrySuccessor,
                    "proposed.entries",
                    "a new generation must carry a reviewed semantic change",
                );
            }
            DomainPackReviewedRegistryEvolutionStatus::AdmissibleCandidate
        }
    } else {
        if proposed.generation != 0 || proposed.previous_registry_digest.is_some() {
            issue(
                &mut issues,
                DomainPackLearningIssueCode::RegistryChainMismatch,
                "proposed.previous_registry_digest",
                "genesis must be generation zero without a predecessor",
            );
        }
        for (index, entry) in proposed.entries.iter().enumerate() {
            require_new_reviewed_entry(entry, index, &mut issues);
        }
        DomainPackReviewedRegistryEvolutionStatus::GenesisCandidate
    };

    let mut eligible_for_new_activation = proposed
        .entries
        .iter()
        .filter(|entry| {
            entry.stage == DomainPackPromotionStage::Reviewed
                && entry.eligibility == DomainPackReviewedEligibility::EligibleReviewed
        })
        .map(|entry| DomainPackActivationCandidate {
            publisher: entry.pack.publisher.0.clone(),
            name: entry.pack.name.0.clone(),
            version: entry.pack.version.clone(),
            package_digest: entry.package_digest.clone(),
            supply_chain_record_digest: entry.supply_chain_record_digest.clone(),
        })
        .collect::<Vec<_>>();
    eligible_for_new_activation.sort_by(|left, right| {
        (
            &left.publisher,
            &left.name,
            &left.version,
            &left.package_digest,
        )
            .cmp(&(
                &right.publisher,
                &right.name,
                &right.version,
                &right.package_digest,
            ))
    });

    sort_and_bound(&mut issues);
    let status = if issues.is_empty() {
        status
    } else {
        DomainPackReviewedRegistryEvolutionStatus::Blocked
    };
    let mut evaluation = DomainPackReviewedRegistryEvolution {
        authority: DomainPackLearningDecisionAuthority::NonAuthoritativeEvaluation,
        status,
        from_generation: input
            .current
            .map(|document| document.domain_pack_reviewed_registry.generation),
        to_generation: proposed.generation,
        eligible_for_new_activation,
        issues,
        evaluation_digest: String::new(),
    };
    evaluation.evaluation_digest = canonical_digest(&evaluation);
    evaluation
}

fn evaluate_comparisons(
    dossier: &forge_core_contracts::DomainPackPromotionDossier,
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    let evidence_ids = dossier
        .evidence
        .iter()
        .map(|evidence| evidence.evidence_id.0.as_str())
        .collect::<BTreeSet<_>>();
    let fixture_producers = dossier
        .fixture_bindings
        .iter()
        .map(|fixture| fixture.producer.0.as_str())
        .collect::<BTreeSet<_>>();
    let authors = dossier
        .provenance
        .authored_by
        .iter()
        .map(|author| author.0.as_str())
        .collect::<BTreeSet<_>>();
    let requires_strong_proof = dossier.transition.to == DomainPackPromotionStage::Reviewed;
    let mut qualifying = false;
    for (index, run) in dossier.evaluator_runs.iter().enumerate() {
        if !evidence_ids.contains(run.evidence_ref.0.as_str()) {
            issue(
                issues,
                DomainPackLearningIssueCode::MissingEvaluationEvidence,
                format!("dossier.evaluator_runs[{index}].evidence_ref"),
                "evaluator run is not joined to durable dossier evidence",
            );
        }
        if run.comparison.baseline_outcome_digest == run.comparison.candidate_outcome_digest {
            issue(
                issues,
                DomainPackLearningIssueCode::NoOpComparison,
                format!("dossier.evaluator_runs[{index}].comparison"),
                "baseline and candidate outcome digests are identical",
            );
        }
        if matches!(
            run.comparison.verdict,
            DomainPackLearningComparisonVerdict::Regressed
        ) || !run.comparison.regression_finding_refs.is_empty()
        {
            issue(
                issues,
                DomainPackLearningIssueCode::RegressionDetected,
                format!("dossier.evaluator_runs[{index}].comparison"),
                "a favorable aggregate cannot hide a regression",
            );
        }
        if !run.comparison.unknown_gap_refs.is_empty() {
            issue(
                issues,
                DomainPackLearningIssueCode::UnknownGap,
                format!("dossier.evaluator_runs[{index}].comparison.unknown_gap_refs"),
                "unknown evaluation gaps remain explicit and block promotion",
            );
        }
        let proof_is_independent = match run.comparison.method {
            DomainPackLearningComparisonMethod::Ablation => true,
            DomainPackLearningComparisonMethod::StrongJudge => {
                run.strong_judge_proof.as_ref().is_some_and(|proof| {
                    proof.blind_ab
                        && proof.judge_principal == run.evaluator_principal
                        && !authors.contains(proof.judge_principal.0.as_str())
                        && !fixture_producers.contains(proof.judge_principal.0.as_str())
                })
            }
            DomainPackLearningComparisonMethod::ControlledReplay => false,
        };
        if run.comparison.method == DomainPackLearningComparisonMethod::StrongJudge
            && !proof_is_independent
        {
            issue(
                issues,
                DomainPackLearningIssueCode::NonIndependentJudge,
                format!("dossier.evaluator_runs[{index}].strong_judge_proof"),
                "strong judge must be blind, exact, and independent from authors and fixture producers",
            );
        }
        qualifying |= matches!(
            run.comparison.method,
            DomainPackLearningComparisonMethod::Ablation
                | DomainPackLearningComparisonMethod::StrongJudge
        ) && proof_is_independent
            && run.comparison.verdict == DomainPackLearningComparisonVerdict::Improved
            && run.comparison.baseline_outcome_digest != run.comparison.candidate_outcome_digest
            && run.comparison.regression_finding_refs.is_empty()
            && run.comparison.unknown_gap_refs.is_empty();
    }
    if !dossier.open_gap_refs.is_empty() {
        issue(
            issues,
            DomainPackLearningIssueCode::UnknownGap,
            "dossier.open_gap_refs",
            "open dossier gaps block promotion",
        );
    }
    if requires_strong_proof && !qualifying {
        issue(
            issues,
            DomainPackLearningIssueCode::MissingEvaluationEvidence,
            "dossier.evaluator_runs",
            "reviewed promotion requires a non-no-op improved ablation or independent strong-judge proof",
        );
    }
}

fn evaluate_reviews(
    dossier: &forge_core_contracts::DomainPackPromotionDossier,
    documents: &[DomainPackIndependentReviewDocument],
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    let review_required = matches!(
        dossier.transition.to,
        DomainPackPromotionStage::Reviewed
            | DomainPackPromotionStage::Deprecated
            | DomainPackPromotionStage::Revoked
            | DomainPackPromotionStage::Superseded
    );
    if !review_required {
        return;
    }
    let authors = dossier
        .provenance
        .authored_by
        .iter()
        .map(|principal| principal.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut reviewers = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for (index, document) in documents.iter().enumerate() {
        let review = &document.domain_pack_independent_review;
        if review.dossier_digest != dossier.dossier_digest
            || review.signed_subject_digest != dossier.dossier_digest
        {
            issue(
                issues,
                DomainPackLearningIssueCode::ReviewBindingMismatch,
                format!("independent_reviews[{index}].dossier_digest"),
                "review does not bind the exact dossier",
            );
            continue;
        }
        let independent = matches!(
            review.independence,
            DomainPackReviewerIndependence::Independent { .. }
        ) && !authors.contains(review.reviewer_id.0.as_str());
        if review.decision == DomainPackReviewDecision::Reject {
            issue(
                issues,
                DomainPackLearningIssueCode::ReviewRejected,
                format!("independent_reviews[{index}].decision"),
                "an exact bound independent-review rejection vetoes promotion",
            );
        } else if review.decision == DomainPackReviewDecision::Approve && independent {
            reviewers.insert(review.reviewer_id.0.as_str());
            roles.insert(review.reviewer_role);
        } else if review.decision == DomainPackReviewDecision::Approve {
            issue(
                issues,
                DomainPackLearningIssueCode::MissingIndependentReview,
                format!("independent_reviews[{index}].independence"),
                "an author or non-independent reviewer cannot approve promotion",
            );
        }
    }
    if reviewers.len() < 2 || roles.len() < 2 {
        issue(
            issues,
            DomainPackLearningIssueCode::MissingIndependentReview,
            "independent_reviews",
            "promotion boundary requires two distinct independent reviewers in two roles",
        );
    }
}

fn detect_semantic_conflicts(
    documents: &[DomainPackLocalLearningCandidateDocument],
) -> Vec<DomainPackDetectedSemanticConflict> {
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for document in documents {
        let candidate = &document.domain_pack_local_learning_candidate;
        let key = canonical_digest(&candidate.target);
        let assertion = candidate
            .assertion
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        groups
            .entry(key)
            .or_default()
            .entry(assertion)
            .or_default()
            .push(candidate.candidate_digest.clone());
    }
    groups
        .into_iter()
        .filter(|(_, assertions)| assertions.len() > 1)
        .map(|(target_digest, assertions)| {
            let mut candidate_digests = assertions.into_values().flatten().collect::<Vec<_>>();
            candidate_digests.sort();
            candidate_digests.dedup();
            let conflict_digest = canonical_digest(&(target_digest, &candidate_digests));
            DomainPackDetectedSemanticConflict {
                conflict_digest,
                candidate_digests,
                reason: "same exact learning target has contradictory normalized assertions"
                    .to_owned(),
            }
        })
        .collect()
}

fn explicit_review_request(
    dossier_digest: &str,
    conflict_digests: Vec<String>,
) -> DomainPackExplicitReviewRequest {
    let mut request = DomainPackExplicitReviewRequest {
        authority: DomainPackLearningDecisionAuthority::NonAuthoritativeEvaluation,
        dossier_digest: dossier_digest.to_owned(),
        conflict_digests,
        required_roles: vec![
            DomainPackReviewerRole::DomainExpert,
            DomainPackReviewerRole::EvidenceReviewer,
        ],
        minimum_independent_reviews: 2,
        request_digest: String::new(),
    };
    request.request_digest = canonical_digest(&request);
    request
}

type EntryIdentity<'a> = (&'a str, &'a str, &'a str, &'a str);

fn entry_identity(entry: &DomainPackReviewedRegistryEntry) -> EntryIdentity<'_> {
    (
        entry.pack.publisher.0.as_str(),
        entry.pack.name.0.as_str(),
        entry.pack.version.as_str(),
        entry.package_digest.as_str(),
    )
}

fn reviewed_registry_body_equal(
    left: &forge_core_contracts::DomainPackReviewedRegistry,
    right: &forge_core_contracts::DomainPackReviewedRegistry,
) -> bool {
    left.registry_id == right.registry_id
        && left.audience == right.audience
        && left.generation == right.generation
        && left.previous_registry_digest == right.previous_registry_digest
        && left.entries == right.entries
        && left.registry_digest == right.registry_digest
}

fn detect_coordinate_version_equivocation(
    entries: &[DomainPackReviewedRegistryEntry],
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    let mut packages = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let coordinate_version = (
            entry.pack.publisher.0.as_str(),
            entry.pack.name.0.as_str(),
            entry.pack.version.as_str(),
        );
        if let Some(existing) = packages.insert(coordinate_version, entry.package_digest.as_str()) {
            if existing != entry.package_digest {
                issue(
                    issues,
                    DomainPackLearningIssueCode::RegistryEquivocation,
                    format!("proposed.entries[{index}].package_digest"),
                    "one publisher/name/version claims divergent package digests",
                );
            }
        }
    }
}

fn compare_registry_entries(
    current: &[DomainPackReviewedRegistryEntry],
    proposed: &[DomainPackReviewedRegistryEntry],
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    let proposed_by_identity = proposed
        .iter()
        .map(|entry| (entry_identity(entry), entry))
        .collect::<BTreeMap<_, _>>();
    let current_identities = current.iter().map(entry_identity).collect::<BTreeSet<_>>();
    for (index, before) in current.iter().enumerate() {
        let Some(after) = proposed_by_identity.get(&entry_identity(before)).copied() else {
            issue(
                issues,
                DomainPackLearningIssueCode::RegistryEntryRemoved,
                format!("current.entries[{index}]"),
                "reviewed history is append-only; removal and tombstoning are forbidden",
            );
            continue;
        };
        compare_existing_entry(before, after, index, issues);
    }
    for (index, entry) in proposed.iter().enumerate() {
        if !current_identities.contains(&entry_identity(entry)) {
            require_new_reviewed_entry(entry, index, issues);
        }
    }
    for (index, entry) in proposed.iter().enumerate() {
        let Some(binding) = &entry.supersession else {
            continue;
        };
        let target = proposed.iter().find(|candidate| {
            candidate.pack == binding.replacement_pack
                && candidate.package_digest == binding.replacement_package_digest
        });
        if !target.is_some_and(|target| {
            target.stage == DomainPackPromotionStage::Reviewed
                && target.eligibility == DomainPackReviewedEligibility::EligibleReviewed
        }) {
            issue(
                issues,
                DomainPackLearningIssueCode::SupersessionTargetNotReviewed,
                format!("proposed.entries[{index}].supersession"),
                "supersession target must be an exact reviewed eligible record in the proposed snapshot",
            );
        }
    }
}

fn compare_existing_entry(
    before: &DomainPackReviewedRegistryEntry,
    after: &DomainPackReviewedRegistryEntry,
    index: usize,
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    let immutable_same = before.pack == after.pack
        && before.package_digest == after.package_digest
        && before.supply_chain_record_digest == after.supply_chain_record_digest
        && before.manifest_digest == after.manifest_digest
        && before.content_digest == after.content_digest
        && before.license_digest == after.license_digest
        && before.fixture_digests == after.fixture_digests
        && before.compatibility == after.compatibility;
    if !immutable_same {
        issue(
            issues,
            DomainPackLearningIssueCode::RegistryEntryRewritten,
            format!("proposed.entries[{index}]"),
            "an existing exact record changed immutable supply-chain or compatibility semantics",
        );
    }
    if before.stage == after.stage {
        if before != after {
            issue(
                issues,
                DomainPackLearningIssueCode::RegistryEntryRewritten,
                format!("proposed.entries[{index}]"),
                "same-stage records must remain byte-semantically identical",
            );
        }
        return;
    }
    if matches!(
        before.stage,
        DomainPackPromotionStage::Revoked | DomainPackPromotionStage::Superseded
    ) {
        issue(
            issues,
            DomainPackLearningIssueCode::TerminalReactivation,
            format!("proposed.entries[{index}].stage"),
            "revoked and superseded records are terminal and cannot be revived",
        );
        return;
    }
    let allowed = matches!(
        (before.stage, after.stage),
        (
            DomainPackPromotionStage::Reviewed,
            DomainPackPromotionStage::Deprecated
                | DomainPackPromotionStage::Revoked
                | DomainPackPromotionStage::Superseded
        ) | (
            DomainPackPromotionStage::Deprecated,
            DomainPackPromotionStage::Revoked | DomainPackPromotionStage::Superseded
        )
    );
    if !allowed {
        issue(
            issues,
            DomainPackLearningIssueCode::InvalidRegistryStage,
            format!("proposed.entries[{index}].stage"),
            "reviewed-registry stage skipped, moved backward, or reactivated",
        );
    }
}

fn require_new_reviewed_entry(
    entry: &DomainPackReviewedRegistryEntry,
    index: usize,
    issues: &mut Vec<DomainPackLearningIssue>,
) {
    if entry.stage != DomainPackPromotionStage::Reviewed
        || entry.eligibility != DomainPackReviewedEligibility::EligibleReviewed
    {
        issue(
            issues,
            DomainPackLearningIssueCode::InvalidRegistryStage,
            format!("proposed.entries[{index}].stage"),
            "a new registry identity must enter as reviewed and eligible; tombstones cannot create history",
        );
    }
}

fn append_contract_issues(
    issues: &mut Vec<DomainPackLearningIssue>,
    prefix: &str,
    contract_issues: Vec<forge_core_contracts::DomainPackLearningContractIssue>,
) {
    for contract_issue in contract_issues {
        issue(
            issues,
            DomainPackLearningIssueCode::InvalidContract,
            format!("{prefix}.{}", contract_issue.path),
            format!("{:?}: {}", contract_issue.code, contract_issue.message),
        );
    }
}

fn issue(
    issues: &mut Vec<DomainPackLearningIssue>,
    code: DomainPackLearningIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    if issues.len() < MAX_DOMAIN_PACK_LEARNING_DIAGNOSTICS {
        issues.push(DomainPackLearningIssue {
            code,
            path: path.into(),
            message: message.into(),
        });
    } else if !issues
        .iter()
        .any(|entry| entry.code == DomainPackLearningIssueCode::ResourceLimitExceeded)
    {
        issues.pop();
        issues.push(DomainPackLearningIssue {
            code: DomainPackLearningIssueCode::ResourceLimitExceeded,
            path: "diagnostics".to_owned(),
            message: format!(
                "diagnostics truncated at {MAX_DOMAIN_PACK_LEARNING_DIAGNOSTICS} entries"
            ),
        });
    }
}

fn sort_and_bound(issues: &mut Vec<DomainPackLearningIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.path.cmp(&right.path))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    issues.truncate(MAX_DOMAIN_PACK_LEARNING_DIAGNOSTICS);
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_vec(value).map_or_else(
        |_| {
            format!(
                "{:x}",
                Sha256::digest(b"domain-pack-learning-encoding-failed")
            )
        },
        |bytes| format!("{:x}", Sha256::digest(bytes)),
    )
}
