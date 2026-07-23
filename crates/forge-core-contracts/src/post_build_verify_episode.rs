//! Candidate-only durable post-BuildVerify episode contracts.
//!
//! These values bind a release subject, its rollback baseline, and post-release
//! observations into one bounded, host-independent snapshot. They do not
//! advance a project phase, persist state, execute a rollback, or establish any
//! release, deployment, operational, or evolution authority.

use std::collections::BTreeSet;

use crate::common::{RepoPath, StableId};
use crate::phase::Phase;
use crate::workflow_governance::WorkflowContentAddressedReference;
use crate::workflow_release::WorkflowGovernanceReleaseIdentity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION: &str = "0.1";
pub const MAX_POST_BUILD_VERIFY_DEPLOYMENT_OBSERVATIONS: usize = 64;
pub const MAX_POST_BUILD_VERIFY_OPERATIONAL_EVIDENCE: usize = 64;
pub const MAX_POST_BUILD_VERIFY_FEEDBACK_ITEMS: usize = 64;
pub const MAX_POST_BUILD_VERIFY_INTAKE_ITEMS: usize = 64;
pub const POST_BUILD_VERIFY_POLICY_REFERENCE_COUNT: usize = 5;

/// A fully bounded snapshot of one durable release-to-evolution episode.
///
/// Every item binds the exact release digest so callers cannot combine evidence
/// or intake from different releases. `episode_digest` is a canonical JSON
/// digest of this full snapshot with that field omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisodeDocument {
    pub schema_version: String,
    pub post_build_verify_episode: PostBuildVerifyEpisode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisode {
    pub episode_id: StableId,
    /// Monotonic version of this durable episode snapshot. A later persistence
    /// owner must make the predecessor binding atomic; this contract does not.
    pub generation: u64,
    #[serde(default)]
    pub previous_episode_digest: Option<String>,
    pub authority: PostBuildVerifyEpisodeAuthority,
    /// Exact candidate release subject inherited from the established workflow
    /// release contract, rather than a copied release id or display version.
    pub release_subject: WorkflowGovernanceReleaseIdentity,
    /// Exact `BuildVerify` material that this release episode starts from.
    pub build_verify_snapshot: WorkflowContentAddressedReference,
    pub rollback_baseline: PostBuildVerifyRollbackBaseline,
    /// References to existing workflow policy records. This contract records
    /// their identities; it neither interprets nor replaces their semantics.
    pub policy_references: Vec<PostBuildVerifyPolicyReference>,
    pub deployment_observations: Vec<PostBuildVerifyDeploymentObservation>,
    pub operational_evidence: Vec<PostBuildVerifyOperationalEvidence>,
    pub feedback: Vec<PostBuildVerifyFeedback>,
    pub intake: Vec<PostBuildVerifyIntake>,
    pub evolution: PostBuildVerifyEvolutionIdentity,
    pub continuity: PostBuildVerifyContinuityBinding,
    pub episode_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEpisodeAuthority {
    /// Parsed state and observations are candidate evidence only. An authority
    /// owner must separately verify admission, deployment, and persistence.
    CandidateOnly,
}

/// An exact previous release or an exact `BuildVerify` snapshot that a later
/// trusted rollback owner may assess. This contract never performs a rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PostBuildVerifyRollbackBaseline {
    PreviousRelease {
        release: WorkflowGovernanceReleaseIdentity,
    },
    BuildVerifySnapshot {
        snapshot: WorkflowContentAddressedReference,
    },
}

