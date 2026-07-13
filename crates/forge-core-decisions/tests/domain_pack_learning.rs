use forge_core_contracts::*;
use forge_core_decisions::{
    evaluate_domain_pack_promotion, evaluate_domain_pack_reviewed_registry_evolution,
    join_reviewed_registry_to_resolution, DomainPackLearningIssueCode,
    DomainPackPromotionEvaluationInput, DomainPackPromotionReadinessStatus,
    DomainPackReviewedRegistryEvolutionInput, DomainPackReviewedRegistryEvolutionStatus,
};

fn digest(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn supply_digest(byte: char) -> String {
    format!("sha256:{}", digest(byte))
}

fn sid(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn principal(value: &str) -> PrincipalId {
    PrincipalId(value.to_owned())
}

fn version(version: &str) -> DomainPackVersionReference {
    DomainPackVersionReference {
        publisher: sid("forge.reference"),
        name: sid("game-production"),
        version: version.to_owned(),
    }
}

fn coordinate() -> DomainPackCoordinate {
    DomainPackCoordinate {
        publisher: sid("forge.reference"),
        name: sid("game-production"),
    }
}

fn evidence(id: &str, kind: DomainPackLearningEvidenceKind) -> DomainPackLearningEvidenceBinding {
    DomainPackLearningEvidenceBinding {
        evidence_id: sid(id),
        kind,
        artifact: DomainPackArtifactBinding {
            artifact_ref: RepoPath(format!("evidence/{id}.yaml")),
            raw_sha256: supply_digest('1'),
            canonical_sha256: supply_digest('2'),
        },
        producer: principal("evaluator.one"),
        produced_at_unix: 100,
        provenance_digest: digest('3'),
    }
}

fn candidate(
    id: &str,
    assertion: &str,
    digest_byte: char,
) -> DomainPackLocalLearningCandidateDocument {
    DomainPackLocalLearningCandidateDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_local_learning_candidate: DomainPackLocalLearningCandidate {
            candidate_id: sid(id),
            authority: DomainPackLocalLearningAuthority::NonAuthoritativeObservation,
            target: DomainPackLearningTarget {
                pack: coordinate(),
                base_version: Some("1.0.0".to_owned()),
                contribution_ref: Some(sid("rule.game-loop")),
                proposed_namespace: sid("game.production"),
            },
            assertion: assertion.to_owned(),
            provenance: DomainPackLearningProvenance {
                source_kind: DomainPackLearningSourceKind::RunEvidence,
                source_ref: "runs/representative-1".to_owned(),
                source_digest: digest('4'),
                captured_by: principal("agent.capture"),
                capture_run_id: sid("run.capture.1"),
                chat_transcript_ref: Some(RepoPath("chat/audit-only.md".to_owned())),
            },
            evidence: vec![evidence(
                "candidate-reproduction",
                DomainPackLearningEvidenceKind::Reproduction,
            )],
            observed_at_unix: 90,
            candidate_digest: digest(digest_byte),
        },
    }
}

fn dossier(candidate_digests: Vec<String>) -> DomainPackPromotionDossierDocument {
    DomainPackPromotionDossierDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_promotion_dossier: DomainPackPromotionDossier {
            dossier_id: sid("dossier.game-production.1"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            pack: version("1.0.0"),
            package_digest: supply_digest('6'),
            manifest_digest: supply_digest('7'),
            content_digest: supply_digest('8'),
            license_digest: supply_digest('9'),
            transition: DomainPackPromotionTransition {
                from: DomainPackPromotionStage::Validated,
                to: DomainPackPromotionStage::Reviewed,
            },
            candidate_digests,
            prior_promotion_record_digest: Some(digest('a')),
            evidence: vec![evidence(
                "evaluation-ablation",
                DomainPackLearningEvidenceKind::Ablation,
            )],
            evaluator_runs: vec![DomainPackLearningEvaluatorRun {
                run_id: sid("evaluation.ablation.1"),
                evaluator_ref: sid("evaluator.ablation.v1"),
                evaluator_principal: principal("evaluator.one"),
                evaluator_digest: digest('b'),
                fixture_set_digest: digest('c'),
                protocol_version: "1".to_owned(),
                comparison: DomainPackLearningComparison {
                    method: DomainPackLearningComparisonMethod::Ablation,
                    baseline_outcome_digest: digest('d'),
                    candidate_outcome_digest: digest('e'),
                    verdict: DomainPackLearningComparisonVerdict::Improved,
                    regression_finding_refs: vec![],
                    unknown_gap_refs: vec![],
                    rationale: "candidate improves representative outcomes".to_owned(),
                },
                strong_judge_proof: None,
                evidence_ref: sid("evaluation-ablation"),
                run_digest: digest('f'),
            }],
            fixture_bindings: vec![DomainPackLearningFixtureBinding {
                fixture_id: sid("fixture.representative.1"),
                fixture_ref: RepoPath("fixtures/representative.yaml".to_owned()),
                producer: principal("fixture.author"),
                raw_sha256: supply_digest('1'),
                canonical_sha256: supply_digest('2'),
                expected_outcome_digest: digest('3'),
                provenance_digest: digest('4'),
            }],
            provenance: DomainPackPromotionProvenance {
                authored_by: vec![principal("pack.author")],
                source_repository: "https://example.invalid/pack".to_owned(),
                source_revision: "deadbeef".to_owned(),
                source_tree_digest: digest('5'),
                build_recipe_digest: digest('6'),
                generated_artifact_refs: vec![],
            },
            conflict_record_digests: vec![],
            open_gap_refs: vec![],
            dossier_digest: digest('7'),
        },
    }
}

fn review(id: &str, role: DomainPackReviewerRole) -> DomainPackIndependentReviewDocument {
    DomainPackIndependentReviewDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_independent_review: DomainPackIndependentReview {
            review_id: sid(&format!("review.{id}")),
            authority: DomainPackReviewAuthority::ReviewEvidenceOnly,
            dossier_digest: digest('7'),
            reviewer_id: principal(id),
            reviewer_role: role,
            reviewer_registry_digest: digest('8'),
            credential_id: sid(&format!("credential.{id}")),
            independence: DomainPackReviewerIndependence::Independent {
                attestation: "no author, evaluator, fixture, or judge overlap".to_owned(),
            },
            decision: DomainPackReviewDecision::Approve,
            findings: vec![],
            signed_subject_digest: digest('7'),
            issued_at_unix: 100,
            expires_at_unix: 200,
            review_digest: digest(if id == "reviewer.one" { '8' } else { '9' }),
        },
    }
}

