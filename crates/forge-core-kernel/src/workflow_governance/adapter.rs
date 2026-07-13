//! Live Project Snapshot Adapter for the agent-native governance lane.
//!
//! The adapter is deliberately opinionated: the admitted bundle is embedded,
//! the durable ledger owns phase/state/prerequisite authority, repository
//! observations are re-hashed, and callers never choose a workflow or target.

// Opaque authorization and completion capabilities are intentionally consumed
// by value so callers cannot reuse them after a durable transition.
#![allow(clippy::needless_pass_by_value)]

use super::{
    evaluate_verified_workflow_governance,
    load_admitted_workflow_governance_reviewed_release_registry, AdmittedWorkflowGovernanceRelease,
    AdmittedWorkflowGovernanceReleaseError, AdmittedWorkflowGovernanceReleaseRegistry,
    TrustedWorkflowGovernanceSnapshot, TrustedWorkflowGovernanceSnapshotError,
    VerifiedWorkflowGovernanceCompletion, VerifiedWorkflowGovernanceDecision,
};
use forge_core_authority::workflow_authority::{
    WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE, WORKFLOW_APPLICABILITY_EVALUATOR_REF,
    WORKFLOW_CAPABILITY_AUTHORITY_SCOPE,
};
use forge_core_authority::{
    AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry, PrincipalCredentialStatus,
    PrincipalRegistryDocument, VerifiedWorkflowApplicabilityAuthorization,
    VerifiedWorkflowCapabilityAuthorization, VerifiedWorkflowDecisionAuthorization,
    VerifiedWorkflowEvidenceAuthorization, VerifiedWorkflowSignalAuthorization,
    VerifiedWorkflowWaiverAuthorization, WorkflowWaiverSubject,
};
use forge_core_contracts::{
    ApplicabilityAssessedEvent, CapabilityProbedEvent, ContinuityRecordedEvent,
    DecisionResolvedEvent, EvaluatorObservedEvent, Phase, PhaseAdvancedEvent, PolicyCompletedEvent,
    PrincipalId, ProjectImportedEvent, ProjectLinkDocument, ReadinessTarget, ReleaseUpgradedEvent,
    SignalChangedEvent, StableId, WaiverAuthorizedEvent, WorkflowClaimWaiverObservation,
    WorkflowClaimWaiverPolicy, WorkflowCompletionAssertion, WorkflowContentAddressedReference,
    WorkflowEvidenceFreshness, WorkflowEvidenceObservation, WorkflowEvidenceProvenance,
    WorkflowEvidenceSubject, WorkflowEvidenceSubjectKind, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluation, WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent,
    WorkflowGovernanceLedgerRecord, WorkflowGovernancePolicy, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceSignal, WorkflowPolicyActivation, WorkflowPrerequisiteRequirement,
    WorkflowReceiptCarryover, WorkflowReleaseRegistryProvenance, WorkflowRuntimeBundleIdentity,
    PROJECT_LINK_FILE_NAME, PROJECT_LINK_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    find_entry, load_embedded_frozen_legacy_catalog, project_legacy_workflow_compatibility,
    simulate_workflow_governance, LegacyWorkflowGovernanceProjection, WorkflowGovernanceRejection,
    WorkflowGovernanceSimulation, WorkflowGovernanceStatus,
};
use forge_core_store::sha256_content_hash;
use forge_core_workflow_governance_tcb::{
    lock_workflow_governance_ledger_tcb, WorkflowGovernanceLedgerError,
    WorkflowGovernanceLedgerIdentity, WorkflowGovernanceLedgerProjection,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const INITIAL_PHASE: &str = "1-discovery";
const ADAPTER_SOURCE_ID: &str = "forge.kernel.project-snapshot-adapter.v0";
const MAX_SNAPSHOT_FILES: usize = 100_000;
const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const TRUSTED_WORKFLOW_REGISTRY_RELATIVE_PATH: &str = "operator/workflow-principal-registry.yaml";
const MAX_TRUSTED_REGISTRY_BYTES: u64 = 1024 * 1024;

/// Canonical project binding used by every live governance operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceProjectBinding {
    pub project_id: StableId,
    pub project_root: PathBuf,
    pub state_root: PathBuf,
}

/// Kernel-owned adapter. It is configured with a resolved project, not with a
/// workflow, bundle, phase, target, evidence result, or completion claim.
#[derive(Debug, Clone)]
pub struct WorkflowGovernanceProjectAdapter {
    binding: WorkflowGovernanceProjectBinding,
}

impl WorkflowGovernanceProjectAdapter {
    /// Bind an existing project and its `.forge-method` state root.
    ///
    /// # Errors
    /// Fails if either root is missing/non-directory, the id is blank, the
    /// state directory is not named `.forge-method`, or canonicalization fails.
    pub fn new(
        project_id: StableId,
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
    ) -> Result<Self, WorkflowGovernanceAdapterError> {
        if project_id.0.trim().is_empty() {
            return Err(WorkflowGovernanceAdapterError::InvalidProjectId);
        }
        let project_root = canonical_directory(project_root.as_ref(), "project_root")?;
        let state_root = canonical_directory(state_root.as_ref(), "state_root")?;
        if state_root.file_name().and_then(|value| value.to_str()) != Some(".forge-method") {
            return Err(WorkflowGovernanceAdapterError::InvalidStateRoot { path: state_root });
        }
        validate_project_state_binding(&project_id, &project_root, &state_root)?;
        Ok(Self {
            binding: WorkflowGovernanceProjectBinding {
                project_id,
                project_root,
                state_root,
            },
        })
    }

    #[must_use]
    pub const fn binding(&self) -> &WorkflowGovernanceProjectBinding {
        &self.binding
    }