/// Roles point at the established readiness and continuity workflows without
/// duplicating their policy prose or execution semantics.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyPolicyRole {
    Readiness,
    ReadyRelease,
    RealityEvidence,
    ContextRecovery,
    EvolveProject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyPolicyReference {
    pub role: PostBuildVerifyPolicyRole,
    pub policy_id: StableId,
    pub policy_ref: RepoPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyDeploymentObservation {
    pub observation_id: StableId,
    pub release_digest: String,
    pub deployment: WorkflowContentAddressedReference,
    pub outcome: PostBuildVerifyDeploymentOutcome,
    pub observed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyDeploymentOutcome {
    Healthy,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyOperationalEvidence {
    pub evidence_id: StableId,
    pub release_digest: String,
    pub evidence: WorkflowContentAddressedReference,
    pub kind: PostBuildVerifyOperationalEvidenceKind,
    pub outcome: PostBuildVerifyOperationalEvidenceOutcome,
    pub observed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyOperationalEvidenceKind {
    Availability,
    Safety,
    Security,
    Usage,
    Support,
    Verification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyOperationalEvidenceOutcome {
    SupportsReadiness,
    Inconclusive,
    DisprovesReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyFeedback {
    pub feedback_id: StableId,
    pub release_digest: String,
    pub feedback: WorkflowContentAddressedReference,
    pub status: PostBuildVerifyFeedbackStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyFeedbackStatus {
    Untriaged,
    Triaged,
    Resolved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyIntake {
    pub intake_id: StableId,
    pub release_digest: String,
    pub report: WorkflowContentAddressedReference,
    pub kind: PostBuildVerifyIntakeKind,
    pub severity: PostBuildVerifyIntakeSeverity,
    pub status: PostBuildVerifyIntakeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyIntakeKind {
    Incident,
    Bug,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyIntakeSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyIntakeStatus {
    Untriaged,
    Triaged,
    Resolved,
    Rejected,
}

/// Identity and safe-routing proposal for the follow-on evolution episode.
/// `proposed_entry_phase` is a recommendation only; it cannot change phase
/// state and intentionally excludes `ReadyOperate` and Evolve as entry targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEvolutionIdentity {
    pub evolution_episode_id: StableId,
    pub generation: u64,
    pub release_digest: String,
    pub status: PostBuildVerifyEvolutionStatus,
    pub trigger: PostBuildVerifyEvolutionTrigger,
    pub proposed_entry_phase: Phase,
    pub continuity_subject: WorkflowContentAddressedReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEvolutionStatus {
    Dormant,
    Open,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEvolutionTrigger {
    PlannedFollowUp,
    Feedback,
    Incident,
    Bug,
    ReadinessDisproof,
    RollbackAssessment,
}

/// Minimum durable continuation material for a later recovery owner. It is a
/// content-addressed handoff, never a Store key, process handle, or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyContinuityBinding {
    pub context_recovery_subject: WorkflowContentAddressedReference,
    pub next_action_ref: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostBuildVerifyEpisodeContractIssue {
    pub path: String,
    pub message: String,
}

impl PostBuildVerifyEpisodeDocument {
    /// Canonically hashes the full document after removing its self-digest.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical JSON encoding fails.
    pub fn episode_digest(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        value
            .get_mut("post_build_verify_episode")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|episode| episode.remove("episode_digest"))
            .ok_or_else(|| "episode digest field is absent".to_owned())?;
        let bytes = serde_json_canonicalizer::to_vec(&value).map_err(|error| error.to_string())?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }

    /// Validates this closed candidate-only record and its self-digest. This
    /// performs no signature, policy, host-time, deployment, or Store check.
    #[must_use]
    pub fn validate(&self) -> Vec<PostBuildVerifyEpisodeContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let episode = &self.post_build_verify_episode;
        require_nonblank(&mut issues, "episode.episode_id", &episode.episode_id.0);
        if episode.generation == 0 {
            issue(
                &mut issues,
                "episode.generation",
                "generation must be at least one",
            );
        }
        match (episode.generation, &episode.previous_episode_digest) {
            (1, Some(_)) => issue(
                &mut issues,
                "episode.previous_episode_digest",
                "initial generation must not name a predecessor digest",
            ),
            (generation, None) if generation > 1 => issue(
                &mut issues,
                "episode.previous_episode_digest",
                "later generation must bind the previous episode digest",
            ),
            (_, Some(digest)) if !valid_digest(digest) => issue(
                &mut issues,
                "episode.previous_episode_digest",
                "must be a sha256 digest",
            ),
            _ => {}
        }
        validate_release(
            &mut issues,
            "episode.release_subject",
            &episode.release_subject,
        );
        validate_reference(
            &mut issues,
            "episode.build_verify_snapshot",
            &episode.build_verify_snapshot,
        );
        validate_rollback_baseline(
            &mut issues,
            "episode.rollback_baseline",
            &episode.rollback_baseline,
            &episode.release_subject,
        );
        validate_policy_references(&mut issues, &episode.policy_references);
        validate_deployments(
            &mut issues,
            &episode.deployment_observations,
            &episode.release_subject.release_digest,
        );
        validate_operational_evidence(
            &mut issues,
            &episode.operational_evidence,
            &episode.release_subject.release_digest,
        );
        validate_feedback(
            &mut issues,
            &episode.feedback,
            &episode.release_subject.release_digest,
        );
        validate_intake(
            &mut issues,
            &episode.intake,
            &episode.release_subject.release_digest,
        );
        validate_evolution(
            &mut issues,
            &episode.evolution,
            &episode.release_subject.release_digest,
        );
        validate_reference(
            &mut issues,
            "episode.continuity.context_recovery_subject",
            &episode.continuity.context_recovery_subject,
        );
        require_nonblank(
            &mut issues,
            "episode.continuity.next_action_ref",
            &episode.continuity.next_action_ref.0,
        );
        if !valid_digest(&episode.episode_digest) {
            issue(
                &mut issues,
                "episode.episode_digest",
                "must be a sha256 digest",
            );
        } else if self.episode_digest().ok().as_deref() != Some(episode.episode_digest.as_str()) {
            issue(
                &mut issues,
                "episode.episode_digest",
                "does not match the canonical episode document",
            );
        }
        issues
    }
}

fn validate_release(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    release: &WorkflowGovernanceReleaseIdentity,
) {
    require_nonblank(issues, &format!("{path}.lineage_id"), &release.lineage_id.0);
    require_nonblank(issues, &format!("{path}.release_id"), &release.release_id.0);
    require_nonblank(
        issues,
        &format!("{path}.release_version"),
        &release.release_version,
    );
    require_digest(
        issues,
        &format!("{path}.release_digest"),
        &release.release_digest,
    );
}

fn validate_reference(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    reference: &WorkflowContentAddressedReference,
) {
    require_nonblank(
        issues,
        &format!("{path}.subject_ref"),
        &reference.subject_ref,
    );
    require_digest(
        issues,
        &format!("{path}.subject_digest"),
        &reference.subject_digest,
    );
}

fn validate_rollback_baseline(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    baseline: &PostBuildVerifyRollbackBaseline,
    subject: &WorkflowGovernanceReleaseIdentity,
) {
    match baseline {
        PostBuildVerifyRollbackBaseline::PreviousRelease { release } => {
            validate_release(issues, &format!("{path}.release"), release);
            if release.release_digest == subject.release_digest {
                issue(
                    issues,
                    path,
                    "previous release baseline must differ from the release subject",
                );
            }
        }
        PostBuildVerifyRollbackBaseline::BuildVerifySnapshot { snapshot } => {
            validate_reference(issues, &format!("{path}.snapshot"), snapshot);
        }
    }
}

fn validate_policy_references(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    references: &[PostBuildVerifyPolicyReference],
) {
    if references.len() != POST_BUILD_VERIFY_POLICY_REFERENCE_COUNT {
        issue(
            issues,
            "episode.policy_references",
            "must contain exactly one reference for every post-BuildVerify policy role",
        );
    }
    let mut roles = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (index, reference) in references.iter().enumerate() {
        let path = format!("episode.policy_references[{index}]");
        if !roles.insert(reference.role) {
            issue(
                issues,
                &format!("{path}.role"),
                "policy role must be unique",
            );
        }
        if !ids.insert(reference.policy_id.0.as_str()) {
            issue(
                issues,
                &format!("{path}.policy_id"),
                "policy id must be unique",
            );
        }
        if !paths.insert(reference.policy_ref.0.as_str()) {
            issue(
                issues,
                &format!("{path}.policy_ref"),
                "policy path must be unique",
            );
        }
        require_nonblank(issues, &format!("{path}.policy_id"), &reference.policy_id.0);
        require_nonblank(
            issues,
            &format!("{path}.policy_ref"),
            &reference.policy_ref.0,
        );
    }
    for role in [
        PostBuildVerifyPolicyRole::Readiness,
        PostBuildVerifyPolicyRole::ReadyRelease,
        PostBuildVerifyPolicyRole::RealityEvidence,
        PostBuildVerifyPolicyRole::ContextRecovery,
        PostBuildVerifyPolicyRole::EvolveProject,
    ] {
        if !roles.contains(&role) {
            issue(
                issues,
                "episode.policy_references",
                "required policy role is absent",
            );
        }
    }
}

fn validate_deployments(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    observations: &[PostBuildVerifyDeploymentObservation],
    release_digest: &str,
) {
    bounded(
        issues,
        "episode.deployment_observations",
        observations.len(),
        MAX_POST_BUILD_VERIFY_DEPLOYMENT_OBSERVATIONS,
    );
    let mut ids = BTreeSet::new();
    for (index, observation) in observations.iter().enumerate() {
        let path = format!("episode.deployment_observations[{index}]");
        require_nonblank(
            issues,
            &format!("{path}.observation_id"),
            &observation.observation_id.0,
        );
        if !ids.insert(observation.observation_id.0.as_str()) {
            issue(
                issues,
                &format!("{path}.observation_id"),
                "observation id must be unique",
            );
        }
        require_exact_release_digest(
            issues,
            &format!("{path}.release_digest"),
            &observation.release_digest,
            release_digest,
        );
        validate_reference(
            issues,
            &format!("{path}.deployment"),
            &observation.deployment,
        );
    }
}

fn validate_operational_evidence(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    evidence: &[PostBuildVerifyOperationalEvidence],
    release_digest: &str,
) {
    bounded(
        issues,
        "episode.operational_evidence",
        evidence.len(),
        MAX_POST_BUILD_VERIFY_OPERATIONAL_EVIDENCE,
    );
    let mut ids = BTreeSet::new();
    for (index, item) in evidence.iter().enumerate() {
        let path = format!("episode.operational_evidence[{index}]");
        require_nonblank(issues, &format!("{path}.evidence_id"), &item.evidence_id.0);
        if !ids.insert(item.evidence_id.0.as_str()) {
            issue(
                issues,
                &format!("{path}.evidence_id"),
                "evidence id must be unique",
            );
        }
        require_exact_release_digest(
            issues,
            &format!("{path}.release_digest"),
            &item.release_digest,
            release_digest,
        );
        validate_reference(issues, &format!("{path}.evidence"), &item.evidence);
    }
}

fn validate_feedback(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    feedback: &[PostBuildVerifyFeedback],
    release_digest: &str,
) {
    bounded(
        issues,
        "episode.feedback",
        feedback.len(),
        MAX_POST_BUILD_VERIFY_FEEDBACK_ITEMS,
    );
    let mut ids = BTreeSet::new();
    for (index, item) in feedback.iter().enumerate() {
        let path = format!("episode.feedback[{index}]");
        require_nonblank(issues, &format!("{path}.feedback_id"), &item.feedback_id.0);
        if !ids.insert(item.feedback_id.0.as_str()) {
            issue(
                issues,
                &format!("{path}.feedback_id"),
                "feedback id must be unique",
            );
        }
        require_exact_release_digest(
            issues,
            &format!("{path}.release_digest"),
            &item.release_digest,
            release_digest,
        );
        validate_reference(issues, &format!("{path}.feedback"), &item.feedback);
    }
}

fn validate_intake(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    intake: &[PostBuildVerifyIntake],
    release_digest: &str,
) {
    bounded(
        issues,
        "episode.intake",
        intake.len(),
        MAX_POST_BUILD_VERIFY_INTAKE_ITEMS,
    );
    let mut ids = BTreeSet::new();
    for (index, item) in intake.iter().enumerate() {
        let path = format!("episode.intake[{index}]");
        require_nonblank(issues, &format!("{path}.intake_id"), &item.intake_id.0);
        if !ids.insert(item.intake_id.0.as_str()) {
            issue(
                issues,
                &format!("{path}.intake_id"),
                "intake id must be unique",
            );
        }
        require_exact_release_digest(
            issues,
            &format!("{path}.release_digest"),
            &item.release_digest,
            release_digest,
        );
        validate_reference(issues, &format!("{path}.report"), &item.report);
    }
}

fn validate_evolution(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    evolution: &PostBuildVerifyEvolutionIdentity,
    release_digest: &str,
) {
    require_nonblank(
        issues,
        "episode.evolution.evolution_episode_id",
        &evolution.evolution_episode_id.0,
    );
    if evolution.generation == 0 {
        issue(
            issues,
            "episode.evolution.generation",
            "generation must be at least one",
        );
    }
    require_exact_release_digest(
        issues,
        "episode.evolution.release_digest",
        &evolution.release_digest,
        release_digest,
    );
    if matches!(
        evolution.proposed_entry_phase,
        Phase::Route | Phase::ReadyOperate | Phase::Evolve
    ) {
        issue(
            issues,
            "episode.evolution.proposed_entry_phase",
            "must recommend Discovery, Specification, Plan, or BuildVerify only",
        );
    }
    validate_reference(
        issues,
        "episode.evolution.continuity_subject",
        &evolution.continuity_subject,
    );
}

fn require_exact_release_digest(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    value: &str,
    expected: &str,
) {
    require_digest(issues, path, value);
    if value != expected {
        issue(issues, path, "must bind the exact release subject digest");
    }
}

fn bounded(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    actual: usize,
    maximum: usize,
) {
    if actual > maximum {
        issue(
            issues,
            path,
            &format!("must contain at most {maximum} entries"),
        );
    }
}

fn require_nonblank(
    issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>,
    path: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        issue(issues, path, "must not be blank");
    }
}

fn require_digest(issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>, path: &str, value: &str) {
    if !valid_digest(value) {
        issue(issues, path, "must be a sha256 digest");
    }
}

fn valid_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn issue(issues: &mut Vec<PostBuildVerifyEpisodeContractIssue>, path: &str, message: &str) {
    issues.push(PostBuildVerifyEpisodeContractIssue {
        path: path.to_owned(),
        message: message.to_owned(),
    });
}