fn promotion_input<'a>(
    dossier: &'a DomainPackPromotionDossierDocument,
    candidates: &'a [DomainPackLocalLearningCandidateDocument],
    reviews: &'a [DomainPackIndependentReviewDocument],
) -> DomainPackPromotionEvaluationInput<'a> {
    DomainPackPromotionEvaluationInput {
        dossier,
        candidates,
        independent_reviews: reviews,
        conflicts: &[],
    }
}

#[test]
fn reviewed_promotion_is_deterministic_but_remains_non_authoritative() {
    let candidates = vec![candidate("candidate.one", "require a playable loop", '5')];
    let dossier = dossier(vec![digest('5')]);
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let input = promotion_input(&dossier, &candidates, &reviews);
    let first = evaluate_domain_pack_promotion(&input);
    let second = evaluate_domain_pack_promotion(&input);
    assert_eq!(first, second);
    assert_eq!(
        first.status,
        DomainPackPromotionReadinessStatus::ReadyForTrustedReview
    );
    assert!(first.issues.is_empty());
    assert_eq!(first.evaluation_digest.len(), 64);
}

#[test]
fn cross_version_candidate_and_exact_rejection_veto_promotion() {
    let mut wrong_version = candidate("candidate.one", "require a playable loop", '5');
    wrong_version
        .domain_pack_local_learning_candidate
        .target
        .base_version = Some("0.9.0".to_owned());
    let dossier = dossier(vec![digest('5')]);
    let mut rejecting = review("reviewer.reject", DomainPackReviewerRole::SafetyReviewer);
    rejecting.domain_pack_independent_review.decision = DomainPackReviewDecision::Reject;
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
        rejecting,
    ];

    let evaluation =
        evaluate_domain_pack_promotion(&promotion_input(&dossier, &[wrong_version], &reviews));
    assert_eq!(
        evaluation.status,
        DomainPackPromotionReadinessStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::CandidateTargetMismatch));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::ReviewRejected));
}

