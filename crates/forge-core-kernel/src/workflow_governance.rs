//! Opaque authority boundary for workflow governance.
//!
//! The pure decisions crate accepts caller-authored YAML and therefore returns
//! only a [`WorkflowGovernanceSimulation`]. This module is the separate trusted
//! lane: only code inside the kernel can construct a
//! [`TrustedWorkflowGovernanceSnapshot`], and only such a snapshot can produce
//! a [`VerifiedWorkflowGovernanceDecision`] or consumable completion token.
//!
//! P5c will connect the private snapshot constructor to durable project-state,
//! capability, decision, and evaluator receipts. Until then the public API is
//! deliberately impossible to enter from YAML/JSON.

use forge_core_contracts::{
    CapabilityGap, DecisionRequest, ReadinessTarget, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument,
};
use forge_core_decisions::{
    simulate_workflow_governance, WorkflowCompletionVerdict, WorkflowEligibilityVerdict,
    WorkflowGovernanceRejection, WorkflowGovernanceSimulation, WorkflowGovernanceStatus,
    WorkflowProgressionVerdict,
};
use forge_core_store::sha256_content_hash;
use serde::Serialize;

mod adapter;
mod policy;

pub use adapter::*;
pub use policy::{
    load_admitted_workflow_governance_bundle, AdmittedWorkflowGovernanceBundle,
    AdmittedWorkflowGovernanceBundleError, ADMITTED_GOLDEN_PATH_BUNDLE_REF,
};

/// Snapshot admitted by trusted kernel-owned adapters.
///
/// This type intentionally has no public constructor and implements neither
/// `Clone`, `Serialize`, nor `Deserialize`.
///
/// ```compile_fail
/// use forge_core_kernel::TrustedWorkflowGovernanceSnapshot;
/// fn clone_snapshot(snapshot: TrustedWorkflowGovernanceSnapshot) {
///     let _copy = snapshot.clone();
/// }
/// ```
pub struct TrustedWorkflowGovernanceSnapshot {
    bundle: WorkflowGovernanceBundleDocument,
    evaluation: WorkflowGovernanceEvaluationDocument,
    snapshot_digest: String,
    policy_bundle_digest: String,
    project_id: String,
    source_id: String,
}

