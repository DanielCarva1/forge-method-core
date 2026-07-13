//! Live Project Snapshot Adapter for the agent-native governance lane.
//!
//! The adapter is deliberately opinionated: the admitted bundle is embedded,
//! the durable ledger owns phase/state/prerequisite authority, repository
//! observations are re-hashed, and callers never choose a workflow or target.

// Opaque authorization and completion capabilities are intentionally consumed
// by value so callers cannot reuse them after a durable transition.
#![allow(clippy::needless_pass_by_value)]

use super::{
    admit_effective_workflow_governance_bundle, derive_core_only_workflow_effective_identity,
    domain_pack_generation_transition_event, evaluate_verified_workflow_governance,
    load_admitted_workflow_governance_reviewed_release_registry,
    AdmittedEffectiveWorkflowGovernanceBundle, AdmittedWorkflowGovernanceRelease,
    AdmittedWorkflowGovernanceReleaseError, AdmittedWorkflowGovernanceReleaseRegistry,
    EffectiveWorkflowGovernanceBundleError, TrustedWorkflowGovernanceSnapshot,
    TrustedWorkflowGovernanceSnapshotError, VerifiedWorkflowGovernanceCompletion,
    VerifiedWorkflowGovernanceDecision, WorkflowDomainPackContextView,
};
use forge_core_authority::workflow_authority::{
    WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE, WORKFLOW_APPLICABILITY_EVALUATOR_REF,
    WORKFLOW_CAPABILITY_AUTHORITY_SCOPE,
};
use forge_core_authority::{
    AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry, AuthorizedWorkflowBrokerRegistry,
    HistoricallyVerifiedWorkflowBrokerEvent, PrincipalCredentialStatus, PrincipalRegistryDocument,
    VerifiedWorkflowApplicabilityAuthorization, VerifiedWorkflowBrokerEvent,
    VerifiedWorkflowBrokerEventAudit, VerifiedWorkflowCapabilityAuthorization,
    VerifiedWorkflowDecisionAuthorization, VerifiedWorkflowEvidenceAuthorization,
    VerifiedWorkflowSignalAuthorization, VerifiedWorkflowWaiverAuthorization,
    WorkflowApplicabilityAuthorizationRequest, WorkflowAuthorizationKind, WorkflowBrokerEventKind,
    WorkflowBrokerIssuerProfile, WorkflowBrokerIssuerStatus, WorkflowBrokerRegistryDocument,
    WorkflowBrokerSemanticInput, WorkflowCapabilityAuthorizationRequest,
    WorkflowDecisionAuthorizationRequest, WorkflowEvidenceAuthorizationRequest,
    WorkflowSignalAuthorizationRequest, WorkflowWaiverAuthorizationRequest, WorkflowWaiverSubject,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::workflow_governance::{
    BrokerOriginAppliedEvent, WorkflowBrokerOriginProfile,
};
use forge_core_contracts::{
    ApplicabilityAssessedEvent, CapabilityProbedEvent, ContinuityRecordedEvent,
    DecisionAlternative, DecisionResolvedEvent, DomainPackCompositionGap, EvaluatorObservedEvent,
    Phase, PhaseAdvancedEvent, PolicyCompletedEvent, PrincipalId, ProjectImportedEvent,
    ProjectLinkDocument, ReadinessTarget, ReleaseUpgradedEvent, SignalChangedEvent, StableId,
    WaiverAuthorizedEvent, WorkflowCapabilityProbeKind, WorkflowClaimWaiverObservation,
    WorkflowClaimWaiverPolicy, WorkflowCompletionAssertion, WorkflowContentAddressedReference,
    WorkflowEffectiveBundleIdentity, WorkflowEvaluatorProvider, WorkflowEvidenceFreshness,
    WorkflowEvidenceKind, WorkflowEvidenceObservation, WorkflowEvidenceOutcome,
    WorkflowEvidenceProvenance, WorkflowEvidenceStrength, WorkflowEvidenceSubject,
    WorkflowEvidenceSubjectKind, WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluation,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernancePolicy, WorkflowGovernanceReleaseIdentity, WorkflowGovernanceSignal,
    WorkflowPolicyActivation, WorkflowPrerequisiteRequirement, WorkflowReceiptCarryover,
    WorkflowReleaseRegistryProvenance, WorkflowRuntimeBundleIdentity, PROJECT_LINK_FILE_NAME,
    PROJECT_LINK_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    find_entry, load_embedded_frozen_legacy_catalog, project_legacy_workflow_compatibility,
    simulate_workflow_governance, LegacyWorkflowGovernanceProjection, WorkflowClaimResultStatus,
    WorkflowGovernanceRejection, WorkflowGovernanceSimulation, WorkflowGovernanceStatus,
};
use forge_core_domain_pack_tcb::{
    lock_domain_pack_lifecycle, AdmittedActiveDomainPackGeneration, DomainPackLifecycleStoreError,
    LockedDomainPackLifecycle,
};
use forge_core_store::sha256_content_hash;
use forge_core_store::workflow_action_replay::{
    commit_workflow_action, initialize_workflow_action_replay, reserve_workflow_action,
    WorkflowActionReplayError,
};
use forge_core_workflow_governance_tcb::{
    lock_workflow_governance_ledger_tcb, LockedWorkflowGovernanceLedger,
    WorkflowGovernanceLedgerError, WorkflowGovernanceLedgerIdentity,
    WorkflowGovernanceLedgerProjection,
};
use serde::{Deserialize, Serialize};
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
const TRUSTED_WORKFLOW_BROKER_REGISTRY_RELATIVE_PATH: &str =
    "operator/workflow-broker-registry.yaml";
const MAX_TRUSTED_REGISTRY_BYTES: u64 = 1024 * 1024;
const WORKFLOW_AUTHORIZATION_ACTION_PACKET_SCHEMA_VERSION: &str =
    "workflow_authorization_action_packets_v1";
const WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS: u64 = 300;

/// Canonical project binding used by every live governance operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceProjectBinding {
    pub project_id: StableId,
    pub project_root: PathBuf,
    pub state_root: PathBuf,
}

/// State and policy coordinates shared by an authorization action packet.
///
/// Every field is derived from the admitted effective bundle and durable
/// project state. Semantic answers and observation timestamps are deliberately
/// absent: a later preparation step must combine this CAS-bound packet with a
/// closed input value and then perform the adapter's existing late recheck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationPacketBinding {
    pub project_id: StableId,
    pub effective_bundle_id: StableId,
    pub effective_bundle_digest: String,
    pub policy_ref: StableId,
    pub subject_ref: StableId,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub trusted_principal_registry_digest: Option<String>,
    pub trusted_broker_registry_digest: Option<String>,
    pub readiness_target: ReadinessTarget,
}

/// External approval boundary required before Forge may consume a packet.
///
/// These labels are intentionally honest about the current local credential
/// bridge: a serialized packet describes the required actor class but is not
/// itself proof that the actor was present or independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationApprovalBoundary {
    HumanApprovalBroker,
    IndependentReviewerBroker,
    TrustedRuntimeBroker,
    OperatorCredentialBroker,
}

/// Exact registry role/grant contract required to authorize a packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationRequiredAuthority {
    pub accepted_roles: Vec<CallerRole>,
    pub required_grant: StableId,
    pub approval_boundary: WorkflowAuthorizationApprovalBoundary,
}

/// The sole state transition a signal packet permits at its captured head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSignalInputTransition {
    Activate,
    Deactivate,
}

/// Closed semantic input contract for a generated action packet.
///
/// This is a choice/shape description, never an authorization response. It
/// prevents hosts from inventing policy ids, claims, evaluators, capability
/// probes, authority scopes, or signal generations when the request builder is
/// added. Free text is retained only where policy semantics require a reason or
/// provenance reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowAuthorizationInputContract {
    Applicability {
        basis_refs_min_items: usize,
        basis_refs_repo_relative: bool,
    },
    Capability {
        capability_ref: StableId,
        probe_kind: WorkflowCapabilityProbeKind,
        subject_kinds: Vec<WorkflowEvidenceSubjectKind>,
        probe_reference_required: bool,
    },
    Decision {
        decision_ref: StableId,
        alternatives: Vec<DecisionAlternative>,
        recommended_alternative_ref: StableId,
    },
    Evidence {
        claim_ref: StableId,
        evaluator_ref: StableId,
        provider: WorkflowEvaluatorProvider,
        evidence_kind: WorkflowEvidenceKind,
        strength: WorkflowEvidenceStrength,
        allowed_outcomes: Vec<WorkflowEvidenceOutcome>,
        subject_kinds: Vec<WorkflowEvidenceSubjectKind>,
        scenario_reference_required: bool,
    },
    Signal {
        signal: WorkflowGovernanceSignal,
        transition: WorkflowSignalInputTransition,
        basis_refs_min_items: usize,
        basis_refs_repo_relative: bool,
    },
    Waiver {
        claim_ref: StableId,
        maximum_readiness_target: ReadinessTarget,
        max_age_seconds: u64,
        reason_required: bool,
        consequence_statements: Vec<String>,
    },
}

/// Deterministic, non-executable description of one currently admissible
/// authority-bearing action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationActionPacket {
    pub schema_version: String,
    pub packet_id: StableId,
    pub packet_digest: String,
    pub authorization_kind: WorkflowAuthorizationKind,
    pub binding: WorkflowAuthorizationPacketBinding,
    pub required_authority: WorkflowAuthorizationRequiredAuthority,
    pub input_contract: WorkflowAuthorizationInputContract,
}

/// Read-only packet projection reconstructed from durable governance state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationActionPacketSet {
    pub authority: WorkflowGovernanceGuidanceAuthority,
    pub project_id: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
    pub registry_setup: WorkflowAuthorizationRegistrySetup,
    pub setup_gaps: Vec<WorkflowAuthorizationSetupGap>,
    pub packets: Vec<WorkflowAuthorizationActionPacket>,
}

/// Setup discovery only. `Ready` proves that a bounded, valid canonical
/// document with an active entry was found; it does not prove that Forge
/// observed enrollment/user presence or that the broker is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationRegistrySetupStatus {
    Missing,
    NoActiveIssuer,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationRegistrySetup {
    pub principal_registry: WorkflowAuthorizationRegistrySetupStatus,
    pub broker_registry: WorkflowAuthorizationRegistrySetupStatus,
}

/// Machine-actionable authority setup gap returned directly by `workflow
/// next`. The argv is an exact command shape with explicit placeholders only
/// for operator-owned public enrollment inputs; Forge never asks an agent for
/// a broker private key or hand-authored authorization document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationSetupGap {
    pub code: WorkflowAuthorizationSetupGapCode,
    pub summary: String,
    pub accepted_profiles: Vec<WorkflowBrokerIssuerProfile>,
    pub setup_argv: Vec<String>,
    pub required_operator_inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationSetupGapCode {
    BrokerRegistryMissing,
    BrokerRegistryNoActiveIssuer,
}

/// Authority actions and setup guidance embedded in the normal governed-next
/// response. Existing guidance fields remain unchanged for compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationGuidance {
    pub registry_setup: WorkflowAuthorizationRegistrySetup,
    pub setup_gaps: Vec<WorkflowAuthorizationSetupGap>,
    pub action_packets: Vec<WorkflowAuthorizationActionPacket>,
}

#[derive(Debug, Clone)]
struct TrustedBrokerRegistryState {
    digest: Option<String>,
    setup: WorkflowAuthorizationRegistrySetupStatus,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowAuthorizationActionPacketDigestBasis<'a> {
    schema_version: &'a str,
    packet_id: &'a StableId,
    authorization_kind: WorkflowAuthorizationKind,
    binding: &'a WorkflowAuthorizationPacketBinding,
    required_authority: &'a WorkflowAuthorizationRequiredAuthority,
    input_contract: &'a WorkflowAuthorizationInputContract,
}

/// Minimal semantic answer accepted by [`WorkflowGovernanceProjectAdapter::prepare_authorization`].
/// All authority, identity, policy, digest, target, and clock fields remain
/// kernel-derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowAuthorizationClosedInput {
    Applicability {
        applicable: bool,
        basis_refs: Vec<String>,
    },
    Capability {
        available: bool,
        probe_ref: String,
        subject_kind: WorkflowEvidenceSubjectKind,
        subject_ref: String,
    },
    Decision {
        selected_alternative_ref: StableId,
    },
    Evidence {
        outcome: WorkflowEvidenceOutcome,
        subject_kind: WorkflowEvidenceSubjectKind,
        subject_ref: String,
        scenario_ref: String,
    },
    Signal {
        active: bool,
        basis_refs: Vec<String>,
    },
    Waiver {
        reason: String,
    },
}