    /// Create the first durable project-import receipt. The initial phase is a
    /// kernel constant; tolerant/hand-edited `state.yaml` is never imported as
    /// authority.
    ///
    /// # Errors
    /// Returns a typed binding, snapshot, ledger, policy, or persistence error.
    pub fn initialize(
        &self,
    ) -> Result<WorkflowGovernanceInitialization, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let genesis = registry.genesis();
        let identity = self.identity(genesis);
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        if !projection.records.is_empty() {
            let admitted = self.resolve_active_release(&registry, &projection)?;
            let active_identity = self.identity(admitted);
            validate_identity(&projection, &active_identity, &self.binding.project_root)?;
            return Ok(WorkflowGovernanceInitialization {
                status: WorkflowGovernanceInitializationStatus::AlreadyInitialized,
                project_id: self.binding.project_id.clone(),
                bundle_id: active_identity.bundle_id,
                bundle_digest: active_identity.bundle_digest,
                release: Self::release_audit(&registry, admitted, &projection),
                snapshot_digest,
                head_digest: projection
                    .head_digest
                    .clone()
                    .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?,
                state_version: projection.current_state_version().unwrap_or_default(),
                current_phase: current_phase(&projection)?.0,
            });
        }
        let event = WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
            source_ref: self.binding.project_root.display().to_string(),
            source_digest: snapshot_digest.clone(),
            snapshot_digest: snapshot_digest.clone(),
            initial_phase: StableId(INITIAL_PHASE.to_owned()),
        });
        let record = ledger.initialize_unchecked_tcb(&identity, 0, event)?;
        Ok(WorkflowGovernanceInitialization {
            status: WorkflowGovernanceInitializationStatus::Initialized,
            project_id: self.binding.project_id.clone(),
            bundle_id: identity.bundle_id,
            bundle_digest: identity.bundle_digest,
            release: Self::release_audit(&registry, genesis, &ledger.recover()?),
            snapshot_digest: snapshot_digest.clone(),
            head_digest: record.record_digest,
            state_version: record.state_version,
            current_phase: INITIAL_PHASE.to_owned(),
        })
    }

    /// Derive the next governed action. Workflow, phase, target, prerequisites,
    /// capabilities, evidence freshness, and completion are ledger/policy owned.
    ///
    /// # Errors
    /// Returns a typed error when binding, recovery, or policy evaluation fails.
    pub fn next(&self) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        self.guidance_from_projection(&registry, admitted, &projection, now)
    }

    /// Replacement-agent view. This is intentionally the same deterministic
    /// authority derivation as `next`; chat history is not an input.
    ///
    /// # Errors
    /// Returns a typed error when durable guidance cannot be reconstructed.
    pub fn resume(&self) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        self.next()
    }

    /// Return the exact durable release pin and the sole admitted adjacent
    /// successor, if one exists.
    ///
    /// # Errors
    /// Fails closed when the registry, ledger chain, project binding, or
    /// snapshot cannot be verified.
    pub fn release_status(
        &self,
    ) -> Result<WorkflowGovernanceReleaseStatus, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let active = self.resolve_active_release(&registry, &projection)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let head_digest = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let available = registry
            .adjacent_successor(active)
            .map(|successor| successor.release().clone());
        let upgrade_argv = available.as_ref().map(|target| {
            vec![
                "forge-core".to_owned(),
                "workflow".to_owned(),
                "release-upgrade".to_owned(),
                "--root".to_owned(),
                self.binding.project_root.display().to_string(),
                "--target-release-id".to_owned(),
                target.release_id.0.clone(),
                "--expected-current-release-digest".to_owned(),
                active.release().release_digest.clone(),
                "--expected-head-digest".to_owned(),
                head_digest.clone(),
                "--expected-snapshot-digest".to_owned(),
                snapshot_digest.clone(),
            ]
        });
        Ok(WorkflowGovernanceReleaseStatus {
            active: Self::release_audit(&registry, active, &projection),
            ledger_head_digest: head_digest,
            snapshot_digest,
            state_version: projection.current_state_version().unwrap_or_default(),
            available_successor: available,
            upgrade_argv,
        })
    }

    /// Atomically move a project pin to one exact adjacent admitted release.
    ///
    /// # Errors
    /// Rejects unknown, self, reverse, skipped, drifted, or stale-CAS requests
    /// without mutating the ledger. A replay of an already committed target is
    /// reported as `already_pinned` and appends nothing.
    pub fn release_upgrade(
        &self,
        target_release_id: &StableId,
        expected_current_release_digest: &str,
        expected_head_digest: &str,
        expected_snapshot_digest: &str,
    ) -> Result<WorkflowGovernanceReleaseUpgradeReceipt, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let source = self.resolve_active_release(&registry, &projection)?;
        let target = registry.release_by_id(target_release_id).ok_or_else(|| {
            WorkflowGovernanceAdapterError::UnknownRelease(target_release_id.0.clone())
        })?;

        if source.release().release_id == target.release().release_id
            && projection.records.iter().any(|record| {
                matches!(
                    &record.event,
                    WorkflowGovernanceEvent::ReleaseUpgraded(upgrade)
                        if upgrade.to_release.release_id == target.release().release_id
                )
            })
        {
            let replay_snapshot = project_snapshot_digest(&self.binding.project_root)?;
            return Self::release_upgrade_receipt(
                WorkflowGovernanceReleaseUpgradeStatus::AlreadyPinned,
                &registry,
                source,
                &projection,
                None,
                &replay_snapshot,
            );
        } else if source.release().release_id == target.release().release_id {
            return Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent);
        }
        if source.release().release_digest != expected_current_release_digest
            || projection.head_digest.as_deref() != Some(expected_head_digest)
        {
            return Err(WorkflowGovernanceAdapterError::ReleaseCasMismatch);
        }
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if snapshot_digest != expected_snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::ReleaseCasMismatch);
        }
        if !target.is_adjacent_successor_of(source) {
            return Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent);
        }
        if target.receipt_carryover() == WorkflowReceiptCarryover::PreservePolicyEquivalent
            && source.runtime_bundle().policy_set_digest
                != target.runtime_bundle().policy_set_digest
        {
            return Err(WorkflowGovernanceAdapterError::ReleasePolicyDrift);
        }
        let event = ReleaseUpgradedEvent {
            from_release: source.release().clone(),
            to_release: target.release().clone(),
            from_runtime_bundle: source.runtime_bundle().clone(),
            to_runtime_bundle: target.runtime_bundle().clone(),
            registry_provenance: registry.registry_provenance(),
            admission_proof: registry.admission_proof(source, target, &snapshot_digest)?,
            receipt_carryover: target.receipt_carryover(),
            prior_ledger_head_digest: expected_head_digest.to_owned(),
        };
        let source_identity = self.identity(source);
        let target_identity = self.identity(target);
        let state_version = projection
            .current_state_version()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        // The ledger lock serializes governance writers, not arbitrary project
        // editors. Narrow the filesystem TOCTOU window with a late snapshot
        // recheck immediately before the release transition.
        if project_snapshot_digest(&self.binding.project_root)? != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::ReleaseCasMismatch);
        }
        match ledger.transition_release_unchecked_tcb(
            expected_head_digest,
            &source_identity,
            &target_identity,
            state_version,
            event,
        ) {
            Ok(record) => {
                let committed = ledger.recover()?;
                let active = self.resolve_active_release(&registry, &committed)?;
                if active.release().release_id != target.release().release_id {
                    return Err(WorkflowGovernanceAdapterError::ReleaseCommitIndeterminate);
                }
                Self::release_upgrade_receipt(
                    WorkflowGovernanceReleaseUpgradeStatus::Upgraded,
                    &registry,
                    active,
                    &committed,
                    Some(record),
                    &snapshot_digest,
                )
            }
            Err(commit_error) => {
                // Replacement reconciliation runs as part of recovery under
                // the still-retained lock. Never report an ordinary failure if
                // the requested target is already the durable active release.
                let recovered = ledger.recover()?;
                let active = self.resolve_active_release(&registry, &recovered)?;
                if active.release().release_id == target.release().release_id {
                    let record = recovered
                        .records
                        .iter()
                        .rev()
                        .find(|record| {
                            matches!(
                                &record.event,
                                WorkflowGovernanceEvent::ReleaseUpgraded(upgrade)
                                    if upgrade.to_release.release_id == target.release().release_id
                            )
                        })
                        .cloned();
                    return Self::release_upgrade_receipt(
                        WorkflowGovernanceReleaseUpgradeStatus::Upgraded,
                        &registry,
                        active,
                        &recovered,
                        record,
                        &snapshot_digest,
                    );
                }
                if active.release().release_id == source.release().release_id {
                    return Err(WorkflowGovernanceAdapterError::Ledger(commit_error));
                }
                Err(WorkflowGovernanceAdapterError::ReleaseCommitIndeterminate)
            }
        }
    }

    /// Read-only migrated/legacy comparison for the exact same live snapshot.
    ///
    /// # Errors
    /// Returns a typed error when migrated or legacy projection cannot be read.
    pub fn shadow(&self) -> Result<WorkflowGovernanceShadowReport, WorkflowGovernanceAdapterError> {
        let guidance = self.next()?;
        // Shadow is an evidence-only comparison, never a routing or authority
        // surface. Retired workflows therefore resolve from the frozen P5d.5
        // subject while operational guidance remains bound to the separate
        // 68-entry catalog.
        let report = load_embedded_frozen_legacy_catalog();
        if !report.errors.is_empty() {
            return Err(WorkflowGovernanceAdapterError::EmbeddedCatalogInvalid);
        }
        let workflow_id = StableId(guidance.simulation.workflow_id.clone());
        let entry = find_entry(&report.catalog, &workflow_id).ok_or(
            WorkflowGovernanceAdapterError::LegacyWorkflowMissing(workflow_id.0),
        )?;
        let legacy = project_legacy_workflow_compatibility(&guidance.simulation, entry).map_err(
            |error| WorkflowGovernanceAdapterError::LegacyProjection(error.issue.message),
        )?;
        Ok(WorkflowGovernanceShadowReport {
            authority: WorkflowGovernanceShadowAuthority::ReadOnlyComparison,
            mutation_allowed: false,
            retirement_allowed: false,
            project_id: guidance.project_id.clone(),
            snapshot_digest: guidance.snapshot_digest.clone(),
            ledger_head_digest: guidance.ledger_head_digest.clone(),
            selected_policy_ref: guidance.selected_policy_ref.clone(),
            migrated: guidance,
            legacy,
        })
    }

    /// Consume a signed applicability authorization from the fixed operator
    /// trust root after re-hashing every confined basis artifact under lock.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, binding, basis, or ledger state.
    pub fn record_authorized_applicability(
        &self,
        authorization: VerifiedWorkflowApplicabilityAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
            || request.evaluator_ref.0 != WORKFLOW_APPLICABILITY_EVALUATOR_REF
            || request.authority_scope.0 != WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(admitted.document(), &request.policy_ref)?;
        self.require_active_policy(&registry, admitted, &projection, &request.policy_ref)?;
        if policy.routing.activation != WorkflowPolicyActivation::WhenApplicable {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let basis =
            content_addressed_basis_from_paths(&self.binding.project_root, &request.basis_refs)?;
        if content_addressed_basis_digest(&basis)? != request.basis_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let event = WorkflowGovernanceEvent::ApplicabilityAssessed(ApplicabilityAssessedEvent {
            policy_ref: request.policy_ref.clone(),
            applicable: request.applicable,
            assessed_by: audit.principal.principal_id,
            evaluator_ref: request.evaluator_ref.clone(),
            credential_id: StableId(audit.principal.credential_id),
            public_key_fingerprint: audit.principal.public_key_fingerprint,
            authorization_registry_digest: registry_digest,
            basis,
            basis_digest: request.basis_digest.clone(),
            snapshot_digest: snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
        });
        let mut batch = ledger.begin_unchecked_tcb_batch(head, &identity)?;
        let record = batch.push_event(request.state_version, event)?;
        if let Some((state_version, event)) =
            self.plan_phase_advance(admitted, batch.projection(), unix_time()?)?
        {
            batch.push_event(state_version, event)?;
        }
        if project_snapshot_digest(&self.binding.project_root)? != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        batch.commit()?;
        Ok(record)
    }

    /// Consume a signed capability observation from an authorized Runtime
    /// principal and bind it to the current snapshot and ledger head.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, binding, subject, or ledger state.
    pub fn record_authorized_capability(
        &self,
        authorization: VerifiedWorkflowCapabilityAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
            || request.authority_scope.0 != WORKFLOW_CAPABILITY_AUTHORITY_SCOPE
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(admitted.document(), &request.policy_ref)?;
        self.require_active_policy(&registry, admitted, &projection, &request.policy_ref)?;
        let requirement = policy
            .capability_requirements
            .iter()
            .find(|requirement| requirement.id == request.capability_ref)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownCapability(request.capability_ref.0.clone())
            })?;
        if requirement.probe_kind != request.probe_kind {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let subject = WorkflowEvidenceSubject {
            kind: request.subject_kind,
            subject_ref: request.subject_ref.clone(),
            subject_digest: request.subject_digest.clone(),
        };
        if !subject_current(&self.binding.project_root, &snapshot_digest, &subject)?
            && request.subject_kind == WorkflowEvidenceSubjectKind::Artifact
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let event = WorkflowGovernanceEvent::CapabilityProbed(CapabilityProbedEvent {
            policy_ref: request.policy_ref.clone(),
            capability_ref: request.capability_ref.clone(),
            probe_kind: request.probe_kind,
            credential_id: StableId(audit.principal.credential_id),
            public_key_fingerprint: audit.principal.public_key_fingerprint,
            authorization_registry_digest: registry_digest,
            available: request.available,
            probe_ref: request.probe_ref.clone(),
            probe_digest: request.probe_digest.clone(),
            subject,
            snapshot_digest: snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
        });
        Ok(ledger.append_unchecked_tcb_event(head, &identity, request.state_version, event)?)
    }

    /// Consume a signed evaluator evidence authorization after binding it
    /// to the current bundle, phase, state, target, evaluator, and subject.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, evidence, freshness, or binding.
    pub fn record_authorized_evidence(
        &self,
        authorization: VerifiedWorkflowEvidenceAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(admitted.document(), &request.policy_ref)?;
        let active_target =
            self.require_active_policy(&registry, admitted, &projection, &request.policy_ref)?;
        if request.readiness_target != active_target
            || request.readiness_target.rank() < policy.routing.readiness_target.rank()
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let claim = policy
            .claims
            .iter()
            .find(|claim| claim.id == request.claim_ref)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownClaim(request.claim_ref.0.clone())
            })?;
        let evaluator = policy
            .evaluators
            .iter()
            .find(|evaluator| evaluator.id == request.evaluator_ref)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownEvaluator(request.evaluator_ref.0.clone())
            })?;
        if claim.evaluator_ref != request.evaluator_ref
            || evaluator.provider != request.provider
            || !evaluator.accepted_evidence_kinds.contains(&request.kind)
            || request.strength < evaluator.minimum_strength
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let subject = WorkflowEvidenceSubject {
            kind: request.subject_kind,
            subject_ref: request.subject_ref.clone(),
            subject_digest: request.subject_digest.clone(),
        };
        if !subject_current(&self.binding.project_root, &snapshot_digest, &subject)? {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let maximum_expiry = request
            .observed_at_unix
            .checked_add(evaluator.max_age_seconds)
            .ok_or(WorkflowGovernanceAdapterError::ClockOverflow)?;
        if request
            .expires_at_unix
            .is_some_and(|expires| expires > maximum_expiry)
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let semantic_basis = serde_json::json!({
            "policy_ref": request.policy_ref,
            "claim_ref": request.claim_ref,
            "evaluator_ref": request.evaluator_ref,
            "provider": request.provider,
            "kind": request.kind,
            "strength": request.strength,
            "outcome": request.outcome,
            "subject_kind": request.subject_kind,
            "subject_digest": request.subject_digest,
            "scenario_digest": request.scenario_digest,
            "principal_id": audit.principal.principal_id,
        });
        let semantic_bytes = serde_json_canonicalizer::to_vec(&semantic_basis)
            .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
        let semantic_digest = sha256_content_hash(&semantic_bytes);
        let event = WorkflowGovernanceEvent::EvaluatorObserved(EvaluatorObservedEvent {
            policy_ref: request.policy_ref.clone(),
            claim_ref: request.claim_ref.clone(),
            evaluator_ref: request.evaluator_ref.clone(),
            provider: request.provider,
            credential_id: StableId(audit.principal.credential_id.clone()),
            public_key_fingerprint: audit.principal.public_key_fingerprint.clone(),
            authorization_registry_digest: registry_digest,
            kind: request.kind,
            strength: request.strength,
            outcome: request.outcome,
            provenance: WorkflowEvidenceProvenance {
                source_ref: request.subject_ref.clone(),
                source_digest: request.subject_digest.clone(),
                scenario_digest: request.scenario_digest.clone(),
                semantic_identity: StableId(format!(
                    "evidence.semantic.{}",
                    semantic_digest.trim_start_matches("sha256:")
                )),
                producer_ref: audit.principal.agent_id,
                principal: Some(audit.principal.principal_id),
                method: format!(
                    "registry_authorized_evidence:{}:{}",
                    audit.intent_digest, audit.signature_fingerprint
                ),
            },
            subject,
            snapshot_digest,
            ledger_head_digest: head.to_owned(),
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
        });
        Ok(ledger.append_unchecked_tcb_event(head, &identity, request.state_version, event)?)
    }

    /// Consume an opaque, registry-verified human decision into the ledger.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, alternative, consequences, or binding.
    pub fn record_authorized_decision(
        &self,
        authorization: VerifiedWorkflowDecisionAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(admitted.document(), &request.policy_ref)?;
        self.require_active_policy(&registry, admitted, &projection, &request.policy_ref)?;
        let rule = policy
            .decision_rules
            .iter()
            .find(|rule| rule.id == request.decision_ref)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownDecision(request.decision_ref.0.clone())
            })?;
        let selected_alternative = rule
            .alternatives
            .iter()
            .find(|candidate| candidate.id == request.selected_alternative_ref)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        let expected_consequences_digest = sha256_content_hash(
            &serde_json_canonicalizer::to_vec(&selected_alternative.consequences).map_err(
                |error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()),
            )?,
        );
        if request.readiness_target != readiness_name(policy.routing.readiness_target)
            || request.consequences_ack_digest != expected_consequences_digest
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let event = WorkflowGovernanceEvent::DecisionResolved(DecisionResolvedEvent {
            policy_ref: request.policy_ref.clone(),
            decision_ref: request.decision_ref.clone(),
            selected_alternative_ref: request.selected_alternative_ref.clone(),
            principal: audit.principal.principal_id,
            authority_scope: StableId("workflow.decision.resolve".to_owned()),
            credential_id: StableId(audit.principal.credential_id),
            public_key_fingerprint: audit.principal.public_key_fingerprint,
            authorization_registry_digest: registry_digest,
            snapshot_digest,
            ledger_head_digest: head.to_owned(),
            authorization_intent_digest: audit.intent_digest,
            signature_fingerprint: audit.signature_fingerprint,
            resolved_at_unix: unix_time()?,
        });
        Ok(ledger.append_unchecked_tcb_event(head, &identity, request.state_version, event)?)
    }

    /// Consume an opaque, registry-verified claim waiver into the ledger after
    /// rechecking policy scope, target, expiry, phase, state, and bundle digest.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, scope, expiry, or binding.
    pub fn record_authorized_waiver(
        &self,
        authorization: VerifiedWorkflowWaiverAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let claim_ref = match &request.subject {
            WorkflowWaiverSubject::Claim { claim_ref } => claim_ref,
            WorkflowWaiverSubject::Obligation { .. } => {
                return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                    "P5c waivers are claim-scoped".to_owned(),
                ));
            }
        };
        let policy = policy_by_id(admitted.document(), &request.policy_ref)?;
        self.require_active_policy(&registry, admitted, &projection, &request.policy_ref)?;
        let claim = policy
            .claims
            .iter()
            .find(|claim| claim.id == *claim_ref)
            .ok_or_else(|| WorkflowGovernanceAdapterError::UnknownClaim(claim_ref.0.clone()))?;
        let WorkflowClaimWaiverPolicy::Authorized {
            max_target,
            authority_scope,
            max_age_seconds,
        } = &claim.waiver
        else {
            return Err(WorkflowGovernanceAdapterError::WaiverNotAllowed);
        };
        let requested_target = parse_readiness(&request.maximum_readiness_target)?;
        let now = unix_time()?;
        let max_expiry = now
            .checked_add(*max_age_seconds)
            .ok_or(WorkflowGovernanceAdapterError::ClockOverflow)?;
        if requested_target.rank() > max_target.rank()
            || request.expires_at_unix < 0
            || u64::try_from(request.expires_at_unix).unwrap_or(u64::MAX) > max_expiry
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let event = WorkflowGovernanceEvent::WaiverAuthorized(WaiverAuthorizedEvent {
            policy_ref: request.policy_ref.clone(),
            claim_ref: claim_ref.clone(),
            principal: audit.principal.principal_id,
            authority_scope: authority_scope.clone(),
            credential_id: StableId(audit.principal.credential_id),
            public_key_fingerprint: audit.principal.public_key_fingerprint,
            authorization_registry_digest: registry_digest,
            max_target: requested_target,
            subject: WorkflowEvidenceSubject {
                kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                subject_ref: self.binding.project_id.0.clone(),
                subject_digest: snapshot_digest,
            },
            snapshot_digest: request.snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            authorization_intent_digest: audit.intent_digest,
            signature_fingerprint: audit.signature_fingerprint,
            consequences_digest: request.consequences_ack_digest.clone(),
            authorized_at_unix: now,
            expires_at_unix: u64::try_from(request.expires_at_unix)
                .map_err(|_| WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?,
        });
        Ok(ledger.append_unchecked_tcb_event(head, &identity, request.state_version, event)?)
    }

    /// Consume one signed, closed governance-signal transition. The adapter
    /// owns monotonic episode/generation semantics and re-hashes every basis
    /// reference before appending the event.
    ///
    /// # Errors
    /// Returns a typed error for invalid authority, episode, basis, or binding.
    pub fn record_authorized_signal(
        &self,
        authorization: VerifiedWorkflowSignalAuthorization,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        let request = authorization.request();
        let phase = current_phase(&projection)?;
        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        if request.project_id != self.binding.project_id
            || request.policy_bundle_digest != admitted.digest()
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let basis =
            content_addressed_basis_from_paths(&self.binding.project_root, &request.basis_refs)?;
        if content_addressed_basis_digest(&basis)? != request.basis_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let prior = projection.records.iter().rev().find_map(|record| {
            if let WorkflowGovernanceEvent::SignalChanged(event) = &record.event {
                (event.signal == request.signal).then_some(event)
            } else {
                None
            }
        });
        let transition_valid = match prior {
            None => request.active && request.generation == 1,
            Some(previous) if previous.active => {
                !request.active
                    && request.generation == previous.generation
                    && request.episode_id == previous.episode_id
            }
            Some(previous) => {
                request.active
                    && request.generation == previous.generation.saturating_add(1)
                    && request.episode_id != previous.episode_id
            }
        };
        if !transition_valid {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let audit = authorization.audit();
        let registry_digest = self.validate_trusted_principal(&audit.principal)?;
        let event = WorkflowGovernanceEvent::SignalChanged(SignalChangedEvent {
            signal: request.signal,
            active: request.active,
            episode_id: request.episode_id.clone(),
            generation: request.generation,
            changed_by: audit.principal.principal_id,
            credential_id: StableId(audit.principal.credential_id),
            public_key_fingerprint: audit.principal.public_key_fingerprint,
            authorization_registry_digest: registry_digest,
            basis,
            basis_digest: request.basis_digest.clone(),
            snapshot_digest: snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
        });
        let mut batch = ledger.begin_unchecked_tcb_batch(head, &identity)?;
        let record = batch.push_event(request.state_version, event)?;
        if let Some((state_version, event)) =
            self.plan_phase_advance(admitted, batch.projection(), unix_time()?)?
        {
            batch.push_event(state_version, event)?;
        }
        if project_snapshot_digest(&self.binding.project_root)? != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        batch.commit()?;
        Ok(record)
    }

    /// Convert currently verified completion into a one-use late-recheck token.
    ///
    /// # Errors
    /// Returns a typed error when current guidance is not exactly completable.
    pub fn prepare_completion(
        &self,
    ) -> Result<PreparedWorkflowGovernanceCompletion, WorkflowGovernanceAdapterError> {
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let (guidance, verified) =
            self.verified_from_projection(&registry, admitted, &projection, now)?;
        if guidance.status == WorkflowGovernanceGuidanceStatus::PhaseComplete {
            return Err(WorkflowGovernanceAdapterError::PolicyAlreadyCompleted);
        }
        if guidance.status != WorkflowGovernanceGuidanceStatus::ReadyToComplete {
            return Err(WorkflowGovernanceAdapterError::PolicyIncomplete);
        }
        let completion = verified
            .try_into_completion()
            .map_err(|_| WorkflowGovernanceAdapterError::PolicyIncomplete)?;
        Ok(PreparedWorkflowGovernanceCompletion {
            completion,
            project_id: guidance.project_id,
            policy_ref: guidance.selected_policy_ref,
            bundle_digest: guidance.bundle_digest,
            snapshot_digest: guidance.snapshot_digest,
            ledger_head_digest: guidance.ledger_head_digest,
            state_version: guidance.state_version,
            current_phase: guidance.current_phase,
            target: guidance.target,
        })
    }

    /// Consume completion only after a fresh project snapshot, ledger head,
    /// phase/state, admitted bundle, selected policy, target, and evidence
    /// evaluation all match the prepared authority under one ledger lock.
    ///
    /// # Errors
    /// Returns a typed error when any late-bound condition drifted or persistence fails.
    pub fn consume_completion(
        &self,
        prepared: PreparedWorkflowGovernanceCompletion,
        continuity_principal: PrincipalId,
    ) -> Result<WorkflowGovernanceCompletionReceipt, WorkflowGovernanceAdapterError> {
        if continuity_principal.0.trim().is_empty() {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                "continuity principal must not be blank".to_owned(),
            ));
        }
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        let (fresh, verified) =
            self.verified_from_projection(&registry, admitted, &projection, now)?;
        if fresh.status != WorkflowGovernanceGuidanceStatus::ReadyToComplete {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        let _fresh_completion = verified
            .try_into_completion()
            .map_err(|_| WorkflowGovernanceAdapterError::CompletionDrift)?;
        if prepared.project_id != fresh.project_id
            || prepared.policy_ref != fresh.selected_policy_ref
            || prepared.bundle_digest != fresh.bundle_digest
            || prepared.snapshot_digest != fresh.snapshot_digest
            || prepared.ledger_head_digest != fresh.ledger_head_digest
            || prepared.state_version != fresh.state_version
            || prepared.current_phase != fresh.current_phase
            || prepared.target != fresh.target
            || prepared.completion.target() != fresh.target
        {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        let completed_state_version = fresh
            .state_version
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let completed_policy = policy_by_id(admitted.document(), &fresh.selected_policy_ref)?;
        let prerequisite_refs = completed_policy
            .prerequisites
            .iter()
            .map(|prerequisite| &prerequisite.policy_ref)
            .collect::<BTreeSet<_>>();
        let mut dependency_receipt_digests = projection
            .records
            .iter()
            .filter_map(|record| match &record.event {
                WorkflowGovernanceEvent::PolicyCompleted(event)
                    if prerequisite_refs.contains(&event.policy_ref) =>
                {
                    Some(record.record_digest.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if completed_policy.routing.activation == WorkflowPolicyActivation::OnSignal {
            let registry_digest = self.current_trusted_registry_digest()?;
            let current_receipts = derive_receipts(
                admitted.document(),
                &projection,
                &self.binding.project_root,
                &fresh.snapshot_digest,
                now,
                registry_digest.as_deref(),
            )?;
            for signal in &completed_policy.routing.signals {
                if let Some(digest) = current_receipts.active_signal_receipt_digests.get(signal) {
                    dependency_receipt_digests.push(digest.clone());
                }
            }
        }
        let evidence_receipt_digests = projection
            .records
            .iter()
            .filter_map(|record| match &record.event {
                WorkflowGovernanceEvent::EvaluatorObserved(event)
                    if event.policy_ref == fresh.selected_policy_ref =>
                {
                    Some(record.record_digest.clone())
                }
                WorkflowGovernanceEvent::WaiverAuthorized(event)
                    if event.policy_ref == fresh.selected_policy_ref =>
                {
                    Some(record.record_digest.clone())
                }
                _ => None,
            })
            .collect();
        let unresolved_deferred_obligation_refs = completed_policy
            .obligations
            .iter()
            .filter(|obligation| obligation.required_before.rank() > fresh.target.rank())
            .map(|obligation| obligation.id.clone())
            .collect();
        let unresolved_deferred_capability_refs = completed_policy
            .capability_requirements
            .iter()
            .filter(|capability| capability.blocks_before.rank() > fresh.target.rank())
            .map(|capability| capability.id.clone())
            .collect();
        let event = WorkflowGovernanceEvent::PolicyCompleted(PolicyCompletedEvent {
            policy_ref: fresh.selected_policy_ref.clone(),
            target: fresh.target,
            phase: StableId(fresh.current_phase.clone()),
            snapshot_digest: fresh.snapshot_digest.clone(),
            ledger_head_digest: fresh.ledger_head_digest.clone(),
            subject: WorkflowEvidenceSubject {
                kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                subject_ref: self.binding.project_id.0.clone(),
                subject_digest: fresh.snapshot_digest.clone(),
            },
            dependency_receipt_digests,
            evidence_receipt_digests,
            unresolved_deferred_obligation_refs,
            unresolved_deferred_capability_refs,
            completed_at_unix: now,
        });
        // The ledger lock serializes governance writers, not arbitrary project
        // editors. Re-hash immediately before append to narrow that TOCTOU
        // window; this is a drift check, not a claim of filesystem atomicity.
        if project_snapshot_digest(&self.binding.project_root)? != fresh.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        let mut batch = ledger.begin_unchecked_tcb_batch(&fresh.ledger_head_digest, &identity)?;
        let completed = batch.push_event(completed_state_version, event)?;
        let phase_advanced = if let Some((state_version, event)) =
            self.plan_phase_advance(admitted, batch.projection(), now)?
        {
            Some(batch.push_event(state_version, event)?)
        } else {
            None
        };
        let next_guidance =
            self.guidance_from_projection(&registry, admitted, batch.projection(), now)?;
        let continuity_event =
            WorkflowGovernanceEvent::ContinuityRecorded(ContinuityRecordedEvent {
                from_principal: None,
                to_principal: continuity_principal,
                snapshot_digest: fresh.snapshot_digest,
                context_digest: sha256_content_hash(
                    &serde_json_canonicalizer::to_vec(&next_guidance).map_err(|error| {
                        WorkflowGovernanceAdapterError::Canonicalization(error.to_string())
                    })?,
                ),
                next_policy_ref: next_guidance.selected_policy_ref.clone(),
                next_action: next_guidance
                    .simulation
                    .candidate_next_actions
                    .first()
                    .map_or_else(
                        || "inspect governed state".to_owned(),
                        |action| action.description.clone(),
                    ),
                continuity_at_unix: now,
            });
        let continuity_state = batch
            .projection()
            .current_state_version()
            .unwrap_or(completed_state_version);
        let continuity = batch.push_event(continuity_state, continuity_event)?;
        let next = self.guidance_from_projection(&registry, admitted, batch.projection(), now)?;
        batch.commit()?;
        Ok(WorkflowGovernanceCompletionReceipt {
            authority: WorkflowGovernanceCompletionAuthority::ConsumedAfterLateRecheck,
            completed_record: completed,
            phase_advanced_record: phase_advanced,
            continuity_record: continuity,
            next,
        })
    }

    fn resolve_active_release<'a>(
        &self,
        registry: &'a AdmittedWorkflowGovernanceReleaseRegistry,
        projection: &WorkflowGovernanceLedgerProjection,
    ) -> Result<&'a AdmittedWorkflowGovernanceRelease, WorkflowGovernanceAdapterError> {
        let genesis = registry.genesis();
        let expected_genesis = self.identity(genesis);
        if projection.genesis_identity().as_ref() != Some(&expected_genesis) {
            return Err(WorkflowGovernanceAdapterError::LedgerIdentityMismatch);
        }
        let mut active = genesis;
        for record in &projection.records {
            let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event else {
                continue;
            };
            if event.from_release != *active.release()
                || event.from_runtime_bundle != *active.runtime_bundle()
                || event.registry_provenance.registry_id
                    != registry.registry_provenance().registry_id
                || event.prior_ledger_head_digest
                    != record.previous_record_digest.clone().unwrap_or_default()
            {
                return Err(WorkflowGovernanceAdapterError::ReleaseChainInvalid);
            }
            let target = registry
                .release_by_id(&event.to_release.release_id)
                .ok_or(WorkflowGovernanceAdapterError::ReleaseChainInvalid)?;
            if !target.is_adjacent_successor_of(active)
                || event.to_release != *target.release()
                || event.to_runtime_bundle != *target.runtime_bundle()
                || event.receipt_carryover != target.receipt_carryover()
                || event.admission_proof
                    != AdmittedWorkflowGovernanceReleaseRegistry::admission_proof_with_provenance(
                        &event.registry_provenance,
                        active,
                        target,
                        &event.admission_proof.snapshot_digest,
                    )?
            {
                return Err(WorkflowGovernanceAdapterError::ReleaseChainInvalid);
            }
            active = target;
        }
        if projection.active_identity().as_ref() != Some(&self.identity(active)) {
            return Err(WorkflowGovernanceAdapterError::LedgerIdentityMismatch);
        }
        Ok(active)
    }

    fn release_audit(
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
    ) -> WorkflowGovernanceReleaseAudit {
        let transition_provenance = projection.records.iter().rev().find_map(|record| {
            if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
                (event.to_release.release_id == admitted.release().release_id)
                    .then(|| event.registry_provenance.clone())
            } else {
                None
            }
        });
        WorkflowGovernanceReleaseAudit {
            release: admitted.release().clone(),
            runtime_bundle: admitted.runtime_bundle().clone(),
            registry: transition_provenance.unwrap_or_else(|| registry.registry_provenance()),
            pin_origin: if projection
                .records
                .iter()
                .any(|record| matches!(record.event, WorkflowGovernanceEvent::ReleaseUpgraded(_)))
            {
                WorkflowGovernanceReleasePinOrigin::LedgerTransition
            } else {
                WorkflowGovernanceReleasePinOrigin::ImplicitP5cGenesis
            },
        }
    }

    fn release_upgrade_receipt(
        status: WorkflowGovernanceReleaseUpgradeStatus,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        active: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
        transition_record: Option<WorkflowGovernanceLedgerRecord>,
        snapshot_digest: &str,
    ) -> Result<WorkflowGovernanceReleaseUpgradeReceipt, WorkflowGovernanceAdapterError> {
        Ok(WorkflowGovernanceReleaseUpgradeReceipt {
            status,
            active: Self::release_audit(registry, active, projection),
            transition_record,
            ledger_head_digest: projection
                .head_digest
                .clone()
                .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?,
            snapshot_digest: snapshot_digest.to_owned(),
            state_version: projection.current_state_version().unwrap_or_default(),
        })
    }

    fn identity(
        &self,
        admitted: &AdmittedWorkflowGovernanceRelease,
    ) -> WorkflowGovernanceLedgerIdentity {
        WorkflowGovernanceLedgerIdentity {
            project_id: self.binding.project_id.clone(),
            bundle_id: admitted.runtime_bundle().bundle_id.clone(),
            bundle_digest: admitted.digest().to_owned(),
        }
    }

    /// Fixed operator registry path used by every workflow authorization.
    /// It is derived from the resolved sidecar, never selected per operation.
    #[must_use]
    pub fn trusted_principal_registry_path(&self) -> PathBuf {
        if self
            .binding
            .state_root
            .starts_with(&self.binding.project_root)
        {
            let project_registry_key = format!(
                "project-{:x}",
                Sha256::digest(self.binding.project_id.0.as_bytes())
            );
            return self
                .binding
                .project_root
                .parent()
                .unwrap_or(&self.binding.project_root)
                .join(".forge-method-operator")
                .join(project_registry_key)
                .join("workflow-principal-registry.yaml");
        }
        self.binding
            .state_root
            .parent()
            .unwrap_or(&self.binding.state_root)
            .join(TRUSTED_WORKFLOW_REGISTRY_RELATIVE_PATH)
    }

    fn validate_trusted_principal(
        &self,
        principal: &AuthorizedPrincipalAudit,
    ) -> Result<String, WorkflowGovernanceAdapterError> {
        let path = self.trusted_principal_registry_path();
        let metadata = fs::metadata(&path).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot stat {}: {error}", path.display()),
            }
        })?;
        if metadata.len() > MAX_TRUSTED_REGISTRY_BYTES {
            return Err(WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!(
                    "{} exceeds {} bytes",
                    path.display(),
                    MAX_TRUSTED_REGISTRY_BYTES
                ),
            });
        }
        let raw = fs::read_to_string(&path).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot read {}: {error}", path.display()),
            }
        })?;
        let document: PrincipalRegistryDocument = yaml_serde::from_str(&raw).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot parse {}: {error}", path.display()),
            }
        })?;
        AuthorizedPrincipalRegistry::from_document(document.clone()).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("{} is invalid: {error}", path.display()),
            }
        })?;
        let entry = document
            .principal_registry
            .principals
            .iter()
            .find(|entry| entry.credential_id == principal.credential_id)
            .ok_or_else(|| WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!(
                    "credential {} is absent from fixed registry",
                    principal.credential_id
                ),
            })?;
        let expected_fingerprint = format!(
            "sha256:{:x}",
            Sha256::digest(entry.public_key_hex.to_ascii_lowercase().as_bytes())
        );
        if entry.status != PrincipalCredentialStatus::Active
            || entry.principal_id != principal.principal_id
            || entry.agent_id != principal.agent_id
            || entry.role != principal.role
            || document.principal_registry.audience != principal.audience
            || !entry.allowed_tools.iter().any(|tool| tool.0 == "workflow")
            || entry.authority_grants != principal.authority_grants
            || expected_fingerprint != principal.public_key_fingerprint
        {
            return Err(WorkflowGovernanceAdapterError::TrustedRegistry {
                source: "verified authorization principal does not match the fixed active registry"
                    .to_owned(),
            });
        }
        let canonical = serde_json_canonicalizer::to_vec(&document)
            .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
        Ok(sha256_content_hash(&canonical))
    }

    fn current_trusted_registry_digest(
        &self,
    ) -> Result<Option<String>, WorkflowGovernanceAdapterError> {
        let path = self.trusted_principal_registry_path();
        if !path.exists() {
            return Ok(None);
        }
        let metadata = fs::metadata(&path).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot stat {}: {error}", path.display()),
            }
        })?;
        if metadata.len() > MAX_TRUSTED_REGISTRY_BYTES {
            return Err(WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!(
                    "{} exceeds {} bytes",
                    path.display(),
                    MAX_TRUSTED_REGISTRY_BYTES
                ),
            });
        }
        let raw = fs::read_to_string(&path).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot read {}: {error}", path.display()),
            }
        })?;
        let document: PrincipalRegistryDocument = yaml_serde::from_str(&raw).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("cannot parse {}: {error}", path.display()),
            }
        })?;
        AuthorizedPrincipalRegistry::from_document(document.clone()).map_err(|error| {
            WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("{} is invalid: {error}", path.display()),
            }
        })?;
        let canonical = serde_json_canonicalizer::to_vec(&document)
            .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
        Ok(Some(sha256_content_hash(&canonical)))
    }

    fn guidance_from_projection(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
    ) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        self.verified_from_projection(registry, admitted, projection, now)
            .map(|(guidance, _)| guidance)
    }

    fn verified_from_projection(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
    ) -> Result<
        (
            WorkflowGovernanceGuidance,
            VerifiedWorkflowGovernanceDecision,
        ),
        WorkflowGovernanceAdapterError,
    > {
        let identity = self.identity(admitted);
        validate_identity(projection, &identity, &self.binding.project_root)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let trusted_registry_digest = self.current_trusted_registry_digest()?;
        let derived = derive_receipts(
            admitted.document(),
            projection,
            &self.binding.project_root,
            &snapshot_digest,
            now,
            trusted_registry_digest.as_deref(),
        )?;
        let phase = current_phase(projection)?;
        let selected = select_policy(admitted.document(), &derived, &phase)?;
        let evaluation_phase =
            if selected.eligible_phases.iter().any(|tag| {
                Phase::tag_eligible(&tag.0, Phase::parse(&phase.0).expect("parsed phase"))
            }) {
                phase.clone()
            } else {
                selected
                    .eligible_phases
                    .iter()
                    .find(|tag| Phase::parse(&tag.0).is_some())
                    .cloned()
                    .ok_or_else(|| WorkflowGovernanceAdapterError::InvalidPhase(phase.0.clone()))?
            };
        let selected_already_completed = derived.completed_policy_refs.contains(&selected.id);
        let boundary_rechecks = boundary_rechecks(
            admitted.document(),
            &derived,
            projection.current_state_version().unwrap_or_default(),
            now,
            selected.routing.readiness_target,
        )?;
        let evaluation = WorkflowGovernanceEvaluationDocument {
            schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
            workflow_governance_evaluation: WorkflowGovernanceEvaluation {
                observation_set_id: StableId(format!(
                    "observation.ledger-{}",
                    projection.next_sequence
                )),
                state_version: projection.current_state_version().unwrap_or_default(),
                observed_at_unix: now,
                bundle_id: identity.bundle_id.clone(),
                policy_id: selected.id.clone(),
                current_phase: evaluation_phase,
                target: selected.routing.readiness_target,
                completed_policy_refs: derived.completed_policy_refs.iter().cloned().collect(),
                not_applicable_policy_refs: derived
                    .not_applicable_policy_refs
                    .iter()
                    .cloned()
                    .collect(),
                available_capability_refs: derived
                    .available_capability_refs
                    .iter()
                    .filter(|capability| {
                        selected
                            .capability_requirements
                            .iter()
                            .any(|requirement| &requirement.id == *capability)
                    })
                    .cloned()
                    .collect(),
                decision_need_refs: derived
                    .decision_need_refs
                    .iter()
                    .filter(|decision| {
                        selected
                            .decision_rules
                            .iter()
                            .any(|rule| &rule.id == *decision)
                    })
                    .cloned()
                    .collect(),
                resolved_decision_refs: derived
                    .resolved_decision_refs
                    .iter()
                    .filter(|decision| {
                        selected
                            .decision_rules
                            .iter()
                            .any(|rule| &rule.id == *decision)
                    })
                    .cloned()
                    .collect(),
                waivers: derived
                    .waivers
                    .into_iter()
                    .filter(|waiver| {
                        selected
                            .claims
                            .iter()
                            .any(|claim| claim.id == waiver.claim_ref)
                    })
                    .collect(),
                evidence: derived
                    .evidence
                    .into_iter()
                    .filter(|evidence| {
                        selected.claims.iter().any(|claim| {
                            claim.id == evidence.claim_ref
                                && claim.evaluator_ref == evidence.evaluator_ref
                        })
                    })
                    .collect(),
                completion_assertion: WorkflowCompletionAssertion::Asserted,
            },
        };
        let trusted = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            admitted.document().clone(),
            evaluation,
            snapshot_digest.clone(),
            self.binding.project_id.0.clone(),
            ADAPTER_SOURCE_ID.to_owned(),
        )?;
        let verified = evaluate_verified_workflow_governance(trusted)?;
        let applicability = derived.applicability.get(&selected.id).copied();
        let guidance_status = if !boundary_rechecks.is_empty() {
            WorkflowGovernanceGuidanceStatus::Blocked
        } else if selected_already_completed {
            WorkflowGovernanceGuidanceStatus::PhaseComplete
        } else if selected.routing.activation == WorkflowPolicyActivation::WhenApplicable
            && applicability.is_none()
        {
            WorkflowGovernanceGuidanceStatus::ApplicabilityRequired
        } else {
            match verified.status() {
                WorkflowGovernanceStatus::Ineligible | WorkflowGovernanceStatus::Blocked => {
                    WorkflowGovernanceGuidanceStatus::Blocked
                }
                WorkflowGovernanceStatus::Active => WorkflowGovernanceGuidanceStatus::Active,
                WorkflowGovernanceStatus::Complete => {
                    WorkflowGovernanceGuidanceStatus::ReadyToComplete
                }
            }
        };
        let guidance = WorkflowGovernanceGuidance {
            authority: WorkflowGovernanceGuidanceAuthority::VerifiedProjectSnapshot,
            status: guidance_status,
            project_id: self.binding.project_id.clone(),
            bundle_id: identity.bundle_id,
            bundle_digest: identity.bundle_digest,
            release: Self::release_audit(registry, admitted, projection),
            snapshot_digest,
            ledger_head_digest: projection
                .head_digest
                .clone()
                .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?,
            state_version: projection.current_state_version().unwrap_or_default(),
            current_phase: phase.0,
            selected_policy_ref: selected.id.clone(),
            compatibility_workflow_id: selected.compatibility_workflow_id.clone(),
            target: selected.routing.readiness_target,
            applicability,
            boundary_rechecks,
            simulation: verified.simulation.clone(),
        };
        Ok((guidance, verified))
    }

    fn require_active_policy(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
        requested_policy_ref: &StableId,
    ) -> Result<ReadinessTarget, WorkflowGovernanceAdapterError> {
        let guidance =
            self.guidance_from_projection(registry, admitted, projection, unix_time()?)?;
        if &guidance.selected_policy_ref == requested_policy_ref {
            return Ok(guidance.target);
        }
        guidance
            .boundary_rechecks
            .iter()
            .find(|boundary| &boundary.policy_ref == requested_policy_ref)
            .map(|boundary| boundary.requested_target)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    }

    fn plan_phase_advance(
        &self,
        admitted: &AdmittedWorkflowGovernanceRelease,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
    ) -> Result<Option<(u64, WorkflowGovernanceEvent)>, WorkflowGovernanceAdapterError> {
        let current = current_phase(projection)?;
        let Some(current_phase_value) = Phase::parse(&current.0) else {
            return Err(WorkflowGovernanceAdapterError::InvalidPhase(current.0));
        };
        let snapshot = project_snapshot_digest(&self.binding.project_root)?;
        let trusted_registry_digest = self.current_trusted_registry_digest()?;
        let derived = derive_receipts(
            admitted.document(),
            projection,
            &self.binding.project_root,
            &snapshot,
            now,
            trusted_registry_digest.as_deref(),
        )?;
        let phase_done = admitted
            .document()
            .workflow_governance_bundle
            .policies
            .iter()
            .filter(|policy| {
                policy
                    .eligible_phases
                    .iter()
                    .any(|tag| Phase::tag_eligible(&tag.0, current_phase_value))
            })
            .filter(|policy| {
                policy.routing.activation != WorkflowPolicyActivation::OnSignal
                    || policy
                        .routing
                        .signals
                        .iter()
                        .any(|signal| derived.active_signals.contains(signal))
            })
            .all(|policy| {
                derived.completed_policy_refs.contains(&policy.id)
                    || derived.not_applicable_policy_refs.contains(&policy.id)
            });
        if !phase_done {
            return Ok(None);
        }
        let next = match current_phase_value {
            Phase::Discovery => Some(Phase::Specification),
            Phase::Specification => Some(Phase::Plan),
            Phase::Plan => Some(Phase::BuildVerify),
            // P5c ends at release readiness. Retaining build-verify lets a
            // replacement agent resume a typed terminal projection.
            _ => None,
        };
        let Some(next) = next else {
            return Ok(None);
        };
        let state_version = projection
            .current_state_version()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let event = WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
            from_phase: Some(current),
            to_phase: StableId(next.to_string()),
            snapshot_digest: snapshot,
        });
        Ok(Some((state_version, event)))
    }
}