#[test]
fn chat_memory_or_authorship_cannot_bypass_durable_reviewed_evidence() {
    let dossier = dossier(vec![]);
    let evaluation = evaluate_domain_pack_promotion(&promotion_input(&dossier, &[], &[]));
    assert_eq!(
        evaluation.status,
        DomainPackPromotionReadinessStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| { issue.code == DomainPackLearningIssueCode::NonAuthoritativeBypass }));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| { issue.code == DomainPackLearningIssueCode::MissingIndependentReview }));
}

#[test]
fn stage_skip_and_no_op_ablation_fail_closed() {
    let candidates = vec![candidate("candidate.one", "require a playable loop", '5')];
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let mut dossier = dossier(vec![digest('5')]);
    dossier.domain_pack_promotion_dossier.transition.from = DomainPackPromotionStage::Trial;
    let run = &mut dossier.domain_pack_promotion_dossier.evaluator_runs[0];
    run.comparison.candidate_outcome_digest = run.comparison.baseline_outcome_digest.clone();
    let evaluation =
        evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews));
    assert!(evaluation.issues.iter().any(|issue| {
        issue.code == DomainPackLearningIssueCode::InvalidContract
            || issue.code == DomainPackLearningIssueCode::InvalidStageTransition
    }));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::NoOpComparison));
}

#[test]
fn hidden_gap_or_regression_blocks_even_when_verdict_says_improved() {
    let candidates = vec![candidate("candidate.one", "require a playable loop", '5')];
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let mut dossier = dossier(vec![digest('5')]);
    let comparison = &mut dossier.domain_pack_promotion_dossier.evaluator_runs[0].comparison;
    comparison
        .regression_finding_refs
        .push(sid("finding.hidden"));
    comparison.unknown_gap_refs.push(sid("gap.hidden"));
    let evaluation =
        evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::RegressionDetected));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::UnknownGap));
}

#[test]
fn non_independent_strong_judge_metadata_is_rejected() {
    let candidates = vec![candidate("candidate.one", "require a playable loop", '5')];
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let mut dossier = dossier(vec![digest('5')]);
    let run = &mut dossier.domain_pack_promotion_dossier.evaluator_runs[0];
    run.comparison.method = DomainPackLearningComparisonMethod::StrongJudge;
    run.evaluator_principal = principal("pack.author");
    run.strong_judge_proof = Some(DomainPackStrongJudgeProof {
        judge_principal: principal("pack.author"),
        independence_domain: sid("domain.same-author"),
        blind_ab: true,
        deterministic_order_digest: digest('1'),
        rubric_digest: digest('2'),
        model_digest: digest('3'),
        prompt_digest: digest('4'),
        input_digest: digest('5'),
        output_digest: digest('6'),
    });
    let evaluation =
        evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews));
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::NonIndependentJudge));
}

#[test]
fn contradictory_learning_emits_an_explicit_review_request() {
    let candidates = vec![
        candidate("candidate.one", "always require a playable loop", '5'),
        candidate("candidate.two", "never require a playable loop", '6'),
    ];
    let mut dossier = dossier(vec![digest('5'), digest('6')]);
    dossier
        .domain_pack_promotion_dossier
        .conflict_record_digests = vec![];
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let first = evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews));
    let second = evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews));
    assert_eq!(first, second);
    assert_eq!(
        first.status,
        DomainPackPromotionReadinessStatus::ReviewRequired
    );
    assert!(first.review_request.is_some());
    assert_eq!(first.detected_conflicts.len(), 1);
}

