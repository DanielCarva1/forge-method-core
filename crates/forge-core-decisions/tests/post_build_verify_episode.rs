use forge_core_contracts::{
    Phase, PostBuildVerifyContinuityBinding, PostBuildVerifyDeploymentObservation,
    PostBuildVerifyDeploymentOutcome, PostBuildVerifyEpisode, PostBuildVerifyEpisodeAuthority,
    PostBuildVerifyEpisodeDocument, PostBuildVerifyEvolutionIdentity,
    PostBuildVerifyEvolutionStatus, PostBuildVerifyEvolutionTrigger, PostBuildVerifyFeedback,
    PostBuildVerifyFeedbackStatus, PostBuildVerifyIntake, PostBuildVerifyIntakeKind,
    PostBuildVerifyIntakeSeverity, PostBuildVerifyIntakeStatus, PostBuildVerifyOperationalEvidence,
    PostBuildVerifyOperationalEvidenceKind, PostBuildVerifyOperationalEvidenceOutcome,
    PostBuildVerifyPolicyReference, PostBuildVerifyPolicyRole, PostBuildVerifyRollbackBaseline,
    RepoPath, StableId, WorkflowContentAddressedReference, WorkflowGovernanceReleaseIdentity,
    POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_post_build_verify_episode, route_post_build_verify_episode,
    verify_post_build_verify_episode_decision, PostBuildVerifyEpisodeDecisionAuthority,
    PostBuildVerifyEpisodeDecisionStatus, PostBuildVerifyEpisodeIssueCode,
    PostBuildVerifyEpisodeRuntimeRoute, PostBuildVerifyEpisodeRuntimeRouteError,
};

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn reference(id: &str, byte: char) -> WorkflowContentAddressedReference {
    WorkflowContentAddressedReference {
        subject_ref: id.to_owned(),
        subject_digest: digest(byte),
    }
}

fn policies() -> Vec<PostBuildVerifyPolicyReference> {
    [
        (PostBuildVerifyPolicyRole::Readiness, "release-readiness"),
        (PostBuildVerifyPolicyRole::ReadyRelease, "ready-release"),
        (
            PostBuildVerifyPolicyRole::RealityEvidence,
            "reality-evidence-gate",
        ),
        (
            PostBuildVerifyPolicyRole::ContextRecovery,
            "context-recovery",
        ),
        (PostBuildVerifyPolicyRole::EvolveProject, "evolve-project"),
    ]
    .into_iter()
    .map(|(role, id)| PostBuildVerifyPolicyReference {
        role,
        policy_id: StableId(id.to_owned()),
        policy_ref: RepoPath(format!("contracts/evidence/workflow-retirement/{id}.yaml")),
    })
    .collect()
}

fn document() -> PostBuildVerifyEpisodeDocument {
    let subject = WorkflowGovernanceReleaseIdentity {
        lineage_id: StableId("lineage.product".to_owned()),
        release_id: StableId("release.current".to_owned()),
        release_version: "1.0.0".to_owned(),
        release_digest: digest('a'),
    };
    let baseline = WorkflowGovernanceReleaseIdentity {
        lineage_id: StableId("lineage.product".to_owned()),
        release_id: StableId("release.previous".to_owned()),
        release_version: "0.9.0".to_owned(),
        release_digest: digest('b'),
    };
    let mut document = PostBuildVerifyEpisodeDocument {
        schema_version: POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION.to_owned(),
        post_build_verify_episode: PostBuildVerifyEpisode {
            episode_id: StableId("episode.release.current".to_owned()),
            generation: 1,
            previous_episode_digest: None,
            authority: PostBuildVerifyEpisodeAuthority::CandidateOnly,
            release_subject: subject.clone(),
            build_verify_snapshot: reference("build-verify/current", 'c'),
            rollback_baseline: PostBuildVerifyRollbackBaseline::PreviousRelease {
                release: baseline,
            },
            policy_references: policies(),
            deployment_observations: Vec::new(),
            operational_evidence: Vec::new(),
            feedback: Vec::new(),
            intake: Vec::new(),
            evolution: PostBuildVerifyEvolutionIdentity {
                evolution_episode_id: StableId("evolution.release.current".to_owned()),
                generation: 1,
                release_digest: subject.release_digest.clone(),
                status: PostBuildVerifyEvolutionStatus::Dormant,
                trigger: PostBuildVerifyEvolutionTrigger::PlannedFollowUp,
                proposed_entry_phase: Phase::Plan,
                continuity_subject: reference("continuity/evolution.current", 'd'),
            },
            continuity: PostBuildVerifyContinuityBinding {
                context_recovery_subject: reference("recovery/release.current", 'e'),
                next_action_ref: StableId("action.monitor-release".to_owned()),
            },
            episode_digest: String::new(),
        },
    };
    rehash(&mut document);
    document
}