/// Prepared completion authority; opaque and intentionally non-Clone/non-serde.
pub struct PreparedWorkflowGovernanceCompletion {
    completion: VerifiedWorkflowGovernanceCompletion,
    project_id: StableId,
    policy_ref: StableId,
    bundle_digest: String,
    snapshot_digest: String,
    ledger_head_digest: String,
    state_version: u64,
    current_phase: String,
    target: ReadinessTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceInitializationStatus {
    Initialized,
    AlreadyInitialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceReleasePinOrigin {
    ImplicitP5cGenesis,
    LedgerTransition,
}

/// Serializable release observation. It is audit only and cannot recreate the
/// opaque admitted release authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseAudit {
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub registry: WorkflowReleaseRegistryProvenance,
    pub pin_origin: WorkflowGovernanceReleasePinOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceInitialization {
    pub status: WorkflowGovernanceInitializationStatus,
    pub project_id: StableId,
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub release: WorkflowGovernanceReleaseAudit,
    pub snapshot_digest: String,
    pub head_digest: String,
    pub state_version: u64,
    pub current_phase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceGuidanceAuthority {
    VerifiedProjectSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceGuidanceStatus {
    ApplicabilityRequired,
    Blocked,
    Active,
    ReadyToComplete,
    PhaseComplete,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceGuidance {
    pub authority: WorkflowGovernanceGuidanceAuthority,
    pub status: WorkflowGovernanceGuidanceStatus,
    pub project_id: StableId,
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub release: WorkflowGovernanceReleaseAudit,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
    pub current_phase: String,
    pub selected_policy_ref: StableId,
    pub compatibility_workflow_id: StableId,
    pub target: ReadinessTarget,
    pub applicability: Option<bool>,
    pub boundary_rechecks: Vec<WorkflowGovernanceBoundaryRecheck>,
    pub simulation: WorkflowGovernanceSimulation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseStatus {
    pub active: WorkflowGovernanceReleaseAudit,
    pub ledger_head_digest: String,
    pub snapshot_digest: String,
    pub state_version: u64,
    pub available_successor: Option<WorkflowGovernanceReleaseIdentity>,
    pub upgrade_argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceReleaseUpgradeStatus {
    Upgraded,
    AlreadyPinned,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseUpgradeReceipt {
    pub status: WorkflowGovernanceReleaseUpgradeStatus,
    pub active: WorkflowGovernanceReleaseAudit,
    pub transition_record: Option<WorkflowGovernanceLedgerRecord>,
    pub ledger_head_digest: String,
    pub snapshot_digest: String,
    pub state_version: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceBoundaryRecheck {
    pub policy_ref: StableId,
    pub requested_target: ReadinessTarget,
    pub simulation: WorkflowGovernanceSimulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceShadowAuthority {
    ReadOnlyComparison,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceShadowReport {
    pub authority: WorkflowGovernanceShadowAuthority,
    pub mutation_allowed: bool,
    pub retirement_allowed: bool,
    pub project_id: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub selected_policy_ref: StableId,
    pub migrated: WorkflowGovernanceGuidance,
    pub legacy: LegacyWorkflowGovernanceProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceCompletionAuthority {
    ConsumedAfterLateRecheck,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceCompletionReceipt {
    pub authority: WorkflowGovernanceCompletionAuthority,
    pub completed_record: WorkflowGovernanceLedgerRecord,
    pub phase_advanced_record: Option<WorkflowGovernanceLedgerRecord>,
    pub continuity_record: WorkflowGovernanceLedgerRecord,
    pub next: WorkflowGovernanceGuidance,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkflowGovernanceAdapterError {
    InvalidProjectId,
    Path {
        field: &'static str,
        path: PathBuf,
        source: String,
    },
    InvalidStateRoot {
        path: PathBuf,
    },
    ProjectBinding {
        source: String,
    },
    TrustedRegistry {
        source: String,
    },
    SnapshotCapacity {
        files: usize,
        bytes: u64,
    },
    SnapshotPathEscape {
        path: PathBuf,
    },
    ReleaseAdmission(AdmittedWorkflowGovernanceReleaseError),
    Ledger(WorkflowGovernanceLedgerError),
    TrustedSnapshot(TrustedWorkflowGovernanceSnapshotError),
    Evaluation(WorkflowGovernanceRejection),
    LedgerIdentityMismatch,
    LedgerUninitialized,
    UnknownRelease(String),
    ReleaseNotAdjacent,
    ReleasePolicyDrift,
    ReleaseCasMismatch,
    ReleaseChainInvalid,
    ReleaseCommitIndeterminate,
    InvalidPhase(String),
    NoEligiblePolicy,
    UnknownPolicy(String),
    UnknownClaim(String),
    UnknownEvaluator(String),
    UnknownCapability(String),
    UnknownDecision(String),
    UnknownReceipt(String),
    InvalidObservation(String),
    AuthorizationBindingMismatch,
    WaiverNotAllowed,
    PolicyIncomplete,
    PolicyAlreadyCompleted,
    CompletionDrift,
    FoundationalReceiptRevocation,
    StateVersionOverflow,
    Clock,
    ClockOverflow,
    Canonicalization(String),
    EmbeddedCatalogInvalid,
    LegacyWorkflowMissing(String),
    LegacyProjection(String),
}

impl fmt::Display for WorkflowGovernanceAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId => f.write_str("project id must not be blank"),
            Self::Path { field, path, source } => write!(f, "{field} {} is unavailable: {source}", path.display()),
            Self::InvalidStateRoot { path } => write!(f, "state root {} must be an existing .forge-method directory", path.display()),
            Self::ProjectBinding { source } => {
                write!(f, "project/state binding is invalid: {source}")
            }
            Self::TrustedRegistry { source } => {
                write!(f, "fixed operator workflow registry is invalid: {source}")
            }
            Self::SnapshotCapacity { files, bytes } => write!(f, "project snapshot exceeds capacity ({files} files, {bytes} bytes)"),
            Self::SnapshotPathEscape { path } => write!(f, "project snapshot path escapes root: {}", path.display()),
            Self::ReleaseAdmission(error) => {
                write!(f, "workflow release admission failed: {error:?}")
            }
            Self::Ledger(error) => write!(f, "governance ledger failed: {error}"),
            Self::TrustedSnapshot(error) => write!(f, "trusted snapshot failed: {error:?}"),
            Self::Evaluation(error) => write!(f, "governance evaluation rejected: {:?}", error.issues),
            Self::LedgerIdentityMismatch => f.write_str("governance ledger identity does not match the resolved project and admitted bundle"),
            Self::LedgerUninitialized => f.write_str("governance ledger is not initialized; run workflow init"),
            Self::UnknownRelease(id) => write!(f, "unknown admitted workflow release {id}"),
            Self::ReleaseNotAdjacent => f.write_str("target workflow release is not the exact adjacent successor"),
            Self::ReleasePolicyDrift => f.write_str("workflow release policy set drift forbids receipt carryover"),
            Self::ReleaseCasMismatch => f.write_str("workflow release upgrade CAS failed; refresh release status"),
            Self::ReleaseChainInvalid => f.write_str("durable workflow release transition chain is not admitted"),
            Self::ReleaseCommitIndeterminate => f.write_str("workflow release commit recovery did not resolve to source or requested target"),
            Self::InvalidPhase(phase) => write!(f, "invalid durable phase {phase}"),
            Self::NoEligiblePolicy => f.write_str("no incomplete governed policy is eligible for the durable phase"),
            Self::UnknownPolicy(id) => write!(f, "unknown workflow policy {id}"),
            Self::UnknownClaim(id) => write!(f, "unknown workflow claim {id}"),
            Self::UnknownEvaluator(id) => write!(f, "unknown workflow evaluator {id}"),
            Self::UnknownCapability(id) => write!(f, "unknown workflow capability {id}"),
            Self::UnknownDecision(id) => write!(f, "unknown workflow decision {id}"),
            Self::UnknownReceipt(id) => write!(f, "unknown governance receipt {id}"),
            Self::InvalidObservation(message) => write!(f, "invalid trusted observation: {message}"),
            Self::AuthorizationBindingMismatch => f.write_str("verified human authorization does not match current governance state"),
            Self::WaiverNotAllowed => f.write_str("claim is not waivable by policy"),
            Self::PolicyIncomplete => f.write_str("selected policy is not ready for governed completion"),
            Self::PolicyAlreadyCompleted => {
                f.write_str("the governed phase is already complete")
            }
            Self::CompletionDrift => f.write_str("governed completion drifted during late recheck; refresh and retry from new guidance"),
            Self::FoundationalReceiptRevocation => f.write_str("the foundational project-import receipt cannot be revoked"),
            Self::StateVersionOverflow => f.write_str("governance state version overflow"),
            Self::Clock => f.write_str("system clock is before Unix epoch"),
            Self::ClockOverflow => f.write_str("governance observation expiry overflow"),
            Self::Canonicalization(error) => write!(f, "canonicalization failed: {error}"),
            Self::EmbeddedCatalogInvalid => f.write_str("embedded legacy catalog is invalid"),
            Self::LegacyWorkflowMissing(id) => write!(f, "legacy compatibility workflow {id} is missing"),
            Self::LegacyProjection(error) => write!(f, "legacy shadow projection failed: {error}"),
        }
    }
}

impl std::error::Error for WorkflowGovernanceAdapterError {}

impl From<AdmittedWorkflowGovernanceReleaseError> for WorkflowGovernanceAdapterError {
    fn from(value: AdmittedWorkflowGovernanceReleaseError) -> Self {
        Self::ReleaseAdmission(value)
    }
}
impl From<WorkflowGovernanceLedgerError> for WorkflowGovernanceAdapterError {
    fn from(value: WorkflowGovernanceLedgerError) -> Self {
        Self::Ledger(value)
    }
}
impl From<TrustedWorkflowGovernanceSnapshotError> for WorkflowGovernanceAdapterError {
    fn from(value: TrustedWorkflowGovernanceSnapshotError) -> Self {
        Self::TrustedSnapshot(value)
    }
}
impl From<WorkflowGovernanceRejection> for WorkflowGovernanceAdapterError {
    fn from(value: WorkflowGovernanceRejection) -> Self {
        Self::Evaluation(value)
    }
}

#[derive(Default)]
struct DerivedReceipts {
    completed_policy_refs: BTreeSet<StableId>,
    not_applicable_policy_refs: BTreeSet<StableId>,
    applicability: BTreeMap<StableId, bool>,
    active_signals: BTreeSet<WorkflowGovernanceSignal>,
    active_signal_receipt_digests: BTreeMap<WorkflowGovernanceSignal, String>,
    available_capability_refs: BTreeSet<StableId>,
    decision_need_refs: BTreeSet<StableId>,
    resolved_decision_refs: BTreeSet<StableId>,
    evidence: Vec<WorkflowEvidenceObservation>,
    waivers: Vec<WorkflowClaimWaiverObservation>,
}

fn derive_receipts(
    bundle: &WorkflowGovernanceBundleDocument,
    projection: &WorkflowGovernanceLedgerProjection,
    project_root: &Path,
    snapshot_digest: &str,
    now: u64,
    trusted_registry_digest: Option<&str>,
) -> Result<DerivedReceipts, WorkflowGovernanceAdapterError> {
    let receipt_records = &projection.records[receipt_window_start(projection)..];
    let revoked = receipt_records
        .iter()
        .filter_map(|record| match &record.event {
            WorkflowGovernanceEvent::ReceiptRevoked(event) => Some((
                event.revoked_record_id.clone(),
                event.revoked_record_digest.clone(),
            )),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let valid_record_digests = receipt_records
        .iter()
        .filter(|record| {
            !revoked.contains(&(record.record_id.clone(), record.record_digest.clone()))
        })
        .map(|record| record.record_digest.clone())
        .collect::<BTreeSet<_>>();
    let mut derived = DerivedReceipts::default();
    let mut signal_states =
        BTreeMap::<WorkflowGovernanceSignal, (bool, StableId, u64, String, bool)>::new();
    for record in receipt_records {
        if revoked.contains(&(record.record_id.clone(), record.record_digest.clone())) {
            continue;
        }
        if let WorkflowGovernanceEvent::SignalChanged(event) = &record.event {
            let trusted = event.observed_at_unix <= now
                && now <= event.expires_at_unix
                && trusted_registry_digest == Some(event.authorization_registry_digest.as_str())
                && record.previous_record_digest.as_deref()
                    == Some(event.ledger_head_digest.as_str())
                && event.snapshot_digest == snapshot_digest
                && content_addressed_basis_current(project_root, &event.basis)?
                && content_addressed_basis_digest(&event.basis)? == event.basis_digest;
            let transition_valid = match signal_states.get(&event.signal) {
                None => event.active && event.generation == 1,
                Some((true, episode, generation, _, _)) => {
                    !event.active && event.generation == *generation && event.episode_id == *episode
                }
                Some((false, episode, generation, _, _)) => {
                    event.active
                        && event.generation == generation.saturating_add(1)
                        && event.episode_id != *episode
                }
            };
            if transition_valid {
                signal_states.insert(
                    event.signal,
                    (
                        event.active,
                        event.episode_id.clone(),
                        event.generation,
                        record.record_digest.clone(),
                        trusted,
                    ),
                );
            }
        }
    }
    for (signal, (active, _, _, digest, trusted)) in signal_states {
        if active && trusted {
            derived.active_signals.insert(signal);
            derived.active_signal_receipt_digests.insert(signal, digest);
        }
    }
    for record in receipt_records {
        if revoked.contains(&(record.record_id.clone(), record.record_digest.clone())) {
            continue;
        }
        match &record.event {
            WorkflowGovernanceEvent::PolicyCompleted(event)
                if record.previous_record_digest.as_deref()
                    == Some(event.ledger_head_digest.as_str())
                    && event.snapshot_digest == snapshot_digest
                    && subject_current(project_root, snapshot_digest, &event.subject)?
                    && event
                        .dependency_receipt_digests
                        .iter()
                        .all(|digest| valid_record_digests.contains(digest))
                    && event
                        .evidence_receipt_digests
                        .iter()
                        .all(|digest| valid_record_digests.contains(digest)) =>
            {
                let signal_bound = bundle
                    .workflow_governance_bundle
                    .policies
                    .iter()
                    .find(|policy| policy.id == event.policy_ref)
                    .is_none_or(|policy| {
                        policy.routing.activation != WorkflowPolicyActivation::OnSignal
                            || policy.routing.signals.iter().any(|signal| {
                                derived
                                    .active_signal_receipt_digests
                                    .get(signal)
                                    .is_some_and(|digest| {
                                        event.dependency_receipt_digests.contains(digest)
                                    })
                            })
                    });
                if signal_bound {
                    derived
                        .completed_policy_refs
                        .insert(event.policy_ref.clone());
                }
            }
            WorkflowGovernanceEvent::ApplicabilityAssessed(event)
                if event.observed_at_unix <= now
                    && now <= event.expires_at_unix
                    && trusted_registry_digest
                        == Some(event.authorization_registry_digest.as_str())
                    && event.evaluator_ref.0 == WORKFLOW_APPLICABILITY_EVALUATOR_REF
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && content_addressed_basis_current(project_root, &event.basis)?
                    && content_addressed_basis_digest(&event.basis)? == event.basis_digest =>
            {
                derived
                    .applicability
                    .insert(event.policy_ref.clone(), event.applicable);
            }
            WorkflowGovernanceEvent::CapabilityProbed(event)
                if event.available
                    && event.observed_at_unix <= now
                    && event.expires_at_unix.is_none_or(|expires| now <= expires)
                    && trusted_registry_digest
                        == Some(event.authorization_registry_digest.as_str())
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str()) =>
            {
                let subject_is_current =
                    subject_current(project_root, snapshot_digest, &event.subject)?;
                let snapshot_is_current = event.subject.kind
                    == WorkflowEvidenceSubjectKind::Artifact
                    || event.snapshot_digest == snapshot_digest;
                if subject_is_current && snapshot_is_current {
                    derived
                        .available_capability_refs
                        .insert(event.capability_ref.clone());
                }
            }
            WorkflowGovernanceEvent::DecisionNeedRaised(event) => {
                derived
                    .decision_need_refs
                    .insert(event.decision_ref.clone());
            }
            WorkflowGovernanceEvent::DecisionResolved(event)
                if event.resolved_at_unix <= now
                    && trusted_registry_digest
                        == Some(event.authorization_registry_digest.as_str())
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str()) =>
            {
                derived
                    .resolved_decision_refs
                    .insert(event.decision_ref.clone());
            }
            WorkflowGovernanceEvent::EvaluatorObserved(event)
                if event.observed_at_unix <= now
                    && trusted_registry_digest
                        == Some(event.authorization_registry_digest.as_str())
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str()) =>
            {
                let Some(policy) = bundle
                    .workflow_governance_bundle
                    .policies
                    .iter()
                    .find(|policy| policy.id == event.policy_ref)
                else {
                    continue;
                };
                let Some(evaluator) = policy
                    .evaluators
                    .iter()
                    .find(|evaluator| evaluator.id == event.evaluator_ref)
                else {
                    continue;
                };
                let age_current =
                    now.saturating_sub(event.observed_at_unix) <= evaluator.max_age_seconds;
                let expiry_current = event.expires_at_unix.is_none_or(|expires| now <= expires);
                let subject_current =
                    subject_current(project_root, snapshot_digest, &event.subject)?;
                let snapshot_current = event.subject.kind == WorkflowEvidenceSubjectKind::Artifact
                    || event.snapshot_digest == snapshot_digest;
                derived.evidence.push(WorkflowEvidenceObservation {
                    evidence_ref: event.provenance.semantic_identity.0.clone(),
                    claim_ref: event.claim_ref.clone(),
                    evaluator_ref: event.evaluator_ref.clone(),
                    principal: event.provenance.principal.clone(),
                    kind: event.kind,
                    strength: event.strength,
                    freshness: if age_current
                        && expiry_current
                        && subject_current
                        && snapshot_current
                    {
                        WorkflowEvidenceFreshness::Current
                    } else {
                        WorkflowEvidenceFreshness::Stale
                    },
                    outcome: event.outcome,
                });
            }
            WorkflowGovernanceEvent::WaiverAuthorized(event)
                if event.authorized_at_unix <= now
                    && now <= event.expires_at_unix
                    && trusted_registry_digest
                        == Some(event.authorization_registry_digest.as_str())
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && subject_current(project_root, snapshot_digest, &event.subject)? =>
            {
                derived.waivers.push(WorkflowClaimWaiverObservation {
                    claim_ref: event.claim_ref.clone(),
                    principal: event.principal.clone(),
                    authority_scope: event.authority_scope.clone(),
                    max_target: event.max_target,
                    authorization_intent_digest: event.authorization_intent_digest.clone(),
                    authorized_at_unix: event.authorized_at_unix,
                    expires_at_unix: event.expires_at_unix,
                });
            }
            _ => {}
        }
    }
    derived.not_applicable_policy_refs.extend(
        derived
            .applicability
            .iter()
            .filter(|(_, applicable)| !**applicable)
            .map(|(policy, _)| policy.clone()),
    );
    Ok(derived)
}

fn receipt_window_start(projection: &WorkflowGovernanceLedgerProjection) -> usize {
    let mut start = 0;
    for (index, record) in projection.records.iter().enumerate() {
        if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
            match event.receipt_carryover {
                WorkflowReceiptCarryover::PreservePolicyEquivalent
                    if event.from_runtime_bundle.policy_set_digest
                        == event.to_runtime_bundle.policy_set_digest => {}
                WorkflowReceiptCarryover::InvalidateAll
                | WorkflowReceiptCarryover::NotApplicable
                | WorkflowReceiptCarryover::PreservePolicyEquivalent => start = index + 1,
            }
        }
    }
    start
}

fn boundary_rechecks(
    bundle: &WorkflowGovernanceBundleDocument,
    derived: &DerivedReceipts,
    state_version: u64,
    observed_at_unix: u64,
    requested_target: ReadinessTarget,
) -> Result<Vec<WorkflowGovernanceBoundaryRecheck>, WorkflowGovernanceAdapterError> {
    if requested_target == ReadinessTarget::Explore {
        return Ok(Vec::new());
    }
    let mut rechecks = Vec::new();
    for policy in &bundle.workflow_governance_bundle.policies {
        if !derived.completed_policy_refs.contains(&policy.id)
            || derived.not_applicable_policy_refs.contains(&policy.id)
        {
            continue;
        }
        let evaluation_phase = policy
            .eligible_phases
            .iter()
            .find(|phase| Phase::parse(&phase.0).is_some())
            .cloned()
            .unwrap_or_else(|| StableId("1-discovery".to_owned()));
        let evaluation = WorkflowGovernanceEvaluationDocument {
            schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
            workflow_governance_evaluation: WorkflowGovernanceEvaluation {
                observation_set_id: StableId(format!(
                    "observation.boundary.{}.{}",
                    policy.id.0, state_version
                )),
                state_version,
                observed_at_unix,
                bundle_id: bundle.workflow_governance_bundle.id.clone(),
                policy_id: policy.id.clone(),
                current_phase: evaluation_phase,
                target: requested_target,
                completed_policy_refs: derived.completed_policy_refs.iter().cloned().collect(),
                not_applicable_policy_refs: derived
                    .not_applicable_policy_refs
                    .iter()
                    .cloned()
                    .collect(),
                available_capability_refs: derived
                    .available_capability_refs
                    .iter()
                    .filter(|capability| {
                        policy
                            .capability_requirements
                            .iter()
                            .any(|requirement| &requirement.id == *capability)
                    })
                    .cloned()
                    .collect(),
                decision_need_refs: derived
                    .decision_need_refs
                    .iter()
                    .filter(|decision| {
                        policy
                            .decision_rules
                            .iter()
                            .any(|rule| &rule.id == *decision)
                    })
                    .cloned()
                    .collect(),
                resolved_decision_refs: derived
                    .resolved_decision_refs
                    .iter()
                    .filter(|decision| {
                        policy
                            .decision_rules
                            .iter()
                            .any(|rule| &rule.id == *decision)
                    })
                    .cloned()
                    .collect(),
                waivers: derived
                    .waivers
                    .iter()
                    .filter(|waiver| {
                        policy
                            .claims
                            .iter()
                            .any(|claim| claim.id == waiver.claim_ref)
                    })
                    .cloned()
                    .collect(),
                evidence: derived
                    .evidence
                    .iter()
                    .filter(|evidence| {
                        policy.claims.iter().any(|claim| {
                            claim.id == evidence.claim_ref
                                && claim.evaluator_ref == evidence.evaluator_ref
                        })
                    })
                    .cloned()
                    .collect(),
                completion_assertion: WorkflowCompletionAssertion::Asserted,
            },
        };
        let simulation = simulate_workflow_governance(bundle, &evaluation)?;
        if simulation.candidate_status != WorkflowGovernanceStatus::Complete {
            rechecks.push(WorkflowGovernanceBoundaryRecheck {
                policy_ref: policy.id.clone(),
                requested_target,
                simulation,
            });
        }
    }
    rechecks.sort_by_key(|recheck| {
        bundle
            .workflow_governance_bundle
            .policies
            .iter()
            .find(|policy| policy.id == recheck.policy_ref)
            .map_or(u16::MAX, |policy| policy.routing.priority)
    });
    Ok(rechecks)
}

fn select_policy<'a>(
    bundle: &'a WorkflowGovernanceBundleDocument,
    derived: &DerivedReceipts,
    phase: &StableId,
) -> Result<&'a WorkflowGovernancePolicy, WorkflowGovernanceAdapterError> {
    let parsed = Phase::parse(&phase.0)
        .ok_or_else(|| WorkflowGovernanceAdapterError::InvalidPhase(phase.0.clone()))?;
    let mut candidates = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .filter(|policy| {
            !derived.completed_policy_refs.contains(&policy.id)
                && !derived.not_applicable_policy_refs.contains(&policy.id)
        })
        .filter(|policy| {
            // A current snapshot can invalidate an earlier phase's completion.
            // Such a policy must become selectable again; otherwise the durable
            // phase pointer would strand the project behind stale prerequisites.
            policy
                .eligible_phases
                .iter()
                .any(|tag| Phase::tag_eligible(&tag.0, parsed))
                || policy.eligible_phases.iter().any(|tag| {
                    Phase::parse(&tag.0).is_some_and(|eligible| eligible.rank() < parsed.rank())
                })
        })
        .filter(|policy| match policy.routing.activation {
            WorkflowPolicyActivation::Required | WorkflowPolicyActivation::WhenApplicable => true,
            WorkflowPolicyActivation::OnSignal => policy
                .routing
                .signals
                .iter()
                .any(|signal| derived.active_signals.contains(signal)),
        })
        .filter(|policy| {
            policy
                .prerequisites
                .iter()
                .all(|prerequisite| match prerequisite.requirement {
                    WorkflowPrerequisiteRequirement::Always => derived
                        .completed_policy_refs
                        .contains(&prerequisite.policy_ref),
                    WorkflowPrerequisiteRequirement::WhenApplicable => {
                        derived
                            .completed_policy_refs
                            .contains(&prerequisite.policy_ref)
                            || derived
                                .not_applicable_policy_refs
                                .contains(&prerequisite.policy_ref)
                    }
                })
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|policy| (policy.routing.priority, policy.id.0.as_str()));
    if let Some(policy) = candidates.into_iter().next() {
        return Ok(policy);
    }
    let mut completed = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .filter(|policy| derived.completed_policy_refs.contains(&policy.id))
        .filter(|policy| {
            policy
                .eligible_phases
                .iter()
                .any(|tag| Phase::tag_eligible(&tag.0, parsed))
        })
        .filter(|policy| match policy.routing.activation {
            WorkflowPolicyActivation::Required | WorkflowPolicyActivation::WhenApplicable => true,
            WorkflowPolicyActivation::OnSignal => policy
                .routing
                .signals
                .iter()
                .any(|signal| derived.active_signals.contains(signal)),
        })
        .collect::<Vec<_>>();
    completed.sort_by_key(|policy| (policy.routing.priority, policy.id.0.as_str()));
    completed
        .into_iter()
        .next_back()
        .ok_or(WorkflowGovernanceAdapterError::NoEligiblePolicy)
}

fn current_phase(
    projection: &WorkflowGovernanceLedgerProjection,
) -> Result<StableId, WorkflowGovernanceAdapterError> {
    if projection.records.is_empty() {
        return Err(WorkflowGovernanceAdapterError::LedgerUninitialized);
    }
    let mut phase = None;
    for record in &projection.records {
        match &record.event {
            WorkflowGovernanceEvent::ProjectImported(event) => {
                phase = Some(event.initial_phase.clone());
            }
            WorkflowGovernanceEvent::PhaseAdvanced(event) => phase = Some(event.to_phase.clone()),
            _ => {}
        }
    }
    phase.ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)
}

fn validate_identity(
    projection: &WorkflowGovernanceLedgerProjection,
    expected: &WorkflowGovernanceLedgerIdentity,
    expected_project_root: &Path,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let Some(found) = projection.active_identity() else {
        return Err(WorkflowGovernanceAdapterError::LedgerUninitialized);
    };
    if &found != expected {
        return Err(WorkflowGovernanceAdapterError::LedgerIdentityMismatch);
    }
    let imported_root = projection.records.first().and_then(|record| {
        if let WorkflowGovernanceEvent::ProjectImported(event) = &record.event {
            Some(event.source_ref.as_str())
        } else {
            None
        }
    });
    let expected_root = expected_project_root.display().to_string();
    if imported_root != Some(expected_root.as_str()) {
        return Err(WorkflowGovernanceAdapterError::LedgerIdentityMismatch);
    }
    Ok(())
}

fn policy_by_id<'a>(
    bundle: &'a WorkflowGovernanceBundleDocument,
    id: &StableId,
) -> Result<&'a WorkflowGovernancePolicy, WorkflowGovernanceAdapterError> {
    bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == *id)
        .ok_or_else(|| WorkflowGovernanceAdapterError::UnknownPolicy(id.0.clone()))
}

// A Result keeps all receipt-derivation predicates uniform and leaves room for
// future provider-specific verification errors without changing the boundary.
#[allow(clippy::unnecessary_wraps)]
fn subject_current(
    project_root: &Path,
    snapshot_digest: &str,
    subject: &WorkflowEvidenceSubject,
) -> Result<bool, WorkflowGovernanceAdapterError> {
    match subject.kind {
        WorkflowEvidenceSubjectKind::Artifact => {
            let path = PathBuf::from(&subject.subject_ref);
            let Ok((_, bytes)) = read_confined_file(project_root, &path) else {
                return Ok(false);
            };
            Ok(sha256_content_hash(&bytes) == subject.subject_digest)
        }
        WorkflowEvidenceSubjectKind::RepositoryState
        | WorkflowEvidenceSubjectKind::ProjectSnapshot => {
            Ok(subject.subject_digest == snapshot_digest)
        }
        WorkflowEvidenceSubjectKind::Runtime
        | WorkflowEvidenceSubjectKind::ExternalSystem
        | WorkflowEvidenceSubjectKind::HumanDecision => Ok(true),
    }
}

fn canonical_directory(
    path: &Path,
    field: &'static str,
) -> Result<PathBuf, WorkflowGovernanceAdapterError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| WorkflowGovernanceAdapterError::Path {
            field,
            path: path.to_path_buf(),
            source: error.to_string(),
        })?;
    if !canonical.is_dir() {
        return Err(WorkflowGovernanceAdapterError::Path {
            field,
            path: canonical,
            source: "not a directory".to_owned(),
        });
    }
    Ok(canonical)
}

fn validate_project_state_binding(
    project_id: &StableId,
    project_root: &Path,
    state_root: &Path,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let link_path = project_root.join(PROJECT_LINK_FILE_NAME);
    if !link_path.exists() {
        let inline = project_root
            .join(".forge-method")
            .canonicalize()
            .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
                source: format!("inline state root is unavailable: {error}"),
            })?;
        if inline != state_root {
            return Err(WorkflowGovernanceAdapterError::ProjectBinding {
                source: "without a Project Link, state_root must be project_root/.forge-method"
                    .to_owned(),
            });
        }
        return Ok(());
    }
    let raw = fs::read_to_string(&link_path).map_err(|error| {
        WorkflowGovernanceAdapterError::ProjectBinding {
            source: format!("cannot read {}: {error}", link_path.display()),
        }
    })?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let link: ProjectLinkDocument = yaml_serde::from_str(raw).map_err(|error| {
        WorkflowGovernanceAdapterError::ProjectBinding {
            source: format!("cannot parse {}: {error}", link_path.display()),
        }
    })?;
    if link.schema_version != PROJECT_LINK_SCHEMA_VERSION || &link.project_id != project_id {
        return Err(WorkflowGovernanceAdapterError::ProjectBinding {
            source: "Project Link schema/project identity mismatch".to_owned(),
        });
    }
    let linked_state = project_root
        .join(&link.state_root.0)
        .canonicalize()
        .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
            source: format!("linked state root is unavailable: {error}"),
        })?;
    let linked_sidecar = project_root
        .join(&link.sidecar_root.0)
        .canonicalize()
        .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
            source: format!("linked sidecar root is unavailable: {error}"),
        })?;
    if linked_state != state_root
        || state_root.parent() != Some(linked_sidecar.as_path())
        || linked_state.starts_with(project_root)
    {
        return Err(WorkflowGovernanceAdapterError::ProjectBinding {
            source: "resolved state root does not match the canonical sidecar Project Link"
                .to_owned(),
        });
    }
    Ok(())
}