#[test]
fn exact_substantive_conflict_resolution_allows_promotion_to_continue() {
    let candidates = vec![
        candidate("candidate.one", "always require a playable loop", '5'),
        candidate("candidate.two", "never require a playable loop", '6'),
    ];
    let mut dossier = dossier(vec![digest('5'), digest('6')]);
    let reviews = vec![
        review("reviewer.one", DomainPackReviewerRole::DomainExpert),
        review("reviewer.two", DomainPackReviewerRole::EvidenceReviewer),
    ];
    let detected =
        evaluate_domain_pack_promotion(&promotion_input(&dossier, &candidates, &reviews))
            .detected_conflicts[0]
            .clone();
    let mut conflict = DomainPackLearningConflictDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_learning_conflict: DomainPackLearningConflict {
            conflict_id: sid("conflict.game-loop.1"),
            authority: DomainPackConflictAuthority::ConflictEvidenceOnly,
            target: candidates[0]
                .domain_pack_local_learning_candidate
                .target
                .clone(),
            kind: DomainPackLearningConflictKind::ContradictoryObservation,
            subject_digests: detected.candidate_digests,
            evidence_refs: vec![sid("evidence.conflict-resolution")],
            status: DomainPackLearningConflictStatus::Resolved,
            review_request_digest: Some(digest('a')),
            resolution: Some(DomainPackLearningConflictResolution {
                decision: DomainPackLearningConflictResolutionDecision::MergeWithQualification,
                rationale: "retain the rule only for first-playable readiness".to_owned(),
                evidence_refs: vec![sid("evidence.conflict-resolution")],
                resolved_by_review_digests: vec![digest('8'), digest('9')],
            }),
            conflict_digest: digest('0'),
        },
    };
    conflict.domain_pack_learning_conflict.conflict_digest =
        domain_pack_learning_conflict_digest(&conflict).unwrap();
    dossier
        .domain_pack_promotion_dossier
        .conflict_record_digests = vec![conflict
        .domain_pack_learning_conflict
        .conflict_digest
        .clone()];
    let conflicts = [conflict];
    let evaluation = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: &dossier,
        candidates: &candidates,
        independent_reviews: &reviews,
        conflicts: &conflicts,
    });

    assert_eq!(
        evaluation.status,
        DomainPackPromotionReadinessStatus::ReadyForTrustedReview
    );
    assert!(evaluation.review_request.is_none());
    assert!(evaluation.issues.is_empty(), "{:?}", evaluation.issues);

    let mut hollow_conflict = conflicts[0].clone();
    let hollow = hollow_conflict
        .domain_pack_learning_conflict
        .resolution
        .as_mut()
        .expect("resolved fixture");
    hollow.rationale.clear();
    hollow.evidence_refs.clear();
    hollow.resolved_by_review_digests.clear();
    let hollow_conflicts = [hollow_conflict];
    let hollow_evaluation = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: &dossier,
        candidates: &candidates,
        independent_reviews: &reviews,
        conflicts: &hollow_conflicts,
    });
    assert_eq!(
        hollow_evaluation.status,
        DomainPackPromotionReadinessStatus::Blocked
    );
    assert!(hollow_evaluation
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::InvalidContract));
}

fn snapshot_signature(
    id: &str,
    role: DomainPackReviewerRole,
) -> DomainPackReviewedRegistrySignature {
    DomainPackReviewedRegistrySignature {
        reviewer_id: principal(id),
        credential_id: sid(&format!("credential.{id}")),
        role,
        algorithm: DomainPackPromotionSignatureAlgorithm::Ed25519,
        payload_digest: digest('a'),
        signature: "deadbeef".to_owned(),
        signed_at_unix: 100,
    }
}

fn entry(version_value: &str, package_byte: char) -> DomainPackReviewedRegistryEntry {
    DomainPackReviewedRegistryEntry {
        pack: version(version_value),
        package_digest: supply_digest(package_byte),
        supply_chain_record_digest: supply_digest('1'),
        manifest_digest: supply_digest('2'),
        content_digest: supply_digest('3'),
        license_digest: supply_digest('4'),
        fixture_digests: vec![supply_digest('5')],
        stage: DomainPackPromotionStage::Reviewed,
        eligibility: DomainPackReviewedEligibility::EligibleReviewed,
        promotion_decision_digest: digest('6'),
        authorization_digest: digest('7'),
        independent_review_digests: vec![digest('8'), digest('9')],
        compatibility: DomainPackReviewedCompatibility {
            forge_core_requirement: ">=0.7.0, <0.8.0".to_owned(),
            pack_schema_requirement: "=0.1".to_owned(),
            evaluator_protocol_versions: vec!["1".to_owned()],
            predecessor_content_digests: vec![],
            breaking_change: false,
            migration_evidence_refs: vec![],
        },
        deprecation: None,
        revocation: None,
        supersession: None,
        entry_digest: digest('b'),
    }
}

