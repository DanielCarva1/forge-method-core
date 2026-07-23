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
    derive_domain_pack_core_binding, domain_pack_generation_transition_event,
    evaluate_verified_workflow_governance,
    load_admitted_workflow_governance_universal_assurance_release_registry,
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
    BrokerOriginAppliedEvent, HumanIntentRevisionAcceptedEvent, WorkflowBrokerOriginProfile,
};
use forge_core_contracts::{
    ApplicabilityAssessedEvent, CapabilityProbedEvent, ContinuityRecordedEvent,
    CoreDomainPackRebasedEvent, DecisionAlternative, DecisionResolvedEvent,
    DomainPackCompositionGap, DomainPackCoreBinding, DomainPackLifecycleOperation,
    DomainPackRebasePlanDocument, DomainPackRebasePlanInput, DurableAssuranceEpistemicState,
    DurableAssuranceProjection, EvaluatorObservedEvent, Phase, PhaseAdvancedEvent,
    PolicyCompletedEvent, PrincipalId, ProjectImportedEvent, ProjectLinkDocument, ReadinessTarget,
    ReleaseUpgradedEvent, SignalChangedEvent, StableId, UniversalAssuranceLens,
    WaiverAuthorizedEvent, WorkflowAssuranceClaimRole, WorkflowCapabilityProbeKind,
    WorkflowClaimWaiverObservation, WorkflowClaimWaiverPolicy, WorkflowCompletionAssertion,
    WorkflowContentAddressedReference, WorkflowEffectiveBundleIdentity, WorkflowEvaluatorProvider,
    WorkflowEvidenceFreshness, WorkflowEvidenceKind, WorkflowEvidenceObservation,
    WorkflowEvidenceOutcome, WorkflowEvidenceProvenance, WorkflowEvidenceStrength,
    WorkflowEvidenceSubject, WorkflowEvidenceSubjectKind, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluation, WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent,
    WorkflowGovernanceLedgerRecord, WorkflowGovernancePolicy, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceSignal, WorkflowHumanIntentRevision, WorkflowPolicyActivation,
    WorkflowPrerequisiteRequirement, WorkflowReceiptCarryover, WorkflowReleaseRegistryProvenance,
    WorkflowRepresentativeSliceDefinitionDocument, WorkflowRuntimeBundleIdentity,
    MAX_REPRESENTATIVE_SLICE_ITEMS, MAX_REPRESENTATIVE_SLICE_ITEM_BYTES,
    MAX_REPRESENTATIVE_SLICE_TEXT_BYTES, MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES,
    MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES, MAX_WORKFLOW_INTENT_ITEM_BYTES,
    MAX_WORKFLOW_INTENT_LIST_ITEMS, MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
    MAX_WORKFLOW_INTENT_TOTAL_BYTES, PROJECT_LINK_FILE_NAME, PROJECT_LINK_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION, WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    find_entry, load_embedded_frozen_legacy_catalog, plan_domain_pack_rebase,
    project_durable_assurance, project_governed_durable_assurance,
    project_legacy_workflow_compatibility, simulate_workflow_governance,
    validate_representative_slice_definition, verify_domain_pack_rebase_plan,
    workflow_human_intent_digest, AssuranceProjectionError, DomainPackRebasePlanError,
    GovernedAssuranceActionPacketFact, GovernedAssuranceCapabilityFact,
    GovernedAssuranceDecisionFact, GovernedAssuranceEvidenceFact, GovernedAssuranceFacts,
    GovernedAssuranceWaiverFact, LegacyWorkflowGovernanceProjection, WorkflowClaimResultStatus,
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
    domain_pack_receipt_carryover, lock_workflow_governance_ledger_tcb,
    LockedWorkflowGovernanceLedger, WorkflowGovernanceLedgerError,
    WorkflowGovernanceLedgerIdentity, WorkflowGovernanceLedgerProjection,
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
const UNIVERSAL_ASSURANCE_POLICY_ID: &str = "policy.workflow.universal-assurance";
const DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH: &str = "domain-packs/rebase-plan.yaml";
const DOMAIN_PACK_REBASE_PLAN_MAX_BYTES: u64 = 16 * 1024 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowRepresentativeSliceActionBinding {
    Definition {
        schema_version: String,
        current_intent_digest: String,
        text_max_bytes: usize,
        list_max_items: usize,
        item_max_bytes: usize,
        total_max_bytes: usize,
    },
    Execution {
        definition_digest: String,
        definition_receipt_digest: String,
        runtime_subject_ref: String,
        runtime_subject_digest: String,
        allowed_scenario_digests: Vec<String>,
    },
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
    IntentRevision {
        intent_id: StableId,
        next_intent_revision: u64,
        next_assurance_epoch: u64,
        desired_outcome_max_bytes: usize,
        list_max_items: usize,
        list_item_max_bytes: usize,
        source_ref_max_bytes: usize,
        total_max_bytes: usize,
    },
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
        #[serde(skip_serializing_if = "Option::is_none")]
        representative_slice: Option<WorkflowRepresentativeSliceActionBinding>,
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

/// Durable human-intent authority reconstructed from the workflow ledger.
/// Proposal-only Assurance Case files and host readiness claims are never
/// consulted by this projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDurableAssuranceGuidance {
    pub status: WorkflowDurableAssuranceStatus,
    pub blockers: Vec<WorkflowDurableAssuranceBlocker>,
    pub current_snapshot_digest: String,
    pub source_ledger_head_digest: String,
    pub case_digest: String,
    pub projection: Option<DurableAssuranceProjection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDurableAssuranceStatus {
    MissingHumanIntent,
    IntentAccepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDurableAssuranceBlocker {
    pub code: WorkflowDurableAssuranceBlockerCode,
    pub lens: Option<UniversalAssuranceLens>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDurableAssuranceBlockerCode {
    MissingAcceptedHumanIntent,
    UniversalLensUnknown,
    UniversalLensSupported,
    UniversalLensDisproven,
}

fn durable_assurance_blockers(
    projection: &DurableAssuranceProjection,
) -> Vec<WorkflowDurableAssuranceBlocker> {
    projection
        .blocker_lenses
        .iter()
        .copied()
        .map(|lens| {
            let state = projection
                .lenses
                .iter()
                .find(|item| item.lens == lens)
                .map_or(DurableAssuranceEpistemicState::Unknown, |item| {
                    item.claim_status
                });
            let (code, label) = match state {
                DurableAssuranceEpistemicState::Disproven => (
                    WorkflowDurableAssuranceBlockerCode::UniversalLensDisproven,
                    "is disproven",
                ),
                DurableAssuranceEpistemicState::Supported => (
                    WorkflowDurableAssuranceBlockerCode::UniversalLensSupported,
                    "is supported but not verified",
                ),
                _ => (
                    WorkflowDurableAssuranceBlockerCode::UniversalLensUnknown,
                    "remains unknown",
                ),
            };
            WorkflowDurableAssuranceBlocker {
                code,
                lens: Some(lens),
                summary: format!("Universal assurance lens {} {label}.", lens.id()),
            }
        })
        .collect()
}

fn durable_assurance_is_enforced(bundle: &WorkflowGovernanceBundleDocument) -> bool {
    bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .any(|policy| policy.id.0 == UNIVERSAL_ASSURANCE_POLICY_ID)
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

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowDurableAssuranceCaseDigestBasis<'a> {
    schema_version: &'static str,
    project_id: &'a StableId,
    current_snapshot_digest: &'a str,
    source_ledger_head_digest: &'a str,
    state_version: u64,
    effective_bundle_digest: &'a str,
    durable_projection_digest: Option<&'a str>,
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

#[derive(Debug, Clone)]
struct WorkflowDomainPackRebaseMaterial {
    source_core: DomainPackCoreBinding,
    lifecycle_operation: DomainPackLifecycleOperation,
    generation: u64,
    lifecycle_pointer_digest: String,
    lifecycle_head_digest: String,
    active_lock_digest: String,
    composition_digest: String,
    supply_chain_registry_digest: String,
    reviewer_registry_digest: String,
    reviewed_registry_digest: String,
    active_package_count: usize,
    active_composition_gaps: Vec<DomainPackCompositionGap>,
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

    fn rebase_material(
        &self,
    ) -> Result<Option<WorkflowDomainPackRebaseMaterial>, WorkflowGovernanceAdapterError> {
        let Self::Active(active) = self else {
            return Ok(None);
        };
        let view = active.verified_view()?;
        Ok(Some(WorkflowDomainPackRebaseMaterial {
            source_core: view.core_binding().clone(),
            lifecycle_operation: view.lifecycle_operation().clone(),
            generation: view.generation_id(),
            lifecycle_pointer_digest: view.lifecycle_pointer_digest().to_owned(),
            lifecycle_head_digest: view.lifecycle_head_digest().to_owned(),
            active_lock_digest: view.lock_digest().to_owned(),
            composition_digest: view.composition_digest().to_owned(),
            supply_chain_registry_digest: view.supply_chain_registry_digest().to_owned(),
            reviewer_registry_digest: view.reviewer_registry_digest().to_owned(),
            reviewed_registry_digest: view.reviewed_registry_digest().to_owned(),
            active_package_count: view.active_package_identities().len(),
            active_composition_gaps: view.degraded_gaps().to_vec(),
        }))
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        self.recover_pending_release_rebase()?;
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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

        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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

        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        // Guidance already derived the canonical packets from this exact
        // projection and snapshot. Later pre-commit checks still reject any
        // project, registry, or ledger drift before the packet can be recorded.
        let packet = guidance
            .authorization
            .action_packets
            .into_iter()
            .find(|packet| packet.packet_digest == audit.action_packet_digest)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        validate_broker_packet_audit(&packet, &semantic_input, &audit, &broker_registry_digest)?;
        let (packet, action_event, phase_may_advance) = if matches!(
            &semantic_input,
            WorkflowBrokerSemanticInput::IntentRevision { .. }
        ) {
            broker_intent_event_from_semantic(&projection, packet, semantic_input, &audit)?
        } else {
            let closed_input = broker_semantic_input_to_closed(semantic_input)?;
            let mut prepared = prepare_authorization_from_packet(
                effective.document(),
                &projection,
                &self.binding.project_root,
                packet,
                closed_input,
                audit.issued_at_unix,
            )?;
            bound_prepared_expiry(&mut prepared, audit.expires_at_unix)?;
            broker_action_event_from_prepared(
                effective.document(),
                &self.binding.project_root,
                prepared,
                &audit,
                &broker_registry_digest,
            )?
        };

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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        self.recover_pending_release_rebase()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let successor = registry.adjacent_successor(active);
        let available = successor.map(|target| target.release().clone());
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
        let rebase_plan = if domain_pack_rebase_required {
            successor
                .map(|target| {
                    self.derive_domain_pack_rebase_plan(
                        active,
                        target,
                        &effective,
                        &domain,
                        &head_digest,
                        &snapshot_digest,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let rebase_plan_digest = rebase_plan
            .as_ref()
            .map(|plan| plan.domain_pack_rebase_plan.plan_digest.clone());
        let rebase_argv = rebase_plan.as_ref().map(|plan| {
            vec![
                "forge-core".to_owned(),
                "workflow".to_owned(),
                "release-rebase-apply".to_owned(),
                "--root".to_owned(),
                self.binding.project_root.display().to_string(),
                "--target-release-id".to_owned(),
                plan.domain_pack_rebase_plan
                    .target_release
                    .release_id
                    .0
                    .clone(),
                "--expected-rebase-plan-digest".to_owned(),
                plan.domain_pack_rebase_plan.plan_digest.clone(),
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
            rebase_plan_digest,
            rebase_argv,
        })
    }

    /// Recompute and return an exact-CAS, read-only coordinated rebase plan.
    /// No lifecycle pointer or workflow ledger event is written by this method.
    ///
    /// # Errors
    ///
    /// Rejects stale plan digests, unreconciled joined epochs, non-adjacent
    /// targets, or any invalid durable authority input without mutation.
    pub fn release_rebase_plan(
        &self,
        target_release_id: &StableId,
        expected_rebase_plan_digest: &str,
    ) -> Result<DomainPackRebasePlanDocument, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        if !domain.has_active_generation() {
            return Err(WorkflowGovernanceAdapterError::DomainPackGenerationMissing);
        }
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let source = self.resolve_active_release(&registry, &projection)?;
        let target = registry.release_by_id(target_release_id).ok_or_else(|| {
            WorkflowGovernanceAdapterError::UnknownRelease(target_release_id.0.clone())
        })?;
        if !target.is_adjacent_successor_of(source) {
            return Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent);
        }
        let effective = domain.admit_effective(source)?;
        if projection.active_effective_bundle_identity().as_ref() != Some(effective.identity()) {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let head_digest = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let snapshot_digest = project_snapshot_digest(&self.binding.project_root)?;
        let plan = self.derive_domain_pack_rebase_plan(
            source,
            target,
            &effective,
            &domain,
            &head_digest,
            &snapshot_digest,
        )?;
        if plan.domain_pack_rebase_plan.plan_digest != expected_rebase_plan_digest
            || project_snapshot_digest(&self.binding.project_root)? != snapshot_digest
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        Ok(plan)
    }

    /// Complete a joined Core/Domain-Pack rebase after the lifecycle TCB has
    /// committed exactly one target-Core generation. The lifecycle pointer is
    /// acquired first; the workflow WAL then advances both effective and core
    /// identities in one record. A crash before this method is recoverable by
    /// replaying it with the original plan CAS.
    ///
    /// # Errors
    ///
    /// Fails closed unless the old workflow head and the new lifecycle
    /// generation form the exact endpoints committed by `plan`.
    pub fn complete_release_rebase(
        &self,
        plan: &DomainPackRebasePlanDocument,
    ) -> Result<WorkflowGovernanceReleaseUpgradeReceipt, WorkflowGovernanceAdapterError> {
        if !verify_domain_pack_rebase_plan(plan) {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let plan = &plan.domain_pack_rebase_plan;
        if !plan.mutation_allowed || !plan.actionable_gaps.is_empty() {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseApplyUnavailable);
        }
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(&self.binding.state_root)?;
        let material = domain
            .rebase_material()?
            .ok_or(WorkflowGovernanceAdapterError::DomainPackGenerationMissing)?;
        let DomainPackLifecycleOperation::RebaseCore {
            target_release_id,
            expected_from_core_digest,
            target_core_digest,
        } = &material.lifecycle_operation
        else {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        };
        if target_release_id != &plan.target_release.release_id
            || expected_from_core_digest != &plan.source_core.bundle_digest
            || target_core_digest != &plan.target_core.bundle_digest
            || material.generation != plan.exact_cas.expected_generation.saturating_add(1)
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let source = self.resolve_active_release(&registry, &projection)?;
        let target = registry
            .release_by_id(&plan.target_release.release_id)
            .ok_or_else(|| {
                WorkflowGovernanceAdapterError::UnknownRelease(
                    plan.target_release.release_id.0.clone(),
                )
            })?;
        if source.release() != &plan.source_release
            || target.release() != &plan.target_release
            || !target.is_adjacent_successor_of(source)
            || projection.head_digest.as_deref()
                != Some(plan.exact_cas.expected_workflow_ledger_head_digest.as_str())
            || project_snapshot_digest(&self.binding.project_root)?
                != plan.exact_cas.expected_project_snapshot_digest
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let from_effective = projection
            .active_effective_bundle_identity()
            .ok_or(WorkflowGovernanceAdapterError::DomainPackGenerationMissing)?;
        if from_effective.effective_runtime_bundle.bundle_digest
            != plan.exact_cas.expected_effective_bundle_digest
            || from_effective.receipt_context_digest
                != plan.exact_cas.expected_receipt_context_digest
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let target_core = derive_domain_pack_core_binding(target)?;
        if target_core != plan.target_core || material.source_core != target_core {
            return Err(WorkflowGovernanceAdapterError::DomainPackCoreMismatch);
        }
        let to_effective = domain.admit_effective(target)?;
        let target_generation = to_effective
            .identity()
            .domain_pack_generation
            .as_ref()
            .ok_or(WorkflowGovernanceAdapterError::DomainPackGenerationMissing)?;
        if target_generation.generation != material.generation
            || target_generation.active_lock_digest != material.active_lock_digest
            || target_generation.composition_digest != material.composition_digest
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let release_transition = ReleaseUpgradedEvent {
            from_release: source.release().clone(),
            to_release: target.release().clone(),
            from_runtime_bundle: source.runtime_bundle().clone(),
            to_runtime_bundle: target.runtime_bundle().clone(),
            registry_provenance: registry.registry_provenance(),
            admission_proof: registry.admission_proof(
                source,
                target,
                &plan.exact_cas.expected_project_snapshot_digest,
            )?,
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            prior_ledger_head_digest: plan.exact_cas.expected_workflow_ledger_head_digest.clone(),
        };
        let carryover = domain_pack_receipt_carryover(&from_effective, to_effective.identity());
        let event = CoreDomainPackRebasedEvent {
            release_transition,
            from_effective_bundle: from_effective,
            to_effective_bundle: to_effective.identity().clone(),
            receipt_carryover: carryover,
            prior_ledger_head_digest: plan.exact_cas.expected_workflow_ledger_head_digest.clone(),
        };
        let source_identity = self.identity(source);
        let target_identity = self.identity(target);
        let state_version = projection
            .current_state_version()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        if project_snapshot_digest(&self.binding.project_root)?
            != plan.exact_cas.expected_project_snapshot_digest
        {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let record = ledger.transition_core_domain_pack_rebase_unchecked_tcb(
            &plan.exact_cas.expected_workflow_ledger_head_digest,
            &source_identity,
            &target_identity,
            state_version,
            event,
        )?;
        let committed = ledger.recover()?;
        let active = self.resolve_active_release(&registry, &committed)?;
        if active.release() != target.release()
            || committed.active_effective_bundle_identity().as_ref()
                != Some(to_effective.identity())
        {
            return Err(WorkflowGovernanceAdapterError::ReleaseCommitIndeterminate);
        }
        Self::release_upgrade_receipt(
            WorkflowGovernanceReleaseUpgradeStatus::Upgraded,
            &registry,
            active,
            &committed,
            Some(record),
            &plan.exact_cas.expected_project_snapshot_digest,
        )
    }

    fn recover_pending_release_rebase(&self) -> Result<bool, WorkflowGovernanceAdapterError> {
        let path = self
            .binding
            .state_root
            .join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(WorkflowGovernanceAdapterError::Path {
                    field: "domain_pack_rebase_plan",
                    path,
                    source: error.to_string(),
                });
            }
        };
        if !metadata.is_file() || metadata.len() > DOMAIN_PACK_REBASE_PLAN_MAX_BYTES {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let bytes = fs::read(&path).map_err(|error| WorkflowGovernanceAdapterError::Path {
            field: "domain_pack_rebase_plan",
            path: path.clone(),
            source: error.to_string(),
        })?;
        let plan: DomainPackRebasePlanDocument =
            yaml_serde::from_slice(&bytes).map_err(|error| {
                WorkflowGovernanceAdapterError::ProjectBinding {
                    source: format!(
                        "invalid persisted rebase plan '{}': {error}",
                        path.display()
                    ),
                }
            })?;
        if !verify_domain_pack_rebase_plan(&plan) {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        let lifecycle = lock_domain_pack_lifecycle(&self.binding.state_root)?;
        let source = lifecycle.active_rebase_source()?;
        let expected_generation = plan
            .domain_pack_rebase_plan
            .exact_cas
            .expected_generation
            .saturating_add(1);
        let lifecycle_is_committed_target = matches!(
            &source.lifecycle_operation,
            DomainPackLifecycleOperation::RebaseCore {
                target_release_id,
                expected_from_core_digest,
                target_core_digest,
            } if target_release_id == &plan.domain_pack_rebase_plan.target_release.release_id
                && expected_from_core_digest == &plan.domain_pack_rebase_plan.source_core.bundle_digest
                && target_core_digest == &plan.domain_pack_rebase_plan.target_core.bundle_digest
                && source.pointer.domain_pack_active_pointer.generation == expected_generation
        );
        drop(lifecycle);
        if !lifecycle_is_committed_target {
            return Ok(false);
        }
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        drop(ledger);
        let active = self.resolve_active_release(&registry, &projection)?;
        if active.release() == &plan.domain_pack_rebase_plan.target_release {
            return Ok(false);
        }
        if active.release() != &plan.domain_pack_rebase_plan.source_release {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
        }
        self.complete_release_rebase(&plan)?;
        Ok(true)
    }

    fn derive_domain_pack_rebase_plan(
        &self,
        source: &AdmittedWorkflowGovernanceRelease,
        target: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle<'_>,
        domain: &LockedWorkflowDomainPackContext,
        workflow_ledger_head_digest: &str,
        project_snapshot_digest: &str,
    ) -> Result<DomainPackRebasePlanDocument, WorkflowGovernanceAdapterError> {
        let material = domain
            .rebase_material()?
            .ok_or(WorkflowGovernanceAdapterError::DomainPackGenerationMissing)?;
        let source_core = derive_domain_pack_core_binding(source)?;
        if source_core != material.source_core {
            return Err(WorkflowGovernanceAdapterError::DomainPackCoreMismatch);
        }
        let target_core = derive_domain_pack_core_binding(target)?;
        Ok(plan_domain_pack_rebase(&DomainPackRebasePlanInput {
            project_id: self.binding.project_id.clone(),
            source_release: source.release().clone(),
            target_release: target.release().clone(),
            source_core,
            target_core,
            target_workflow_receipt_carryover: target.receipt_carryover(),
            effective_identity: effective.identity().clone(),
            lifecycle_operation: material.lifecycle_operation,
            generation: material.generation,
            lifecycle_pointer_digest: material.lifecycle_pointer_digest,
            lifecycle_head_digest: material.lifecycle_head_digest,
            active_lock_digest: material.active_lock_digest,
            composition_digest: material.composition_digest,
            supply_chain_registry_digest: material.supply_chain_registry_digest,
            reviewer_registry_digest: material.reviewer_registry_digest,
            reviewed_registry_digest: material.reviewed_registry_digest,
            active_package_count: material.active_package_count,
            active_composition_gaps: material.active_composition_gaps,
            workflow_ledger_head_digest: workflow_ledger_head_digest.to_owned(),
            project_snapshot_digest: project_snapshot_digest.to_owned(),
        })?)
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let historical_consequences_digest = sha256_content_hash(
            &serde_json_canonicalizer::to_vec(&selected_alternative.consequences).map_err(
                |error| WorkflowGovernanceAdapterError::Canonicalization(error.to_string()),
            )?,
        );
        let historical_authority = !durable_assurance_is_enforced(effective.document());
        let consequences_current = if historical_authority
            && request.consequences_ack_digest == historical_consequences_digest
        {
            true
        } else {
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
                    trusted_broker_registry_digest: self
                        .current_trusted_broker_registry_digest()?,
                    readiness_target: policy.routing.readiness_target,
                },
                human_authority("workflow.decision.resolve"),
                WorkflowAuthorizationInputContract::Decision {
                    decision_ref: rule.id.clone(),
                    alternatives: rule.alternatives.clone(),
                    recommended_alternative_ref: rule.recommended_alternative_ref.clone(),
                },
            )?;
            request.consequences_ack_digest
                == decision_consequences_ack_digest(
                    &decision_packet.packet_digest,
                    &rule.id,
                    &selected_alternative.id,
                    &selected_alternative.consequences,
                )?
        };
        if request.readiness_target != readiness_name(policy.routing.readiness_target)
            || !consequences_current
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
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
            let broker_registry_digest = self.current_trusted_broker_registry_state()?.digest;
            let current_receipts = derive_receipts(
                effective.document(),
                &projection,
                &self.binding.project_root,
                &fresh.snapshot_digest,
                now,
                registry_digest.as_deref(),
                broker_registry_digest.as_deref(),
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
            let (event, joined_rebase) = match &record.event {
                WorkflowGovernanceEvent::ReleaseUpgraded(event) => (event, false),
                WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                    (&event.release_transition, true)
                }
                _ => continue,
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
                || if joined_rebase {
                    event.receipt_carryover != WorkflowReceiptCarryover::InvalidateAll
                } else {
                    event.receipt_carryover != target.receipt_carryover()
                }
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
            let event = match &record.event {
                WorkflowGovernanceEvent::ReleaseUpgraded(event) => event,
                WorkflowGovernanceEvent::CoreDomainPackRebased(event) => &event.release_transition,
                _ => return None,
            };
            (event.to_release.release_id == admitted.release().release_id)
                .then(|| event.registry_provenance.clone())
        });
        WorkflowGovernanceReleaseAudit {
            release: admitted.release().clone(),
            runtime_bundle: admitted.runtime_bundle().clone(),
            registry: transition_provenance.unwrap_or_else(|| registry.registry_provenance()),
            pin_origin: if projection.records.iter().any(|record| {
                matches!(
                    record.event,
                    WorkflowGovernanceEvent::ReleaseUpgraded(_)
                        | WorkflowGovernanceEvent::CoreDomainPackRebased(_)
                )
            }) {
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
            trusted_broker_registry.digest.as_deref(),
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
        let base_assurance_projection = project_durable_assurance(&projection.records)?;
        let mut assurance_facts = if let Some(base) = base_assurance_projection.as_ref() {
            derive_governed_assurance_facts(
                effective.document(),
                effective.identity(),
                projection,
                base,
                &self.binding.project_root,
                &snapshot_digest,
                selected.routing.readiness_target,
                now,
                trusted_registry_digest.as_deref(),
                trusted_broker_registry.digest.as_deref(),
            )?
        } else {
            GovernedAssuranceFacts {
                target: selected.routing.readiness_target,
                evidence: Vec::new(),
                capabilities: Vec::new(),
                decisions: Vec::new(),
                waivers: Vec::new(),
                action_packets: Vec::new(),
            }
        };
        let assurance_is_enforced = durable_assurance_is_enforced(effective.document());
        let durable_assurance_projection = base_assurance_projection
            .clone()
            .map(|base| {
                if assurance_is_enforced {
                    project_governed_durable_assurance(base, effective.document(), &assurance_facts)
                } else {
                    Ok(base)
                }
            })
            .transpose()?;
        let applicability = derived.applicability.get(&selected.id).copied();
        let policy_guidance_status =
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
        let assurance_has_blockers = assurance_is_enforced
            && durable_assurance_projection
                .as_ref()
                .is_none_or(|projection| !projection.blocker_lenses.is_empty());
        let guidance_status = if assurance_has_blockers {
            WorkflowGovernanceGuidanceStatus::Blocked
        } else {
            policy_guidance_status
        };
        let assurance_source_head = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let assurance_case_digest = durable_assurance_case_digest(
            &self.binding.project_id,
            &snapshot_digest,
            &assurance_source_head,
            projection.current_state_version().unwrap_or_default(),
            &effective.identity().effective_runtime_bundle.bundle_digest,
            durable_assurance_projection
                .as_ref()
                .map(|projection| projection.projection_digest.as_str()),
        )?;
        let durable_assurance = match durable_assurance_projection {
            Some(projection) => {
                let blockers = durable_assurance_blockers(&projection);
                WorkflowDurableAssuranceGuidance {
                    status: WorkflowDurableAssuranceStatus::IntentAccepted,
                    blockers,
                    current_snapshot_digest: snapshot_digest.clone(),
                    source_ledger_head_digest: assurance_source_head.clone(),
                    case_digest: assurance_case_digest.clone(),
                    projection: Some(projection),
                }
            }
            None => WorkflowDurableAssuranceGuidance {
                status: WorkflowDurableAssuranceStatus::MissingHumanIntent,
                blockers: vec![WorkflowDurableAssuranceBlocker {
                    code: WorkflowDurableAssuranceBlockerCode::MissingAcceptedHumanIntent,
                    lens: None,
                    summary: "A human-origin intent revision must be accepted before governed work can proceed."
                        .to_owned(),
                }],
                current_snapshot_digest: snapshot_digest.clone(),
                source_ledger_head_digest: assurance_source_head,
                case_digest: assurance_case_digest,
                projection: None,
            },
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
            durable_assurance,
            authorization: WorkflowAuthorizationGuidance {
                registry_setup: WorkflowAuthorizationRegistrySetup {
                    principal_registry: registry_setup_status(trusted_registry_digest.as_deref()),
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
            Some(&assurance_facts),
            trusted_registry_digest.clone(),
            trusted_broker_registry.digest.clone(),
        )?;
        assurance_facts.action_packets = action_packets
            .iter()
            .map(|packet| GovernedAssuranceActionPacketFact {
                policy_ref: packet.binding.policy_ref.clone(),
                subject_ref: packet.binding.subject_ref.clone(),
                packet_digest: packet.packet_digest.clone(),
            })
            .collect();
        if let Some(base) = base_assurance_projection {
            let final_projection = if assurance_is_enforced {
                project_governed_durable_assurance(base, effective.document(), &assurance_facts)?
            } else {
                base
            };
            let final_case_digest = durable_assurance_case_digest(
                &self.binding.project_id,
                &guidance.snapshot_digest,
                &guidance.ledger_head_digest,
                guidance.state_version,
                &effective.identity().effective_runtime_bundle.bundle_digest,
                Some(&final_projection.projection_digest),
            )?;
            guidance.status =
                if !assurance_is_enforced || final_projection.blocker_lenses.is_empty() {
                    policy_guidance_status
                } else {
                    WorkflowGovernanceGuidanceStatus::Blocked
                };
            guidance.durable_assurance.blockers = durable_assurance_blockers(&final_projection);
            guidance.durable_assurance.case_digest = final_case_digest;
            guidance.durable_assurance.projection = Some(final_projection);
        }
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
        let trusted_broker_registry_digest = self.current_trusted_broker_registry_state()?.digest;
        let derived = derive_receipts(
            effective.document(),
            projection,
            &self.binding.project_root,
            &snapshot,
            now,
            trusted_registry_digest.as_deref(),
            trusted_broker_registry_digest.as_deref(),
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
        let boundary_target = effective
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
            .map(|policy| policy.routing.readiness_target)
            .max_by_key(|target| target.rank())
            .unwrap_or(ReadinessTarget::Explore);
        let base_assurance = project_durable_assurance(&projection.records)?;
        let assurance_is_enforced = durable_assurance_is_enforced(effective.document());
        let governed_assurance = if let Some(base) = base_assurance {
            if assurance_is_enforced {
                let facts = derive_governed_assurance_facts(
                    effective.document(),
                    effective.identity(),
                    projection,
                    &base,
                    &self.binding.project_root,
                    &snapshot,
                    boundary_target,
                    now,
                    trusted_registry_digest.as_deref(),
                    trusted_broker_registry_digest.as_deref(),
                )?;
                Some(project_governed_durable_assurance(
                    base,
                    effective.document(),
                    &facts,
                )?)
            } else {
                Some(base)
            }
        } else {
            None
        };
        if !phase_advance_allowed_by_assurance(
            governed_assurance.as_ref(),
            phase_done,
            assurance_is_enforced,
        ) {
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

fn phase_advance_allowed_by_assurance(
    assurance: Option<&DurableAssuranceProjection>,
    legacy_phase_done: bool,
    assurance_is_enforced: bool,
) -> bool {
    if !legacy_phase_done {
        return false;
    }
    !assurance_is_enforced || assurance.is_some_and(|assurance| assurance.blocker_lenses.is_empty())
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
    pub durable_assurance: WorkflowDurableAssuranceGuidance,
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
    pub rebase_plan_digest: Option<String>,
    pub rebase_argv: Option<Vec<String>>,
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
    DomainPackRebasePlan(DomainPackRebasePlanError),
    EffectiveBundle(EffectiveWorkflowGovernanceBundleError),
    Ledger(WorkflowGovernanceLedgerError),
    ActionReplay(WorkflowActionReplayError),
    TrustedSnapshot(TrustedWorkflowGovernanceSnapshotError),
    Evaluation(WorkflowGovernanceRejection),
    AssuranceProjection(AssuranceProjectionError),
    LedgerIdentityMismatch,
    LedgerUninitialized,
    UnknownRelease(String),
    ReleaseNotAdjacent,
    ReleasePolicyDrift,
    ReleaseCasMismatch,
    ReleaseChainInvalid,
    ReleaseCommitIndeterminate,
    DomainPackRebaseCasMismatch,
    DomainPackRebaseApplyUnavailable,
    DomainPackRebaseLifecycle(String),
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
            Self::DomainPackRebasePlan(error) => {
                write!(f, "Domain Pack rebase planning failed: {error}")
            }
            Self::Ledger(error) => write!(f, "governance ledger failed: {error}"),
            Self::ActionReplay(error) => write!(f, "workflow action replay failed: {error}"),
            Self::TrustedSnapshot(error) => write!(f, "trusted snapshot failed: {error:?}"),
            Self::Evaluation(error) => write!(f, "governance evaluation rejected: {:?}", error.issues),
            Self::AssuranceProjection(error) => {
                write!(f, "durable Assurance projection rejected: {error}")
            }
            Self::LedgerIdentityMismatch => f.write_str("governance ledger identity does not match the resolved project and admitted bundle"),
            Self::LedgerUninitialized => f.write_str("governance ledger is not initialized; run workflow init"),
            Self::UnknownRelease(id) => write!(f, "unknown admitted workflow release {id}"),
            Self::ReleaseNotAdjacent => f.write_str("target workflow release is not the exact adjacent successor"),
            Self::ReleasePolicyDrift => f.write_str("workflow release policy set drift forbids receipt carryover"),
            Self::ReleaseCasMismatch => f.write_str("workflow release upgrade CAS failed; refresh release status"),
            Self::ReleaseChainInvalid => f.write_str("durable workflow release transition chain is not admitted"),
            Self::ReleaseCommitIndeterminate => f.write_str("workflow release commit recovery did not resolve to source or requested target"),
            Self::DomainPackRebaseCasMismatch => f.write_str("DomainPackRebaseCasMismatch: joined Core/Domain Pack rebase plan is stale; refresh release status"),
            Self::DomainPackRebaseApplyUnavailable => f.write_str("DomainPackRebaseApplyUnavailable: rebase plan is not ready for TCB revalidation"),
            Self::DomainPackRebaseLifecycle(reason) => write!(f, "DomainPackRebaseLifecycle: {reason}"),
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
impl From<DomainPackRebasePlanError> for WorkflowGovernanceAdapterError {
    fn from(value: DomainPackRebasePlanError) -> Self {
        Self::DomainPackRebasePlan(value)
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

impl From<AssuranceProjectionError> for WorkflowGovernanceAdapterError {
    fn from(value: AssuranceProjectionError) -> Self {
        Self::AssuranceProjection(value)
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

/// Trust root retained while deriving a receipt. Broker provenance remains
/// structured so later Assurance projection can consume profile/separation
/// metadata without re-inferring it from free-form evidence fields.
#[derive(Debug, Clone, Copy)]
enum DerivedReceiptTrustRoot<'a> {
    LocalPrincipalRegistry,
    ExternalBroker(&'a BrokerOriginAppliedEvent),
}

fn receipt_trust_root<'a>(
    records: &'a [WorkflowGovernanceLedgerRecord],
    index: usize,
    action_record: &WorkflowGovernanceLedgerRecord,
    action_registry_digest: &str,
    trusted_principal_registry_digest: Option<&str>,
    trusted_broker_registry_digest: Option<&str>,
) -> Option<DerivedReceiptTrustRoot<'a>> {
    if let Some(origin_record) = records.get(index + 1) {
        if let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &origin_record.event {
            let exact = origin.action_record_digest == action_record.record_digest
                && origin_record.previous_record_digest.as_deref()
                    == Some(action_record.record_digest.as_str())
                && origin_record.project_id == action_record.project_id
                && origin_record.bundle_id == action_record.bundle_id
                && origin_record.bundle_digest == action_record.bundle_digest
                && origin_record.state_version == action_record.state_version
                && origin.broker_registry_digest == action_registry_digest
                && trusted_broker_registry_digest == Some(origin.broker_registry_digest.as_str())
                && origin.issued_at_unix < origin.expires_at_unix;
            return exact.then_some(DerivedReceiptTrustRoot::ExternalBroker(origin));
        }
    }
    (trusted_principal_registry_digest == Some(action_registry_digest))
        .then_some(DerivedReceiptTrustRoot::LocalPrincipalRegistry)
}

fn broker_common_binding(
    origin: &BrokerOriginAppliedEvent,
    credential_id: &StableId,
    public_key_fingerprint: &str,
    action_time: u64,
) -> bool {
    origin.issuer_id == *credential_id
        && origin.public_key_fingerprint == public_key_fingerprint
        && origin.issued_at_unix == action_time
}

fn evidence_time_is_current(
    observed_at_unix: u64,
    expires_at_unix: Option<u64>,
    evaluator_max_age_seconds: u64,
    now: u64,
    admitted_by_external_broker: bool,
) -> bool {
    observed_at_unix <= now
        && now.saturating_sub(observed_at_unix) <= evaluator_max_age_seconds
        && (admitted_by_external_broker || expires_at_unix.is_none_or(|expires| now <= expires))
}

fn broker_evidence_profile_allowed(
    provider: WorkflowEvaluatorProvider,
    profile: WorkflowBrokerOriginProfile,
) -> bool {
    match provider {
        WorkflowEvaluatorProvider::AuthorizedHuman => profile == WorkflowBrokerOriginProfile::Human,
        WorkflowEvaluatorProvider::IndependentReviewer => {
            profile == WorkflowBrokerOriginProfile::Reviewer
        }
        WorkflowEvaluatorProvider::RepositoryInspector
        | WorkflowEvaluatorProvider::DeterministicTool
        | WorkflowEvaluatorProvider::RepresentativeRuntime => {
            profile == WorkflowBrokerOriginProfile::Runtime
        }
        WorkflowEvaluatorProvider::ExternalAuthority
        | WorkflowEvaluatorProvider::ResearchSource => matches!(
            profile,
            WorkflowBrokerOriginProfile::Reviewer | WorkflowBrokerOriginProfile::Runtime
        ),
    }
}

fn derive_receipts(
    bundle: &WorkflowGovernanceBundleDocument,
    projection: &WorkflowGovernanceLedgerProjection,
    project_root: &Path,
    snapshot_digest: &str,
    now: u64,
    trusted_registry_digest: Option<&str>,
    trusted_broker_registry_digest: Option<&str>,
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
    let mut current_evidence_receipt_digests = BTreeSet::new();
    let mut signal_states =
        BTreeMap::<WorkflowGovernanceSignal, (bool, StableId, u64, String, bool)>::new();
    for (index, record) in receipt_records.iter().enumerate() {
        if revoked.contains(&(record.record_id.clone(), record.record_digest.clone())) {
            continue;
        }
        if let WorkflowGovernanceEvent::SignalChanged(event) = &record.event {
            let authority = receipt_trust_root(
                receipt_records,
                index,
                record,
                &event.authorization_registry_digest,
                trusted_registry_digest,
                trusted_broker_registry_digest,
            );
            let authority_current = match authority {
                Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                    origin.issuer_profile == WorkflowBrokerOriginProfile::Runtime
                        && origin.origin_principal_id == event.changed_by
                        && broker_common_binding(
                            origin,
                            &event.credential_id,
                            &event.public_key_fingerprint,
                            event.observed_at_unix,
                        )
                        && event.expires_at_unix <= origin.expires_at_unix
                }
                None => false,
            };
            let trusted = event.observed_at_unix <= now
                && now <= event.expires_at_unix
                && authority_current
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
    for (index, record) in receipt_records.iter().enumerate() {
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
                        .all(|digest| current_evidence_receipt_digests.contains(digest)) =>
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
            WorkflowGovernanceEvent::ApplicabilityAssessed(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        origin.issuer_profile == WorkflowBrokerOriginProfile::Human
                            && origin.origin_principal_id == event.assessed_by
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.observed_at_unix,
                            )
                            && event.expires_at_unix <= origin.expires_at_unix
                    }
                    None => false,
                };
                if event.observed_at_unix <= now
                    && now <= event.expires_at_unix
                    && authority_current
                    && event.evaluator_ref.0 == WORKFLOW_APPLICABILITY_EVALUATOR_REF
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && content_addressed_basis_current(project_root, &event.basis)?
                    && content_addressed_basis_digest(&event.basis)? == event.basis_digest
                {
                    derived
                        .applicability
                        .insert(event.policy_ref.clone(), event.applicable);
                }
            }
            WorkflowGovernanceEvent::CapabilityProbed(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        origin.issuer_profile == WorkflowBrokerOriginProfile::Runtime
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.observed_at_unix,
                            )
                            && event
                                .expires_at_unix
                                .is_none_or(|expires| expires <= origin.expires_at_unix)
                    }
                    None => false,
                };
                let subject_is_current =
                    subject_current(project_root, snapshot_digest, &event.subject)?;
                let snapshot_is_current = event.subject.kind
                    == WorkflowEvidenceSubjectKind::Artifact
                    || event.snapshot_digest == snapshot_digest;
                if event.available
                    && event.observed_at_unix <= now
                    && event.expires_at_unix.is_none_or(|expires| now <= expires)
                    && authority_current
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && subject_is_current
                    && snapshot_is_current
                {
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
            WorkflowGovernanceEvent::DecisionResolved(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        origin.issuer_profile == WorkflowBrokerOriginProfile::Human
                            && origin.origin_principal_id == event.principal
                            && origin.broker_event_digest == event.authorization_intent_digest
                            && origin.signature_fingerprint == event.signature_fingerprint
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.resolved_at_unix,
                            )
                    }
                    None => false,
                };
                if event.resolved_at_unix <= now
                    && authority_current
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                {
                    derived
                        .resolved_decision_refs
                        .insert(event.decision_ref.clone());
                }
            }
            WorkflowGovernanceEvent::EvaluatorObserved(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        broker_evidence_profile_allowed(event.provider, origin.issuer_profile)
                            && event.provenance.principal.as_ref()
                                == Some(&origin.origin_principal_id)
                            && event.provenance.producer_ref == origin.issuer_id
                            && event.provenance.method
                                == format!(
                                    "verified_workflow_broker:{}",
                                    origin.broker_event_digest
                                )
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.observed_at_unix,
                            )
                    }
                    None => false,
                };
                if event.observed_at_unix > now
                    || !authority_current
                    || record.previous_record_digest.as_deref()
                        != Some(event.ledger_head_digest.as_str())
                {
                    continue;
                }
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
                if evaluator.provider != event.provider
                    || !evaluator.accepted_evidence_kinds.contains(&event.kind)
                    || event.strength < evaluator.minimum_strength
                {
                    continue;
                }
                // A broker envelope's short expiry bounds when Forge may admit
                // the signed observation. After admission, evaluator policy owns
                // evidence freshness; otherwise a five-minute broker envelope
                // silently overrides a multi-day evaluator max age.
                let time_current = evidence_time_is_current(
                    event.observed_at_unix,
                    event.expires_at_unix,
                    evaluator.max_age_seconds,
                    now,
                    matches!(authority, Some(DerivedReceiptTrustRoot::ExternalBroker(_))),
                );
                let subject_current =
                    subject_current(project_root, snapshot_digest, &event.subject)?;
                let snapshot_current = event.subject.kind == WorkflowEvidenceSubjectKind::Artifact
                    || event.snapshot_digest == snapshot_digest;
                let freshness = if time_current && subject_current && snapshot_current {
                    WorkflowEvidenceFreshness::Current
                } else {
                    WorkflowEvidenceFreshness::Stale
                };
                if freshness == WorkflowEvidenceFreshness::Current {
                    current_evidence_receipt_digests.insert(record.record_digest.clone());
                }
                derived.evidence.push(WorkflowEvidenceObservation {
                    evidence_ref: event.provenance.semantic_identity.0.clone(),
                    claim_ref: event.claim_ref.clone(),
                    evaluator_ref: event.evaluator_ref.clone(),
                    principal: event.provenance.principal.clone(),
                    kind: event.kind,
                    strength: event.strength,
                    freshness,
                    outcome: event.outcome,
                });
            }
            WorkflowGovernanceEvent::WaiverAuthorized(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        origin.issuer_profile == WorkflowBrokerOriginProfile::Human
                            && origin.origin_principal_id == event.principal
                            && origin.broker_event_digest == event.authorization_intent_digest
                            && origin.signature_fingerprint == event.signature_fingerprint
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.authorized_at_unix,
                            )
                            && event.expires_at_unix <= origin.expires_at_unix
                    }
                    None => false,
                };
                if event.authorized_at_unix <= now
                    && now <= event.expires_at_unix
                    && authority_current
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && subject_current(project_root, snapshot_digest, &event.subject)?
                {
                    current_evidence_receipt_digests.insert(record.record_digest.clone());
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

#[allow(clippy::too_many_arguments)] // The assurance projection binds all independent authority roots explicitly.
fn derive_governed_assurance_facts(
    bundle: &WorkflowGovernanceBundleDocument,
    effective_identity: &WorkflowEffectiveBundleIdentity,
    projection: &WorkflowGovernanceLedgerProjection,
    assurance: &DurableAssuranceProjection,
    project_root: &Path,
    snapshot_digest: &str,
    target: ReadinessTarget,
    now: u64,
    trusted_principal_registry_digest: Option<&str>,
    trusted_broker_registry_digest: Option<&str>,
) -> Result<GovernedAssuranceFacts, WorkflowGovernanceAdapterError> {
    let active_effective_identity = projection.active_effective_bundle_identity();
    match active_effective_identity.as_ref() {
        Some(active) if active != effective_identity => {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        None if effective_identity.domain_pack_generation.is_some() => {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        _ => {}
    }
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
    let mut facts = GovernedAssuranceFacts {
        target,
        evidence: Vec::new(),
        capabilities: Vec::new(),
        decisions: Vec::new(),
        waivers: Vec::new(),
        action_packets: Vec::new(),
    };

    for (index, record) in receipt_records.iter().enumerate() {
        if record.sequence <= assurance.binding.accepted_sequence
            || revoked.contains(&(record.record_id.clone(), record.record_digest.clone()))
        {
            continue;
        }
        let adjacent_origin_revoked = receipt_records.get(index + 1).is_some_and(|origin| {
            matches!(
                &origin.event,
                WorkflowGovernanceEvent::BrokerOriginApplied(_)
            ) && revoked.contains(&(origin.record_id.clone(), origin.record_digest.clone()))
        });
        match &record.event {
            WorkflowGovernanceEvent::EvaluatorObserved(event) => {
                let Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_principal_registry_digest,
                    trusted_broker_registry_digest,
                ) else {
                    continue;
                };
                let Some(origin_record) = receipt_records.get(index + 1) else {
                    continue;
                };
                if revoked.contains(&(
                    origin_record.record_id.clone(),
                    origin_record.record_digest.clone(),
                )) || event.observed_at_unix > now
                    || (event.subject.kind != WorkflowEvidenceSubjectKind::Artifact
                        && event.snapshot_digest != snapshot_digest)
                    || record.previous_record_digest.as_deref()
                        != Some(event.ledger_head_digest.as_str())
                    || !subject_current(project_root, snapshot_digest, &event.subject)?
                    || !broker_evidence_profile_allowed(event.provider, origin.issuer_profile)
                    || event.provenance.principal.as_ref() != Some(&origin.origin_principal_id)
                    || event.provenance.producer_ref != origin.issuer_id
                    || event.provenance.method
                        != format!("verified_workflow_broker:{}", origin.broker_event_digest)
                    || !broker_common_binding(
                        origin,
                        &event.credential_id,
                        &event.public_key_fingerprint,
                        event.observed_at_unix,
                    )
                {
                    continue;
                }
                let Some((claim, evaluator)) = bundle
                    .workflow_governance_bundle
                    .policies
                    .iter()
                    .find(|policy| policy.id == event.policy_ref)
                    .and_then(|policy| {
                        policy
                            .claims
                            .iter()
                            .find(|claim| {
                                claim.id == event.claim_ref
                                    && claim.evaluator_ref == event.evaluator_ref
                            })
                            .zip(
                                policy
                                    .evaluators
                                    .iter()
                                    .find(|evaluator| evaluator.id == event.evaluator_ref),
                            )
                    })
                else {
                    continue;
                };
                if evaluator.provider != event.provider
                    || !evaluator.accepted_evidence_kinds.contains(&event.kind)
                    || event.strength < evaluator.minimum_strength
                    || !evidence_time_is_current(
                        event.observed_at_unix,
                        event.expires_at_unix,
                        evaluator.max_age_seconds,
                        now,
                        true,
                    )
                {
                    continue;
                }
                let representative_slice = if claim.assurance_role
                    == Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition)
                    && event.provider == WorkflowEvaluatorProvider::IndependentReviewer
                    && event.kind == WorkflowEvidenceKind::IndependentReview
                    && event.outcome == WorkflowEvidenceOutcome::Pass
                    && origin.issuer_profile == WorkflowBrokerOriginProfile::Reviewer
                    && event.subject.kind == WorkflowEvidenceSubjectKind::Artifact
                {
                    load_representative_slice_definition(
                        project_root,
                        event,
                        &assurance.binding.intent_digest,
                    )
                } else {
                    None
                };
                let representative_slice_definition_digest = match claim.assurance_role {
                    Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition) => {
                        representative_slice
                            .as_ref()
                            .map(|_| event.subject.subject_digest.clone())
                    }
                    Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution) => {
                        let Some(definition) = latest_representative_definition(bundle, &facts)
                        else {
                            continue;
                        };
                        let Some(manifest) = definition.representative_slice.as_ref() else {
                            continue;
                        };
                        if record.sequence <= definition.sequence
                            || event.provider != WorkflowEvaluatorProvider::RepresentativeRuntime
                            || event.kind != WorkflowEvidenceKind::RepresentativeExecution
                            || origin.issuer_profile != WorkflowBrokerOriginProfile::Runtime
                            || event.subject.kind != WorkflowEvidenceSubjectKind::Runtime
                            || event.subject.subject_ref
                                != manifest
                                    .representative_slice
                                    .representative_environment
                                    .runtime_subject_ref
                            || event.subject.subject_digest
                                != manifest
                                    .representative_slice
                                    .representative_environment
                                    .runtime_subject_digest
                            || !manifest
                                .representative_slice
                                .scenarios
                                .iter()
                                .any(|scenario| {
                                    scenario.declared_scenario_digest
                                        == event.provenance.scenario_digest
                                })
                        {
                            continue;
                        }
                        Some(definition.subject_digest.clone())
                    }
                    Some(WorkflowAssuranceClaimRole::LensEvidence) | None => None,
                };
                facts.evidence.push(GovernedAssuranceEvidenceFact {
                    assurance_epoch: assurance.binding.assurance_epoch,
                    sequence: record.sequence,
                    policy_ref: event.policy_ref.clone(),
                    claim_ref: event.claim_ref.clone(),
                    evaluator_ref: event.evaluator_ref.clone(),
                    evidence_ref: event.provenance.semantic_identity.0.clone(),
                    evidence_record_digest: record.record_digest.clone(),
                    origin_record_digest: origin_record.record_digest.clone(),
                    provider: event.provider,
                    kind: event.kind,
                    strength: event.strength,
                    outcome: event.outcome,
                    subject_kind: event.subject.kind,
                    subject_ref: event.subject.subject_ref.clone(),
                    subject_digest: event.subject.subject_digest.clone(),
                    scenario_digest: event.provenance.scenario_digest.clone(),
                    origin_principal: origin.origin_principal_id.clone(),
                    separation_domain: origin.separation_domain.clone(),
                    broker_profile: origin.issuer_profile,
                    representative_slice,
                    representative_slice_definition_digest,
                });
            }
            WorkflowGovernanceEvent::CapabilityProbed(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_principal_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        !adjacent_origin_revoked
                            && origin.issuer_profile == WorkflowBrokerOriginProfile::Runtime
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.observed_at_unix,
                            )
                    }
                    None => false,
                };
                if authority_current
                    && event.observed_at_unix <= now
                    && event.expires_at_unix.is_none_or(|expires| now <= expires)
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && (event.subject.kind == WorkflowEvidenceSubjectKind::Artifact
                        || event.snapshot_digest == snapshot_digest)
                    && subject_current(project_root, snapshot_digest, &event.subject)?
                {
                    facts.capabilities.push(GovernedAssuranceCapabilityFact {
                        assurance_epoch: assurance.binding.assurance_epoch,
                        sequence: record.sequence,
                        policy_ref: event.policy_ref.clone(),
                        capability_ref: event.capability_ref.clone(),
                        available: event.available,
                        receipt_digest: record.record_digest.clone(),
                    });
                }
            }
            WorkflowGovernanceEvent::DecisionResolved(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_principal_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        !adjacent_origin_revoked
                            && origin.issuer_profile == WorkflowBrokerOriginProfile::Human
                            && origin.origin_principal_id == event.principal
                            && origin.broker_event_digest == event.authorization_intent_digest
                            && origin.signature_fingerprint == event.signature_fingerprint
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.resolved_at_unix,
                            )
                    }
                    None => false,
                };
                if authority_current
                    && event.resolved_at_unix <= now
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                {
                    facts.decisions.push(GovernedAssuranceDecisionFact {
                        assurance_epoch: assurance.binding.assurance_epoch,
                        sequence: record.sequence,
                        policy_ref: event.policy_ref.clone(),
                        decision_ref: event.decision_ref.clone(),
                        resolved: true,
                        receipt_digest: record.record_digest.clone(),
                    });
                }
            }
            WorkflowGovernanceEvent::WaiverAuthorized(event) => {
                let authority = receipt_trust_root(
                    receipt_records,
                    index,
                    record,
                    &event.authorization_registry_digest,
                    trusted_principal_registry_digest,
                    trusted_broker_registry_digest,
                );
                let authority_current = match authority {
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => true,
                    Some(DerivedReceiptTrustRoot::ExternalBroker(origin)) => {
                        !adjacent_origin_revoked
                            && origin.issuer_profile == WorkflowBrokerOriginProfile::Human
                            && origin.origin_principal_id == event.principal
                            && origin.broker_event_digest == event.authorization_intent_digest
                            && origin.signature_fingerprint == event.signature_fingerprint
                            && broker_common_binding(
                                origin,
                                &event.credential_id,
                                &event.public_key_fingerprint,
                                event.authorized_at_unix,
                            )
                    }
                    None => false,
                };
                if authority_current
                    && event.authorized_at_unix <= now
                    && now <= event.expires_at_unix
                    && event.snapshot_digest == snapshot_digest
                    && record.previous_record_digest.as_deref()
                        == Some(event.ledger_head_digest.as_str())
                    && subject_current(project_root, snapshot_digest, &event.subject)?
                {
                    facts.waivers.push(GovernedAssuranceWaiverFact {
                        assurance_epoch: assurance.binding.assurance_epoch,
                        sequence: record.sequence,
                        policy_ref: event.policy_ref.clone(),
                        claim_ref: event.claim_ref.clone(),
                        receipt_digest: record.record_digest.clone(),
                        expires_at_unix: event.expires_at_unix,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(facts)
}

fn load_representative_slice_definition(
    project_root: &Path,
    event: &EvaluatorObservedEvent,
    current_intent_digest: &str,
) -> Option<WorkflowRepresentativeSliceDefinitionDocument> {
    let Ok((subject_ref, bytes)) =
        read_confined_file(project_root, Path::new(&event.subject.subject_ref))
    else {
        return None;
    };
    if subject_ref != event.subject.subject_ref
        || sha256_content_hash(&bytes) != event.subject.subject_digest
        || event.provenance.source_ref != event.subject.subject_ref
        || event.provenance.source_digest != event.subject.subject_digest
    {
        return None;
    }
    let Ok(raw) = std::str::from_utf8(&bytes) else {
        return None;
    };
    let Ok(document) = yaml_serde::from_str::<WorkflowRepresentativeSliceDefinitionDocument>(raw)
    else {
        return None;
    };
    if document.representative_slice.intent_digest != current_intent_digest
        || validate_representative_slice_definition(&document, current_intent_digest).is_err()
    {
        return None;
    }
    for scenario in &document.representative_slice.scenarios {
        let Ok((_, scenario_bytes)) =
            read_confined_file(project_root, Path::new(&scenario.scenario_ref))
        else {
            return None;
        };
        if sha256_content_hash(&scenario_bytes) != scenario.declared_scenario_digest {
            return None;
        }
    }
    Some(document)
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
            WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                Some((event.receipt_carryover, false))
            }
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
) -> Result<WorkflowAuthorizationClosedInput, WorkflowGovernanceAdapterError> {
    Ok(match input {
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
        WorkflowBrokerSemanticInput::IntentRevision { .. } => {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        WorkflowBrokerSemanticInput::Signal { active, basis_refs } => {
            WorkflowAuthorizationClosedInput::Signal { active, basis_refs }
        }
        WorkflowBrokerSemanticInput::Waiver { reason } => {
            WorkflowAuthorizationClosedInput::Waiver { reason }
        }
    })
}

fn validate_broker_packet_audit(
    packet: &WorkflowAuthorizationActionPacket,
    input: &WorkflowBrokerSemanticInput,
    audit: &VerifiedWorkflowBrokerEventAudit,
    broker_registry_digest: &str,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let expected_kind = match packet.authorization_kind {
        WorkflowAuthorizationKind::IntentRevision => WorkflowBrokerEventKind::IntentRevision,
        WorkflowAuthorizationKind::Applicability => WorkflowBrokerEventKind::Applicability,
        WorkflowAuthorizationKind::Capability => WorkflowBrokerEventKind::Capability,
        WorkflowAuthorizationKind::Decision => WorkflowBrokerEventKind::Decision,
        WorkflowAuthorizationKind::Evidence => WorkflowBrokerEventKind::Evidence,
        WorkflowAuthorizationKind::Signal => WorkflowBrokerEventKind::Signal,
        WorkflowAuthorizationKind::Waiver => WorkflowBrokerEventKind::Waiver,
    };
    let input_kind = input.kind();
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

fn broker_intent_event_from_semantic(
    projection: &WorkflowGovernanceLedgerProjection,
    packet: WorkflowAuthorizationActionPacket,
    input: WorkflowBrokerSemanticInput,
    audit: &VerifiedWorkflowBrokerEventAudit,
) -> Result<
    (
        WorkflowAuthorizationActionPacket,
        WorkflowGovernanceEvent,
        bool,
    ),
    WorkflowGovernanceAdapterError,
> {
    let WorkflowAuthorizationInputContract::IntentRevision {
        intent_id,
        next_intent_revision,
        next_assurance_epoch,
        ..
    } = &packet.input_contract
    else {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    };
    let WorkflowBrokerSemanticInput::IntentRevision {
        desired_outcome,
        constraints,
        preferences,
        unacceptable_outcomes,
        uncertainties,
        conversation_ref,
        conversation_digest,
    } = input
    else {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    };

    let current = project_durable_assurance(&projection.records)?;
    let (expected_revision, expected_epoch, expected_intent_id, previous_intent_digest) =
        if let Some(current) = current {
            (
                current
                    .binding
                    .intent_revision
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
                current
                    .binding
                    .assurance_epoch
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
                current.binding.intent_id,
                Some(current.binding.intent_digest),
            )
        } else {
            (
                1,
                1,
                StableId(format!("intent.workflow.{}", packet.binding.project_id.0)),
                None,
            )
        };
    if *next_intent_revision != expected_revision
        || *next_assurance_epoch != expected_epoch
        || *intent_id != expected_intent_id
    {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    }

    let intent = WorkflowHumanIntentRevision {
        intent_id: expected_intent_id,
        revision: expected_revision,
        desired_outcome,
        constraints,
        preferences,
        unacceptable_outcomes,
        uncertainties,
        source_conversation_ref: conversation_ref,
        source_conversation_digest: conversation_digest,
    };
    let intent_digest = workflow_human_intent_digest(&intent)?;
    let event = HumanIntentRevisionAcceptedEvent {
        assurance_epoch: expected_epoch,
        intent,
        intent_digest,
        previous_intent_digest,
        snapshot_digest: packet.binding.snapshot_digest.clone(),
        ledger_head_digest: packet.binding.ledger_head_digest.clone(),
        acceptance_action_packet_digest: packet.packet_digest.clone(),
        accepted_by: audit.origin_principal_id.clone(),
        accepted_at_unix: audit.issued_at_unix,
    };
    Ok((
        packet,
        WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event),
        false,
    ))
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
        WorkflowAuthorizationInputContract::IntentRevision { .. } => {
            WorkflowAuthorizationKind::IntentRevision
        }
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
                representative_slice,
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
            let (scenario_ref, scenario_bytes) =
                read_confined_file(project_root, Path::new(&scenario_ref))?;
            let scenario_digest = sha256_content_hash(&scenario_bytes);
            match &representative_slice {
                Some(WorkflowRepresentativeSliceActionBinding::Definition {
                    schema_version,
                    current_intent_digest,
                    ..
                }) => {
                    if subject_kind != WorkflowEvidenceSubjectKind::Artifact
                        || schema_version != WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION
                        || scenario_ref != subject_ref
                    {
                        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
                    }
                    let (_, manifest_bytes) =
                        read_confined_file(project_root, Path::new(&subject_ref))?;
                    if sha256_content_hash(&manifest_bytes) != subject_digest {
                        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
                    }
                    let raw = std::str::from_utf8(&manifest_bytes).map_err(|_| {
                        WorkflowGovernanceAdapterError::AuthorizationBindingMismatch
                    })?;
                    let manifest: WorkflowRepresentativeSliceDefinitionDocument =
                        yaml_serde::from_str(raw).map_err(|_| {
                            WorkflowGovernanceAdapterError::AuthorizationBindingMismatch
                        })?;
                    if validate_representative_slice_definition(&manifest, current_intent_digest)
                        .is_err()
                    {
                        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
                    }
                    for declared in &manifest.representative_slice.scenarios {
                        let (_, bytes) =
                            read_confined_file(project_root, Path::new(&declared.scenario_ref))?;
                        if sha256_content_hash(&bytes) != declared.declared_scenario_digest {
                            return Err(
                                WorkflowGovernanceAdapterError::AuthorizationBindingMismatch,
                            );
                        }
                    }
                }
                Some(WorkflowRepresentativeSliceActionBinding::Execution {
                    runtime_subject_ref,
                    runtime_subject_digest,
                    allowed_scenario_digests,
                    ..
                }) if subject_kind != WorkflowEvidenceSubjectKind::Runtime
                    || &subject_ref != runtime_subject_ref
                    || &subject_digest != runtime_subject_digest
                    || !allowed_scenario_digests.contains(&scenario_digest) =>
                {
                    return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
                }
                Some(WorkflowRepresentativeSliceActionBinding::Execution { .. }) | None => {}
            }
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
                scenario_digest,
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

fn latest_representative_definition<'a>(
    bundle: &WorkflowGovernanceBundleDocument,
    facts: &'a GovernedAssuranceFacts,
) -> Option<&'a GovernedAssuranceEvidenceFact> {
    facts
        .evidence
        .iter()
        .filter(|fact| {
            bundle
                .workflow_governance_bundle
                .policies
                .iter()
                .find(|policy| policy.id == fact.policy_ref)
                .and_then(|policy| {
                    policy
                        .claims
                        .iter()
                        .find(|claim| claim.id == fact.claim_ref)
                })
                .is_some_and(|claim| {
                    claim.assurance_role
                        == Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition)
                })
        })
        .max_by_key(|fact| fact.sequence)
        .filter(|fact| {
            fact.outcome == WorkflowEvidenceOutcome::Pass && fact.representative_slice.is_some()
        })
}

fn authorization_action_packets(
    bundle: &WorkflowGovernanceBundleDocument,
    guidance: &WorkflowGovernanceGuidance,
    derived: &DerivedReceipts,
    assurance_facts: Option<&GovernedAssuranceFacts>,
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

    let (intent_id, next_intent_revision, next_assurance_epoch) =
        if let Some(assurance) = guidance.durable_assurance.projection.as_ref() {
            if assurance.binding.project_id != guidance.project_id {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
            (
                assurance.binding.intent_id.clone(),
                assurance
                    .binding
                    .intent_revision
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
                assurance
                    .binding
                    .assurance_epoch
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
            )
        } else {
            (
                StableId(format!("intent.workflow.{}", guidance.project_id.0)),
                1,
                1,
            )
        };
    packets.push(make_authorization_action_packet(
        WorkflowAuthorizationKind::IntentRevision,
        StableId(format!("packet.workflow.intent-revision.{}", intent_id.0)),
        binding_for(selected, intent_id.clone()),
        human_authority("workflow.intent.accept_revision"),
        WorkflowAuthorizationInputContract::IntentRevision {
            intent_id,
            next_intent_revision,
            next_assurance_epoch,
            desired_outcome_max_bytes: MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
            list_max_items: MAX_WORKFLOW_INTENT_LIST_ITEMS,
            list_item_max_bytes: MAX_WORKFLOW_INTENT_ITEM_BYTES,
            source_ref_max_bytes: MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
            total_max_bytes: MAX_WORKFLOW_INTENT_TOTAL_BYTES,
        },
    )?);

    // Until a human-origin intent is durably accepted, no policy mutation is
    // actionable. The single intent packet is the complete executable next
    // step; policy simulation remains visible only as read-only context.
    if guidance.durable_assurance.projection.is_none() {
        return Ok(packets);
    }

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

    let mut actionable_policies = vec![selected];
    if let Some(assurance_policy) = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == UNIVERSAL_ASSURANCE_POLICY_ID)
        .filter(|policy| policy.id != selected.id)
    {
        actionable_policies.push(assurance_policy);
    }
    for action_policy in actionable_policies {
        for claim in &action_policy.claims {
            let governed_role_complete = guidance
                .durable_assurance
                .projection
                .as_ref()
                .is_some_and(|projection| {
                    projection.lenses.iter().any(|lens| {
                        lens.claims.iter().any(|binding| {
                            binding.policy_ref == action_policy.id
                                && binding.claim_ref == claim.id
                                && matches!(
                                    binding.state,
                                    DurableAssuranceEpistemicState::Verified
                                        | DurableAssuranceEpistemicState::Waived
                                )
                        })
                    })
                });
            let claim_complete = if claim.assurance_role.is_some() {
                governed_role_complete
            } else {
                let result = guidance
                    .simulation
                    .candidate_claim_results
                    .iter()
                    .find(|candidate| candidate.claim_id == claim.id.0)
                    .ok_or_else(|| {
                        WorkflowGovernanceAdapterError::UnknownClaim(claim.id.0.clone())
                    })?;
                matches!(
                    result.status,
                    WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                )
            };
            if claim_complete {
                continue;
            }
            let evaluator = action_policy
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
            let representative_slice = match claim.assurance_role {
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition) => {
                    let intent_digest = guidance
                        .durable_assurance
                        .projection
                        .as_ref()
                        .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?
                        .binding
                        .intent_digest
                        .clone();
                    Some(WorkflowRepresentativeSliceActionBinding::Definition {
                        schema_version: WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION.to_owned(),
                        current_intent_digest: intent_digest,
                        text_max_bytes: MAX_REPRESENTATIVE_SLICE_TEXT_BYTES,
                        list_max_items: MAX_REPRESENTATIVE_SLICE_ITEMS,
                        item_max_bytes: MAX_REPRESENTATIVE_SLICE_ITEM_BYTES,
                        total_max_bytes: MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES,
                    })
                }
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution) => {
                    let Some(definition) = assurance_facts
                        .and_then(|facts| latest_representative_definition(bundle, facts))
                    else {
                        continue;
                    };
                    let manifest = definition
                        .representative_slice
                        .as_ref()
                        .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
                    Some(WorkflowRepresentativeSliceActionBinding::Execution {
                        definition_digest: definition.subject_digest.clone(),
                        definition_receipt_digest: definition.evidence_record_digest.clone(),
                        runtime_subject_ref: manifest
                            .representative_slice
                            .representative_environment
                            .runtime_subject_ref
                            .clone(),
                        runtime_subject_digest: manifest
                            .representative_slice
                            .representative_environment
                            .runtime_subject_digest
                            .clone(),
                        allowed_scenario_digests: manifest
                            .representative_slice
                            .scenarios
                            .iter()
                            .map(|scenario| scenario.declared_scenario_digest.clone())
                            .collect(),
                    })
                }
                Some(WorkflowAssuranceClaimRole::LensEvidence) | None => None,
            };
            let subject_kinds = match claim.assurance_role {
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition) => {
                    vec![WorkflowEvidenceSubjectKind::Artifact]
                }
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution) => {
                    vec![WorkflowEvidenceSubjectKind::Runtime]
                }
                _ => subject_kinds,
            };
            packets.push(make_authorization_action_packet(
                WorkflowAuthorizationKind::Evidence,
                StableId(format!("packet.workflow.evidence.{}", claim.id.0)),
                binding_for(action_policy, claim.id.clone()),
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
                    representative_slice,
                },
            )?);

            if let WorkflowClaimWaiverPolicy::Authorized {
                max_target,
                max_age_seconds,
                ..
            } = &claim.waiver
            {
                let maximum_readiness_target =
                    if max_target.rank() < action_policy.routing.readiness_target.rank() {
                        *max_target
                    } else {
                        action_policy.routing.readiness_target
                    };
                let mut consequence_statements = vec![format!(
                    "Claim {} will be treated as waived without verified evidence: {}",
                    claim.id.0, claim.statement
                )];
                let mut obligations = action_policy
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
                    binding_for(action_policy, claim.id.clone()),
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

fn durable_assurance_case_digest(
    project_id: &StableId,
    current_snapshot_digest: &str,
    source_ledger_head_digest: &str,
    state_version: u64,
    effective_bundle_digest: &str,
    durable_projection_digest: Option<&str>,
) -> Result<String, WorkflowGovernanceAdapterError> {
    let basis = WorkflowDurableAssuranceCaseDigestBasis {
        schema_version: "workflow_durable_assurance_case_v1",
        project_id,
        current_snapshot_digest,
        source_ledger_head_digest,
        state_version,
        effective_bundle_digest,
        durable_projection_digest,
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

fn registry_setup_status(digest: Option<&str>) -> WorkflowAuthorizationRegistrySetupStatus {
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
    use std::fmt::Write as _;

    fn temp_project(label: &str) -> (PathBuf, PathBuf) {
        let fixture_root =
            std::env::temp_dir().join(format!("forge-p5c-adapter-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&fixture_root);
        let root = fixture_root.join("project");
        fs::create_dir_all(root.join(".forge-method")).expect("state root");
        fs::write(root.join("README.md"), b"project\n").expect("project file");
        let root = root.canonicalize().expect("canonical temp");
        let state = root.join(".forge-method");
        (root, state)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(
            String::with_capacity(bytes.len().saturating_mul(2)),
            |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to String cannot fail");
                output
            },
        )
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

    fn install_human_broker_registry(
        adapter: &WorkflowGovernanceProjectAdapter,
        key: &SigningKey,
    ) -> WorkflowBrokerRegistryDocument {
        let document = WorkflowBrokerRegistryDocument {
            schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: adapter.expected_broker_audience(),
            issuers: vec![WorkflowBrokerIssuerEntry {
                issuer_id: StableId("broker.human.test".to_owned()),
                profile: WorkflowBrokerIssuerProfile::Human,
                public_key_hex: hex(key.verifying_key().as_bytes()),
                status: WorkflowBrokerIssuerStatus::Active,
                enrollment: WorkflowBrokerEnrollmentDeclaration {
                    ceremony_ref: "operator://ceremony/human-test".to_owned(),
                    ceremony_digest: format!("sha256:{}", "b".repeat(64)),
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

    fn signed_intent_envelope(
        project_id: &StableId,
        packet: &WorkflowAuthorizationActionPacket,
        key: &SigningKey,
        issued_at_unix: u64,
        nonce: &str,
        desired_outcome: &str,
    ) -> WorkflowBrokerEventEnvelope {
        assert!(matches!(
            &packet.input_contract,
            WorkflowAuthorizationInputContract::IntentRevision { .. }
        ));
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: format!("forge-core:workflow:{}", project_id.0),
            issuer_id: StableId("broker.human.test".to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId("principal.human.origin".to_owned()),
            separation_domain: StableId("human.test.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::IntentRevision,
            project_id: project_id.clone(),
            action_packet_digest: packet.packet_digest.clone(),
            semantic_input: WorkflowBrokerSemanticInput::IntentRevision {
                desired_outcome: desired_outcome.to_owned(),
                constraints: vec!["Keep the governed result recoverable".to_owned()],
                preferences: vec!["Prefer reversible choices".to_owned()],
                unacceptable_outcomes: vec!["Do not claim unverified readiness".to_owned()],
                uncertainties: vec!["Delivery constraints remain unknown".to_owned()],
                conversation_ref: "conversation://test/intent".to_owned(),
                conversation_digest: format!("sha256:{}", "c".repeat(64)),
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

    fn accept_test_intent(adapter: &WorkflowGovernanceProjectAdapter) {
        let key = SigningKey::from_bytes(&[17_u8; 32]);
        let broker_document = install_human_broker_registry(adapter, &key);
        let now = unix_time().expect("clock");
        let packets = adapter.action_packets_at(now).expect("intent packet set");
        assert_eq!(packets.packets.len(), 1);
        let envelope = signed_intent_envelope(
            &packets.project_id,
            &packets.packets[0],
            &key,
            now,
            "test-intent-acceptance-nonce-0001",
            "Build a dependable governed product",
        );
        adapter
            .apply_verified_broker_action(
                AuthorizedWorkflowBrokerRegistry::from_document(broker_document)
                    .expect("authorized broker registry")
                    .verify_event(
                        envelope,
                        &packets.project_id,
                        i64::try_from(now).expect("clock fits i64"),
                        WorkflowBrokerFreshnessPolicy::default(),
                    )
                    .expect("verified intent"),
                now,
            )
            .expect("accepted intent");
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
    fn human_intent_is_the_durable_first_blocker_and_revises_monotonically() {
        let (root, state) = temp_project("durable-human-intent");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");

        let missing = adapter.next().expect("missing-intent guidance");
        assert_eq!(
            missing.durable_assurance.status,
            WorkflowDurableAssuranceStatus::MissingHumanIntent
        );
        assert_eq!(missing.durable_assurance.blockers.len(), 1);
        assert!(missing.durable_assurance.projection.is_none());
        assert_eq!(missing.authorization.action_packets.len(), 1);
        assert_eq!(
            adapter
                .resume()
                .expect("repeat missing-intent view")
                .durable_assurance
                .case_digest,
            missing.durable_assurance.case_digest,
            "missing-intent case identity must be deterministic at one state"
        );
        let first_packet = missing.authorization.action_packets[0].clone();
        assert_eq!(
            first_packet.authorization_kind,
            WorkflowAuthorizationKind::IntentRevision
        );
        assert_eq!(
            first_packet.required_authority.approval_boundary,
            WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker
        );

        let key = SigningKey::from_bytes(&[41_u8; 32]);
        let broker_document = install_human_broker_registry(&adapter, &key);
        let now = unix_time().expect("clock");
        let refreshed = adapter.action_packets_at(now).expect("broker-bound packet");
        assert_eq!(refreshed.packets.len(), 1);
        let first_packet = refreshed.packets[0].clone();
        let first_envelope = signed_intent_envelope(
            &refreshed.project_id,
            &first_packet,
            &key,
            now,
            "human-intent-first-nonce-0001",
            "Enable a novice to create a dependable product",
        );
        let verify = |envelope: WorkflowBrokerEventEnvelope| {
            AuthorizedWorkflowBrokerRegistry::from_document(broker_document.clone())
                .expect("authorized human broker")
                .verify_event(
                    envelope,
                    &refreshed.project_id,
                    i64::try_from(now).expect("clock fits i64"),
                    WorkflowBrokerFreshnessPolicy::default(),
                )
                .expect("verified human intent")
        };
        let first = adapter
            .apply_verified_broker_action(verify(first_envelope.clone()), now)
            .expect("first accepted intent");
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(first_event) =
            &first.action_record.event
        else {
            panic!("typed intent action");
        };
        assert_eq!(first_event.assurance_epoch, 1);
        assert_eq!(first_event.intent.revision, 1);
        assert_eq!(
            first.origin_record.previous_record_digest.as_deref(),
            Some(first.action_record.record_digest.as_str())
        );

        let accepted = &first
            .next
            .durable_assurance
            .projection
            .as_ref()
            .expect("durable assurance projection");
        assert_eq!(accepted.binding.assurance_epoch, 1);
        assert_eq!(accepted.binding.intent_revision, 1);
        assert_eq!(accepted.intent, first_event.intent);
        assert_eq!(
            accepted.lenses.len(),
            forge_core_contracts::UniversalAssuranceLens::ALL.len()
        );
        assert!(accepted.lenses.iter().all(|lens| {
            lens.claim_status == DurableAssuranceEpistemicState::Unknown
                && lens.evidence.is_empty()
                && lens.claims.is_empty()
        }));
        assert_eq!(
            first.next.status,
            WorkflowGovernanceGuidanceStatus::Active,
            "a historical bundle projects unknown lenses without retroactively enforcing them"
        );
        assert_eq!(first.next.durable_assurance.blockers.len(), 8);
        assert!(first.next.durable_assurance.blockers.iter().all(|blocker| {
            blocker.code == WorkflowDurableAssuranceBlockerCode::UniversalLensUnknown
                && blocker.lens.is_some()
        }));
        let accepted_case_digest = first.next.durable_assurance.case_digest.clone();
        let accepted_projection_digest = accepted.projection_digest.clone();

        let record_count = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("projection")
            .records
            .len();
        let retry = adapter
            .apply_verified_broker_action(verify(first_envelope), now)
            .expect("exact idempotent retry");
        assert_eq!(retry.action_record, first.action_record);
        assert_eq!(retry.origin_record, first.origin_record);
        assert_eq!(
            lock_workflow_governance_ledger_tcb(&state)
                .expect("ledger")
                .recover()
                .expect("projection")
                .records
                .len(),
            record_count,
            "retry must append no ledger records"
        );

        let revision_packet = retry
            .next
            .authorization
            .action_packets
            .iter()
            .find(|packet| packet.authorization_kind == WorkflowAuthorizationKind::IntentRevision)
            .expect("revision packet")
            .clone();
        let stale_envelope = signed_intent_envelope(
            &refreshed.project_id,
            &revision_packet,
            &key,
            now,
            "human-intent-stale-nonce-0002",
            "This stale revision must not commit",
        );
        let replay_count =
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay before stale attempt")
                .entries
                .len();
        fs::write(root.join("README.md"), b"project changed\n").expect("snapshot drift");
        let drifted = adapter.resume().expect("drifted replacement-agent view");
        assert_ne!(
            drifted.durable_assurance.case_digest, accepted_case_digest,
            "current project drift must change the case digest"
        );
        assert_ne!(
            drifted.durable_assurance.current_snapshot_digest,
            first.next.durable_assurance.current_snapshot_digest
        );
        assert_eq!(
            drifted
                .durable_assurance
                .projection
                .as_ref()
                .expect("accepted intent survives project drift")
                .projection_digest,
            accepted_projection_digest,
            "project drift must not rewrite accepted human intent history"
        );
        assert!(matches!(
            adapter.apply_verified_broker_action(verify(stale_envelope), now),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        assert_eq!(
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay after stale attempt")
                .entries
                .len(),
            replay_count,
            "stale packet must write no replay reservation"
        );
        assert_eq!(
            lock_workflow_governance_ledger_tcb(&state)
                .expect("ledger")
                .recover()
                .expect("projection")
                .records
                .len(),
            record_count,
            "stale packet must write no ledger record"
        );

        let current_packets = adapter.action_packets_at(now).expect("current packets");
        let revision_packet = current_packets
            .packets
            .iter()
            .find(|packet| packet.authorization_kind == WorkflowAuthorizationKind::IntentRevision)
            .expect("current revision packet");
        let second_envelope = signed_intent_envelope(
            &current_packets.project_id,
            revision_packet,
            &key,
            now,
            "human-intent-second-nonce-0003",
            "Enable a novice to create, verify, and recover a dependable product",
        );
        let second = adapter
            .apply_verified_broker_action(verify(second_envelope), now)
            .expect("second accepted intent");
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(second_event) =
            &second.action_record.event
        else {
            panic!("second typed intent action");
        };
        assert_eq!(second_event.assurance_epoch, 2);
        assert_eq!(second_event.intent.revision, 2);
        assert_eq!(
            second_event.previous_intent_digest.as_deref(),
            Some(first_event.intent_digest.as_str())
        );
        let durable = second
            .next
            .durable_assurance
            .projection
            .as_ref()
            .expect("revised durable projection");
        assert_eq!(durable.binding.assurance_epoch, 2);
        assert_eq!(durable.binding.intent_revision, 2);
        assert_eq!(durable.intent, second_event.intent);
        let ledger = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("projection");
        assert_eq!(
            ledger
                .records
                .iter()
                .filter(|record| matches!(
                    &record.event,
                    WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
                ))
                .count(),
            2,
            "the prior accepted revision must remain in append-only history"
        );
        let resumed = adapter.resume().expect("replacement-agent resume");
        assert_eq!(
            resumed
                .durable_assurance
                .projection
                .expect("resumed durable projection"),
            (*durable).clone()
        );
    }

    #[test]
    fn unknown_assurance_blocks_phase_even_when_legacy_phase_is_otherwise_done() {
        let (root, state) = temp_project("unknown-assurance-phase-boundary");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.unknown-assurance-boundary".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        accept_test_intent(&adapter);

        let projection = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("projection");
        let assurance = project_durable_assurance(&projection.records)
            .expect("durable projection")
            .expect("accepted intent");
        assert_eq!(assurance.blocker_lenses.len(), 8);
        assert!(assurance
            .lenses
            .iter()
            .all(|lens| lens.claim_status == DurableAssuranceEpistemicState::Unknown));

        assert!(
            !phase_advance_allowed_by_assurance(Some(&assurance), true, true),
            "legacy phase completion cannot outrank eight unknown Assurance lenses"
        );
        assert!(
            phase_advance_allowed_by_assurance(Some(&assurance), true, false),
            "a historical bundle without the Universal Assurance policy must retain its admitted phase semantics"
        );
        assert_eq!(
            projection
                .records
                .iter()
                .filter(|record| {
                    matches!(&record.event, WorkflowGovernanceEvent::PhaseAdvanced(_))
                })
                .count(),
            0,
            "the blocked boundary must contain no PhaseAdvanced authority"
        );
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
        accept_test_intent(&adapter);

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
        accept_test_intent(&adapter);
        fs::remove_file(adapter.trusted_broker_registry_path())
            .expect("remove test intent broker registry");
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
        let release_registry =
            load_admitted_workflow_governance_universal_assurance_release_registry()
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
        let stale_result = adapter.prepare_authorization(
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
            stale_result,
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
        accept_test_intent(&adapter);
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
        assert_eq!(
            lines.len(),
            4,
            "intent and signal each have reserve and commit records"
        );
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
        accept_test_intent(&adapter);
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
        let baseline_replay_entries =
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay before dropped batch")
                .entries
                .len();

        let release_registry =
            load_admitted_workflow_governance_universal_assurance_release_registry()
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
            broker_semantic_input_to_closed(semantic_input).expect("closed broker input"),
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

        assert_eq!(
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay after dropped batch")
                .entries
                .len(),
            baseline_replay_entries,
            "dropped precommit batch must not add a replay tombstone"
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
    fn admitted_broker_evidence_uses_evaluator_freshness_not_envelope_expiry() {
        assert!(evidence_time_is_current(100, Some(400), 1_000, 500, true));
        assert!(
            !evidence_time_is_current(100, Some(400), 1_000, 500, false),
            "a local receipt retains its explicit evidence expiry"
        );
        assert!(
            !evidence_time_is_current(100, Some(400), 399, 500, true),
            "broker admission cannot outrank evaluator max age"
        );
    }

    #[test]
    fn broker_capability_receipt_requires_exact_current_origin_companion() {
        let (root, _) = temp_project("broker-capability-receipt-provenance");
        let snapshot = project_snapshot_digest(&root).expect("snapshot");
        let broker_registry_digest = format!("sha256:{}", "a".repeat(64));
        let prior_head = format!("sha256:{}", "b".repeat(64));
        let action_record_digest = format!("sha256:{}", "c".repeat(64));
        let capability_ref = StableId("capability.broker.runtime".to_owned());
        let issuer_id = StableId("broker.runtime.receipts".to_owned());
        let public_key_fingerprint = format!("sha256:{}", "d".repeat(64));
        let origin_principal = PrincipalId("principal.runtime.receipts".to_owned());
        let separation_domain = StableId("runtime.receipts.session".to_owned());
        let readme_digest = sha256_content_hash(&fs::read(root.join("README.md")).expect("README"));
        let action = WorkflowGovernanceLedgerRecord {
            record_id: StableId("record.broker.capability".to_owned()),
            sequence: 1,
            project_id: StableId("project.receipts".to_owned()),
            bundle_id: StableId("bundle.receipts".to_owned()),
            bundle_digest: format!("sha256:{}", "e".repeat(64)),
            state_version: 3,
            previous_record_digest: Some(prior_head.clone()),
            record_digest: action_record_digest.clone(),
            recorded_at_unix: 10,
            event: WorkflowGovernanceEvent::CapabilityProbed(CapabilityProbedEvent {
                policy_ref: StableId("policy.workflow.receipts".to_owned()),
                capability_ref: capability_ref.clone(),
                probe_kind: WorkflowCapabilityProbeKind::ExternalVerification,
                credential_id: issuer_id.clone(),
                public_key_fingerprint: public_key_fingerprint.clone(),
                authorization_registry_digest: broker_registry_digest.clone(),
                available: true,
                probe_ref: "README.md".to_owned(),
                probe_digest: readme_digest.clone(),
                subject: WorkflowEvidenceSubject {
                    kind: WorkflowEvidenceSubjectKind::Artifact,
                    subject_ref: "README.md".to_owned(),
                    subject_digest: readme_digest,
                },
                snapshot_digest: snapshot.clone(),
                ledger_head_digest: prior_head,
                observed_at_unix: 10,
                expires_at_unix: Some(100),
            }),
        };
        let origin = WorkflowGovernanceLedgerRecord {
            record_id: StableId("record.broker.capability.origin".to_owned()),
            sequence: 2,
            project_id: action.project_id.clone(),
            bundle_id: action.bundle_id.clone(),
            bundle_digest: action.bundle_digest.clone(),
            state_version: action.state_version,
            previous_record_digest: Some(action_record_digest.clone()),
            record_digest: format!("sha256:{}", "f".repeat(64)),
            recorded_at_unix: 11,
            event: WorkflowGovernanceEvent::BrokerOriginApplied(BrokerOriginAppliedEvent {
                action_packet_digest: format!("sha256:{}", "1".repeat(64)),
                broker_event_digest: format!("sha256:{}", "2".repeat(64)),
                action_record_digest,
                origin_principal_id: origin_principal,
                separation_domain: separation_domain.clone(),
                nonce_fingerprint: format!("sha256:{}", "3".repeat(64)),
                issuer_id,
                issuer_profile: WorkflowBrokerOriginProfile::Runtime,
                public_key_fingerprint,
                signature_fingerprint: format!("sha256:{}", "4".repeat(64)),
                enrollment_ceremony_digest: format!("sha256:{}", "5".repeat(64)),
                broker_registry_digest: broker_registry_digest.clone(),
                issued_at_unix: 10,
                expires_at_unix: 120,
            }),
        };
        let projection =
            |records: Vec<WorkflowGovernanceLedgerRecord>| WorkflowGovernanceLedgerProjection {
                next_sequence: u64::try_from(records.len()).expect("record count") + 1,
                next_state_version: 4,
                head_digest: records.last().map(|record| record.record_digest.clone()),
                records,
            };
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()
            .expect("admitted registry");
        let derive = |projection: &WorkflowGovernanceLedgerProjection, current_broker: &str| {
            derive_receipts(
                registry.genesis().document(),
                projection,
                &root,
                &snapshot,
                20,
                None,
                Some(current_broker),
            )
            .expect("derive broker receipts")
        };

        let valid = projection(vec![action.clone(), origin.clone()]);
        let Some(DerivedReceiptTrustRoot::ExternalBroker(provenance)) = receipt_trust_root(
            &valid.records,
            0,
            &valid.records[0],
            &broker_registry_digest,
            None,
            Some(&broker_registry_digest),
        ) else {
            panic!("structured broker provenance");
        };
        assert_eq!(provenance.separation_domain, separation_domain);
        assert_eq!(
            provenance.issuer_profile,
            WorkflowBrokerOriginProfile::Runtime
        );
        assert!(derive(&valid, &broker_registry_digest)
            .available_capability_refs
            .contains(&capability_ref));

        let missing = projection(vec![action.clone()]);
        assert!(!derive(&missing, &broker_registry_digest)
            .available_capability_refs
            .contains(&capability_ref));

        let mut mismatched_origin = origin.clone();
        let WorkflowGovernanceEvent::BrokerOriginApplied(mismatch) = &mut mismatched_origin.event
        else {
            unreachable!();
        };
        mismatch.action_record_digest = format!("sha256:{}", "0".repeat(64));
        let mismatch = projection(vec![action.clone(), mismatched_origin]);
        assert!(!derive(&mismatch, &broker_registry_digest)
            .available_capability_refs
            .contains(&capability_ref));

        let mut wrong_profile_origin = origin.clone();
        let WorkflowGovernanceEvent::BrokerOriginApplied(wrong_profile) =
            &mut wrong_profile_origin.event
        else {
            unreachable!();
        };
        wrong_profile.issuer_profile = WorkflowBrokerOriginProfile::Human;
        let wrong_profile = projection(vec![action.clone(), wrong_profile_origin]);
        assert!(!derive(&wrong_profile, &broker_registry_digest)
            .available_capability_refs
            .contains(&capability_ref));

        let wrong_registry = format!("sha256:{}", "9".repeat(64));
        assert!(!derive(&valid, &wrong_registry)
            .available_capability_refs
            .contains(&capability_ref));
    }

    #[test]
    fn current_legacy_local_evidence_keeps_its_admitted_provider_semantics() {
        let (root, _) = temp_project("legacy-local-evidence-receipt");
        let snapshot = project_snapshot_digest(&root).expect("snapshot");
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()
            .expect("admitted registry");
        let bundle = registry.genesis().document();
        let (policy_ref, claim_ref, evaluator) = bundle
            .workflow_governance_bundle
            .policies
            .iter()
            .find_map(|policy| {
                policy.evaluators.iter().find_map(|evaluator| {
                    (evaluator.provider == WorkflowEvaluatorProvider::RepositoryInspector)
                        .then(|| {
                            policy
                                .claims
                                .iter()
                                .find(|claim| claim.evaluator_ref == evaluator.id)
                                .map(|claim| {
                                    (policy.id.clone(), claim.id.clone(), evaluator.clone())
                                })
                        })
                        .flatten()
                })
            })
            .expect("repository-inspector evaluator with a bound claim");
        let kind = *evaluator
            .accepted_evidence_kinds
            .first()
            .expect("accepted evidence kind");
        let principal_registry_digest = format!("sha256:{}", "6".repeat(64));
        let prior_head = format!("sha256:{}", "7".repeat(64));
        let readme_digest = sha256_content_hash(&fs::read(root.join("README.md")).expect("README"));
        let local = WorkflowGovernanceLedgerRecord {
            record_id: StableId("record.local.repository-inspection".to_owned()),
            sequence: 1,
            project_id: StableId("project.local.receipts".to_owned()),
            bundle_id: StableId("bundle.local.receipts".to_owned()),
            bundle_digest: format!("sha256:{}", "8".repeat(64)),
            state_version: 2,
            previous_record_digest: Some(prior_head.clone()),
            record_digest: format!("sha256:{}", "9".repeat(64)),
            recorded_at_unix: 10,
            event: WorkflowGovernanceEvent::EvaluatorObserved(EvaluatorObservedEvent {
                policy_ref,
                claim_ref,
                evaluator_ref: evaluator.id,
                provider: evaluator.provider,
                credential_id: StableId("credential.local.runtime".to_owned()),
                public_key_fingerprint: format!("sha256:{}", "a".repeat(64)),
                authorization_registry_digest: principal_registry_digest.clone(),
                kind,
                strength: evaluator.minimum_strength,
                outcome: WorkflowEvidenceOutcome::Pass,
                provenance: WorkflowEvidenceProvenance {
                    source_ref: "README.md".to_owned(),
                    source_digest: readme_digest.clone(),
                    scenario_digest: format!("sha256:{}", "b".repeat(64)),
                    semantic_identity: StableId("evidence.local.repository".to_owned()),
                    producer_ref: StableId("agent.local.runtime".to_owned()),
                    principal: Some(PrincipalId("principal.local.runtime".to_owned())),
                    method: "registry_authorized_evidence:test".to_owned(),
                },
                subject: WorkflowEvidenceSubject {
                    kind: WorkflowEvidenceSubjectKind::Artifact,
                    subject_ref: "README.md".to_owned(),
                    subject_digest: readme_digest,
                },
                snapshot_digest: snapshot.clone(),
                ledger_head_digest: prior_head,
                observed_at_unix: 10,
                expires_at_unix: Some(100),
            }),
        };
        let projection =
            |record: WorkflowGovernanceLedgerRecord| WorkflowGovernanceLedgerProjection {
                head_digest: Some(record.record_digest.clone()),
                records: vec![record],
                next_sequence: 2,
                next_state_version: 3,
            };
        let derive = |projection: &WorkflowGovernanceLedgerProjection| {
            derive_receipts(
                bundle,
                projection,
                &root,
                &snapshot,
                20,
                Some(&principal_registry_digest),
                None,
            )
            .expect("derive local receipts")
        };

        assert_eq!(derive(&projection(local.clone())).evidence.len(), 1);

        let mut wrong_provider = local;
        let WorkflowGovernanceEvent::EvaluatorObserved(event) = &mut wrong_provider.event else {
            unreachable!();
        };
        event.provider = WorkflowEvaluatorProvider::ExternalAuthority;
        assert!(derive(&projection(wrong_provider)).evidence.is_empty());
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()
            .expect("admitted registry");
        let admitted = registry.genesis();
        let derived = derive_receipts(
            admitted.document(),
            &projection,
            &root,
            &current_snapshot,
            20,
            Some(&registry_digest),
            None,
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
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()
            .expect("admitted registry");
        let admitted = registry.genesis();
        let derived = derive_receipts(
            admitted.document(),
            &projection,
            &root,
            &current_snapshot,
            20,
            None,
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