fn read_confined_file(
    root: &Path,
    relative: &Path,
) -> Result<(String, Vec<u8>), WorkflowGovernanceAdapterError> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(WorkflowGovernanceAdapterError::SnapshotPathEscape {
            path: relative.to_path_buf(),
        });
    }
    let candidate = root.join(relative);
    let canonical =
        candidate
            .canonicalize()
            .map_err(|error| WorkflowGovernanceAdapterError::Path {
                field: "evidence_path",
                path: candidate.clone(),
                source: error.to_string(),
            })?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(WorkflowGovernanceAdapterError::SnapshotPathEscape { path: canonical });
    }
    let bytes = fs::read(&canonical).map_err(|error| WorkflowGovernanceAdapterError::Path {
        field: "evidence_path",
        path: canonical.clone(),
        source: error.to_string(),
    })?;
    let normalized = canonical
        .strip_prefix(root)
        .expect("confined path")
        .to_string_lossy()
        .replace('\\', "/");
    Ok((normalized, bytes))
}

// This predicate intentionally shares the Result-returning derivation contract
// even though missing or escaped basis files currently map to a stale result.
#[allow(clippy::unnecessary_wraps)]
fn content_addressed_basis_current(
    root: &Path,
    basis: &[WorkflowContentAddressedReference],
) -> Result<bool, WorkflowGovernanceAdapterError> {
    if basis.is_empty() {
        return Ok(false);
    }
    for reference in basis {
        let Ok((_, bytes)) = read_confined_file(root, Path::new(&reference.subject_ref)) else {
            return Ok(false);
        };
        if sha256_content_hash(&bytes) != reference.subject_digest {
            return Ok(false);
        }
    }
    Ok(true)
}