fn registry(
    generation: u64,
    previous: Option<String>,
    entries: Vec<DomainPackReviewedRegistryEntry>,
    registry_byte: char,
) -> DomainPackReviewedRegistryDocument {
    DomainPackReviewedRegistryDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_reviewed_registry: DomainPackReviewedRegistry {
            registry_id: sid("registry.reviewed-domain-packs"),
            audience: "forge.domain-pack-resolution".to_owned(),
            generation,
            previous_registry_digest: previous,
            entries,
            snapshot_signatures: vec![
                snapshot_signature(
                    "registry.authorizer",
                    DomainPackReviewerRole::RegistryAuthorizer,
                ),
                snapshot_signature("domain.expert", DomainPackReviewerRole::DomainExpert),
            ],
            registry_digest: digest(registry_byte),
        },
    }
}

#[test]
fn append_only_successor_allows_new_reviewed_record_and_filters_activation() {
    let old_entry = entry("1.0.0", 'c');
    let current = registry(0, None, vec![old_entry.clone()], 'd');
    let mut deprecated = old_entry;
    deprecated.stage = DomainPackPromotionStage::Deprecated;
    deprecated.eligibility = DomainPackReviewedEligibility::IneligibleDeprecated;
    deprecated.deprecation = Some(DomainPackDeprecationBinding {
        reason: "supported migration".to_owned(),
        announced_at_unix: 100,
        removal_after_unix: Some(200),
    });
    deprecated.promotion_decision_digest = digest('e');
    deprecated.authorization_digest = digest('f');
    deprecated.entry_digest = digest('a');
    let new_entry = entry("2.0.0", 'e');
    let proposed = registry(1, Some(digest('d')), vec![deprecated, new_entry], 'e');
    let result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &proposed,
            competing_heads: &[],
        },
    );
    assert_eq!(
        result.status,
        DomainPackReviewedRegistryEvolutionStatus::AdmissibleCandidate
    );
    assert_eq!(result.eligible_for_new_activation.len(), 1);
    assert_eq!(result.eligible_for_new_activation[0].version, "2.0.0");
}

#[test]
fn same_digest_body_drift_and_same_version_package_equivocation_are_blocked() {
    let original = entry("1.0.0", 'c');
    let current = registry(0, None, vec![original.clone()], 'd');
    let mut drift = current.clone();
    drift.domain_pack_reviewed_registry.entries[0].content_digest = supply_digest('a');
    drift.domain_pack_reviewed_registry.entries[0].entry_digest = digest('c');
    let drift_result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &drift,
            competing_heads: &[],
        },
    );
    assert_eq!(
        drift_result.status,
        DomainPackReviewedRegistryEvolutionStatus::Blocked
    );
    assert!(drift_result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::RegistryEquivocation));

    let mut divergent = entry("1.0.0", 'e');
    divergent.entry_digest = digest('c');
    let equivocated = registry(0, None, vec![original, divergent], 'e');
    let equivocation_result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: None,
            proposed: &equivocated,
            competing_heads: &[],
        },
    );
    assert_eq!(
        equivocation_result.status,
        DomainPackReviewedRegistryEvolutionStatus::Blocked
    );
    assert!(equivocation_result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::RegistryEquivocation));
}