fn rehash(document: &mut PostBuildVerifyEpisodeDocument) {
    document.post_build_verify_episode.episode_digest.clear();
    document.post_build_verify_episode.episode_digest =
        document.episode_digest().expect("episode canonicalizes");
}

fn observed_release(document: &mut PostBuildVerifyEpisodeDocument) {
    let release_digest = document
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    document
        .post_build_verify_episode
        .deployment_observations
        .push(PostBuildVerifyDeploymentObservation {
            observation_id: StableId("deployment.current".to_owned()),
            release_digest,
            deployment: reference("deployment/current", 'f'),
            outcome: PostBuildVerifyDeploymentOutcome::Healthy,
            observed_at_unix: 10,
        });
}

#[test]
fn absent_observation_requires_referenced_release_readiness() {
    let document = document();
    let decision = evaluate_post_build_verify_episode(&document);

    assert_eq!(
        decision.status,
        PostBuildVerifyEpisodeDecisionStatus::ReleaseReadinessRequired
    );
    assert_eq!(
        decision.required_policy_roles,
        vec![
            PostBuildVerifyPolicyRole::Readiness,
            PostBuildVerifyPolicyRole::ReadyRelease,
        ]
    );
    assert!(verify_post_build_verify_episode_decision(
        &document, &decision
    ));
}

#[test]
fn healthy_observed_release_is_operational_monitoring_only() {
    let mut document = document();
    observed_release(&mut document);
    rehash(&mut document);

    let decision = evaluate_post_build_verify_episode(&document);
    assert_eq!(
        decision.status,
        PostBuildVerifyEpisodeDecisionStatus::OperationalMonitoring
    );
    assert_eq!(
        decision.authority,
        PostBuildVerifyEpisodeDecisionAuthority::NonAuthoritative
    );
    assert_eq!(
        decision.required_policy_roles,
        vec![PostBuildVerifyPolicyRole::ContextRecovery]
    );
    assert!(verify_post_build_verify_episode_decision(
        &document, &decision
    ));
}

#[test]
fn feedback_and_unresolved_bug_require_evolution_triage() {
    let mut document = document();
    observed_release(&mut document);
    let release_digest = document
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    document
        .post_build_verify_episode
        .feedback
        .push(PostBuildVerifyFeedback {
            feedback_id: StableId("feedback.current".to_owned()),
            release_digest: release_digest.clone(),
            feedback: reference("feedback/current", '1'),
            status: PostBuildVerifyFeedbackStatus::Untriaged,
        });
    document
        .post_build_verify_episode
        .intake
        .push(PostBuildVerifyIntake {
            intake_id: StableId("bug.current".to_owned()),
            release_digest,
            report: reference("intake/bug.current", '2'),
            kind: PostBuildVerifyIntakeKind::Bug,
            severity: PostBuildVerifyIntakeSeverity::Medium,
            status: PostBuildVerifyIntakeStatus::Triaged,
        });
    rehash(&mut document);

    let decision = evaluate_post_build_verify_episode(&document);
    assert_eq!(
        decision.status,
        PostBuildVerifyEpisodeDecisionStatus::EvolutionTriageRequired
    );
    assert!(decision
        .issues
        .iter()
        .any(|issue| issue.code == PostBuildVerifyEpisodeIssueCode::UntriagedFeedback));
    assert!(decision
        .issues
        .iter()
        .any(|issue| { issue.code == PostBuildVerifyEpisodeIssueCode::UnresolvedIncidentOrBug }));
}

#[test]
fn readiness_disproof_has_precedence_over_feedback_triage() {
    let mut document = document();
    observed_release(&mut document);
    let release_digest = document
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    document
        .post_build_verify_episode
        .operational_evidence
        .push(PostBuildVerifyOperationalEvidence {
            evidence_id: StableId("evidence.disproof".to_owned()),
            release_digest: release_digest.clone(),
            evidence: reference("evidence/disproof", '3'),
            kind: PostBuildVerifyOperationalEvidenceKind::Safety,
            outcome: PostBuildVerifyOperationalEvidenceOutcome::DisprovesReadiness,
            observed_at_unix: 11,
        });
    document
        .post_build_verify_episode
        .feedback
        .push(PostBuildVerifyFeedback {
            feedback_id: StableId("feedback.current".to_owned()),
            release_digest,
            feedback: reference("feedback/current", '4'),
            status: PostBuildVerifyFeedbackStatus::Untriaged,
        });
    rehash(&mut document);

    let decision = evaluate_post_build_verify_episode(&document);
    assert_eq!(
        decision.status,
        PostBuildVerifyEpisodeDecisionStatus::RollbackAssessmentRequired
    );
    assert!(decision.issues.iter().any(|issue| {
        issue.code == PostBuildVerifyEpisodeIssueCode::OperationalReadinessDisproved
    }));
    assert!(!decision
        .issues
        .iter()
        .any(|issue| issue.code == PostBuildVerifyEpisodeIssueCode::UntriagedFeedback));
}