fn content_addressed_basis_from_paths(
    root: &Path,
    paths: &[String],
) -> Result<Vec<WorkflowContentAddressedReference>, WorkflowGovernanceAdapterError> {
    if paths.is_empty() {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "applicability authorization requires at least one basis artifact".to_owned(),
        ));
    }
    let mut basis = Vec::with_capacity(paths.len());
    for path in paths {
        let (subject_ref, bytes) = read_confined_file(root, Path::new(path))?;
        basis.push(WorkflowContentAddressedReference {
            subject_ref,
            subject_digest: sha256_content_hash(&bytes),
        });
    }
    basis.sort_by(|left, right| {
        left.subject_ref
            .cmp(&right.subject_ref)
            .then_with(|| left.subject_digest.cmp(&right.subject_digest))
    });
    basis.dedup();
    Ok(basis)
}

fn content_addressed_basis_digest(
    basis: &[WorkflowContentAddressedReference],
) -> Result<String, WorkflowGovernanceAdapterError> {
    let mut canonical_basis = basis.to_vec();
    canonical_basis.sort_by(|left, right| {
        left.subject_ref
            .cmp(&right.subject_ref)
            .then_with(|| left.subject_digest.cmp(&right.subject_digest))
    });
    let canonical = serde_json_canonicalizer::to_vec(&canonical_basis)
        .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
    Ok(sha256_content_hash(&canonical))
}

