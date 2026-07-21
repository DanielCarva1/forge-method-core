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
use serde_json::json;

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn reference(id: &str, byte: char) -> WorkflowContentAddressedReference {
    WorkflowContentAddressedReference {
        subject_ref: id.to_owned(),
        subject_digest: digest(byte),
    }
}

fn release(id: &str, byte: char) -> WorkflowGovernanceReleaseIdentity {
    WorkflowGovernanceReleaseIdentity {
        lineage_id: StableId("lineage.product".to_owned()),
        release_id: StableId(id.to_owned()),
        release_version: "1.0.0".to_owned(),
        release_digest: digest(byte),
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
    let subject = release("release.current", 'a');
    let mut document = PostBuildVerifyEpisodeDocument {
        schema_version: POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION.to_owned(),
        post_build_verify_episode: PostBuildVerifyEpisode {
            episode_id: StableId("episode.release.current".to_owned()),
            generation: 1,
            previous_episode_digest: None,
            authority: PostBuildVerifyEpisodeAuthority::CandidateOnly,
            release_subject: subject.clone(),
            build_verify_snapshot: reference("build-verify/current", 'b'),
            rollback_baseline: PostBuildVerifyRollbackBaseline::PreviousRelease {
                release: release("release.previous", 'c'),
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
    let digest = document.episode_digest().expect("episode canonicalizes");
    document.post_build_verify_episode.episode_digest = digest;
}

#[test]
fn valid_episode_round_trips_and_validates() {
    let document = document();
    assert!(document.validate().is_empty());

    let encoded = serde_json::to_string(&document).expect("episode serializes");
    let decoded: PostBuildVerifyEpisodeDocument =
        serde_json::from_str(&encoded).expect("episode deserializes");
    assert_eq!(document, decoded);
}

#[test]
fn unknown_fields_and_self_digest_mutation_reject() {
    let document = document();
    let mut value = serde_json::to_value(&document).expect("episode converts");
    value["unexpected"] = json!(true);
    assert!(serde_json::from_value::<PostBuildVerifyEpisodeDocument>(value).is_err());

    let mut tampered = document;
    tampered
        .post_build_verify_episode
        .continuity
        .next_action_ref = StableId("action.rewritten".to_owned());
    assert!(tampered
        .validate()
        .iter()
        .any(|issue| issue.path == "episode.episode_digest"));
}

#[test]
fn rejects_misbound_release_baseline_policy_and_evolution_phase() {
    let mut episode = document();
    episode
        .post_build_verify_episode
        .feedback
        .push(PostBuildVerifyFeedback {
            feedback_id: StableId("feedback.cross-release".to_owned()),
            release_digest: digest('f'),
            feedback: reference("feedback/cross-release", '1'),
            status: PostBuildVerifyFeedbackStatus::Untriaged,
        });
    episode.post_build_verify_episode.rollback_baseline =
        PostBuildVerifyRollbackBaseline::PreviousRelease {
            release: episode.post_build_verify_episode.release_subject.clone(),
        };
    episode.post_build_verify_episode.policy_references.pop();
    episode
        .post_build_verify_episode
        .evolution
        .proposed_entry_phase = Phase::Evolve;
    rehash(&mut episode);

    let issues = episode.validate();
    assert!(issues
        .iter()
        .any(|issue| issue.path == "episode.feedback[0].release_digest"));
    assert!(issues
        .iter()
        .any(|issue| issue.path == "episode.rollback_baseline"));
    assert!(issues
        .iter()
        .any(|issue| issue.path == "episode.policy_references"));
    assert!(issues
        .iter()
        .any(|issue| issue.path == "episode.evolution.proposed_entry_phase"));
}

#[test]
fn all_observations_bind_the_exact_release_subject() {
    let mut episode = document();
    let release_digest = episode
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    episode
        .post_build_verify_episode
        .deployment_observations
        .push(PostBuildVerifyDeploymentObservation {
            observation_id: StableId("deployment.one".to_owned()),
            release_digest: release_digest.clone(),
            deployment: reference("deployment/one", '2'),
            outcome: PostBuildVerifyDeploymentOutcome::Healthy,
            observed_at_unix: 1,
        });
    episode.post_build_verify_episode.operational_evidence.push(
        PostBuildVerifyOperationalEvidence {
            evidence_id: StableId("evidence.one".to_owned()),
            release_digest: release_digest.clone(),
            evidence: reference("evidence/one", '3'),
            kind: PostBuildVerifyOperationalEvidenceKind::Availability,
            outcome: PostBuildVerifyOperationalEvidenceOutcome::SupportsReadiness,
            observed_at_unix: 2,
        },
    );
    episode
        .post_build_verify_episode
        .intake
        .push(PostBuildVerifyIntake {
            intake_id: StableId("incident.one".to_owned()),
            release_digest,
            report: reference("intake/one", '4'),
            kind: PostBuildVerifyIntakeKind::Incident,
            severity: PostBuildVerifyIntakeSeverity::High,
            status: PostBuildVerifyIntakeStatus::Resolved,
        });
    rehash(&mut episode);
    assert!(episode.validate().is_empty());
}

#[test]
fn bounded_feedback_collection_rejects_oversized_input() {
    let mut episode = document();
    let release_digest = episode
        .post_build_verify_episode
        .release_subject
        .release_digest
        .clone();
    for index in 0..65 {
        episode
            .post_build_verify_episode
            .feedback
            .push(PostBuildVerifyFeedback {
                feedback_id: StableId(format!("feedback.{index}")),
                release_digest: release_digest.clone(),
                feedback: reference(&format!("feedback/{index}"), '5'),
                status: PostBuildVerifyFeedbackStatus::Triaged,
            });
    }
    rehash(&mut episode);
    assert!(episode
        .validate()
        .iter()
        .any(|issue| issue.path == "episode.feedback"));
}