/// Prepared but unsigned workflow request. This type deliberately implements
/// neither serde nor Clone and grants no mutation authority.
#[derive(Debug)]
pub enum PreparedWorkflowAuthorization {
    Applicability {
        request: WorkflowApplicabilityAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
    Capability {
        request: WorkflowCapabilityAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
    Decision {
        request: WorkflowDecisionAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
    Evidence {
        request: WorkflowEvidenceAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
    Signal {
        request: WorkflowSignalAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
    Waiver {
        request: WorkflowWaiverAuthorizationRequest,
        packet: WorkflowAuthorizationActionPacket,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerActionReceipt {
    pub action_record: WorkflowGovernanceLedgerRecord,
    pub origin_record: WorkflowGovernanceLedgerRecord,
    pub phase_advanced_record: Option<WorkflowGovernanceLedgerRecord>,
    pub replay_commit_repaired: bool,
    pub next: WorkflowGovernanceGuidance,
}

/// Kernel-owned adapter. It is configured with a resolved project, not with a
/// workflow, bundle, phase, target, evidence result, or completion claim.
#[derive(Debug, Clone)]
pub struct WorkflowGovernanceProjectAdapter {
    binding: WorkflowGovernanceProjectBinding,
}

/// Retains the Domain Pack lifecycle lock until the complete workflow
/// transaction ends. This enforces the global lifecycle -> workflow-ledger
/// lock order even for projects that currently have no active generation.
enum LockedWorkflowDomainPackContext {
    CoreOnly(Box<LockedDomainPackLifecycle>),
    Active(Box<AdmittedActiveDomainPackGeneration>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DomainPackTransitionRecovery {
    TargetCommitted,
    SourceUnchanged,
    Indeterminate,
}

impl LockedWorkflowDomainPackContext {
    fn acquire(state_root: &Path) -> Result<Self, WorkflowGovernanceAdapterError> {
        let lifecycle = lock_domain_pack_lifecycle(state_root)?;
        if lifecycle.projection().active_pointer.is_some() {
            Ok(Self::Active(Box::new(lifecycle.admit_active_generation()?)))
        } else {
            Ok(Self::CoreOnly(Box::new(lifecycle)))
        }
    }

    fn has_active_generation(&self) -> bool {
        match self {
            Self::CoreOnly(lifecycle) => {
                debug_assert!(lifecycle.projection().active_pointer.is_none());
                false
            }
            Self::Active(_) => true,
        }
    }

    fn admit_effective(
        &self,
        core: &AdmittedWorkflowGovernanceRelease,
    ) -> Result<AdmittedEffectiveWorkflowGovernanceBundle<'_>, WorkflowGovernanceAdapterError> {
        match self {
            Self::CoreOnly(lifecycle) => {
                debug_assert!(lifecycle.projection().active_pointer.is_none());
                let view = lifecycle.verified_core_only_view()?;
                Ok(admit_effective_workflow_governance_bundle(
                    core,
                    WorkflowDomainPackContextView::CoreOnly(view),
                )?)
            }
            Self::Active(active) => {
                let view = active.verified_view()?;
                Ok(admit_effective_workflow_governance_bundle(
                    core,
                    WorkflowDomainPackContextView::Active(view),
                )?)
            }
        }
    }
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
        initialize_workflow_action_replay(&self.binding.state_root)?;
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let genesis = registry.genesis();
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        if !projection.records.is_empty() {
            let admitted = self.resolve_active_release(&registry, &projection)?;
            let effective = domain.admit_effective(admitted)?;
            projection =
                self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
            let active_identity = self.identity(admitted);
            validate_identity(&projection, &active_identity, &self.binding.project_root)?;
            return Ok(WorkflowGovernanceInitialization {
                status: WorkflowGovernanceInitializationStatus::AlreadyInitialized,
                project_id: self.binding.project_id.clone(),
                bundle_id: effective
                    .identity()
                    .effective_runtime_bundle
                    .bundle_id
                    .clone(),
                bundle_digest: effective
                    .identity()
                    .effective_runtime_bundle
                    .bundle_digest
                    .clone(),
                release: Self::release_audit(&registry, admitted, &projection),
                effective: effective.identity().clone(),
                domain_pack_degraded: effective.is_domain_pack_degraded(),
                domain_pack_gaps: effective.domain_pack_gaps().to_vec(),
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
        let identity = self.identity(genesis);
        let record = ledger.initialize_unchecked_tcb(&identity, 0, event)?;
        projection = ledger.recover()?;
        let effective = domain.admit_effective(genesis)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, genesis, &effective, projection)?;
        Ok(WorkflowGovernanceInitialization {
            status: WorkflowGovernanceInitializationStatus::Initialized,
            project_id: self.binding.project_id.clone(),
            bundle_id: effective
                .identity()
                .effective_runtime_bundle
                .bundle_id
                .clone(),
            bundle_digest: effective
                .identity()
                .effective_runtime_bundle
                .bundle_digest
                .clone(),
            release: Self::release_audit(&registry, genesis, &projection),
            effective: effective.identity().clone(),
            domain_pack_degraded: effective.is_domain_pack_degraded(),
            domain_pack_gaps: effective.domain_pack_gaps().to_vec(),
            snapshot_digest: snapshot_digest.clone(),
            head_digest: projection
                .head_digest
                .clone()
                .unwrap_or(record.record_digest),
            state_version: projection
                .current_state_version()
                .unwrap_or(record.state_version),
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        self.guidance_from_projection(&registry, admitted, &effective, &projection, now)
    }

    /// Project the currently admissible authority-bearing actions without
    /// accepting an answer or constructing a signed authorization request.
    ///
    /// Packet identity is deterministic and every digest is bound to the
    /// admitted effective policy, durable state/head, live project snapshot,
    /// operator registry (when present), subject, and readiness target.
    ///
    /// # Errors
    /// Returns a typed error when durable guidance or a closed authority/input
    /// contract cannot be reconstructed from admitted state.
    pub fn action_packets(
        &self,
    ) -> Result<WorkflowAuthorizationActionPacketSet, WorkflowGovernanceAdapterError> {
        self.action_packets_at(unix_time()?)
    }

    fn action_packets_at(
        &self,
        now: u64,
    ) -> Result<WorkflowAuthorizationActionPacketSet, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        let guidance =
            self.guidance_from_projection(&registry, admitted, &effective, &projection, now)?;
        let WorkflowAuthorizationGuidance {
            registry_setup,
            setup_gaps,
            action_packets,
        } = guidance.authorization;
        Ok(WorkflowAuthorizationActionPacketSet {
            authority: WorkflowGovernanceGuidanceAuthority::VerifiedProjectSnapshot,
            project_id: guidance.project_id,
            snapshot_digest: guidance.snapshot_digest,
            ledger_head_digest: guidance.ledger_head_digest,
            state_version: guidance.state_version,
            registry_setup,
            setup_gaps,
            packets: action_packets,
        })
    }

    /// Re-derive one current packet by digest and prepare its exact unsigned
    /// authority request from a minimal closed input.
    ///
    /// This operation neither signs nor records anything. A stale packet,
    /// unsupported choice, mismatched input kind, unconfined reference, or
    /// changed project snapshot fails closed.
    ///
    /// # Errors
    /// Returns a typed binding/observation error when the packet or input no
    /// longer matches admitted live state.
    pub fn prepare_authorization(
        &self,
        packet_digest: &str,
        closed_input: WorkflowAuthorizationClosedInput,
        now: u64,
    ) -> Result<PreparedWorkflowAuthorization, WorkflowGovernanceAdapterError> {
        if now == 0 {
            return Err(WorkflowGovernanceAdapterError::Clock);
        }
        let packet_set = self.action_packets_at(now)?;
        let packet = packet_set
            .packets
            .into_iter()
            .find(|candidate| candidate.packet_digest == packet_digest)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;

        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        let prepared = prepare_authorization_from_packet(
            effective.document(),
            &projection,
            &self.binding.project_root,
            packet,
            closed_input,
            now,
        )?;
        if project_snapshot_digest(&self.binding.project_root)? != packet_set.snapshot_digest
            || projection.head_digest.as_deref() != Some(packet_set.ledger_head_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        Ok(prepared)
    }

    /// Apply one separately verified broker-origin answer as an atomic action
    /// plus provenance companion, guarded by the dedicated replay WAL.
    ///
    /// # Errors
    /// Fails closed for stale packets, profile/boundary mismatch, broker
    /// registry rotation, replay conflict, project drift, or indeterminate
    /// ledger/replay recovery.
    pub fn apply_verified_broker_action(
        &self,
        verified: VerifiedWorkflowBrokerEvent,
        now: u64,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        if now == 0 {
            return Err(WorkflowGovernanceAdapterError::Clock);
        }
        let (semantic_input, audit) = verified.into_parts();
        if audit.project_id != self.binding.project_id {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }

        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let replay_origin_id = broker_replay_origin_id(&audit)?;
        if let Some((action_record, origin_record)) =
            matching_broker_origin_retry(&projection, &audit)?
        {
            let replay_repaired = ensure_broker_replay_committed(
                &self.binding.state_root,
                &audit.action_packet_digest,
                &replay_origin_id,
                &action_record.record_digest,
            )?;
            let next = self.guidance_from_projection(
                &registry,
                admitted,
                &effective,
                &projection,
                unix_time()?,
            )?;
            return Ok(WorkflowBrokerActionReceipt {
                action_record,
                origin_record,
                phase_advanced_record: None,
                replay_commit_repaired: replay_repaired,
                next,
            });
        }

        let current_now = unix_time()?;
        if audit.issued_at_unix > current_now || audit.expires_at_unix <= current_now {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let broker_registry_digest =
            self.current_trusted_broker_registry_digest()?
                .ok_or_else(|| WorkflowGovernanceAdapterError::TrustedRegistry {
                    source: format!(
                        "broker registry is missing at {}",
                        self.trusted_broker_registry_path().display()
                    ),
                })?;

        let guidance = self.guidance_from_projection(
            &registry,
            admitted,
            &effective,
            &projection,
            current_now,
        )?;
        let principal_registry_digest = self.current_trusted_registry_digest()?;
        let derived = derive_receipts(
            effective.document(),
            &projection,
            &self.binding.project_root,
            &guidance.snapshot_digest,
            current_now,
            principal_registry_digest.as_deref(),
        )?;
        let packet = authorization_action_packets(
            effective.document(),
            &guidance,
            &derived,
            principal_registry_digest,
            Some(broker_registry_digest.clone()),
        )?
        .into_iter()
        .find(|packet| packet.packet_digest == audit.action_packet_digest)
        .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        validate_broker_packet_audit(&packet, &semantic_input, &audit, &broker_registry_digest)?;
        let closed_input = broker_semantic_input_to_closed(semantic_input);
        let mut prepared = prepare_authorization_from_packet(
            effective.document(),
            &projection,
            &self.binding.project_root,
            packet,
            closed_input,
            audit.issued_at_unix,
        )?;
        bound_prepared_expiry(&mut prepared, audit.expires_at_unix)?;
        let (packet, action_event, phase_may_advance) = broker_action_event_from_prepared(
            effective.document(),
            &self.binding.project_root,
            prepared,
            &audit,
            &broker_registry_digest,
        )?;

        let head = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let identity = self.identity(admitted);
        let mut batch = ledger.begin_unchecked_tcb_batch(&head, &identity)?;
        let action_record = batch.push_verified_broker_action_unchecked_tcb(
            packet.binding.state_version,
            action_event,
            &packet.packet_digest,
            &audit.event_digest,
            audit.issued_at_unix,
        )?;
        let commit_now = unix_time()?;
        if audit.issued_at_unix > commit_now
            || audit.expires_at_unix <= commit_now
            || self.current_trusted_broker_registry_digest()?.as_deref()
                != Some(broker_registry_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let origin_event = WorkflowGovernanceEvent::BrokerOriginApplied(
            broker_origin_applied_event(&packet, &audit, &broker_registry_digest, &action_record),
        );
        let origin_record = batch.push_event(packet.binding.state_version, origin_event)?;
        let phase_advanced_record = if phase_may_advance {
            self.plan_phase_advance(&effective, batch.projection(), commit_now)?
                .map(|(state_version, event)| batch.push_event(state_version, event))
                .transpose()?
        } else {
            None
        };
        if project_snapshot_digest(&self.binding.project_root)? != packet.binding.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let final_commit_now = unix_time()?;
        if audit.issued_at_unix > final_commit_now
            || audit.expires_at_unix <= final_commit_now
            || self.current_trusted_broker_registry_digest()?.as_deref()
                != Some(broker_registry_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        batch.commit()?;
        ensure_broker_replay_committed(
            &self.binding.state_root,
            &packet.packet_digest,
            &replay_origin_id,
            &action_record.record_digest,
        )?;
        let committed = ledger.recover()?;
        let next = self.guidance_from_projection(
            &registry,
            admitted,
            &effective,
            &committed,
            final_commit_now,
        )?;
        Ok(WorkflowBrokerActionReceipt {
            action_record,
            origin_record,
            phase_advanced_record,
            replay_commit_repaired: false,
            next,
        })
    }

    /// Reconcile replay state for an already durable broker-origin action
    /// using a historically verified envelope. This capability can never
    /// append a workflow ledger event.
    ///
    /// # Errors
    /// Fails unless an exact, hash-chain-valid `BrokerOriginApplied` companion
    /// already exists for every historical audit coordinate.
    pub fn recover_historically_verified_broker_action(
        &self,
        verified: HistoricallyVerifiedWorkflowBrokerEvent,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        let (_, audit) = verified.into_parts();
        if audit.project_id != self.binding.project_id {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let registry = load_admitted_workflow_governance_reviewed_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        let (action_record, origin_record) = matching_broker_origin_retry(&projection, &audit)?
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        let replay_repaired = ensure_broker_replay_committed(
            &self.binding.state_root,
            &audit.action_packet_digest,
            &broker_replay_origin_id(&audit)?,
            &action_record.record_digest,
        )?;
        let next = self.guidance_from_projection(
            &registry,
            admitted,
            &effective,
            &projection,
            unix_time()?,
        )?;
        Ok(WorkflowBrokerActionReceipt {
            action_record,
            origin_record,
            phase_advanced_record: None,
            replay_commit_repaired: replay_repaired,
            next,
        })
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let active = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(active)?;
        projection = self.reconcile_effective_epoch(&mut ledger, active, &effective, projection)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let head_digest = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let available = registry
            .adjacent_successor(active)
            .map(|successor| successor.release().clone());
        let domain_pack_rebase_required = domain.has_active_generation();
        let upgrade_argv = (!domain_pack_rebase_required)
            .then_some(available.as_ref())
            .flatten()
            .map(|target| {
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
            effective: effective.identity().clone(),
            domain_pack_degraded: effective.is_domain_pack_degraded(),
            domain_pack_gaps: effective.domain_pack_gaps().to_vec(),
            ledger_head_digest: head_digest,
            snapshot_digest,
            state_version: projection.current_state_version().unwrap_or_default(),
            available_successor: available,
            upgrade_argv,
            domain_pack_rebase_required,
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        if domain.has_active_generation() {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseRequired);
        }
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
            || request.evaluator_ref.0 != WORKFLOW_APPLICABILITY_EVALUATOR_REF
            || request.authority_scope.0 != WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(effective.document(), &request.policy_ref)?;
        self.require_active_policy(
            &registry,
            admitted,
            &effective,
            &projection,
            &request.policy_ref,
        )?;
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
            self.plan_phase_advance(&effective, batch.projection(), unix_time()?)?
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
            || request.authority_scope.0 != WORKFLOW_CAPABILITY_AUTHORITY_SCOPE
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(effective.document(), &request.policy_ref)?;
        self.require_active_policy(
            &registry,
            admitted,
            &effective,
            &projection,
            &request.policy_ref,
        )?;
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(effective.document(), &request.policy_ref)?;
        let active_target = self.require_active_policy(
            &registry,
            admitted,
            &effective,
            &projection,
            &request.policy_ref,
        )?;
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
            || request.state_version != projection.current_state_version().unwrap_or_default()
            || request.current_phase != phase
            || request.snapshot_digest != snapshot_digest
            || request.ledger_head_digest != head
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let policy = policy_by_id(effective.document(), &request.policy_ref)?;
        self.require_active_policy(
            &registry,
            admitted,
            &effective,
            &projection,
            &request.policy_ref,
        )?;
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
        let decision_packet = make_authorization_action_packet(
            WorkflowAuthorizationKind::Decision,
            StableId(format!("packet.workflow.decision.{}", rule.id.0)),
            WorkflowAuthorizationPacketBinding {
                project_id: self.binding.project_id.clone(),
                effective_bundle_id: effective
                    .identity()
                    .effective_runtime_bundle
                    .bundle_id
                    .clone(),
                effective_bundle_digest: effective
                    .identity()
                    .effective_runtime_bundle
                    .bundle_digest
                    .clone(),
                policy_ref: policy.id.clone(),
                subject_ref: rule.id.clone(),
                state_version: request.state_version,
                current_phase: phase.clone(),
                snapshot_digest: snapshot_digest.clone(),
                ledger_head_digest: head.to_owned(),
                trusted_principal_registry_digest: self.current_trusted_registry_digest()?,
                trusted_broker_registry_digest: self.current_trusted_broker_registry_digest()?,
                readiness_target: policy.routing.readiness_target,
            },
            human_authority("workflow.decision.resolve"),
            WorkflowAuthorizationInputContract::Decision {
                decision_ref: rule.id.clone(),
                alternatives: rule.alternatives.clone(),
                recommended_alternative_ref: rule.recommended_alternative_ref.clone(),
            },
        )?;
        let expected_consequences_digest = decision_consequences_ack_digest(
            &decision_packet.packet_digest,
            &rule.id,
            &selected_alternative.id,
            &selected_alternative.consequences,
        )?;
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
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
        let policy = policy_by_id(effective.document(), &request.policy_ref)?;
        self.require_active_policy(
            &registry,
            admitted,
            &effective,
            &projection,
            &request.policy_ref,
        )?;
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
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
            || request.policy_bundle_digest
                != effective.identity().effective_runtime_bundle.bundle_digest
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
            self.plan_phase_advance(&effective, batch.projection(), unix_time()?)?
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let (guidance, verified) =
            self.verified_from_projection(&registry, admitted, &effective, &projection, now)?;
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
            effective_bundle_identity: effective.identity().clone(),
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
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let identity = self.identity(admitted);
        let (fresh, verified) =
            self.verified_from_projection(&registry, admitted, &effective, &projection, now)?;
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
            || prepared.effective_bundle_identity != *effective.identity()
        {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        let completed_state_version = fresh
            .state_version
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let completed_policy = policy_by_id(effective.document(), &fresh.selected_policy_ref)?;
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
                effective.document(),
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
            self.plan_phase_advance(&effective, batch.projection(), now)?
        {
            Some(batch.push_event(state_version, event)?)
        } else {
            None
        };
        let next_guidance = self.guidance_from_projection(
            &registry,
            admitted,
            &effective,
            batch.projection(),
            now,
        )?;
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
        let next = self.guidance_from_projection(
            &registry,
            admitted,
            &effective,
            batch.projection(),
            now,
        )?;
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

    /// Reconcile the independently committed lifecycle generation into the
    /// workflow ledger before any guidance or mutation is derived. The caller
    /// already retains the lifecycle lock, so this never inverts lock order.
    fn reconcile_effective_epoch(
        &self,
        ledger: &mut LockedWorkflowGovernanceLedger,
        core: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        mut projection: WorkflowGovernanceLedgerProjection,
    ) -> Result<WorkflowGovernanceLedgerProjection, WorkflowGovernanceAdapterError> {
        if projection.records.is_empty() {
            return Ok(projection);
        }
        let core_identity = self.identity(core);
        validate_identity(&projection, &core_identity, &self.binding.project_root)?;

        let target = effective.identity();
        let active = match projection.active_effective_bundle_identity() {
            Some(active) => active,
            None => derive_core_only_workflow_effective_identity(core)?,
        };
        if active == *target {
            return Ok(projection);
        }
        if active.core_runtime_bundle != target.core_runtime_bundle {
            return Err(WorkflowGovernanceAdapterError::DomainPackCoreMismatch);
        }
        let Some(target_generation) = target.domain_pack_generation.as_ref() else {
            return Err(WorkflowGovernanceAdapterError::DomainPackGenerationMissing);
        };
        if let Some(active_generation) = active.domain_pack_generation.as_ref() {
            if target_generation.generation < active_generation.generation {
                return Err(
                    WorkflowGovernanceAdapterError::DomainPackGenerationRegression {
                        active: active_generation.generation,
                        found: target_generation.generation,
                    },
                );
            }
            if target_generation.generation == active_generation.generation {
                return Err(WorkflowGovernanceAdapterError::DomainPackGenerationFork {
                    generation: target_generation.generation,
                });
            }
        }
        let prior_head = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let state_version = projection
            .current_state_version()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let event = domain_pack_generation_transition_event(&active, effective, prior_head.clone());
        let transition = ledger.transition_domain_pack_generation_unchecked_tcb(
            &prior_head,
            &core_identity,
            state_version,
            event,
        );
        match transition {
            Ok(_) => {
                projection = ledger.recover()?;
                if classify_domain_pack_transition_recovery(
                    &projection,
                    &active,
                    target,
                    &prior_head,
                    state_version,
                ) != DomainPackTransitionRecovery::TargetCommitted
                {
                    return Err(WorkflowGovernanceAdapterError::DomainPackCommitIndeterminate);
                }
            }
            Err(commit_error) => {
                // Atomic replacement can become durable before a directory
                // sync reports failure. Reconcile under the still-retained
                // lifecycle and workflow locks rather than falsely reporting
                // failure for a committed target epoch.
                let recovered = ledger.recover()?;
                match classify_domain_pack_transition_recovery(
                    &recovered,
                    &active,
                    target,
                    &prior_head,
                    state_version,
                ) {
                    DomainPackTransitionRecovery::TargetCommitted => projection = recovered,
                    DomainPackTransitionRecovery::SourceUnchanged => {
                        return Err(WorkflowGovernanceAdapterError::Ledger(commit_error));
                    }
                    DomainPackTransitionRecovery::Indeterminate => {
                        return Err(WorkflowGovernanceAdapterError::DomainPackCommitIndeterminate);
                    }
                }
            }
        }
        Ok(projection)
    }

    fn ensure_domain_pack_ready_for_mutation(
        effective: &AdmittedEffectiveWorkflowGovernanceBundle<'_>,
    ) -> Result<(), WorkflowGovernanceAdapterError> {
        if effective.is_domain_pack_degraded() {
            return Err(WorkflowGovernanceAdapterError::DomainPackGapsBlocking(
                effective.domain_pack_gaps().to_vec(),
            ));
        }
        Ok(())
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

    /// Fixed broker-registry path kept separate from both project content and
    /// the principal credential registry. Presence is setup discovery only.
    #[must_use]
    pub fn trusted_broker_registry_path(&self) -> PathBuf {
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
                .join("workflow-broker-registry.yaml");
        }
        self.binding
            .state_root
            .parent()
            .unwrap_or(&self.binding.state_root)
            .join(TRUSTED_WORKFLOW_BROKER_REGISTRY_RELATIVE_PATH)
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

    fn current_trusted_broker_registry_digest(
        &self,
    ) -> Result<Option<String>, WorkflowGovernanceAdapterError> {
        self.current_trusted_broker_registry_state()
            .map(|state| state.digest)
    }

    fn current_trusted_broker_registry_state(
        &self,
    ) -> Result<TrustedBrokerRegistryState, WorkflowGovernanceAdapterError> {
        let path = self.trusted_broker_registry_path();
        if !path.exists() {
            return Ok(TrustedBrokerRegistryState {
                digest: None,
                setup: WorkflowAuthorizationRegistrySetupStatus::Missing,
            });
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
        let document: WorkflowBrokerRegistryDocument =
            yaml_serde::from_str(&raw).map_err(|error| {
                WorkflowGovernanceAdapterError::TrustedRegistry {
                    source: format!("cannot parse {}: {error}", path.display()),
                }
            })?;
        let expected_audience = self.expected_broker_audience();
        AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
            document.clone(),
            &expected_audience,
        )
        .map_err(|error| WorkflowGovernanceAdapterError::TrustedRegistry {
            source: format!("{} is invalid: {error}", path.display()),
        })?;
        let setup = if document
            .issuers
            .iter()
            .any(|issuer| issuer.status == WorkflowBrokerIssuerStatus::Active)
        {
            WorkflowAuthorizationRegistrySetupStatus::Ready
        } else {
            WorkflowAuthorizationRegistrySetupStatus::NoActiveIssuer
        };
        let canonical = serde_json_canonicalizer::to_vec(&document)
            .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
        Ok(TrustedBrokerRegistryState {
            digest: Some(sha256_content_hash(&canonical)),
            setup,
        })
    }

    fn expected_broker_audience(&self) -> String {
        format!("forge-core:workflow:{}", self.binding.project_id.0)
    }

    fn guidance_from_projection(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
    ) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        self.verified_from_projection(registry, admitted, effective, projection, now)
            .map(|(guidance, _)| guidance)
    }

    fn verified_from_projection(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
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
        let trusted_broker_registry = self.current_trusted_broker_registry_state()?;
        let derived = derive_receipts(
            effective.document(),
            projection,
            &self.binding.project_root,
            &snapshot_digest,
            now,
            trusted_registry_digest.as_deref(),
        )?;
        let phase = current_phase(projection)?;
        let selected = select_policy(effective.document(), &derived, &phase)?;
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
            effective.document(),
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
                bundle_id: effective
                    .identity()
                    .effective_runtime_bundle
                    .bundle_id
                    .clone(),
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
                    .iter()
                    .filter(|waiver| {
                        selected
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
                        selected.claims.iter().any(|claim| {
                            claim.id == evidence.claim_ref
                                && claim.evaluator_ref == evidence.evaluator_ref
                        })
                    })
                    .cloned()
                    .collect(),
                completion_assertion: WorkflowCompletionAssertion::Asserted,
            },
        };
        let trusted = TrustedWorkflowGovernanceSnapshot::from_trusted_parts(
            effective.document().clone(),
            evaluation,
            snapshot_digest.clone(),
            self.binding.project_id.0.clone(),
            ADAPTER_SOURCE_ID.to_owned(),
        )?;
        let verified = evaluate_verified_workflow_governance(trusted)?;
        let applicability = derived.applicability.get(&selected.id).copied();
        let guidance_status =
            if effective.is_domain_pack_degraded() || !boundary_rechecks.is_empty() {
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
        let mut guidance = WorkflowGovernanceGuidance {
            authority: WorkflowGovernanceGuidanceAuthority::VerifiedProjectSnapshot,
            status: guidance_status,
            project_id: self.binding.project_id.clone(),
            bundle_id: effective
                .identity()
                .effective_runtime_bundle
                .bundle_id
                .clone(),
            bundle_digest: effective
                .identity()
                .effective_runtime_bundle
                .bundle_digest
                .clone(),
            release: Self::release_audit(registry, admitted, projection),
            effective: effective.identity().clone(),
            domain_pack_degraded: effective.is_domain_pack_degraded(),
            domain_pack_gaps: effective.domain_pack_gaps().to_vec(),
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
            authorization: WorkflowAuthorizationGuidance {
                registry_setup: WorkflowAuthorizationRegistrySetup {
                    principal_registry: registry_setup_status(&trusted_registry_digest),
                    broker_registry: trusted_broker_registry.setup,
                },
                setup_gaps: Vec::new(),
                action_packets: Vec::new(),
            },
        };
        let action_packets = authorization_action_packets(
            effective.document(),
            &guidance,
            &derived,
            trusted_registry_digest,
            trusted_broker_registry.digest,
        )?;
        guidance.authorization.setup_gaps = authorization_setup_gaps(
            &self.binding.project_root,
            guidance.authorization.registry_setup.broker_registry,
            &action_packets,
        );
        guidance.authorization.action_packets = action_packets;
        Ok((guidance, verified))
    }

    fn require_active_policy(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        requested_policy_ref: &StableId,
    ) -> Result<ReadinessTarget, WorkflowGovernanceAdapterError> {
        let guidance =
            self.guidance_from_projection(registry, admitted, effective, projection, unix_time()?)?;
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
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
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
            effective.document(),
            projection,
            &self.binding.project_root,
            &snapshot,
            now,
            trusted_registry_digest.as_deref(),
        )?;
        let phase_done = effective
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
    effective_bundle_identity: WorkflowEffectiveBundleIdentity,
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
    /// Project-local effective core-plus-packs identity, kept separate from
    /// the universal reviewed core release audit above.
    pub effective: WorkflowEffectiveBundleIdentity,
    /// True only for a governed remove/rollback generation with no active
    /// packages. The typed gaps below are the actionable recovery surface.
    pub domain_pack_degraded: bool,
    pub domain_pack_gaps: Vec<DomainPackCompositionGap>,
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
    pub effective: WorkflowEffectiveBundleIdentity,
    pub domain_pack_degraded: bool,
    pub domain_pack_gaps: Vec<DomainPackCompositionGap>,
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
    pub authorization: WorkflowAuthorizationGuidance,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseStatus {
    pub active: WorkflowGovernanceReleaseAudit,
    pub effective: WorkflowEffectiveBundleIdentity,
    pub domain_pack_degraded: bool,
    pub domain_pack_gaps: Vec<DomainPackCompositionGap>,
    pub ledger_head_digest: String,
    pub snapshot_digest: String,
    pub state_version: u64,
    pub available_successor: Option<WorkflowGovernanceReleaseIdentity>,
    pub upgrade_argv: Option<Vec<String>>,
    pub domain_pack_rebase_required: bool,
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
    DomainPackLifecycle(DomainPackLifecycleStoreError),
    EffectiveBundle(EffectiveWorkflowGovernanceBundleError),
    Ledger(WorkflowGovernanceLedgerError),
    ActionReplay(WorkflowActionReplayError),
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
    DomainPackRebaseRequired,
    DomainPackCoreMismatch,
    DomainPackGenerationMissing,
    DomainPackGenerationRegression {
        active: u64,
        found: u64,
    },
    DomainPackGenerationFork {
        generation: u64,
    },
    DomainPackCommitIndeterminate,
    DomainPackGapsBlocking(Vec<DomainPackCompositionGap>),
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
            Self::DomainPackLifecycle(error) => {
                write!(f, "Domain Pack lifecycle admission failed: {error}")
            }
            Self::EffectiveBundle(error) => {
                write!(f, "effective workflow bundle admission failed: {error}")
            }
            Self::Ledger(error) => write!(f, "governance ledger failed: {error}"),
            Self::ActionReplay(error) => write!(f, "workflow action replay failed: {error}"),
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
            Self::DomainPackRebaseRequired => f.write_str("DomainPackRebaseRequired: an active Domain Pack generation must be explicitly rebased before workflow release upgrade"),
            Self::DomainPackCoreMismatch => f.write_str("active Domain Pack generation is bound to a different universal workflow core runtime"),
            Self::DomainPackGenerationMissing => f.write_str("workflow ledger has an effective Domain Pack epoch but the lifecycle has no active generation"),
            Self::DomainPackGenerationRegression { active, found } => write!(f, "Domain Pack generation regressed from workflow-ledger generation {active} to lifecycle generation {found}"),
            Self::DomainPackGenerationFork { generation } => write!(f, "Domain Pack generation {generation} conflicts with the effective identity already adopted by the workflow ledger"),
            Self::DomainPackCommitIndeterminate => f.write_str("Domain Pack generation transition recovery did not resolve to the admitted lifecycle identity"),
            Self::DomainPackGapsBlocking(gaps) => {
                let actionable = gaps
                    .iter()
                    .map(|gap| format!("{}: {}", gap.subject_ref.0, gap.message))
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "Domain Pack gaps block workflow mutation: {actionable}")
            }
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
impl From<DomainPackLifecycleStoreError> for WorkflowGovernanceAdapterError {
    fn from(value: DomainPackLifecycleStoreError) -> Self {
        Self::DomainPackLifecycle(value)
    }
}
impl From<EffectiveWorkflowGovernanceBundleError> for WorkflowGovernanceAdapterError {
    fn from(value: EffectiveWorkflowGovernanceBundleError) -> Self {
        Self::EffectiveBundle(value)
    }
}
impl From<WorkflowGovernanceLedgerError> for WorkflowGovernanceAdapterError {
    fn from(value: WorkflowGovernanceLedgerError) -> Self {
        Self::Ledger(value)
    }
}
impl From<WorkflowActionReplayError> for WorkflowGovernanceAdapterError {
    fn from(value: WorkflowActionReplayError) -> Self {
        Self::ActionReplay(value)
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

fn classify_domain_pack_transition_recovery(
    projection: &WorkflowGovernanceLedgerProjection,
    source: &WorkflowEffectiveBundleIdentity,
    target: &WorkflowEffectiveBundleIdentity,
    prior_head: &str,
    transition_state_version: u64,
) -> DomainPackTransitionRecovery {
    if projection.active_effective_bundle_identity().as_ref() == Some(target) {
        let exact_transition = projection.records.last().is_some_and(|record| {
            record.state_version == transition_state_version
                && record.previous_record_digest.as_deref() == Some(prior_head)
                && projection.head_digest.as_deref() == Some(record.record_digest.as_str())
                && matches!(
                    &record.event,
                    WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event)
                        if event.from_effective_bundle == *source
                            && event.to_effective_bundle == *target
                            && event.prior_ledger_head_digest == prior_head
                )
        });
        return if exact_transition {
            DomainPackTransitionRecovery::TargetCommitted
        } else {
            DomainPackTransitionRecovery::Indeterminate
        };
    }

    let source_is_active = projection
        .active_effective_bundle_identity()
        .as_ref()
        .map_or_else(
            || {
                source.domain_pack_generation.is_none()
                    && source.core_runtime_bundle == source.effective_runtime_bundle
            },
            |active| active == source,
        );
    let source_state_unchanged = projection.head_digest.as_deref() == Some(prior_head)
        && projection
            .current_state_version()
            .and_then(|version| version.checked_add(1))
            == Some(transition_state_version);
    if source_is_active && source_state_unchanged {
        DomainPackTransitionRecovery::SourceUnchanged
    } else {
        DomainPackTransitionRecovery::Indeterminate
    }
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
        let carryover = match &record.event {
            WorkflowGovernanceEvent::ReleaseUpgraded(event) => Some((
                event.receipt_carryover,
                event.from_runtime_bundle.policy_set_digest
                    == event.to_runtime_bundle.policy_set_digest,
            )),
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event) => Some((
                event.receipt_carryover,
                event.from_effective_bundle.core_runtime_bundle
                    == event.to_effective_bundle.core_runtime_bundle
                    && event.from_effective_bundle.effective_runtime_bundle
                        == event.to_effective_bundle.effective_runtime_bundle
                    && event.from_effective_bundle.receipt_context_digest
                        == event.to_effective_bundle.receipt_context_digest,
            )),
            _ => None,
        };
        if let Some((carryover, exactly_equivalent)) = carryover {
            match carryover {
                WorkflowReceiptCarryover::PreservePolicyEquivalent if exactly_equivalent => {}
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

fn broker_semantic_input_to_closed(
    input: WorkflowBrokerSemanticInput,
) -> WorkflowAuthorizationClosedInput {
    match input {
        WorkflowBrokerSemanticInput::Applicability {
            applicable,
            basis_refs,
        } => WorkflowAuthorizationClosedInput::Applicability {
            applicable,
            basis_refs,
        },
        WorkflowBrokerSemanticInput::Capability {
            available,
            probe_ref,
            subject_kind,
            subject_ref,
        } => WorkflowAuthorizationClosedInput::Capability {
            available,
            probe_ref,
            subject_kind,
            subject_ref,
        },
        WorkflowBrokerSemanticInput::Decision {
            selected_alternative_ref,
        } => WorkflowAuthorizationClosedInput::Decision {
            selected_alternative_ref,
        },
        WorkflowBrokerSemanticInput::Evidence {
            outcome,
            subject_kind,
            subject_ref,
            scenario_ref,
        } => WorkflowAuthorizationClosedInput::Evidence {
            outcome,
            subject_kind,
            subject_ref,
            scenario_ref,
        },
        WorkflowBrokerSemanticInput::Signal { active, basis_refs } => {
            WorkflowAuthorizationClosedInput::Signal { active, basis_refs }
        }
        WorkflowBrokerSemanticInput::Waiver { reason } => {
            WorkflowAuthorizationClosedInput::Waiver { reason }
        }
    }
}

fn validate_broker_packet_audit(
    packet: &WorkflowAuthorizationActionPacket,
    input: &WorkflowBrokerSemanticInput,
    audit: &VerifiedWorkflowBrokerEventAudit,
    broker_registry_digest: &str,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let expected_kind = match packet.authorization_kind {
        WorkflowAuthorizationKind::Applicability => WorkflowBrokerEventKind::Applicability,
        WorkflowAuthorizationKind::Capability => WorkflowBrokerEventKind::Capability,
        WorkflowAuthorizationKind::Decision => WorkflowBrokerEventKind::Decision,
        WorkflowAuthorizationKind::Evidence => WorkflowBrokerEventKind::Evidence,
        WorkflowAuthorizationKind::Signal => WorkflowBrokerEventKind::Signal,
        WorkflowAuthorizationKind::Waiver => WorkflowBrokerEventKind::Waiver,
    };
    let input_kind = match input {
        WorkflowBrokerSemanticInput::Applicability { .. } => WorkflowBrokerEventKind::Applicability,
        WorkflowBrokerSemanticInput::Capability { .. } => WorkflowBrokerEventKind::Capability,
        WorkflowBrokerSemanticInput::Decision { .. } => WorkflowBrokerEventKind::Decision,
        WorkflowBrokerSemanticInput::Evidence { .. } => WorkflowBrokerEventKind::Evidence,
        WorkflowBrokerSemanticInput::Signal { .. } => WorkflowBrokerEventKind::Signal,
        WorkflowBrokerSemanticInput::Waiver { .. } => WorkflowBrokerEventKind::Waiver,
    };
    let profile_allowed = match packet.required_authority.approval_boundary {
        WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker => {
            audit.issuer_profile == WorkflowBrokerIssuerProfile::Human
        }
        WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker => {
            audit.issuer_profile == WorkflowBrokerIssuerProfile::Reviewer
        }
        WorkflowAuthorizationApprovalBoundary::TrustedRuntimeBroker => {
            audit.issuer_profile == WorkflowBrokerIssuerProfile::Runtime
        }
        WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker => matches!(
            audit.issuer_profile,
            WorkflowBrokerIssuerProfile::Reviewer | WorkflowBrokerIssuerProfile::Runtime
        ),
    };
    if audit.action_packet_digest != packet.packet_digest
        || audit.project_id != packet.binding.project_id
        || audit.event_kind != expected_kind
        || input_kind != expected_kind
        || !profile_allowed
        || packet.binding.trusted_broker_registry_digest.as_deref() != Some(broker_registry_digest)
    {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    }
    Ok(())
}

fn bound_prepared_expiry(
    prepared: &mut PreparedWorkflowAuthorization,
    broker_expires_at_unix: u64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    match prepared {
        PreparedWorkflowAuthorization::Applicability { request, .. } => {
            request.expires_at_unix = request.expires_at_unix.min(broker_expires_at_unix);
        }
        PreparedWorkflowAuthorization::Capability { request, .. } => {
            request.expires_at_unix = request
                .expires_at_unix
                .map(|expires| expires.min(broker_expires_at_unix));
        }
        PreparedWorkflowAuthorization::Evidence { request, .. } => {
            request.expires_at_unix = request
                .expires_at_unix
                .map(|expires| expires.min(broker_expires_at_unix));
        }
        PreparedWorkflowAuthorization::Signal { request, .. } => {
            request.expires_at_unix = request.expires_at_unix.min(broker_expires_at_unix);
        }
        PreparedWorkflowAuthorization::Waiver { request, .. } => {
            let broker_expiry = i64::try_from(broker_expires_at_unix)
                .map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?;
            request.expires_at_unix = request.expires_at_unix.min(broker_expiry);
        }
        PreparedWorkflowAuthorization::Decision { .. } => {}
    }
    Ok(())
}

fn broker_action_event_from_prepared(
    bundle: &WorkflowGovernanceBundleDocument,
    project_root: &Path,
    prepared: PreparedWorkflowAuthorization,
    audit: &VerifiedWorkflowBrokerEventAudit,
    broker_registry_digest: &str,
) -> Result<
    (
        WorkflowAuthorizationActionPacket,
        WorkflowGovernanceEvent,
        bool,
    ),
    WorkflowGovernanceAdapterError,
> {
    let issuer = audit.issuer_id.clone();
    let fingerprint = audit.public_key_fingerprint.clone();
    let registry_digest = broker_registry_digest.to_owned();
    let origin = audit.origin_principal_id.clone();
    match prepared {
        PreparedWorkflowAuthorization::Applicability { request, packet } => {
            let basis = content_addressed_basis_from_paths(project_root, &request.basis_refs)?;
            if content_addressed_basis_digest(&basis)? != request.basis_digest {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            let event = ApplicabilityAssessedEvent {
                policy_ref: request.policy_ref,
                applicable: request.applicable,
                assessed_by: origin,
                evaluator_ref: request.evaluator_ref,
                credential_id: issuer,
                public_key_fingerprint: fingerprint,
                authorization_registry_digest: registry_digest,
                basis,
                basis_digest: request.basis_digest,
                snapshot_digest: request.snapshot_digest,
                ledger_head_digest: request.ledger_head_digest,
                observed_at_unix: request.observed_at_unix,
                expires_at_unix: request.expires_at_unix,
            };
            Ok((
                packet,
                WorkflowGovernanceEvent::ApplicabilityAssessed(event),
                true,
            ))
        }
        PreparedWorkflowAuthorization::Capability { request, packet } => Ok((
            packet,
            WorkflowGovernanceEvent::CapabilityProbed(CapabilityProbedEvent {
                policy_ref: request.policy_ref,
                capability_ref: request.capability_ref,
                probe_kind: request.probe_kind,
                credential_id: issuer,
                public_key_fingerprint: fingerprint,
                authorization_registry_digest: registry_digest,
                available: request.available,
                probe_ref: request.probe_ref,
                probe_digest: request.probe_digest,
                subject: WorkflowEvidenceSubject {
                    kind: request.subject_kind,
                    subject_ref: request.subject_ref,
                    subject_digest: request.subject_digest,
                },
                snapshot_digest: request.snapshot_digest,
                ledger_head_digest: request.ledger_head_digest,
                observed_at_unix: request.observed_at_unix,
                expires_at_unix: request.expires_at_unix,
            }),
            false,
        )),
        PreparedWorkflowAuthorization::Decision { request, packet } => Ok((
            packet,
            WorkflowGovernanceEvent::DecisionResolved(DecisionResolvedEvent {
                policy_ref: request.policy_ref,
                decision_ref: request.decision_ref,
                selected_alternative_ref: request.selected_alternative_ref,
                principal: origin,
                authority_scope: StableId("workflow.decision.resolve".to_owned()),
                credential_id: issuer,
                public_key_fingerprint: fingerprint,
                authorization_registry_digest: registry_digest,
                snapshot_digest: request.snapshot_digest,
                ledger_head_digest: request.ledger_head_digest,
                authorization_intent_digest: audit.event_digest.clone(),
                signature_fingerprint: audit.signature_fingerprint.clone(),
                resolved_at_unix: audit.issued_at_unix,
            }),
            false,
        )),
        PreparedWorkflowAuthorization::Evidence { request, packet } => {
            let semantic_basis = serde_json::json!({
                "packet_digest": packet.packet_digest,
                "broker_event_digest": audit.event_digest,
                "origin_principal_id": audit.origin_principal_id,
                "subject_digest": request.subject_digest,
                "scenario_digest": request.scenario_digest,
            });
            let semantic_digest =
                sha256_content_hash(&serde_json_canonicalizer::to_vec(&semantic_basis).map_err(
                    |error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()),
                )?);
            Ok((
                packet,
                WorkflowGovernanceEvent::EvaluatorObserved(EvaluatorObservedEvent {
                    policy_ref: request.policy_ref,
                    claim_ref: request.claim_ref,
                    evaluator_ref: request.evaluator_ref,
                    provider: request.provider,
                    credential_id: issuer,
                    public_key_fingerprint: fingerprint,
                    authorization_registry_digest: registry_digest,
                    kind: request.kind,
                    strength: request.strength,
                    outcome: request.outcome,
                    provenance: WorkflowEvidenceProvenance {
                        source_ref: request.subject_ref.clone(),
                        source_digest: request.subject_digest.clone(),
                        scenario_digest: request.scenario_digest,
                        semantic_identity: StableId(format!(
                            "evidence.broker.{}",
                            semantic_digest.trim_start_matches("sha256:")
                        )),
                        producer_ref: audit.issuer_id.clone(),
                        principal: Some(audit.origin_principal_id.clone()),
                        method: format!("verified_workflow_broker:{}", audit.event_digest),
                    },
                    subject: WorkflowEvidenceSubject {
                        kind: request.subject_kind,
                        subject_ref: request.subject_ref,
                        subject_digest: request.subject_digest,
                    },
                    snapshot_digest: request.snapshot_digest,
                    ledger_head_digest: request.ledger_head_digest,
                    observed_at_unix: request.observed_at_unix,
                    expires_at_unix: request.expires_at_unix,
                }),
                false,
            ))
        }
        PreparedWorkflowAuthorization::Signal { request, packet } => {
            let basis = content_addressed_basis_from_paths(project_root, &request.basis_refs)?;
            if content_addressed_basis_digest(&basis)? != request.basis_digest {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            Ok((
                packet,
                WorkflowGovernanceEvent::SignalChanged(SignalChangedEvent {
                    signal: request.signal,
                    active: request.active,
                    episode_id: request.episode_id,
                    generation: request.generation,
                    changed_by: origin,
                    credential_id: issuer,
                    public_key_fingerprint: fingerprint,
                    authorization_registry_digest: registry_digest,
                    basis,
                    basis_digest: request.basis_digest,
                    snapshot_digest: request.snapshot_digest,
                    ledger_head_digest: request.ledger_head_digest,
                    observed_at_unix: request.observed_at_unix,
                    expires_at_unix: request.expires_at_unix,
                }),
                true,
            ))
        }
        PreparedWorkflowAuthorization::Waiver { request, packet } => {
            let claim_ref = match request.subject {
                WorkflowWaiverSubject::Claim { claim_ref } => claim_ref,
                WorkflowWaiverSubject::Obligation { .. } => {
                    return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
                }
            };
            let policy = policy_by_id(bundle, &request.policy_ref)?;
            let claim = policy
                .claims
                .iter()
                .find(|claim| claim.id == claim_ref)
                .ok_or_else(|| WorkflowGovernanceAdapterError::UnknownClaim(claim_ref.0.clone()))?;
            let WorkflowClaimWaiverPolicy::Authorized {
                authority_scope, ..
            } = &claim.waiver
            else {
                return Err(WorkflowGovernanceAdapterError::WaiverNotAllowed);
            };
            Ok((
                packet,
                WorkflowGovernanceEvent::WaiverAuthorized(WaiverAuthorizedEvent {
                    policy_ref: request.policy_ref,
                    claim_ref,
                    principal: origin,
                    authority_scope: authority_scope.clone(),
                    credential_id: issuer,
                    public_key_fingerprint: fingerprint,
                    authorization_registry_digest: registry_digest,
                    max_target: parse_readiness(&request.maximum_readiness_target)?,
                    subject: WorkflowEvidenceSubject {
                        kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                        subject_ref: audit.project_id.0.clone(),
                        subject_digest: request.snapshot_digest.clone(),
                    },
                    snapshot_digest: request.snapshot_digest,
                    ledger_head_digest: request.ledger_head_digest,
                    authorization_intent_digest: audit.event_digest.clone(),
                    signature_fingerprint: audit.signature_fingerprint.clone(),
                    consequences_digest: request.consequences_ack_digest,
                    authorized_at_unix: audit.issued_at_unix,
                    expires_at_unix: u64::try_from(request.expires_at_unix)
                        .map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?,
                }),
                false,
            ))
        }
    }
}

fn broker_origin_applied_event(
    packet: &WorkflowAuthorizationActionPacket,
    audit: &VerifiedWorkflowBrokerEventAudit,
    broker_registry_digest: &str,
    action_record: &WorkflowGovernanceLedgerRecord,
) -> BrokerOriginAppliedEvent {
    BrokerOriginAppliedEvent {
        action_packet_digest: packet.packet_digest.clone(),
        broker_event_digest: audit.event_digest.clone(),
        action_record_digest: action_record.record_digest.clone(),
        origin_principal_id: audit.origin_principal_id.clone(),
        separation_domain: audit.separation_domain.clone(),
        nonce_fingerprint: audit.replay_key.nonce_fingerprint.clone(),
        issuer_id: audit.issuer_id.clone(),
        issuer_profile: match audit.issuer_profile {
            WorkflowBrokerIssuerProfile::Human => WorkflowBrokerOriginProfile::Human,
            WorkflowBrokerIssuerProfile::Reviewer => WorkflowBrokerOriginProfile::Reviewer,
            WorkflowBrokerIssuerProfile::Runtime => WorkflowBrokerOriginProfile::Runtime,
        },
        public_key_fingerprint: audit.public_key_fingerprint.clone(),
        signature_fingerprint: audit.signature_fingerprint.clone(),
        enrollment_ceremony_digest: audit.enrollment_ceremony_digest.clone(),
        broker_registry_digest: broker_registry_digest.to_owned(),
        issued_at_unix: audit.issued_at_unix,
        expires_at_unix: audit.expires_at_unix,
    }
}

fn matching_broker_origin_retry(
    projection: &WorkflowGovernanceLedgerProjection,
    audit: &VerifiedWorkflowBrokerEventAudit,
) -> Result<
    Option<(
        WorkflowGovernanceLedgerRecord,
        WorkflowGovernanceLedgerRecord,
    )>,
    WorkflowGovernanceAdapterError,
> {
    for origin_record in &projection.records {
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &origin_record.event else {
            continue;
        };
        let packet_matches = origin.action_packet_digest == audit.action_packet_digest;
        let event_matches = origin.broker_event_digest == audit.event_digest;
        let origin_identity_matches = origin.issuer_id == audit.issuer_id
            && origin.nonce_fingerprint == audit.replay_key.nonce_fingerprint
            && origin.origin_principal_id == audit.origin_principal_id
            && origin.separation_domain == audit.separation_domain;
        if !packet_matches && !event_matches && !origin_identity_matches {
            continue;
        }
        let profile = match audit.issuer_profile {
            WorkflowBrokerIssuerProfile::Human => WorkflowBrokerOriginProfile::Human,
            WorkflowBrokerIssuerProfile::Reviewer => WorkflowBrokerOriginProfile::Reviewer,
            WorkflowBrokerIssuerProfile::Runtime => WorkflowBrokerOriginProfile::Runtime,
        };
        if !(packet_matches
            && event_matches
            && origin.origin_principal_id == audit.origin_principal_id
            && origin.separation_domain == audit.separation_domain
            && origin.nonce_fingerprint == audit.replay_key.nonce_fingerprint
            && origin.issuer_id == audit.issuer_id
            && origin.issuer_profile == profile
            && origin.public_key_fingerprint == audit.public_key_fingerprint
            && origin.signature_fingerprint == audit.signature_fingerprint
            && origin.enrollment_ceremony_digest == audit.enrollment_ceremony_digest
            && origin.issued_at_unix == audit.issued_at_unix
            && origin.expires_at_unix == audit.expires_at_unix)
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let action_record = projection
            .records
            .iter()
            .find(|record| record.record_digest == origin.action_record_digest)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        if origin_record.previous_record_digest.as_deref()
            != Some(action_record.record_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        return Ok(Some((action_record.clone(), origin_record.clone())));
    }
    Ok(None)
}

fn broker_replay_origin_id(
    audit: &VerifiedWorkflowBrokerEventAudit,
) -> Result<String, WorkflowGovernanceAdapterError> {
    let identity = serde_json::json!({
        "schema_version": "workflow_broker_replay_origin_v1",
        "issuer_id": audit.issuer_id,
        "nonce_fingerprint": audit.replay_key.nonce_fingerprint,
        "origin_principal_id": audit.origin_principal_id,
        "separation_domain": audit.separation_domain,
    });
    let canonical = serde_json_canonicalizer::to_vec(&identity)
        .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
    Ok(format!(
        "broker-origin:{}",
        sha256_content_hash(&canonical).trim_start_matches("sha256:")
    ))
}

fn ensure_broker_replay_committed(
    state_root: &Path,
    packet_digest: &str,
    replay_origin_id: &str,
    action_record_digest: &str,
) -> Result<bool, WorkflowGovernanceAdapterError> {
    let reservation = reserve_workflow_action(
        state_root,
        packet_digest,
        replay_origin_id,
        action_record_digest,
    )?;
    let commit = commit_workflow_action(
        state_root,
        packet_digest,
        replay_origin_id,
        action_record_digest,
    )?;
    Ok(reservation.appended || commit.appended)
}

fn prepare_authorization_from_packet(
    bundle: &WorkflowGovernanceBundleDocument,
    projection: &WorkflowGovernanceLedgerProjection,
    project_root: &Path,
    packet: WorkflowAuthorizationActionPacket,
    input: WorkflowAuthorizationClosedInput,
    now: u64,
) -> Result<PreparedWorkflowAuthorization, WorkflowGovernanceAdapterError> {
    let policy = policy_by_id(bundle, &packet.binding.policy_ref)?;
    let contract_kind = match &packet.input_contract {
        WorkflowAuthorizationInputContract::Applicability { .. } => {
            WorkflowAuthorizationKind::Applicability
        }
        WorkflowAuthorizationInputContract::Capability { .. } => {
            WorkflowAuthorizationKind::Capability
        }
        WorkflowAuthorizationInputContract::Decision { .. } => WorkflowAuthorizationKind::Decision,
        WorkflowAuthorizationInputContract::Evidence { .. } => WorkflowAuthorizationKind::Evidence,
        WorkflowAuthorizationInputContract::Signal { .. } => WorkflowAuthorizationKind::Signal,
        WorkflowAuthorizationInputContract::Waiver { .. } => WorkflowAuthorizationKind::Waiver,
    };
    if packet.authorization_kind != contract_kind {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    }
    let expires = |ttl: u64| {
        now.checked_add(ttl)
            .ok_or(WorkflowGovernanceAdapterError::ClockOverflow)
    };
    match (packet.input_contract.clone(), input) {
        (
            WorkflowAuthorizationInputContract::Applicability {
                basis_refs_min_items,
                basis_refs_repo_relative: true,
            },
            WorkflowAuthorizationClosedInput::Applicability {
                applicable,
                basis_refs,
            },
        ) => {
            let basis = content_addressed_basis_from_paths(project_root, &basis_refs)?;
            if basis.len() < basis_refs_min_items {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            let request = WorkflowApplicabilityAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                policy_ref: packet.binding.policy_ref.clone(),
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                applicable,
                evaluator_ref: StableId(WORKFLOW_APPLICABILITY_EVALUATOR_REF.to_owned()),
                authority_scope: StableId(WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE.to_owned()),
                basis_refs: basis
                    .iter()
                    .map(|reference| reference.subject_ref.clone())
                    .collect(),
                basis_digest: content_addressed_basis_digest(&basis)?,
                observed_at_unix: now,
                expires_at_unix: expires(WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS)?,
            };
            Ok(PreparedWorkflowAuthorization::Applicability { request, packet })
        }
        (
            WorkflowAuthorizationInputContract::Capability {
                capability_ref,
                probe_kind,
                subject_kinds,
                probe_reference_required: true,
            },
            WorkflowAuthorizationClosedInput::Capability {
                available,
                probe_ref,
                subject_kind,
                subject_ref,
            },
        ) if subject_kinds.contains(&subject_kind) => {
            let (probe_ref, probe_bytes) = read_confined_file(project_root, Path::new(&probe_ref))?;
            let (subject_ref, subject_digest) = confined_subject_reference(
                project_root,
                &packet.binding.project_id,
                &packet.binding.snapshot_digest,
                subject_kind,
                &subject_ref,
            )?;
            let request = WorkflowCapabilityAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                policy_ref: packet.binding.policy_ref.clone(),
                capability_ref,
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                probe_kind,
                available,
                authority_scope: StableId(WORKFLOW_CAPABILITY_AUTHORITY_SCOPE.to_owned()),
                probe_ref,
                probe_digest: sha256_content_hash(&probe_bytes),
                subject_kind,
                subject_ref,
                subject_digest,
                observed_at_unix: now,
                expires_at_unix: Some(expires(WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS)?),
            };
            Ok(PreparedWorkflowAuthorization::Capability { request, packet })
        }
        (
            WorkflowAuthorizationInputContract::Decision {
                decision_ref,
                alternatives,
                ..
            },
            WorkflowAuthorizationClosedInput::Decision {
                selected_alternative_ref,
            },
        ) => {
            let selected = alternatives
                .iter()
                .find(|candidate| candidate.id == selected_alternative_ref)
                .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
            let consequences_ack_digest = decision_consequences_ack_digest(
                &packet.packet_digest,
                &decision_ref,
                &selected_alternative_ref,
                &selected.consequences,
            )?;
            let request = WorkflowDecisionAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                policy_ref: packet.binding.policy_ref.clone(),
                decision_ref,
                selected_alternative_ref,
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                readiness_target: readiness_target_label(packet.binding.readiness_target)
                    .to_owned(),
                consequences_ack_digest,
            };
            Ok(PreparedWorkflowAuthorization::Decision { request, packet })
        }
        (
            WorkflowAuthorizationInputContract::Evidence {
                claim_ref,
                evaluator_ref,
                provider,
                evidence_kind,
                strength,
                allowed_outcomes,
                subject_kinds,
                scenario_reference_required: true,
            },
            WorkflowAuthorizationClosedInput::Evidence {
                outcome,
                subject_kind,
                subject_ref,
                scenario_ref,
            },
        ) if allowed_outcomes.contains(&outcome) && subject_kinds.contains(&subject_kind) => {
            let evaluator = policy
                .evaluators
                .iter()
                .find(|candidate| candidate.id == evaluator_ref)
                .ok_or_else(|| {
                    WorkflowGovernanceAdapterError::UnknownEvaluator(evaluator_ref.0.clone())
                })?;
            let (subject_ref, subject_digest) = confined_subject_reference(
                project_root,
                &packet.binding.project_id,
                &packet.binding.snapshot_digest,
                subject_kind,
                &subject_ref,
            )?;
            let (_, scenario_bytes) = read_confined_file(project_root, Path::new(&scenario_ref))?;
            let request = WorkflowEvidenceAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                policy_ref: packet.binding.policy_ref.clone(),
                claim_ref,
                evaluator_ref,
                provider,
                kind: evidence_kind,
                strength,
                outcome,
                subject_kind,
                subject_ref,
                subject_digest,
                scenario_digest: sha256_content_hash(&scenario_bytes),
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                readiness_target: packet.binding.readiness_target,
                observed_at_unix: now,
                expires_at_unix: Some(expires(evaluator.max_age_seconds)?),
            };
            Ok(PreparedWorkflowAuthorization::Evidence { request, packet })
        }
        (
            WorkflowAuthorizationInputContract::Signal {
                signal,
                transition,
                basis_refs_min_items,
                basis_refs_repo_relative: true,
            },
            WorkflowAuthorizationClosedInput::Signal { active, basis_refs },
        ) if active == matches!(transition, WorkflowSignalInputTransition::Activate) => {
            let basis = content_addressed_basis_from_paths(project_root, &basis_refs)?;
            if basis.len() < basis_refs_min_items {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            let prior = projection.records.iter().rev().find_map(|record| {
                if let WorkflowGovernanceEvent::SignalChanged(event) = &record.event {
                    (event.signal == signal).then_some(event)
                } else {
                    None
                }
            });
            let (episode_id, generation) = match (active, prior) {
                (true, None) => (signal_episode_id(&packet, signal, 1)?, 1),
                (false, Some(previous)) if previous.active => {
                    (previous.episode_id.clone(), previous.generation)
                }
                (true, Some(previous)) if !previous.active => {
                    let generation = previous
                        .generation
                        .checked_add(1)
                        .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
                    (signal_episode_id(&packet, signal, generation)?, generation)
                }
                _ => return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch),
            };
            let request = WorkflowSignalAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                signal,
                active,
                episode_id,
                generation,
                basis_refs: basis
                    .iter()
                    .map(|reference| reference.subject_ref.clone())
                    .collect(),
                basis_digest: content_addressed_basis_digest(&basis)?,
                observed_at_unix: now,
                expires_at_unix: expires(WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS)?,
            };
            Ok(PreparedWorkflowAuthorization::Signal { request, packet })
        }
        (
            WorkflowAuthorizationInputContract::Waiver {
                claim_ref,
                maximum_readiness_target,
                max_age_seconds,
                reason_required: true,
                consequence_statements,
            },
            WorkflowAuthorizationClosedInput::Waiver { reason },
        ) if !reason.trim().is_empty() => {
            let acknowledgement = serde_json::json!({
                "schema_version": "workflow_waiver_consequence_ack_v1",
                "packet_digest": packet.packet_digest,
                "claim_ref": claim_ref,
                "consequences": consequence_statements,
            });
            let consequences_ack_digest =
                sha256_content_hash(&serde_json_canonicalizer::to_vec(&acknowledgement).map_err(
                    |error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()),
                )?);
            let expires_at_unix = i64::try_from(expires(max_age_seconds)?)
                .map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?;
            let request = WorkflowWaiverAuthorizationRequest {
                project_id: packet.binding.project_id.clone(),
                policy_bundle_digest: packet.binding.effective_bundle_digest.clone(),
                policy_ref: packet.binding.policy_ref.clone(),
                subject: WorkflowWaiverSubject::Claim { claim_ref },
                state_version: packet.binding.state_version,
                current_phase: packet.binding.current_phase.clone(),
                snapshot_digest: packet.binding.snapshot_digest.clone(),
                ledger_head_digest: packet.binding.ledger_head_digest.clone(),
                maximum_readiness_target: readiness_target_label(maximum_readiness_target)
                    .to_owned(),
                reason: reason.trim().to_owned(),
                consequences_ack_digest,
                expires_at_unix,
            };
            Ok(PreparedWorkflowAuthorization::Waiver { request, packet })
        }
        _ => Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch),
    }
}

fn confined_subject_reference(
    project_root: &Path,
    project_id: &StableId,
    snapshot_digest: &str,
    subject_kind: WorkflowEvidenceSubjectKind,
    subject_ref: &str,
) -> Result<(String, String), WorkflowGovernanceAdapterError> {
    match subject_kind {
        WorkflowEvidenceSubjectKind::Artifact => {
            let (subject_ref, bytes) = read_confined_file(project_root, Path::new(subject_ref))?;
            Ok((subject_ref, sha256_content_hash(&bytes)))
        }
        WorkflowEvidenceSubjectKind::RepositoryState
        | WorkflowEvidenceSubjectKind::ProjectSnapshot => {
            if subject_ref != project_id.0 {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            Ok((project_id.0.clone(), snapshot_digest.to_owned()))
        }
        WorkflowEvidenceSubjectKind::Runtime
        | WorkflowEvidenceSubjectKind::ExternalSystem
        | WorkflowEvidenceSubjectKind::HumanDecision => {
            let subject_ref = subject_ref.trim();
            if subject_ref.is_empty() {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            let basis = serde_json::json!({
                "schema_version": "workflow_broker_subject_identity_v1",
                "subject_kind": subject_kind,
                "subject_ref": subject_ref,
            });
            let canonical = serde_json_canonicalizer::to_vec(&basis).map_err(|error| {
                WorkflowGovernanceAdapterError::Canonicalization(error.to_string())
            })?;
            Ok((subject_ref.to_owned(), sha256_content_hash(&canonical)))
        }
    }
}

fn signal_episode_id(
    packet: &WorkflowAuthorizationActionPacket,
    signal: WorkflowGovernanceSignal,
    generation: u64,
) -> Result<StableId, WorkflowGovernanceAdapterError> {
    let basis = serde_json::json!({
        "schema_version": "workflow_signal_episode_v1",
        "packet_digest": packet.packet_digest,
        "signal": signal,
        "generation": generation,
    });
    let digest =
        sha256_content_hash(&serde_json_canonicalizer::to_vec(&basis).map_err(|error| {
            WorkflowGovernanceAdapterError::Canonicalization(error.to_string())
        })?);
    Ok(StableId(format!(
        "episode.workflow.{}",
        digest.trim_start_matches("sha256:")
    )))
}

fn decision_consequences_ack_digest(
    packet_digest: &str,
    decision_ref: &StableId,
    selected_alternative_ref: &StableId,
    consequences: &[String],
) -> Result<String, WorkflowGovernanceAdapterError> {
    let acknowledgement = serde_json::json!({
        "schema_version": "workflow_decision_consequence_ack_v1",
        "packet_digest": packet_digest,
        "decision_ref": decision_ref,
        "selected_alternative_ref": selected_alternative_ref,
        "consequences": consequences,
    });
    let canonical = serde_json_canonicalizer::to_vec(&acknowledgement)
        .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
    Ok(sha256_content_hash(&canonical))
}

fn authorization_action_packets(
    bundle: &WorkflowGovernanceBundleDocument,
    guidance: &WorkflowGovernanceGuidance,
    derived: &DerivedReceipts,
    trusted_principal_registry_digest: Option<String>,
    trusted_broker_registry_digest: Option<String>,
) -> Result<Vec<WorkflowAuthorizationActionPacket>, WorkflowGovernanceAdapterError> {
    let selected = policy_by_id(bundle, &guidance.selected_policy_ref)?;
    let binding_for = |policy: &WorkflowGovernancePolicy, subject_ref: StableId| {
        WorkflowAuthorizationPacketBinding {
            project_id: guidance.project_id.clone(),
            effective_bundle_id: guidance
                .effective
                .effective_runtime_bundle
                .bundle_id
                .clone(),
            effective_bundle_digest: guidance
                .effective
                .effective_runtime_bundle
                .bundle_digest
                .clone(),
            policy_ref: policy.id.clone(),
            subject_ref,
            state_version: guidance.state_version,
            current_phase: StableId(guidance.current_phase.clone()),
            snapshot_digest: guidance.snapshot_digest.clone(),
            ledger_head_digest: guidance.ledger_head_digest.clone(),
            trusted_principal_registry_digest: trusted_principal_registry_digest.clone(),
            trusted_broker_registry_digest: trusted_broker_registry_digest.clone(),
            readiness_target: policy.routing.readiness_target,
        }
    };
    let mut packets = Vec::new();

    if guidance.status == WorkflowGovernanceGuidanceStatus::ApplicabilityRequired {
        packets.push(make_authorization_action_packet(
            WorkflowAuthorizationKind::Applicability,
            StableId(format!("packet.workflow.applicability.{}", selected.id.0)),
            binding_for(selected, selected.id.clone()),
            human_authority("workflow.applicability.assess"),
            WorkflowAuthorizationInputContract::Applicability {
                basis_refs_min_items: 1,
                basis_refs_repo_relative: true,
            },
        )?);
    }

    for gap in &guidance.simulation.candidate_capability_gaps {
        let requirement = selected
            .capability_requirements
            .iter()
            .find(|candidate| candidate.id == gap.id)
            .ok_or_else(|| WorkflowGovernanceAdapterError::UnknownCapability(gap.id.0.clone()))?;
        packets.push(make_authorization_action_packet(
            WorkflowAuthorizationKind::Capability,
            StableId(format!("packet.workflow.capability.{}", requirement.id.0)),
            binding_for(selected, requirement.id.clone()),
            runtime_authority("workflow.capability.authorize"),
            WorkflowAuthorizationInputContract::Capability {
                capability_ref: requirement.id.clone(),
                probe_kind: requirement.probe_kind,
                subject_kinds: capability_subject_kinds(requirement.probe_kind),
                probe_reference_required: true,
            },
        )?);
    }

    for request in &guidance.simulation.candidate_decision_requests {
        packets.push(make_authorization_action_packet(
            WorkflowAuthorizationKind::Decision,
            StableId(format!("packet.workflow.decision.{}", request.id.0)),
            binding_for(selected, request.id.clone()),
            human_authority("workflow.decision.resolve"),
            WorkflowAuthorizationInputContract::Decision {
                decision_ref: request.id.clone(),
                alternatives: request.alternatives.clone(),
                recommended_alternative_ref: request.recommended_alternative_ref.clone(),
            },
        )?);
    }

    for claim in &selected.claims {
        let result = guidance
            .simulation
            .candidate_claim_results
            .iter()
            .find(|candidate| candidate.claim_id == claim.id.0)
            .ok_or_else(|| WorkflowGovernanceAdapterError::UnknownClaim(claim.id.0.clone()))?;
        if matches!(
            result.status,
            WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
        ) {
            continue;
        }
        let evaluator = selected
            .evaluators
            .iter()
            .find(|candidate| candidate.id == claim.evaluator_ref)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownEvaluator(claim.evaluator_ref.0.clone())
            })?;
        let (required_authority, evidence_kind, strength, subject_kinds) =
            evidence_action_contract(evaluator.provider);
        if !evaluator.accepted_evidence_kinds.contains(&evidence_kind)
            || strength < evaluator.minimum_strength
        {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(format!(
                "evaluator {} is incompatible with the closed {:?} authority contract",
                evaluator.id.0, evaluator.provider
            )));
        }
        packets.push(make_authorization_action_packet(
            WorkflowAuthorizationKind::Evidence,
            StableId(format!("packet.workflow.evidence.{}", claim.id.0)),
            binding_for(selected, claim.id.clone()),
            required_authority,
            WorkflowAuthorizationInputContract::Evidence {
                claim_ref: claim.id.clone(),
                evaluator_ref: evaluator.id.clone(),
                provider: evaluator.provider,
                evidence_kind,
                strength,
                allowed_outcomes: vec![
                    WorkflowEvidenceOutcome::Pass,
                    WorkflowEvidenceOutcome::Fail,
                    WorkflowEvidenceOutcome::Inconclusive,
                ],
                subject_kinds,
                scenario_reference_required: true,
            },
        )?);

        if let WorkflowClaimWaiverPolicy::Authorized {
            max_target,
            max_age_seconds,
            ..
        } = &claim.waiver
        {
            let maximum_readiness_target =
                if max_target.rank() < selected.routing.readiness_target.rank() {
                    *max_target
                } else {
                    selected.routing.readiness_target
                };
            let mut consequence_statements = vec![format!(
                "Claim {} will be treated as waived without verified evidence: {}",
                claim.id.0, claim.statement
            )];
            let mut obligations = selected
                .obligations
                .iter()
                .filter(|obligation| obligation.claim_refs.contains(&claim.id))
                .collect::<Vec<_>>();
            obligations.sort_by(|left, right| left.id.cmp(&right.id));
            consequence_statements.extend(obligations.into_iter().map(|obligation| {
                format!(
                    "Obligation {} will rely on this waiver: {}",
                    obligation.id.0, obligation.description
                )
            }));
            consequence_statements.push(format!(
                "The waiver cannot authorize readiness beyond {}.",
                readiness_target_label(maximum_readiness_target)
            ));
            packets.push(make_authorization_action_packet(
                WorkflowAuthorizationKind::Waiver,
                StableId(format!("packet.workflow.waiver.{}", claim.id.0)),
                binding_for(selected, claim.id.clone()),
                human_authority("workflow.waiver.authorize"),
                WorkflowAuthorizationInputContract::Waiver {
                    claim_ref: claim.id.clone(),
                    maximum_readiness_target,
                    max_age_seconds: *max_age_seconds,
                    reason_required: true,
                    consequence_statements,
                },
            )?);
        }
    }

    let mut policies = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .collect::<Vec<_>>();
    policies.sort_by(|left, right| left.id.cmp(&right.id));
    for policy in policies {
        let mut signals = policy.routing.signals.clone();
        signals.sort();
        signals.dedup();
        for signal in signals {
            let transition = if derived.active_signals.contains(&signal) {
                WorkflowSignalInputTransition::Deactivate
            } else {
                WorkflowSignalInputTransition::Activate
            };
            let subject_ref = StableId(format!(
                "signal.{}.{}",
                policy.id.0,
                workflow_signal_label(signal)
            ));
            packets.push(make_authorization_action_packet(
                WorkflowAuthorizationKind::Signal,
                StableId(format!("packet.workflow.{}", subject_ref.0)),
                binding_for(policy, subject_ref),
                operator_authority("workflow.signal.authorize"),
                WorkflowAuthorizationInputContract::Signal {
                    signal,
                    transition,
                    basis_refs_min_items: 1,
                    basis_refs_repo_relative: true,
                },
            )?);
        }
    }

    packets.sort_by(|left, right| left.packet_id.cmp(&right.packet_id));
    if packets
        .windows(2)
        .any(|pair| pair[0].packet_id == pair[1].packet_id)
    {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "duplicate deterministic action packet id".to_owned(),
        ));
    }
    Ok(packets)
}

fn make_authorization_action_packet(
    authorization_kind: WorkflowAuthorizationKind,
    packet_id: StableId,
    binding: WorkflowAuthorizationPacketBinding,
    required_authority: WorkflowAuthorizationRequiredAuthority,
    input_contract: WorkflowAuthorizationInputContract,
) -> Result<WorkflowAuthorizationActionPacket, WorkflowGovernanceAdapterError> {
    let schema_version = WORKFLOW_AUTHORIZATION_ACTION_PACKET_SCHEMA_VERSION.to_owned();
    let packet_digest = authorization_action_packet_digest(
        &schema_version,
        &packet_id,
        authorization_kind,
        &binding,
        &required_authority,
        &input_contract,
    )?;
    Ok(WorkflowAuthorizationActionPacket {
        schema_version,
        packet_id,
        packet_digest,
        authorization_kind,
        binding,
        required_authority,
        input_contract,
    })
}

fn authorization_action_packet_digest(
    schema_version: &str,
    packet_id: &StableId,
    authorization_kind: WorkflowAuthorizationKind,
    binding: &WorkflowAuthorizationPacketBinding,
    required_authority: &WorkflowAuthorizationRequiredAuthority,
    input_contract: &WorkflowAuthorizationInputContract,
) -> Result<String, WorkflowGovernanceAdapterError> {
    let basis = WorkflowAuthorizationActionPacketDigestBasis {
        schema_version,
        packet_id,
        authorization_kind,
        binding,
        required_authority,
        input_contract,
    };
    let canonical = serde_json_canonicalizer::to_vec(&basis)
        .map_err(|error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()))?;
    Ok(sha256_content_hash(&canonical))
}

fn human_authority(grant: &str) -> WorkflowAuthorizationRequiredAuthority {
    WorkflowAuthorizationRequiredAuthority {
        accepted_roles: vec![CallerRole::Human],
        required_grant: StableId(grant.to_owned()),
        approval_boundary: WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker,
    }
}

fn runtime_authority(grant: &str) -> WorkflowAuthorizationRequiredAuthority {
    WorkflowAuthorizationRequiredAuthority {
        accepted_roles: vec![CallerRole::Runtime],
        required_grant: StableId(grant.to_owned()),
        approval_boundary: WorkflowAuthorizationApprovalBoundary::TrustedRuntimeBroker,
    }
}

fn operator_authority(grant: &str) -> WorkflowAuthorizationRequiredAuthority {
    WorkflowAuthorizationRequiredAuthority {
        accepted_roles: vec![CallerRole::Runtime, CallerRole::Worker, CallerRole::Driver],
        required_grant: StableId(grant.to_owned()),
        approval_boundary: WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker,
    }
}

fn capability_subject_kinds(
    probe_kind: WorkflowCapabilityProbeKind,
) -> Vec<WorkflowEvidenceSubjectKind> {
    match probe_kind {
        WorkflowCapabilityProbeKind::StaticRegistry | WorkflowCapabilityProbeKind::LocalCommand => {
            vec![
                WorkflowEvidenceSubjectKind::Artifact,
                WorkflowEvidenceSubjectKind::RepositoryState,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ]
        }
        WorkflowCapabilityProbeKind::RuntimeHandshake => vec![
            WorkflowEvidenceSubjectKind::Runtime,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
        ],
        WorkflowCapabilityProbeKind::CredentialCheck => vec![
            WorkflowEvidenceSubjectKind::ExternalSystem,
            WorkflowEvidenceSubjectKind::Runtime,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
        ],
        WorkflowCapabilityProbeKind::HumanAttestation => vec![
            WorkflowEvidenceSubjectKind::HumanDecision,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
        ],
        WorkflowCapabilityProbeKind::ExternalVerification => vec![
            WorkflowEvidenceSubjectKind::ExternalSystem,
            WorkflowEvidenceSubjectKind::Artifact,
        ],
    }
}

fn evidence_action_contract(
    provider: WorkflowEvaluatorProvider,
) -> (
    WorkflowAuthorizationRequiredAuthority,
    WorkflowEvidenceKind,
    WorkflowEvidenceStrength,
    Vec<WorkflowEvidenceSubjectKind>,
) {
    match provider {
        WorkflowEvaluatorProvider::AuthorizedHuman => (
            human_authority("workflow.evidence.authorize_human"),
            WorkflowEvidenceKind::HumanAcceptance,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
            vec![
                WorkflowEvidenceSubjectKind::HumanDecision,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ],
        ),
        WorkflowEvaluatorProvider::IndependentReviewer => (
            WorkflowAuthorizationRequiredAuthority {
                accepted_roles: vec![CallerRole::Worker, CallerRole::Driver],
                required_grant: StableId("workflow.evidence.authorize_review".to_owned()),
                approval_boundary: WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker,
            },
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
            vec![
                WorkflowEvidenceSubjectKind::Artifact,
                WorkflowEvidenceSubjectKind::RepositoryState,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ],
        ),
        WorkflowEvaluatorProvider::RepositoryInspector => (
            runtime_authority("workflow.evidence.authorize_runtime"),
            WorkflowEvidenceKind::ArtifactInspection,
            WorkflowEvidenceStrength::InspectedArtifact,
            vec![
                WorkflowEvidenceSubjectKind::Artifact,
                WorkflowEvidenceSubjectKind::RepositoryState,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ],
        ),
        WorkflowEvaluatorProvider::DeterministicTool => (
            runtime_authority("workflow.evidence.authorize_runtime"),
            WorkflowEvidenceKind::DeterministicCheck,
            WorkflowEvidenceStrength::DeterministicVerification,
            vec![
                WorkflowEvidenceSubjectKind::Artifact,
                WorkflowEvidenceSubjectKind::RepositoryState,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ],
        ),
        WorkflowEvaluatorProvider::RepresentativeRuntime => (
            runtime_authority("workflow.evidence.authorize_runtime"),
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
            vec![
                WorkflowEvidenceSubjectKind::Runtime,
                WorkflowEvidenceSubjectKind::ProjectSnapshot,
            ],
        ),
        WorkflowEvaluatorProvider::ExternalAuthority => (
            WorkflowAuthorizationRequiredAuthority {
                accepted_roles: vec![CallerRole::Worker, CallerRole::Runtime],
                required_grant: StableId("workflow.evidence.authorize_external".to_owned()),
                approval_boundary: WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker,
            },
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
            vec![
                WorkflowEvidenceSubjectKind::ExternalSystem,
                WorkflowEvidenceSubjectKind::Artifact,
            ],
        ),
        WorkflowEvaluatorProvider::ResearchSource => (
            WorkflowAuthorizationRequiredAuthority {
                accepted_roles: vec![CallerRole::Worker, CallerRole::Runtime],
                required_grant: StableId("workflow.evidence.authorize_external".to_owned()),
                approval_boundary: WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker,
            },
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
            vec![
                WorkflowEvidenceSubjectKind::ExternalSystem,
                WorkflowEvidenceSubjectKind::Artifact,
            ],
        ),
    }
}

fn authorization_setup_gaps(
    project_root: &Path,
    broker_status: WorkflowAuthorizationRegistrySetupStatus,
    packets: &[WorkflowAuthorizationActionPacket],
) -> Vec<WorkflowAuthorizationSetupGap> {
    let (code, state_label) = match broker_status {
        WorkflowAuthorizationRegistrySetupStatus::Missing => (
            WorkflowAuthorizationSetupGapCode::BrokerRegistryMissing,
            "the project has no external workflow broker registry",
        ),
        WorkflowAuthorizationRegistrySetupStatus::NoActiveIssuer => (
            WorkflowAuthorizationSetupGapCode::BrokerRegistryNoActiveIssuer,
            "the external workflow broker registry has no active issuer",
        ),
        WorkflowAuthorizationRegistrySetupStatus::Ready => return Vec::new(),
    };
    if packets.is_empty() {
        return Vec::new();
    }

    let mut human = false;
    let mut reviewer = false;
    let mut runtime = false;
    for packet in packets {
        match packet.required_authority.approval_boundary {
            WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker => human = true,
            WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker => reviewer = true,
            WorkflowAuthorizationApprovalBoundary::TrustedRuntimeBroker
            | WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker => runtime = true,
        }
    }

    [
        (human, WorkflowBrokerIssuerProfile::Human, "human"),
        (reviewer, WorkflowBrokerIssuerProfile::Reviewer, "reviewer"),
        (runtime, WorkflowBrokerIssuerProfile::Runtime, "runtime"),
    ]
    .into_iter()
    .filter(|(required, _, _)| *required)
    .map(|(_, profile, profile_label)| WorkflowAuthorizationSetupGap {
        code,
        summary: format!(
            "{state_label}; enroll an operator-owned {profile_label} broker before applying the corresponding action packet"
        ),
        accepted_profiles: vec![profile],
        setup_argv: vec![
            "forge-core".to_owned(),
            "workflow".to_owned(),
            "broker".to_owned(),
            "trust".to_owned(),
            "--root".to_owned(),
            project_root.display().to_string(),
            "--issuer-id".to_owned(),
            format!("<broker-{profile_label}-issuer-id>"),
            "--profile".to_owned(),
            profile_label.to_owned(),
            "--public-key-file".to_owned(),
            format!("<broker-{profile_label}-public-key-file>"),
            "--ceremony-ref".to_owned(),
            format!("<broker-{profile_label}-ceremony-ref>"),
            "--ceremony-file".to_owned(),
            format!("<broker-{profile_label}-ceremony-file>"),
            "--json".to_owned(),
        ],
        required_operator_inputs: vec![
            "issuer_id".to_owned(),
            "public_key_file".to_owned(),
            "ceremony_ref".to_owned(),
            "ceremony_file".to_owned(),
        ],
    })
    .collect()
}

const fn readiness_target_label(target: ReadinessTarget) -> &'static str {
    match target {
        ReadinessTarget::Explore => "explore",
        ReadinessTarget::Execute => "execute",
        ReadinessTarget::Release => "release",
    }
}

fn registry_setup_status(digest: &Option<String>) -> WorkflowAuthorizationRegistrySetupStatus {
    if digest.is_some() {
        WorkflowAuthorizationRegistrySetupStatus::Ready
    } else {
        WorkflowAuthorizationRegistrySetupStatus::Missing
    }
}

const fn workflow_signal_label(signal: WorkflowGovernanceSignal) -> &'static str {
    match signal {
        WorkflowGovernanceSignal::ContextRecoveryRequired => "context_recovery_required",
        WorkflowGovernanceSignal::CourseCorrectionRequired => "course_correction_required",
        WorkflowGovernanceSignal::AdversarialReviewRequested => "adversarial_review_requested",
        WorkflowGovernanceSignal::ReadinessRequested => "readiness_requested",
        WorkflowGovernanceSignal::BuildCompleted => "build_completed",
    }
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
    use ed25519_dalek::{Signer as _, SigningKey};
    use forge_core_authority::{
        workflow_broker_event_signing_bytes, WorkflowBrokerEnrollmentDeclaration,
        WorkflowBrokerEventEnvelope, WorkflowBrokerFreshnessPolicy, WorkflowBrokerIssuerEntry,
        WORKFLOW_BROKER_EVENT_SCHEMA_VERSION, WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION,
    };
    use forge_core_store::workflow_action_replay::WorkflowActionReplayState;

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

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn install_runtime_broker_registry(
        adapter: &WorkflowGovernanceProjectAdapter,
        key: &SigningKey,
    ) -> WorkflowBrokerRegistryDocument {
        let document = WorkflowBrokerRegistryDocument {
            schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: adapter.expected_broker_audience(),
            issuers: vec![WorkflowBrokerIssuerEntry {
                issuer_id: StableId("broker.runtime.test".to_owned()),
                profile: WorkflowBrokerIssuerProfile::Runtime,
                public_key_hex: hex(key.verifying_key().as_bytes()),
                status: WorkflowBrokerIssuerStatus::Active,
                enrollment: WorkflowBrokerEnrollmentDeclaration {
                    ceremony_ref: "operator://ceremony/runtime-test".to_owned(),
                    ceremony_digest: format!("sha256:{}", "a".repeat(64)),
                    declared_at_unix: 10,
                },
            }],
        };
        let path = adapter.trusted_broker_registry_path();
        fs::create_dir_all(path.parent().expect("broker registry parent"))
            .expect("broker registry parent");
        fs::write(
            path,
            yaml_serde::to_string(&document).expect("broker registry YAML"),
        )
        .expect("broker registry");
        document
    }

    fn signed_signal_envelope(
        project_id: &StableId,
        packet: &WorkflowAuthorizationActionPacket,
        key: &SigningKey,
        issued_at_unix: u64,
        nonce: &str,
    ) -> WorkflowBrokerEventEnvelope {
        let WorkflowAuthorizationInputContract::Signal { transition, .. } = packet.input_contract
        else {
            panic!("signal packet");
        };
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: format!("forge-core:workflow:{}", project_id.0),
            issuer_id: StableId("broker.runtime.test".to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Runtime,
            origin_principal_id: PrincipalId("principal.runtime.origin".to_owned()),
            separation_domain: StableId("runtime.test.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::Signal,
            project_id: project_id.clone(),
            action_packet_digest: packet.packet_digest.clone(),
            semantic_input: WorkflowBrokerSemanticInput::Signal {
                active: transition == WorkflowSignalInputTransition::Activate,
                basis_refs: vec!["README.md".to_owned()],
            },
            issued_at_unix,
            expires_at_unix: issued_at_unix + 120,
            nonce: nonce.to_owned(),
            signature: String::new(),
        };
        let signing_bytes =
            workflow_broker_event_signing_bytes(&envelope).expect("broker signing bytes");
        envelope.signature = hex(&key.sign(&signing_bytes).to_bytes());
        envelope
    }

    fn verify_broker_envelope(
        document: &WorkflowBrokerRegistryDocument,
        envelope: WorkflowBrokerEventEnvelope,
        now: u64,
    ) -> VerifiedWorkflowBrokerEvent {
        AuthorizedWorkflowBrokerRegistry::from_document(document.clone())
            .expect("authorized broker registry")
            .verify_event(
                envelope,
                &StableId("project.broker-apply".to_owned()),
                i64::try_from(now).expect("clock fits i64"),
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("verified broker event")
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
    fn domain_epoch_invalidates_receipts_unless_runtime_and_context_are_exact() {
        let runtime = WorkflowRuntimeBundleIdentity {
            bundle_id: StableId("bundle.test".to_owned()),
            bundle_digest: format!("sha256:{}", "1".repeat(64)),
            policy_set_digest: format!("sha256:{}", "2".repeat(64)),
        };
        let from = WorkflowEffectiveBundleIdentity {
            core_runtime_bundle: runtime.clone(),
            effective_runtime_bundle: runtime.clone(),
            domain_pack_generation: None,
            receipt_context_digest: format!("sha256:{}", "3".repeat(64)),
        };
        let mut to = from.clone();
        to.domain_pack_generation =
            Some(forge_core_contracts::WorkflowDomainPackGenerationIdentity {
                generation: 1,
                active_lock_digest: format!("sha256:{}", "4".repeat(64)),
                composition_digest: format!("sha256:{}", "5".repeat(64)),
                base_core_bundle_digest: format!("sha256:{}", "6".repeat(64)),
                supply_chain_registry_digest: format!("sha256:{}", "7".repeat(64)),
                reviewer_registry_digest: format!("sha256:{}", "8".repeat(64)),
                reviewed_registry_digest: format!("sha256:{}", "9".repeat(64)),
            });
        to.receipt_context_digest = format!("sha256:{}", "a".repeat(64));
        let transition = WorkflowGovernanceLedgerRecord {
            record_id: StableId("record.domain-transition".to_owned()),
            sequence: 2,
            project_id: StableId("project.test".to_owned()),
            bundle_id: runtime.bundle_id.clone(),
            bundle_digest: runtime.bundle_digest.clone(),
            state_version: 1,
            previous_record_digest: Some(format!("sha256:{}", "b".repeat(64))),
            record_digest: format!("sha256:{}", "c".repeat(64)),
            recorded_at_unix: 10,
            event: WorkflowGovernanceEvent::DomainPackGenerationTransitioned(
                forge_core_contracts::DomainPackGenerationTransitionedEvent {
                    from_effective_bundle: from.clone(),
                    to_effective_bundle: to.clone(),
                    receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
                    prior_ledger_head_digest: format!("sha256:{}", "b".repeat(64)),
                },
            ),
        };
        let projection = WorkflowGovernanceLedgerProjection {
            records: vec![transition],
            head_digest: Some(format!("sha256:{}", "c".repeat(64))),
            next_sequence: 3,
            next_state_version: 2,
        };
        assert_eq!(receipt_window_start(&projection), 1);
        assert_eq!(
            classify_domain_pack_transition_recovery(
                &projection,
                &from,
                &to,
                &format!("sha256:{}", "b".repeat(64)),
                1,
            ),
            DomainPackTransitionRecovery::TargetCommitted
        );

        let mut forked_envelope = projection.clone();
        forked_envelope.records[0].previous_record_digest =
            Some(format!("sha256:{}", "d".repeat(64)));
        assert_eq!(
            classify_domain_pack_transition_recovery(
                &forked_envelope,
                &from,
                &to,
                &format!("sha256:{}", "b".repeat(64)),
                1,
            ),
            DomainPackTransitionRecovery::Indeterminate,
            "a target identity under the wrong envelope must never hide a fork"
        );

        let source_projection = WorkflowGovernanceLedgerProjection {
            records: vec![WorkflowGovernanceLedgerRecord {
                record_id: StableId("record.project-import".to_owned()),
                sequence: 0,
                project_id: StableId("project.test".to_owned()),
                bundle_id: runtime.bundle_id,
                bundle_digest: runtime.bundle_digest,
                state_version: 0,
                previous_record_digest: None,
                record_digest: format!("sha256:{}", "b".repeat(64)),
                recorded_at_unix: 9,
                event: WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                    source_ref: "project.test".to_owned(),
                    source_digest: format!("sha256:{}", "e".repeat(64)),
                    snapshot_digest: format!("sha256:{}", "e".repeat(64)),
                    initial_phase: StableId("1-discovery".to_owned()),
                }),
            }],
            head_digest: Some(format!("sha256:{}", "b".repeat(64))),
            next_sequence: 1,
            next_state_version: 1,
        };
        assert_eq!(
            classify_domain_pack_transition_recovery(
                &source_projection,
                &from,
                &to,
                &format!("sha256:{}", "b".repeat(64)),
                1,
            ),
            DomainPackTransitionRecovery::SourceUnchanged
        );
    }

    #[test]
    fn degraded_domain_pack_gap_error_preserves_actionable_subject_and_message() {
        let error = WorkflowGovernanceAdapterError::DomainPackGapsBlocking(vec![
            DomainPackCompositionGap {
                code: forge_core_contracts::DomainPackCompositionGapCode::MissingDomain,
                requirement_ref: StableId("requirement.domain.required".to_owned()),
                subject_ref: StableId("domain.removed.required".to_owned()),
                message: "install or restore an eligible reviewed Domain Pack".to_owned(),
                authority: forge_core_contracts::DomainPackCandidateAuthority::CandidateOnly,
            },
        ]);
        let rendered = error.to_string();
        assert!(rendered.contains("domain.removed.required"));
        assert!(rendered.contains("install or restore"));
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
        assert_eq!(
            initialized.effective.core_runtime_bundle,
            initialized.effective.effective_runtime_bundle
        );
        assert!(initialized.effective.domain_pack_generation.is_none());
        assert!(!initialized.domain_pack_degraded);
        assert!(initialized.domain_pack_gaps.is_empty());
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
        assert_eq!(
            next.release.runtime_bundle,
            next.effective.core_runtime_bundle
        );
        assert_eq!(
            next.bundle_digest,
            next.effective.effective_runtime_bundle.bundle_digest
        );
        assert!(!next.domain_pack_degraded);
        assert!(next.domain_pack_gaps.is_empty());
    }

    #[test]
    fn action_packets_are_deterministic_cas_bound_and_authority_typed() {
        let (root, state) = temp_project("action-packets");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.action-packets".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");

        let first = adapter.action_packets().expect("first packets");
        let repeated = adapter.action_packets().expect("repeated packets");
        assert_eq!(first, repeated, "packet projection must be deterministic");
        assert!(first
            .packets
            .windows(2)
            .all(|pair| pair[0].packet_id < pair[1].packet_id));

        let evidence = first
            .packets
            .iter()
            .find(|packet| {
                packet.authorization_kind == WorkflowAuthorizationKind::Evidence
                    && packet.binding.policy_ref.0 == "policy.workflow.discover-intent"
            })
            .expect("discover intent evidence packet");
        assert_eq!(
            evidence.schema_version,
            WORKFLOW_AUTHORIZATION_ACTION_PACKET_SCHEMA_VERSION
        );
        assert_eq!(evidence.binding.project_id, first.project_id);
        assert_eq!(evidence.binding.snapshot_digest, first.snapshot_digest);
        assert_eq!(
            evidence.binding.ledger_head_digest,
            first.ledger_head_digest
        );
        assert_eq!(evidence.binding.state_version, first.state_version);
        assert_eq!(evidence.binding.current_phase.0, "1-discovery");
        assert_eq!(
            evidence.binding.effective_bundle_digest,
            adapter.next().expect("guidance").bundle_digest
        );
        assert_eq!(evidence.binding.readiness_target, ReadinessTarget::Explore);
        assert_eq!(
            evidence.required_authority.accepted_roles,
            vec![CallerRole::Human]
        );
        assert_eq!(
            evidence.required_authority.required_grant.0,
            "workflow.evidence.authorize_human"
        );
        assert_eq!(
            evidence.required_authority.approval_boundary,
            WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker
        );
        assert!(matches!(
            &evidence.input_contract,
            WorkflowAuthorizationInputContract::Evidence {
                provider: WorkflowEvaluatorProvider::AuthorizedHuman,
                evidence_kind: WorkflowEvidenceKind::HumanAcceptance,
                strength: WorkflowEvidenceStrength::AuthoritativeAcceptance,
                allowed_outcomes,
                ..
            } if allowed_outcomes == &vec![
                WorkflowEvidenceOutcome::Pass,
                WorkflowEvidenceOutcome::Fail,
                WorkflowEvidenceOutcome::Inconclusive,
            ]
        ));
        assert_eq!(
            evidence.packet_digest,
            authorization_action_packet_digest(
                &evidence.schema_version,
                &evidence.packet_id,
                evidence.authorization_kind,
                &evidence.binding,
                &evidence.required_authority,
                &evidence.input_contract,
            )
            .expect("canonical packet digest")
        );

        let serialized = serde_json::to_string(&first).expect("serialize packets");
        for forbidden in [
            "observed_at_unix",
            "expires_at_unix",
            "attestation",
            "selected_alternative_ref",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "packet projection leaked response field {forbidden}"
            );
        }

        let mut changed_binding = evidence.binding.clone();
        changed_binding.snapshot_digest = format!("sha256:{}", "f".repeat(64));
        let changed_digest = authorization_action_packet_digest(
            &evidence.schema_version,
            &evidence.packet_id,
            evidence.authorization_kind,
            &changed_binding,
            &evidence.required_authority,
            &evidence.input_contract,
        )
        .expect("changed digest");
        assert_ne!(changed_digest, evidence.packet_digest);

        fs::write(root.join("README.md"), b"project changed\n").expect("mutate project");
        let changed = adapter.action_packets().expect("changed packets");
        let changed_evidence = changed
            .packets
            .iter()
            .find(|packet| packet.packet_id == evidence.packet_id)
            .expect("stable packet id");
        assert_ne!(changed.snapshot_digest, first.snapshot_digest);
        assert_eq!(changed.ledger_head_digest, first.ledger_head_digest);
        assert_ne!(changed_evidence.packet_digest, evidence.packet_digest);
    }

    #[test]
    fn next_exposes_actionable_broker_setup_and_survives_last_issuer_revocation() {
        let (root, state) = temp_project("broker-setup-guidance");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-setup-guidance".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");

        let missing = adapter.next().expect("missing broker guidance");
        assert_eq!(
            missing.authorization.registry_setup.broker_registry,
            WorkflowAuthorizationRegistrySetupStatus::Missing
        );
        assert!(!missing.authorization.action_packets.is_empty());
        assert!(!missing.authorization.setup_gaps.is_empty());
        for gap in &missing.authorization.setup_gaps {
            assert_eq!(
                gap.code,
                WorkflowAuthorizationSetupGapCode::BrokerRegistryMissing
            );
            assert_eq!(
                gap.setup_argv.first().map(String::as_str),
                Some("forge-core")
            );
            assert!(gap
                .setup_argv
                .windows(2)
                .any(|pair| { pair[0] == "--root" && pair[1] == root.display().to_string() }));
            let serialized = serde_json::to_string(gap).expect("gap JSON");
            assert!(!serialized.contains("private_key"));
            assert!(!serialized.contains("request-file"));
            assert!(!serialized.contains("attestation"));
        }

        let key = SigningKey::from_bytes(&[31_u8; 32]);
        let mut document = install_runtime_broker_registry(&adapter, &key);
        document.issuers[0].status = WorkflowBrokerIssuerStatus::Revoked;
        fs::write(
            adapter.trusted_broker_registry_path(),
            yaml_serde::to_string(&document).expect("revoked registry YAML"),
        )
        .expect("revoked registry");

        let revoked = adapter.next().expect("revoked broker guidance");
        assert_eq!(
            revoked.authorization.registry_setup.broker_registry,
            WorkflowAuthorizationRegistrySetupStatus::NoActiveIssuer
        );
        assert!(!revoked.authorization.action_packets.is_empty());
        assert!(revoked
            .authorization
            .action_packets
            .iter()
            .all(|packet| { packet.binding.trusted_broker_registry_digest.is_some() }));
        assert!(revoked.authorization.setup_gaps.iter().all(|gap| {
            gap.code == WorkflowAuthorizationSetupGapCode::BrokerRegistryNoActiveIssuer
        }));

        document.audience = "forge-core:workflow:project.other".to_owned();
        fs::write(
            adapter.trusted_broker_registry_path(),
            yaml_serde::to_string(&document).expect("foreign registry YAML"),
        )
        .expect("foreign registry");
        assert!(matches!(
            adapter.next(),
            Err(WorkflowGovernanceAdapterError::TrustedRegistry { .. })
        ));
    }

    #[test]
    fn prepares_closed_requests_and_rejects_stale_packets() {
        let (root, state) = temp_project("prepare-authorization");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.prepare-authorization".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let now = unix_time().expect("clock");
        let packet_set = adapter.action_packets_at(now).expect("packets");
        assert_eq!(
            packet_set.registry_setup.principal_registry,
            WorkflowAuthorizationRegistrySetupStatus::Missing
        );
        assert_eq!(
            packet_set.registry_setup.broker_registry,
            WorkflowAuthorizationRegistrySetupStatus::Missing
        );
        assert!(packet_set.packets.iter().all(|packet| {
            packet.binding.trusted_principal_registry_digest.is_none()
                && packet.binding.trusted_broker_registry_digest.is_none()
        }));

        let signal_packet = packet_set
            .packets
            .iter()
            .find(|packet| {
                matches!(
                    packet.input_contract,
                    WorkflowAuthorizationInputContract::Signal {
                        transition: WorkflowSignalInputTransition::Activate,
                        ..
                    }
                )
            })
            .expect("activation signal packet");
        let prepared = adapter
            .prepare_authorization(
                &signal_packet.packet_digest,
                WorkflowAuthorizationClosedInput::Signal {
                    active: true,
                    basis_refs: vec!["README.md".to_owned()],
                },
                now,
            )
            .expect("prepared signal");
        let PreparedWorkflowAuthorization::Signal { request, packet } = prepared else {
            panic!("expected signal request");
        };
        assert_eq!(packet.packet_digest, signal_packet.packet_digest);
        assert_eq!(request.basis_refs, vec!["README.md"]);
        let basis = content_addressed_basis_from_paths(&root, &request.basis_refs)
            .expect("canonical basis");
        assert_eq!(
            request.basis_digest,
            content_addressed_basis_digest(&basis).expect("basis digest")
        );
        assert_eq!(
            request.expires_at_unix,
            now + WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS
        );

        let (artifact_ref, artifact_digest) = confined_subject_reference(
            &root,
            &packet_set.project_id,
            &packet_set.snapshot_digest,
            WorkflowEvidenceSubjectKind::Artifact,
            "README.md",
        )
        .expect("artifact subject");
        assert_eq!(artifact_ref, "README.md");
        assert_eq!(
            artifact_digest,
            sha256_content_hash(&fs::read(root.join("README.md")).expect("readme"))
        );

        let alternative = DecisionAlternative {
            id: StableId("alternative.accept".to_owned()),
            description: "Accept the bounded direction".to_owned(),
            consequences: vec!["The selected direction becomes authoritative".to_owned()],
        };
        let evidence_packet = packet_set
            .packets
            .iter()
            .find(|packet| packet.authorization_kind == WorkflowAuthorizationKind::Evidence)
            .expect("evidence packet");
        let decision_packet = make_authorization_action_packet(
            WorkflowAuthorizationKind::Decision,
            StableId("packet.workflow.decision.test".to_owned()),
            WorkflowAuthorizationPacketBinding {
                subject_ref: StableId("decision.test".to_owned()),
                ..evidence_packet.binding.clone()
            },
            human_authority("workflow.decision.resolve"),
            WorkflowAuthorizationInputContract::Decision {
                decision_ref: StableId("decision.test".to_owned()),
                alternatives: vec![alternative.clone()],
                recommended_alternative_ref: alternative.id.clone(),
            },
        )
        .expect("decision packet");
        let release_registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("release registry");
        let domain = LockedWorkflowDomainPackContext::acquire(&state).expect("domain");
        let ledger = lock_workflow_governance_ledger_tcb(&state).expect("ledger");
        let projection = ledger.recover().expect("projection");
        let admitted = adapter
            .resolve_active_release(&release_registry, &projection)
            .expect("release");
        let effective = domain.admit_effective(admitted).expect("effective");
        let prepared = prepare_authorization_from_packet(
            effective.document(),
            &projection,
            &root,
            decision_packet.clone(),
            WorkflowAuthorizationClosedInput::Decision {
                selected_alternative_ref: alternative.id.clone(),
            },
            now,
        )
        .expect("prepared decision");
        let PreparedWorkflowAuthorization::Decision { request, .. } = prepared else {
            panic!("expected decision request");
        };
        assert_eq!(request.selected_alternative_ref, alternative.id);
        assert_eq!(
            request.consequences_ack_digest,
            decision_consequences_ack_digest(
                &decision_packet.packet_digest,
                &StableId("decision.test".to_owned()),
                &request.selected_alternative_ref,
                &alternative.consequences,
            )
            .expect("ack digest")
        );
        assert!(matches!(
            prepare_authorization_from_packet(
                effective.document(),
                &projection,
                &root,
                decision_packet,
                WorkflowAuthorizationClosedInput::Decision {
                    selected_alternative_ref: StableId("alternative.unknown".to_owned()),
                },
                now,
            ),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        drop(effective);
        drop(domain);
        drop(ledger);

        let stale_packet = evidence_packet.clone();
        fs::write(root.join("README.md"), b"stale packet\n").expect("mutate project");
        let stale = adapter.prepare_authorization(
            &stale_packet.packet_digest,
            WorkflowAuthorizationClosedInput::Evidence {
                outcome: WorkflowEvidenceOutcome::Pass,
                subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                subject_ref: packet_set.project_id.0,
                scenario_ref: "README.md".to_owned(),
            },
            now,
        );
        assert!(matches!(
            stale,
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
    }

    #[test]
    fn broker_action_repairs_replay_commit_after_durable_ledger_response_loss() {
        let (root, state) = temp_project("broker-after-ledger");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize with replay");
        let key = SigningKey::from_bytes(&[23_u8; 32]);
        let broker_document = install_runtime_broker_registry(&adapter, &key);
        let now = unix_time().expect("clock");
        let packets = adapter.action_packets_at(now).expect("packets");
        let packet = packets
            .packets
            .iter()
            .find(|packet| {
                matches!(
                    packet.input_contract,
                    WorkflowAuthorizationInputContract::Signal {
                        transition: WorkflowSignalInputTransition::Activate,
                        ..
                    }
                )
            })
            .expect("runtime signal packet");
        let envelope = signed_signal_envelope(
            &packets.project_id,
            packet,
            &key,
            now,
            "broker-response-loss-nonce-0001",
        );
        let receipt = adapter
            .apply_verified_broker_action(
                verify_broker_envelope(&broker_document, envelope.clone(), now),
                now,
            )
            .expect("first broker apply");
        assert_eq!(
            receipt.origin_record.previous_record_digest.as_deref(),
            Some(receipt.action_record.record_digest.as_str())
        );
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &receipt.origin_record.event
        else {
            panic!("origin companion");
        };
        assert_eq!(origin.action_packet_digest, packet.packet_digest);
        assert_eq!(
            origin.action_record_digest,
            receipt.action_record.record_digest
        );
        assert_eq!(
            origin.origin_principal_id,
            PrincipalId("principal.runtime.origin".to_owned())
        );

        let next_packets = adapter.action_packets_at(now).expect("next packets");
        let next_signal = next_packets
            .packets
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.input_contract,
                    WorkflowAuthorizationInputContract::Signal {
                        transition: WorkflowSignalInputTransition::Deactivate,
                        ..
                    }
                )
            })
            .expect("deactivation signal packet");
        let nonce_replay = signed_signal_envelope(
            &next_packets.project_id,
            next_signal,
            &key,
            now,
            "broker-response-loss-nonce-0001",
        );
        assert!(matches!(
            adapter.apply_verified_broker_action(
                verify_broker_envelope(&broker_document, nonce_replay, now),
                now,
            ),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));

        let replay =
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay recovery");
        let raw = fs::read_to_string(&replay.wal_path).expect("replay WAL");
        let mut lines = raw.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2, "reserve and commit records");
        lines.pop();
        fs::write(&replay.wal_path, format!("{}\n", lines.join("\n")))
            .expect("simulate crash before replay commit");

        let mut revoked_document = broker_document.clone();
        revoked_document.issuers[0].status = WorkflowBrokerIssuerStatus::Revoked;
        let historical = AuthorizedWorkflowBrokerRegistry::from_document(revoked_document)
            .expect("retained revoked broker key")
            .verify_event_for_recovery(envelope, &packets.project_id)
            .expect("historically verified response-loss event");
        fs::remove_file(adapter.trusted_broker_registry_path())
            .expect("simulate registry rotation/removal");
        let recovered = adapter
            .recover_historically_verified_broker_action(historical)
            .expect("response-loss recovery after rotation");
        assert_eq!(recovered.action_record, receipt.action_record);
        assert_eq!(recovered.origin_record, receipt.origin_record);
        assert!(recovered.replay_commit_repaired);
        let replay =
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("repaired replay");
        assert!(replay
            .entries
            .values()
            .all(|entry| { entry.state == WorkflowActionReplayState::Committed }));
    }

    #[test]
    fn broker_action_retry_after_dropped_precommit_batch_has_no_replay_tombstone() {
        let (root, state) = temp_project("broker-before-ledger");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize with replay");
        let key = SigningKey::from_bytes(&[29_u8; 32]);
        let broker_document = install_runtime_broker_registry(&adapter, &key);
        let now = unix_time().expect("clock");
        let packets = adapter.action_packets_at(now).expect("packets");
        let packet = packets
            .packets
            .iter()
            .find(|packet| {
                matches!(
                    packet.input_contract,
                    WorkflowAuthorizationInputContract::Signal {
                        transition: WorkflowSignalInputTransition::Activate,
                        ..
                    }
                )
            })
            .expect("runtime signal packet")
            .clone();
        let envelope = signed_signal_envelope(
            &packets.project_id,
            &packet,
            &key,
            now,
            "broker-before-ledger-nonce-0001",
        );
        let verified = verify_broker_envelope(&broker_document, envelope.clone(), now);
        let audit = verified.audit().clone();
        let semantic_input = verified.semantic_input().clone();

        let release_registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("release registry");
        let domain = LockedWorkflowDomainPackContext::acquire(&state).expect("domain");
        let mut ledger = lock_workflow_governance_ledger_tcb(&state).expect("ledger");
        let projection = ledger.recover().expect("projection");
        let admitted = adapter
            .resolve_active_release(&release_registry, &projection)
            .expect("release");
        let effective = domain.admit_effective(admitted).expect("effective");
        let broker_digest = adapter
            .current_trusted_broker_registry_digest()
            .expect("broker registry")
            .expect("broker registry digest");
        validate_broker_packet_audit(&packet, &semantic_input, &audit, &broker_digest)
            .expect("packet audit");
        let mut prepared = prepare_authorization_from_packet(
            effective.document(),
            &projection,
            &root,
            packet.clone(),
            broker_semantic_input_to_closed(semantic_input),
            audit.issued_at_unix,
        )
        .expect("prepare");
        bound_prepared_expiry(&mut prepared, audit.expires_at_unix).expect("bound expiry");
        let (_, event, _) = broker_action_event_from_prepared(
            effective.document(),
            &root,
            prepared,
            &audit,
            &broker_digest,
        )
        .expect("action event");
        let head = projection.head_digest.clone().expect("head");
        let identity = adapter.identity(admitted);
        let mut batch = ledger
            .begin_unchecked_tcb_batch(&head, &identity)
            .expect("batch");
        let planned = batch
            .push_verified_broker_action_unchecked_tcb(
                packet.binding.state_version,
                event,
                &packet.packet_digest,
                &audit.event_digest,
                audit.issued_at_unix,
            )
            .expect("planned action");
        drop(batch);
        drop(ledger);
        drop(effective);
        drop(domain);

        assert!(
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay after dropped batch")
                .entries
                .is_empty()
        );

        let historical = AuthorizedWorkflowBrokerRegistry::from_document(broker_document.clone())
            .expect("historical registry")
            .verify_event_for_recovery(envelope.clone(), &packets.project_id)
            .expect("historical proof");
        assert!(matches!(
            adapter.recover_historically_verified_broker_action(historical),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));

        let recovered = adapter
            .apply_verified_broker_action(
                verify_broker_envelope(&broker_document, envelope, now + 7),
                now + 7,
            )
            .expect("finish after dropped precommit batch");
        assert_eq!(recovered.action_record.record_digest, planned.record_digest);
        assert_eq!(recovered.action_record.record_id, planned.record_id);
        assert_eq!(
            recovered.action_record.recorded_at_unix,
            audit.issued_at_unix
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