fn project_snapshot_digest(root: &Path) -> Result<String, WorkflowGovernanceAdapterError> {
    let mut stack = vec![root.to_path_buf()];
    let mut entries = Vec::new();
    let mut files = 0usize;
    let mut bytes_total = 0u64;
    while let Some(directory) = stack.pop() {
        let mut children = fs::read_dir(&directory)
            .map_err(|error| WorkflowGovernanceAdapterError::Path {
                field: "project_snapshot",
                path: directory.clone(),
                source: error.to_string(),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| WorkflowGovernanceAdapterError::Path {
                field: "project_snapshot",
                path: directory.clone(),
                source: error.to_string(),
            })?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children.into_iter().rev() {
            let path = child.path();
            let relative = path.strip_prefix(root).map_err(|_| {
                WorkflowGovernanceAdapterError::SnapshotPathEscape { path: path.clone() }
            })?;
            let name = relative
                .components()
                .next()
                .and_then(|component| component.as_os_str().to_str())
                .unwrap_or_default();
            if matches!(name, ".git" | ".forge-method" | "target" | "node_modules") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                WorkflowGovernanceAdapterError::Path {
                    field: "project_snapshot",
                    path: path.clone(),
                    source: error.to_string(),
                }
            })?;
            if metadata.file_type().is_symlink() {
                let target =
                    fs::read_link(&path).map_err(|error| WorkflowGovernanceAdapterError::Path {
                        field: "project_snapshot",
                        path: path.clone(),
                        source: error.to_string(),
                    })?;
                entries.push((
                    relative.to_string_lossy().replace('\\', "/"),
                    format!("symlink:{}", target.display()),
                ));
            } else if metadata.is_dir() {
                let canonical =
                    path.canonicalize()
                        .map_err(|error| WorkflowGovernanceAdapterError::Path {
                            field: "project_snapshot",
                            path: path.clone(),
                            source: error.to_string(),
                        })?;
                if !canonical.starts_with(root) {
                    return Err(WorkflowGovernanceAdapterError::SnapshotPathEscape {
                        path: canonical,
                    });
                }
                stack.push(path);
            } else if metadata.is_file() {
                files += 1;
                bytes_total = bytes_total.saturating_add(metadata.len());
                if files > MAX_SNAPSHOT_FILES || bytes_total > MAX_SNAPSHOT_BYTES {
                    return Err(WorkflowGovernanceAdapterError::SnapshotCapacity {
                        files,
                        bytes: bytes_total,
                    });
                }
                let bytes =
                    fs::read(&path).map_err(|error| WorkflowGovernanceAdapterError::Path {
                        field: "project_snapshot",
                        path: path.clone(),
                        source: error.to_string(),
                    })?;
                entries.push((
                    relative.to_string_lossy().replace('\\', "/"),
                    sha256_content_hash(&bytes),
                ));
            }
        }
    }
    entries.sort();
    let canonical = serde_json_canonicalizer::to_vec(&entries)
        .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}

