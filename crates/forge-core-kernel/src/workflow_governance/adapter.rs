//! Live Project Snapshot Adapter for the agent-native governance lane.
//!
//! The adapter is deliberately opinionated: the admitted bundle is embedded,
//! the durable ledger owns phase/state/prerequisite authority, repository
//! observations are re-hashed, and callers never choose a workflow or target.

// Opaque authorization and completion capabilities are intentionally consumed
// by value so callers cannot reuse them after a durable transition.
#![allow(clippy::missing_errors_doc, clippy::needless_pass_by_value)]

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
#[cfg(test)]
use forge_core_authority::workflow_origin_broker::WorkflowBrokerIssuerStatus;
use forge_core_authority::{
    AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry, AuthorizedWorkflowBrokerControlPlane,
    AuthorizedWorkflowBrokerRegistry, HistoricallyVerifiedBoundWorkflowBrokerEvent,
    HistoricallyVerifiedWorkflowBrokerEvent, PrincipalCredentialStatus, PrincipalRegistryDocument,
    VerifiedBoundWorkflowBrokerEvent, VerifiedWorkflowApplicabilityAuthorization,
    VerifiedWorkflowBrokerEvent, VerifiedWorkflowBrokerEventAudit,
    VerifiedWorkflowCapabilityAuthorization, VerifiedWorkflowDecisionAuthorization,
    VerifiedWorkflowEvidenceAuthorization, VerifiedWorkflowSignalAuthorization,
    VerifiedWorkflowWaiverAuthorization, WorkflowApplicabilityAuthorizationRequest,
    WorkflowAuthorizationKind, WorkflowBrokerEventKind, WorkflowBrokerIssuerProfile,
    WorkflowBrokerRegistryDocument, WorkflowBrokerSemanticInput,
    WorkflowCapabilityAuthorizationRequest, WorkflowDecisionAuthorizationRequest,
    WorkflowEvidenceAuthorizationRequest, WorkflowSignalAuthorizationRequest,
    WorkflowWaiverAuthorizationRequest, WorkflowWaiverSubject,
};
use forge_core_contracts::completion::CompletionStatus;
use forge_core_contracts::gate::GateStatus;
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::recovery::{HealthStatus, RecoveryAction};
use forge_core_contracts::request::{DependencyKind, RequestStatus};
use forge_core_contracts::workflow_governance::{
    BrokerOriginAppliedEvent, HumanIntentRevisionAcceptedEvent, LegacySoloProfileAdoptedEvent,
    WorkflowBrokerOriginProfile, WorkflowCooperativeAuthorityBasis,
    WorkflowCooperativeObjectiveRevisionKind, WorkflowReadinessProfile,
};
use forge_core_contracts::{
    AgentAutonomyAssessment, AgentAutonomyAssessmentInput, AgentAutonomyBinding,
    AgentAutonomyEffectDescriptor, AgentOwnedWorkClass, ApplicabilityAssessedEvent,
    CapabilityProbedEvent, ClaimContract, ContinuityRecordedEvent,
    CooperativeObjectiveAcceptedEvent, CoordinationCompletionState,
    CoordinationHealthRecoveryState, CoordinationMutationHandoff, CoordinationRequestState,
    CoordinationStateAppliedEvent, CoordinationStateRecord, CoreDomainPackRebasedEvent,
    DecisionAlternative, DecisionRequest, DecisionResolvedEvent, DomainPackCompositionGap,
    DomainPackCoreBinding, DomainPackLifecycleOperation, DomainPackRebasePlanDocument,
    DomainPackRebasePlanInput, DurableAssuranceEpistemicState, DurableAssuranceProjection,
    EvaluatorObservedEvent, HumanDecisionClass, IsolationContract, IsolationStatus, NextAction,
    Phase, PhaseAdvancedEvent, PolicyCompletedEvent, PostBuildVerifyAdmittedGateResult,
    PostBuildVerifyEpisodeAppliedEvent, PostBuildVerifyEpisodeDocument,
    PostBuildVerifyEpisodeOutcome, PostBuildVerifyGateKind, PrincipalId, ProjectImportedEvent,
    ProjectLinkDocument, PromotionGitWorktreeBinding, ProtectedEffect, ReadinessTarget,
    ReleaseUpgradedEvent, SignalChangedEvent, StableId, UniversalAssuranceLens,
    WaiverAuthorizedEvent, WorkflowAdmittedCooperativeEvidence, WorkflowAssuranceClaimRole,
    WorkflowBrokerCredentialStatus, WorkflowBrokerExternalSetupBlockReason,
    WorkflowBrokerExternalSetupState, WorkflowBrokerPublicRegistryDocument,
    WorkflowCapabilityProbeKind, WorkflowClaimWaiverObservation, WorkflowClaimWaiverPolicy,
    WorkflowCompletionAssertion, WorkflowContentAddressedReference,
    WorkflowCooperativeEvidenceAssuranceEffect, WorkflowCooperativeEvidenceBinding,
    WorkflowCooperativeEvidenceCurrentStatus, WorkflowCooperativeEvidenceDisposition,
    WorkflowCooperativeEvidenceNonProof, WorkflowCooperativeEvidenceObservedEvent,
    WorkflowCooperativeEvidenceOffer, WorkflowCooperativeEvidenceProof,
    WorkflowCooperativeEvidenceRejection, WorkflowCooperativeEvidenceRoute,
    WorkflowCooperativeHostProvenance, WorkflowCooperativeMaterialScenarioKind,
    WorkflowCooperativeObjectiveInput, WorkflowCooperativeObjectiveProposal,
    WorkflowEffectiveBundleIdentity, WorkflowEvaluatorProvider, WorkflowEvidenceFreshness,
    WorkflowEvidenceKind, WorkflowEvidenceObservation, WorkflowEvidenceOutcome,
    WorkflowEvidenceProvenance, WorkflowEvidenceStrength, WorkflowEvidenceSubject,
    WorkflowEvidenceSubjectKind, WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluation,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernancePolicy, WorkflowGovernanceReleaseIdentity, WorkflowGovernanceSignal,
    WorkflowHumanIntentRevision, WorkflowPolicyActivation, WorkflowPrerequisiteRequirement,
    WorkflowReceiptCarryover, WorkflowReleaseRegistryProvenance,
    WorkflowRepresentativeSliceDefinitionDocument, WorkflowRuntimeBundleIdentity,
    COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION, COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION,
    MAX_REPRESENTATIVE_SLICE_ITEMS, MAX_REPRESENTATIVE_SLICE_ITEM_BYTES,
    MAX_REPRESENTATIVE_SLICE_TEXT_BYTES, MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES,
    MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES, MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES,
    MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES, MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES,
    MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES, MAX_WORKFLOW_INTENT_ITEM_BYTES,
    MAX_WORKFLOW_INTENT_LIST_ITEMS, MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
    MAX_WORKFLOW_INTENT_TOTAL_BYTES, PROJECT_LINK_FILE_NAME, PROJECT_LINK_SCHEMA_VERSION,
    SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION, SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION, WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_agent_autonomy, evaluate_cooperative_evidence, evaluate_post_build_verify_episode,
    evaluate_transition, find_entry, is_live, load_embedded_frozen_legacy_catalog,
    plan_domain_pack_rebase, project_cooperative_durable_assurance, project_durable_assurance,
    project_governed_durable_assurance, project_legacy_workflow_compatibility, rfc3339_to_unix,
    route_post_build_verify_episode, simulate_workflow_governance,
    validate_representative_slice_definition, verify_domain_pack_rebase_plan,
    workflow_cooperative_objective_digest, workflow_cooperative_revision_input_digest,
    workflow_human_intent_digest, AgentAutonomyEvaluationError, AssuranceProjectionError,
    CooperativeEvidenceDecision, DomainPackRebasePlanError, GateKind,
    GovernedAssuranceActionPacketFact, GovernedAssuranceCapabilityFact,
    GovernedAssuranceDecisionFact, GovernedAssuranceEvidenceFact, GovernedAssuranceFacts,
    GovernedAssuranceWaiverFact, LegacyWorkflowGovernanceProjection,
    PostBuildVerifyEpisodeRuntimeRoute, ProvidedGateResult, TransitionDecision, TransitionRequest,
    WorkflowClaimResultStatus, WorkflowGovernanceRejection, WorkflowGovernanceSimulation,
    WorkflowGovernanceStatus,
};
use forge_core_domain_pack_tcb::{
    lock_domain_pack_lifecycle_for_project, observe_domain_pack_lifecycle_for_project,
    observe_existing_domain_pack_lifecycle_for_project, AdmittedActiveDomainPackGeneration,
    DomainPackLifecycleStoreError, LockedCoreOnlyDomainPackLifecycleObservation,
    LockedDomainPackLifecycleObservation,
};
use forge_core_store::claim_wal::{
    claim_wal_lock_path, claim_wal_path, project_claim_wal, project_existing_claim_wal,
    retain_existing_claim_wal_projection, ClaimWalProjection, ClaimWalProjectionOptions,
    ClaimWalProjectionStopPolicy,
};
use forge_core_store::retained_crash_replace::observe_file_crash_safe_under_owned_lock;
use forge_core_store::retained_project_tree::{RetainedProjectTree, RetainedProjectTreeError};
use forge_core_store::workflow_action_replay::{
    begin_workflow_action_replay_reservation, initialize_workflow_action_replay,
    workflow_action_replay_origin_fingerprint, WorkflowActionReplayError,
};
use forge_core_store::{
    acquire_effect_store_lock, sha256_content_hash, ReferenceIndexBuilder, RetainedEffectStoreRoot,
};
use forge_core_validate::{
    validate_completion, validate_health_recovery, validate_request, ReferenceIndex, ReferenceKind,
};
use forge_core_workflow_governance_tcb::{
    domain_pack_receipt_carryover, lock_workflow_governance_ledger_tcb,
    observe_existing_workflow_governance_ledger, LockedWorkflowGovernanceLedger,
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
const MAX_SNAPSHOT_ENTRIES: usize = MAX_SNAPSHOT_FILES * 2;
const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const TRUSTED_WORKFLOW_REGISTRY_RELATIVE_PATH: &str = "operator/workflow-principal-registry.yaml";
const TRUSTED_WORKFLOW_BROKER_REGISTRY_RELATIVE_PATH: &str =
    "operator/workflow-broker-registry.yaml";
const MAX_TRUSTED_REGISTRY_BYTES: u64 = 1024 * 1024;
const WORKFLOW_AUTHORIZATION_ACTION_PACKET_SCHEMA_VERSION: &str =
    "workflow_authorization_action_packets_v1";
const WORKFLOW_AUTHORIZATION_PREPARATION_TTL_SECONDS: u64 = 300;
const MAX_WORKFLOW_COOPERATIVE_DECISION_QUESTION_BYTES: usize = 512;
const MIN_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES: usize = 2;
const MAX_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES: usize = 8;
const MAX_WORKFLOW_COOPERATIVE_DECISION_CONSEQUENCES: usize = 8;
const UNIVERSAL_ASSURANCE_POLICY_ID: &str = "policy.workflow.universal-assurance";
const DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH: &str = "domain-packs/rebase-plan.yaml";
const DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH: &str = "locks/domain-packs.rebase-plan.lock";
#[cfg(test)]
const TEST_REPLAY_APPEND_FAILURE_MARKER: &str = ".test-fail-replay-append-after-ledger";
#[cfg(test)]
const TEST_REPLAY_APPEND_FAILURE_BACKUP: &str = ".test-replay-wal-before-append-failure";
#[cfg(test)]
const TEST_EXPIRE_AFTER_REPLAY_RESERVATION_MARKER: &str = ".test-expire-after-replay-reservation";
#[cfg(all(test, unix))]
const TEST_REPLACE_PROJECT_FILE_AFTER_REPLAY_RESERVATION_MARKER: &str =
    ".test-replace-project-file-after-replay-reservation";
#[cfg(test)]
const TEST_CHANGE_PROJECT_BEFORE_COOPERATIVE_COMMIT_MARKER: &str =
    ".test-change-project-before-cooperative-commit";
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
    CooperativeSameOwner,
    HumanApprovalBroker,
    IndependentReviewerBroker,
    TrustedRuntimeBroker,
    ExternalAuthorityBroker,
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

/// One machine-readable example for a closed cooperative-objective input
/// variant. Templates contain placeholder values, not authority, and are
/// emitted inside the packet so a generic host need not inspect Forge source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeObjectiveInputTemplate {
    pub variant: String,
    pub template: serde_json::Value,
}

/// Complete host-facing bounds for the cooperative-objective JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeObjectiveInputLimits {
    pub input_max_bytes: u64,
    pub objective_id_max_bytes: usize,
    pub carrying_principal_max_bytes: usize,
    pub host_coordinate_max_bytes: usize,
    pub revision_reason_max_bytes: usize,
    pub outcome_max_bytes: usize,
    pub list_max_items: usize,
    pub list_item_max_bytes: usize,
    pub proposal_total_max_bytes: usize,
    pub decision_question_max_bytes: usize,
    pub decision_alternatives_min_items: usize,
    pub decision_alternatives_max_items: usize,
    pub decision_consequences_max_items: usize,
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
    CooperativeObjective {
        objective_id: StableId,
        next_objective_revision: u64,
        next_assurance_epoch: u64,
        input_encoding: String,
        discriminator_field: String,
        unknown_fields_allowed: bool,
        variants: Vec<WorkflowCooperativeObjectiveInputTemplate>,
        limits: WorkflowCooperativeObjectiveInputLimits,
        command_argv_template: Vec<String>,
    },
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_management_packet: Option<WorkflowAuthorizationActionPacket>,
}

/// Setup discovery only. `Ready` proves that a bounded, valid canonical
/// document with an active entry was found; it does not prove that Forge
/// observed enrollment/user presence or that the broker is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationRegistrySetupStatus {
    Missing,
    /// A frozen legacy registry remains admissible only for exact replay repair;
    /// it cannot authorize a new workflow mutation.
    LegacyRecoveryOnly,
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
/// next`. An argv is emitted only when the external selected-host dependency is
/// available and the command can round-trip through the strict broker parser.
/// A blocked external setup emits no executable suggestion; Forge never asks an
/// agent to fabricate a private key, trust anchor, or native authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuthorizationSetupGap {
    pub code: WorkflowAuthorizationSetupGapCode,
    pub summary: String,
    pub accepted_profiles: Vec<WorkflowBrokerIssuerProfile>,
    pub external_setup: WorkflowBrokerExternalSetupState,
    pub setup_argv: Vec<String>,
    pub required_operator_inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationSetupGapCode {
    BrokerRegistryMissing,
    BrokerRegistryLegacyRecoveryOnly,
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
    /// Out-of-band objective history management. It does not replace or rank
    /// ahead of the governed policy action packets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_management_packet: Option<WorkflowAuthorizationActionPacket>,
}

/// Host-neutral, read-only autonomy boundary projected alongside the governed
/// next action. It never grants workflow mutation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAgentAutonomyGuidance {
    pub status: WorkflowAgentAutonomyGuidanceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<AgentAutonomyBinding>,
    pub delegated_work_classes: Vec<AgentOwnedWorkClass>,
    pub human_decision_classes: Vec<HumanDecisionClass>,
    pub protected_effects: Vec<ProtectedEffect>,
    /// Structured argv for execution. Display strings are not an execution contract.
    pub assessment_argv: Vec<String>,
    pub input_contract: WorkflowAgentAutonomyInputContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAgentAutonomyInputContract {
    pub schema_version: &'static str,
    pub max_input_file_bytes: u64,
    pub max_summary_bytes: usize,
    pub unknown_fields_allowed: bool,
    pub temporary_input_must_be_outside_project_snapshot: bool,
    pub effect_descriptor_source: &'static str,
    pub agent_owned_work_classes: Vec<AgentOwnedWorkClass>,
    pub human_decision_classes: Vec<HumanDecisionClass>,
    pub protected_effects: Vec<ProtectedEffect>,
    pub effect_descriptors: Vec<AgentAutonomyEffectDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAgentAutonomyGuidanceStatus {
    ObjectiveRequired,
    Active,
    UnsupportedProfile,
}

fn workflow_agent_autonomy_guidance(
    readiness_profile: WorkflowReadinessProfile,
    objective: Option<&WorkflowActiveCooperativeObjective>,
    snapshot_digest: &str,
    ledger_head_digest: &str,
    state_version: u64,
) -> WorkflowAgentAutonomyGuidance {
    let active =
        readiness_profile == WorkflowReadinessProfile::SoloCooperative && objective.is_some();
    WorkflowAgentAutonomyGuidance {
        status: match readiness_profile {
            WorkflowReadinessProfile::StrictExternal => {
                WorkflowAgentAutonomyGuidanceStatus::UnsupportedProfile
            }
            WorkflowReadinessProfile::SoloCooperative if objective.is_some() => {
                WorkflowAgentAutonomyGuidanceStatus::Active
            }
            WorkflowReadinessProfile::SoloCooperative => {
                WorkflowAgentAutonomyGuidanceStatus::ObjectiveRequired
            }
        },
        binding: active.then(|| {
            let objective = objective.expect("active cooperative objective");
            AgentAutonomyBinding {
                objective_id: objective.objective_id.clone(),
                objective_revision: objective.revision,
                objective_digest: objective.objective_digest.clone(),
                assurance_epoch: objective.assurance_epoch,
                snapshot_digest: snapshot_digest.to_owned(),
                ledger_head_digest: ledger_head_digest.to_owned(),
                state_version,
            }
        }),
        delegated_work_classes: AgentOwnedWorkClass::ALL.to_vec(),
        human_decision_classes: HumanDecisionClass::ALL.to_vec(),
        protected_effects: ProtectedEffect::ALL.to_vec(),
        assessment_argv: vec![
            "forge-core".to_owned(),
            "workflow".to_owned(),
            "autonomy".to_owned(),
            "assess".to_owned(),
            "--root".to_owned(),
            "<project-root>".to_owned(),
            "--input-file".to_owned(),
            "<temporary-file-outside-project-snapshot>".to_owned(),
            "--json".to_owned(),
        ],
        input_contract: WorkflowAgentAutonomyInputContract {
            schema_version: forge_core_contracts::AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION,
            max_input_file_bytes: forge_core_contracts::MAX_AGENT_AUTONOMY_INPUT_BYTES,
            max_summary_bytes: forge_core_contracts::MAX_AGENT_AUTONOMY_SUMMARY_BYTES,
            unknown_fields_allowed: false,
            temporary_input_must_be_outside_project_snapshot: true,
            effect_descriptor_source: "derive from the host tool and concrete operation boundary, never from free-form task text alone",
            agent_owned_work_classes: AgentOwnedWorkClass::ALL.to_vec(),
            human_decision_classes: HumanDecisionClass::ALL.to_vec(),
            protected_effects: ProtectedEffect::ALL.to_vec(),
            effect_descriptors: AgentAutonomyEffectDescriptor::ALL.to_vec(),
        },
    }
}

/// Origin-aware durable objective projection reconstructed from the workflow
/// ledger. `status` distinguishes strict human intent from same-owner
/// objectives, while `active_cooperative_objective.authority_basis` preserves
/// the weaker cooperative origin. Proposal-only Assurance Case files and host
/// readiness claims are never consulted.
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
    MissingObjective,
    ObjectiveAccepted,
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
    MissingAcceptedObjective,
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

/// Exact read-only project observation retained across one governance operation.
///
/// The workflow digest preserves the persisted v0 regular-file projection while
/// the Store capability also retains and revalidates the complete directory and
/// ancestor namespace. The capability is never serialized or returned to callers.
pub(super) struct RetainedWorkflowProjectSnapshot {
    tree: RetainedProjectTree,
}

impl RetainedWorkflowProjectSnapshot {
    fn capture(root: &Path) -> Result<Self, WorkflowGovernanceAdapterError> {
        Self::capture_with_limits(root, MAX_SNAPSHOT_FILES, MAX_SNAPSHOT_BYTES)
    }

    fn capture_with_limits(
        root: &Path,
        maximum_files: usize,
        maximum_bytes: u64,
    ) -> Result<Self, WorkflowGovernanceAdapterError> {
        let maximum_entries = maximum_files.saturating_mul(2).min(MAX_SNAPSHOT_ENTRIES);
        let tree = RetainedProjectTree::capture_allowing_stable_file_aliases(
            root,
            maximum_entries,
            maximum_files,
            maximum_bytes,
        )?;
        Ok(Self { tree })
    }

    fn digest(&self) -> &str {
        self.tree.regular_file_snapshot_digest()
    }

    fn revalidate(&self) -> Result<(), WorkflowGovernanceAdapterError> {
        self.tree.revalidate()?;
        Ok(())
    }

    pub(super) const fn tree(&self) -> &RetainedProjectTree {
        &self.tree
    }

    pub(super) const fn tree_mut(&mut self) -> &mut RetainedProjectTree {
        &mut self.tree
    }
}

/// Exact compare-and-swap bindings required to consume one candidate-only C5.1
/// episode. The document remains data; only the successful kernel operation can
/// produce the durable applied record.
pub struct PostBuildVerifyEpisodeApplyRequest<'a> {
    pub document: &'a PostBuildVerifyEpisodeDocument,
    pub expected_snapshot_digest: &'a str,
    pub expected_ledger_head_digest: &'a str,
    pub expected_state_version: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisodeApplyReceipt {
    pub outcome: PostBuildVerifyEpisodeOutcome,
    pub record: WorkflowGovernanceLedgerRecord,
}

/// Exact compare-and-swap request for one kernel-validated coordination update.
/// The serialized contracts remain data and cannot invoke this operation.
pub struct CoordinationStateApplyRequest<'a> {
    pub state: &'a CoordinationStateRecord,
    pub expected_ledger_head_digest: &'a str,
    pub expected_state_version: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinationStateApplyReceipt {
    pub record: WorkflowGovernanceLedgerRecord,
    pub appended: bool,
}

/// Complete C5.3 episode recovered from one `0.8` ledger record. This is a
/// read-only projection and does not recreate the consumed phase admission.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementEpisodeProjection {
    pub document: PostBuildVerifyEpisodeDocument,
    pub outcome: PostBuildVerifyEpisodeOutcome,
    pub from_phase: StableId,
    pub to_phase: Option<StableId>,
    pub decision_digest: String,
    pub ledger_record_digest: String,
    pub state_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementClaimLiveness {
    Live,
    Expired,
    NonActive,
}

/// Authority-free claim snapshot joined while the claim-WAL retained recovery
/// lock is held. Returning this value releases that lock and carries no claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementClaimProjection {
    pub claim: ClaimContract,
    pub last_sequence: u64,
    pub liveness: ReplacementClaimLiveness,
}

/// Exact fresh-process reconstruction across the workflow ledger and claim WAL.
/// Every member is audit/recovery data only; no opaque kernel capability is
/// serialized or returned.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementContinuityProjection {
    pub ledger_head_digest: String,
    pub state_version: u64,
    pub current_phase: StableId,
    pub active_release: WorkflowGovernanceReleaseIdentity,
    pub active_episode_id: StableId,
    pub episodes_by_id: BTreeMap<String, ReplacementEpisodeProjection>,
    pub requests_by_id: BTreeMap<String, CoordinationRequestState>,
    pub completions_by_task_id: BTreeMap<String, CoordinationCompletionState>,
    pub health_recovery_by_runtime_id: BTreeMap<String, CoordinationHealthRecoveryState>,
    pub claims_by_id: BTreeMap<String, ReplacementClaimProjection>,
}

/// Retains the Domain Pack lifecycle lock until the complete workflow
/// transaction ends. This enforces the global lifecycle -> workflow-ledger
/// lock order even for projects that currently have no active generation.
enum LockedWorkflowDomainPackContext {
    CoreOnly(Box<LockedCoreOnlyDomainPackLifecycleObservation>),
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
    operator_source_binding_digest: String,
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
    fn acquire(
        project_root: &Path,
        state_root: &Path,
    ) -> Result<Self, WorkflowGovernanceAdapterError> {
        match observe_domain_pack_lifecycle_for_project(project_root, state_root)? {
            LockedDomainPackLifecycleObservation::CoreOnly(lifecycle) => {
                debug_assert!(lifecycle.projection().active_pointer.is_none());
                Ok(Self::CoreOnly(Box::new(lifecycle)))
            }
            LockedDomainPackLifecycleObservation::Active(lifecycle) => {
                Ok(Self::Active(Box::new(lifecycle.admit_active_generation()?)))
            }
        }
    }

    fn acquire_existing(
        project_root: &Path,
        state_root: &Path,
    ) -> Result<Self, WorkflowGovernanceAdapterError> {
        match observe_existing_domain_pack_lifecycle_for_project(project_root, state_root)? {
            LockedDomainPackLifecycleObservation::CoreOnly(lifecycle) => {
                debug_assert!(lifecycle.projection().active_pointer.is_none());
                Ok(Self::CoreOnly(Box::new(lifecycle)))
            }
            LockedDomainPackLifecycleObservation::Active(lifecycle) => {
                Ok(Self::Active(Box::new(lifecycle.admit_active_generation()?)))
            }
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
            operator_source_binding_digest: view.operator_source_binding_digest().to_owned(),
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

    /// Admit and atomically record one exact C5.2 route from a candidate-only
    /// post-BuildVerify episode.
    ///
    /// Forward advancement requires the complete current phase policy boundary,
    /// governed assurance, and the hard transition gate to pass. Rollback and
    /// evolution-triage outcomes record a typed durable episode without changing
    /// phase. The caller cannot construct or serialize admission authority.
    pub fn apply_post_build_verify_episode(
        &self,
        request: PostBuildVerifyEpisodeApplyRequest<'_>,
    ) -> Result<PostBuildVerifyEpisodeApplyReceipt, WorkflowGovernanceAdapterError> {
        self.recover_pending_release_rebase()?;
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;

        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let state_version = projection.current_state_version().unwrap_or_default();
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot = project_snapshot.digest().to_owned();
        if request.expected_ledger_head_digest != head {
            return Err(
                WorkflowGovernanceAdapterError::PostBuildVerifyEpisodeBindingMismatch(
                    "ledger head",
                ),
            );
        }
        if request.expected_state_version != state_version {
            return Err(
                WorkflowGovernanceAdapterError::PostBuildVerifyEpisodeBindingMismatch(
                    "state version",
                ),
            );
        }
        if request.expected_snapshot_digest != snapshot {
            return Err(
                WorkflowGovernanceAdapterError::PostBuildVerifyEpisodeBindingMismatch(
                    "project snapshot",
                ),
            );
        }

        let episode = &request.document.post_build_verify_episode;
        if episode.release_subject != *admitted.release()
            || episode.build_verify_snapshot.subject_digest != snapshot
        {
            return Err(
                WorkflowGovernanceAdapterError::PostBuildVerifyEpisodeBindingMismatch(
                    "release subject or BuildVerify snapshot",
                ),
            );
        }
        let current = current_phase(&projection)?;
        let current_phase_value = Phase::parse(&current.0)
            .ok_or_else(|| WorkflowGovernanceAdapterError::InvalidPhase(current.0.clone()))?;
        let route = route_post_build_verify_episode(request.document, current_phase_value)
            .map_err(|_| WorkflowGovernanceAdapterError::PostBuildVerifyEpisodeRouteInvalid)?;
        let decision = evaluate_post_build_verify_episode(request.document);

        let (outcome, to_phase, admitted_gate) = match route {
            PostBuildVerifyEpisodeRuntimeRoute::AdvanceToReadyOperate => {
                self.require_post_build_verify_gate(
                    &effective,
                    &projection,
                    now,
                    current_phase_value,
                    Phase::ReadyOperate,
                    &snapshot,
                    GateKind::Readiness,
                )?;
                (
                    PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate,
                    Some(StableId(Phase::ReadyOperate.to_string())),
                    Some(PostBuildVerifyAdmittedGateResult {
                        kind: PostBuildVerifyGateKind::Readiness,
                        status: GateStatus::Pass,
                        effective_bundle_digest: effective
                            .identity()
                            .effective_runtime_bundle
                            .bundle_digest
                            .clone(),
                    }),
                )
            }
            PostBuildVerifyEpisodeRuntimeRoute::AdvanceToEvolve => {
                self.require_post_build_verify_gate(
                    &effective,
                    &projection,
                    now,
                    current_phase_value,
                    Phase::Evolve,
                    &snapshot,
                    GateKind::Release,
                )?;
                (
                    PostBuildVerifyEpisodeOutcome::AdvancedToEvolve,
                    Some(StableId(Phase::Evolve.to_string())),
                    Some(PostBuildVerifyAdmittedGateResult {
                        kind: PostBuildVerifyGateKind::Release,
                        status: GateStatus::Pass,
                        effective_bundle_digest: effective
                            .identity()
                            .effective_runtime_bundle
                            .bundle_digest
                            .clone(),
                    }),
                )
            }
            PostBuildVerifyEpisodeRuntimeRoute::OpenRollbackAssessment => (
                PostBuildVerifyEpisodeOutcome::RollbackAssessmentOpened,
                None,
                None,
            ),
            PostBuildVerifyEpisodeRuntimeRoute::OpenEvolutionTriage => (
                PostBuildVerifyEpisodeOutcome::EvolutionTriageOpened,
                None,
                None,
            ),
        };
        let next_state_version = state_version
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let event = PostBuildVerifyEpisodeAppliedEvent {
            episode_id: episode.episode_id.clone(),
            generation: episode.generation,
            previous_episode_digest: episode.previous_episode_digest.clone(),
            episode_digest: episode.episode_digest.clone(),
            release_subject: episode.release_subject.clone(),
            decision_digest: decision.decision_digest,
            from_phase: current,
            to_phase,
            outcome,
            snapshot_digest: snapshot,
            prior_ledger_head_digest: head.to_owned(),
            prior_state_version: state_version,
            admitted_gate,
            episode_snapshot: Some(request.document.clone()),
        };
        let identity = self.identity(admitted);
        project_snapshot.revalidate()?;
        let record = ledger.apply_post_build_verify_episode_unchecked_tcb(
            head,
            &identity,
            next_state_version,
            event,
        )?;
        Ok(PostBuildVerifyEpisodeApplyReceipt { outcome, record })
    }

    /// Validate and atomically append one Request, Completion, or `HealthRecovery`
    /// projection. Contract bytes never carry the retained claim/ledger locks or
    /// any mutation, claim, phase, release, signing, trust, or lifecycle authority.
    pub fn apply_coordination_state(
        &self,
        request: CoordinationStateApplyRequest<'_>,
    ) -> Result<CoordinationStateApplyReceipt, WorkflowGovernanceAdapterError> {
        self.recover_pending_release_rebase()?;
        let now = i64::try_from(unix_time()?)
            .map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?;
        let reference_index = coordination_reference_index(&self.binding.project_root)?;
        let claim_projection = project_claim_wal_clean(&self.binding.state_root)?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;

        if let Some(record) = exact_coordination_retry(
            &projection,
            request.state,
            request.expected_ledger_head_digest,
            request.expected_state_version,
        ) {
            return Ok(CoordinationStateApplyReceipt {
                record: record.clone(),
                appended: false,
            });
        }

        let head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let state_version = projection.current_state_version().unwrap_or_default();
        if request.expected_ledger_head_digest != head
            || request.expected_state_version != state_version
        {
            return Err(WorkflowGovernanceAdapterError::CoordinationCasMismatch);
        }
        validate_coordination_kernel_state(
            request.state,
            &projection,
            &claim_projection,
            &reference_index,
            state_version,
            now,
        )?;

        let identity = projection
            .active_identity()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let next_state_version = state_version
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
        let event = CoordinationStateAppliedEvent {
            prior_ledger_head_digest: head.to_owned(),
            prior_state_version: state_version,
            state: request.state.clone(),
        };
        let record = ledger.apply_coordination_state_unchecked_tcb(
            head,
            &identity,
            next_state_version,
            event,
        )?;
        Ok(CoordinationStateApplyReceipt {
            record,
            appended: true,
        })
    }

    /// Reconstruct the exact replacement-agent state from fresh durable reads.
    /// Historical `0.7` episode summaries fail closed because they cannot recover
    /// the rollback baseline, observations, intake, evolution identity, or next action.
    pub fn recover_replacement_continuity(
        &self,
    ) -> Result<ReplacementContinuityProjection, WorkflowGovernanceAdapterError> {
        self.recover_pending_release_rebase()?;
        let now = i64::try_from(unix_time()?)
            .map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?;
        let claim_projection = project_claim_wal_clean(&self.binding.state_root)?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        project_replacement_continuity(&projection, &claim_projection, now)
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
        self.initialize_with_readiness_profile(None)
    }

    /// Initialize with an explicit readiness profile. An absent selector
    /// defaults only a pristine ledger to the cooperative solo posture; an
    /// existing ledger always retains its durable genesis profile.
    pub fn initialize_with_readiness_profile(
        &self,
        requested_profile: Option<WorkflowReadinessProfile>,
    ) -> Result<WorkflowGovernanceInitialization, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let genesis = registry.genesis();
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
        initialize_workflow_action_replay(&self.binding.state_root)?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        if !projection.records.is_empty() {
            let readiness_profile = projection
                .readiness_profile()
                .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
            if requested_profile.is_some_and(|requested| requested != readiness_profile) {
                return Err(
                    WorkflowGovernanceAdapterError::ReadinessProfileReconfiguration {
                        current: readiness_profile,
                        requested: requested_profile.unwrap_or(readiness_profile),
                    },
                );
            }
            let admitted = self.resolve_active_release(&registry, &projection)?;
            let effective = domain.admit_effective(admitted)?;
            projection =
                self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
            let active_identity = self.identity(admitted);
            validate_identity(&projection, &active_identity, &self.binding.project_root)?;
            return Ok(WorkflowGovernanceInitialization {
                status: WorkflowGovernanceInitializationStatus::AlreadyInitialized,
                readiness_profile,
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
        let readiness_profile =
            requested_profile.unwrap_or(WorkflowReadinessProfile::SoloCooperative);
        let event = WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
            source_ref: self.binding.project_root.display().to_string(),
            source_digest: snapshot_digest.clone(),
            snapshot_digest: snapshot_digest.clone(),
            initial_phase: StableId(INITIAL_PHASE.to_owned()),
            readiness_profile: Some(readiness_profile),
        });
        let identity = self.identity(genesis);
        project_snapshot.revalidate()?;
        let record = ledger.initialize_unchecked_tcb(&identity, 0, event)?;
        projection = ledger.recover()?;
        let effective = domain.admit_effective(genesis)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, genesis, &effective, projection)?;
        Ok(WorkflowGovernanceInitialization {
            status: WorkflowGovernanceInitializationStatus::Initialized,
            readiness_profile,
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

    /// Inspect whether a historical profile-less ledger may explicitly adopt
    /// Solo Cooperative without changing any state.
    pub fn profile_status(
        &self,
    ) -> Result<WorkflowLegacyProfileStatus, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
        let ledger = observe_existing_workflow_governance_ledger(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        validate_identity(
            &projection,
            &self.identity(admitted),
            &self.binding.project_root,
        )?;
        project_snapshot.revalidate()?;
        profile_status_projection(&self.binding.project_root, &projection, snapshot_digest)
    }

    /// Append the single explicit transition from legacy profile-less strict
    /// compatibility to Solo Cooperative under exact head and snapshot CAS.
    pub fn adopt_legacy_solo_profile(
        &self,
        expected_head_digest: &str,
        expected_snapshot_digest: &str,
    ) -> Result<WorkflowLegacySoloAdoptionReceipt, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let current_snapshot_digest = project_snapshot.digest().to_owned();
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;

        if let Some(record) = projection.records.iter().find(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)
            )
        }) {
            let WorkflowGovernanceEvent::LegacySoloProfileAdopted(event) = &record.event else {
                unreachable!("filtered event kind")
            };
            let adoption_is_current_head = projection
                .records
                .last()
                .is_some_and(|current| current.record_digest == record.record_digest);
            if event.prior_ledger_head_digest != expected_head_digest
                || event.snapshot_digest != expected_snapshot_digest
                || !adoption_is_current_head
            {
                return Err(WorkflowGovernanceAdapterError::LegacySoloAdoptionRetryConflict);
            }
            project_snapshot.revalidate()?;
            return Ok(WorkflowLegacySoloAdoptionReceipt {
                status: WorkflowLegacySoloAdoptionReceiptStatus::AlreadyAdopted,
                readiness_profile: WorkflowReadinessProfile::SoloCooperative,
                legacy_profileless_genesis: true,
                provenance: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
                snapshot_digest: event.snapshot_digest.clone(),
                ledger_head_digest: projection
                    .head_digest
                    .clone()
                    .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?,
                state_version: projection.current_state_version().unwrap_or_default(),
                transition_record: Some(record.clone()),
            });
        }

        if current_snapshot_digest != expected_snapshot_digest
            || projection.head_digest.as_deref() != Some(expected_head_digest)
        {
            return Err(WorkflowGovernanceAdapterError::LegacySoloAdoptionCasMismatch);
        }
        let (availability, reason) = legacy_solo_adoption_availability(&projection)?;
        if availability != WorkflowLegacySoloAdoptionAvailability::Eligible {
            return Err(WorkflowGovernanceAdapterError::LegacySoloAdoptionUnavailable(reason));
        }
        let genesis = projection
            .records
            .first()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let event = LegacySoloProfileAdoptedEvent {
            legacy_project_import_record_digest: genesis.record_digest.clone(),
            prior_ledger_head_digest: expected_head_digest.to_owned(),
            snapshot_digest: current_snapshot_digest.clone(),
            authority_basis: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
        };
        project_snapshot.revalidate()?;
        let record = ledger.adopt_legacy_solo_unchecked_tcb(
            expected_head_digest,
            &identity,
            projection.next_state_version,
            event,
        )?;
        Ok(WorkflowLegacySoloAdoptionReceipt {
            status: WorkflowLegacySoloAdoptionReceiptStatus::Adopted,
            readiness_profile: WorkflowReadinessProfile::SoloCooperative,
            legacy_profileless_genesis: true,
            provenance: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
            snapshot_digest: current_snapshot_digest,
            ledger_head_digest: record.record_digest.clone(),
            state_version: record.state_version,
            transition_record: Some(record),
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        self.guidance_from_projection(&registry, admitted, &effective, &projection, now)
    }

    /// Assess one host-neutral work description against the exact active
    /// objective and current project/ledger CAS coordinates. This path is
    /// read-only: it does not reconcile epochs, append events, or initialize
    /// replay/state files.
    pub fn assess_agent_autonomy(
        &self,
        input: AgentAutonomyAssessmentInput,
    ) -> Result<AgentAutonomyAssessment, WorkflowGovernanceAdapterError> {
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
            return Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired);
        }
        let objective = active_cooperative_objective_from_ledger(&projection.records)?
            .ok_or(WorkflowGovernanceAdapterError::AgentAutonomyObjectiveRequired)?;
        let current_binding = AgentAutonomyBinding {
            objective_id: objective.objective_id,
            objective_revision: objective.revision,
            objective_digest: objective.objective_digest,
            assurance_epoch: objective.assurance_epoch,
            snapshot_digest: snapshot.digest().to_owned(),
            ledger_head_digest: projection
                .head_digest
                .clone()
                .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?,
            state_version: projection.current_state_version().unwrap_or_default(),
        };
        let assessment = evaluate_agent_autonomy(&current_binding, &input)?;
        snapshot.revalidate()?;
        Ok(assessment)
    }

    /// Build a caller-carried, read-only preview for one exact active isolation.
    ///
    /// This path performs no reconciliation, ledger append, replay reservation,
    /// or project write. Every governance and filesystem coordinate is retained
    /// until the preview has been derived and revalidated.
    pub fn preview_promotion(
        &self,
        isolation_id: &StableId,
    ) -> Result<forge_core_contracts::GovernedPromotionPreview, WorkflowGovernanceAdapterError>
    {
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire_existing(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let ledger = observe_existing_workflow_governance_ledger(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        let destination = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &destination,
        )?;
        let preview = super::promotion::preview_governed_promotion(
            &self.binding,
            isolation_id,
            &guidance,
            destination.tree(),
            now,
        )?;
        destination.revalidate()?;
        Ok(preview)
    }

    /// Apply one exact reviewable local-reversible promotion preview.
    ///
    /// The preview digest is only a compare-and-swap identity. Domain, ledger,
    /// live claim/principal, evidence, source, destination, effect, and payload
    /// are re-derived under retained locks. No broker or strict-external
    /// authorization is required for this solo-cooperative local lane.
    pub fn apply_promotion(
        &self,
        isolation_id: &StableId,
        expected_preview_digest: &str,
    ) -> Result<forge_core_contracts::GovernedPromotionApplication, WorkflowGovernanceAdapterError>
    {
        super::promotion::validate_expected_preview_digest(expected_preview_digest)?;
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire_existing(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let ledger = observe_existing_workflow_governance_ledger(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        let claim_guard = retain_existing_claim_wal_projection(
            &self.binding.state_root,
            &ClaimWalProjectionOptions {
                repair: false,
                stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
            },
        )
        .map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        let effect_lock = acquire_effect_store_lock(
            &self.binding.state_root,
            super::promotion::PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        )
        .map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        if let Some(committed) = super::promotion::inspect_promotion_retry_under_lock(
            &self.binding,
            &effect_lock,
            isolation_id,
            expected_preview_digest,
            false,
        )? {
            return Ok(committed);
        }
        let mut destination = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &destination,
        )?;
        let prepared = super::promotion::prepare_governed_promotion_with_claim_projection(
            &self.binding,
            isolation_id,
            &guidance,
            destination.tree(),
            now,
            claim_guard.projection(),
        )?;
        claim_guard.revalidate().map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        destination.revalidate()?;
        super::promotion::apply_prepared_promotion_under_lock(
            &self.binding,
            expected_preview_digest,
            prepared,
            destination.tree_mut(),
            &effect_lock,
            &claim_guard,
        )
        .map_err(WorkflowGovernanceAdapterError::PromotionApply)
    }

    /// Reconcile one interrupted governed promotion without asking the caller
    /// to generate a replacement preview.
    ///
    /// Durable intent/WAL authority and exact source/destination bytes select
    /// the only admissible continuation. Ambiguous, corrupt, rolled-back, or
    /// mismatched state fails closed before any additional canonical write.
    pub fn recover_promotion(
        &self,
        isolation_id: &StableId,
        expected_preview_digest: &str,
    ) -> Result<forge_core_contracts::GovernedPromotionApplication, WorkflowGovernanceAdapterError>
    {
        super::promotion::validate_expected_preview_digest(expected_preview_digest)?;
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire_existing(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let ledger = observe_existing_workflow_governance_ledger(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        let claim_guard = retain_existing_claim_wal_projection(
            &self.binding.state_root,
            &ClaimWalProjectionOptions {
                repair: false,
                stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
            },
        )
        .map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        let effect_lock = acquire_effect_store_lock(
            &self.binding.state_root,
            super::promotion::PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        )
        .map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        if let Some(committed) = super::promotion::inspect_promotion_retry_under_lock(
            &self.binding,
            &effect_lock,
            isolation_id,
            expected_preview_digest,
            true,
        )? {
            return Ok(committed);
        }
        let mut destination = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let stored_preview = super::promotion::recovery_preview_under_lock(
            &self.binding,
            &effect_lock,
            isolation_id,
            expected_preview_digest,
        )?;
        let fallback_prepared = if stored_preview.is_none() {
            let guidance = self.guidance_from_projection_with_snapshot(
                &registry,
                admitted,
                &effective,
                &projection,
                now,
                &destination,
            )?;
            Some(
                super::promotion::prepare_governed_promotion_with_claim_projection(
                    &self.binding,
                    isolation_id,
                    &guidance,
                    destination.tree(),
                    now,
                    claim_guard.projection(),
                )?,
            )
        } else {
            None
        };
        claim_guard.revalidate().map_err(|error| {
            WorkflowGovernanceAdapterError::PromotionApply(
                super::promotion::PromotionApplyError::Store(error.to_string()),
            )
        })?;
        destination.revalidate()?;
        super::promotion::recover_promotion_under_lock(
            &self.binding,
            isolation_id,
            expected_preview_digest,
            fallback_prepared,
            destination.tree_mut(),
            &effect_lock,
        )
        .map_err(WorkflowGovernanceAdapterError::PromotionApply)
    }

    /// Adjudicate and durably record a same-owner evidence offer. Rejections
    /// are successful audit writes, never support for a claim.
    pub fn record_cooperative_evidence(
        &self,
        raw_offer: &[u8],
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceAdapterError> {
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let identity = self.identity(admitted);
        validate_identity(&projection, &identity, &self.binding.project_root)?;
        if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
            return Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired);
        }
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let head = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let state_version = projection.current_state_version().unwrap_or_default();
        let objective = active_cooperative_objective_from_ledger(&projection.records)?
            .ok_or(WorkflowGovernanceAdapterError::AgentAutonomyObjectiveRequired)?;
        let current = WorkflowCooperativeEvidenceBinding {
            objective_id: objective.objective_id.clone(),
            objective_revision: objective.revision,
            objective_digest: objective.objective_digest.clone(),
            assurance_epoch: objective.assurance_epoch,
            accepted_objective_record_digest: objective.accepted_record_digest.clone(),
            accepted_objective_record_sequence: objective.accepted_sequence,
            policy_bundle_digest: effective
                .identity()
                .effective_runtime_bundle
                .bundle_digest
                .clone(),
            snapshot_digest: snapshot.digest().to_owned(),
            ledger_head_digest: head.clone(),
            state_version,
        };
        let parsed = if raw_offer.len() <= MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES {
            serde_json::from_slice::<WorkflowCooperativeEvidenceOffer>(raw_offer).ok()
        } else {
            None
        };
        let bounded_offer_id = parsed
            .as_ref()
            .and_then(|offer| cooperative_bounded_offer_id(&offer.offer_id));
        let canonical = parsed
            .as_ref()
            .and_then(|offer| serde_json::to_vec(offer).ok());
        let offer_digest = sha256_content_hash(canonical.as_deref().unwrap_or(raw_offer));
        if let Some(existing) = projection.records.iter().find(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::CooperativeEvidenceObserved(event)
                    if event.offer_digest == offer_digest
            )
        }) {
            return Ok(existing.clone());
        }

        let conflicting_id = bounded_offer_id.as_ref().is_some_and(|offer_id| {
            projection.records.iter().any(|record| {
                matches!(
                    &record.event,
                    WorkflowGovernanceEvent::CooperativeEvidenceObserved(event)
                        if event.offer_id.as_ref() == Some(offer_id)
                            && event.offer_digest != offer_digest
                )
            })
        });
        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &snapshot,
        )?;
        let selected_policy_ref = guidance.selected_policy_ref;
        let selected_policy = policy_by_id(effective.document(), &selected_policy_ref)?;
        let selected_claim =
            selected_cooperative_source_claim(selected_policy, &guidance.simulation);
        let route = parsed.as_ref().and_then(|offer| {
            derived_solo_cooperative_evidence_route(
                selected_policy,
                selected_claim?,
                offer,
                &objective,
            )
        });
        let mut decision = if conflicting_id {
            CooperativeEvidenceDecision {
                disposition: WorkflowCooperativeEvidenceDisposition::Rejected,
                rejection: Some(WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey),
            }
        } else if let (Some(offer), Some(route)) = (parsed.as_ref(), route.as_ref()) {
            if bounded_offer_id.is_none() || !cooperative_offer_text_is_bounded(offer) {
                CooperativeEvidenceDecision {
                    disposition: WorkflowCooperativeEvidenceDisposition::Rejected,
                    rejection: Some(
                        WorkflowCooperativeEvidenceRejection::MalformedOrOversizedOffer,
                    ),
                }
            } else {
                evaluate_cooperative_evidence(&current, route, offer, now)
            }
        } else {
            CooperativeEvidenceDecision {
                disposition: WorkflowCooperativeEvidenceDisposition::Rejected,
                rejection: Some(if parsed.is_some() {
                    WorkflowCooperativeEvidenceRejection::PolicyDoesNotPermitCooperation
                } else {
                    WorkflowCooperativeEvidenceRejection::MalformedOrOversizedOffer
                }),
            }
        };
        if decision.disposition == WorkflowCooperativeEvidenceDisposition::Admitted {
            let subject_is_current = if let Some(offer) = parsed.as_ref() {
                cooperative_subject_current(
                    &self.binding.project_root,
                    snapshot.digest(),
                    &offer.attestation.subject,
                )?
            } else {
                false
            };
            if !subject_is_current {
                decision = CooperativeEvidenceDecision {
                    disposition: WorkflowCooperativeEvidenceDisposition::Rejected,
                    rejection: Some(WorkflowCooperativeEvidenceRejection::SubjectDigestMismatch),
                };
            }
        }
        let admitted_evidence = if decision.disposition
            == WorkflowCooperativeEvidenceDisposition::Admitted
        {
            parsed
                .as_ref()
                .map(|offer| WorkflowAdmittedCooperativeEvidence {
                    offer_id: offer.offer_id.clone(),
                    offer_digest: offer_digest.clone(),
                    policy_version: offer.attestation.policy_version.clone(),
                    claim_descriptor_version: offer.attestation.claim_descriptor_version.clone(),
                    binding: offer.attestation.binding.clone(),
                    policy_ref: offer.attestation.policy_ref.clone(),
                    claim_ref: offer.attestation.claim_ref.clone(),
                    evaluator_ref: offer.attestation.evaluator_ref.clone(),
                    cooperative_claim_ref: offer.attestation.cooperative_claim_ref.clone(),
                    cooperative_evaluator_ref: offer.attestation.cooperative_evaluator_ref.clone(),
                    producer: offer.attestation.producer.clone(),
                    subject: offer.attestation.subject.clone(),
                    scenario_kind: offer.attestation.scenario_kind,
                    scenario_digest: offer.attestation.scenario_digest.clone(),
                    outcome: WorkflowEvidenceOutcome::Pass,
                    execution_observed_at_unix: now,
                    readback_observed_at_unix: now,
                })
        } else {
            None
        };
        let event = WorkflowCooperativeEvidenceObservedEvent {
            offer_id: bounded_offer_id,
            offer_digest,
            admitted_evidence,
            disposition: decision.disposition,
            rejection: decision.rejection,
            admission_snapshot_digest: snapshot.digest().to_owned(),
            admission_ledger_head_digest: head.clone(),
            admission_state_version: state_version,
            observed_at_unix: now,
        };
        snapshot.revalidate()?;
        ledger
            .record_cooperative_evidence_unchecked_tcb(&head, &identity, state_version, event)
            .map_err(WorkflowGovernanceAdapterError::from)
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

    /// Admit one initial same-owner objective or return one irreducible
    /// decision request after read-only validation of the current packet and
    /// workflow state.
    pub fn accept_cooperative_objective(
        &self,
        packet_digest: &str,
        input: WorkflowCooperativeObjectiveInput,
    ) -> Result<WorkflowCooperativeObjectiveAcceptance, WorkflowGovernanceAdapterError> {
        validate_cooperative_objective_input(&input)?;
        let revision_input_digest = if matches!(
            &input,
            WorkflowCooperativeObjectiveInput::MaterialSupersession { .. }
                | WorkflowCooperativeObjectiveInput::NonMaterialClarification { .. }
        ) {
            Some(workflow_cooperative_revision_input_digest(&input)?)
        } else {
            None
        };
        let now = unix_time()?;
        if cooperative_input_host_provenance(&input)
            .is_some_and(|provenance| provenance.observed_at_unix > now)
        {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                "cooperative host observation cannot be in the future".to_owned(),
            ));
        }
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
            return Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired);
        }

        if let Some((objective_record, accepted_event)) =
            accepted_cooperative_objective_record(&projection.records)?
        {
            if packet_digest == accepted_event.acceptance_action_packet_digest {
                if !cooperative_retry_matches(accepted_event, &input)? {
                    return Err(WorkflowGovernanceAdapterError::CooperativeObjectiveRetryConflict);
                }
                self.require_effective_epoch_current(admitted, &effective, &projection)?;
                if projection.head_digest.as_deref()
                    != Some(objective_record.record_digest.as_str())
                    || snapshot.digest() != accepted_event.snapshot_digest
                {
                    return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
                }
                snapshot.revalidate()?;
                let next = self.guidance_from_projection_with_snapshot(
                    &registry,
                    admitted,
                    &effective,
                    &projection,
                    accepted_event.accepted_at_unix,
                    &snapshot,
                )?;
                let active_objective = next
                    .active_cooperative_objective
                    .clone()
                    .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
                return Ok(WorkflowCooperativeObjectiveAcceptance::Accepted {
                    objective_record: objective_record.clone(),
                    active_objective,
                    next: Box::new(next),
                });
            }
        } else if project_durable_assurance(&projection.records)?.is_some() {
            return Err(WorkflowGovernanceAdapterError::CooperativeObjectiveAlreadyAccepted);
        }
        let objective_active =
            accepted_cooperative_objective_record(&projection.records)?.is_some();

        if let WorkflowCooperativeObjectiveInput::DecisionRequired { decision_request } = input {
            self.require_effective_epoch_current(admitted, &effective, &projection)?;
            let guidance = self.guidance_from_projection_with_snapshot(
                &registry,
                admitted,
                &effective,
                &projection,
                now,
                &snapshot,
            )?;
            if let Err(error) = validated_cooperative_objective_packet(&guidance, packet_digest) {
                return Err(
                    if objective_active
                        && matches!(
                            error,
                            WorkflowGovernanceAdapterError::AuthorizationBindingMismatch
                        )
                    {
                        WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket
                    } else {
                        error
                    },
                );
            }
            snapshot.revalidate()?;
            if projection.head_digest.as_deref() != Some(guidance.ledger_head_digest.as_str())
                || snapshot.digest() != guidance.snapshot_digest
            {
                return Err(if objective_active {
                    WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket
                } else {
                    WorkflowGovernanceAdapterError::AuthorizationBindingMismatch
                });
            }
            return Ok(WorkflowCooperativeObjectiveAcceptance::DecisionRequired {
                decision_request,
            });
        }

        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &snapshot,
        )?;
        let previous =
            accepted_cooperative_objective_record(&projection.records)?.map(|(_, event)| event);
        let (packet, objective_id, next_objective_revision, next_assurance_epoch) =
            match validated_cooperative_objective_packet(&guidance, packet_digest) {
                Ok(packet) => packet,
                Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
                    if previous.is_some() =>
                {
                    return Err(
                        WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket,
                    );
                }
                Err(error) => return Err(error),
            };
        if packet.binding.project_id != self.binding.project_id {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let (expected_revision, expected_epoch) = match previous {
            Some(event) => (
                event
                    .revision
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
                event
                    .assurance_epoch
                    .checked_add(1)
                    .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?,
            ),
            None => (1, 1),
        };
        if next_objective_revision != expected_revision
            || next_assurance_epoch != expected_epoch
            || previous.is_some_and(|event| event.objective_id != objective_id)
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let (proposal, revision_kind, revision_reason, carrying_principal, host_provenance) =
            cooperative_revision_from_input(previous, input)?;
        let objective_digest = workflow_cooperative_objective_digest(
            &objective_id,
            next_objective_revision,
            next_assurance_epoch,
            &proposal,
        )?;
        #[cfg(test)]
        inject_project_change_before_cooperative_commit(
            &self.binding.state_root,
            &self.binding.project_root,
        );
        snapshot.revalidate()?;
        if snapshot.digest() != packet.binding.snapshot_digest
            || projection.head_digest.as_deref() != Some(packet.binding.ledger_head_digest.as_str())
        {
            return Err(if previous.is_some() {
                WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket
            } else {
                WorkflowGovernanceAdapterError::AuthorizationBindingMismatch
            });
        }
        let event = CooperativeObjectiveAcceptedEvent {
            objective_id,
            revision: next_objective_revision,
            assurance_epoch: next_assurance_epoch,
            proposal,
            objective_digest,
            previous_objective_digest: previous.map(|event| event.objective_digest.clone()),
            revision_kind,
            revision_reason,
            revision_input_digest,
            snapshot_digest: packet.binding.snapshot_digest.clone(),
            ledger_head_digest: packet.binding.ledger_head_digest.clone(),
            acceptance_action_packet_digest: packet.packet_digest.clone(),
            carrying_principal,
            host_provenance,
            authority_basis: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
            accepted_at_unix: now,
        };
        let identity = self.identity(admitted);
        let objective_record = ledger.accept_cooperative_objective_unchecked_tcb(
            &packet.binding.ledger_head_digest,
            &identity,
            projection.current_state_version().unwrap_or_default(),
            event,
        )?;
        let committed = ledger.recover()?;
        let next = self.guidance_from_projection_with_snapshot(
            &registry, admitted, &effective, &committed, now, &snapshot,
        )?;
        let active_objective = next
            .active_cooperative_objective
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        Ok(WorkflowCooperativeObjectiveAcceptance::Accepted {
            objective_record,
            active_objective,
            next: Box::new(next),
        })
    }

    fn action_packets_at(
        &self,
        now: u64,
    ) -> Result<WorkflowAuthorizationActionPacketSet, WorkflowGovernanceAdapterError> {
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        self.action_packets_at_with_snapshot(now, &snapshot)
    }

    fn action_packets_at_with_snapshot(
        &self,
        now: u64,
        snapshot: &RetainedWorkflowProjectSnapshot,
    ) -> Result<WorkflowAuthorizationActionPacketSet, WorkflowGovernanceAdapterError> {
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            snapshot,
        )?;
        snapshot.revalidate()?;
        let WorkflowAuthorizationGuidance {
            registry_setup,
            setup_gaps,
            action_packets,
            objective_management_packet,
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
            objective_management_packet,
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
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let packet_set = self.action_packets_at_with_snapshot(now, &snapshot)?;
        let packet = packet_set
            .packets
            .into_iter()
            .find(|candidate| candidate.packet_digest == packet_digest)
            .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;

        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        snapshot.revalidate()?;
        if snapshot.digest() != packet_set.snapshot_digest
            || projection.head_digest.as_deref() != Some(packet_set.ledger_head_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        Ok(prepared)
    }

    /// Unit-test seam for the frozen legacy authority. Production callers have
    /// no unbound live-mutation API: only a strict control-plane capability may
    /// enter the append path below.
    #[cfg(test)]
    fn apply_verified_broker_action(
        &self,
        verified: VerifiedWorkflowBrokerEvent,
        now: u64,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        self.apply_verified_broker_action_inner(verified, now, None, None)
    }

    /// Apply a strict control-plane capability while retaining the exact admitted
    /// registry digest and rotation-stable native interaction replay identity.
    pub fn apply_verified_bound_broker_action(
        &self,
        verified: VerifiedBoundWorkflowBrokerEvent,
        now: u64,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        let (verified, bound) = verified.into_parts();
        self.apply_verified_broker_action_inner(
            verified,
            now,
            Some(bound.registry_digest),
            Some(bound.native_interaction_replay_digest),
        )
    }

    fn apply_verified_broker_action_inner(
        &self,
        verified: VerifiedWorkflowBrokerEvent,
        now: u64,
        admitted_registry_digest: Option<String>,
        stable_replay_origin_id: Option<String>,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        if now == 0 {
            return Err(WorkflowGovernanceAdapterError::Clock);
        }
        let (semantic_input, audit) = verified.into_parts();
        if audit.project_id != self.binding.project_id {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        if let Some(expected) = admitted_registry_digest.as_deref() {
            if self.current_trusted_broker_registry_digest()?.as_deref() != Some(expected) {
                return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
            }
        }

        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let replay_origin_id = stable_replay_origin_id
            .clone()
            .map_or_else(|| broker_replay_origin_id(&audit), Ok)?;
        if let Some((action_record, origin_record)) = matching_broker_origin_retry(
            &projection,
            &audit,
            admitted_registry_digest.as_deref(),
            stable_replay_origin_id.as_deref(),
        )? {
            let replay_repaired = ensure_broker_replay_committed(
                &self.binding.state_root,
                &audit.action_packet_digest,
                &replay_origin_id,
                &action_record.record_digest,
            )?;
            let next = self.guidance_from_projection_with_snapshot(
                &registry,
                admitted,
                &effective,
                &projection,
                unix_time()?,
                &project_snapshot,
            )?;
            project_snapshot.revalidate()?;
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
        if admitted_registry_digest
            .as_deref()
            .is_some_and(|expected| expected != broker_registry_digest.as_str())
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }

        let guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            current_now,
            &project_snapshot,
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
        let origin_event =
            WorkflowGovernanceEvent::BrokerOriginApplied(broker_origin_applied_event(
                &packet,
                &audit,
                &broker_registry_digest,
                stable_replay_origin_id.as_deref(),
                &action_record,
            ));
        let origin_record = batch.push_event(packet.binding.state_version, origin_event)?;
        let phase_advanced_record = if phase_may_advance {
            self.plan_phase_advance_with_snapshot(
                &effective,
                batch.projection(),
                commit_now,
                &project_snapshot,
            )?
            .map(|(state_version, event)| batch.push_event(state_version, event))
            .transpose()?
        } else {
            None
        };
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != packet.binding.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let final_commit_now = unix_time()?;
        if audit.issued_at_unix > final_commit_now
            || audit.expires_at_unix <= final_commit_now
            || self.current_trusted_registry_digest()?
                != packet.binding.trusted_principal_registry_digest
            || self.current_trusted_broker_registry_digest()?
                != packet.binding.trusted_broker_registry_digest
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let replay_reservation = begin_workflow_action_replay_reservation(
            &self.binding.state_root,
            &packet.packet_digest,
            &replay_origin_id,
            &action_record.record_digest,
        )?;
        #[cfg(all(test, unix))]
        inject_byte_identical_project_replacement_after_replay_reservation(
            &self.binding.state_root,
            &self.binding.project_root,
        );
        let locked_commit_now = replay_locked_commit_time(&self.binding.state_root)?;
        if audit.issued_at_unix > locked_commit_now
            || audit.expires_at_unix <= locked_commit_now
            || self.current_trusted_registry_digest()?
                != packet.binding.trusted_principal_registry_digest
            || self.current_trusted_broker_registry_digest()?
                != packet.binding.trusted_broker_registry_digest
        {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != packet.binding.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        batch.commit()?;
        #[cfg(test)]
        inject_replay_append_failure_after_ledger(&self.binding.state_root);
        replay_reservation.commit_after_authoritative_ledger()?;
        let committed = ledger.recover()?;
        let next = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &committed,
            final_commit_now,
            &project_snapshot,
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
        self.recover_historically_verified_broker_action_inner(verified, None, None)
    }

    /// Repair replay state for a strict control-plane event using its
    /// rotation-stable native interaction identity. This path still cannot append
    /// a governance event.
    pub fn recover_historically_verified_bound_broker_action(
        &self,
        verified: HistoricallyVerifiedBoundWorkflowBrokerEvent,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        let (verified, bound) = verified.into_parts();
        self.recover_historically_verified_broker_action_inner(
            verified,
            Some(bound.registry_digest),
            Some(bound.native_interaction_replay_digest),
        )
    }

    fn recover_historically_verified_broker_action_inner(
        &self,
        verified: HistoricallyVerifiedWorkflowBrokerEvent,
        admitted_registry_digest: Option<String>,
        stable_replay_origin_id: Option<String>,
    ) -> Result<WorkflowBrokerActionReceipt, WorkflowGovernanceAdapterError> {
        let (_, audit) = verified.into_parts();
        if audit.project_id != self.binding.project_id {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        let active_effective = match projection.active_effective_bundle_identity() {
            Some(active) => active,
            None => derive_core_only_workflow_effective_identity(admitted)?,
        };
        if active_effective != *effective.identity() {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let (action_record, origin_record) = matching_broker_origin_retry(
            &projection,
            &audit,
            admitted_registry_digest.as_deref(),
            stable_replay_origin_id.as_deref(),
        )?
        .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
        let replay_origin_id =
            stable_replay_origin_id.map_or_else(|| broker_replay_origin_id(&audit), Ok)?;
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
        Ok(WorkflowBrokerActionReceipt {
            action_record,
            origin_record,
            phase_advanced_record: None,
            replay_commit_repaired: replay_repaired,
            next,
        })
    }

    /// Replacement-agent view. This is intentionally the same deterministic
    /// authority derivation as `next`; chat history is not an input. Unlike
    /// `next`, this observer never repairs a pending release rebase, creates a
    /// missing lock, or reconciles a Domain Pack generation into the ledger.
    ///
    /// # Errors
    /// Returns a typed error when durable guidance cannot be reconstructed.
    pub fn resume(&self) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire_existing(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let ledger = observe_existing_workflow_governance_ledger(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        self.require_effective_epoch_current(admitted, &effective, &projection)
            .map_err(|_| {
                WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
                    "the Domain Pack generation or pending release rebase requires an explicit mutating workflow command before continuation can be projected",
                )
            })?;
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let mut guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &snapshot,
        )?;
        let continuity = self.replacement_continuity(&guidance, &projection, &snapshot, now)?;
        snapshot.revalidate()?;
        let final_projection = ledger.recover()?;
        if final_projection.head_digest != projection.head_digest
            || final_projection.current_state_version() != projection.current_state_version()
            || final_projection.records != projection.records
        {
            return Err(
                WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
                    "durable workflow state changed during read-only replacement inspection",
                ),
            );
        }
        guidance.replacement_continuity = Some(continuity);
        Ok(guidance)
    }

    fn replacement_continuity(
        &self,
        guidance: &WorkflowGovernanceGuidance,
        projection: &WorkflowGovernanceLedgerProjection,
        snapshot: &RetainedWorkflowProjectSnapshot,
        now: u64,
    ) -> Result<WorkflowReplacementContinuity, WorkflowGovernanceAdapterError> {
        let head = projection
            .head_digest
            .clone()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        let state_version = projection.current_state_version().unwrap_or_default();
        if head != guidance.ledger_head_digest || state_version != guidance.state_version {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        let claims = replacement_claims_from_existing_state(&self.binding.state_root, now)?;
        let (objective_history, active_objective_digest, active_objective_revision, active_epoch) =
            replacement_objective_history(&projection.records, guidance.readiness_profile)?;
        let decision_history = replacement_decision_history(&projection.records);
        let durable_pending_decisions = decision_history
            .iter()
            .filter(|decision| decision.status == WorkflowReplacementDecisionStatus::Unresolved)
            .cloned()
            .collect::<Vec<_>>();
        let governed_evidence = replacement_evidence_history(&projection.records, guidance, now);

        let workspace = super::promotion::inspect_replacement_workspace(
            &self.binding,
            guidance.readiness_profile,
            guidance,
            now,
        );
        let workspace_binding = workspace.clone();
        let mut gaps = workspace
            .gaps
            .into_iter()
            .map(|gap| WorkflowReplacementGap {
                code: match gap.code {
                    super::promotion::ReplacementWorkspaceGapCode::IsolationRegistryInvalid => {
                        WorkflowReplacementGapCode::IsolationRegistryInvalid
                    }
                    super::promotion::ReplacementWorkspaceGapCode::IsolationConflict => {
                        WorkflowReplacementGapCode::IsolationConflict
                    }
                    super::promotion::ReplacementWorkspaceGapCode::WorktreeMissing => {
                        WorkflowReplacementGapCode::WorktreeMissing
                    }
                    super::promotion::ReplacementWorkspaceGapCode::GitWorktreeMismatch => {
                        WorkflowReplacementGapCode::GitWorktreeMismatch
                    }
                    super::promotion::ReplacementWorkspaceGapCode::PromotionStateInvalid => {
                        WorkflowReplacementGapCode::PromotionStateInvalid
                    }
                    super::promotion::ReplacementWorkspaceGapCode::PromotionRequiresSoloProfile => {
                        WorkflowReplacementGapCode::PromotionRequiresSoloProfile
                    }
                },
                blocking: gap.blocking,
                summary: gap.summary,
                isolation_id: gap.isolation_id,
            })
            .collect::<Vec<_>>();
        let mut isolations = workspace
            .isolations
            .into_iter()
            .map(|isolation| WorkflowReplacementIsolationAudit {
                contract_path: isolation.contract_path,
                contract_digest: isolation.contract_digest,
                declared_worktree: isolation.declared_worktree,
                validation: match isolation.validation {
                    super::promotion::ReplacementIsolationValidation::Valid => {
                        WorkflowReplacementIsolationValidation::Valid
                    }
                    super::promotion::ReplacementIsolationValidation::ProposedNotCreated => {
                        WorkflowReplacementIsolationValidation::ProposedNotCreated
                    }
                    super::promotion::ReplacementIsolationValidation::RetiredWorktreeAbsent => {
                        WorkflowReplacementIsolationValidation::RetiredWorktreeAbsent
                    }
                    super::promotion::ReplacementIsolationValidation::Missing => {
                        WorkflowReplacementIsolationValidation::Missing
                    }
                    super::promotion::ReplacementIsolationValidation::Mismatched => {
                        WorkflowReplacementIsolationValidation::Mismatched
                    }
                },
                git: isolation.git,
                gap_codes: isolation
                    .gap_codes
                    .into_iter()
                    .map(|code| match code {
                        super::promotion::ReplacementWorkspaceGapCode::IsolationRegistryInvalid => {
                            WorkflowReplacementGapCode::IsolationRegistryInvalid
                        }
                        super::promotion::ReplacementWorkspaceGapCode::IsolationConflict => {
                            WorkflowReplacementGapCode::IsolationConflict
                        }
                        super::promotion::ReplacementWorkspaceGapCode::WorktreeMissing => {
                            WorkflowReplacementGapCode::WorktreeMissing
                        }
                        super::promotion::ReplacementWorkspaceGapCode::GitWorktreeMismatch => {
                            WorkflowReplacementGapCode::GitWorktreeMismatch
                        }
                        super::promotion::ReplacementWorkspaceGapCode::PromotionStateInvalid => {
                            WorkflowReplacementGapCode::PromotionStateInvalid
                        }
                        super::promotion::ReplacementWorkspaceGapCode::PromotionRequiresSoloProfile => {
                            WorkflowReplacementGapCode::PromotionRequiresSoloProfile
                        }
                    })
                    .collect(),
                contract: isolation.contract,
            })
            .collect::<Vec<_>>();

        for isolation in &mut isolations {
            let Some(claim_id) = isolation.contract.claim_id.as_ref() else {
                continue;
            };
            let linked = claims
                .iter()
                .find(|claim| claim.claim.id.0.as_str() == claim_id.0.as_str());
            match linked {
                None => {
                    isolation
                        .gap_codes
                        .push(WorkflowReplacementGapCode::LinkedClaimMissing);
                    gaps.push(WorkflowReplacementGap {
                        code: WorkflowReplacementGapCode::LinkedClaimMissing,
                        blocking: matches!(
                            isolation.contract.status,
                            IsolationStatus::Active | IsolationStatus::Merging
                        ),
                        summary: format!(
                            "Isolation {} points to claim {} but that durable claim is absent.",
                            isolation.contract.id.0, claim_id.0
                        ),
                        isolation_id: Some(isolation.contract.id.clone()),
                    });
                }
                Some(claim)
                    if claim.claim.claim.claimant_agent_id != isolation.contract.agent_id =>
                {
                    isolation
                        .gap_codes
                        .push(WorkflowReplacementGapCode::LinkedClaimOwnerMismatch);
                    gaps.push(WorkflowReplacementGap {
                        code: WorkflowReplacementGapCode::LinkedClaimOwnerMismatch,
                        blocking: matches!(
                            isolation.contract.status,
                            IsolationStatus::Active | IsolationStatus::Merging
                        ),
                        summary: format!(
                            "Isolation {} belongs to agent {}, but linked claim {} belongs to agent {}.",
                            isolation.contract.id.0,
                            isolation.contract.agent_id.0,
                            claim_id.0,
                            claim.claim.claim.claimant_agent_id.0
                        ),
                        isolation_id: Some(isolation.contract.id.clone()),
                    });
                }
                Some(claim) => {
                    let (code, summary) = match claim.liveness {
                        ReplacementClaimLiveness::Live => continue,
                        ReplacementClaimLiveness::Expired => (
                            WorkflowReplacementGapCode::LinkedClaimExpired,
                            format!(
                                "Isolation {} is linked to claim {}, but that claim has expired.",
                                isolation.contract.id.0, claim_id.0
                            ),
                        ),
                        ReplacementClaimLiveness::NonActive => (
                            WorkflowReplacementGapCode::LinkedClaimInactive,
                            format!(
                                "Isolation {} is linked to claim {}, but that claim was released or is otherwise inactive.",
                                isolation.contract.id.0, claim_id.0
                            ),
                        ),
                    };
                    isolation.gap_codes.push(code);
                    gaps.push(WorkflowReplacementGap {
                        code,
                        blocking: matches!(
                            isolation.contract.status,
                            IsolationStatus::Active | IsolationStatus::Merging
                        ),
                        summary,
                        isolation_id: Some(isolation.contract.id.clone()),
                    });
                }
            }
            isolation.gap_codes.sort();
            isolation.gap_codes.dedup();
        }
        gaps.sort_by(|left, right| {
            left.isolation_id
                .cmp(&right.isolation_id)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.summary.cmp(&right.summary))
        });
        let promotions = workspace
            .promotions
            .into_iter()
            .map(|promotion| {
                let status = match promotion.status {
                    super::promotion::ReplacementPromotionStatus::NotStarted => {
                        WorkflowReplacementPromotionStatus::NotStarted
                    }
                    super::promotion::ReplacementPromotionStatus::Recoverable => {
                        WorkflowReplacementPromotionStatus::Recoverable
                    }
                    super::promotion::ReplacementPromotionStatus::Completed => {
                        WorkflowReplacementPromotionStatus::Completed
                    }
                    super::promotion::ReplacementPromotionStatus::BlockedCorrupt => {
                        WorkflowReplacementPromotionStatus::BlockedCorrupt
                    }
                };
                let recovery_argv = (status == WorkflowReplacementPromotionStatus::Recoverable)
                    .then(|| {
                        vec![
                            "forge-core".to_owned(),
                            "workflow".to_owned(),
                            "promotion".to_owned(),
                            "recover".to_owned(),
                            "--root".to_owned(),
                            self.binding.project_root.display().to_string(),
                            "--isolation-id".to_owned(),
                            promotion.isolation_id.0.clone(),
                            "--expected-preview-digest".to_owned(),
                            promotion
                                .preview_digest
                                .clone()
                                .expect("recoverable promotion has preview digest"),
                            "--json".to_owned(),
                        ]
                    });
                WorkflowReplacementPromotionAudit {
                    isolation_id: promotion.isolation_id,
                    status,
                    preview_digest: promotion.preview_digest,
                    receipt_digest: promotion.receipt_digest,
                    recovery_argv: recovery_argv.unwrap_or_default(),
                    summary: promotion.summary,
                }
            })
            .collect::<Vec<_>>();

        let claim_projection_digest =
            replacement_projection_digest("replacement.claims.v1", &claims)?;
        let isolation_registry_digest =
            replacement_projection_digest("replacement.isolations.v1", &isolations)?;
        let promotion_projection_digest =
            replacement_projection_digest("replacement.promotions.v1", &promotions)?;
        let mut ranked_next_actions = replacement_ranked_actions(
            &promotions,
            &gaps,
            &guidance.simulation.candidate_next_actions,
        );
        for (index, action) in ranked_next_actions.iter_mut().enumerate() {
            action.rank = u32::try_from(index + 1).unwrap_or(u32::MAX);
        }
        let ranked_action_digest =
            replacement_projection_digest("replacement.ranked_actions.v1", &ranked_next_actions)?;
        snapshot.revalidate()?;
        let final_claims = replacement_claims_from_existing_state(&self.binding.state_root, now)?;
        if final_claims != claims {
            return Err(
                WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
                    "claim state changed during read-only replacement inspection",
                ),
            );
        }
        let final_workspace = super::promotion::inspect_replacement_workspace(
            &self.binding,
            guidance.readiness_profile,
            guidance,
            now,
        );
        if final_workspace != workspace_binding {
            return Err(
                WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
                    "isolation or promotion state changed during read-only replacement inspection",
                ),
            );
        }
        let status = if gaps.iter().any(|gap| gap.blocking)
            || promotions.iter().any(|promotion| {
                promotion.status == WorkflowReplacementPromotionStatus::BlockedCorrupt
            }) {
            WorkflowReplacementContinuityStatus::Blocked
        } else {
            WorkflowReplacementContinuityStatus::Ready
        };
        Ok(WorkflowReplacementContinuity {
            schema_version: WORKFLOW_REPLACEMENT_CONTINUITY_SCHEMA_VERSION.to_owned(),
            status,
            binding: WorkflowReplacementContinuityBinding {
                project_id: guidance.project_id.clone(),
                readiness_profile: guidance.readiness_profile,
                project_snapshot_digest: guidance.snapshot_digest.clone(),
                ledger_head_digest: guidance.ledger_head_digest.clone(),
                state_version: guidance.state_version,
                active_release_digest: guidance.release.release.release_digest.clone(),
                active_objective_digest,
                active_objective_revision,
                active_assurance_epoch: active_epoch,
                claim_projection_digest,
                isolation_registry_digest,
                promotion_projection_digest,
            },
            objective_history,
            durable_pending_decisions,
            decision_history,
            governed_evidence,
            cooperative_evidence: guidance.cooperative_evidence.clone(),
            claims,
            isolations,
            promotions,
            gaps,
            ranked_next_actions,
            ranked_action_digest,
        })
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
        let plan = self.derive_domain_pack_rebase_plan(
            source,
            target,
            &effective,
            &domain,
            &head_digest,
            &snapshot_digest,
        )?;
        project_snapshot.revalidate()?;
        if plan.domain_pack_rebase_plan.plan_digest != expected_rebase_plan_digest
            || project_snapshot.digest() != snapshot_digest
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
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
        project_snapshot.revalidate()?;
        if source.release() != &plan.source_release
            || target.release() != &plan.target_release
            || !target.is_adjacent_successor_of(source)
            || projection.head_digest.as_deref()
                != Some(plan.exact_cas.expected_workflow_ledger_head_digest.as_str())
            || project_snapshot.digest() != plan.exact_cas.expected_project_snapshot_digest
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
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != plan.exact_cas.expected_project_snapshot_digest {
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
        let bytes = {
            let retained_root = RetainedEffectStoreRoot::acquire(&self.binding.state_root)
                .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
                    source: format!("cannot retain Domain Pack rebase-plan root: {error}"),
                })?;
            let lock = retained_root
                .acquire_effect_store_lock(DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH)
                .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
                    source: format!("cannot lock Domain Pack rebase plan: {error}"),
                })?;
            let observation = observe_file_crash_safe_under_owned_lock(
                lock,
                Path::new(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH),
                DOMAIN_PACK_REBASE_PLAN_MAX_BYTES,
            )
            .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
                source: format!("cannot recover Domain Pack rebase plan: {error}"),
            })?;
            let Some(session) = observation.into_present_session() else {
                return Ok(false);
            };
            let Some(mut read) = session.read_exact().map_err(|error| {
                WorkflowGovernanceAdapterError::ProjectBinding {
                    source: format!("cannot retain exact Domain Pack rebase plan: {error}"),
                }
            })?
            else {
                return Ok(false);
            };
            read.revalidate()
                .map_err(|error| WorkflowGovernanceAdapterError::ProjectBinding {
                    source: format!("Domain Pack rebase plan changed after recovery: {error}"),
                })?;
            read.raw_bytes().to_vec()
        };
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
        let lifecycle = lock_domain_pack_lifecycle_for_project(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let source = lifecycle.active_rebase_source()?;
        let expected_generation = plan
            .domain_pack_rebase_plan
            .exact_cas
            .expected_generation
            .checked_add(1)
            .ok_or(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch)?;
        let expected_source_binding_digest = &plan
            .domain_pack_rebase_plan
            .exact_cas
            .expected_operator_source_binding_digest;
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
                && source.operator_source_binding.generation == expected_generation
                && source.from_operator_source_binding_digest.as_ref()
                    == Some(expected_source_binding_digest)
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
            operator_source_binding_digest: material.operator_source_binding_digest,
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        if domain.has_active_generation() {
            return Err(WorkflowGovernanceAdapterError::DomainPackRebaseRequired);
        }
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let projection = ledger.recover()?;
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
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
            project_snapshot.revalidate()?;
            let replay_snapshot = project_snapshot.digest().to_owned();
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
        let snapshot_digest = project_snapshot.digest().to_owned();
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
        // editors. Revalidate the same retained namespace and file handles at the
        // final boundary so byte-identical replacement cannot satisfy this CAS.
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
            &project_snapshot,
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
        if let Some((state_version, event)) = self.plan_phase_advance_with_snapshot(
            &effective,
            batch.projection(),
            unix_time()?,
            &project_snapshot,
        )? {
            batch.push_event(state_version, event)?;
        }
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
            &project_snapshot,
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
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
            &project_snapshot,
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
            snapshot_digest: snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
        });
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
            &project_snapshot,
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
            snapshot_digest: snapshot_digest.clone(),
            ledger_head_digest: head.to_owned(),
            authorization_intent_digest: audit.intent_digest,
            signature_fingerprint: audit.signature_fingerprint,
            resolved_at_unix: unix_time()?,
        });
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
            &project_snapshot,
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
                subject_digest: snapshot_digest.clone(),
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
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
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
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let snapshot_digest = project_snapshot.digest().to_owned();
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
        if let Some((state_version, event)) = self.plan_phase_advance_with_snapshot(
            &effective,
            batch.projection(),
            unix_time()?,
            &project_snapshot,
        )? {
            batch.push_event(state_version, event)?;
        }
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != snapshot_digest {
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
        self.prepare_completion_for_snapshot_inner(None)
    }

    /// Convert currently verified completion into a one-use token while also
    /// binding the preparation to the caller's current project snapshot.
    ///
    /// # Errors
    /// Returns a typed drift error when the expected snapshot is no longer
    /// current, or the same completion errors as [`Self::prepare_completion`].
    pub fn prepare_completion_for_snapshot(
        &self,
        expected_snapshot_digest: &str,
    ) -> Result<PreparedWorkflowGovernanceCompletion, WorkflowGovernanceAdapterError> {
        self.prepare_completion_for_snapshot_inner(Some(expected_snapshot_digest))
    }

    fn prepare_completion_for_snapshot_inner(
        &self,
        expected_snapshot_digest: Option<&str>,
    ) -> Result<PreparedWorkflowGovernanceCompletion, WorkflowGovernanceAdapterError> {
        self.recover_pending_release_rebase()?;
        let now = unix_time()?;
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()?;
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let project_snapshot =
            RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        let (guidance, verified) = self.verified_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            &project_snapshot,
        )?;
        if expected_snapshot_digest.is_some_and(|expected| expected != guidance.snapshot_digest) {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        if guidance.status == WorkflowGovernanceGuidanceStatus::PhaseComplete {
            return Err(WorkflowGovernanceAdapterError::PolicyAlreadyCompleted);
        }
        if guidance.status != WorkflowGovernanceGuidanceStatus::ReadyToComplete {
            return Err(WorkflowGovernanceAdapterError::PolicyIncomplete);
        }
        let completion = verified
            .try_into_completion()
            .map_err(|_| WorkflowGovernanceAdapterError::PolicyIncomplete)?;
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != guidance.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        Ok(PreparedWorkflowGovernanceCompletion {
            completion,
            project_snapshot,
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
        let domain = LockedWorkflowDomainPackContext::acquire(
            &self.binding.project_root,
            &self.binding.state_root,
        )?;
        let mut ledger = lock_workflow_governance_ledger_tcb(&self.binding.state_root)?;
        let mut projection = ledger.recover()?;
        let admitted = self.resolve_active_release(&registry, &projection)?;
        let effective = domain.admit_effective(admitted)?;
        projection =
            self.reconcile_effective_epoch(&mut ledger, admitted, &effective, projection)?;
        Self::ensure_domain_pack_ready_for_mutation(&effective)?;
        let identity = self.identity(admitted);
        let project_snapshot = &prepared.project_snapshot;
        let (fresh, verified) = self.verified_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            &projection,
            now,
            project_snapshot,
        )?;
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
        // editors. Revalidate the exact retained namespace and file handles at
        // every late boundary rather than accepting a byte-identical remint.
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != fresh.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
        let mut batch = ledger.begin_unchecked_tcb_batch(&fresh.ledger_head_digest, &identity)?;
        let completed = batch.push_event(completed_state_version, event)?;
        let phase_advanced = if let Some((state_version, event)) = self
            .plan_phase_advance_with_snapshot(
                &effective,
                batch.projection(),
                now,
                project_snapshot,
            )? {
            Some(batch.push_event(state_version, event)?)
        } else {
            None
        };
        let next_guidance = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            batch.projection(),
            now,
            project_snapshot,
        )?;
        let continuity_event =
            WorkflowGovernanceEvent::ContinuityRecorded(ContinuityRecordedEvent {
                from_principal: None,
                to_principal: continuity_principal,
                snapshot_digest: fresh.snapshot_digest.clone(),
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
        let next = self.guidance_from_projection_with_snapshot(
            &registry,
            admitted,
            &effective,
            batch.projection(),
            now,
            project_snapshot,
        )?;
        project_snapshot.revalidate()?;
        if project_snapshot.digest() != fresh.snapshot_digest {
            return Err(WorkflowGovernanceAdapterError::CompletionDrift);
        }
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
                    &record.event,
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

    /// Prove that a read-only cooperative lane can derive its packet without
    /// silently performing the normal Domain Pack reconciliation write.
    fn require_effective_epoch_current(
        &self,
        core: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
    ) -> Result<(), WorkflowGovernanceAdapterError> {
        let core_identity = self.identity(core);
        validate_identity(projection, &core_identity, &self.binding.project_root)?;
        let active = match projection.active_effective_bundle_identity() {
            Some(active) => active,
            None => derive_core_only_workflow_effective_identity(core)?,
        };
        if active != *effective.identity() {
            return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
        }
        Ok(())
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
        let expected_audience = self.expected_broker_audience();
        if let Ok(document) = yaml_serde::from_str::<WorkflowBrokerPublicRegistryDocument>(&raw) {
            let control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
                document.clone(),
                &expected_audience,
                &self.binding.project_id,
                &StableId("workflow.governance".to_owned()),
            )
            .map_err(|error| WorkflowGovernanceAdapterError::TrustedRegistry {
                source: format!("{} is invalid: {error}", path.display()),
            })?;
            let setup = if document.credentials.iter().any(|credential| {
                credential.purpose
                    == forge_core_contracts::WorkflowBrokerCredentialPurpose::EventIssuer
                    && credential.status == WorkflowBrokerCredentialStatus::Active
            }) {
                WorkflowAuthorizationRegistrySetupStatus::Ready
            } else {
                WorkflowAuthorizationRegistrySetupStatus::NoActiveIssuer
            };
            return Ok(TrustedBrokerRegistryState {
                digest: Some(control.registry_digest().to_owned()),
                setup,
            });
        }
        let document: WorkflowBrokerRegistryDocument =
            yaml_serde::from_str(&raw).map_err(|error| {
                WorkflowGovernanceAdapterError::TrustedRegistry {
                    source: format!("cannot parse {}: {error}", path.display()),
                }
            })?;
        AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
            document.clone(),
            &expected_audience,
        )
        .map_err(|error| WorkflowGovernanceAdapterError::TrustedRegistry {
            source: format!("{} is invalid: {error}", path.display()),
        })?;
        // A legacy registry remains readable so exact historical replay repair
        // can verify frozen v0.1 evidence. Its active issuers are deliberately
        // not advertised as live setup authority.
        let setup = WorkflowAuthorizationRegistrySetupStatus::LegacyRecoveryOnly;
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
        let snapshot = RetainedWorkflowProjectSnapshot::capture(&self.binding.project_root)?;
        self.guidance_from_projection_with_snapshot(
            registry, admitted, effective, projection, now, &snapshot,
        )
    }

    fn guidance_from_projection_with_snapshot(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
        snapshot: &RetainedWorkflowProjectSnapshot,
    ) -> Result<WorkflowGovernanceGuidance, WorkflowGovernanceAdapterError> {
        self.verified_from_projection_with_snapshot(
            registry, admitted, effective, projection, now, snapshot,
        )
        .map(|(guidance, _)| guidance)
    }

    fn verified_from_projection_with_snapshot(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
        snapshot: &RetainedWorkflowProjectSnapshot,
    ) -> Result<
        (
            WorkflowGovernanceGuidance,
            VerifiedWorkflowGovernanceDecision,
        ),
        WorkflowGovernanceAdapterError,
    > {
        let identity = self.identity(admitted);
        validate_identity(projection, &identity, &self.binding.project_root)?;
        let readiness_profile = projection
            .readiness_profile()
            .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
        snapshot.revalidate()?;
        let snapshot_digest = snapshot.digest().to_owned();
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
        let strict_assurance_projection = project_durable_assurance(&projection.records)?;
        let cooperative_assurance_projection =
            project_cooperative_durable_assurance(&projection.records)?;
        let base_assurance_projection = match readiness_profile {
            WorkflowReadinessProfile::SoloCooperative => {
                cooperative_assurance_projection.or(strict_assurance_projection)
            }
            WorkflowReadinessProfile::StrictExternal => strict_assurance_projection,
        };
        let active_cooperative_objective =
            active_cooperative_objective_from_ledger(&projection.records)?;
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
        let agent_autonomy = workflow_agent_autonomy_guidance(
            readiness_profile,
            active_cooperative_objective.as_ref(),
            &snapshot_digest,
            &assurance_source_head,
            projection.current_state_version().unwrap_or_default(),
        );
        let durable_assurance = match durable_assurance_projection {
            Some(projection) => {
                let blockers = durable_assurance_blockers(&projection);
                WorkflowDurableAssuranceGuidance {
                    status: if readiness_profile == WorkflowReadinessProfile::SoloCooperative
                        && active_cooperative_objective.is_some()
                    {
                        WorkflowDurableAssuranceStatus::ObjectiveAccepted
                    } else {
                        WorkflowDurableAssuranceStatus::IntentAccepted
                    },
                    blockers,
                    current_snapshot_digest: snapshot_digest.clone(),
                    source_ledger_head_digest: assurance_source_head.clone(),
                    case_digest: assurance_case_digest.clone(),
                    projection: Some(projection),
                }
            }
            None => WorkflowDurableAssuranceGuidance {
                status: if readiness_profile == WorkflowReadinessProfile::SoloCooperative {
                    WorkflowDurableAssuranceStatus::MissingObjective
                } else {
                    WorkflowDurableAssuranceStatus::MissingHumanIntent
                },
                blockers: vec![
                    if readiness_profile == WorkflowReadinessProfile::SoloCooperative {
                        WorkflowDurableAssuranceBlocker {
                        code: WorkflowDurableAssuranceBlockerCode::MissingAcceptedObjective,
                        lens: None,
                        summary: "An unambiguous same-owner objective must be accepted before governed work can proceed."
                            .to_owned(),
                    }
                    } else {
                        WorkflowDurableAssuranceBlocker {
                        code: WorkflowDurableAssuranceBlockerCode::MissingAcceptedHumanIntent,
                        lens: None,
                        summary: "A human-origin intent revision must be accepted before governed work can proceed."
                            .to_owned(),
                    }
                    },
                ],
                current_snapshot_digest: snapshot_digest.clone(),
                source_ledger_head_digest: assurance_source_head.clone(),
                case_digest: assurance_case_digest,
                projection: None,
            },
        };
        let cooperative_source_claim =
            selected_cooperative_source_claim(selected, &verified.simulation);
        let cooperative_evidence = if readiness_profile == WorkflowReadinessProfile::SoloCooperative
        {
            cooperative_evidence_audit(
                &projection.records,
                selected,
                cooperative_source_claim,
                active_cooperative_objective.as_ref(),
                &effective.identity().effective_runtime_bundle.bundle_digest,
                &snapshot_digest,
                now,
            )
        } else {
            Vec::new()
        };
        let cooperative_evidence_action_packet =
            active_cooperative_objective.as_ref().and_then(|objective| {
                (readiness_profile == WorkflowReadinessProfile::SoloCooperative)
                    .then(|| WorkflowCooperativeEvidenceBinding {
                        objective_id: objective.objective_id.clone(),
                        objective_revision: objective.revision,
                        objective_digest: objective.objective_digest.clone(),
                        assurance_epoch: objective.assurance_epoch,
                        accepted_objective_record_digest: objective.accepted_record_digest.clone(),
                        accepted_objective_record_sequence: objective.accepted_sequence,
                        policy_bundle_digest: effective
                            .identity()
                            .effective_runtime_bundle
                            .bundle_digest
                            .clone(),
                        snapshot_digest: snapshot_digest.clone(),
                        ledger_head_digest: assurance_source_head.clone(),
                        state_version: projection.current_state_version().unwrap_or_default(),
                    })
                    .and_then(|binding| {
                        cooperative_evidence_action_packet(
                            selected,
                            cooperative_source_claim?,
                            objective,
                            binding,
                        )
                    })
            });
        let cooperative_evidence_action_gap = (readiness_profile
            == WorkflowReadinessProfile::SoloCooperative
            && active_cooperative_objective.is_some()
            && cooperative_evidence_action_packet.is_none())
        .then(|| {
            format!(
                "selected policy {} has no current claim with a bound evaluator; cooperative evidence is default-denied and no route was published",
                selected.id.0
            )
        });
        let mut guidance = WorkflowGovernanceGuidance {
            authority: WorkflowGovernanceGuidanceAuthority::VerifiedProjectSnapshot,
            status: guidance_status,
            readiness_profile,
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
            active_cooperative_objective,
            cooperative_evidence,
            cooperative_evidence_action_packet,
            cooperative_evidence_action_gap,
            agent_autonomy,
            durable_assurance,
            authorization: WorkflowAuthorizationGuidance {
                registry_setup: WorkflowAuthorizationRegistrySetup {
                    principal_registry: registry_setup_status(trusted_registry_digest.as_deref()),
                    broker_registry: trusted_broker_registry.setup,
                },
                setup_gaps: Vec::new(),
                action_packets: Vec::new(),
                objective_management_packet: None,
            },
            replacement_continuity: None,
        };
        let (action_packets, objective_management_packet) =
            if readiness_profile == WorkflowReadinessProfile::SoloCooperative {
                let mut objective_packets = cooperative_objective_action_packets(&guidance)?;
                if guidance.active_cooperative_objective.is_some() {
                    (Vec::new(), objective_packets.pop())
                } else {
                    (objective_packets, None)
                }
            } else {
                (
                    authorization_action_packets(
                        effective.document(),
                        &guidance,
                        &derived,
                        Some(&assurance_facts),
                        trusted_registry_digest.clone(),
                        trusted_broker_registry.digest.clone(),
                    )?,
                    None,
                )
            };
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
        guidance.authorization.objective_management_packet = objective_management_packet;
        Ok((guidance, verified))
    }

    fn require_active_policy(
        &self,
        registry: &AdmittedWorkflowGovernanceReleaseRegistry,
        admitted: &AdmittedWorkflowGovernanceRelease,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        requested_policy_ref: &StableId,
        snapshot: &RetainedWorkflowProjectSnapshot,
    ) -> Result<ReadinessTarget, WorkflowGovernanceAdapterError> {
        let guidance = self.guidance_from_projection_with_snapshot(
            registry,
            admitted,
            effective,
            projection,
            unix_time()?,
            snapshot,
        )?;
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

    fn require_post_build_verify_gate(
        &self,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
        from: Phase,
        to: Phase,
        snapshot: &str,
        gate_kind: GateKind,
    ) -> Result<(), WorkflowGovernanceAdapterError> {
        if !self.phase_boundary_admitted(effective, projection, now, from, snapshot)? {
            return Err(WorkflowGovernanceAdapterError::PostBuildVerifyGateNotAdmitted);
        }
        let gates = [ProvidedGateResult {
            gate_kind,
            status: GateStatus::Pass,
        }];
        if evaluate_transition(&TransitionRequest {
            from,
            to,
            gates: &gates,
            waiver: None,
        }) != TransitionDecision::Allowed
        {
            return Err(WorkflowGovernanceAdapterError::PostBuildVerifyGateNotAdmitted);
        }
        Ok(())
    }

    fn phase_boundary_admitted(
        &self,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
        current_phase_value: Phase,
        snapshot: &str,
    ) -> Result<bool, WorkflowGovernanceAdapterError> {
        let trusted_registry_digest = self.current_trusted_registry_digest()?;
        let trusted_broker_registry_digest = self.current_trusted_broker_registry_state()?.digest;
        let derived = derive_receipts(
            effective.document(),
            projection,
            &self.binding.project_root,
            snapshot,
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
                    snapshot,
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
        Ok(phase_advance_allowed_by_assurance(
            governed_assurance.as_ref(),
            phase_done,
            assurance_is_enforced,
        ))
    }

    fn plan_phase_advance_with_snapshot(
        &self,
        effective: &AdmittedEffectiveWorkflowGovernanceBundle,
        projection: &WorkflowGovernanceLedgerProjection,
        now: u64,
        snapshot: &RetainedWorkflowProjectSnapshot,
    ) -> Result<Option<(u64, WorkflowGovernanceEvent)>, WorkflowGovernanceAdapterError> {
        let current = current_phase(projection)?;
        let Some(current_phase_value) = Phase::parse(&current.0) else {
            return Err(WorkflowGovernanceAdapterError::InvalidPhase(current.0));
        };
        snapshot.revalidate()?;
        if !self.phase_boundary_admitted(
            effective,
            projection,
            now,
            current_phase_value,
            snapshot.digest(),
        )? {
            return Ok(None);
        }
        let next = automatic_phase_successor(current_phase_value);
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
            snapshot_digest: snapshot.digest().to_owned(),
        });
        Ok(Some((state_version, event)))
    }
}

fn automatic_phase_successor(current: Phase) -> Option<Phase> {
    match current {
        Phase::Discovery => Some(Phase::Specification),
        Phase::Specification => Some(Phase::Plan),
        Phase::Plan => Some(Phase::BuildVerify),
        // Automatic P5c advancement ends at release readiness. Only the exact
        // C5.2 episode admission boundary may cross this point.
        _ => None,
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
    project_snapshot: RetainedWorkflowProjectSnapshot,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowLegacySoloAdoptionAvailability {
    Eligible,
    AlreadyAdopted,
    AlreadySolo,
    Ineligible,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowLegacyProfileStatus {
    pub current_profile: WorkflowReadinessProfile,
    pub legacy_profileless_genesis: bool,
    pub solo_adoption: WorkflowLegacySoloAdoptionAvailability,
    pub reason: &'static str,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adopt_solo_argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowLegacySoloAdoptionReceiptStatus {
    Adopted,
    AlreadyAdopted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowLegacySoloAdoptionReceipt {
    pub status: WorkflowLegacySoloAdoptionReceiptStatus,
    pub readiness_profile: WorkflowReadinessProfile,
    pub legacy_profileless_genesis: bool,
    pub provenance: WorkflowCooperativeAuthorityBasis,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_record: Option<WorkflowGovernanceLedgerRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceInitialization {
    pub status: WorkflowGovernanceInitializationStatus,
    pub readiness_profile: WorkflowReadinessProfile,
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
    pub readiness_profile: WorkflowReadinessProfile,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_cooperative_objective: Option<WorkflowActiveCooperativeObjective>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cooperative_evidence: Vec<WorkflowCooperativeEvidenceAudit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooperative_evidence_action_packet: Option<WorkflowCooperativeEvidenceActionPacket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooperative_evidence_action_gap: Option<String>,
    pub agent_autonomy: WorkflowAgentAutonomyGuidance,
    pub durable_assurance: WorkflowDurableAssuranceGuidance,
    pub authorization: WorkflowAuthorizationGuidance,
    /// Fresh-process continuation reconstructed only by `workflow resume`.
    ///
    /// Ordinary `workflow next` responses omit this additive block so every
    /// historical field and action packet retains its exact public shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_continuity: Option<WorkflowReplacementContinuity>,
}

pub const WORKFLOW_REPLACEMENT_CONTINUITY_SCHEMA_VERSION: &str =
    "workflow_replacement_continuity_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementContinuityStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementContinuityBinding {
    pub project_id: StableId,
    pub readiness_profile: WorkflowReadinessProfile,
    pub project_snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
    pub active_release_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_objective_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_objective_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_assurance_epoch: Option<u64>,
    pub claim_projection_digest: String,
    pub isolation_registry_digest: String,
    pub promotion_projection_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "authority_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowReplacementObjectiveRevision {
    CooperativeSameOwner {
        active: bool,
        record_digest: String,
        sequence: u64,
        state_version: u64,
        objective: WorkflowActiveCooperativeObjective,
    },
    HumanIntent {
        active: bool,
        record_digest: String,
        sequence: u64,
        state_version: u64,
        event: HumanIntentRevisionAcceptedEvent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementDecisionStatus {
    Unresolved,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementDecisionAudit {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub status: WorkflowReplacementDecisionStatus,
    pub need_record_digest: String,
    pub need_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_record_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_alternative_ref: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementEvidenceStatus {
    Admitted,
    Rejected,
    Expired,
    Revoked,
    HistoricalNotCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementEvidenceAudit {
    pub record_digest: String,
    pub sequence: u64,
    pub status: WorkflowReplacementEvidenceStatus,
    pub evidence: EvaluatorObservedEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementIsolationValidation {
    Valid,
    ProposedNotCreated,
    RetiredWorktreeAbsent,
    Missing,
    Mismatched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementIsolationAudit {
    pub contract_path: String,
    pub contract_digest: String,
    pub contract: IsolationContract,
    pub declared_worktree: String,
    pub validation: WorkflowReplacementIsolationValidation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<PromotionGitWorktreeBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gap_codes: Vec<WorkflowReplacementGapCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementPromotionStatus {
    NotStarted,
    Recoverable,
    Completed,
    BlockedCorrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementPromotionAudit {
    pub isolation_id: StableId,
    pub status: WorkflowReplacementPromotionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_argv: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementGapCode {
    IsolationRegistryInvalid,
    IsolationConflict,
    WorktreeMissing,
    GitWorktreeMismatch,
    LinkedClaimMissing,
    LinkedClaimOwnerMismatch,
    LinkedClaimExpired,
    LinkedClaimInactive,
    PromotionStateInvalid,
    PromotionRequiresSoloProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementGap {
    pub code: WorkflowReplacementGapCode,
    pub blocking: bool,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolation_id: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReplacementRankedActionKind {
    RecoverPromotion,
    ResolveContinuityGap,
    GovernedNext,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementRankedAction {
    pub rank: u32,
    pub kind: WorkflowReplacementRankedActionKind,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governed_action: Option<NextAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReplacementContinuity {
    pub schema_version: String,
    pub status: WorkflowReplacementContinuityStatus,
    pub binding: WorkflowReplacementContinuityBinding,
    pub objective_history: Vec<WorkflowReplacementObjectiveRevision>,
    /// Only decisions actually present as unresolved ledger events. Questions
    /// calculated by the current simulation remain under
    /// `simulation.candidate_decision_requests` and are never presented as
    /// recovered chat history.
    pub durable_pending_decisions: Vec<WorkflowReplacementDecisionAudit>,
    pub decision_history: Vec<WorkflowReplacementDecisionAudit>,
    pub governed_evidence: Vec<WorkflowReplacementEvidenceAudit>,
    pub cooperative_evidence: Vec<WorkflowCooperativeEvidenceAudit>,
    pub claims: Vec<ReplacementClaimProjection>,
    pub isolations: Vec<WorkflowReplacementIsolationAudit>,
    pub promotions: Vec<WorkflowReplacementPromotionAudit>,
    pub gaps: Vec<WorkflowReplacementGap>,
    pub ranked_next_actions: Vec<WorkflowReplacementRankedAction>,
    pub ranked_action_digest: String,
}

/// Honest ledger-derived readback for the same-owner cooperative objective.
///
/// The carrying identity and host coordinates are audit provenance only. This
/// type has no field that can imply a signature, verified issuer, human origin,
/// or independent identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowActiveCooperativeObjective {
    pub objective_id: StableId,
    pub revision: u64,
    pub assurance_epoch: u64,
    pub proposal: WorkflowCooperativeObjectiveProposal,
    pub objective_digest: String,
    pub previous_objective_digest: Option<String>,
    pub revision_kind: WorkflowCooperativeObjectiveRevisionKind,
    pub revision_reason: Option<String>,
    pub accepted_record_digest: String,
    pub accepted_sequence: u64,
    pub accepted_state_version: u64,
    pub snapshot_digest_at_acceptance: String,
    pub ledger_head_before_acceptance: String,
    pub acceptance_action_packet_digest: String,
    pub carrying_principal: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
    pub authority_basis: WorkflowCooperativeAuthorityBasis,
    pub accepted_at_unix: u64,
}

/// Read-only, durable audit projection. `proves` is intentionally narrow;
/// `does_not_prove` is explicit so consumers cannot relabel cooperative work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceAudit {
    pub record_digest: String,
    pub offer_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_cooperative_claim_ref: Option<StableId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub does_not_satisfy_source_claim_ref: Option<StableId>,
    pub historical_disposition: WorkflowCooperativeEvidenceDisposition,
    pub current_status: WorkflowCooperativeEvidenceCurrentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<WorkflowCooperativeEvidenceRejection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admitted_evidence: Option<WorkflowAdmittedCooperativeEvidence>,
    pub proves: Vec<WorkflowCooperativeEvidenceProof>,
    pub does_not_prove: Vec<WorkflowCooperativeEvidenceNonProof>,
}

/// Host-neutral instructions for one closed evidence offer. `argv` is an exact
/// vector; hosts must substitute the one input-file token, never parse a
/// display string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceActionPacket {
    pub argv: Vec<String>,
    pub input_file_token: String,
    pub input_file_must_be_outside_project_snapshot: bool,
    pub offer_schema_version: String,
    pub attestation_schema_version: String,
    pub maximum_input_bytes: usize,
    pub binding: WorkflowCooperativeEvidenceBinding,
    pub route: WorkflowCooperativeEvidenceRoute,
    pub offer_template: serde_json::Value,
    pub required_replacements: Vec<String>,
    pub kernel_derived_outcome: WorkflowEvidenceOutcome,
    pub readback_contract: String,
}
/// Result of one closed cooperative-objective input. The decision branch has
/// no workflow state fields because it validates the current packet and state
/// read-only and performs zero Forge writes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowCooperativeObjectiveAcceptance {
    DecisionRequired {
        decision_request: DecisionRequest,
    },
    Accepted {
        objective_record: WorkflowGovernanceLedgerRecord,
        active_objective: WorkflowActiveCooperativeObjective,
        next: Box<WorkflowGovernanceGuidance>,
    },
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
    RetainedProjectSnapshot(RetainedProjectTreeError),
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
    ReadinessProfileReconfiguration {
        current: WorkflowReadinessProfile,
        requested: WorkflowReadinessProfile,
    },
    LegacySoloAdoptionUnavailable(&'static str),
    LegacySoloAdoptionCasMismatch,
    LegacySoloAdoptionRetryConflict,
    CooperativeObjectiveProfileRequired,
    AgentAutonomyObjectiveRequired,
    AgentAutonomyEvaluation(AgentAutonomyEvaluationError),
    CooperativeObjectiveAlreadyAccepted,
    CooperativeObjectiveRetryConflict,
    StaleCooperativeObjectiveManagementPacket,
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
    PostBuildVerifyEpisodeBindingMismatch(&'static str),
    PostBuildVerifyEpisodeRouteInvalid,
    PostBuildVerifyGateNotAdmitted,
    CoordinationCasMismatch,
    CoordinationInvalid(String),
    ClaimProjection(String),
    PromotionPreview(super::promotion::PromotionPreviewError),
    PromotionApply(super::promotion::PromotionApplyError),
    ReplacementContinuityUnavailable(&'static str),
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
            Self::RetainedProjectSnapshot(error) => {
                write!(f, "retained project snapshot failed: {error}")
            }
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
            Self::ReadinessProfileReconfiguration { current, requested } => write!(
                f,
                "workflow readiness profile cannot be reconfigured from {} to {} after initialization",
                current.wire_name(),
                requested.wire_name()
            ),
            Self::LegacySoloAdoptionUnavailable(reason) => write!(
                f,
                "legacy Solo Cooperative adoption is unavailable: {reason}"
            ),
            Self::LegacySoloAdoptionCasMismatch => f.write_str(
                "legacy Solo Cooperative adoption is stale; refresh workflow profile status",
            ),
            Self::LegacySoloAdoptionRetryConflict => f.write_str(
                "legacy Solo Cooperative adoption retry conflicts with the durable transition",
            ),
            Self::CooperativeObjectiveProfileRequired => f.write_str(
                "cooperative objective admission requires the solo_cooperative readiness profile",
            ),
            Self::AgentAutonomyObjectiveRequired => f.write_str(
                "agent autonomy assessment requires an active cooperative objective",
            ),
            Self::AgentAutonomyEvaluation(error) => {
                write!(f, "agent autonomy assessment rejected: {error}")
            }
            Self::CooperativeObjectiveAlreadyAccepted => f.write_str(
                "an initial objective is already durable; refresh workflow next",
            ),
            Self::CooperativeObjectiveRetryConflict => f.write_str(
                "cooperative objective retry conflicts with the durably accepted payload",
            ),
            Self::StaleCooperativeObjectiveManagementPacket => f.write_str(
                "stale cooperative objective-management packet; run workflow next and retry with the current packet",
            ),
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
            Self::PostBuildVerifyEpisodeBindingMismatch(field) => write!(
                f,
                "post-BuildVerify episode does not match the exact current {field}"
            ),
            Self::PostBuildVerifyEpisodeRouteInvalid => f.write_str(
                "post-BuildVerify candidate does not describe a valid route for the current phase",
            ),
            Self::PostBuildVerifyGateNotAdmitted => f.write_str(
                "post-BuildVerify phase advancement is blocked by the admitted gate or current assurance boundary",
            ),
            Self::CoordinationCasMismatch => f.write_str(
                "coordination update CAS failed; refresh the ledger head and state version",
            ),
            Self::CoordinationInvalid(reason) => {
                write!(f, "coordination state is invalid: {reason}")
            }
            Self::ClaimProjection(reason) => {
                write!(f, "claim WAL projection failed: {reason}")
            }
            Self::PromotionPreview(error) => {
                write!(f, "governed promotion preview failed: {error}")
            }
            Self::PromotionApply(error) => {
                write!(f, "governed promotion apply failed: {error}")
            }
            Self::ReplacementContinuityUnavailable(reason) => {
                write!(f, "replacement continuity is unavailable: {reason}")
            }
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

impl From<AgentAutonomyEvaluationError> for WorkflowGovernanceAdapterError {
    fn from(value: AgentAutonomyEvaluationError) -> Self {
        Self::AgentAutonomyEvaluation(value)
    }
}

impl From<RetainedProjectTreeError> for WorkflowGovernanceAdapterError {
    fn from(value: RetainedProjectTreeError) -> Self {
        Self::RetainedProjectSnapshot(value)
    }
}

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

impl From<super::promotion::PromotionPreviewError> for WorkflowGovernanceAdapterError {
    fn from(value: super::promotion::PromotionPreviewError) -> Self {
        Self::PromotionPreview(value)
    }
}

impl From<super::promotion::PromotionApplyError> for WorkflowGovernanceAdapterError {
    fn from(value: super::promotion::PromotionApplyError) -> Self {
        Self::PromotionApply(value)
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
        WorkflowEvaluatorProvider::IndependentReviewer
        | WorkflowEvaluatorProvider::ResearchSource => {
            profile == WorkflowBrokerOriginProfile::Reviewer
        }
        WorkflowEvaluatorProvider::RepositoryInspector
        | WorkflowEvaluatorProvider::DeterministicTool
        | WorkflowEvaluatorProvider::RepresentativeRuntime
        | WorkflowEvaluatorProvider::ExternalAuthority => {
            profile == WorkflowBrokerOriginProfile::Runtime
        }
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
                    Some(DerivedReceiptTrustRoot::LocalPrincipalRegistry) => {
                        !matches!(
                            event.provider,
                            WorkflowEvaluatorProvider::ExternalAuthority
                                | WorkflowEvaluatorProvider::ResearchSource
                        ) && event.subject.kind != WorkflowEvidenceSubjectKind::ExternalSystem
                    }
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
            // Same-owner cooperative evidence has a dedicated audit lane. It
            // never becomes a policy receipt and therefore cannot satisfy the
            // selected source claim or promote governed assurance.
            WorkflowGovernanceEvent::CooperativeEvidenceObserved(_) => {}
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
            // Cooperative observations are never governed-assurance facts.
            WorkflowGovernanceEvent::CooperativeEvidenceObserved(_) => {}
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

fn project_replacement_continuity(
    projection: &WorkflowGovernanceLedgerProjection,
    claim_projection: &ClaimWalProjection,
    now: i64,
) -> Result<ReplacementContinuityProjection, WorkflowGovernanceAdapterError> {
    let head = projection
        .head_digest
        .clone()
        .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
    let state_version = projection.current_state_version().unwrap_or_default();
    let current_phase = current_phase(projection)?;
    let active_release = projected_active_release(projection).ok_or(
        WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
            "active release identity is absent",
        ),
    )?;

    let mut episodes_by_id = BTreeMap::new();
    let mut requests_by_id = BTreeMap::new();
    let mut completions_by_task_id = BTreeMap::new();
    let mut health_recovery_by_runtime_id = BTreeMap::new();
    let mut active_episode_id = None;
    for record in &projection.records {
        match &record.event {
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) => {
                if let Some(document) = event.episode_snapshot.as_ref() {
                    episodes_by_id.insert(
                        event.episode_id.0.clone(),
                        ReplacementEpisodeProjection {
                            document: document.clone(),
                            outcome: event.outcome,
                            from_phase: event.from_phase.clone(),
                            to_phase: event.to_phase.clone(),
                            decision_digest: event.decision_digest.clone(),
                            ledger_record_digest: record.record_digest.clone(),
                            state_version: record.state_version,
                        },
                    );
                    if event.release_subject == active_release {
                        active_episode_id = Some(event.episode_id.clone());
                    }
                }
            }
            WorkflowGovernanceEvent::CoordinationStateApplied(event) => match &event.state {
                CoordinationStateRecord::Request(state) => {
                    requests_by_id
                        .insert(state.request.request_contract.id.0.clone(), state.clone());
                }
                CoordinationStateRecord::Completion(state) => {
                    completions_by_task_id.insert(
                        state.completion.completion_contract.task.task_id.0.clone(),
                        state.clone(),
                    );
                }
                CoordinationStateRecord::HealthRecovery(state) => {
                    health_recovery_by_runtime_id.insert(
                        state
                            .recovery
                            .health_recovery_contract
                            .runtime
                            .agent_id
                            .0
                            .clone(),
                        state.clone(),
                    );
                }
            },
            _ => {}
        }
    }
    let active_episode_id = active_episode_id.ok_or(
        WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
            "no complete episode snapshot binds the active release",
        ),
    )?;
    let claims_by_id = claim_projection
        .latest_by_claim_id
        .iter()
        .map(|(id, projected)| {
            let liveness = if claim_projection.active_by_claim_id.contains_key(id) {
                if is_live(&projected.claim_contract, now) {
                    ReplacementClaimLiveness::Live
                } else {
                    ReplacementClaimLiveness::Expired
                }
            } else {
                ReplacementClaimLiveness::NonActive
            };
            (
                id.clone(),
                ReplacementClaimProjection {
                    claim: projected.claim_contract.clone(),
                    last_sequence: projected.last_seq,
                    liveness,
                },
            )
        })
        .collect();

    Ok(ReplacementContinuityProjection {
        ledger_head_digest: head,
        state_version,
        current_phase,
        active_release,
        active_episode_id,
        episodes_by_id,
        requests_by_id,
        completions_by_task_id,
        health_recovery_by_runtime_id,
        claims_by_id,
    })
}

fn coordination_reference_index(
    project_root: &Path,
) -> Result<ReferenceIndex, WorkflowGovernanceAdapterError> {
    let mut embedded_refs = forge_core_decisions::embedded_yaml_paths();
    embedded_refs.extend(
        forge_core_decisions::catalog::embedded_frozen_legacy_workflow_source_bytes()
            .into_iter()
            .map(|(path, _)| path.0),
    );
    ReferenceIndexBuilder::new()
        .with_known_embedded_refs(embedded_refs)
        .build(project_root)
        .map_err(|error| {
            WorkflowGovernanceAdapterError::CoordinationInvalid(format!(
                "coordination reference index failed: {error}"
            ))
        })
}

fn project_claim_wal_clean(
    state_root: &Path,
) -> Result<ClaimWalProjection, WorkflowGovernanceAdapterError> {
    project_claim_wal(
        state_root,
        &ClaimWalProjectionOptions {
            repair: false,
            stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
        },
    )
    .map_err(|error| WorkflowGovernanceAdapterError::ClaimProjection(error.to_string()))
}

fn exact_coordination_retry<'a>(
    projection: &'a WorkflowGovernanceLedgerProjection,
    state: &CoordinationStateRecord,
    expected_head: &str,
    expected_state_version: u64,
) -> Option<&'a WorkflowGovernanceLedgerRecord> {
    projection.records.iter().rev().find(|record| {
        matches!(
            &record.event,
            WorkflowGovernanceEvent::CoordinationStateApplied(event)
                if &event.state == state
                    && event.prior_ledger_head_digest == expected_head
                    && event.prior_state_version == expected_state_version
        )
    })
}

fn validate_coordination_kernel_state(
    state: &CoordinationStateRecord,
    ledger: &WorkflowGovernanceLedgerProjection,
    claims: &ClaimWalProjection,
    reference_index: &ReferenceIndex,
    state_version: u64,
    now: i64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    match state {
        CoordinationStateRecord::Request(request) => {
            let report = validate_request(&request.request);
            if report.has_errors() {
                return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                    format!(
                        "request contract validation failed: {:?}",
                        report.diagnostics()
                    ),
                ));
            }
            validate_request_coordination(request, ledger, claims, reference_index, now)
        }
        CoordinationStateRecord::Completion(completion) => {
            let report = validate_completion(&completion.completion);
            if report.has_errors() {
                return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                    format!(
                        "completion contract validation failed: {:?}",
                        report.diagnostics()
                    ),
                ));
            }
            validate_completion_coordination(completion, ledger, claims, state_version, now)
        }
        CoordinationStateRecord::HealthRecovery(recovery) => {
            let report = validate_health_recovery(&recovery.recovery);
            if report.has_errors() {
                return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                    format!(
                        "health-recovery contract validation failed: {:?}",
                        report.diagnostics()
                    ),
                ));
            }
            validate_recovery_coordination(recovery, ledger, claims, now)
        }
    }
}

fn latest_request_by_reference<'a>(
    projection: &'a WorkflowGovernanceLedgerProjection,
    reference: &str,
) -> Option<&'a CoordinationRequestState> {
    projection.records.iter().rev().find_map(|record| {
        let WorkflowGovernanceEvent::CoordinationStateApplied(event) = &record.event else {
            return None;
        };
        let CoordinationStateRecord::Request(state) = &event.state else {
            return None;
        };
        let request = &state.request.request_contract;
        (request.id.0 == reference).then_some(state)
    })
}

fn active_claim_by_reference<'a>(
    claims: &'a ClaimWalProjection,
    reference: &str,
) -> Option<&'a ClaimContract> {
    claims
        .active_by_claim_id
        .get(reference)
        .map(|projected| &projected.claim_contract)
}

fn validate_request_coordination(
    state: &CoordinationRequestState,
    ledger: &WorkflowGovernanceLedgerProjection,
    claims: &ClaimWalProjection,
    reference_index: &ReferenceIndex,
    now: i64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let request = &state.request.request_contract;
    for dependency in &request.payload.dependency_refs {
        if dependency.reference.trim().is_empty() {
            return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                "request dependency reference is blank".to_owned(),
            ));
        }
        match dependency.kind {
            DependencyKind::Request => {
                if latest_request_by_reference(ledger, &dependency.reference).is_none() {
                    return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                        format!("request dependency {} is not durable", dependency.reference),
                    ));
                }
            }
            DependencyKind::Claim => {
                let claim =
                    active_claim_by_reference(claims, &dependency.reference).ok_or_else(|| {
                        WorkflowGovernanceAdapterError::CoordinationInvalid(format!(
                            "claim dependency {} is not active",
                            dependency.reference
                        ))
                    })?;
                if !is_live(claim, now) {
                    return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                        format!("claim dependency {} is expired", dependency.reference),
                    ));
                }
            }
            DependencyKind::Gate
            | DependencyKind::Effect
            | DependencyKind::RuntimeHandoff
            | DependencyKind::Decision => {
                let expected = match dependency.kind {
                    DependencyKind::Gate => ReferenceKind::GateContract,
                    DependencyKind::Effect => ReferenceKind::ToolEffectContract,
                    DependencyKind::RuntimeHandoff => ReferenceKind::RuntimeHandoffContract,
                    DependencyKind::Decision => ReferenceKind::DecisionCloseContract,
                    DependencyKind::Request | DependencyKind::Claim => unreachable!(),
                };
                if reference_index.kind_of(&dependency.reference) != Some(expected) {
                    return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                        format!(
                            "request dependency {} does not resolve as {:?}",
                            dependency.reference, expected
                        ),
                    ));
                }
            }
        }
    }

    let deadline = request
        .deadline
        .as_ref()
        .or(request.response.deadline.as_ref());
    let deadline_unix = deadline
        .map(|value| {
            rfc3339_to_unix(value).ok_or_else(|| {
                WorkflowGovernanceAdapterError::CoordinationInvalid(
                    "request deadline is not strict RFC3339 UTC".to_owned(),
                )
            })
        })
        .transpose()?;
    if request.status == RequestStatus::Expired {
        if deadline_unix.is_none_or(|deadline| now < deadline) {
            return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                "request cannot expire before its durable deadline".to_owned(),
            ));
        }
    } else if matches!(
        request.status,
        RequestStatus::Pending | RequestStatus::Accepted
    ) && deadline_unix.is_some_and(|deadline| now >= deadline)
    {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "past-deadline request must be recorded as expired".to_owned(),
        ));
    }

    if request.status != RequestStatus::Pending {
        if request.status != RequestStatus::Accepted
            && !request.response.allowed_statuses.contains(&request.status)
        {
            return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                "request transition status is not allowed by its response contract".to_owned(),
            ));
        }
        if request.response.required
            && request
                .response
                .required_evidence_refs
                .iter()
                .any(|required| {
                    !state
                        .response_evidence_refs
                        .iter()
                        .any(|found| found == required)
                })
        {
            return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                "request response is missing required evidence".to_owned(),
            ));
        }
    }

    if let Some(handoff) = state.mutation_handoff.as_ref() {
        validate_request_mutation_handoff(
            handoff,
            request.target_driver.0.as_str(),
            claims,
            reference_index,
            now,
        )?;
    }
    Ok(())
}

fn validate_request_mutation_handoff(
    handoff: &CoordinationMutationHandoff,
    target_driver: &str,
    claims: &ClaimWalProjection,
    reference_index: &ReferenceIndex,
    now: i64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let claim =
        active_claim_by_reference(claims, &handoff.claim_contract_ref.0).ok_or_else(|| {
            WorkflowGovernanceAdapterError::CoordinationInvalid(
                "mutation handoff does not reference an active claim".to_owned(),
            )
        })?;
    if !is_live(claim, now) || claim.claim.claimant_agent_id.0 != target_driver {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "mutation handoff claim is expired or owned by another agent".to_owned(),
        ));
    }
    if let Some(reference) = handoff.effect_contract_refs.iter().find(|reference| {
        reference_index.kind_of(reference) != Some(ReferenceKind::ToolEffectContract)
    }) {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            format!("mutation handoff effect {reference} is not an exact ToolEffectContract"),
        ));
    }
    Ok(())
}

fn validate_completion_coordination(
    state: &CoordinationCompletionState,
    ledger: &WorkflowGovernanceLedgerProjection,
    claims: &ClaimWalProjection,
    state_version: u64,
    now: i64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let completion = &state.completion.completion_contract;
    if completion.status.checked_at_state_version != state_version {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "completion proof was checked against a stale state version".to_owned(),
        ));
    }
    if completion.status.value == CompletionStatus::Invalidated
        || completion.invalidation.invalidated_by.is_some()
        || completion.invalidation.invalidated_at.is_some()
        || completion.invalidation.reason_code.is_some()
    {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "invalidated completion cannot be applied".to_owned(),
        ));
    }
    if ledger.records.iter().any(|record| {
        matches!(
            &record.event,
            WorkflowGovernanceEvent::CoordinationStateApplied(event)
                if matches!(
                    &event.state,
                    CoordinationStateRecord::Completion(previous)
                        if previous.completion.completion_contract.task.task_id == completion.task.task_id
                            && previous != state
                )
        )
    }) {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "task already has a conflicting durable completion".to_owned(),
        ));
    }
    let claim_ref = completion
        .claim
        .claim_contract_ref
        .as_ref()
        .ok_or_else(|| {
            WorkflowGovernanceAdapterError::CoordinationInvalid(
                "completion is missing its claim reference".to_owned(),
            )
        })?;
    let claim = active_claim_by_reference(claims, &claim_ref.0).ok_or_else(|| {
        WorkflowGovernanceAdapterError::CoordinationInvalid(
            "completion claim is not active".to_owned(),
        )
    })?;
    if claim.id.0 != state.applied_claim_id.0
        || !is_live(claim, now)
        || completion.claim.claim_expires_at.as_deref() != Some(claim.lease.expires_at.as_str())
        || completion.status.changed_by != claim.claim.claimant_agent_id
    {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "completion does not match the exact live claimant and lease".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recovery_coordination(
    state: &CoordinationHealthRecoveryState,
    ledger: &WorkflowGovernanceLedgerProjection,
    claims: &ClaimWalProjection,
    now: i64,
) -> Result<(), WorkflowGovernanceAdapterError> {
    let recovery = &state.recovery.health_recovery_contract;
    let request = recovery
        .recovery
        .request_ref
        .as_ref()
        .map(|reference| latest_request_by_reference(ledger, &reference.0))
        .transpose_option("health recovery request is not durable")?;
    let claim = recovery
        .recovery
        .claim_ref
        .as_ref()
        .map(|reference| active_claim_by_reference(claims, &reference.0))
        .transpose_option("health recovery claim is not active")?;
    if claim.is_some_and(|claim| !is_live(claim, now)) {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "health recovery claim is expired".to_owned(),
        ));
    }
    if matches!(
        recovery.status,
        HealthStatus::Stalled | HealthStatus::Crashed
    ) && recovery.recovery.automatic_allowed
    {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "stalled or crashed runtime cannot be silently reassigned".to_owned(),
        ));
    }
    if matches!(
        recovery.recovery.action,
        RecoveryAction::HandoffToDriver | RecoveryAction::ReclaimAfterReview
    ) && (request.is_none() || claim.is_none())
    {
        return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
            "reviewed handoff/reclaim requires exact durable request and claim references"
                .to_owned(),
        ));
    }
    if let Some(request) = request {
        if state.actor_agent_id != request.request.request_contract.target_driver {
            return Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                "health recovery actor is not the request target driver".to_owned(),
            ));
        }
    }
    Ok(())
}

trait TransposeCoordinationOption<T> {
    fn transpose_option(
        self,
        message: &'static str,
    ) -> Result<Option<T>, WorkflowGovernanceAdapterError>;
}

impl<T> TransposeCoordinationOption<T> for Option<Option<T>> {
    fn transpose_option(
        self,
        message: &'static str,
    ) -> Result<Option<T>, WorkflowGovernanceAdapterError> {
        match self {
            Some(Some(value)) => Ok(Some(value)),
            Some(None) => Err(WorkflowGovernanceAdapterError::CoordinationInvalid(
                message.to_owned(),
            )),
            None => Ok(None),
        }
    }
}

fn legacy_solo_adoption_availability(
    projection: &WorkflowGovernanceLedgerProjection,
) -> Result<(WorkflowLegacySoloAdoptionAvailability, &'static str), WorkflowGovernanceAdapterError>
{
    let genesis = projection
        .records
        .first()
        .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
    let WorkflowGovernanceEvent::ProjectImported(imported) = &genesis.event else {
        return Err(WorkflowGovernanceAdapterError::LedgerUninitialized);
    };
    if projection.contains_legacy_solo_adoption() {
        return Ok((
            WorkflowLegacySoloAdoptionAvailability::AlreadyAdopted,
            "this legacy project already adopted Solo Cooperative",
        ));
    }
    if imported.readiness_profile == Some(WorkflowReadinessProfile::SoloCooperative) {
        return Ok((
            WorkflowLegacySoloAdoptionAvailability::AlreadySolo,
            "this project already started in Solo Cooperative mode; no adoption is needed",
        ));
    }
    if imported.readiness_profile.is_some() {
        return Ok((
            WorkflowLegacySoloAdoptionAvailability::Ineligible,
            "genesis already selected the explicit strict_external readiness profile",
        ));
    }
    if projection.readiness_profile() != Some(WorkflowReadinessProfile::StrictExternal) {
        return Ok((
            WorkflowLegacySoloAdoptionAvailability::Ineligible,
            "legacy profile is not in its strict-compatible starting state",
        ));
    }
    if projection.records.iter().any(|record| {
        !matches!(
            record.event,
            WorkflowGovernanceEvent::ProjectImported(_)
                | WorkflowGovernanceEvent::ReleaseUpgraded(_)
        )
    }) {
        return Ok((
            WorkflowLegacySoloAdoptionAvailability::Ineligible,
            "legacy history already contains workflow decisions, evidence, coordination, claims, or authority-bearing events",
        ));
    }
    Ok((
        WorkflowLegacySoloAdoptionAvailability::Eligible,
        "profile-less legacy history contains only project import and release upgrades",
    ))
}

fn profile_status_projection(
    project_root: &Path,
    projection: &WorkflowGovernanceLedgerProjection,
    snapshot_digest: String,
) -> Result<WorkflowLegacyProfileStatus, WorkflowGovernanceAdapterError> {
    let head = projection
        .head_digest
        .clone()
        .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
    let genesis = projection
        .records
        .first()
        .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
    let WorkflowGovernanceEvent::ProjectImported(imported) = &genesis.event else {
        return Err(WorkflowGovernanceAdapterError::LedgerUninitialized);
    };
    let current_profile = projection
        .readiness_profile()
        .ok_or(WorkflowGovernanceAdapterError::LedgerUninitialized)?;
    let (solo_adoption, reason) = legacy_solo_adoption_availability(projection)?;
    let adopt_solo_argv =
        (solo_adoption == WorkflowLegacySoloAdoptionAvailability::Eligible).then(|| {
            vec![
                "forge-core".to_owned(),
                "workflow".to_owned(),
                "profile".to_owned(),
                "adopt-solo".to_owned(),
                "--root".to_owned(),
                project_root.display().to_string(),
                "--expected-head-digest".to_owned(),
                head.clone(),
                "--expected-snapshot-digest".to_owned(),
                snapshot_digest.clone(),
                "--json".to_owned(),
            ]
        });
    Ok(WorkflowLegacyProfileStatus {
        current_profile,
        legacy_profileless_genesis: imported.readiness_profile.is_none(),
        solo_adoption,
        reason,
        snapshot_digest,
        ledger_head_digest: head,
        state_version: projection.current_state_version().unwrap_or_default(),
        adopt_solo_argv,
    })
}

fn projected_active_release(
    projection: &WorkflowGovernanceLedgerProjection,
) -> Option<WorkflowGovernanceReleaseIdentity> {
    projection
        .records
        .iter()
        .rev()
        .find_map(|record| match &record.event {
            WorkflowGovernanceEvent::ReleaseUpgraded(event) => Some(event.to_release.clone()),
            WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                Some(event.release_transition.to_release.clone())
            }
            _ => None,
        })
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
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) => {
                if let Some(to_phase) = event.to_phase.as_ref() {
                    phase = Some(to_phase.clone());
                }
            }
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
        WorkflowAuthorizationApprovalBoundary::CooperativeSameOwner => false,
        WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker => {
            audit.issuer_profile == WorkflowBrokerIssuerProfile::Reviewer
        }
        WorkflowAuthorizationApprovalBoundary::TrustedRuntimeBroker
        | WorkflowAuthorizationApprovalBoundary::ExternalAuthorityBroker => {
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
    native_interaction_replay_digest: Option<&str>,
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
        native_interaction_replay_digest: native_interaction_replay_digest.map(str::to_owned),
        issued_at_unix: audit.issued_at_unix,
        expires_at_unix: audit.expires_at_unix,
        native_host_provenance: audit.native_host_provenance.clone(),
    }
}

fn broker_native_replay_tuple_matches(
    origin: &BrokerOriginAppliedEvent,
    audit: &VerifiedWorkflowBrokerEventAudit,
) -> bool {
    match (
        origin.native_host_provenance.as_ref(),
        audit.native_host_provenance.as_ref(),
    ) {
        (Some(origin_provenance), Some(audit_provenance)) => {
            origin.issuer_id == audit.issuer_id
                && origin_provenance.host_kind == audit_provenance.host_kind
                && origin_provenance.adapter_id == audit_provenance.adapter_id
                && origin_provenance.host_event_ref == audit_provenance.host_event_ref
                && origin_provenance.host_session_ref == audit_provenance.host_session_ref
                && origin_provenance.host_interaction_ref == audit_provenance.host_interaction_ref
        }
        _ => false,
    }
}
fn matching_broker_origin_retry(
    projection: &WorkflowGovernanceLedgerProjection,
    audit: &VerifiedWorkflowBrokerEventAudit,
    admitted_registry_digest: Option<&str>,
    native_interaction_replay_digest: Option<&str>,
) -> Result<
    Option<(
        WorkflowGovernanceLedgerRecord,
        WorkflowGovernanceLedgerRecord,
    )>,
    WorkflowGovernanceAdapterError,
> {
    let strict_replay_digest = match (admitted_registry_digest, native_interaction_replay_digest) {
        (Some(_), Some(replay_digest)) => Some(replay_digest),
        (None, None) => None,
        _ => return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch),
    };
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
        let native_tuple_matches = broker_native_replay_tuple_matches(origin, audit);
        let stable_replay_matches = native_interaction_replay_digest.is_some_and(|expected| {
            origin.native_interaction_replay_digest.as_deref() == Some(expected)
        });
        if !packet_matches
            && !event_matches
            && !origin_identity_matches
            && !native_tuple_matches
            && !stable_replay_matches
        {
            continue;
        }
        let profile = match audit.issuer_profile {
            WorkflowBrokerIssuerProfile::Human => WorkflowBrokerOriginProfile::Human,
            WorkflowBrokerIssuerProfile::Reviewer => WorkflowBrokerOriginProfile::Reviewer,
            WorkflowBrokerIssuerProfile::Runtime => WorkflowBrokerOriginProfile::Runtime,
        };
        // The durable companion keeps the registry digest that originally
        // admitted the event. An exact response-loss retry may be reverified by
        // a later registry generation that retains the historical credential,
        // so rotation-stable replay identity—not the caller's current registry
        // digest—joins the otherwise exact signed audit coordinates.
        let strict_binding_matches = strict_replay_digest.map_or_else(
            || origin.native_interaction_replay_digest.is_none(),
            |replay_digest| {
                origin.native_interaction_replay_digest.as_deref() == Some(replay_digest)
            },
        );
        // Revocation necessarily changes the retained credential metadata digest.
        // Strict recovery has already revalidated that retained history and is
        // joined to the durable origin by the stable native replay digest; the
        // frozen legacy path still requires its exact enrollment digest.
        let credential_history_matches = strict_replay_digest.is_some()
            || origin.enrollment_ceremony_digest == audit.enrollment_ceremony_digest;
        let exact_match = packet_matches
            && event_matches
            && origin.origin_principal_id == audit.origin_principal_id
            && origin.separation_domain == audit.separation_domain
            && origin.nonce_fingerprint == audit.replay_key.nonce_fingerprint
            && origin.issuer_id == audit.issuer_id
            && origin.issuer_profile == profile
            && origin.public_key_fingerprint == audit.public_key_fingerprint
            && origin.signature_fingerprint == audit.signature_fingerprint
            && credential_history_matches
            && origin.native_host_provenance == audit.native_host_provenance
            && origin.issued_at_unix == audit.issued_at_unix
            && origin.expires_at_unix == audit.expires_at_unix
            && strict_binding_matches;
        if !exact_match {
            if stable_replay_matches || native_tuple_matches {
                let replay_origin_id = native_interaction_replay_digest
                    .map(str::to_owned)
                    .map_or_else(|| broker_replay_origin_id(audit), Ok)?;
                return Err(WorkflowGovernanceAdapterError::ActionReplay(
                    WorkflowActionReplayError::OriginReplayConflict {
                        origin_event_id_hash: workflow_action_replay_origin_fingerprint(
                            &replay_origin_id,
                        )?,
                    },
                ));
            }
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
    let identity = if let Some(provenance) = audit.native_host_provenance.as_ref() {
        serde_json::json!({
            "schema_version": "workflow_broker_replay_origin_v2",
            "issuer_id": audit.issuer_id,
            "host_kind": provenance.host_kind,
            "adapter_id": provenance.adapter_id,
            "host_event_ref": provenance.host_event_ref,
            "host_session_ref": provenance.host_session_ref,
            "host_interaction_ref": provenance.host_interaction_ref,
        })
    } else {
        serde_json::json!({
            "schema_version": "workflow_broker_replay_origin_v1",
            "issuer_id": audit.issuer_id,
            "nonce_fingerprint": audit.replay_key.nonce_fingerprint,
            "origin_principal_id": audit.origin_principal_id,
            "separation_domain": audit.separation_domain,
        })
    };
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
    let mutation = begin_workflow_action_replay_reservation(
        state_root,
        packet_digest,
        replay_origin_id,
        action_record_digest,
    )?
    .commit_after_authoritative_ledger()?;
    Ok(mutation.appended)
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
        WorkflowAuthorizationInputContract::CooperativeObjective { .. }
        | WorkflowAuthorizationInputContract::IntentRevision { .. } => {
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

fn selected_cooperative_source_claim<'a>(
    policy: &'a WorkflowGovernancePolicy,
    simulation: &WorkflowGovernanceSimulation,
) -> Option<&'a forge_core_contracts::WorkflowClaimPolicy> {
    policy
        .claims
        .iter()
        .find(|claim| {
            !simulation
                .candidate_claim_results
                .iter()
                .find(|result| result.claim_id == claim.id.0)
                .is_some_and(|result| {
                    matches!(
                        result.status,
                        WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                    )
                })
        })
        .or_else(|| policy.claims.first())
}

fn cooperative_evidence_audit(
    records: &[WorkflowGovernanceLedgerRecord],
    selected_policy: &WorkflowGovernancePolicy,
    selected_claim: Option<&forge_core_contracts::WorkflowClaimPolicy>,
    active_objective: Option<&WorkflowActiveCooperativeObjective>,
    effective_bundle_digest: &str,
    snapshot_digest: &str,
    now: u64,
) -> Vec<WorkflowCooperativeEvidenceAudit> {
    let current_route = active_objective.and_then(|objective| {
        derived_solo_cooperative_evidence_route_for_policy(
            selected_policy,
            selected_claim?,
            objective,
        )
    });
    records
        .iter()
        .filter_map(|record| {
            let WorkflowGovernanceEvent::CooperativeEvidenceObserved(event) = &record.event else {
                return None;
            };
            let current_status = if event.disposition
                == WorkflowCooperativeEvidenceDisposition::Rejected
            {
                WorkflowCooperativeEvidenceCurrentStatus::Rejected
            } else if event.admitted_evidence.as_ref().is_some_and(|admitted| {
                let objective_current = active_objective.is_some_and(|objective| {
                    admitted.binding.objective_id == objective.objective_id
                        && admitted.binding.objective_revision == objective.revision
                        && admitted.binding.objective_digest == objective.objective_digest
                        && admitted.binding.assurance_epoch == objective.assurance_epoch
                        && admitted.binding.accepted_objective_record_digest
                            == objective.accepted_record_digest
                        && admitted.binding.accepted_objective_record_sequence
                            == objective.accepted_sequence
                });
                current_route.as_ref().is_some_and(|route| {
                    objective_current
                        && event.offer_id.as_ref() == Some(&admitted.offer_id)
                        && admitted.offer_digest == event.offer_digest
                        && event.rejection.is_none()
                        && record.previous_record_digest.as_deref()
                            == Some(event.admission_ledger_head_digest.as_str())
                        && record.state_version == event.admission_state_version
                        && event.admission_snapshot_digest == snapshot_digest
                        && event.admission_snapshot_digest == admitted.binding.snapshot_digest
                        && admitted.binding.policy_bundle_digest == effective_bundle_digest
                        && admitted.policy_version == route.policy_version
                        && admitted.claim_descriptor_version == route.claim_descriptor_version
                        && admitted.policy_ref == route.policy_ref
                        && admitted.claim_ref == route.claim_ref
                        && admitted.evaluator_ref == route.evaluator_ref
                        && admitted.cooperative_claim_ref == route.cooperative_claim_ref
                        && admitted.cooperative_evaluator_ref == route.cooperative_evaluator_ref
                        && admitted.producer == route.producer
                        && admitted.subject.kind == WorkflowEvidenceSubjectKind::ProjectSnapshot
                        && route.allowed_subject_kinds.contains(&admitted.subject.kind)
                        && admitted.subject.subject_ref == route.subject_ref
                        && admitted.subject.subject_digest == snapshot_digest
                        && admitted.scenario_kind
                            == WorkflowCooperativeMaterialScenarioKind::KernelProjectSnapshotReadback
                        && admitted.scenario_digest == route.scenario_digest
                        && admitted.outcome == WorkflowEvidenceOutcome::Pass
                        && admitted.execution_observed_at_unix <= now
                        && admitted.readback_observed_at_unix <= now
                        && admitted.readback_observed_at_unix == event.observed_at_unix
                        && admitted.readback_observed_at_unix >= admitted.execution_observed_at_unix
                        && route.assurance_effect
                            == WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim
                        && route.provider == WorkflowEvaluatorProvider::RepositoryInspector
                        && route.kind == WorkflowEvidenceKind::ArtifactInspection
                        && route.strength == WorkflowEvidenceStrength::InspectedArtifact
                        && route.claim_descriptor_version
                            == SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
                        && {
                        now.saturating_sub(admitted.readback_observed_at_unix)
                            <= route.max_age_seconds
                            && admitted
                                .readback_observed_at_unix
                                .saturating_sub(admitted.execution_observed_at_unix)
                                <= route.max_age_seconds
                        }
                })
            }) {
                WorkflowCooperativeEvidenceCurrentStatus::Supporting
            } else {
                WorkflowCooperativeEvidenceCurrentStatus::Stale
            };
            let admitted = event.admitted_evidence.as_ref();
            let mut does_not_prove = vec![
                WorkflowCooperativeEvidenceNonProof::IndependentSemanticReview,
                WorkflowCooperativeEvidenceNonProof::TrustedRuntimeSeparation,
                WorkflowCooperativeEvidenceNonProof::TamperResistance,
                WorkflowCooperativeEvidenceNonProof::HumanPresence,
                WorkflowCooperativeEvidenceNonProof::EnterpriseCompliance,
                WorkflowCooperativeEvidenceNonProof::SelectedSourceClaim,
            ];
            if current_route.as_ref().is_some_and(|route| {
                route.source_provider == WorkflowEvaluatorProvider::RepresentativeRuntime
            }) {
                does_not_prove.push(
                    WorkflowCooperativeEvidenceNonProof::SelectedRepresentativeRuntimeClaim,
                );
            }
            Some(WorkflowCooperativeEvidenceAudit {
                record_digest: record.record_digest.clone(),
                offer_digest: event.offer_digest.clone(),
                supports_cooperative_claim_ref: matches!(
                    current_status,
                    WorkflowCooperativeEvidenceCurrentStatus::Supporting
                )
                .then(|| admitted.map(|evidence| evidence.cooperative_claim_ref.clone()))
                .flatten(),
                does_not_satisfy_source_claim_ref: admitted
                    .map(|evidence| evidence.claim_ref.clone()),
                historical_disposition: event.disposition,
                current_status,
                rejection: event.rejection,
                admitted_evidence: event.admitted_evidence.clone(),
                proves: if current_status == WorkflowCooperativeEvidenceCurrentStatus::Supporting {
                    vec![
                        WorkflowCooperativeEvidenceProof::SoloCooperativeClaimSatisfied,
                        WorkflowCooperativeEvidenceProof::KernelExecutedProjectSnapshotScenario,
                        WorkflowCooperativeEvidenceProof::KernelVerifiedProjectStateReadback,
                    ]
                } else {
                    Vec::new()
                },
                does_not_prove,
            })
        })
        .collect()
}
fn derived_solo_cooperative_evidence_route(
    selected_policy: &WorkflowGovernancePolicy,
    selected_claim: &forge_core_contracts::WorkflowClaimPolicy,
    offer: &WorkflowCooperativeEvidenceOffer,
    objective: &WorkflowActiveCooperativeObjective,
) -> Option<WorkflowCooperativeEvidenceRoute> {
    let statement = &offer.attestation;
    let route = derived_solo_cooperative_evidence_route_for_policy(
        selected_policy,
        selected_claim,
        objective,
    )?;
    (route.policy_ref == statement.policy_ref
        && route.claim_ref == statement.claim_ref
        && route.evaluator_ref == statement.evaluator_ref
        && route.cooperative_claim_ref == statement.cooperative_claim_ref
        && route.cooperative_evaluator_ref == statement.cooperative_evaluator_ref)
        .then_some(route)
}

fn derived_solo_cooperative_evidence_route_for_policy(
    policy: &WorkflowGovernancePolicy,
    claim: &forge_core_contracts::WorkflowClaimPolicy,
    objective: &WorkflowActiveCooperativeObjective,
) -> Option<WorkflowCooperativeEvidenceRoute> {
    let claim = policy
        .claims
        .iter()
        .find(|candidate| candidate.id == claim.id)?;
    let evaluator = policy
        .evaluators
        .iter()
        .find(|evaluator| evaluator.id == claim.evaluator_ref)?;
    let source_provider = serde_json::to_string(&evaluator.provider).ok()?;
    let scenario_material = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION,
        SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION,
        objective.objective_digest,
        policy.id.0,
        claim.id.0,
        evaluator.id.0,
        source_provider,
    );
    let scenario_digest = sha256_content_hash(scenario_material.as_bytes());
    let descriptor_key = &scenario_digest[7..23];
    Some(WorkflowCooperativeEvidenceRoute {
        policy_version: SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION.to_owned(),
        claim_descriptor_version: SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION.to_owned(),
        policy_ref: policy.id.clone(),
        claim_ref: claim.id.clone(),
        evaluator_ref: evaluator.id.clone(),
        source_provider: evaluator.provider,
        cooperative_claim_ref: StableId(format!(
            "claim.solo-cooperative.project-snapshot.{descriptor_key}"
        )),
        cooperative_evaluator_ref: StableId(format!(
            "evaluator.solo-cooperative.kernel-project-snapshot.{descriptor_key}"
        )),
        producer: objective.carrying_principal.clone(),
        provider: WorkflowEvaluatorProvider::RepositoryInspector,
        kind: WorkflowEvidenceKind::ArtifactInspection,
        strength: WorkflowEvidenceStrength::InspectedArtifact,
        allowed_subject_kinds: vec![WorkflowEvidenceSubjectKind::ProjectSnapshot],
        subject_ref: "project.current_snapshot".to_owned(),
        scenario_digest,
        max_age_seconds: evaluator.max_age_seconds,
        assurance_effect:
            WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim,
    })
}

fn cooperative_evidence_action_packet(
    selected_policy: &WorkflowGovernancePolicy,
    selected_claim: &forge_core_contracts::WorkflowClaimPolicy,
    objective: &WorkflowActiveCooperativeObjective,
    binding: WorkflowCooperativeEvidenceBinding,
) -> Option<WorkflowCooperativeEvidenceActionPacket> {
    let route = derived_solo_cooperative_evidence_route_for_policy(
        selected_policy,
        selected_claim,
        objective,
    )?;
    let input_file_token = "${FORGE_COOPERATIVE_EVIDENCE_INPUT_FILE}".to_owned();
    let offer_template = serde_json::json!({
        "schema_version": COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION,
        "offer_id": "${UNIQUE_OFFER_ID}",
        "attestation": {
            "schema_version": COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION,
            "policy_version": route.policy_version.clone(),
            "claim_descriptor_version": route.claim_descriptor_version.clone(),
            "binding": binding.clone(),
            "policy_ref": route.policy_ref.clone(),
            "claim_ref": route.claim_ref.clone(),
            "evaluator_ref": route.evaluator_ref.clone(),
            "cooperative_claim_ref": route.cooperative_claim_ref.clone(),
            "cooperative_evaluator_ref": route.cooperative_evaluator_ref.clone(),
            "producer": route.producer.clone(),
            "subject": {
                "kind": "project_snapshot",
                "subject_ref": route.subject_ref.clone(),
                "subject_digest": binding.snapshot_digest.clone(),
            },
            "scenario_kind": "kernel_project_snapshot_readback",
            "scenario_digest": route.scenario_digest.clone(),
        }
    });
    Some(WorkflowCooperativeEvidenceActionPacket {
        argv: vec![
            "forge-core".to_owned(),
            "workflow".to_owned(),
            "evidence".to_owned(),
            "admit-cooperative".to_owned(),
            "--root".to_owned(),
            ".".to_owned(),
            "--input-file".to_owned(),
            input_file_token.clone(),
            "--json".to_owned(),
        ],
        input_file_token,
        input_file_must_be_outside_project_snapshot: true,
        offer_schema_version: COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION.to_owned(),
        attestation_schema_version: COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION.to_owned(),
        maximum_input_bytes: MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES,
        binding,
        route,
        offer_template,
        required_replacements: vec!["${UNIQUE_OFFER_ID}".to_owned()],
        kernel_derived_outcome: WorkflowEvidenceOutcome::Pass,
        readback_contract: "kernel_executes_the_versioned_project_snapshot_scenario_and_recomputes_current_snapshot_readback; the result supports_only_the_derived_solo_cooperative_claim; the_selected_source_claim_remains_unsatisfied_including_representative_runtime; runtime_external_system_and_human_decision_subjects_are_rejected"
            .to_owned(),
    })
}

fn cooperative_offer_text_is_bounded(offer: &WorkflowCooperativeEvidenceOffer) -> bool {
    let statement = &offer.attestation;
    [
        offer.offer_id.0.as_str(),
        statement.policy_version.as_str(),
        statement.claim_descriptor_version.as_str(),
        statement.policy_ref.0.as_str(),
        statement.claim_ref.0.as_str(),
        statement.evaluator_ref.0.as_str(),
        statement.cooperative_claim_ref.0.as_str(),
        statement.cooperative_evaluator_ref.0.as_str(),
        statement.producer.0.as_str(),
        statement.subject.subject_ref.as_str(),
    ]
    .into_iter()
    .all(|value| {
        !value.trim().is_empty() && value.len() <= MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
    })
}

fn cooperative_bounded_offer_id(offer_id: &StableId) -> Option<StableId> {
    (!offer_id.0.trim().is_empty()
        && offer_id.0.len() <= MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES)
        .then(|| offer_id.clone())
}

fn replacement_claims(
    projection: &ClaimWalProjection,
    now: u64,
) -> Result<Vec<ReplacementClaimProjection>, WorkflowGovernanceAdapterError> {
    let now = i64::try_from(now).map_err(|_| WorkflowGovernanceAdapterError::ClockOverflow)?;
    Ok(projection
        .latest_by_claim_id
        .iter()
        .map(|(id, projected)| {
            let liveness = if projection.active_by_claim_id.contains_key(id) {
                if is_live(&projected.claim_contract, now) {
                    ReplacementClaimLiveness::Live
                } else {
                    ReplacementClaimLiveness::Expired
                }
            } else {
                ReplacementClaimLiveness::NonActive
            };
            ReplacementClaimProjection {
                claim: projected.claim_contract.clone(),
                last_sequence: projected.last_seq,
                liveness,
            }
        })
        .collect())
}

fn replacement_claims_from_existing_state(
    state_root: &Path,
    now: u64,
) -> Result<Vec<ReplacementClaimProjection>, WorkflowGovernanceAdapterError> {
    let wal = claim_wal_path(state_root);
    let lock = claim_wal_lock_path(state_root);
    let wal_exists = fs::symlink_metadata(&wal);
    let lock_exists = fs::symlink_metadata(&lock);
    match (&wal_exists, &lock_exists) {
        (Err(wal_error), Err(lock_error))
            if wal_error.kind() == std::io::ErrorKind::NotFound
                && lock_error.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(Vec::new());
        }
        (Ok(_), Ok(_)) => {}
        _ => {
            return Err(WorkflowGovernanceAdapterError::ClaimProjection(
                "claim WAL and its lock must both be present or both be absent".to_owned(),
            ));
        }
    }
    let projection = project_existing_claim_wal(
        state_root,
        &ClaimWalProjectionOptions {
            repair: false,
            stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
        },
    )
    .map_err(|error| WorkflowGovernanceAdapterError::ClaimProjection(error.to_string()))?;
    replacement_claims(&projection, now)
}

fn replacement_objective_history(
    records: &[WorkflowGovernanceLedgerRecord],
    readiness_profile: WorkflowReadinessProfile,
) -> Result<
    (
        Vec<WorkflowReplacementObjectiveRevision>,
        Option<String>,
        Option<u64>,
        Option<u64>,
    ),
    WorkflowGovernanceAdapterError,
> {
    let active_cooperative = (readiness_profile == WorkflowReadinessProfile::SoloCooperative)
        .then(|| {
            records.iter().rev().find_map(|record| {
                matches!(
                    &record.event,
                    WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
                )
                .then_some(record.record_digest.as_str())
            })
        })
        .flatten();
    let active_human = (readiness_profile == WorkflowReadinessProfile::StrictExternal)
        .then(|| {
            records.iter().rev().find_map(|record| {
                matches!(
                    record.event,
                    WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
                )
                .then_some(record.record_digest.as_str())
            })
        })
        .flatten();
    let mut history = Vec::new();
    let mut active_digest = None;
    let mut active_revision = None;
    let mut active_epoch = None;
    for record in records {
        match &record.event {
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) => {
                let active = active_cooperative == Some(record.record_digest.as_str());
                let objective = WorkflowActiveCooperativeObjective {
                    objective_id: event.objective_id.clone(),
                    revision: event.revision,
                    assurance_epoch: event.assurance_epoch,
                    proposal: event.proposal.clone(),
                    objective_digest: event.objective_digest.clone(),
                    previous_objective_digest: event.previous_objective_digest.clone(),
                    revision_kind: event.revision_kind,
                    revision_reason: event.revision_reason.clone(),
                    accepted_record_digest: record.record_digest.clone(),
                    accepted_sequence: record.sequence,
                    accepted_state_version: record.state_version,
                    snapshot_digest_at_acceptance: event.snapshot_digest.clone(),
                    ledger_head_before_acceptance: event.ledger_head_digest.clone(),
                    acceptance_action_packet_digest: event.acceptance_action_packet_digest.clone(),
                    carrying_principal: event.carrying_principal.clone(),
                    host_provenance: event.host_provenance.clone(),
                    authority_basis: event.authority_basis,
                    accepted_at_unix: event.accepted_at_unix,
                };
                if active {
                    active_digest = Some(event.objective_digest.clone());
                    active_revision = Some(event.revision);
                    active_epoch = Some(event.assurance_epoch);
                }
                history.push(WorkflowReplacementObjectiveRevision::CooperativeSameOwner {
                    active,
                    record_digest: record.record_digest.clone(),
                    sequence: record.sequence,
                    state_version: record.state_version,
                    objective,
                });
            }
            WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) => {
                let active = active_human == Some(record.record_digest.as_str());
                if active {
                    active_digest = Some(event.intent_digest.clone());
                    active_revision = Some(event.intent.revision);
                    active_epoch = Some(event.assurance_epoch);
                }
                history.push(WorkflowReplacementObjectiveRevision::HumanIntent {
                    active,
                    record_digest: record.record_digest.clone(),
                    sequence: record.sequence,
                    state_version: record.state_version,
                    event: event.clone(),
                });
            }
            _ => {}
        }
    }
    Ok((history, active_digest, active_revision, active_epoch))
}

fn replacement_decision_history(
    records: &[WorkflowGovernanceLedgerRecord],
) -> Vec<WorkflowReplacementDecisionAudit> {
    let mut history = Vec::<WorkflowReplacementDecisionAudit>::new();
    for record in records {
        match &record.event {
            WorkflowGovernanceEvent::DecisionNeedRaised(event) => {
                history.push(WorkflowReplacementDecisionAudit {
                    policy_ref: event.policy_ref.clone(),
                    decision_ref: event.decision_ref.clone(),
                    status: WorkflowReplacementDecisionStatus::Unresolved,
                    need_record_digest: record.record_digest.clone(),
                    need_sequence: record.sequence,
                    resolution_record_digest: None,
                    selected_alternative_ref: None,
                });
            }
            WorkflowGovernanceEvent::DecisionResolved(event) => {
                if let Some(pending) = history.iter_mut().rev().find(|pending| {
                    pending.policy_ref == event.policy_ref
                        && pending.decision_ref == event.decision_ref
                        && pending.status == WorkflowReplacementDecisionStatus::Unresolved
                }) {
                    pending.status = WorkflowReplacementDecisionStatus::Resolved;
                    pending.resolution_record_digest = Some(record.record_digest.clone());
                    pending.selected_alternative_ref = Some(event.selected_alternative_ref.clone());
                }
            }
            _ => {}
        }
    }
    history
}

fn replacement_evidence_history(
    records: &[WorkflowGovernanceLedgerRecord],
    guidance: &WorkflowGovernanceGuidance,
    now: u64,
) -> Vec<WorkflowReplacementEvidenceAudit> {
    let mut admitted = guidance
        .simulation
        .candidate_claim_results
        .iter()
        .flat_map(|claim| claim.accepted_evidence_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    if let Some(assurance) = guidance.durable_assurance.projection.as_ref() {
        admitted.extend(assurance.lenses.iter().flat_map(|lens| {
            lens.evidence
                .iter()
                .map(|evidence| evidence.evidence_ref.clone())
        }));
    }
    let rejected = guidance
        .simulation
        .candidate_claim_results
        .iter()
        .flat_map(|claim| claim.rejected_evidence_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    let revoked = records
        .iter()
        .filter_map(|record| {
            if let WorkflowGovernanceEvent::ReceiptRevoked(event) = &record.event {
                Some(event.revoked_record_digest.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    records
        .iter()
        .filter_map(|record| {
            let WorkflowGovernanceEvent::EvaluatorObserved(event) = &record.event else {
                return None;
            };
            let status = if revoked.contains(&record.record_digest) {
                WorkflowReplacementEvidenceStatus::Revoked
            } else if event
                .expires_at_unix
                .is_some_and(|expires_at| expires_at < now)
            {
                WorkflowReplacementEvidenceStatus::Expired
            } else if admitted.contains(&event.provenance.semantic_identity.0) {
                WorkflowReplacementEvidenceStatus::Admitted
            } else if rejected.contains(&event.provenance.semantic_identity.0) {
                WorkflowReplacementEvidenceStatus::Rejected
            } else {
                WorkflowReplacementEvidenceStatus::HistoricalNotCurrent
            };
            Some(WorkflowReplacementEvidenceAudit {
                record_digest: record.record_digest.clone(),
                sequence: record.sequence,
                status,
                evidence: event.clone(),
            })
        })
        .collect()
}

fn replacement_projection_digest(
    domain: &str,
    value: &impl Serialize,
) -> Result<String, WorkflowGovernanceAdapterError> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        WorkflowGovernanceAdapterError::InvalidObservation(format!(
            "replacement continuity projection could not be canonicalized: {error}"
        ))
    })?;
    let mut material = Vec::with_capacity(domain.len() + canonical.len() + 1);
    material.extend_from_slice(domain.as_bytes());
    material.push(0);
    material.extend_from_slice(&canonical);
    Ok(sha256_content_hash(&material))
}

fn replacement_ranked_actions(
    promotions: &[WorkflowReplacementPromotionAudit],
    gaps: &[WorkflowReplacementGap],
    governed: &[NextAction],
) -> Vec<WorkflowReplacementRankedAction> {
    if let Some(gap) = gaps.iter().find(|gap| gap.blocking) {
        return vec![WorkflowReplacementRankedAction {
            rank: 1,
            kind: WorkflowReplacementRankedActionKind::ResolveContinuityGap,
            description: gap.summary.clone(),
            argv: Vec::new(),
            governed_action: None,
        }];
    }
    let mut actions = promotions
        .iter()
        .filter(|promotion| promotion.status == WorkflowReplacementPromotionStatus::Recoverable)
        .map(|promotion| WorkflowReplacementRankedAction {
            rank: 0,
            kind: WorkflowReplacementRankedActionKind::RecoverPromotion,
            description: promotion.summary.clone(),
            argv: promotion.recovery_argv.clone(),
            governed_action: None,
        })
        .collect::<Vec<_>>();
    let mut governed = governed.to_vec();
    governed.sort_by_key(|action| (action.rank, action.id.0.clone()));
    actions.extend(
        governed
            .into_iter()
            .map(|action| WorkflowReplacementRankedAction {
                rank: 0,
                kind: WorkflowReplacementRankedActionKind::GovernedNext,
                description: action.description.clone(),
                argv: Vec::new(),
                governed_action: Some(action),
            }),
    );
    if actions.is_empty() {
        actions.push(WorkflowReplacementRankedAction {
            rank: 1,
            kind: WorkflowReplacementRankedActionKind::Continue,
            description: "Continue from the durable governed state shown above.".to_owned(),
            argv: Vec::new(),
            governed_action: None,
        });
    }
    actions
}

fn active_cooperative_objective_from_ledger(
    records: &[WorkflowGovernanceLedgerRecord],
) -> Result<Option<WorkflowActiveCooperativeObjective>, WorkflowGovernanceAdapterError> {
    let mut active = None;
    for record in records {
        let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event else {
            continue;
        };
        active = Some(WorkflowActiveCooperativeObjective {
            objective_id: event.objective_id.clone(),
            revision: event.revision,
            assurance_epoch: event.assurance_epoch,
            proposal: event.proposal.clone(),
            objective_digest: event.objective_digest.clone(),
            previous_objective_digest: event.previous_objective_digest.clone(),
            revision_kind: event.revision_kind,
            revision_reason: event.revision_reason.clone(),
            accepted_record_digest: record.record_digest.clone(),
            accepted_sequence: record.sequence,
            accepted_state_version: record.state_version,
            snapshot_digest_at_acceptance: event.snapshot_digest.clone(),
            ledger_head_before_acceptance: event.ledger_head_digest.clone(),
            acceptance_action_packet_digest: event.acceptance_action_packet_digest.clone(),
            carrying_principal: event.carrying_principal.clone(),
            host_provenance: event.host_provenance.clone(),
            authority_basis: event.authority_basis,
            accepted_at_unix: event.accepted_at_unix,
        });
    }
    Ok(active)
}

fn accepted_cooperative_objective_record(
    records: &[WorkflowGovernanceLedgerRecord],
) -> Result<
    Option<(
        &WorkflowGovernanceLedgerRecord,
        &CooperativeObjectiveAcceptedEvent,
    )>,
    WorkflowGovernanceAdapterError,
> {
    let mut accepted = None;
    for record in records {
        let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event else {
            continue;
        };
        accepted = Some((record, event));
    }
    Ok(accepted)
}

fn validated_cooperative_objective_packet(
    guidance: &WorkflowGovernanceGuidance,
    packet_digest: &str,
) -> Result<(WorkflowAuthorizationActionPacket, StableId, u64, u64), WorkflowGovernanceAdapterError>
{
    let packet = guidance
        .authorization
        .action_packets
        .iter()
        .chain(guidance.authorization.objective_management_packet.iter())
        .find(|packet| packet.packet_digest == packet_digest)
        .cloned()
        .ok_or(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)?;
    if packet.authorization_kind != WorkflowAuthorizationKind::IntentRevision
        || packet.required_authority.approval_boundary
            != WorkflowAuthorizationApprovalBoundary::CooperativeSameOwner
        || packet.binding.snapshot_digest != guidance.snapshot_digest
        || packet.binding.ledger_head_digest != guidance.ledger_head_digest
        || packet.binding.state_version != guidance.state_version
        || packet.binding.trusted_principal_registry_digest.is_some()
        || packet.binding.trusted_broker_registry_digest.is_some()
    {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    }
    let WorkflowAuthorizationInputContract::CooperativeObjective {
        objective_id,
        next_objective_revision,
        next_assurance_epoch,
        ..
    } = &packet.input_contract
    else {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    };
    if objective_id.0.trim().is_empty()
        || objective_id.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
    {
        return Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch);
    }
    Ok((
        packet.clone(),
        objective_id.clone(),
        *next_objective_revision,
        *next_assurance_epoch,
    ))
}

fn cooperative_objective_action_packets(
    guidance: &WorkflowGovernanceGuidance,
) -> Result<Vec<WorkflowAuthorizationActionPacket>, WorkflowGovernanceAdapterError> {
    let (objective_id, next_objective_revision, next_assurance_epoch, variants) =
        if let Some(active) = guidance.active_cooperative_objective.as_ref() {
            let next_revision = active
                .revision
                .checked_add(1)
                .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
            let next_epoch = active
                .assurance_epoch
                .checked_add(1)
                .ok_or(WorkflowGovernanceAdapterError::StateVersionOverflow)?;
            (
                active.objective_id.clone(),
                next_revision,
                next_epoch,
                cooperative_objective_revision_templates(),
            )
        } else {
            (
                StableId(format!("objective.workflow.{}", guidance.project_id.0)),
                1,
                1,
                cooperative_objective_initial_templates(),
            )
        };
    if objective_id.0.trim().is_empty()
        || objective_id.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
    {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(format!(
            "derived objective_id must be nonblank and at most {MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES} bytes"
        )));
    }
    let binding = WorkflowAuthorizationPacketBinding {
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
        policy_ref: guidance.selected_policy_ref.clone(),
        subject_ref: objective_id.clone(),
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        trusted_principal_registry_digest: None,
        trusted_broker_registry_digest: None,
        readiness_target: guidance.target,
    };
    make_authorization_action_packet(
        WorkflowAuthorizationKind::IntentRevision,
        StableId(format!(
            "packet.workflow.cooperative-objective.{}",
            objective_id.0
        )),
        binding,
        cooperative_authority("workflow.objective.accept_cooperative"),
        WorkflowAuthorizationInputContract::CooperativeObjective {
            objective_id,
            next_objective_revision,
            next_assurance_epoch,
            input_encoding: "utf8_json_file".to_owned(),
            discriminator_field: "kind".to_owned(),
            unknown_fields_allowed: false,
            variants,
            limits: WorkflowCooperativeObjectiveInputLimits {
                input_max_bytes: MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES,
                objective_id_max_bytes: MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES,
                carrying_principal_max_bytes: MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES,
                host_coordinate_max_bytes: MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES,
                revision_reason_max_bytes: MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES,
                outcome_max_bytes: MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
                list_max_items: MAX_WORKFLOW_INTENT_LIST_ITEMS,
                list_item_max_bytes: MAX_WORKFLOW_INTENT_ITEM_BYTES,
                proposal_total_max_bytes: MAX_WORKFLOW_INTENT_TOTAL_BYTES,
                decision_question_max_bytes: MAX_WORKFLOW_COOPERATIVE_DECISION_QUESTION_BYTES,
                decision_alternatives_min_items: MIN_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES,
                decision_alternatives_max_items: MAX_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES,
                decision_consequences_max_items: MAX_WORKFLOW_COOPERATIVE_DECISION_CONSEQUENCES,
            },
            command_argv_template: vec![
                "forge-core".to_owned(),
                "workflow".to_owned(),
                "intent".to_owned(),
                "accept-cooperative".to_owned(),
                "--root".to_owned(),
                "<project-root>".to_owned(),
                "--packet-digest".to_owned(),
                "<packet-digest>".to_owned(),
                "--input-file".to_owned(),
                "<temporary-utf8-json-path>".to_owned(),
                "--json".to_owned(),
            ],
        },
    )
    .map(|packet| vec![packet])
}

fn cooperative_host_template() -> serde_json::Value {
    serde_json::json!({
        "host_id": "<host id>",
        "host_version": "<host version>",
        "session_ref": "<chat/session reference>",
        "interaction_ref": "<human interaction reference>",
        "conversation_digest": "sha256:<64 lowercase hex>",
        "observed_at_unix": 1
    })
}

fn cooperative_decision_template() -> WorkflowCooperativeObjectiveInputTemplate {
    WorkflowCooperativeObjectiveInputTemplate {
        variant: "decision_required".to_owned(),
        template: serde_json::json!({
            "kind": "decision_required",
            "decision_request": {
                "id": "<decision id>",
                "question": "<one concise irreducible question>",
                "reason": "product_direction",
                "alternatives": [
                    {
                        "id": "<alternative id>",
                        "description": "<alternative description>",
                        "consequences": ["<consequence>"]
                    },
                    {
                        "id": "<alternative id>",
                        "description": "<alternative description>",
                        "consequences": ["<consequence>"]
                    }
                ],
                "recommended_alternative_ref": "<one supplied alternative id>",
                "blocking": true,
                "blocks_before": "execute"
            }
        }),
    }
}

fn cooperative_objective_initial_templates() -> Vec<WorkflowCooperativeObjectiveInputTemplate> {
    vec![
        WorkflowCooperativeObjectiveInputTemplate {
            variant: "unambiguous".to_owned(),
            template: serde_json::json!({
                "kind": "unambiguous",
                "proposal": {
                    "outcome": "<chat-derived outcome>",
                    "constraints": ["<constraint>"],
                    "unacceptable_outcomes": ["<unacceptable outcome>"],
                    "open_uncertainties": ["<open uncertainty>"]
                },
                "carrying_principal": "<same-owner host principal>",
                "host_provenance": cooperative_host_template()
            }),
        },
        cooperative_decision_template(),
    ]
}

fn cooperative_objective_revision_templates() -> Vec<WorkflowCooperativeObjectiveInputTemplate> {
    vec![
        WorkflowCooperativeObjectiveInputTemplate {
            variant: "material_supersession".to_owned(),
            template: serde_json::json!({
                "kind": "material_supersession",
                "proposal": {
                    "outcome": "<corrected chat-derived outcome>",
                    "constraints": ["<constraint>"],
                    "unacceptable_outcomes": ["<unacceptable outcome>"],
                    "open_uncertainties": ["<open uncertainty>"]
                },
                "supersession_reason": "<why the active objective changed materially>",
                "carrying_principal": "<same-owner host principal>",
                "host_provenance": cooperative_host_template()
            }),
        },
        WorkflowCooperativeObjectiveInputTemplate {
            variant: "non_material_clarification".to_owned(),
            template: serde_json::json!({
                "kind": "non_material_clarification",
                "added_constraints": ["<additional constraint>"],
                "added_unacceptable_outcomes": ["<additional unacceptable outcome>"],
                "added_open_uncertainties": ["<additional open uncertainty>"],
                "clarification_reason": "<why this adds detail without changing direction>",
                "carrying_principal": "<same-owner host principal>",
                "host_provenance": cooperative_host_template()
            }),
        },
        cooperative_decision_template(),
    ]
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
    let binding_for = |policy: &WorkflowGovernancePolicy,
                       subject_ref: StableId,
                       readiness_target: ReadinessTarget| {
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
            readiness_target,
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
        binding_for(selected, intent_id.clone(), guidance.target),
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

    let mut policy_contexts = vec![(selected, guidance.target, &guidance.simulation)];
    for boundary in &guidance.boundary_rechecks {
        let policy = policy_by_id(bundle, &boundary.policy_ref)?;
        if policy_contexts
            .iter()
            .all(|(candidate, _, _)| candidate.id != policy.id)
        {
            policy_contexts.push((policy, boundary.requested_target, &boundary.simulation));
        }
    }

    if guidance.status == WorkflowGovernanceGuidanceStatus::ApplicabilityRequired {
        packets.push(make_authorization_action_packet(
            WorkflowAuthorizationKind::Applicability,
            StableId(format!("packet.workflow.applicability.{}", selected.id.0)),
            binding_for(selected, selected.id.clone(), guidance.target),
            human_authority("workflow.applicability.assess"),
            WorkflowAuthorizationInputContract::Applicability {
                basis_refs_min_items: 1,
                basis_refs_repo_relative: true,
            },
        )?);
    }

    for (action_policy, readiness_target, simulation) in &policy_contexts {
        for gap in &simulation.candidate_capability_gaps {
            let requirement = action_policy
                .capability_requirements
                .iter()
                .find(|candidate| candidate.id == gap.id)
                .ok_or_else(|| {
                    WorkflowGovernanceAdapterError::UnknownCapability(gap.id.0.clone())
                })?;
            packets.push(make_authorization_action_packet(
                WorkflowAuthorizationKind::Capability,
                StableId(format!("packet.workflow.capability.{}", requirement.id.0)),
                binding_for(action_policy, requirement.id.clone(), *readiness_target),
                runtime_authority("workflow.capability.authorize"),
                WorkflowAuthorizationInputContract::Capability {
                    capability_ref: requirement.id.clone(),
                    probe_kind: requirement.probe_kind,
                    subject_kinds: capability_subject_kinds(requirement.probe_kind),
                    probe_reference_required: true,
                },
            )?);
        }

        for request in &simulation.candidate_decision_requests {
            packets.push(make_authorization_action_packet(
                WorkflowAuthorizationKind::Decision,
                StableId(format!("packet.workflow.decision.{}", request.id.0)),
                binding_for(action_policy, request.id.clone(), *readiness_target),
                human_authority("workflow.decision.resolve"),
                WorkflowAuthorizationInputContract::Decision {
                    decision_ref: request.id.clone(),
                    alternatives: request.alternatives.clone(),
                    recommended_alternative_ref: request.recommended_alternative_ref.clone(),
                },
            )?);
        }
    }

    let mut actionable_policies = policy_contexts
        .iter()
        .map(|(policy, target, simulation)| (*policy, *target, Some(*simulation)))
        .collect::<Vec<_>>();
    if let Some(assurance_policy) = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == UNIVERSAL_ASSURANCE_POLICY_ID)
        .filter(|policy| {
            actionable_policies
                .iter()
                .all(|(candidate, _, _)| candidate.id != policy.id)
        })
    {
        actionable_policies.push((
            assurance_policy,
            assurance_policy.routing.readiness_target,
            None,
        ));
    }
    for (action_policy, readiness_target, simulation) in actionable_policies {
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
                let result = simulation
                    .and_then(|simulation| {
                        simulation
                            .candidate_claim_results
                            .iter()
                            .find(|candidate| candidate.claim_id == claim.id.0)
                    })
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
                binding_for(action_policy, claim.id.clone(), readiness_target),
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
                let maximum_readiness_target = if max_target.rank() < readiness_target.rank() {
                    *max_target
                } else {
                    readiness_target
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
                    binding_for(action_policy, claim.id.clone(), readiness_target),
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
                binding_for(policy, subject_ref, policy.routing.readiness_target),
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

fn cooperative_authority(grant: &str) -> WorkflowAuthorizationRequiredAuthority {
    WorkflowAuthorizationRequiredAuthority {
        accepted_roles: vec![CallerRole::Runtime, CallerRole::Worker, CallerRole::Driver],
        required_grant: StableId(grant.to_owned()),
        approval_boundary: WorkflowAuthorizationApprovalBoundary::CooperativeSameOwner,
    }
}

fn cooperative_input_host_provenance(
    input: &WorkflowCooperativeObjectiveInput,
) -> Option<&WorkflowCooperativeHostProvenance> {
    match input {
        WorkflowCooperativeObjectiveInput::Unambiguous {
            host_provenance, ..
        }
        | WorkflowCooperativeObjectiveInput::MaterialSupersession {
            host_provenance, ..
        }
        | WorkflowCooperativeObjectiveInput::NonMaterialClarification {
            host_provenance, ..
        } => Some(host_provenance),
        WorkflowCooperativeObjectiveInput::DecisionRequired { .. } => None,
    }
}

fn validate_cooperative_objective_input(
    input: &WorkflowCooperativeObjectiveInput,
) -> Result<(), WorkflowGovernanceAdapterError> {
    match input {
        WorkflowCooperativeObjectiveInput::DecisionRequired { decision_request } => {
            validate_cooperative_decision_request(decision_request)
        }
        WorkflowCooperativeObjectiveInput::Unambiguous {
            proposal,
            carrying_principal,
            host_provenance,
        } => {
            validate_cooperative_carrier(carrying_principal, host_provenance)?;
            workflow_cooperative_objective_digest(
                &StableId("objective.validation".to_owned()),
                1,
                1,
                proposal,
            )?;
            Ok(())
        }
        WorkflowCooperativeObjectiveInput::MaterialSupersession {
            proposal,
            supersession_reason,
            carrying_principal,
            host_provenance,
        } => {
            validate_cooperative_carrier(carrying_principal, host_provenance)?;
            validate_cooperative_revision_reason("supersession_reason", supersession_reason)?;
            workflow_cooperative_objective_digest(
                &StableId("objective.validation".to_owned()),
                1,
                1,
                proposal,
            )?;
            Ok(())
        }
        WorkflowCooperativeObjectiveInput::NonMaterialClarification {
            added_constraints,
            added_unacceptable_outcomes,
            added_open_uncertainties,
            clarification_reason,
            carrying_principal,
            host_provenance,
        } => {
            validate_cooperative_carrier(carrying_principal, host_provenance)?;
            validate_cooperative_revision_reason("clarification_reason", clarification_reason)?;
            validate_cooperative_additions(&[
                added_constraints,
                added_unacceptable_outcomes,
                added_open_uncertainties,
            ])
        }
    }
}

fn validate_cooperative_revision_reason(
    field: &str,
    reason: &str,
) -> Result<(), WorkflowGovernanceAdapterError> {
    if reason.trim().is_empty() || reason.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(format!(
            "{field} must be nonblank and at most {MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_cooperative_additions(
    lists: &[&Vec<String>],
) -> Result<(), WorkflowGovernanceAdapterError> {
    if lists.iter().all(|values| values.is_empty()) {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "non_material_clarification must add at least one bounded detail".to_owned(),
        ));
    }
    for values in lists {
        if values.len() > MAX_WORKFLOW_INTENT_LIST_ITEMS
            || values.iter().any(|value| {
                value.trim().is_empty() || value.len() > MAX_WORKFLOW_INTENT_ITEM_BYTES
            })
        {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                "non_material_clarification additions exceed the objective list bounds".to_owned(),
            ));
        }
    }
    Ok(())
}

fn cooperative_revision_from_input(
    previous: Option<&CooperativeObjectiveAcceptedEvent>,
    input: WorkflowCooperativeObjectiveInput,
) -> Result<
    (
        WorkflowCooperativeObjectiveProposal,
        WorkflowCooperativeObjectiveRevisionKind,
        Option<String>,
        PrincipalId,
        WorkflowCooperativeHostProvenance,
    ),
    WorkflowGovernanceAdapterError,
> {
    match (previous, input) {
        (
            None,
            WorkflowCooperativeObjectiveInput::Unambiguous {
                proposal,
                carrying_principal,
                host_provenance,
            },
        ) => Ok((
            proposal,
            WorkflowCooperativeObjectiveRevisionKind::Initial,
            None,
            carrying_principal,
            host_provenance,
        )),
        (
            Some(previous),
            WorkflowCooperativeObjectiveInput::MaterialSupersession {
                proposal,
                supersession_reason,
                carrying_principal,
                host_provenance,
            },
        ) => {
            if proposal == previous.proposal {
                return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                    "material_supersession must change the active objective proposal".to_owned(),
                ));
            }
            Ok((
                proposal,
                WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession,
                Some(supersession_reason),
                carrying_principal,
                host_provenance,
            ))
        }
        (
            Some(previous),
            WorkflowCooperativeObjectiveInput::NonMaterialClarification {
                added_constraints,
                added_unacceptable_outcomes,
                added_open_uncertainties,
                clarification_reason,
                carrying_principal,
                host_provenance,
            },
        ) => {
            let proposal = merge_cooperative_clarification(
                &previous.proposal,
                &added_constraints,
                &added_unacceptable_outcomes,
                &added_open_uncertainties,
            )?;
            Ok((
                proposal,
                WorkflowCooperativeObjectiveRevisionKind::NonMaterialClarification,
                Some(clarification_reason),
                carrying_principal,
                host_provenance,
            ))
        }
        (None, _) => Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "the initial cooperative objective requires the unambiguous input variant".to_owned(),
        )),
        (Some(_), _) => Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "an active cooperative objective requires material_supersession or non_material_clarification"
                .to_owned(),
        )),
    }
}

fn merge_cooperative_clarification(
    previous: &WorkflowCooperativeObjectiveProposal,
    added_constraints: &[String],
    added_unacceptable_outcomes: &[String],
    added_open_uncertainties: &[String],
) -> Result<WorkflowCooperativeObjectiveProposal, WorkflowGovernanceAdapterError> {
    let mut proposal = previous.clone();
    for (target, additions) in [
        (&mut proposal.constraints, added_constraints),
        (
            &mut proposal.unacceptable_outcomes,
            added_unacceptable_outcomes,
        ),
        (&mut proposal.open_uncertainties, added_open_uncertainties),
    ] {
        let mut appended = BTreeSet::new();
        for addition in additions {
            if target.contains(addition) || !appended.insert(addition) {
                return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                    "non_material_clarification cannot repeat an active or appended detail"
                        .to_owned(),
                ));
            }
            target.push(addition.clone());
        }
    }
    workflow_cooperative_objective_digest(
        &StableId("objective.validation".to_owned()),
        1,
        1,
        &proposal,
    )?;
    Ok(proposal)
}

fn cooperative_retry_matches(
    accepted: &CooperativeObjectiveAcceptedEvent,
    input: &WorkflowCooperativeObjectiveInput,
) -> Result<bool, WorkflowGovernanceAdapterError> {
    match input {
        WorkflowCooperativeObjectiveInput::Unambiguous {
            proposal,
            carrying_principal,
            host_provenance,
        } => Ok(
            accepted.revision_kind == WorkflowCooperativeObjectiveRevisionKind::Initial
                && accepted.revision_reason.is_none()
                && accepted.revision_input_digest.is_none()
                && proposal == &accepted.proposal
                && carrying_principal == &accepted.carrying_principal
                && host_provenance == &accepted.host_provenance,
        ),
        WorkflowCooperativeObjectiveInput::MaterialSupersession { .. } => {
            let expected = workflow_cooperative_revision_input_digest(input)?;
            Ok(accepted.revision_kind
                == WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession
                && accepted.revision_input_digest.as_deref() == Some(expected.as_str()))
        }
        WorkflowCooperativeObjectiveInput::NonMaterialClarification { .. } => {
            let expected = workflow_cooperative_revision_input_digest(input)?;
            Ok(accepted.revision_kind
                == WorkflowCooperativeObjectiveRevisionKind::NonMaterialClarification
                && accepted.revision_input_digest.as_deref() == Some(expected.as_str()))
        }
        WorkflowCooperativeObjectiveInput::DecisionRequired { .. } => Ok(false),
    }
}

fn validate_cooperative_carrier(
    principal: &PrincipalId,
    provenance: &WorkflowCooperativeHostProvenance,
) -> Result<(), WorkflowGovernanceAdapterError> {
    for (field, value) in [
        ("carrying_principal", principal.0.as_str()),
        ("host_id", provenance.host_id.0.as_str()),
        ("host_version", provenance.host_version.as_str()),
        ("session_ref", provenance.session_ref.as_str()),
        ("interaction_ref", provenance.interaction_ref.as_str()),
    ] {
        if value.trim().is_empty() || value.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(format!(
                "{field} must be nonblank and at most {MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES} bytes"
            )));
        }
    }
    if provenance.observed_at_unix == 0 || !is_lower_sha256_text(&provenance.conversation_digest) {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "cooperative host provenance requires a nonzero observation time and canonical conversation digest"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_cooperative_decision_request(
    request: &DecisionRequest,
) -> Result<(), WorkflowGovernanceAdapterError> {
    if request.id.0.trim().is_empty()
        || request.id.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
        || request.question.trim().is_empty()
        || request.question.len() > MAX_WORKFLOW_COOPERATIVE_DECISION_QUESTION_BYTES
        || request.question.contains('\r')
        || request.question.contains('\n')
        || !(MIN_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES
            ..=MAX_WORKFLOW_COOPERATIVE_DECISION_ALTERNATIVES)
            .contains(&request.alternatives.len())
    {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "decision_required must contain one concise question and two to eight alternatives"
                .to_owned(),
        ));
    }
    let mut ids = BTreeSet::new();
    for alternative in &request.alternatives {
        if alternative.id.0.trim().is_empty()
            || alternative.id.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
            || alternative.description.trim().is_empty()
            || alternative.description.len() > MAX_WORKFLOW_INTENT_ITEM_BYTES
            || !ids.insert(alternative.id.clone())
            || alternative.consequences.len() > MAX_WORKFLOW_COOPERATIVE_DECISION_CONSEQUENCES
            || alternative.consequences.iter().any(|consequence| {
                consequence.trim().is_empty() || consequence.len() > MAX_WORKFLOW_INTENT_ITEM_BYTES
            })
        {
            return Err(WorkflowGovernanceAdapterError::InvalidObservation(
                "decision_required alternatives must be unique, bounded, and concrete".to_owned(),
            ));
        }
    }
    if !ids.contains(&request.recommended_alternative_ref) {
        return Err(WorkflowGovernanceAdapterError::InvalidObservation(
            "decision_required recommendation must reference one supplied alternative".to_owned(),
        ));
    }
    Ok(())
}

fn is_lower_sha256_text(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
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
                accepted_roles: vec![CallerRole::Runtime],
                required_grant: StableId("workflow.evidence.authorize_external".to_owned()),
                approval_boundary: WorkflowAuthorizationApprovalBoundary::ExternalAuthorityBroker,
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
                accepted_roles: vec![CallerRole::Worker, CallerRole::Driver],
                required_grant: StableId("workflow.evidence.authorize_review".to_owned()),
                approval_boundary: WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker,
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
    _project_root: &Path,
    broker_status: WorkflowAuthorizationRegistrySetupStatus,
    packets: &[WorkflowAuthorizationActionPacket],
) -> Vec<WorkflowAuthorizationSetupGap> {
    let (code, state_label) = match broker_status {
        WorkflowAuthorizationRegistrySetupStatus::Missing => (
            WorkflowAuthorizationSetupGapCode::BrokerRegistryMissing,
            "the project has no external workflow broker registry",
        ),
        WorkflowAuthorizationRegistrySetupStatus::LegacyRecoveryOnly => (
            WorkflowAuthorizationSetupGapCode::BrokerRegistryLegacyRecoveryOnly,
            "the project has only a frozen legacy broker registry that is recovery-only",
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
            WorkflowAuthorizationApprovalBoundary::CooperativeSameOwner => {}
            WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker => human = true,
            WorkflowAuthorizationApprovalBoundary::IndependentReviewerBroker => reviewer = true,
            WorkflowAuthorizationApprovalBoundary::TrustedRuntimeBroker
            | WorkflowAuthorizationApprovalBoundary::ExternalAuthorityBroker
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
            "{state_label}; {profile_label} broker setup is blocked because selected_host is unresolved and no preconfigured external operator trust anchor is available"
        ),
        accepted_profiles: vec![profile],
        external_setup: WorkflowBrokerExternalSetupState::Blocked {
            reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
        },
        setup_argv: Vec::new(),
        required_operator_inputs: vec![
            "selected_host_adapter".to_owned(),
            "external_operator_trust_anchor".to_owned(),
            "strict_registry_file".to_owned(),
            "signed_native_admin_authorization".to_owned(),
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

fn cooperative_subject_current(
    project_root: &Path,
    snapshot_digest: &str,
    subject: &WorkflowEvidenceSubject,
) -> Result<bool, WorkflowGovernanceAdapterError> {
    if !matches!(
        subject.kind,
        WorkflowEvidenceSubjectKind::Artifact
            | WorkflowEvidenceSubjectKind::RepositoryState
            | WorkflowEvidenceSubjectKind::ProjectSnapshot
    ) {
        return Ok(false);
    }
    subject_current(project_root, snapshot_digest, subject)
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

#[cfg(all(test, unix))]
fn inject_byte_identical_project_replacement_after_replay_reservation(
    state_root: &Path,
    project_root: &Path,
) {
    let marker = state_root.join(TEST_REPLACE_PROJECT_FILE_AFTER_REPLAY_RESERVATION_MARKER);
    if !marker.is_file() {
        return;
    }
    let relative = fs::read_to_string(&marker)
        .expect("test replacement marker must contain a project-relative path");
    fs::remove_file(&marker).expect("test replacement marker must be consumed");
    let relative = Path::new(relative.trim());
    assert!(
        relative.components().next().is_some()
            && relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "test replacement path must be non-empty and project-relative"
    );
    let target = project_root.join(relative);
    let bytes = fs::read(&target).expect("test replacement target must be readable");
    fs::remove_file(&target).expect("test replacement target must be removable");
    fs::write(&target, bytes).expect("test replacement target must be reminted byte-identically");
}

#[cfg(test)]
fn inject_project_change_before_cooperative_commit(state_root: &Path, project_root: &Path) {
    let marker = state_root.join(TEST_CHANGE_PROJECT_BEFORE_COOPERATIVE_COMMIT_MARKER);
    if !marker.is_file() {
        return;
    }
    let relative = fs::read_to_string(&marker)
        .expect("cooperative precommit marker must contain a project-relative path");
    fs::remove_file(&marker).expect("cooperative precommit marker must be consumed");
    let relative = Path::new(relative.trim());
    assert!(
        relative.components().next().is_some()
            && relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "cooperative precommit test path must be non-empty and project-relative"
    );
    fs::write(
        project_root.join(relative),
        b"deterministic project change before cooperative commit\n",
    )
    .expect("cooperative precommit test change");
}

fn replay_locked_commit_time(state_root: &Path) -> Result<u64, WorkflowGovernanceAdapterError> {
    let now = unix_time()?;
    #[cfg(not(test))]
    let _ = state_root;
    #[cfg(test)]
    {
        let marker = state_root.join(TEST_EXPIRE_AFTER_REPLAY_RESERVATION_MARKER);
        if marker.is_file() {
            fs::remove_file(&marker)
                .expect("test expiry marker must be consumed after replay lock acquisition");
            return now
                .checked_add(3_600)
                .ok_or(WorkflowGovernanceAdapterError::ClockOverflow);
        }
    }
    Ok(now)
}
#[cfg(test)]
fn inject_replay_append_failure_after_ledger(state_root: &Path) {
    let marker = state_root.join(TEST_REPLAY_APPEND_FAILURE_MARKER);
    if !marker.is_file() {
        return;
    }
    let wal_path = state_root
        .join(forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH);
    let backup_path = state_root.join(TEST_REPLAY_APPEND_FAILURE_BACKUP);
    fs::rename(&wal_path, &backup_path)
        .expect("test failpoint must move the replay WAL after ledger commit");
    fs::create_dir(&wal_path)
        .expect("test failpoint must replace the replay WAL path with a directory");
}
fn project_snapshot_digest(root: &Path) -> Result<String, WorkflowGovernanceAdapterError> {
    let snapshot = RetainedWorkflowProjectSnapshot::capture(root)?;
    snapshot.revalidate()?;
    Ok(snapshot.digest().to_owned())
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
        workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
        WorkflowBrokerEnrollmentDeclaration, WorkflowBrokerEventEnvelope,
        WorkflowBrokerFreshnessPolicy, WorkflowBrokerIssuerEntry, WorkflowBrokerReplayKey,
        WorkflowBrokerVerificationContext, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
        WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION,
    };
    use forge_core_contracts::request::DependencyRef;
    use forge_core_contracts::{
        AgentAutonomyAssessmentInput, AgentAutonomyWork, AgentOwnedWorkClass,
        ClaimContractDocument, CompletionContractDocument, HealthRecoveryContractDocument,
        PostBuildVerifyContinuityBinding, PostBuildVerifyEpisode, PostBuildVerifyEpisodeAuthority,
        PostBuildVerifyEvolutionIdentity, PostBuildVerifyEvolutionStatus,
        PostBuildVerifyEvolutionTrigger, PostBuildVerifyPolicyReference, PostBuildVerifyPolicyRole,
        PostBuildVerifyRollbackBaseline, RepoPath, RequestContractDocument, RuntimeKind,
        WorkflowBrokerBoundOperation, WorkflowBrokerCredentialProfile,
        WorkflowBrokerCredentialPurpose, WorkflowBrokerCustodyKind, WorkflowBrokerHostBinding,
        WorkflowBrokerHostInteractionKind, WorkflowBrokerNativeHostProvenance,
        WorkflowBrokerPublicCredentialMetadata, WorkflowBrokerPublicKeyAlgorithm,
        POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION, WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION,
        WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION,
    };
    use forge_core_store::claim_wal::{
        ClaimWalOperation, ClaimWalRecovery, ClaimWalStopReason, ProjectedClaim,
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

    fn crash_replace_residue_paths(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).expect("walk Forge state") {
                let entry = entry.expect("Forge state entry");
                let file_type = entry.file_type().expect("Forge state entry type");
                if file_type.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.contains(".forge-retained-delete-")
                    || name.contains(".forge-crash-absence-claim-")
                    || name.ends_with(".forge-next")
                    || name.ends_with(".forge-previous")
                    || name.ends_with(".forge-transaction")
                {
                    found.push(entry.path());
                }
            }
        }
        found
    }

    fn state_file_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).expect("walk state") {
                let entry = entry.expect("state entry");
                let kind = entry.file_type().expect("state entry type");
                if kind.is_dir() {
                    pending.push(entry.path());
                } else if kind.is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(root)
                        .expect("relative state path")
                        .to_path_buf();
                    files.insert(relative, fs::read(entry.path()).expect("state bytes"));
                }
            }
        }
        files
    }

    fn coordination_request_document() -> RequestContractDocument {
        yaml_serde::from_str(include_str!(
            "../../../../contracts/requests/worker-state-transition-request.yaml"
        ))
        .expect("request fixture")
    }

    fn coordination_completion_document() -> CompletionContractDocument {
        yaml_serde::from_str(include_str!(
            "../../../../contracts/completion/story-done-completion.yaml"
        ))
        .expect("completion fixture")
    }

    fn coordination_recovery_document() -> HealthRecoveryContractDocument {
        yaml_serde::from_str(include_str!(
            "../../../../contracts/recovery/runtime-crashed-recovery.yaml"
        ))
        .expect("health recovery fixture")
    }

    fn coordination_claim_document() -> ClaimContractDocument {
        yaml_serde::from_str(include_str!(
            "../../../../contracts/claims/driver-active-claim.yaml"
        ))
        .expect("claim fixture")
    }

    fn coordination_reference_fixture() -> ReferenceIndex {
        let mut index = ReferenceIndex::new();
        index.insert(
            "contracts/gates/story-ready-lane-gate.yaml",
            ReferenceKind::GateContract,
        );
        index.insert(
            "contracts/effects/story-artifact-write-effect.yaml",
            ReferenceKind::ToolEffectContract,
        );
        index
    }

    fn claim_projection(entries: Vec<(ClaimContract, bool)>) -> ClaimWalProjection {
        let mut latest_by_claim_id = BTreeMap::new();
        let mut active_by_claim_id = BTreeMap::new();
        let mut released_by_claim_id = BTreeMap::new();
        let mut claims = Vec::new();
        for (index, (claim, active)) in entries.into_iter().enumerate() {
            let sequence = u64::try_from(index).expect("claim index fits u64") + 1;
            let projected = ProjectedClaim {
                claim_contract: claim.clone(),
                last_seq: sequence,
                last_operation: if active {
                    ClaimWalOperation::Acquire
                } else {
                    ClaimWalOperation::Release
                },
                recorded_at: "2026-06-25T00:05:00Z".to_owned(),
                wal_offset: sequence * 100,
            };
            if active {
                active_by_claim_id.insert(claim.id.0.clone(), projected.clone());
            } else {
                released_by_claim_id.insert(claim.id.0.clone(), projected.clone());
            }
            latest_by_claim_id.insert(claim.id.0.clone(), projected);
            claims.push(claim);
        }
        ClaimWalProjection {
            recovery: ClaimWalRecovery {
                wal_path: PathBuf::from("claims.fmw1"),
                records: Vec::new(),
                checkpoint: None,
                last_observed_seq: u64::try_from(claims.len()).expect("claim count fits u64"),
                valid_record_count: claims.len(),
                last_good_offset: 0,
                original_len: 0,
                repaired: false,
                stop_reason: ClaimWalStopReason::CleanEof,
                retained_authority: None,
            },
            last_applied_seq: u64::try_from(claims.len()).expect("claim count fits u64"),
            applied_records: claims.len(),
            claims,
            latest_by_claim_id,
            active_by_claim_id,
            released_by_claim_id,
            handoff_recorded_by_claim_id: BTreeMap::new(),
            active_claim_ids_by_agent: BTreeMap::new(),
            active_claim_ids_by_scope: BTreeMap::new(),
            active_claim_ids_by_path: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn empty_claim_projection() -> ClaimWalProjection {
        claim_projection(Vec::new())
    }

    fn coordination_record(
        sequence: u64,
        state_version: u64,
        event: WorkflowGovernanceEvent,
    ) -> WorkflowGovernanceLedgerRecord {
        WorkflowGovernanceLedgerRecord {
            record_id: StableId(format!("record.coordination.{sequence}")),
            sequence,
            project_id: StableId("project.coordination".to_owned()),
            bundle_id: StableId("bundle.coordination".to_owned()),
            bundle_digest: format!("sha256:{}", "b".repeat(64)),
            state_version,
            previous_record_digest: (sequence > 1).then(|| format!("sha256:{:064x}", sequence - 1)),
            record_digest: format!("sha256:{sequence:064x}"),
            recorded_at_unix: sequence,
            event,
        }
    }

    fn coordination_state_record(
        sequence: u64,
        state_version: u64,
        prior_head: &str,
        prior_state_version: u64,
        state: CoordinationStateRecord,
    ) -> WorkflowGovernanceLedgerRecord {
        coordination_record(
            sequence,
            state_version,
            WorkflowGovernanceEvent::CoordinationStateApplied(CoordinationStateAppliedEvent {
                prior_ledger_head_digest: prior_head.to_owned(),
                prior_state_version,
                state,
            }),
        )
    }

    fn coordination_projection(
        records: Vec<WorkflowGovernanceLedgerRecord>,
    ) -> WorkflowGovernanceLedgerProjection {
        let next_sequence = records.last().map_or(1, |record| record.sequence + 1);
        let next_state_version = records.last().map_or(0, |record| record.state_version + 1);
        let head_digest = records.last().map(|record| record.record_digest.clone());
        WorkflowGovernanceLedgerProjection {
            records,
            head_digest,
            next_sequence,
            next_state_version,
        }
    }

    fn coordination_request_state(status: RequestStatus) -> CoordinationRequestState {
        let mut request = coordination_request_document();
        request.request_contract.status = status;
        CoordinationRequestState {
            actor_agent_id: if status == RequestStatus::Pending {
                request.request_contract.sender_agent_id.clone()
            } else {
                request.request_contract.target_driver.clone()
            },
            previous_status: match status {
                RequestStatus::Pending => None,
                RequestStatus::Accepted => Some(RequestStatus::Pending),
                _ => Some(RequestStatus::Accepted),
            },
            response_evidence_refs: if status == RequestStatus::Pending {
                Vec::new()
            } else {
                request
                    .request_contract
                    .response
                    .required_evidence_refs
                    .clone()
            },
            request,
            mutation_handoff: None,
        }
    }

    fn coordination_completion_state(
        state_version: u64,
        claim: &ClaimContract,
    ) -> CoordinationCompletionState {
        let mut completion = coordination_completion_document();
        completion
            .completion_contract
            .status
            .checked_at_state_version = state_version;
        completion.completion_contract.status.changed_by = claim.claim.claimant_agent_id.clone();
        completion.completion_contract.claim.claim_contract_ref =
            Some(RepoPath(claim.id.0.clone()));
        completion.completion_contract.claim.claim_expires_at =
            Some(claim.lease.expires_at.clone());
        CoordinationCompletionState {
            completion,
            applied_claim_id: StableId(claim.id.0.clone()),
        }
    }

    fn coordination_recovery_state(
        request_id: &str,
        claim_id: &str,
    ) -> CoordinationHealthRecoveryState {
        let mut recovery = coordination_recovery_document();
        recovery.health_recovery_contract.recovery.action = RecoveryAction::HandoffToDriver;
        recovery.health_recovery_contract.recovery.request_ref =
            Some(RepoPath(request_id.to_owned()));
        recovery.health_recovery_contract.recovery.claim_ref = Some(RepoPath(claim_id.to_owned()));
        CoordinationHealthRecoveryState {
            actor_agent_id: StableId("codex-main".to_owned()),
            recovery,
        }
    }

    fn episode_reference(name: &str, digest_byte: char) -> WorkflowContentAddressedReference {
        WorkflowContentAddressedReference {
            subject_ref: name.to_owned(),
            subject_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
        }
    }

    fn replacement_episode_document(
        release: WorkflowGovernanceReleaseIdentity,
    ) -> PostBuildVerifyEpisodeDocument {
        let build_verify_snapshot = episode_reference("build-verify/current", '1');
        let policy_references = [
            PostBuildVerifyPolicyRole::Readiness,
            PostBuildVerifyPolicyRole::ReadyRelease,
            PostBuildVerifyPolicyRole::RealityEvidence,
            PostBuildVerifyPolicyRole::ContextRecovery,
            PostBuildVerifyPolicyRole::EvolveProject,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, role)| PostBuildVerifyPolicyReference {
            role,
            policy_id: StableId(format!("policy.post-build-verify.{index}")),
            policy_ref: RepoPath(format!("contracts/policies/post-build-verify-{index}.yaml")),
        })
        .collect();
        let mut document = PostBuildVerifyEpisodeDocument {
            schema_version: POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION.to_owned(),
            post_build_verify_episode: PostBuildVerifyEpisode {
                episode_id: StableId("episode.release.target".to_owned()),
                generation: 1,
                previous_episode_digest: None,
                authority: PostBuildVerifyEpisodeAuthority::CandidateOnly,
                release_subject: release.clone(),
                build_verify_snapshot: build_verify_snapshot.clone(),
                rollback_baseline: PostBuildVerifyRollbackBaseline::BuildVerifySnapshot {
                    snapshot: build_verify_snapshot,
                },
                policy_references,
                deployment_observations: Vec::new(),
                operational_evidence: Vec::new(),
                feedback: Vec::new(),
                intake: Vec::new(),
                evolution: PostBuildVerifyEvolutionIdentity {
                    evolution_episode_id: StableId("evolution.release.target".to_owned()),
                    generation: 1,
                    release_digest: release.release_digest.clone(),
                    status: PostBuildVerifyEvolutionStatus::Dormant,
                    trigger: PostBuildVerifyEvolutionTrigger::PlannedFollowUp,
                    proposed_entry_phase: Phase::Plan,
                    continuity_subject: episode_reference("continuity/evolution", '2'),
                },
                continuity: PostBuildVerifyContinuityBinding {
                    context_recovery_subject: episode_reference("continuity/recovery", '3'),
                    next_action_ref: StableId("action.monitor-release".to_owned()),
                },
                episode_digest: String::new(),
            },
        };
        let digest = document.episode_digest().expect("episode canonicalizes");
        document.post_build_verify_episode.episode_digest = digest;
        assert!(document.validate().is_empty());
        document
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

    fn test_native_host_provenance(
        host_kind: RuntimeKind,
        interaction_kind: WorkflowBrokerHostInteractionKind,
        adapter_id: &str,
        issued_at_unix: u64,
        nonce: &str,
    ) -> WorkflowBrokerNativeHostProvenance {
        WorkflowBrokerNativeHostProvenance {
            host_kind,
            host_version: "0.12.0".to_owned(),
            adapter_id: StableId(adapter_id.to_owned()),
            adapter_version: "0.1.0".to_owned(),
            interaction_kind,
            host_event_ref: format!("host-event-{nonce}"),
            host_session_ref: format!("host-session-{nonce}"),
            host_interaction_ref: format!("host-interaction-{nonce}"),
            host_event_descriptor_digest: format!("sha256:{}", "0".repeat(64)),
            host_observed_at_unix: issued_at_unix,
        }
    }

    fn strict_test_host_binding() -> WorkflowBrokerHostBinding {
        WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "0.12.0".to_owned(),
            adapter_id: StableId("adapter.forge-standalone.kernel-test".to_owned()),
            adapter_version: "0.1.0".to_owned(),
            host_installation_id: StableId("host.installation.kernel-test".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        }
    }

    fn strict_test_event_credential(
        credential_id: &str,
        broker_id: &str,
        issuer_id: &str,
        profile: WorkflowBrokerCredentialProfile,
        operation: WorkflowBrokerBoundOperation,
        generation: u64,
        key: &SigningKey,
        not_before_unix: u64,
        enrollment_operation_id: &str,
    ) -> WorkflowBrokerPublicCredentialMetadata {
        WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId(credential_id.to_owned()),
            broker_id: StableId(broker_id.to_owned()),
            subject_id: StableId(issuer_id.to_owned()),
            purpose: WorkflowBrokerCredentialPurpose::EventIssuer,
            profile,
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: hex(key.verifying_key().as_bytes()),
            key_generation: generation,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
            host_binding: strict_test_host_binding(),
            allowed_operations: vec![operation],
            not_before_unix,
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId(enrollment_operation_id.to_owned()),
            revocation_operation_id: None,
        }
    }

    fn strict_test_registry(
        adapter: &WorkflowGovernanceProjectAdapter,
        human_key: &SigningKey,
        runtime_key: &SigningKey,
        now: u64,
    ) -> AuthorizedWorkflowBrokerControlPlane {
        let admin_key = SigningKey::from_bytes(&[53_u8; 32]);
        let not_before_unix = now.saturating_sub(600).max(1);
        let admin = WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId("credential.admin.1".to_owned()),
            broker_id: StableId("broker.admin.stable".to_owned()),
            subject_id: StableId("administrator.operator.test".to_owned()),
            purpose: WorkflowBrokerCredentialPurpose::RegistryAdministrator,
            profile: WorkflowBrokerCredentialProfile::Administrator,
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: hex(admin_key.verifying_key().as_bytes()),
            key_generation: 1,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
            host_binding: strict_test_host_binding(),
            allowed_operations: Vec::new(),
            not_before_unix,
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId("admin.operation.genesis".to_owned()),
            revocation_operation_id: None,
        };
        let human = strict_test_event_credential(
            "credential.human.1",
            "broker.human.stable",
            "broker.human.test",
            WorkflowBrokerCredentialProfile::Human,
            WorkflowBrokerBoundOperation::IntentRevision,
            1,
            human_key,
            not_before_unix,
            "admin.operation.genesis",
        );
        let runtime = strict_test_event_credential(
            "credential.runtime.1",
            "broker.runtime.stable",
            "broker.runtime.test",
            WorkflowBrokerCredentialProfile::Runtime,
            WorkflowBrokerBoundOperation::Signal,
            1,
            runtime_key,
            not_before_unix,
            "admin.operation.genesis",
        );
        let mut credentials = vec![admin, human, runtime];
        credentials.sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        let document = WorkflowBrokerPublicRegistryDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: adapter.expected_broker_audience(),
            project_id: adapter.binding.project_id.clone(),
            workflow_id: StableId("workflow.governance".to_owned()),
            registry_generation: 1,
            previous_registry_digest: None,
            required_event_schema_version: WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION.to_owned(),
            credentials,
        };
        let control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            document.clone(),
            &document.audience,
            &document.project_id,
            &document.workflow_id,
        )
        .expect("strict broker registry");
        let path = adapter.trusted_broker_registry_path();
        fs::create_dir_all(path.parent().expect("broker registry parent"))
            .expect("broker registry parent");
        fs::write(
            path,
            yaml_serde::to_string(&document).expect("strict broker registry YAML"),
        )
        .expect("strict broker registry");
        control
    }

    fn rotate_strict_runtime_registry(
        adapter: &WorkflowGovernanceProjectAdapter,
        current: &AuthorizedWorkflowBrokerControlPlane,
        replacement_key: &SigningKey,
        now: u64,
    ) -> AuthorizedWorkflowBrokerControlPlane {
        let operation_id = StableId("admin.operation.rotate.runtime.2".to_owned());
        let mut document = current.document().clone();
        document.registry_generation = 2;
        document.previous_registry_digest = Some(current.registry_digest().to_owned());
        let predecessor = document
            .credentials
            .iter_mut()
            .find(|credential| credential.credential_id.0 == "credential.runtime.1")
            .expect("runtime predecessor");
        predecessor.status = WorkflowBrokerCredentialStatus::Revoked;
        predecessor.revoked_at_unix = Some(now);
        predecessor.revocation_operation_id = Some(operation_id.clone());
        let mut replacement = strict_test_event_credential(
            "credential.runtime.2",
            "broker.runtime.stable",
            "broker.runtime.rotated",
            WorkflowBrokerCredentialProfile::Runtime,
            WorkflowBrokerBoundOperation::Signal,
            2,
            replacement_key,
            now,
            &operation_id.0,
        );
        replacement.predecessor_credential_id = Some(StableId("credential.runtime.1".to_owned()));
        document.credentials.push(replacement);
        document
            .credentials
            .sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        let control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            document.clone(),
            &document.audience,
            &document.project_id,
            &document.workflow_id,
        )
        .expect("rotated strict broker registry");
        fs::write(
            adapter.trusted_broker_registry_path(),
            yaml_serde::to_string(&document).expect("rotated strict broker registry YAML"),
        )
        .expect("rotated strict broker registry");
        control
    }

    fn strict_verification_context(
        adapter: &WorkflowGovernanceProjectAdapter,
        operation: WorkflowBrokerBoundOperation,
    ) -> WorkflowBrokerVerificationContext {
        WorkflowBrokerVerificationContext {
            audience: adapter.expected_broker_audience(),
            project_id: adapter.binding.project_id.clone(),
            workflow_id: StableId("workflow.governance".to_owned()),
            operation,
        }
    }

    fn seal_test_host_descriptor(envelope: &mut WorkflowBrokerEventEnvelope) {
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("native host provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("host descriptor digest");
    }

    #[test]
    fn native_host_replay_origin_is_stable_across_packet_and_nonce_changes() {
        let provenance = test_native_host_provenance(
            RuntimeKind::ForgeStandalone,
            WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
            "adapter.forge-standalone.replay-test",
            100,
            "fixed-native-interaction-0001",
        );
        let audit = VerifiedWorkflowBrokerEventAudit {
            issuer_id: StableId("broker.human.test".to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId("principal.human.origin".to_owned()),
            separation_domain: StableId("human.test.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::Decision,
            project_id: StableId("project.test".to_owned()),
            action_packet_digest: format!("sha256:{}", "1".repeat(64)),
            event_digest: format!("sha256:{}", "2".repeat(64)),
            public_key_fingerprint: format!("sha256:{}", "3".repeat(64)),
            signature_fingerprint: format!("sha256:{}", "4".repeat(64)),
            enrollment_ceremony_digest: format!("sha256:{}", "5".repeat(64)),
            replay_key: WorkflowBrokerReplayKey {
                issuer_id: StableId("broker.human.test".to_owned()),
                origin_principal_id: PrincipalId("principal.human.origin".to_owned()),
                separation_domain: StableId("human.test.session".to_owned()),
                project_id: StableId("project.test".to_owned()),
                nonce_fingerprint: format!("sha256:{}", "6".repeat(64)),
                event_digest: format!("sha256:{}", "2".repeat(64)),
            },
            native_host_provenance: Some(provenance),
            issued_at_unix: 100,
            expires_at_unix: 200,
        };
        let first_origin = broker_replay_origin_id(&audit).expect("v2 replay origin");
        let mut changed = audit.clone();
        changed.action_packet_digest = format!("sha256:{}", "7".repeat(64));
        changed.event_digest = format!("sha256:{}", "8".repeat(64));
        changed.replay_key.nonce_fingerprint = format!("sha256:{}", "9".repeat(64));
        changed.replay_key.event_digest = changed.event_digest.clone();
        let changed_origin =
            broker_replay_origin_id(&changed).expect("v2 replay origin after agent changes");
        assert_eq!(
            first_origin, changed_origin,
            "one native interaction must not authorize another packet by changing the nonce"
        );

        let (root, state) = temp_project("native-host-replay-origin");
        forge_core_store::workflow_action_replay::initialize_workflow_action_replay(&state)
            .expect("initialize replay store");
        forge_core_store::workflow_action_replay::reserve_workflow_action(
            &state,
            &audit.action_packet_digest,
            &first_origin,
            &format!("sha256:{}", "b".repeat(64)),
        )
        .expect("reserve native interaction for its first packet");
        assert!(matches!(
            forge_core_store::workflow_action_replay::reserve_workflow_action(
                &state,
                &changed.action_packet_digest,
                &changed_origin,
                &format!("sha256:{}", "c".repeat(64)),
            ),
            Err(
                forge_core_store::workflow_action_replay::WorkflowActionReplayError::OriginReplayConflict { .. }
            )
        ));

        let mut legacy = audit;
        legacy.native_host_provenance = None;
        let mut legacy_changed = legacy.clone();
        legacy_changed.replay_key.nonce_fingerprint = format!("sha256:{}", "a".repeat(64));
        assert_ne!(
            broker_replay_origin_id(&legacy).expect("v1 replay origin"),
            broker_replay_origin_id(&legacy_changed).expect("changed v1 replay origin"),
            "frozen v1 replay identity remains nonce-based"
        );
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup fixture");
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
            native_host_provenance: Some(test_native_host_provenance(
                RuntimeKind::ForgeStandalone,
                WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
                "adapter.forge-standalone.kernel-test",
                issued_at_unix,
                nonce,
            )),
            issued_at_unix,
            expires_at_unix: issued_at_unix + 120,
            nonce: nonce.to_owned(),
            signature: String::new(),
        };
        seal_test_host_descriptor(&mut envelope);
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
            native_host_provenance: Some(test_native_host_provenance(
                RuntimeKind::ForgeStandalone,
                WorkflowBrokerHostInteractionKind::AttestedRuntimeObservation,
                "adapter.forge-standalone.kernel-test",
                issued_at_unix,
                nonce,
            )),
            issued_at_unix,
            expires_at_unix: issued_at_unix + 120,
            nonce: nonce.to_owned(),
            signature: String::new(),
        };
        seal_test_host_descriptor(&mut envelope);
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
    fn coordination_dependencies_require_exact_typed_durable_ids() {
        let durable_request = coordination_request_state(RequestStatus::Pending);
        let durable_request_id = durable_request.request.request_contract.id.0.clone();
        let durable_request_schema = durable_request
            .request
            .request_contract
            .contract_ref
            .0
            .clone();
        let ledger = coordination_projection(vec![coordination_state_record(
            1,
            1,
            &format!("sha256:{}", "0".repeat(64)),
            0,
            CoordinationStateRecord::Request(durable_request),
        )]);
        let mut claim = coordination_claim_document().claim_contract;
        claim.lease.expires_at = "2026-06-25T00:30:00Z".to_owned();
        let claim_id = claim.id.0.clone();
        let claim_schema = claim.contract_ref.0.clone();
        let claims = claim_projection(vec![(claim, true)]);
        let index = coordination_reference_fixture();
        let now = rfc3339_to_unix("2026-06-25T00:10:00Z").expect("fixed clock");

        let mut dependent = coordination_request_state(RequestStatus::Pending);
        dependent.request.request_contract.payload.dependency_refs = vec![DependencyRef {
            kind: DependencyKind::Request,
            reference: durable_request_id,
        }];
        assert!(validate_request_coordination(&dependent, &ledger, &claims, &index, now).is_ok());

        dependent.request.request_contract.payload.dependency_refs[0].reference =
            durable_request_schema;
        assert!(matches!(
            validate_request_coordination(&dependent, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        dependent.request.request_contract.payload.dependency_refs = vec![DependencyRef {
            kind: DependencyKind::Claim,
            reference: claim_id,
        }];
        assert!(validate_request_coordination(&dependent, &ledger, &claims, &index, now).is_ok());

        dependent.request.request_contract.payload.dependency_refs[0].reference = claim_schema;
        assert!(matches!(
            validate_request_coordination(&dependent, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        dependent.request.request_contract.payload.dependency_refs = vec![DependencyRef {
            kind: DependencyKind::Gate,
            reference: "contracts/effects/story-artifact-write-effect.yaml".to_owned(),
        }];
        assert!(matches!(
            validate_request_coordination(&dependent, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));
    }

    fn legacy_profile_projection(
        readiness_profile: Option<WorkflowReadinessProfile>,
        extra_event: Option<WorkflowGovernanceEvent>,
    ) -> WorkflowGovernanceLedgerProjection {
        let mut records = vec![coordination_record(
            1,
            0,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                source_ref: "/tmp/project with spaces".to_owned(),
                source_digest: format!("sha256:{}", "1".repeat(64)),
                snapshot_digest: format!("sha256:{}", "2".repeat(64)),
                initial_phase: StableId("1-discovery".to_owned()),
                readiness_profile,
            }),
        )];
        if let Some(event) = extra_event {
            let mut record = coordination_record(2, 1, event);
            record.previous_record_digest = Some(records[0].record_digest.clone());
            records.push(record);
        }
        let head_digest = records.last().map(|record| record.record_digest.clone());
        let next_sequence = u64::try_from(records.len())
            .expect("small test projection")
            .saturating_add(1);
        let next_state_version = records
            .last()
            .map_or(0, |record| record.state_version.saturating_add(1));
        WorkflowGovernanceLedgerProjection {
            records,
            head_digest,
            next_sequence,
            next_state_version,
        }
    }

    #[test]
    fn legacy_profile_status_publishes_exact_host_neutral_argv_with_spaced_root() {
        let projection = legacy_profile_projection(None, None);
        assert_eq!(
            projection.readiness_profile(),
            Some(WorkflowReadinessProfile::StrictExternal)
        );
        let snapshot = format!("sha256:{}", "a".repeat(64));
        let status = profile_status_projection(
            Path::new("/tmp/project with spaces"),
            &projection,
            snapshot.clone(),
        )
        .expect("legacy profile status");
        assert_eq!(
            status.solo_adoption,
            WorkflowLegacySoloAdoptionAvailability::Eligible
        );
        let argv = status.adopt_solo_argv.expect("exact adoption argv");
        assert_eq!(argv[0], "forge-core");
        assert_eq!(argv[1], "workflow");
        assert_eq!(argv[2], "profile");
        assert_eq!(argv[3], "adopt-solo");
        assert_eq!(argv[4], "--root");
        assert_eq!(argv[5], "/tmp/project with spaces");
        assert_eq!(argv[7], projection.head_digest.expect("head"));
        assert_eq!(argv[9], snapshot);
        assert_eq!(argv[10], "--json");
    }

    #[test]
    fn legacy_profile_status_distinguishes_existing_solo_strict_and_authority_history() {
        let strict = profile_status_projection(
            Path::new("/tmp/project"),
            &legacy_profile_projection(Some(WorkflowReadinessProfile::StrictExternal), None),
            format!("sha256:{}", "a".repeat(64)),
        )
        .expect("explicit strict status");
        assert_eq!(
            strict.solo_adoption,
            WorkflowLegacySoloAdoptionAvailability::Ineligible
        );
        assert!(strict.adopt_solo_argv.is_none());

        let solo = profile_status_projection(
            Path::new("/tmp/project"),
            &legacy_profile_projection(Some(WorkflowReadinessProfile::SoloCooperative), None),
            format!("sha256:{}", "a".repeat(64)),
        )
        .expect("explicit solo status");
        assert_eq!(
            solo.solo_adoption,
            WorkflowLegacySoloAdoptionAvailability::AlreadySolo
        );
        assert_eq!(
            solo.current_profile,
            WorkflowReadinessProfile::SoloCooperative
        );
        assert!(solo.adopt_solo_argv.is_none());

        let authority = WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
            from_phase: Some(StableId("1-discovery".to_owned())),
            to_phase: StableId("2-definition".to_owned()),
            snapshot_digest: format!("sha256:{}", "b".repeat(64)),
        });
        let status = profile_status_projection(
            Path::new("/tmp/project"),
            &legacy_profile_projection(None, Some(authority)),
            format!("sha256:{}", "a".repeat(64)),
        )
        .expect("authority-bearing status");
        assert_eq!(
            status.solo_adoption,
            WorkflowLegacySoloAdoptionAvailability::Ineligible
        );
        assert!(status.adopt_solo_argv.is_none());
    }

    #[test]
    fn coordination_request_deadlines_evidence_and_handoff_fail_closed() {
        let ledger = coordination_projection(Vec::new());
        let mut claim = coordination_claim_document().claim_contract;
        claim.lease.expires_at = "2026-06-25T00:30:00Z".to_owned();
        let claims = claim_projection(vec![(claim.clone(), true)]);
        let index = coordination_reference_fixture();
        let now = rfc3339_to_unix("2026-06-25T00:10:00Z").expect("fixed clock");

        let mut accepted = coordination_request_state(RequestStatus::Accepted);
        accepted.response_evidence_refs.clear();
        assert!(matches!(
            validate_coordination_kernel_state(
                &CoordinationStateRecord::Request(accepted),
                &ledger,
                &claims,
                &index,
                0,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut overdue = coordination_request_state(RequestStatus::Pending);
        overdue.request.request_contract.deadline = Some("2026-06-25T00:10:00Z".to_owned());
        assert!(matches!(
            validate_request_coordination(&overdue, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut early_expiration = coordination_request_state(RequestStatus::Expired);
        early_expiration.request.request_contract.deadline =
            Some("2026-06-25T00:20:00Z".to_owned());
        assert!(matches!(
            validate_request_coordination(&early_expiration, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut applied = coordination_request_state(RequestStatus::Applied);
        applied.mutation_handoff = Some(CoordinationMutationHandoff {
            driver_agent_id: applied.request.request_contract.target_driver.clone(),
            requested_operation: applied.request.request_contract.requested_operation.clone(),
            claim_contract_ref: RepoPath(claim.id.0.clone()),
            authority_refs: vec!["contracts/gates/story-ready-lane-gate.yaml".to_owned()],
            effect_contract_refs: vec![
                "contracts/effects/story-artifact-write-effect.yaml".to_owned()
            ],
        });
        assert!(validate_request_coordination(&applied, &ledger, &claims, &index, now).is_ok());

        applied
            .mutation_handoff
            .as_mut()
            .expect("handoff")
            .claim_contract_ref = claim.contract_ref.clone();
        assert!(matches!(
            validate_request_coordination(&applied, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        applied
            .mutation_handoff
            .as_mut()
            .expect("handoff")
            .claim_contract_ref = RepoPath(claim.id.0.clone());
        applied
            .mutation_handoff
            .as_mut()
            .expect("handoff")
            .effect_contract_refs = vec!["contracts/gates/story-ready-lane-gate.yaml".to_owned()];
        assert!(matches!(
            validate_request_coordination(&applied, &ledger, &claims, &index, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));
    }

    #[test]
    fn coordination_completion_requires_exact_live_claim_state_and_unique_task() {
        let mut claim = coordination_claim_document().claim_contract;
        claim.lease.expires_at = "2026-06-25T00:30:00Z".to_owned();
        let claims = claim_projection(vec![(claim.clone(), true)]);
        let now = rfc3339_to_unix("2026-06-25T00:10:00Z").expect("fixed clock");
        let state_version = 31;
        let valid = coordination_completion_state(state_version, &claim);
        let empty_ledger = coordination_projection(Vec::new());
        assert!(validate_completion_coordination(
            &valid,
            &empty_ledger,
            &claims,
            state_version,
            now,
        )
        .is_ok());

        let mut missing_claim = valid.clone();
        missing_claim
            .completion
            .completion_contract
            .claim
            .claim_contract_ref = None;
        assert!(matches!(
            validate_completion_coordination(
                &missing_claim,
                &empty_ledger,
                &claims,
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut schema_claim = valid.clone();
        schema_claim
            .completion
            .completion_contract
            .claim
            .claim_contract_ref = Some(claim.contract_ref.clone());
        assert!(matches!(
            validate_completion_coordination(
                &schema_claim,
                &empty_ledger,
                &claims,
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        assert!(matches!(
            validate_completion_coordination(
                &valid,
                &empty_ledger,
                &claims,
                state_version + 1,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut invalidated = valid.clone();
        invalidated
            .completion
            .completion_contract
            .invalidation
            .invalidated_by = Some(StableId("reviewer".to_owned()));
        assert!(matches!(
            validate_completion_coordination(
                &invalidated,
                &empty_ledger,
                &claims,
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut wrong_lease = valid.clone();
        wrong_lease
            .completion
            .completion_contract
            .claim
            .claim_expires_at = Some("2026-06-25T00:29:59Z".to_owned());
        assert!(matches!(
            validate_completion_coordination(
                &wrong_lease,
                &empty_ledger,
                &claims,
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let previous = coordination_state_record(
            1,
            state_version,
            &format!("sha256:{}", "0".repeat(64)),
            state_version - 1,
            CoordinationStateRecord::Completion(valid.clone()),
        );
        let ledger_with_completion = coordination_projection(vec![previous]);
        let mut conflicting = valid.clone();
        conflicting.completion.completion_contract.id =
            StableId("completion.conflicting".to_owned());
        assert!(matches!(
            validate_completion_coordination(
                &conflicting,
                &ledger_with_completion,
                &claims,
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut no_proof = valid;
        no_proof.completion.completion_contract.proof_refs.clear();
        assert!(matches!(
            validate_coordination_kernel_state(
                &CoordinationStateRecord::Completion(no_proof),
                &empty_ledger,
                &claims,
                &coordination_reference_fixture(),
                state_version,
                now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));
    }

    #[test]
    fn coordination_recovery_requires_reviewed_exact_durable_joins() {
        let request = coordination_request_state(RequestStatus::Pending);
        let request_id = request.request.request_contract.id.0.clone();
        let request_schema = request.request.request_contract.contract_ref.0.clone();
        let ledger = coordination_projection(vec![coordination_state_record(
            1,
            1,
            &format!("sha256:{}", "0".repeat(64)),
            0,
            CoordinationStateRecord::Request(request),
        )]);
        let mut claim = coordination_claim_document().claim_contract;
        claim.lease.expires_at = "2026-06-25T00:30:00Z".to_owned();
        let claim_id = claim.id.0.clone();
        let claims = claim_projection(vec![(claim.clone(), true)]);
        let now = rfc3339_to_unix("2026-06-25T00:10:00Z").expect("fixed clock");

        let valid = coordination_recovery_state(&request_id, &claim_id);
        assert!(validate_recovery_coordination(&valid, &ledger, &claims, now).is_ok());

        let mut request_schema_ref = valid.clone();
        request_schema_ref
            .recovery
            .health_recovery_contract
            .recovery
            .request_ref = Some(RepoPath(request_schema));
        assert!(matches!(
            validate_recovery_coordination(&request_schema_ref, &ledger, &claims, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut claim_schema_ref = valid.clone();
        claim_schema_ref
            .recovery
            .health_recovery_contract
            .recovery
            .claim_ref = Some(claim.contract_ref.clone());
        assert!(matches!(
            validate_recovery_coordination(&claim_schema_ref, &ledger, &claims, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut wrong_actor = valid.clone();
        wrong_actor.actor_agent_id = StableId("worker.other".to_owned());
        assert!(matches!(
            validate_recovery_coordination(&wrong_actor, &ledger, &claims, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut automatic = valid.clone();
        automatic
            .recovery
            .health_recovery_contract
            .recovery
            .automatic_allowed = true;
        automatic
            .recovery
            .health_recovery_contract
            .recovery
            .requires_review = false;
        assert!(matches!(
            validate_recovery_coordination(&automatic, &ledger, &claims, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let mut missing_refs = valid;
        missing_refs
            .recovery
            .health_recovery_contract
            .recovery
            .request_ref = None;
        assert!(matches!(
            validate_recovery_coordination(&missing_refs, &ledger, &claims, now),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));

        let expired_now = rfc3339_to_unix("2026-06-25T00:31:00Z").expect("fixed clock");
        assert!(matches!(
            validate_recovery_coordination(
                &coordination_recovery_state(&request_id, &claim_id),
                &ledger,
                &claims,
                expired_now,
            ),
            Err(WorkflowGovernanceAdapterError::CoordinationInvalid(_))
        ));
    }

    #[test]
    fn coordination_exact_retry_matches_state_and_original_cas() {
        let state =
            CoordinationStateRecord::Request(coordination_request_state(RequestStatus::Pending));
        let prior_head = format!("sha256:{}", "a".repeat(64));
        let record = coordination_state_record(1, 8, &prior_head, 7, state.clone());
        let projection = coordination_projection(vec![record.clone()]);

        assert_eq!(
            exact_coordination_retry(&projection, &state, &prior_head, 7),
            Some(&record)
        );
        assert!(exact_coordination_retry(&projection, &state, &prior_head, 8).is_none());
        assert!(exact_coordination_retry(
            &projection,
            &state,
            &format!("sha256:{}", "b".repeat(64)),
            7,
        )
        .is_none());
    }

    #[test]
    fn replacement_projection_recovers_latest_complete_authority_free_state() {
        let imported = coordination_record(
            1,
            0,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                source_ref: "project/root".to_owned(),
                source_digest: format!("sha256:{}", "1".repeat(64)),
                snapshot_digest: format!("sha256:{}", "2".repeat(64)),
                initial_phase: StableId("4-build-verify".to_owned()),
                readiness_profile: None,
            }),
        );
        let mut release = release_record(
            WorkflowReceiptCarryover::InvalidateAll,
            &format!("sha256:{}", "3".repeat(64)),
            &format!("sha256:{}", "4".repeat(64)),
        );
        let WorkflowGovernanceEvent::ReleaseUpgraded(release_event) = &release.event else {
            unreachable!();
        };
        let active_release = release_event.to_release.clone();
        release.sequence = 2;
        release.previous_record_digest = Some(imported.record_digest.clone());

        let first_document = replacement_episode_document(active_release.clone());
        let first_episode = PostBuildVerifyEpisodeAppliedEvent {
            episode_id: first_document.post_build_verify_episode.episode_id.clone(),
            generation: 1,
            previous_episode_digest: None,
            episode_digest: first_document
                .post_build_verify_episode
                .episode_digest
                .clone(),
            release_subject: active_release.clone(),
            decision_digest: format!("sha256:{}", "5".repeat(64)),
            from_phase: StableId("4-build-verify".to_owned()),
            to_phase: Some(StableId("5-ready-operate".to_owned())),
            outcome: PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate,
            snapshot_digest: first_document
                .post_build_verify_episode
                .build_verify_snapshot
                .subject_digest
                .clone(),
            prior_ledger_head_digest: release.record_digest.clone(),
            prior_state_version: release.state_version,
            admitted_gate: None,
            episode_snapshot: Some(first_document.clone()),
        };
        let first_episode_record = coordination_record(
            3,
            2,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(first_episode),
        );

        let mut second_document = first_document;
        let previous_episode_digest = second_document
            .post_build_verify_episode
            .episode_digest
            .clone();
        second_document.post_build_verify_episode.generation = 2;
        second_document
            .post_build_verify_episode
            .previous_episode_digest = Some(previous_episode_digest.clone());
        second_document
            .post_build_verify_episode
            .continuity
            .next_action_ref = StableId("action.review-feedback".to_owned());
        second_document.post_build_verify_episode.episode_digest = String::new();
        second_document.post_build_verify_episode.episode_digest = second_document
            .episode_digest()
            .expect("follow-on episode canonicalizes");
        assert!(second_document.validate().is_empty());
        let second_episode = PostBuildVerifyEpisodeAppliedEvent {
            episode_id: second_document.post_build_verify_episode.episode_id.clone(),
            generation: 2,
            previous_episode_digest: Some(previous_episode_digest),
            episode_digest: second_document
                .post_build_verify_episode
                .episode_digest
                .clone(),
            release_subject: active_release.clone(),
            decision_digest: format!("sha256:{}", "6".repeat(64)),
            from_phase: StableId("5-ready-operate".to_owned()),
            to_phase: None,
            outcome: PostBuildVerifyEpisodeOutcome::EvolutionTriageOpened,
            snapshot_digest: second_document
                .post_build_verify_episode
                .build_verify_snapshot
                .subject_digest
                .clone(),
            prior_ledger_head_digest: first_episode_record.record_digest.clone(),
            prior_state_version: first_episode_record.state_version,
            admitted_gate: None,
            episode_snapshot: Some(second_document.clone()),
        };
        let second_episode_record = coordination_record(
            4,
            3,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(second_episode),
        );

        let mut request = coordination_request_state(RequestStatus::Applied);
        let mut live_claim = coordination_claim_document().claim_contract;
        live_claim.lease.expires_at = "2026-06-25T00:30:00Z".to_owned();
        request.mutation_handoff = Some(CoordinationMutationHandoff {
            driver_agent_id: request.request.request_contract.target_driver.clone(),
            requested_operation: request.request.request_contract.requested_operation.clone(),
            claim_contract_ref: RepoPath(live_claim.id.0.clone()),
            authority_refs: vec!["contracts/gates/story-ready-lane-gate.yaml".to_owned()],
            effect_contract_refs: vec![
                "contracts/effects/story-artifact-write-effect.yaml".to_owned()
            ],
        });
        let request_id = request.request.request_contract.id.0.clone();
        let request_record = coordination_state_record(
            5,
            4,
            &second_episode_record.record_digest,
            second_episode_record.state_version,
            CoordinationStateRecord::Request(request),
        );
        let completion = coordination_completion_state(4, &live_claim);
        let task_id = completion
            .completion
            .completion_contract
            .task
            .task_id
            .0
            .clone();
        let completion_record = coordination_state_record(
            6,
            5,
            &request_record.record_digest,
            request_record.state_version,
            CoordinationStateRecord::Completion(completion),
        );
        let recovery = coordination_recovery_state(&request_id, &live_claim.id.0);
        let runtime_id = recovery
            .recovery
            .health_recovery_contract
            .runtime
            .agent_id
            .0
            .clone();
        let recovery_record = coordination_state_record(
            7,
            6,
            &completion_record.record_digest,
            completion_record.state_version,
            CoordinationStateRecord::HealthRecovery(recovery),
        );
        let projection = coordination_projection(vec![
            imported,
            release,
            first_episode_record,
            second_episode_record,
            request_record,
            completion_record,
            recovery_record,
        ]);

        let mut expired_claim = live_claim.clone();
        expired_claim.id = forge_core_contracts::ClaimId("claim.driver.expired".to_owned());
        expired_claim.lease.expires_at = "2026-06-25T00:05:00Z".to_owned();
        let mut non_active_claim = live_claim.clone();
        non_active_claim.id = forge_core_contracts::ClaimId("claim.driver.released".to_owned());
        let claims = claim_projection(vec![
            (live_claim.clone(), true),
            (expired_claim.clone(), true),
            (non_active_claim.clone(), false),
        ]);
        let now = rfc3339_to_unix("2026-06-25T00:10:00Z").expect("fixed clock");
        let replacement =
            project_replacement_continuity(&projection, &claims, now).expect("replacement state");

        assert_eq!(replacement.active_release, active_release);
        assert_eq!(replacement.current_phase.0, "5-ready-operate");
        assert_eq!(replacement.state_version, 6);
        assert_eq!(replacement.requests_by_id.len(), 1);
        assert!(replacement.requests_by_id.contains_key(&request_id));
        assert!(replacement.completions_by_task_id.contains_key(&task_id));
        assert!(replacement
            .health_recovery_by_runtime_id
            .contains_key(&runtime_id));
        let episode = replacement
            .episodes_by_id
            .get(&replacement.active_episode_id.0)
            .expect("active episode");
        assert_eq!(episode.document.post_build_verify_episode.generation, 2);
        assert_eq!(episode.state_version, 3);
        assert_eq!(
            replacement.claims_by_id[&live_claim.id.0].liveness,
            ReplacementClaimLiveness::Live
        );
        assert_eq!(
            replacement.claims_by_id[&expired_claim.id.0].liveness,
            ReplacementClaimLiveness::Expired
        );
        assert_eq!(
            replacement.claims_by_id[&non_active_claim.id.0].liveness,
            ReplacementClaimLiveness::NonActive
        );

        let serialized = serde_json::to_string(&replacement).expect("serialize projection");
        for forbidden in [
            "retained_authority",
            "mutation_authority",
            "phase_authority",
            "release_authority",
            "signing_key",
            "private_key",
            "selected_host",
        ] {
            assert!(!serialized.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn replacement_projection_rejects_historical_summary_without_snapshot() {
        let imported = coordination_record(
            1,
            0,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                source_ref: "project/root".to_owned(),
                source_digest: format!("sha256:{}", "1".repeat(64)),
                snapshot_digest: format!("sha256:{}", "2".repeat(64)),
                initial_phase: StableId("4-build-verify".to_owned()),
                readiness_profile: None,
            }),
        );
        let release = release_record(
            WorkflowReceiptCarryover::InvalidateAll,
            &format!("sha256:{}", "3".repeat(64)),
            &format!("sha256:{}", "4".repeat(64)),
        );
        let WorkflowGovernanceEvent::ReleaseUpgraded(release_event) = &release.event else {
            unreachable!();
        };
        let release_subject = release_event.to_release.clone();
        let document = replacement_episode_document(release_subject.clone());
        let summary = coordination_record(
            3,
            2,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(
                PostBuildVerifyEpisodeAppliedEvent {
                    episode_id: document.post_build_verify_episode.episode_id,
                    generation: document.post_build_verify_episode.generation,
                    previous_episode_digest: None,
                    episode_digest: document.post_build_verify_episode.episode_digest,
                    release_subject,
                    decision_digest: format!("sha256:{}", "5".repeat(64)),
                    from_phase: StableId("4-build-verify".to_owned()),
                    to_phase: Some(StableId("5-ready-operate".to_owned())),
                    outcome: PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate,
                    snapshot_digest: document
                        .post_build_verify_episode
                        .build_verify_snapshot
                        .subject_digest,
                    prior_ledger_head_digest: release.record_digest.clone(),
                    prior_state_version: release.state_version,
                    admitted_gate: None,
                    episode_snapshot: None,
                },
            ),
        );
        let projection = coordination_projection(vec![imported, release, summary]);

        assert!(matches!(
            project_replacement_continuity(&projection, &empty_claim_projection(), 0),
            Err(
                WorkflowGovernanceAdapterError::ReplacementContinuityUnavailable(
                    "no complete episode snapshot binds the active release"
                )
            )
        ));
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
                    readiness_profile: None,
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
    fn clean_rebase_plan_observation_leaves_no_placeholder_or_cleanup_debt() {
        let (root, state) = temp_project("clean-rebase-plan-observation");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.clean-rebase-observation".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");

        for _ in 0..4 {
            assert!(
                !adapter
                    .recover_pending_release_rebase()
                    .expect("observe absent rebase plan"),
                "an absent rebase plan has nothing to recover"
            );
        }

        assert!(
            !state.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH).exists(),
            "read-only rebase recovery must leave the plan absent"
        );
        assert!(
            crash_replace_residue_paths(&state).is_empty(),
            "read-only rebase recovery must not create placeholders or cleanup debt"
        );
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup");
    }

    #[test]
    fn resume_source_stays_on_existing_only_observer_paths() {
        let source = include_str!("adapter.rs");
        let start = source
            .find("    pub fn resume(")
            .expect("resume function source");
        let end = source[start..]
            .find("    fn replacement_continuity(")
            .map(|offset| start + offset)
            .expect("resume function boundary");
        let resume = &source[start..end];
        for forbidden in [
            "self.next()",
            "recover_pending_release_rebase",
            "reconcile_effective_epoch",
            "LockedWorkflowDomainPackContext::acquire(",
            "lock_workflow_governance_ledger_tcb",
        ] {
            assert!(
                !resume.contains(forbidden),
                "read-only resume must not use mutating path {forbidden}"
            );
        }
        assert!(resume.contains("LockedWorkflowDomainPackContext::acquire_existing"));
        assert!(resume.contains("observe_existing_workflow_governance_ledger"));
        assert!(resume.contains("require_effective_epoch_current"));
        assert!(resume.contains("snapshot.revalidate()"));
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
        assert_eq!(
            initialized.readiness_profile,
            WorkflowReadinessProfile::SoloCooperative
        );
        assert_eq!(initialized.current_phase, "1-discovery");
        assert_eq!(
            initialized.effective.core_runtime_bundle,
            initialized.effective.effective_runtime_bundle
        );
        assert!(initialized.effective.domain_pack_generation.is_none());
        assert!(!initialized.domain_pack_degraded);
        assert!(initialized.domain_pack_gaps.is_empty());
        let repeated = adapter.initialize().expect("idempotent initialization");
        assert_eq!(
            repeated.status,
            WorkflowGovernanceInitializationStatus::AlreadyInitialized
        );
        assert_eq!(repeated.readiness_profile, initialized.readiness_profile);
        assert_eq!(repeated.head_digest, initialized.head_digest);
        assert_eq!(repeated.state_version, initialized.state_version);
        assert!(matches!(
            adapter
                .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal)),
            Err(
                WorkflowGovernanceAdapterError::ReadinessProfileReconfiguration {
                    current: WorkflowReadinessProfile::SoloCooperative,
                    requested: WorkflowReadinessProfile::StrictExternal,
                }
            )
        ));
        assert_eq!(
            lock_workflow_governance_ledger_tcb(&state)
                .expect("ledger")
                .recover()
                .expect("projection")
                .records
                .len(),
            1,
            "repeated initialization must not append"
        );
        let next = adapter.next().expect("next");
        assert_eq!(next.readiness_profile, initialized.readiness_profile);
        assert_eq!(
            next.durable_assurance.status,
            WorkflowDurableAssuranceStatus::MissingObjective
        );
        assert_eq!(next.authorization.action_packets.len(), 1);
        assert_eq!(
            next.authorization.action_packets[0]
                .required_authority
                .approval_boundary,
            WorkflowAuthorizationApprovalBoundary::CooperativeSameOwner
        );
        let WorkflowAuthorizationInputContract::CooperativeObjective {
            input_encoding,
            variants,
            limits,
            command_argv_template,
            ..
        } = &next.authorization.action_packets[0].input_contract
        else {
            panic!("solo packet must expose its dedicated cooperative contract");
        };
        assert_eq!(input_encoding, "utf8_json_file");
        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.variant.as_str())
                .collect::<Vec<_>>(),
            ["unambiguous", "decision_required"]
        );
        assert_eq!(limits.input_max_bytes, MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES);
        assert_eq!(
            command_argv_template,
            &[
                "forge-core",
                "workflow",
                "intent",
                "accept-cooperative",
                "--root",
                "<project-root>",
                "--packet-digest",
                "<packet-digest>",
                "--input-file",
                "<temporary-utf8-json-path>",
                "--json",
            ]
        );
        assert!(next.authorization.setup_gaps.is_empty());
        assert_eq!(
            adapter.resume().expect("resume").readiness_profile,
            initialized.readiness_profile
        );
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
        for _ in 0..3 {
            adapter.initialize().expect("repeated initialization");
            adapter.next().expect("repeated guidance");
            adapter.resume().expect("repeated resume");
        }
        assert!(
            crash_replace_residue_paths(&state).is_empty(),
            "read-only init/next/resume must not create crash-replace residue"
        );
        assert!(
            !state
                .join(forge_core_domain_pack_tcb::DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH)
                .exists(),
            "read-only workflow observation must leave absent Domain Pack pointer absent"
        );
    }

    fn cooperative_objective_input() -> WorkflowCooperativeObjectiveInput {
        WorkflowCooperativeObjectiveInput::Unambiguous {
            proposal: WorkflowCooperativeObjectiveProposal {
                outcome: "Make solo developer plus agent dogfooding reliable".to_owned(),
                constraints: vec!["Remain host neutral".to_owned()],
                unacceptable_outcomes: vec!["Claim verified human origin".to_owned()],
                open_uncertainties: vec!["Later multi-user authority model".to_owned()],
            },
            carrying_principal: PrincipalId("principal.agent.codex".to_owned()),
            host_provenance: WorkflowCooperativeHostProvenance {
                host_id: StableId("host.codex".to_owned()),
                host_version: "test".to_owned(),
                session_ref: "session.test".to_owned(),
                interaction_ref: "turn.test".to_owned(),
                conversation_digest: format!("sha256:{}", "a".repeat(64)),
                observed_at_unix: 1,
            },
        }
    }

    fn cooperative_material_supersession_input() -> WorkflowCooperativeObjectiveInput {
        WorkflowCooperativeObjectiveInput::MaterialSupersession {
            proposal: WorkflowCooperativeObjectiveProposal {
                outcome: "Make Forge excellent for solo developer dogfooding first".to_owned(),
                constraints: vec!["Remain host neutral".to_owned()],
                unacceptable_outcomes: vec!["Claim verified human origin".to_owned()],
                open_uncertainties: vec!["Later multi-user authority model".to_owned()],
            },
            supersession_reason: "The owner narrowed the near-term product direction".to_owned(),
            carrying_principal: PrincipalId("principal.agent.codex".to_owned()),
            host_provenance: WorkflowCooperativeHostProvenance {
                host_id: StableId("host.codex".to_owned()),
                host_version: "test".to_owned(),
                session_ref: "session.test".to_owned(),
                interaction_ref: "turn.material-correction".to_owned(),
                conversation_digest: format!("sha256:{}", "b".repeat(64)),
                observed_at_unix: 1,
            },
        }
    }

    fn cooperative_clarification_input() -> WorkflowCooperativeObjectiveInput {
        WorkflowCooperativeObjectiveInput::NonMaterialClarification {
            added_constraints: vec!["Keep per-ticket verification focused".to_owned()],
            added_unacceptable_outcomes: Vec::new(),
            added_open_uncertainties: vec!["Batch cadence remains adjustable".to_owned()],
            clarification_reason:
                "The owner clarified execution constraints without changing direction".to_owned(),
            carrying_principal: PrincipalId("principal.agent.codex".to_owned()),
            host_provenance: WorkflowCooperativeHostProvenance {
                host_id: StableId("host.codex".to_owned()),
                host_version: "test".to_owned(),
                session_ref: "session.test".to_owned(),
                interaction_ref: "turn.clarification".to_owned(),
                conversation_digest: format!("sha256:{}", "c".repeat(64)),
                observed_at_unix: 1,
            },
        }
    }

    fn cooperative_decision_input() -> WorkflowCooperativeObjectiveInput {
        WorkflowCooperativeObjectiveInput::DecisionRequired {
            decision_request: DecisionRequest {
                id: StableId("decision.objective-scope".to_owned()),
                question: "Should the objective include enterprise authority now?".to_owned(),
                reason: forge_core_contracts::HumanDecisionReason::ProductDirection,
                alternatives: vec![
                    DecisionAlternative {
                        id: StableId("solo-first".to_owned()),
                        description: "Keep the objective solo-first".to_owned(),
                        consequences: vec!["Enterprise authority stays deferred".to_owned()],
                    },
                    DecisionAlternative {
                        id: StableId("enterprise-now".to_owned()),
                        description: "Include enterprise authority now".to_owned(),
                        consequences: vec!["The objective becomes materially larger".to_owned()],
                    },
                ],
                recommended_alternative_ref: StableId("solo-first".to_owned()),
                blocking: true,
                blocks_before: ReadinessTarget::Execute,
            },
        }
    }

    #[test]
    fn cooperative_objective_material_supersession_and_additive_clarification_are_historical() {
        let (root, state) = temp_project("cooperative-objective-history");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-objective-history".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let initial = adapter.next().expect("initial objective packet");
        let initial_packet = initial.authorization.action_packets[0].clone();
        let first = adapter
            .accept_cooperative_objective(
                &initial_packet.packet_digest,
                cooperative_objective_input(),
            )
            .expect("accept initial objective");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            active_objective: first_active,
            next: first_next,
            ..
        } = first
        else {
            panic!("initial objective must be accepted");
        };
        let material_packet = first_next
            .authorization
            .objective_management_packet
            .clone()
            .expect("material revision packet");
        let material_input = cooperative_material_supersession_input();
        let material = adapter
            .accept_cooperative_objective(&material_packet.packet_digest, material_input.clone())
            .expect("accept material correction");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            active_objective: material_active,
            next: material_next,
            ..
        } = material
        else {
            panic!("material correction must be accepted");
        };
        assert_eq!(material_active.revision, 2);
        assert_eq!(material_active.assurance_epoch, 2);
        assert_eq!(
            material_active.previous_objective_digest.as_deref(),
            Some(first_active.objective_digest.as_str())
        );
        assert_eq!(
            material_active.revision_kind,
            WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession
        );
        assert_eq!(
            material_active.revision_reason.as_deref(),
            Some("The owner narrowed the near-term product direction")
        );
        assert!(matches!(
            adapter.accept_cooperative_objective(
                &initial_packet.packet_digest,
                cooperative_objective_input()
            ),
            Err(WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket)
        ));

        let clarification_packet = material_next
            .authorization
            .objective_management_packet
            .clone()
            .expect("clarification packet");
        let clarification_input = cooperative_clarification_input();
        let clarification = adapter
            .accept_cooperative_objective(
                &clarification_packet.packet_digest,
                clarification_input.clone(),
            )
            .expect("accept additive clarification");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            objective_record: clarification_record,
            active_objective: clarified,
            ..
        } = clarification
        else {
            panic!("clarification must be accepted");
        };
        assert_eq!(clarified.revision, 3);
        assert_eq!(clarified.assurance_epoch, 3);
        assert_eq!(clarified.proposal.outcome, material_active.proposal.outcome);
        assert!(clarified
            .proposal
            .constraints
            .starts_with(&material_active.proposal.constraints));
        assert!(clarified
            .proposal
            .constraints
            .contains(&"Keep per-ticket verification focused".to_owned()));
        assert_eq!(
            clarified.revision_kind,
            WorkflowCooperativeObjectiveRevisionKind::NonMaterialClarification
        );
        let exact_retry = adapter
            .accept_cooperative_objective(&clarification_packet.packet_digest, clarification_input)
            .expect("exact clarification retry");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            objective_record: retry_record,
            ..
        } = exact_retry
        else {
            panic!("exact clarification retry must remain accepted");
        };
        assert_eq!(retry_record, clarification_record);
        assert!(matches!(
            adapter.accept_cooperative_objective(&material_packet.packet_digest, material_input),
            Err(WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket)
        ));

        let recovered = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("recover history");
        let objectives = recovered
            .records
            .iter()
            .filter_map(|record| match &record.event {
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) => Some(event),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(objectives.len(), 3);
        assert_eq!(
            objectives[0].objective_digest,
            first_active.objective_digest
        );
        assert_eq!(
            objectives[2].previous_objective_digest.as_deref(),
            Some(objectives[1].objective_digest.as_str())
        );
        let replacement = adapter.resume().expect("replacement readback");
        let active = replacement
            .active_cooperative_objective
            .expect("active objective");
        assert_eq!(active.revision, 3);
        assert_eq!(
            active.revision_reason.as_deref(),
            Some("The owner clarified execution constraints without changing direction")
        );
    }

    #[test]
    fn agent_autonomy_is_bound_read_only_and_stales_after_objective_supersession() {
        let (root, state) = temp_project("agent-autonomy-boundary");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.agent-autonomy-boundary".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let missing = AgentAutonomyAssessmentInput {
            schema_version: forge_core_contracts::AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION
                .to_owned(),
            binding: AgentAutonomyBinding {
                objective_id: StableId("objective.missing".to_owned()),
                objective_revision: 1,
                objective_digest: format!("sha256:{}", "a".repeat(64)),
                assurance_epoch: 1,
                snapshot_digest: format!("sha256:{}", "b".repeat(64)),
                ledger_head_digest: format!("sha256:{}", "c".repeat(64)),
                state_version: 0,
            },
            work: AgentAutonomyWork::AgentOwned {
                class: AgentOwnedWorkClass::ResearchAndAnalysis,
                summary: "inspect the codebase".to_owned(),
            },
            effect: AgentAutonomyEffectDescriptor::LocalReadOnly,
        };
        assert!(matches!(
            adapter.assess_agent_autonomy(missing),
            Err(WorkflowGovernanceAdapterError::AgentAutonomyObjectiveRequired)
        ));

        let initial = adapter.next().expect("objective packet");
        assert_eq!(
            initial.agent_autonomy.status,
            WorkflowAgentAutonomyGuidanceStatus::ObjectiveRequired
        );
        let accepted = adapter
            .accept_cooperative_objective(
                &initial.authorization.action_packets[0].packet_digest,
                cooperative_objective_input(),
            )
            .expect("accept objective");
        let WorkflowCooperativeObjectiveAcceptance::Accepted { next, .. } = accepted else {
            panic!("objective accepted");
        };
        let binding = next
            .agent_autonomy
            .binding
            .clone()
            .expect("active autonomy binding");
        assert_eq!(binding.assurance_epoch, 1);
        assert_eq!(
            next.agent_autonomy.status,
            WorkflowAgentAutonomyGuidanceStatus::Active
        );
        assert_eq!(
            next.agent_autonomy.delegated_work_classes,
            AgentOwnedWorkClass::ALL
        );
        assert_eq!(
            next.agent_autonomy.human_decision_classes,
            HumanDecisionClass::ALL
        );
        assert_eq!(next.agent_autonomy.protected_effects, ProtectedEffect::ALL);
        assert!(!next.agent_autonomy.input_contract.unknown_fields_allowed);
        assert!(
            next.agent_autonomy
                .input_contract
                .temporary_input_must_be_outside_project_snapshot
        );
        assert_eq!(next.agent_autonomy.assessment_argv[0], "forge-core");

        let input = |work, effect| AgentAutonomyAssessmentInput {
            schema_version: forge_core_contracts::AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION
                .to_owned(),
            binding: binding.clone(),
            work,
            effect,
        };
        let before = state_file_bytes(&state);
        let local = adapter
            .assess_agent_autonomy(input(
                AgentAutonomyWork::AgentOwned {
                    class: AgentOwnedWorkClass::TacticFileOrderOrRetryChange,
                    summary: "retry a focused test using a different file order".to_owned(),
                },
                AgentAutonomyEffectDescriptor::LocalReversible,
            ))
            .expect("local autonomy");
        assert!(matches!(
            local,
            AgentAutonomyAssessment::ProceedAutonomously {
                class: AgentOwnedWorkClass::TacticFileOrderOrRetryChange,
                ..
            }
        ));
        let objective = adapter
            .assess_agent_autonomy(input(
                AgentAutonomyWork::HumanDecision {
                    class: HumanDecisionClass::ProductObjectiveChange,
                    summary: "expand the accepted product direction".to_owned(),
                },
                AgentAutonomyEffectDescriptor::LocalReadOnly,
            ))
            .expect("objective decision");
        assert!(matches!(
            objective,
            AgentAutonomyAssessment::DecisionRequired { .. }
        ));
        let publication = adapter
            .assess_agent_autonomy(input(
                AgentAutonomyWork::AgentOwned {
                    class: AgentOwnedWorkClass::ReversibleLocalEditing,
                    summary: "publish a public release".to_owned(),
                },
                AgentAutonomyEffectDescriptor::ProtectedEffect {
                    effect: ProtectedEffect::Publication,
                },
            ))
            .expect("publication decision");
        assert!(matches!(
            publication,
            AgentAutonomyAssessment::DecisionRequired { .. }
        ));
        assert_eq!(
            state_file_bytes(&state),
            before,
            "assessment must not write state"
        );

        let revision_packet = next
            .authorization
            .objective_management_packet
            .as_ref()
            .expect("revision packet");
        adapter
            .accept_cooperative_objective(
                &revision_packet.packet_digest,
                cooperative_material_supersession_input(),
            )
            .expect("supersede objective");
        let after_revision = state_file_bytes(&state);
        assert!(matches!(
            adapter.assess_agent_autonomy(input(
                AgentAutonomyWork::AgentOwned {
                    class: AgentOwnedWorkClass::TestingAndVerification,
                    summary: "run focused tests".to_owned(),
                },
                AgentAutonomyEffectDescriptor::LocalReadOnly,
            )),
            Err(WorkflowGovernanceAdapterError::AgentAutonomyEvaluation(
                AgentAutonomyEvaluationError::StaleBinding
            ))
        ));
        assert_eq!(
            state_file_bytes(&state),
            after_revision,
            "stale rejection must not write state"
        );
    }

    #[test]
    fn strict_profile_projects_autonomy_as_unsupported_and_assessment_fails_closed() {
        let (root, state) = temp_project("agent-autonomy-strict-profile");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.agent-autonomy-strict-profile".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("strict initialize");
        let next = adapter.next().expect("strict guidance");
        assert_eq!(
            next.agent_autonomy.status,
            WorkflowAgentAutonomyGuidanceStatus::UnsupportedProfile
        );
        assert!(next.agent_autonomy.binding.is_none());

        let input = AgentAutonomyAssessmentInput {
            schema_version: forge_core_contracts::AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION
                .to_owned(),
            binding: AgentAutonomyBinding {
                objective_id: StableId("objective.strict".to_owned()),
                objective_revision: 1,
                objective_digest: format!("sha256:{}", "a".repeat(64)),
                assurance_epoch: 1,
                snapshot_digest: next.snapshot_digest,
                ledger_head_digest: next.ledger_head_digest,
                state_version: next.state_version,
            },
            work: AgentAutonomyWork::AgentOwned {
                class: AgentOwnedWorkClass::ResearchAndAnalysis,
                summary: "inspect locally".to_owned(),
            },
            effect: AgentAutonomyEffectDescriptor::LocalReadOnly,
        };
        assert!(matches!(
            adapter.assess_agent_autonomy(input),
            Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired)
        ));
    }

    #[test]
    fn solo_objective_acceptance_is_durable_bound_and_ledger_derived() {
        let (root, state) = temp_project("cooperative-objective");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-objective".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let before = adapter.next().expect("objective packet");
        let packet = before.authorization.action_packets[0].clone();

        let accepted = adapter
            .accept_cooperative_objective(&packet.packet_digest, cooperative_objective_input())
            .expect("accept cooperative objective");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            objective_record,
            active_objective,
            next,
        } = accepted
        else {
            panic!("unambiguous objective must be accepted");
        };
        assert_eq!(
            active_objective.authority_basis,
            WorkflowCooperativeAuthorityBasis::CooperativeSameOwner
        );
        assert_eq!(
            active_objective.accepted_record_digest,
            objective_record.record_digest
        );
        assert_eq!(
            active_objective.snapshot_digest_at_acceptance,
            before.snapshot_digest
        );
        assert_eq!(
            active_objective.ledger_head_before_acceptance,
            before.ledger_head_digest
        );
        assert_eq!(
            active_objective.acceptance_action_packet_digest,
            packet.packet_digest
        );
        assert_eq!(
            next.durable_assurance.status,
            WorkflowDurableAssuranceStatus::ObjectiveAccepted
        );
        assert!(next.durable_assurance.projection.is_some());
        assert!(next.authorization.action_packets.is_empty());
        assert!(next.authorization.objective_management_packet.is_some());

        let fresh = adapter.next().expect("fresh ledger-derived guidance");
        assert_eq!(
            fresh
                .active_cooperative_objective
                .as_ref()
                .expect("active objective")
                .accepted_record_digest,
            objective_record.record_digest
        );
        let retry = adapter
            .accept_cooperative_objective(&packet.packet_digest, cooperative_objective_input())
            .expect("exact cooperative retry");
        let WorkflowCooperativeObjectiveAcceptance::Accepted {
            objective_record: retry_record,
            active_objective: retry_objective,
            ..
        } = retry
        else {
            panic!("exact retry must reproduce accepted receipt");
        };
        assert_eq!(retry_record, objective_record);
        assert_eq!(retry_objective, active_objective);
        let mut divergent = cooperative_objective_input();
        let WorkflowCooperativeObjectiveInput::Unambiguous { proposal, .. } = &mut divergent else {
            unreachable!();
        };
        proposal.outcome.push_str(" but with changed scope");
        assert!(matches!(
            adapter.accept_cooperative_objective(&packet.packet_digest, divergent),
            Err(WorkflowGovernanceAdapterError::CooperativeObjectiveRetryConflict)
        ));
        assert!(matches!(
            adapter.accept_cooperative_objective(
                &format!("sha256:{}", "f".repeat(64)),
                cooperative_objective_input()
            ),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        let projection = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("projection");
        assert_eq!(projection.records.len(), 2, "replay must not append");
        assert!(
            project_durable_assurance(&projection.records)
                .expect("strict projection")
                .is_none(),
            "same-owner authority must never satisfy strict external intent"
        );
    }

    #[test]
    fn cooperative_evidence_is_admitted_recovered_and_projected_honestly() {
        let (root, state) = temp_project("cooperative-evidence");
        let project_id = StableId("project.cooperative-evidence".to_owned());
        let adapter = WorkflowGovernanceProjectAdapter::new(project_id.clone(), &root, &state)
            .expect("adapter");
        adapter.initialize().expect("initialize");
        let objective_packet = adapter
            .next()
            .expect("objective guidance")
            .authorization
            .action_packets[0]
            .clone();
        let accepted = adapter
            .accept_cooperative_objective(
                &objective_packet.packet_digest,
                cooperative_objective_input(),
            )
            .expect("accept objective");
        let WorkflowCooperativeObjectiveAcceptance::Accepted { next, .. } = accepted else {
            panic!("objective must be accepted");
        };
        let packet = next
            .cooperative_evidence_action_packet
            .expect("host-neutral cooperative evidence packet");
        let route_policy_ref = packet.route.policy_ref.clone();
        let route_claim_ref = packet.route.claim_ref.clone();
        assert_eq!(
            packet.argv,
            [
                "forge-core",
                "workflow",
                "evidence",
                "admit-cooperative",
                "--root",
                ".",
                "--input-file",
                "${FORGE_COOPERATIVE_EVIDENCE_INPUT_FILE}",
                "--json",
            ]
        );
        assert!(packet.input_file_must_be_outside_project_snapshot);
        assert_eq!(
            packet.route.provider,
            WorkflowEvaluatorProvider::RepositoryInspector
        );
        assert_eq!(
            packet.route.source_provider,
            WorkflowEvaluatorProvider::AuthorizedHuman
        );
        assert_eq!(
            packet.route.assurance_effect,
            WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim
        );

        let mut offer_value = packet.offer_template;
        offer_value["offer_id"] = serde_json::json!("offer.cooperative-evidence.pass");
        let offer: WorkflowCooperativeEvidenceOffer =
            serde_json::from_value(offer_value).expect("closed offer template");
        let raw = serde_json::to_vec(&offer).expect("offer JSON");
        let admitted = adapter
            .record_cooperative_evidence(&raw)
            .expect("admit cooperative evidence");
        let WorkflowGovernanceEvent::CooperativeEvidenceObserved(admitted_event) = &admitted.event
        else {
            panic!("dedicated cooperative evidence event");
        };
        assert_eq!(
            admitted_event.disposition,
            WorkflowCooperativeEvidenceDisposition::Admitted
        );
        assert!(admitted_event.rejection.is_none());
        assert_eq!(
            admitted_event
                .admitted_evidence
                .as_ref()
                .expect("normalized admitted evidence")
                .outcome,
            WorkflowEvidenceOutcome::Pass
        );

        let second_packet = adapter
            .next()
            .expect("refreshed evidence guidance")
            .cooperative_evidence_action_packet
            .expect("refreshed host-neutral packet");
        assert_eq!(second_packet.route.policy_ref, route_policy_ref);
        assert_eq!(second_packet.route.claim_ref, route_claim_ref);
        let mut second_offer_value = second_packet.offer_template;
        second_offer_value["offer_id"] = serde_json::json!("offer.cooperative-evidence.pass.2");
        let second_offer: WorkflowCooperativeEvidenceOffer =
            serde_json::from_value(second_offer_value).expect("second closed offer template");
        let second_raw = serde_json::to_vec(&second_offer).expect("second offer JSON");
        adapter
            .record_cooperative_evidence(&second_raw)
            .expect("admit second current observation");

        assert_eq!(
            adapter
                .record_cooperative_evidence(&raw)
                .expect("idempotent retry"),
            admitted,
            "an exact retry must not append another event"
        );

        let projection = lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger")
            .recover()
            .expect("recovered evidence ledger");
        let registry = load_admitted_workflow_governance_universal_assurance_release_registry()
            .expect("admitted release registry");
        let release = adapter
            .resolve_active_release(&registry, &projection)
            .expect("active release");
        let domain = LockedWorkflowDomainPackContext::acquire(&root, &state)
            .expect("locked Domain Pack context");
        let effective = domain.admit_effective(release).expect("effective bundle");
        let snapshot =
            RetainedWorkflowProjectSnapshot::capture(&root).expect("current project snapshot");
        let now = unix_time().expect("clock");
        let derived = derive_receipts(
            effective.document(),
            &projection,
            &root,
            snapshot.digest(),
            now,
            None,
            None,
        )
        .expect("derive same-owner receipts");
        let claim_evidence = derived
            .evidence
            .iter()
            .filter(|entry| {
                entry.claim_ref == route_claim_ref
                    && entry.freshness == WorkflowEvidenceFreshness::Current
            })
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            claim_evidence.is_empty(),
            "cooperative audit evidence must never become a source-policy receipt"
        );
        let selected_policy = effective
            .document()
            .workflow_governance_bundle
            .policies
            .iter()
            .find(|policy| policy.id == route_policy_ref)
            .expect("selected source policy");
        let active_objective = active_cooperative_objective_from_ledger(&projection.records)
            .expect("active objective projection")
            .expect("accepted active objective");
        let selected_claim = selected_policy
            .claims
            .iter()
            .find(|claim| claim.id == route_claim_ref)
            .expect("selected source claim");
        let assert_tamper_stale = |mutate: fn(&mut WorkflowAdmittedCooperativeEvidence)| {
            let mut tampered = projection.records.clone();
            let evidence = tampered
                .iter_mut()
                .find(|record| record.record_digest == admitted.record_digest)
                .and_then(|record| {
                    let WorkflowGovernanceEvent::CooperativeEvidenceObserved(event) =
                        &mut record.event
                    else {
                        return None;
                    };
                    event.admitted_evidence.as_mut()
                })
                .expect("tamper admitted evidence");
            mutate(evidence);
            let audit = cooperative_evidence_audit(
                &tampered,
                selected_policy,
                Some(selected_claim),
                Some(&active_objective),
                &effective.identity().effective_runtime_bundle.bundle_digest,
                snapshot.digest(),
                now,
            );
            assert_eq!(
                audit
                    .iter()
                    .find(|entry| entry.record_digest == admitted.record_digest)
                    .expect("tampered audit entry")
                    .current_status,
                WorkflowCooperativeEvidenceCurrentStatus::Stale
            );
        };
        assert_tamper_stale(|evidence| {
            evidence.producer = PrincipalId("principal.tampered".to_owned());
        });
        assert_tamper_stale(|evidence| {
            evidence.scenario_digest = format!("sha256:{}", "f".repeat(64));
        });
        assert_tamper_stale(|evidence| {
            evidence.policy_ref = StableId("policy.tampered".to_owned());
        });
        assert_tamper_stale(|evidence| {
            evidence.readback_observed_at_unix = u64::MAX;
        });
        drop(effective);
        drop(domain);

        let replacement_adapter = WorkflowGovernanceProjectAdapter::new(project_id, &root, &state)
            .expect("fresh adapter");
        let recovered = replacement_adapter.next().expect("fresh-process guidance");
        let audit = recovered
            .cooperative_evidence
            .iter()
            .find(|entry| entry.record_digest == admitted.record_digest)
            .expect("durably recovered audit entry");
        assert_eq!(
            audit.current_status,
            WorkflowCooperativeEvidenceCurrentStatus::Supporting
        );
        assert_eq!(
            audit.proves,
            [
                WorkflowCooperativeEvidenceProof::SoloCooperativeClaimSatisfied,
                WorkflowCooperativeEvidenceProof::KernelExecutedProjectSnapshotScenario,
                WorkflowCooperativeEvidenceProof::KernelVerifiedProjectStateReadback,
            ]
        );
        assert_eq!(
            audit.does_not_satisfy_source_claim_ref.as_ref(),
            Some(&route_claim_ref)
        );
        assert!(audit
            .does_not_prove
            .contains(&WorkflowCooperativeEvidenceNonProof::IndependentSemanticReview));
        assert!(audit
            .does_not_prove
            .contains(&WorkflowCooperativeEvidenceNonProof::TrustedRuntimeSeparation));
        assert!(audit
            .does_not_prove
            .contains(&WorkflowCooperativeEvidenceNonProof::HumanPresence));
        assert!(audit
            .does_not_prove
            .contains(&WorkflowCooperativeEvidenceNonProof::EnterpriseCompliance));
        assert!(audit
            .does_not_prove
            .contains(&WorkflowCooperativeEvidenceNonProof::SelectedSourceClaim));

        let mut rejected_offer = adapter
            .next()
            .expect("current packet for rejection")
            .cooperative_evidence_action_packet
            .expect("cooperative packet")
            .offer_template;
        rejected_offer["offer_id"] =
            serde_json::json!("offer.cooperative-evidence.rejected-idempotency");
        rejected_offer["attestation"]["subject"]["kind"] = serde_json::json!("runtime");
        let rejected_raw = serde_json::to_vec(&rejected_offer).expect("rejected offer JSON");
        let first_rejection = adapter
            .record_cooperative_evidence(&rejected_raw)
            .expect("durably reject parseable offer");
        let WorkflowGovernanceEvent::CooperativeEvidenceObserved(first_rejected_event) =
            &first_rejection.event
        else {
            panic!("dedicated rejection event");
        };
        assert_eq!(
            first_rejected_event.offer_id.as_ref(),
            Some(&StableId(
                "offer.cooperative-evidence.rejected-idempotency".to_owned()
            ))
        );
        assert_eq!(
            adapter
                .record_cooperative_evidence(&rejected_raw)
                .expect("exact rejected retry"),
            first_rejection
        );

        let mut conflicting_offer = replacement_adapter
            .next()
            .expect("fresh-process packet after rejection")
            .cooperative_evidence_action_packet
            .expect("cooperative packet")
            .offer_template;
        conflicting_offer["offer_id"] =
            serde_json::json!("offer.cooperative-evidence.rejected-idempotency");
        let conflicting_raw =
            serde_json::to_vec(&conflicting_offer).expect("conflicting offer JSON");
        let conflict = replacement_adapter
            .record_cooperative_evidence(&conflicting_raw)
            .expect("durably reject conflicting reuse after restart");
        let WorkflowGovernanceEvent::CooperativeEvidenceObserved(conflict_event) = conflict.event
        else {
            panic!("dedicated conflict event");
        };
        assert_eq!(
            conflict_event.rejection,
            Some(WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey)
        );
        lock_workflow_governance_ledger_tcb(&state)
            .expect("ledger after conflicting rejected id")
            .recover()
            .expect("TCB recovers original and conflict id records");

        let mut inconclusive_offer = adapter
            .next()
            .expect("current packet for inconclusive rejection")
            .cooperative_evidence_action_packet
            .expect("cooperative packet")
            .offer_template;
        inconclusive_offer["offer_id"] =
            serde_json::json!("offer.cooperative-evidence.inconclusive");
        inconclusive_offer["attestation"]["outcome"] = serde_json::json!("inconclusive");
        let inconclusive = adapter
            .record_cooperative_evidence(
                &serde_json::to_vec(&inconclusive_offer).expect("inconclusive offer JSON"),
            )
            .expect("durably reject caller-supplied inconclusive outcome");
        let WorkflowGovernanceEvent::CooperativeEvidenceObserved(inconclusive_event) =
            inconclusive.event
        else {
            panic!("dedicated inconclusive rejection event");
        };
        assert_eq!(
            inconclusive_event.rejection,
            Some(WorkflowCooperativeEvidenceRejection::MalformedOrOversizedOffer)
        );
        assert!(inconclusive_event.admitted_evidence.is_none());

        let malformed = adapter
            .record_cooperative_evidence(b"{not-json")
            .expect("durably reject malformed offer");
        let WorkflowGovernanceEvent::CooperativeEvidenceObserved(rejected_event) = malformed.event
        else {
            panic!("dedicated rejection event");
        };
        assert_eq!(
            rejected_event.disposition,
            WorkflowCooperativeEvidenceDisposition::Rejected
        );
        assert_eq!(
            rejected_event.rejection,
            Some(WorkflowCooperativeEvidenceRejection::MalformedOrOversizedOffer)
        );
        assert!(rejected_event.admitted_evidence.is_none());
    }

    #[test]
    fn cooperative_decision_required_returns_without_ledger_write() {
        let (root, state) = temp_project("cooperative-decision");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-decision".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let packet = adapter
            .next()
            .expect("decision packet")
            .authorization
            .action_packets
            .into_iter()
            .next()
            .expect("cooperative packet");
        let before = fs::read(
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH),
        )
        .expect("ledger bytes");
        assert!(matches!(
            adapter
                .accept_cooperative_objective(&packet.packet_digest, cooperative_decision_input())
                .expect("typed decision"),
            WorkflowCooperativeObjectiveAcceptance::DecisionRequired { .. }
        ));
        assert_eq!(
            fs::read(
                state.join(
                    forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH,
                )
            )
            .expect("ledger bytes after"),
            before
        );
    }

    #[test]
    fn cooperative_decision_rejects_wrong_stale_strict_and_consumed_packets_without_write() {
        let (root, state) = temp_project("cooperative-decision-rejections");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-decision-rejections".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let packet = adapter
            .next()
            .expect("decision packet")
            .authorization
            .action_packets[0]
            .clone();
        let wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let before = fs::read(&wal).expect("WAL before wrong packet");
        assert!(matches!(
            adapter.accept_cooperative_objective(
                &format!("sha256:{}", "f".repeat(64)),
                cooperative_decision_input()
            ),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        assert_eq!(fs::read(&wal).expect("WAL after wrong packet"), before);

        fs::write(root.join("README.md"), "stale project snapshot\n").expect("stale project");
        assert!(matches!(
            adapter
                .accept_cooperative_objective(&packet.packet_digest, cooperative_decision_input()),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        assert_eq!(fs::read(&wal).expect("WAL after stale packet"), before);
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup stale fixture");

        let (strict_root, strict_state) = temp_project("cooperative-decision-strict");
        let strict = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-decision-strict".to_owned()),
            &strict_root,
            &strict_state,
        )
        .expect("strict adapter");
        strict
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict");
        let strict_guidance = strict.next().expect("strict packet");
        assert!(
            strict_guidance.cooperative_evidence_action_packet.is_none(),
            "strict external guidance must never offer the same-owner evidence lane"
        );
        let strict_packet = strict_guidance.authorization.action_packets[0]
            .packet_digest
            .clone();
        let strict_wal = strict_state
            .join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let strict_before = fs::read(&strict_wal).expect("strict WAL before");
        assert!(matches!(
            strict.accept_cooperative_objective(&strict_packet, cooperative_decision_input()),
            Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired)
        ));
        assert_eq!(
            fs::read(&strict_wal).expect("strict WAL after objective"),
            strict_before
        );
        assert!(matches!(
            strict.record_cooperative_evidence(b"{}"),
            Err(WorkflowGovernanceAdapterError::CooperativeObjectiveProfileRequired)
        ));
        assert_eq!(
            fs::read(&strict_wal).expect("strict WAL after evidence"),
            strict_before,
            "profile rejection must not append an evidence audit event"
        );
        fs::remove_dir_all(strict_root.parent().expect("strict fixture root"))
            .expect("cleanup strict fixture");

        let (accepted_root, accepted_state) = temp_project("cooperative-decision-consumed");
        let accepted = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-decision-consumed".to_owned()),
            &accepted_root,
            &accepted_state,
        )
        .expect("accepted adapter");
        accepted.initialize().expect("initialize accepted");
        let accepted_packet = accepted
            .next()
            .expect("accepted packet")
            .authorization
            .action_packets[0]
            .packet_digest
            .clone();
        accepted
            .accept_cooperative_objective(&accepted_packet, cooperative_objective_input())
            .expect("accept objective");
        let accepted_wal = accepted_state
            .join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let accepted_before = fs::read(&accepted_wal).expect("accepted WAL before decision");
        assert!(matches!(
            accepted.accept_cooperative_objective(&accepted_packet, cooperative_decision_input()),
            Err(WorkflowGovernanceAdapterError::CooperativeObjectiveRetryConflict)
        ));
        assert_eq!(
            fs::read(&accepted_wal).expect("accepted WAL after decision"),
            accepted_before
        );
        fs::remove_dir_all(accepted_root.parent().expect("accepted fixture root"))
            .expect("cleanup accepted fixture");
    }

    #[test]
    fn cooperative_objective_revalidates_project_immediately_before_ledger_commit() {
        let (root, state) = temp_project("cooperative-precommit-snapshot");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.cooperative-precommit-snapshot".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("initialize");
        let packet = adapter
            .next()
            .expect("cooperative packet")
            .authorization
            .action_packets[0]
            .packet_digest
            .clone();
        let wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let before = fs::read(&wal).expect("WAL before precommit drift");
        fs::write(
            state.join(TEST_CHANGE_PROJECT_BEFORE_COOPERATIVE_COMMIT_MARKER),
            b"README.md\n",
        )
        .expect("arm precommit drift");
        assert!(matches!(
            adapter.accept_cooperative_objective(&packet, cooperative_objective_input()),
            Err(WorkflowGovernanceAdapterError::RetainedProjectSnapshot(_))
        ));
        assert_eq!(
            fs::read(&wal).expect("WAL after precommit drift"),
            before,
            "project drift must fail before the cooperative ledger append"
        );
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup");
    }

    #[test]
    fn initialization_recovers_missing_action_replay_authority_before_early_return() {
        let (root, state) = temp_project("init-replay-recovery");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.replay-recovery".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        let initialized = adapter.initialize().expect("initialize ledger and replay");
        let wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let manifest = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_MANIFEST_RELATIVE_PATH,
        );
        fs::remove_file(&wal).expect("remove replay WAL");
        fs::remove_file(&manifest).expect("remove replay manifest");

        let recovered = adapter
            .initialize()
            .expect("existing ledger must recreate the absent replay pair");
        assert_eq!(
            recovered.status,
            WorkflowGovernanceInitializationStatus::AlreadyInitialized
        );
        assert_eq!(recovered.readiness_profile, initialized.readiness_profile);
        assert_eq!(recovered.head_digest, initialized.head_digest);
        assert_eq!(recovered.state_version, initialized.state_version);
        assert!(wal.is_file(), "replay WAL must be recreated");
        assert!(manifest.is_file(), "replay manifest must be recreated");
    }

    #[test]
    fn initialization_source_preserves_replay_then_domain_then_ledger_lock_order() {
        let source = include_str!("adapter.rs");
        let start = source
            .find("pub fn initialize_with_readiness_profile")
            .expect("initialization function");
        let end = source[start..]
            .find("    pub fn next(")
            .map(|offset| start + offset)
            .expect("next function boundary");
        let initialization = &source[start..end];
        let replay = initialization
            .find("initialize_workflow_action_replay")
            .expect("replay initialization");
        let domain = initialization
            .find("LockedWorkflowDomainPackContext::acquire")
            .expect("Domain Pack lock");
        let ledger = initialization
            .find("lock_workflow_governance_ledger_tcb")
            .expect("workflow ledger lock");
        assert!(
            replay < domain && domain < ledger,
            "initialization must validate replay, then retain Domain Pack authority, then acquire the workflow ledger"
        );
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");

        let missing = adapter.next().expect("missing-intent guidance");
        assert_eq!(
            missing.readiness_profile,
            WorkflowReadinessProfile::StrictExternal
        );
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
        assert_eq!(
            accepted.intent,
            forge_core_contracts::WorkflowObjectiveRevision::from(&first_event.intent)
        );
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
        assert_eq!(
            durable.intent,
            forge_core_contracts::WorkflowObjectiveRevision::from(&second_event.intent)
        );
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
    fn automatic_phase_advancement_stops_before_post_build_verify_routes() {
        assert_eq!(
            automatic_phase_successor(Phase::Discovery),
            Some(Phase::Specification)
        );
        assert_eq!(
            automatic_phase_successor(Phase::Specification),
            Some(Phase::Plan)
        );
        assert_eq!(
            automatic_phase_successor(Phase::Plan),
            Some(Phase::BuildVerify)
        );
        assert_eq!(automatic_phase_successor(Phase::BuildVerify), None);
        assert_eq!(automatic_phase_successor(Phase::ReadyOperate), None);
        assert_eq!(automatic_phase_successor(Phase::Evolve), None);
    }

    #[test]
    fn admitted_episode_projection_changes_phase_only_with_an_explicit_target() {
        let digest = |byte: char| format!("sha256:{}", byte.to_string().repeat(64));
        let record =
            |sequence: u64, event: WorkflowGovernanceEvent| WorkflowGovernanceLedgerRecord {
                record_id: StableId(format!("record.{sequence}")),
                sequence,
                project_id: StableId("project.test".to_owned()),
                bundle_id: StableId("bundle.test".to_owned()),
                bundle_digest: digest('a'),
                state_version: sequence - 1,
                previous_record_digest: (sequence > 1).then(|| digest('b')),
                record_digest: digest('c'),
                recorded_at_unix: 1,
                event,
            };
        let release = WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("lineage.test".to_owned()),
            release_id: StableId("release.test".to_owned()),
            release_version: "1.0.0".to_owned(),
            release_digest: digest('d'),
        };
        let imported = record(
            1,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                source_ref: "project/state.yaml".to_owned(),
                source_digest: digest('e'),
                snapshot_digest: digest('f'),
                initial_phase: StableId(Phase::BuildVerify.to_string()),
                readiness_profile: None,
            }),
        );
        let advanced = record(
            2,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(
                PostBuildVerifyEpisodeAppliedEvent {
                    episode_id: StableId("episode.ready".to_owned()),
                    generation: 1,
                    previous_episode_digest: None,
                    episode_digest: digest('1'),
                    release_subject: release.clone(),
                    decision_digest: digest('2'),
                    from_phase: StableId(Phase::BuildVerify.to_string()),
                    to_phase: Some(StableId(Phase::ReadyOperate.to_string())),
                    outcome: PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate,
                    snapshot_digest: digest('f'),
                    prior_ledger_head_digest: digest('c'),
                    prior_state_version: 0,
                    admitted_gate: Some(PostBuildVerifyAdmittedGateResult {
                        kind: PostBuildVerifyGateKind::Readiness,
                        status: GateStatus::Pass,
                        effective_bundle_digest: digest('3'),
                    }),
                    episode_snapshot: None,
                },
            ),
        );
        let follow_on = record(
            3,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(
                PostBuildVerifyEpisodeAppliedEvent {
                    episode_id: StableId("episode.rollback".to_owned()),
                    generation: 2,
                    previous_episode_digest: Some(digest('1')),
                    episode_digest: digest('4'),
                    release_subject: release,
                    decision_digest: digest('5'),
                    from_phase: StableId(Phase::ReadyOperate.to_string()),
                    to_phase: None,
                    outcome: PostBuildVerifyEpisodeOutcome::RollbackAssessmentOpened,
                    snapshot_digest: digest('f'),
                    prior_ledger_head_digest: digest('c'),
                    prior_state_version: 1,
                    admitted_gate: None,
                    episode_snapshot: None,
                },
            ),
        );
        let projection = WorkflowGovernanceLedgerProjection {
            records: vec![imported, advanced, follow_on],
            head_digest: Some(digest('c')),
            next_sequence: 4,
            next_state_version: 3,
        };

        assert_eq!(
            current_phase(&projection).expect("episode phase projection"),
            StableId(Phase::ReadyOperate.to_string())
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");
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
    fn boundary_rechecks_emit_cas_bound_packets_at_the_requested_target() {
        let (root, state) = temp_project("boundary-action-packets");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.boundary-action-packets".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");
        accept_test_intent(&adapter);

        let bundle: WorkflowGovernanceBundleDocument = yaml_serde::from_str(include_str!(
            "../../../../contracts/workflow-governance/golden-path-v0.yaml"
        ))
        .expect("golden path bundle");
        let mut guidance = adapter.next().expect("selected-policy guidance");
        let mut derived = DerivedReceipts::default();
        for policy_ref in [
            "policy.workflow.domain-scan",
            "policy.workflow.product-requirements",
        ] {
            let policy = policy_by_id(&bundle, &StableId(policy_ref.to_owned())).expect("policy");
            derived.completed_policy_refs.insert(policy.id.clone());
            derived
                .decision_need_refs
                .extend(policy.decision_rules.iter().map(|rule| rule.id.clone()));
        }
        guidance.boundary_rechecks = boundary_rechecks(
            &bundle,
            &derived,
            guidance.state_version,
            1_800_000_000,
            ReadinessTarget::Release,
        )
        .expect("boundary rechecks");
        assert_eq!(guidance.boundary_rechecks.len(), 2);
        assert!(guidance
            .boundary_rechecks
            .iter()
            .all(|boundary| boundary.requested_target == ReadinessTarget::Release));

        let principal_registry_digest = Some(format!("sha256:{}", "a".repeat(64)));
        let broker_registry_digest = Some(format!("sha256:{}", "b".repeat(64)));
        let packets = authorization_action_packets(
            &bundle,
            &guidance,
            &derived,
            None,
            principal_registry_digest.clone(),
            broker_registry_digest.clone(),
        )
        .expect("boundary packets");
        let boundary_packet = |kind, policy_ref: &str, subject_ref: &str| {
            packets
                .iter()
                .find(|packet| {
                    packet.authorization_kind == kind
                        && packet.binding.policy_ref.0 == policy_ref
                        && packet.binding.subject_ref.0 == subject_ref
                })
                .expect("boundary action packet")
        };

        let capability = boundary_packet(
            WorkflowAuthorizationKind::Capability,
            "policy.workflow.domain-scan",
            "capability.workflow.domain-scan.qualified-review",
        );
        assert!(matches!(
            &capability.input_contract,
            WorkflowAuthorizationInputContract::Capability {
                capability_ref,
                probe_kind: WorkflowCapabilityProbeKind::ExternalVerification,
                probe_reference_required: true,
                ..
            } if capability_ref.0 == "capability.workflow.domain-scan.qualified-review"
        ));

        let evidence = boundary_packet(
            WorkflowAuthorizationKind::Evidence,
            "policy.workflow.domain-scan",
            "claim.workflow.domain-scan.domain-risks-bounded",
        );
        assert!(matches!(
            &evidence.input_contract,
            WorkflowAuthorizationInputContract::Evidence {
                provider: WorkflowEvaluatorProvider::ExternalAuthority,
                evidence_kind: WorkflowEvidenceKind::ExternalAuthority,
                strength: WorkflowEvidenceStrength::AuthoritativeAcceptance,
                ..
            }
        ));
        assert_eq!(
            evidence.required_authority.approval_boundary,
            WorkflowAuthorizationApprovalBoundary::ExternalAuthorityBroker
        );
        assert_ne!(
            evidence.required_authority.approval_boundary,
            WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker
        );

        let decision = boundary_packet(
            WorkflowAuthorizationKind::Decision,
            "policy.workflow.product-requirements",
            "decision.workflow.product-requirements.product-direction",
        );
        assert!(matches!(
            &decision.input_contract,
            WorkflowAuthorizationInputContract::Decision {
                decision_ref,
                recommended_alternative_ref,
                ..
            } if decision_ref.0 == "decision.workflow.product-requirements.product-direction"
                && recommended_alternative_ref.0 == "alternative.preserve-intent"
        ));

        for packet in [capability, evidence, decision] {
            assert_eq!(packet.binding.readiness_target, ReadinessTarget::Release);
            assert_eq!(packet.binding.project_id, guidance.project_id);
            assert_eq!(packet.binding.state_version, guidance.state_version);
            assert_eq!(packet.binding.current_phase.0, guidance.current_phase);
            assert_eq!(packet.binding.snapshot_digest, guidance.snapshot_digest);
            assert_eq!(
                packet.binding.ledger_head_digest,
                guidance.ledger_head_digest
            );
            assert_eq!(
                packet.binding.trusted_principal_registry_digest,
                principal_registry_digest
            );
            assert_eq!(
                packet.binding.trusted_broker_registry_digest,
                broker_registry_digest
            );
            assert_eq!(
                packet.packet_digest,
                authorization_action_packet_digest(
                    &packet.schema_version,
                    &packet.packet_id,
                    packet.authorization_kind,
                    &packet.binding,
                    &packet.required_authority,
                    &packet.input_contract,
                )
                .expect("canonical packet digest")
            );
        }
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");

        let missing = adapter.next().expect("missing broker guidance");
        assert_eq!(
            missing.readiness_profile,
            WorkflowReadinessProfile::StrictExternal
        );
        assert!(missing.authorization.action_packets.iter().any(|packet| {
            packet.required_authority.approval_boundary
                == WorkflowAuthorizationApprovalBoundary::HumanApprovalBroker
        }));
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
                gap.external_setup,
                WorkflowBrokerExternalSetupState::Blocked {
                    reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
                }
            );
            assert!(gap.setup_argv.is_empty());
            assert_eq!(
                gap.required_operator_inputs,
                vec![
                    "selected_host_adapter".to_owned(),
                    "external_operator_trust_anchor".to_owned(),
                    "strict_registry_file".to_owned(),
                    "signed_native_admin_authorization".to_owned(),
                ]
            );
            let serialized = serde_json::to_string(gap).expect("gap JSON");
            for forbidden in [
                "private_key",
                "request-file",
                "attestation",
                "--issuer-id",
                "--public-key-file",
                "--ceremony-ref",
                "--ceremony-file",
            ] {
                assert!(
                    !serialized.contains(forbidden),
                    "obsolete setup field {forbidden}"
                );
            }
        }

        let key = SigningKey::from_bytes(&[31_u8; 32]);
        let mut document = install_runtime_broker_registry(&adapter, &key);
        let legacy = adapter.next().expect("legacy broker guidance");
        assert_eq!(
            legacy.authorization.registry_setup.broker_registry,
            WorkflowAuthorizationRegistrySetupStatus::LegacyRecoveryOnly
        );
        assert!(legacy.authorization.setup_gaps.iter().all(|gap| {
            gap.code == WorkflowAuthorizationSetupGapCode::BrokerRegistryLegacyRecoveryOnly
                && gap.external_setup
                    == (WorkflowBrokerExternalSetupState::Blocked {
                        reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
                    })
                && gap.setup_argv.is_empty()
        }));

        document.issuers[0].status = WorkflowBrokerIssuerStatus::Revoked;
        fs::write(
            adapter.trusted_broker_registry_path(),
            yaml_serde::to_string(&document).expect("revoked registry YAML"),
        )
        .expect("revoked registry");

        let revoked = adapter.next().expect("revoked broker guidance");
        assert_eq!(
            revoked.authorization.registry_setup.broker_registry,
            WorkflowAuthorizationRegistrySetupStatus::LegacyRecoveryOnly
        );
        assert!(!revoked.authorization.action_packets.is_empty());
        assert!(revoked
            .authorization
            .action_packets
            .iter()
            .all(|packet| { packet.binding.trusted_broker_registry_digest.is_some() }));
        assert!(revoked.authorization.setup_gaps.iter().all(|gap| {
            gap.code == WorkflowAuthorizationSetupGapCode::BrokerRegistryLegacyRecoveryOnly
                && gap.external_setup
                    == (WorkflowBrokerExternalSetupState::Blocked {
                        reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
                    })
                && gap.setup_argv.is_empty()
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile");
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
        let domain = LockedWorkflowDomainPackContext::acquire(&root, &state).expect("domain");
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
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
        let mut native_tuple_replay = signed_signal_envelope(
            &next_packets.project_id,
            next_signal,
            &key,
            now,
            "broker-response-loss-nonce-0002",
        );
        let original_provenance = envelope
            .native_host_provenance
            .as_ref()
            .expect("original native provenance");
        let replay_provenance = native_tuple_replay
            .native_host_provenance
            .as_mut()
            .expect("replay native provenance");
        replay_provenance.host_event_ref = original_provenance.host_event_ref.clone();
        replay_provenance.host_session_ref = original_provenance.host_session_ref.clone();
        replay_provenance.host_interaction_ref = original_provenance.host_interaction_ref.clone();
        seal_test_host_descriptor(&mut native_tuple_replay);
        let signing_bytes = workflow_broker_event_signing_bytes(&native_tuple_replay)
            .expect("native tuple replay signing bytes");
        native_tuple_replay.signature = hex(&key.sign(&signing_bytes).to_bytes());
        assert_ne!(native_tuple_replay.nonce, envelope.nonce);
        assert_ne!(
            native_tuple_replay
                .native_host_provenance
                .as_ref()
                .expect("replay provenance")
                .host_event_descriptor_digest,
            original_provenance.host_event_descriptor_digest
        );

        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let replay_wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let workflow_before_conflict =
            fs::read(&workflow_wal).expect("workflow WAL before native tuple conflict");
        let replay_before_conflict =
            fs::read(&replay_wal).expect("replay WAL before native tuple conflict");
        assert!(matches!(
            adapter.apply_verified_broker_action(
                verify_broker_envelope(&broker_document, native_tuple_replay, now),
                now,
            ),
            Err(WorkflowGovernanceAdapterError::ActionReplay(
                WorkflowActionReplayError::OriginReplayConflict { .. }
            ))
        ));
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after native tuple conflict"),
            workflow_before_conflict,
            "native tuple reuse must fail before governance append"
        );
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after native tuple conflict"),
            replay_before_conflict,
            "native tuple conflict must leave replay WAL byte-identical"
        );

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
        let historical = AuthorizedWorkflowBrokerRegistry::from_document(revoked_document.clone())
            .expect("retained revoked broker key")
            .verify_event_for_recovery(envelope.clone(), &packets.project_id)
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

        let release_registry =
            load_admitted_workflow_governance_universal_assurance_release_registry()
                .expect("release registry");
        let mut ledger = lock_workflow_governance_ledger_tcb(&state).expect("ledger for drift");
        let projection = ledger.recover().expect("projection before drift");
        let admitted = adapter
            .resolve_active_release(&release_registry, &projection)
            .expect("active release");
        let from_effective = projection.active_effective_bundle_identity().unwrap_or(
            derive_core_only_workflow_effective_identity(admitted).expect("core identity"),
        );
        let mut to_effective = from_effective.clone();
        to_effective.domain_pack_generation =
            Some(forge_core_contracts::WorkflowDomainPackGenerationIdentity {
                generation: 1,
                active_lock_digest: sha256_content_hash(b"historical-drift-active-lock"),
                composition_digest: sha256_content_hash(b"historical-drift-composition"),
                base_core_bundle_digest: from_effective.core_runtime_bundle.bundle_digest.clone(),
                supply_chain_registry_digest: sha256_content_hash(b"historical-drift-supply-chain"),
                reviewer_registry_digest: "1".repeat(64),
                reviewed_registry_digest: "2".repeat(64),
            });
        to_effective.receipt_context_digest =
            sha256_content_hash(b"historical-drift-receipt-context");
        let head = projection.head_digest.clone().expect("head before drift");
        let identity = adapter.identity(admitted);
        ledger
            .transition_domain_pack_generation_unchecked_tcb(
                &head,
                &identity,
                projection.next_state_version,
                forge_core_contracts::DomainPackGenerationTransitionedEvent {
                    from_effective_bundle: from_effective,
                    to_effective_bundle: to_effective,
                    receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
                    prior_ledger_head_digest: head.clone(),
                },
            )
            .expect("advance ledger effective epoch");
        drop(ledger);

        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let workflow_before = fs::read(&workflow_wal).expect("workflow WAL before drift refusal");
        let replay_before = fs::read(&replay.wal_path).expect("replay WAL before drift refusal");
        let historical_after_drift =
            AuthorizedWorkflowBrokerRegistry::from_document(revoked_document)
                .expect("historical registry after drift")
                .verify_event_for_recovery(envelope, &packets.project_id)
                .expect("historical event after drift");
        assert!(matches!(
            adapter.recover_historically_verified_broker_action(historical_after_drift),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after drift refusal"),
            workflow_before,
            "historical recovery must never append an effective-epoch reconciliation"
        );
        assert_eq!(
            fs::read(&replay.wal_path).expect("replay WAL after drift refusal"),
            replay_before,
            "effective-epoch drift must refuse before replay repair"
        );
    }

    #[test]
    fn broker_action_repairs_replay_append_failure_after_authoritative_ledger() {
        let (root, state) = temp_project("broker-replay-append-failure");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
        accept_test_intent(&adapter);
        let key = SigningKey::from_bytes(&[31_u8; 32]);
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
            "broker-replay-append-failure-nonce-0001",
        );
        let verified = verify_broker_envelope(&broker_document, envelope.clone(), now);
        let audit = verified.audit().clone();
        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let replay_wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let workflow_before = fs::read(&workflow_wal).expect("workflow WAL before failure");
        let replay_before = fs::read(&replay_wal).expect("replay WAL before failure");
        fs::write(state.join(TEST_REPLAY_APPEND_FAILURE_MARKER), b"fail\n")
            .expect("arm replay append failpoint");

        assert!(matches!(
            adapter.apply_verified_broker_action(verified, now),
            Err(WorkflowGovernanceAdapterError::ActionReplay(
                WorkflowActionReplayError::WriteWal { .. }
            ))
        ));
        assert_ne!(
            fs::read(&workflow_wal).expect("workflow WAL after failure"),
            workflow_before,
            "authoritative action and origin companion must be durable before replay append"
        );
        assert!(
            replay_wal.is_dir(),
            "failpoint must reach the actual replay append with the WAL path blocked"
        );

        let replay_backup = state.join(TEST_REPLAY_APPEND_FAILURE_BACKUP);
        fs::remove_dir(&replay_wal).expect("remove blocking replay directory");
        fs::rename(&replay_backup, &replay_wal).expect("restore original replay WAL");
        fs::remove_file(state.join(TEST_REPLAY_APPEND_FAILURE_MARKER))
            .expect("disarm replay append failpoint");
        assert_eq!(
            fs::read(&replay_wal).expect("restored replay WAL"),
            replay_before,
            "failed first replay append must preserve the prior replay WAL bytes"
        );
        let next_packets = adapter
            .action_packets_at(now)
            .expect("packets after durable action");
        let next_packet = next_packets
            .packets
            .iter()
            .find(|packet| {
                matches!(
                    packet.input_contract,
                    WorkflowAuthorizationInputContract::Signal {
                        transition: WorkflowSignalInputTransition::Deactivate,
                        ..
                    }
                )
            })
            .expect("deactivation signal packet");
        let mut conflicting_envelope = signed_signal_envelope(
            &next_packets.project_id,
            next_packet,
            &key,
            now,
            "broker-replay-append-failure-nonce-0002",
        );
        let durable_provenance = envelope
            .native_host_provenance
            .as_ref()
            .expect("durable native provenance");
        let conflicting_provenance = conflicting_envelope
            .native_host_provenance
            .as_mut()
            .expect("conflicting native provenance");
        conflicting_provenance.host_event_ref = durable_provenance.host_event_ref.clone();
        conflicting_provenance.host_session_ref = durable_provenance.host_session_ref.clone();
        conflicting_provenance.host_interaction_ref =
            durable_provenance.host_interaction_ref.clone();
        seal_test_host_descriptor(&mut conflicting_envelope);
        let signing_bytes = workflow_broker_event_signing_bytes(&conflicting_envelope)
            .expect("conflicting envelope signing bytes");
        conflicting_envelope.signature = hex(&key.sign(&signing_bytes).to_bytes());
        assert_ne!(conflicting_envelope.nonce, envelope.nonce);
        assert_ne!(
            conflicting_envelope.action_packet_digest,
            envelope.action_packet_digest
        );
        let workflow_before_conflict =
            fs::read(&workflow_wal).expect("workflow WAL before ledger-native conflict");
        let replay_before_conflict =
            fs::read(&replay_wal).expect("replay WAL before ledger-native conflict");
        assert!(matches!(
            adapter.apply_verified_broker_action(
                verify_broker_envelope(&broker_document, conflicting_envelope, now),
                now,
            ),
            Err(WorkflowGovernanceAdapterError::ActionReplay(
                WorkflowActionReplayError::OriginReplayConflict { .. }
            ))
        ));
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after ledger-native conflict"),
            workflow_before_conflict,
            "durable native tuple conflict must fail before another governance append"
        );
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after ledger-native conflict"),
            replay_before_conflict,
            "ledger companion conflict detection must not fabricate replay state"
        );

        let ledger = lock_workflow_governance_ledger_tcb(&state).expect("ledger after failure");
        let projection = ledger.recover().expect("recover durable companions");
        let (durable_action, durable_origin) =
            matching_broker_origin_retry(&projection, &audit, None, None)
                .expect("match durable broker origin")
                .expect("ledger commit must survive replay failure");
        drop(ledger);
        let historical = AuthorizedWorkflowBrokerRegistry::from_document(broker_document.clone())
            .expect("historical registry")
            .verify_event_for_recovery(envelope.clone(), &packets.project_id)
            .expect("historically verified event");
        let repaired = adapter
            .recover_historically_verified_broker_action(historical)
            .expect("repair replay from durable companion truth");
        assert_eq!(repaired.action_record, durable_action);
        assert_eq!(repaired.origin_record, durable_origin);
        assert!(repaired.replay_commit_repaired);
        let replay_after_repair = fs::read(&replay_wal).expect("repaired replay WAL");
        assert_ne!(replay_after_repair, replay_before);
        let recovered_replay =
            forge_core_store::workflow_action_replay::recover_workflow_action_replay(&state)
                .expect("replay recovery after repair");
        assert!(recovered_replay
            .entries
            .values()
            .all(|entry| entry.state == WorkflowActionReplayState::Committed));

        let exact_retry = AuthorizedWorkflowBrokerRegistry::from_document(broker_document)
            .expect("retry registry")
            .verify_event_for_recovery(envelope, &packets.project_id)
            .expect("exact historical retry");
        let retried = adapter
            .recover_historically_verified_broker_action(exact_retry)
            .expect("idempotent exact retry");
        assert_eq!(retried.action_record, durable_action);
        assert_eq!(retried.origin_record, durable_origin);
        assert!(!retried.replay_commit_repaired);
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after exact retry"),
            replay_after_repair,
            "exact retry after repair must not append again"
        );
    }

    #[test]
    fn strict_replay_digest_blocks_rotated_reuse_after_post_ledger_crash() {
        let (root, state) = temp_project("strict-replay-crash-rotation");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
        let human_key = SigningKey::from_bytes(&[61_u8; 32]);
        let runtime_key = SigningKey::from_bytes(&[62_u8; 32]);
        let replacement_key = SigningKey::from_bytes(&[63_u8; 32]);
        let now = unix_time().expect("clock");
        let strict = strict_test_registry(&adapter, &human_key, &runtime_key, now);
        let intent_packets = adapter.action_packets_at(now).expect("intent packet set");
        let intent_envelope = signed_intent_envelope(
            &intent_packets.project_id,
            &intent_packets.packets[0],
            &human_key,
            now,
            "strict-intent-native-interaction-0001",
            "Build a dependable governed product",
        );
        let intent_context =
            strict_verification_context(&adapter, WorkflowBrokerBoundOperation::IntentRevision);
        let verified_intent = strict
            .verify_bound_event(
                intent_envelope,
                &intent_context,
                i64::try_from(now).expect("clock fits i64"),
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("strict intent event");
        adapter
            .apply_verified_bound_broker_action(verified_intent, now)
            .expect("apply strict intent");

        let packets = adapter.action_packets_at(now).expect("signal packets");
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
            .expect("activation signal packet");
        let envelope = signed_signal_envelope(
            &packets.project_id,
            packet,
            &runtime_key,
            now,
            "strict-runtime-native-interaction-0001",
        );
        let signal_context =
            strict_verification_context(&adapter, WorkflowBrokerBoundOperation::Signal);
        let verified = strict
            .verify_bound_event(
                envelope.clone(),
                &signal_context,
                i64::try_from(now).expect("clock fits i64"),
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("strict signal event");
        let bound_audit = verified.audit().clone();
        let event_audit = verified.verified().audit().clone();
        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let replay_wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let replay_before = fs::read(&replay_wal).expect("replay WAL before strict failure");
        fs::write(state.join(TEST_REPLAY_APPEND_FAILURE_MARKER), b"fail\n")
            .expect("arm strict replay append failpoint");

        assert!(matches!(
            adapter.apply_verified_bound_broker_action(verified, now),
            Err(WorkflowGovernanceAdapterError::ActionReplay(
                WorkflowActionReplayError::WriteWal { .. }
            ))
        ));
        let ledger = lock_workflow_governance_ledger_tcb(&state).expect("strict ledger");
        let projection = ledger.recover().expect("strict durable projection");
        let durable_origin = projection
            .records
            .iter()
            .find_map(|record| match &record.event {
                WorkflowGovernanceEvent::BrokerOriginApplied(origin)
                    if origin.broker_event_digest == event_audit.event_digest =>
                {
                    Some((record.clone(), origin.clone()))
                }
                _ => None,
            })
            .expect("strict durable origin companion");
        assert_eq!(
            durable_origin.1.native_interaction_replay_digest.as_deref(),
            Some(bound_audit.native_interaction_replay_digest.as_str()),
            "the rotation-stable replay identity must survive the ledger/replay crash gap"
        );
        drop(ledger);

        let replay_backup = state.join(TEST_REPLAY_APPEND_FAILURE_BACKUP);
        fs::remove_dir(&replay_wal).expect("remove blocking strict replay directory");
        fs::rename(&replay_backup, &replay_wal).expect("restore strict replay WAL");
        fs::remove_file(state.join(TEST_REPLAY_APPEND_FAILURE_MARKER))
            .expect("disarm strict replay failpoint");
        assert_eq!(
            fs::read(&replay_wal).expect("restored strict replay WAL"),
            replay_before
        );

        let rotated_at = now.checked_add(1).expect("clock increment");
        let rotated =
            rotate_strict_runtime_registry(&adapter, &strict, &replacement_key, rotated_at);
        let next_packets = adapter
            .action_packets_at(rotated_at)
            .expect("packets after strict durable action");
        let next_packet = next_packets
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
            .expect("rotated signal packet");
        let mut reused_envelope = signed_signal_envelope(
            &next_packets.project_id,
            next_packet,
            &replacement_key,
            rotated_at,
            "strict-runtime-native-interaction-rotated-nonce-0002",
        );
        reused_envelope.issuer_id = StableId("broker.runtime.rotated".to_owned());
        let durable_provenance = envelope
            .native_host_provenance
            .as_ref()
            .expect("durable strict native provenance");
        let reused_provenance = reused_envelope
            .native_host_provenance
            .as_mut()
            .expect("reused strict native provenance");
        reused_provenance.host_event_ref = durable_provenance.host_event_ref.clone();
        reused_provenance.host_session_ref = durable_provenance.host_session_ref.clone();
        reused_provenance.host_interaction_ref = durable_provenance.host_interaction_ref.clone();
        seal_test_host_descriptor(&mut reused_envelope);
        reused_envelope.signature = hex(&replacement_key
            .sign(
                &workflow_broker_event_signing_bytes(&reused_envelope)
                    .expect("rotated broker signing bytes"),
            )
            .to_bytes());
        let rotated_verified = rotated
            .verify_bound_event(
                reused_envelope.clone(),
                &signal_context,
                i64::try_from(rotated_at).expect("clock fits i64"),
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("rotated strict signal event");
        assert_eq!(
            rotated_verified.audit().native_interaction_replay_digest,
            bound_audit.native_interaction_replay_digest
        );
        assert_ne!(
            rotated_verified.audit().registry_digest,
            bound_audit.registry_digest
        );
        assert_ne!(
            rotated_verified.audit().credential_generation,
            bound_audit.credential_generation
        );
        assert_ne!(
            rotated_verified.verified().audit().issuer_id,
            event_audit.issuer_id
        );
        assert_ne!(
            rotated_verified.verified().audit().action_packet_digest,
            event_audit.action_packet_digest
        );
        assert_ne!(reused_envelope.nonce, envelope.nonce);

        let workflow_before_conflict =
            fs::read(&workflow_wal).expect("workflow WAL before strict rotation conflict");
        let replay_before_conflict =
            fs::read(&replay_wal).expect("replay WAL before strict rotation conflict");
        assert!(matches!(
            adapter.apply_verified_bound_broker_action(rotated_verified, rotated_at),
            Err(WorkflowGovernanceAdapterError::ActionReplay(
                WorkflowActionReplayError::OriginReplayConflict { .. }
            ))
        ));
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after strict rotation conflict"),
            workflow_before_conflict,
            "issuer, packet, nonce, and credential rotation reuse must fail before ledger append"
        );
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after strict rotation conflict"),
            replay_before_conflict,
            "ledger conflict detection must not fabricate replay state"
        );

        let historical = rotated
            .verify_bound_event_for_recovery(envelope.clone(), &signal_context)
            .expect("current rotated registry retains historical event authority");
        assert_ne!(
            historical.audit().registry_digest,
            durable_origin.1.broker_registry_digest,
            "recovery must preserve the durable admitting registry while accepting current retained history"
        );
        let repaired = adapter
            .recover_historically_verified_bound_broker_action(historical)
            .expect("repair strict replay through the current rotated registry");
        assert_eq!(repaired.origin_record, durable_origin.0);
        assert!(repaired.replay_commit_repaired);
        let replay_after_repair = fs::read(&replay_wal).expect("strict replay after repair");
        assert_ne!(replay_after_repair, replay_before);

        let exact_retry = rotated
            .verify_bound_event_for_recovery(envelope, &signal_context)
            .expect("current rotated registry verifies the idempotent retry");
        let retried = adapter
            .recover_historically_verified_bound_broker_action(exact_retry)
            .expect("idempotent strict historical retry");
        assert!(!retried.replay_commit_repaired);
        assert_eq!(
            fs::read(&replay_wal).expect("strict replay after exact retry"),
            replay_after_repair
        );
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup fixture");
    }

    #[test]
    fn broker_action_rechecks_expiry_after_replay_lock_acquisition() {
        let (root, state) = temp_project("broker-expiry-after-replay-lock");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
        accept_test_intent(&adapter);
        let key = SigningKey::from_bytes(&[37_u8; 32]);
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
            "broker-expiry-after-replay-lock-nonce-0001",
        );
        let verified = verify_broker_envelope(&broker_document, envelope, now);
        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let replay_wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let workflow_before = fs::read(&workflow_wal).expect("workflow WAL before expiry");
        let replay_before = fs::read(&replay_wal).expect("replay WAL before expiry");
        let marker = state.join(TEST_EXPIRE_AFTER_REPLAY_RESERVATION_MARKER);
        fs::write(&marker, b"expire\n").expect("arm post-replay-lock expiry hook");

        assert!(matches!(
            adapter.apply_verified_broker_action(verified, now),
            Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
        ));
        assert!(
            !marker.exists(),
            "expiry hook must be consumed only after replay lock acquisition"
        );
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after expiry"),
            workflow_before,
            "expiry while waiting for replay authority must fail before ledger commit"
        );
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after expiry"),
            replay_before,
            "expired lock-held reservation must be dropped without a replay tombstone"
        );
    }

    #[cfg(unix)]
    #[test]
    fn broker_action_rejects_byte_identical_remint_after_replay_reservation() {
        let (root, state) = temp_project("broker-remint-after-replay-lock");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId("project.broker-apply".to_owned()),
            &root,
            &state,
        )
        .expect("adapter");
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
        accept_test_intent(&adapter);
        let key = SigningKey::from_bytes(&[43_u8; 32]);
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
            "broker-remint-after-replay-lock-nonce-0001",
        );
        let verified = verify_broker_envelope(&broker_document, envelope, now);
        let workflow_wal =
            state.join(forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let replay_wal = state.join(
            forge_core_store::workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
        );
        let workflow_before = fs::read(&workflow_wal).expect("workflow WAL before remint");
        let replay_before = fs::read(&replay_wal).expect("replay WAL before remint");
        let original_bytes = fs::read(root.join("README.md")).expect("project bytes before remint");
        let marker = state.join(TEST_REPLACE_PROJECT_FILE_AFTER_REPLAY_RESERVATION_MARKER);
        fs::write(&marker, b"README.md\n").expect("arm byte-identical remint hook");

        assert!(matches!(
            adapter.apply_verified_broker_action(verified, now),
            Err(WorkflowGovernanceAdapterError::RetainedProjectSnapshot(_))
        ));
        assert!(
            !marker.exists(),
            "replacement hook must be consumed only after replay lock acquisition"
        );
        assert_eq!(
            fs::read(root.join("README.md")).expect("project bytes after remint"),
            original_bytes,
            "the adversarial replacement must preserve bytes while changing identity"
        );
        assert_eq!(
            fs::read(&workflow_wal).expect("workflow WAL after remint"),
            workflow_before,
            "retained identity drift must fail before ledger commit"
        );
        assert_eq!(
            fs::read(&replay_wal).expect("replay WAL after remint"),
            replay_before,
            "the dropped replay reservation must leave no tombstone"
        );
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup fixture");
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
        adapter
            .initialize_with_readiness_profile(Some(WorkflowReadinessProfile::StrictExternal))
            .expect("initialize strict profile with replay");
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
        let domain = LockedWorkflowDomainPackContext::acquire(&root, &state).expect("domain");
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
        let replay_origin = broker_replay_origin_id(&audit).expect("replay origin");
        let replay_reservation = begin_workflow_action_replay_reservation(
            &state,
            &packet.packet_digest,
            &replay_origin,
            &planned.record_digest,
        )
        .expect("lock-held replay reservation");
        drop(batch);
        drop(replay_reservation);
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
    fn retained_workflow_snapshot_preserves_file_projection_and_exact_limit() {
        let (root, _) = temp_project("retained-workflow-snapshot-limit");
        fs::create_dir_all(root.join("empty")).expect("empty directory");
        let retained = RetainedWorkflowProjectSnapshot::capture_with_limits(&root, 16, 8)
            .expect("exact limit");
        let store_projection = RetainedProjectTree::capture(&root, 16, 8).expect("store snapshot");
        assert_eq!(
            retained.digest(),
            store_projection.regular_file_snapshot_digest()
        );
        retained.revalidate().expect("retained workflow snapshot");
        assert!(matches!(
            RetainedWorkflowProjectSnapshot::capture_with_limits(&root, 16, 7),
            Err(WorkflowGovernanceAdapterError::RetainedProjectSnapshot(
                RetainedProjectTreeError::ResourceLimit {
                    resource: "snapshot bytes",
                    maximum: 7,
                }
            ))
        ));
        drop(store_projection);
        drop(retained);
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup fixture");
    }

    #[test]
    fn retained_workflow_snapshot_preserves_file_count_limit() {
        let (root, _) = temp_project("retained-workflow-snapshot-file-limit");
        fs::remove_file(root.join("README.md")).expect("remove fixture README");
        fs::create_dir_all(root.join("nested")).expect("nested directory");
        fs::write(root.join("nested/one"), b"1").expect("first file");

        let retained = RetainedWorkflowProjectSnapshot::capture_with_limits(&root, 1, 1)
            .expect("one nested file remains within the file-only limit");
        retained.revalidate().expect("retained nested file");
        drop(retained);

        fs::remove_file(root.join("nested/one")).expect("remove nested file");
        fs::remove_dir(root.join("nested")).expect("remove nested directory");
        fs::write(root.join("one"), b"1").expect("first root file");
        fs::write(root.join("two"), b"2").expect("second root file");
        assert!(matches!(
            RetainedWorkflowProjectSnapshot::capture_with_limits(&root, 1, 2),
            Err(WorkflowGovernanceAdapterError::RetainedProjectSnapshot(
                RetainedProjectTreeError::ResourceLimit {
                    resource: "snapshot files",
                    maximum: 1,
                }
            ))
        ));
        fs::remove_dir_all(root.parent().expect("fixture root")).expect("cleanup fixture");
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
                native_interaction_replay_digest: None,
                issued_at_unix: 10,
                expires_at_unix: 120,
                native_host_provenance: None,
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

        let mut unverified_external_subject = local.clone();
        let WorkflowGovernanceEvent::EvaluatorObserved(event) =
            &mut unverified_external_subject.event
        else {
            unreachable!();
        };
        event.subject.kind = WorkflowEvidenceSubjectKind::ExternalSystem;
        event.subject.subject_ref = "external://unverified/system".to_owned();
        event.subject.subject_digest = format!("sha256:{}", "c".repeat(64));
        assert!(derive(&projection(unverified_external_subject))
            .evidence
            .is_empty());

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