#[test]
fn runtime_routes_are_phase_bound_and_non_authoritative() {
    let document = document();
    assert_eq!(
        route_post_build_verify_episode(&document, Phase::BuildVerify),
        Ok(PostBuildVerifyEpisodeRuntimeRoute::AdvanceToReadyOperate)
    );
    assert_eq!(
        route_post_build_verify_episode(&document, Phase::ReadyOperate),
        Err(PostBuildVerifyEpisodeRuntimeRouteError::IncompatibleEvolutionIdentity)
    );
}

#[test]
fn operational_monitoring_routes_only_from_ready_operate_to_evolve() {
    let mut document = document();
    observed_release(&mut document);
    rehash(&mut document);

    assert_eq!(
        route_post_build_verify_episode(&document, Phase::ReadyOperate),
        Ok(PostBuildVerifyEpisodeRuntimeRoute::AdvanceToEvolve)
    );
    assert_eq!(
        route_post_build_verify_episode(&document, Phase::BuildVerify),
        Err(PostBuildVerifyEpisodeRuntimeRouteError::IncompatibleEvolutionIdentity)
    );
}

#[test]
fn rollback_and_bug_routes_open_follow_on_episodes_without_phase_advancement() {
    let mut rollback = document();
    observed_release(&mut rollback);
    let release_digest = rollback
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    rollback
        .post_build_verify_episode
        .operational_evidence
        .push(PostBuildVerifyOperationalEvidence {
            evidence_id: StableId("evidence.rollback".to_owned()),
            release_digest,
            evidence: reference("evidence/rollback", '6'),
            kind: PostBuildVerifyOperationalEvidenceKind::Safety,
            outcome: PostBuildVerifyOperationalEvidenceOutcome::DisprovesReadiness,
            observed_at_unix: 12,
        });
    rollback.post_build_verify_episode.evolution.status = PostBuildVerifyEvolutionStatus::Open;
    rollback.post_build_verify_episode.evolution.trigger =
        PostBuildVerifyEvolutionTrigger::ReadinessDisproof;
    rehash(&mut rollback);
    assert_eq!(
        route_post_build_verify_episode(&rollback, Phase::ReadyOperate),
        Ok(PostBuildVerifyEpisodeRuntimeRoute::OpenRollbackAssessment)
    );

    let mut bug = document();
    observed_release(&mut bug);
    let release_digest = bug
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    bug.post_build_verify_episode
        .intake
        .push(PostBuildVerifyIntake {
            intake_id: StableId("bug.follow-on".to_owned()),
            release_digest,
            report: reference("intake/bug.follow-on", '7'),
            kind: PostBuildVerifyIntakeKind::Bug,
            severity: PostBuildVerifyIntakeSeverity::High,
            status: PostBuildVerifyIntakeStatus::Untriaged,
        });
    bug.post_build_verify_episode.evolution.status = PostBuildVerifyEvolutionStatus::Open;
    bug.post_build_verify_episode.evolution.trigger = PostBuildVerifyEvolutionTrigger::Bug;
    rehash(&mut bug);
    assert_eq!(
        route_post_build_verify_episode(&bug, Phase::Evolve),
        Ok(PostBuildVerifyEpisodeRuntimeRoute::OpenEvolutionTriage)
    );
}

#[test]
fn malformed_record_blocks_without_phase_or_runtime_authority() {
    let mut document = document();
    document.post_build_verify_episode.evolution.release_digest = digest('9');
    rehash(&mut document);

    let decision = evaluate_post_build_verify_episode(&document);
    assert_eq!(
        decision.status,
        PostBuildVerifyEpisodeDecisionStatus::Blocked
    );
    assert_eq!(
        decision.required_policy_roles,
        vec![PostBuildVerifyPolicyRole::ContextRecovery]
    );
    assert!(decision
        .issues
        .iter()
        .all(|issue| issue.code == PostBuildVerifyEpisodeIssueCode::InvalidEpisode));
}
