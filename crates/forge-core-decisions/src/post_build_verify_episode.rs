#![allow(clippy::missing_errors_doc)]

//! Pure C5.1 composition for durable post-BuildVerify episode snapshots.
//!
//! The evaluator only validates an authored candidate record and selects the
//! next referenced policy role. It never changes project phase, writes durable
//! state, admits a release, operates a deployment, performs a rollback, or
//! creates execution authority.

use forge_core_contracts::{
    Phase, PostBuildVerifyDeploymentOutcome, PostBuildVerifyEpisodeDocument,
    PostBuildVerifyEvolutionStatus, PostBuildVerifyEvolutionTrigger, PostBuildVerifyFeedbackStatus,
    PostBuildVerifyIntakeKind, PostBuildVerifyIntakeStatus,
    PostBuildVerifyOperationalEvidenceOutcome, PostBuildVerifyPolicyRole, StableId,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Pure outcome for a post-BuildVerify candidate snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEpisodeDecisionStatus {
    Blocked,
    ReleaseReadinessRequired,
    OperationalMonitoring,
    EvolutionTriageRequired,
    RollbackAssessmentRequired,
}

/// This evaluator only composes untrusted candidate records and established
/// policy references. It conveys no deployment, release, phase, or rollback
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEpisodeDecisionAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEpisodeIssueCode {
    InvalidEpisode,
    MissingDeploymentObservation,
    OperationalReadinessDisproved,
    UntriagedFeedback,
    UnresolvedIncidentOrBug,
    FailedDeployment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisodeIssue {
    pub code: PostBuildVerifyEpisodeIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisodeDecision {
    pub status: PostBuildVerifyEpisodeDecisionStatus,
    pub authority: PostBuildVerifyEpisodeDecisionAuthority,
    pub episode_id: StableId,
    pub generation: u64,
    pub release_digest: String,
    /// Existing workflow policy roles selected for the next owner. Selection is
    /// descriptive only; a caller must resolve and run the referenced policy.
    pub required_policy_roles: Vec<PostBuildVerifyPolicyRole>,
    pub issues: Vec<PostBuildVerifyEpisodeIssue>,
    pub decision_digest: String,
}

/// Validate and deterministically compose one durable post-BuildVerify episode.
///
/// The precedence is intentionally fail-closed: malformed state blocks first;
/// a failed deployment or operational evidence that disproves readiness requires
/// rollback assessment before evolution triage; unresolved intake and feedback
/// then require evolution triage; and only a valid episode with an observed
/// deployment reaches operational monitoring.
#[must_use]
pub fn evaluate_post_build_verify_episode(
    document: &PostBuildVerifyEpisodeDocument,
) -> PostBuildVerifyEpisodeDecision {
    let episode = &document.post_build_verify_episode;
    let mut issues = document
        .validate()
        .into_iter()
        .map(|issue| PostBuildVerifyEpisodeIssue {
            code: PostBuildVerifyEpisodeIssueCode::InvalidEpisode,
            path: issue.path,
            message: issue.message,
        })
        .collect::<Vec<_>>();

    let (status, required_policy_roles) = if !issues.is_empty() {
        (
            PostBuildVerifyEpisodeDecisionStatus::Blocked,
            vec![PostBuildVerifyPolicyRole::ContextRecovery],
        )
    } else if episode.deployment_observations.is_empty() {
        issues.push(decision_issue(
            PostBuildVerifyEpisodeIssueCode::MissingDeploymentObservation,
            "episode.deployment_observations",
            "an observed deployment is required before operational monitoring",
        ));
        (
            PostBuildVerifyEpisodeDecisionStatus::ReleaseReadinessRequired,
            vec![
                PostBuildVerifyPolicyRole::Readiness,
                PostBuildVerifyPolicyRole::ReadyRelease,
            ],
        )
    } else if episode
        .deployment_observations
        .iter()
        .any(|observation| observation.outcome == PostBuildVerifyDeploymentOutcome::Failed)
    {
        issues.push(decision_issue(
            PostBuildVerifyEpisodeIssueCode::FailedDeployment,
            "episode.deployment_observations",
            "a deployment failed and requires rollback-baseline assessment",
        ));
        (
            PostBuildVerifyEpisodeDecisionStatus::RollbackAssessmentRequired,
            vec![
                PostBuildVerifyPolicyRole::ContextRecovery,
                PostBuildVerifyPolicyRole::RealityEvidence,
            ],
        )
    } else if episode.operational_evidence.iter().any(|evidence| {
        evidence.outcome == PostBuildVerifyOperationalEvidenceOutcome::DisprovesReadiness
    }) {
        issues.push(decision_issue(
            PostBuildVerifyEpisodeIssueCode::OperationalReadinessDisproved,
            "episode.operational_evidence",
            "operational evidence disproves current readiness",
        ));
        (
            PostBuildVerifyEpisodeDecisionStatus::RollbackAssessmentRequired,
            vec![
                PostBuildVerifyPolicyRole::ContextRecovery,
                PostBuildVerifyPolicyRole::RealityEvidence,
            ],
        )
    } else {
        let untriaged_feedback = episode
            .feedback
            .iter()
            .any(|feedback| feedback.status == PostBuildVerifyFeedbackStatus::Untriaged);
        let unresolved_intake = episode.intake.iter().any(|item| {
            matches!(
                item.status,
                PostBuildVerifyIntakeStatus::Untriaged | PostBuildVerifyIntakeStatus::Triaged
            )
        });
        if untriaged_feedback {
            issues.push(decision_issue(
                PostBuildVerifyEpisodeIssueCode::UntriagedFeedback,
                "episode.feedback",
                "untriaged feedback requires evolution triage",
            ));
        }
        if unresolved_intake {
            issues.push(decision_issue(
                PostBuildVerifyEpisodeIssueCode::UnresolvedIncidentOrBug,
                "episode.intake",
                "unresolved incident or bug intake requires evolution triage",
            ));
        }
        if untriaged_feedback || unresolved_intake {
            (
                PostBuildVerifyEpisodeDecisionStatus::EvolutionTriageRequired,
                vec![
                    PostBuildVerifyPolicyRole::RealityEvidence,
                    PostBuildVerifyPolicyRole::EvolveProject,
                ],
            )
        } else {
            (
                PostBuildVerifyEpisodeDecisionStatus::OperationalMonitoring,
                vec![PostBuildVerifyPolicyRole::ContextRecovery],
            )
        }
    };

    let mut decision = PostBuildVerifyEpisodeDecision {
        status,
        authority: PostBuildVerifyEpisodeDecisionAuthority::NonAuthoritative,
        episode_id: episode.episode_id.clone(),
        generation: episode.generation,
        release_digest: episode.release_subject.release_digest.clone(),
        required_policy_roles,
        issues,
        decision_digest: String::new(),
    };
    decision.decision_digest = canonical_digest(&decision).unwrap_or_default();
    decision
}

/// Replays the pure evaluator to verify a persisted decision projection.
#[must_use]
pub fn verify_post_build_verify_episode_decision(
    document: &PostBuildVerifyEpisodeDocument,
    decision: &PostBuildVerifyEpisodeDecision,
) -> bool {
    evaluate_post_build_verify_episode(document) == *decision
}

/// Kernel-owned route derived from a valid candidate episode and its replayed
/// non-authoritative decision. The route still conveys no phase or ledger
/// authority; the kernel must bind it to the exact current project state and
/// admit the required workflow gate before committing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostBuildVerifyEpisodeRuntimeRoute {
    AdvanceToReadyOperate,
    AdvanceToEvolve,
    OpenRollbackAssessment,
    OpenEvolutionTriage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostBuildVerifyEpisodeRuntimeRouteError {
    InvalidCandidate,
    UnsupportedCurrentPhase,
    IncompatibleDecision,
    IncompatibleEvolutionIdentity,
}

/// Classifies the only C5.2 runtime routes permitted for a valid C5.1
/// candidate. Exact snapshot, release, ledger, generation, and gate checks stay
/// in the kernel authority boundary.
pub fn route_post_build_verify_episode(
    document: &PostBuildVerifyEpisodeDocument,
    current_phase: Phase,
) -> Result<PostBuildVerifyEpisodeRuntimeRoute, PostBuildVerifyEpisodeRuntimeRouteError> {
    let decision = evaluate_post_build_verify_episode(document);
    if decision.status == PostBuildVerifyEpisodeDecisionStatus::Blocked {
        return Err(PostBuildVerifyEpisodeRuntimeRouteError::InvalidCandidate);
    }

    let evolution = &document.post_build_verify_episode.evolution;
    match (current_phase, decision.status) {
        (Phase::BuildVerify, PostBuildVerifyEpisodeDecisionStatus::ReleaseReadinessRequired)
            if evolution.status == PostBuildVerifyEvolutionStatus::Dormant
                && evolution.trigger == PostBuildVerifyEvolutionTrigger::PlannedFollowUp =>
        {
            Ok(PostBuildVerifyEpisodeRuntimeRoute::AdvanceToReadyOperate)
        }
        (Phase::ReadyOperate, PostBuildVerifyEpisodeDecisionStatus::OperationalMonitoring)
            if evolution.status == PostBuildVerifyEvolutionStatus::Dormant
                && evolution.trigger == PostBuildVerifyEvolutionTrigger::PlannedFollowUp =>
        {
            Ok(PostBuildVerifyEpisodeRuntimeRoute::AdvanceToEvolve)
        }
        (
            Phase::ReadyOperate | Phase::Evolve,
            PostBuildVerifyEpisodeDecisionStatus::RollbackAssessmentRequired,
        ) if evolution.status == PostBuildVerifyEvolutionStatus::Open
            && matches!(
                evolution.trigger,
                PostBuildVerifyEvolutionTrigger::RollbackAssessment
                    | PostBuildVerifyEvolutionTrigger::ReadinessDisproof
            ) =>
        {
            Ok(PostBuildVerifyEpisodeRuntimeRoute::OpenRollbackAssessment)
        }
        (
            Phase::ReadyOperate | Phase::Evolve,
            PostBuildVerifyEpisodeDecisionStatus::EvolutionTriageRequired,
        ) if evolution.status == PostBuildVerifyEvolutionStatus::Open
            && evolution.trigger == expected_evolution_trigger(document) =>
        {
            Ok(PostBuildVerifyEpisodeRuntimeRoute::OpenEvolutionTriage)
        }
        (Phase::BuildVerify | Phase::ReadyOperate | Phase::Evolve, _) => {
            Err(PostBuildVerifyEpisodeRuntimeRouteError::IncompatibleEvolutionIdentity)
        }
        _ => Err(PostBuildVerifyEpisodeRuntimeRouteError::UnsupportedCurrentPhase),
    }
}

fn expected_evolution_trigger(
    document: &PostBuildVerifyEpisodeDocument,
) -> PostBuildVerifyEvolutionTrigger {
    let episode = &document.post_build_verify_episode;
    if episode.intake.iter().any(|item| {
        item.kind == PostBuildVerifyIntakeKind::Bug
            && matches!(
                item.status,
                PostBuildVerifyIntakeStatus::Untriaged | PostBuildVerifyIntakeStatus::Triaged
            )
    }) {
        PostBuildVerifyEvolutionTrigger::Bug
    } else if episode.intake.iter().any(|item| {
        item.kind == PostBuildVerifyIntakeKind::Incident
            && matches!(
                item.status,
                PostBuildVerifyIntakeStatus::Untriaged | PostBuildVerifyIntakeStatus::Triaged
            )
    }) {
        PostBuildVerifyEvolutionTrigger::Incident
    } else {
        PostBuildVerifyEvolutionTrigger::Feedback
    }
}

fn decision_issue(
    code: PostBuildVerifyEpisodeIssueCode,
    path: &str,
    message: &str,
) -> PostBuildVerifyEpisodeIssue {
    PostBuildVerifyEpisodeIssue {
        code,
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let mut value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let _ = value
        .as_object_mut()
        .and_then(|object| object.remove("decision_digest"));
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