#[test]
fn removal_tombstone_and_terminal_revival_are_rejected() {
    let mut revoked = entry("1.0.0", 'c');
    revoked.stage = DomainPackPromotionStage::Revoked;
    revoked.eligibility = DomainPackReviewedEligibility::IneligibleRevoked;
    revoked.revocation = Some(DomainPackRevocationBinding {
        reason: "unsafe".to_owned(),
        effective_at_unix: 100,
        authorization_digest: digest('d'),
    });
    let current = registry(0, None, vec![revoked.clone()], 'd');

    let removed = registry(1, Some(digest('d')), vec![], 'e');
    let removal = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &removed,
            competing_heads: &[],
        },
    );
    assert!(removal
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::RegistryEntryRemoved));

    let tombstone = registry(
        1,
        Some(digest('d')),
        vec![
            {
                let mut value = entry("2.0.0", 'e');
                value.stage = DomainPackPromotionStage::Revoked;
                value.eligibility = DomainPackReviewedEligibility::IneligibleRevoked;
                value.revocation = Some(DomainPackRevocationBinding {
                    reason: "invented tombstone".to_owned(),
                    effective_at_unix: 110,
                    authorization_digest: digest('f'),
                });
                value
            },
            revoked.clone(),
        ],
        'e',
    );
    let tombstone_result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &tombstone,
            competing_heads: &[],
        },
    );
    assert!(tombstone_result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::InvalidRegistryStage));

    let mut revived = revoked;
    revived.stage = DomainPackPromotionStage::Reviewed;
    revived.eligibility = DomainPackReviewedEligibility::EligibleReviewed;
    revived.revocation = None;
    let revival = registry(1, Some(digest('d')), vec![revived], 'e');
    let revival_result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &revival,
            competing_heads: &[],
        },
    );
    assert!(revival_result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::TerminalReactivation));
}

#[test]
fn equivocation_and_unreviewed_supersession_target_block() {
    let original = entry("1.0.0", 'c');
    let current = registry(0, None, vec![original.clone()], 'd');
    let mut superseded = original;
    superseded.stage = DomainPackPromotionStage::Superseded;
    superseded.eligibility = DomainPackReviewedEligibility::IneligibleSuperseded;
    superseded.supersession = Some(DomainPackSupersessionBinding {
        replacement_pack: version("2.0.0"),
        replacement_package_digest: supply_digest('e'),
        authorization_digest: digest('f'),
    });
    superseded.promotion_decision_digest = digest('e');
    superseded.authorization_digest = digest('f');
    superseded.entry_digest = digest('a');
    let proposed = registry(1, Some(digest('d')), vec![superseded], 'e');
    let competing = registry(1, Some(digest('d')), vec![entry("2.0.0", 'e')], 'f');
    let result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&current),
            proposed: &proposed,
            competing_heads: &[competing],
        },
    );
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::RegistryEquivocation));
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningIssueCode::SupersessionTargetNotReviewed));
}