fn unix_time() -> Result<u64, WorkflowGovernanceAdapterError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| WorkflowGovernanceAdapterError::Clock)
}

fn readiness_name(target: ReadinessTarget) -> &'static str {
    match target {
        ReadinessTarget::Explore => "explore",
        ReadinessTarget::Execute => "execute",
        ReadinessTarget::Release => "release",
    }
}
fn parse_readiness(value: &str) -> Result<ReadinessTarget, WorkflowGovernanceAdapterError> {
    match value {
        "explore" => Ok(ReadinessTarget::Explore),
        "execute" => Ok(ReadinessTarget::Execute),
        "release" => Ok(ReadinessTarget::Release),
        _ => Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(label: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("forge-p5c-adapter-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".forge-method")).expect("state root");
        fs::write(root.join("README.md"), b"project\n").expect("project file");
        let root = root.canonicalize().expect("canonical temp");
        let state = root.join(".forge-method");
        (root, state)
    }

    fn release_record(
        carryover: WorkflowReceiptCarryover,
        from_policy_set: &str,
        to_policy_set: &str,
    ) -> WorkflowGovernanceLedgerRecord {
        let release = |id: &str, digest_byte: char| WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("workflow-governance.core".to_owned()),
            release_id: StableId(id.to_owned()),
            release_version: "0.1.0".to_owned(),
            release_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
        };
        let runtime =
            |id: &str, digest_byte: char, policy_set_digest: &str| WorkflowRuntimeBundleIdentity {
                bundle_id: StableId(id.to_owned()),
                bundle_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
                policy_set_digest: policy_set_digest.to_owned(),
            };
        WorkflowGovernanceLedgerRecord {
            record_id: StableId("record.release-upgrade".to_owned()),
            sequence: 2,
            project_id: StableId("project.test".to_owned()),
            bundle_id: StableId("bundle.source".to_owned()),
            bundle_digest: format!("sha256:{}", "3".repeat(64)),
            state_version: 1,
            previous_record_digest: Some(format!("sha256:{}", "4".repeat(64))),
            record_digest: format!("sha256:{}", "5".repeat(64)),
            recorded_at_unix: 10,
            event: WorkflowGovernanceEvent::ReleaseUpgraded(ReleaseUpgradedEvent {
                from_release: release("release.source", 'a'),
                to_release: release("release.target", 'b'),
                from_runtime_bundle: runtime("bundle.source", 'c', from_policy_set),
                to_runtime_bundle: runtime("bundle.target", 'd', to_policy_set),
                registry_provenance: WorkflowReleaseRegistryProvenance {
                    registry_id: StableId("registry.test".to_owned()),
                    registry_version: "0.1.0".to_owned(),
                    registry_digest: format!("sha256:{}", "6".repeat(64)),
                },
                admission_proof: forge_core_contracts::WorkflowReleaseAdmissionProof {
                    proof_id: StableId("proof.test".to_owned()),
                    proof_digest: format!("sha256:{}", "7".repeat(64)),
                    snapshot_digest: format!("sha256:{}", "8".repeat(64)),
                    from_policy_set_digest: from_policy_set.to_owned(),
                    to_policy_set_digest: to_policy_set.to_owned(),
                },
                receipt_carryover: carryover,
                prior_ledger_head_digest: format!("sha256:{}", "4".repeat(64)),
            }),
        }
    }

    #[test]
    fn receipt_window_preserves_only_exact_policy_equivalence() {
        let policy_set = format!("sha256:{}", "1".repeat(64));
        let equivalent = WorkflowGovernanceLedgerProjection {
            records: vec![release_record(
                WorkflowReceiptCarryover::PreservePolicyEquivalent,
                &policy_set,
                &policy_set,
            )],
            head_digest: None,
            next_sequence: 3,
            next_state_version: 2,
        };
        assert_eq!(receipt_window_start(&equivalent), 0);

        let drifted = WorkflowGovernanceLedgerProjection {
            records: vec![release_record(
                WorkflowReceiptCarryover::PreservePolicyEquivalent,
                &policy_set,
                &format!("sha256:{}", "2".repeat(64)),
            )],
            head_digest: None,
            next_sequence: 3,
            next_state_version: 2,
        };
        assert_eq!(receipt_window_start(&drifted), 1);

        let invalidated = WorkflowGovernanceLedgerProjection {
            records: vec![release_record(
                WorkflowReceiptCarryover::InvalidateAll,
                &policy_set,
                &policy_set,
            )],
            head_digest: None,
            next_sequence: 3,
            next_state_version: 2,
        };
        assert_eq!(receipt_window_start(&invalidated), 1);
    }

    #[test]
    fn initializes_and_derives_first_policy_without_state_yaml() {
        let (root, state) = temp_project("init-next");
        fs::write(state.join("state.yaml"), "current_phase: 4-build-verify\n")
            .expect("compat state");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.test".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        let initialized = adapter.initialize().expect("initialize");
        assert_eq!(initialized.current_phase, "1-discovery");
        let next = adapter.next().expect("next");
        assert_eq!(
            next.selected_policy_ref.0,
            "policy.workflow.discover-intent"
        );
        assert_eq!(next.current_phase, "1-discovery");
        assert_eq!(
            next.authority,
            WorkflowGovernanceGuidanceAuthority::VerifiedProjectSnapshot
        );
    }

    #[test]
    fn project_change_invalidates_prepared_completion_snapshot() {
        let (root, state) = temp_project("snapshot-drift");
        let first = project_snapshot_digest(&root).expect("first digest");
        fs::write(root.join("README.md"), b"changed\n").expect("change");
        let second = project_snapshot_digest(&root).expect("second digest");
        assert_ne!(first, second);
        assert!(!first.contains(&root.to_string_lossy().to_string()));
        assert!(state.ends_with(".forge-method"));
    }

    #[test]
    fn applicability_receipt_is_stale_after_project_snapshot_drift() {
        let (root, _) = temp_project("applicability-snapshot-drift");
        let captured_snapshot = project_snapshot_digest(&root).expect("captured snapshot");
        let basis = content_addressed_basis_from_paths(&root, &["README.md".to_owned()])
            .expect("content-addressed applicability basis");
        let basis_digest = content_addressed_basis_digest(&basis).expect("basis digest");
        let registry_digest = format!("sha256:{}", "a".repeat(64));
        let head = format!("sha256:{}", "b".repeat(64));
        let projection = WorkflowGovernanceLedgerProjection {
            records: vec![WorkflowGovernanceLedgerRecord {
                record_id: StableId("record.applicability".to_owned()),
                sequence: 1,
                project_id: StableId("project.test".to_owned()),
                bundle_id: StableId("bundle.test".to_owned()),
                bundle_digest: format!("sha256:{}", "c".repeat(64)),
                state_version: 0,
                previous_record_digest: Some(head.clone()),
                record_digest: format!("sha256:{}", "d".repeat(64)),
                recorded_at_unix: 10,
                event: WorkflowGovernanceEvent::ApplicabilityAssessed(ApplicabilityAssessedEvent {
                    policy_ref: StableId("policy.workflow.domain-scan".to_owned()),
                    applicable: false,
                    assessed_by: PrincipalId("principal.human".to_owned()),
                    evaluator_ref: StableId(WORKFLOW_APPLICABILITY_EVALUATOR_REF.to_owned()),
                    credential_id: StableId("credential.human".to_owned()),
                    public_key_fingerprint: format!("sha256:{}", "e".repeat(64)),
                    authorization_registry_digest: registry_digest.clone(),
                    basis,
                    basis_digest,
                    snapshot_digest: captured_snapshot,
                    ledger_head_digest: head,
                    observed_at_unix: 10,
                    expires_at_unix: 1_000,
                }),
            }],
            head_digest: None,
            next_sequence: 2,
            next_state_version: 1,
        };

        // Drift outside the still-current basis must invalidate the assessment.
        fs::write(root.join("new-domain-input.md"), b"new domain constraint\n")
            .expect("snapshot drift");
        let current_snapshot = project_snapshot_digest(&root).expect("current snapshot");
        let registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("admitted registry");
        let admitted = registry.genesis();
        let derived = derive_receipts(
            admitted.document(),
            &projection,
            &root,
            &current_snapshot,
            20,
            Some(&registry_digest),
        )
        .expect("derive receipts");
        assert!(!derived
            .applicability
            .contains_key(&StableId("policy.workflow.domain-scan".to_owned())));
    }

    #[test]
    fn non_release_completion_is_stale_after_project_snapshot_drift() {
        let (root, _) = temp_project("non-release-completion-drift");
        let captured_snapshot = project_snapshot_digest(&root).expect("captured snapshot");
        let head = format!("sha256:{}", "1".repeat(64));
        let policy_ref = StableId("policy.workflow.discover-intent".to_owned());
        let projection = WorkflowGovernanceLedgerProjection {
            records: vec![WorkflowGovernanceLedgerRecord {
                record_id: StableId("record.completion".to_owned()),
                sequence: 1,
                project_id: StableId("project.test".to_owned()),
                bundle_id: StableId("bundle.test".to_owned()),
                bundle_digest: format!("sha256:{}", "2".repeat(64)),
                state_version: 1,
                previous_record_digest: Some(head.clone()),
                record_digest: format!("sha256:{}", "3".repeat(64)),
                recorded_at_unix: 10,
                event: WorkflowGovernanceEvent::PolicyCompleted(PolicyCompletedEvent {
                    policy_ref: policy_ref.clone(),
                    target: ReadinessTarget::Explore,
                    phase: StableId("1-discovery".to_owned()),
                    snapshot_digest: captured_snapshot.clone(),
                    ledger_head_digest: head,
                    subject: WorkflowEvidenceSubject {
                        kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                        subject_ref: "project.test".to_owned(),
                        subject_digest: captured_snapshot,
                    },
                    dependency_receipt_digests: Vec::new(),
                    evidence_receipt_digests: Vec::new(),
                    unresolved_deferred_obligation_refs: Vec::new(),
                    unresolved_deferred_capability_refs: Vec::new(),
                    completed_at_unix: 10,
                }),
            }],
            head_digest: None,
            next_sequence: 2,
            next_state_version: 2,
        };

        fs::write(root.join("README.md"), b"changed after completion\n").expect("snapshot drift");
        let current_snapshot = project_snapshot_digest(&root).expect("current snapshot");
        let registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("admitted registry");
        let admitted = registry.genesis();
        let derived = derive_receipts(
            admitted.document(),
            &projection,
            &root,
            &current_snapshot,
            20,
            None,
        )
        .expect("derive receipts");
        assert!(!derived.completed_policy_refs.contains(&policy_ref));
    }

    #[test]
    fn artifact_paths_are_confined() {
        let (root, _) = temp_project("confined");
        assert!(read_confined_file(&root, Path::new("README.md")).is_ok());
        assert!(read_confined_file(&root, Path::new("../outside")).is_err());
    }

    #[test]
    fn shared_sidecar_rejects_same_id_bound_to_a_different_project_root() {
        let (first_root, _) = temp_project("binding-first");
        let (second_root, _) = temp_project("binding-second");
        let sidecar =
            std::env::temp_dir().join(format!("forge-p5c-shared-sidecar-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sidecar);
        fs::create_dir_all(sidecar.join(".forge-method")).expect("shared sidecar state");
        let sidecar = sidecar.canonicalize().expect("canonical shared sidecar");
        let state = sidecar.join(".forge-method");
        let project_id = StableId("project.same-id".to_owned());
        for root in [&first_root, &second_root] {
            let link = ProjectLinkDocument {
                schema_version: PROJECT_LINK_SCHEMA_VERSION.to_owned(),
                project_id: project_id.clone(),
                sidecar_root: forge_core_contracts::RepoPath(sidecar.to_string_lossy().to_string()),
                state_root: forge_core_contracts::RepoPath(state.to_string_lossy().to_string()),
            };
            fs::write(
                root.join(PROJECT_LINK_FILE_NAME),
                yaml_serde::to_string(&link).expect("Project Link YAML"),
            )
            .expect("Project Link");
        }
        let first = WorkflowGovernanceProjectAdapter::new(project_id.clone(), &first_root, &state)
            .expect("first adapter");
        first.initialize().expect("bind ledger to first root");
        let second = WorkflowGovernanceProjectAdapter::new(project_id, &second_root, &state)
            .expect("link shape is valid before durable identity check");
        assert!(matches!(
            second.next(),
            Err(WorkflowGovernanceAdapterError::LedgerIdentityMismatch)
        ));
    }
}