impl TrustedWorkflowGovernanceSnapshot {
    /// Kernel-only seam for P5c's trusted Project Snapshot Adapter.
    #[allow(dead_code)] // P5c's kernel-owned Project Snapshot Adapter will call this seam.
    pub(crate) fn from_trusted_parts(
        bundle: WorkflowGovernanceBundleDocument,
        evaluation: WorkflowGovernanceEvaluationDocument,
        snapshot_digest: String,
        project_id: String,
        source_id: String,
    ) -> Result<Self, TrustedWorkflowGovernanceSnapshotError> {
        if snapshot_digest.trim().is_empty() {
            return Err(TrustedWorkflowGovernanceSnapshotError::BlankSnapshotDigest);
        }
        if source_id.trim().is_empty() {
            return Err(TrustedWorkflowGovernanceSnapshotError::BlankSourceId);
        }
        if project_id.trim().is_empty() {
            return Err(TrustedWorkflowGovernanceSnapshotError::BlankProjectId);
        }
        let canonical_bundle = serde_json_canonicalizer::to_vec(&bundle).map_err(|_| {
            TrustedWorkflowGovernanceSnapshotError::PolicyBundleCanonicalizationFailed
        })?;
        let policy_bundle_digest = sha256_content_hash(&canonical_bundle);
        Ok(Self {
            bundle,
            evaluation,
            snapshot_digest,
            policy_bundle_digest,
            project_id,
            source_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedWorkflowGovernanceSnapshotError {
    BlankSnapshotDigest,
    BlankProjectId,
    BlankSourceId,
    PolicyBundleCanonicalizationFailed,
}

/// A verified decision that cannot be reconstructed from its audit projection.
///
/// It deliberately implements neither `Clone`, `Serialize`, nor `Deserialize`.
/// Future state transitions must consume this value or a token derived from it.
///
/// ```compile_fail
/// use forge_core_kernel::VerifiedWorkflowGovernanceDecision;
/// fn clone_decision(decision: VerifiedWorkflowGovernanceDecision) {
///     let _copy = decision.clone();
/// }
/// ```
///
/// ```compile_fail
/// use forge_core_kernel::VerifiedWorkflowGovernanceDecision;
/// let _: VerifiedWorkflowGovernanceDecision = serde_json::from_str("{}").unwrap();
/// ```
pub struct VerifiedWorkflowGovernanceDecision {
    simulation: WorkflowGovernanceSimulation,
    snapshot_digest: String,
    policy_bundle_digest: String,
    project_id: String,
    source_id: String,
}

impl VerifiedWorkflowGovernanceDecision {
    #[must_use]
    pub const fn status(&self) -> WorkflowGovernanceStatus {
        self.simulation.candidate_status
    }

    #[must_use]
    pub const fn eligibility(&self) -> WorkflowEligibilityVerdict {
        self.simulation.candidate_eligibility
    }

    #[must_use]
    pub const fn progression(&self) -> WorkflowProgressionVerdict {
        self.simulation.candidate_progression
    }

    #[must_use]
    pub const fn completion(&self) -> WorkflowCompletionVerdict {
        self.simulation.candidate_completion
    }

    #[must_use]
    pub const fn target(&self) -> ReadinessTarget {
        self.simulation.target
    }

    #[must_use]
    pub fn current_phase(&self) -> &str {
        &self.simulation.current_phase
    }

    #[must_use]
    pub const fn state_version(&self) -> u64 {
        self.simulation.state_version
    }

    #[must_use]
    pub fn capability_gaps(&self) -> &[CapabilityGap] {
        &self.simulation.candidate_capability_gaps
    }

    #[must_use]
    pub fn decision_requests(&self) -> &[DecisionRequest] {
        &self.simulation.candidate_decision_requests
    }

    /// Serializable evidence view. It is not, and cannot become, authority.
    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowGovernanceAudit<'_> {
        VerifiedWorkflowGovernanceAudit {
            authority: VerifiedWorkflowGovernanceAuditAuthority::VerifiedSnapshot,
            snapshot_digest: &self.snapshot_digest,
            policy_bundle_digest: &self.policy_bundle_digest,
            project_id: &self.project_id,
            source_id: &self.source_id,
            simulation: &self.simulation,
        }
    }

    /// Convert a verified complete decision into the one-use completion token.
    /// Returns the original opaque decision unchanged when completion is not
    /// verified, so callers cannot manufacture a token by retrying raw data.
    ///
    /// # Errors
    /// Returns the unchanged opaque decision in a box when governed completion
    /// is incomplete.
    pub fn try_into_completion(self) -> Result<VerifiedWorkflowGovernanceCompletion, Box<Self>> {
        if self.completion() == WorkflowCompletionVerdict::Complete {
            Ok(VerifiedWorkflowGovernanceCompletion { verified: self })
        } else {
            Err(Box::new(self))
        }
    }
}

/// One-use capability for a future governed completion transition.
///
/// No public fields, constructors, cloning, or serde implementations exist.
pub struct VerifiedWorkflowGovernanceCompletion {
    verified: VerifiedWorkflowGovernanceDecision,
}

impl VerifiedWorkflowGovernanceCompletion {
    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowGovernanceAudit<'_> {
        self.verified.audit()
    }

    #[must_use]
    pub const fn target(&self) -> ReadinessTarget {
        self.verified.target()
    }

    #[must_use]
    pub fn current_phase(&self) -> &str {
        self.verified.current_phase()
    }

    #[must_use]
    pub const fn state_version(&self) -> u64 {
        self.verified.state_version()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedWorkflowGovernanceAuditAuthority {
    VerifiedSnapshot,
}

/// Serializable observability projection. Deserializing this projection can
/// never recreate either opaque authority type.
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowGovernanceAudit<'a> {
    pub authority: VerifiedWorkflowGovernanceAuditAuthority,
    pub snapshot_digest: &'a str,
    pub policy_bundle_digest: &'a str,
    pub project_id: &'a str,
    pub source_id: &'a str,
    pub simulation: &'a WorkflowGovernanceSimulation,
}

/// Evaluate a policy from a kernel-admitted snapshot.
///
/// # Errors
/// Returns structural or semantic rejection from the shared deterministic
/// engine. Caller-authored documents cannot call this function because they
/// cannot construct `TrustedWorkflowGovernanceSnapshot`.
pub fn evaluate_verified_workflow_governance(
    snapshot: TrustedWorkflowGovernanceSnapshot,
) -> Result<VerifiedWorkflowGovernanceDecision, WorkflowGovernanceRejection> {
    let TrustedWorkflowGovernanceSnapshot {
        bundle,
        evaluation,
        snapshot_digest,
        policy_bundle_digest,
        project_id,
        source_id,
    } = snapshot;
    let simulation = simulate_workflow_governance(&bundle, &evaluation)?;
    Ok(VerifiedWorkflowGovernanceDecision {
        simulation,
        snapshot_digest,
        policy_bundle_digest,
        project_id,
        source_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn load_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
        yaml_serde::from_str(
            &std::fs::read_to_string(repo_root().join(relative)).expect("P5b fixture"),
        )
        .unwrap_or_else(|error| panic!("invalid fixture {relative}: {error}"))
    }

    #[test]
    fn only_a_trusted_snapshot_can_create_verified_completion_authority() {
        let bundle = load_yaml("contracts/workflow-governance/kernel-v0.yaml");
        let expected_policy_digest = sha256_content_hash(
            &serde_json_canonicalizer::to_vec(&bundle).expect("canonical bundle"),
        );
        let evaluation = load_yaml("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
        let snapshot = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            bundle,
            evaluation,
            "sha256:test-snapshot".to_owned(),
            "project.test".to_owned(),
            "test.project-snapshot-adapter".to_owned(),
        )
        .expect("trusted snapshot");
        let verified = evaluate_verified_workflow_governance(snapshot).expect("verified decision");

        assert_eq!(verified.status(), WorkflowGovernanceStatus::Complete);
        assert_eq!(verified.completion(), WorkflowCompletionVerdict::Complete);
        let completion = verified
            .try_into_completion()
            .map_err(|_| "expected completion token")
            .expect("completion token");
        let audit = serde_json::to_value(completion.audit()).expect("audit JSON");
        assert_eq!(audit["authority"], "verified_snapshot");
        assert_eq!(audit["project_id"], "project.test");
        assert_eq!(audit["policy_bundle_digest"], expected_policy_digest);
        assert_eq!(audit["simulation"]["authority"], "simulation_only");
        assert_eq!(audit["simulation"]["candidate_completion"], "complete");
        assert_eq!(audit["simulation"]["target"], "execute");
        assert_eq!(completion.target(), ReadinessTarget::Execute);
        assert_eq!(completion.current_phase(), "4-build-verify");
    }

    #[test]
    fn incomplete_verified_decision_cannot_mint_completion_token() {
        let bundle = load_yaml("contracts/workflow-governance/kernel-v0.yaml");
        let evaluation =
            load_yaml("docs/fixtures/workflow-governance-kernel-v0/missing-evidence.yaml");
        let snapshot = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            bundle,
            evaluation,
            "sha256:test-incomplete".to_owned(),
            "project.test".to_owned(),
            "test.project-snapshot-adapter".to_owned(),
        )
        .expect("trusted snapshot");
        let verified = evaluate_verified_workflow_governance(snapshot).expect("verified decision");

        assert!(verified.try_into_completion().is_err());
    }

    #[test]
    fn execute_scoped_completion_cannot_substitute_for_release_completion() {
        let mut bundle: WorkflowGovernanceBundleDocument =
            load_yaml("contracts/workflow-governance/kernel-v0.yaml");
        let policy = bundle
            .workflow_governance_bundle
            .policies
            .iter_mut()
            .find(|policy| policy.id.0 == "policy.workflow.build-story")
            .expect("build policy");
        policy.capability_requirements[0].blocks_before = ReadinessTarget::Release;

        let execute_evaluation =
            load_yaml("docs/fixtures/workflow-governance-kernel-v0/missing-capability.yaml");
        let execute_snapshot = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            bundle.clone(),
            execute_evaluation,
            "sha256:execute-snapshot".to_owned(),
            "project.test".to_owned(),
            "test.project-snapshot-adapter".to_owned(),
        )
        .expect("execute snapshot");
        let execute_completion = evaluate_verified_workflow_governance(execute_snapshot)
            .expect("execute decision")
            .try_into_completion()
            .map_err(|_| "execute target should be complete")
            .expect("execute completion");
        assert_eq!(execute_completion.target(), ReadinessTarget::Execute);

        let mut release_evaluation: WorkflowGovernanceEvaluationDocument =
            load_yaml("docs/fixtures/workflow-governance-kernel-v0/missing-capability.yaml");
        release_evaluation.workflow_governance_evaluation.target = ReadinessTarget::Release;
        let release_snapshot = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            bundle,
            release_evaluation,
            "sha256:release-snapshot".to_owned(),
            "project.test".to_owned(),
            "test.project-snapshot-adapter".to_owned(),
        )
        .expect("release snapshot");
        let release_decision =
            evaluate_verified_workflow_governance(release_snapshot).expect("release decision");
        assert_eq!(release_decision.target(), ReadinessTarget::Release);
        assert_eq!(
            release_decision.progression(),
            WorkflowProgressionVerdict::Blocked
        );
        assert!(release_decision.try_into_completion().is_err());
    }

    #[test]
    fn trusted_snapshot_rejects_blank_provenance() {
        let bundle = load_yaml("contracts/workflow-governance/kernel-v0.yaml");
        let evaluation = load_yaml("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
        assert!(matches!(
            TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
                bundle,
                evaluation,
                String::new(),
                "project.test".to_owned(),
                "adapter".to_owned(),
            ),
            Err(TrustedWorkflowGovernanceSnapshotError::BlankSnapshotDigest)
        ));
    }
}