#[test]
#[allow(clippy::too_many_lines)]
fn exact_reviewed_resolution_join_accepts_complete_binding_and_rejects_revoked_or_tampered() {
    let mut reviewed_entry = entry("1.0.0", 'c');
    reviewed_entry.supply_chain_record_digest = supply_digest('d');
    let reviewed = registry(0, None, vec![reviewed_entry.clone()], 'e');
    let resolution = DomainPackResolutionProjectionDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_projection: DomainPackResolutionProjection {
            request_id: sid("resolution.join.1"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            status: DomainPackResolutionStatus::Resolved,
            selected: vec![DomainPackResolvedPackage {
                identity: DomainPackIdentity {
                    publisher: sid("forge.reference"),
                    name: sid("game-production"),
                    namespace: sid("game.production"),
                    version: "1.0.0".to_owned(),
                },
                package: DomainPackPackageBinding {
                    package_ref: RepoPath("packs/game-production.yaml".to_owned()),
                    package_digest: supply_digest('c'),
                    manifest: DomainPackArtifactBinding {
                        artifact_ref: RepoPath("packs/manifest.yaml".to_owned()),
                        raw_sha256: supply_digest('a'),
                        canonical_sha256: supply_digest('2'),
                    },
                    content: DomainPackContentBinding {
                        content_ref: RepoPath("packs/content.yaml".to_owned()),
                        raw_sha256: supply_digest('b'),
                        canonical_sha256: supply_digest('3'),
                    },
                    license: DomainPackArtifactBinding {
                        artifact_ref: RepoPath("packs/LICENSE".to_owned()),
                        raw_sha256: supply_digest('c'),
                        canonical_sha256: supply_digest('4'),
                    },
                    fixtures: vec![DomainPackArtifactBinding {
                        artifact_ref: RepoPath("packs/fixture.yaml".to_owned()),
                        raw_sha256: supply_digest('d'),
                        canonical_sha256: supply_digest('5'),
                    }],
                },
                registry_record_digest: supply_digest('d'),
                namespace_grant_id: sid("grant.game-production"),
                source_assurance: DomainPackSourceAssurance::ExplicitlyUntrusted,
                semantic_assurance: forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance::Unreviewed,
                reviewed_entry_digest: None,
                promotion_authorization_digest: None,
                dependencies: vec![],
                deterministic_order: 0,
            }],
            dependency_edges: vec![],
            rejected: vec![],
            issues: vec![],
            resolution_digest: supply_digest('f'),
        },
    };

    let accepted = join_reviewed_registry_to_resolution(&resolution, &reviewed);
    assert!(accepted.all_selected_eligible);
    assert_eq!(
        accepted.joins[0].status,
        forge_core_decisions::DomainPackReviewedResolutionJoinStatus::EligibleReviewed
    );

    let mut unreviewed_transitive = resolution.clone();
    let mut dependency = unreviewed_transitive
        .domain_pack_resolution_projection
        .selected[0]
        .clone();
    dependency.identity.name = sid("unreviewed-dependency");
    dependency.package.package_digest = supply_digest('9');
    dependency.registry_record_digest = supply_digest('8');
    dependency.deterministic_order = 1;
    unreviewed_transitive
        .domain_pack_resolution_projection
        .selected
        .push(dependency);
    let joined = join_reviewed_registry_to_resolution(&unreviewed_transitive, &reviewed);
    assert!(!joined.all_selected_eligible);
    assert!(joined.joins.iter().any(|join| {
        join.name == "unreviewed-dependency"
            && join.status
                == forge_core_decisions::DomainPackReviewedResolutionJoinStatus::MissingReviewedRecord
    }));

    let mut invalid_resolution = resolution.clone();
    invalid_resolution
        .domain_pack_resolution_projection
        .selected[0]
        .package
        .package_digest = digest('c');
    let invalid = join_reviewed_registry_to_resolution(&invalid_resolution, &reviewed);
    assert_eq!(
        invalid.joins[0].status,
        forge_core_decisions::DomainPackReviewedResolutionJoinStatus::InvalidContract
    );

    let mut empty_removal = resolution.clone();
    empty_removal
        .domain_pack_resolution_projection
        .selected
        .clear();
    let empty_join = join_reviewed_registry_to_resolution(&empty_removal, &reviewed);
    assert!(empty_join.joins.is_empty());
    assert!(!empty_join.all_selected_eligible);
    assert!(empty_removal.validate().is_empty());

    let mut revoked = reviewed_entry;
    revoked.stage = DomainPackPromotionStage::Revoked;
    revoked.eligibility = DomainPackReviewedEligibility::IneligibleRevoked;
    revoked.revocation = Some(DomainPackRevocationBinding {
        reason: "unsafe".to_owned(),
        effective_at_unix: 100,
        authorization_digest: digest('a'),
    });
    let revoked_registry = registry(0, None, vec![revoked], 'f');
    let denied = join_reviewed_registry_to_resolution(&resolution, &revoked_registry);
    assert!(!denied.all_selected_eligible);
    assert_eq!(
        denied.joins[0].status,
        forge_core_decisions::DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
    );

    let mut tampered_resolution = resolution;
    tampered_resolution
        .domain_pack_resolution_projection
        .selected[0]
        .package
        .license
        .canonical_sha256 = supply_digest('0');
    let tampered = join_reviewed_registry_to_resolution(&tampered_resolution, &reviewed);
    assert_eq!(
        tampered.joins[0].status,
        forge_core_decisions::DomainPackReviewedResolutionJoinStatus::ArtifactBindingMismatch
    );
}
