//! Closed P5b policy and untrusted-observation contracts for workflow governance.
//!
//! Raw documents are simulation input only. Policies define outcomes, evidence
//! rules, capability requirements, and irreducible decisions; verified
//! authority exists only behind the mutation kernel's opaque trusted-snapshot
//! boundary. Advisory playbooks are carried for agent leverage but deliberately
//! have no field capable of authorizing progression or done.

use crate::assurance::{
    CapabilityGapKind, DecisionAlternative, DecisionRequest, HumanDecisionReason,
    ObligationCriticality, ReadinessTarget, UniversalAssuranceLens, WorkflowHumanIntentRevision,
};
use crate::common::{PrincipalId, StableId};
use crate::completion::CompletionContractDocument;
use crate::post_build_verify_episode::PostBuildVerifyEpisodeDocument;
use crate::recovery::HealthRecoveryContractDocument;
use crate::request::{RequestContractDocument, RequestStatus};
use crate::runtime::RuntimeKind;
use crate::workflow_release::{
    WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover, WorkflowRuntimeBundleIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_GOVERNANCE_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION: &str = "0.1";
/// Ledger records written after a Domain Pack effective-bundle epoch exists.
/// Readers continue to accept the frozen `0.1` history; `0.2` makes the new
/// transition impossible to smuggle into the historical wire version. An
/// accepted human intent supersedes it with `0.3`; frozen pre-intent `0.2`
/// bytes remain valid and are never rewritten.
pub const WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION: &str = "0.2";
/// Ledger records written from the first accepted human-intent revision.
///
/// `0.3` is a strict successor of both historical wires: a ledger remains on
/// `0.1` until a Domain Pack epoch (`0.2`) or an accepted intent (`0.3`), and
/// an accepted intent permanently advances subsequent records to `0.3`.
/// Readers retain exact `0.1`/`0.2` compatibility but never admit the new
/// event under either older schema.
pub const WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION: &str = "0.3";
/// Ledger records written from the first joined Core/Domain-Pack rebase.
/// The new event cannot appear under frozen `0.1`/`0.2`/`0.3` wires; readers
/// preserve all historical bytes and permanently retain `0.4` thereafter.
pub const WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION: &str = "0.4";
/// Ledger records written from the first provenance-bearing broker companion.
/// Readers preserve frozen `0.1` through `0.4` bytes; `0.5` remains active after
/// native host provenance until the strict replay successor advances to `0.6`.
pub const WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION: &str = "0.5";
/// Ledger records written from the first strict broker companion carrying the
/// rotation-stable native-interaction replay digest. Frozen `0.5` companions
/// decode with no strict digest; `0.6` requires one and is permanent thereafter.
pub const WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION: &str = "0.6";
/// Ledger records written from the first kernel-admitted post-BuildVerify
/// episode route. Frozen `0.1` through `0.6` histories remain readable, while
/// the typed C5.2 event and every successor record require this wire epoch.
pub const WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION: &str = "0.7";
/// Ledger records written from the first C5.3 replacement-continuity snapshot or
/// coordination-state event. Frozen `0.7` episode summaries remain readable;
/// `0.8` requires the complete candidate snapshot and permanently carries exact
/// request, completion, and health-recovery projections for fresh processes.
pub const WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION: &str = "0.8";
/// Ledger records written from a profile-bearing project genesis. Historical
/// project imports omit the profile and retain their exact bytes and earlier
/// wire epochs; explicit profiles require at least `0.9` and remain durable
/// across every successor record.
pub const WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION: &str = "0.9";
/// Ledger records written from the first same-owner cooperative objective.
///
/// The new event is intentionally distinct from human intent and external
/// broker provenance. Frozen `0.1` through `0.9` records retain their exact
/// bytes; a cooperative objective requires `0.10` and every successor record
/// remains on that epoch.
pub const WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION: &str = "0.10";
/// Ledger records written from the first cooperative objective revision.
///
/// Initial objective records remain byte-identical on frozen `0.10`. A
/// material supersession or additive clarification requires `0.11`, and every
/// successor record permanently remains on that wire epoch.
pub const WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION: &str = "0.11";
/// Ledger records written from the first kernel-adjudicated cooperative evidence offer.
pub const WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION: &str = "0.12";
/// Ledger records written from the explicit adoption of Solo Cooperative by a
/// legacy profile-less project. Frozen history remains byte-identical; the
/// typed transition and every successor record require this wire epoch.
pub const WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION: &str = "0.13";
/// Ledger records written after a source assessment first cites current prior
/// Solo Cooperative evidence. Frozen `0.12` and `0.13` history remains readable.
pub const WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION: &str = "0.14";
/// Ledger records written from the first accepted Work Focus snapshot.
/// Frozen `0.1` through `0.14` histories remain readable; the new event and
/// every successor record require `0.15` so older readers fail closed.
pub const WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION: &str = "0.15";
/// Work Focus records with explicit canonical blocker/evidence bindings require
/// a new wire so older readers fail closed instead of ignoring the relation.
pub const WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION: &str = "0.16";
/// Work Focus records carrying a bounded Quick Cycle snapshot require a new
/// wire so older readers fail closed instead of ignoring lifecycle continuity.
pub const WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION: &str = "0.17";

/// Maximum encoded UTF-8 JSON input accepted by the cooperative objective
/// command. The action packet publishes this same bound so a host never needs
/// source-code knowledge to materialize the closed input.
pub const MAX_WORKFLOW_COOPERATIVE_INPUT_BYTES: u64 = 128 * 1024;
pub const MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES: usize = 1024;

/// Non-authoritative typed policy contribution. It is deliberately not a
/// runtime bundle: references into the declared base are resolved only by the
/// deterministic repository compiler, and trusted admission accepts only the
/// fully composed, validated bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernancePolicyOverlayDocument {
    pub schema_version: String,
    pub workflow_governance_policy_overlay: WorkflowGovernancePolicyOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernancePolicyOverlay {
    pub id: StableId,
    pub base_bundle_id: StableId,
    pub policies: Vec<WorkflowGovernancePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceBundleDocument {
    pub schema_version: String,
    pub workflow_governance_bundle: WorkflowGovernanceBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceBundle {
    pub id: StableId,
    pub policies: Vec<WorkflowGovernancePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernancePolicy {
    pub id: StableId,
    pub compatibility_workflow_id: StableId,
    pub routing: WorkflowPolicyRouting,
    pub eligible_phases: Vec<StableId>,
    #[serde(default)]
    pub prerequisites: Vec<WorkflowPrerequisite>,
    pub obligations: Vec<WorkflowObligationPolicy>,
    pub claims: Vec<WorkflowClaimPolicy>,
    pub evaluators: Vec<WorkflowEvaluatorBinding>,
    #[serde(default)]
    pub capability_requirements: Vec<WorkflowCapabilityRequirement>,
    #[serde(default)]
    pub decision_rules: Vec<WorkflowDecisionRule>,
    pub advisory_playbook: AdvisoryWorkflowPlaybook,
}

/// Deterministic policy-selection metadata. Priority is an ordering key, not
/// permission to skip prerequisites or evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPolicyRouting {
    pub priority: u16,
    pub readiness_target: ReadinessTarget,
    pub activation: WorkflowPolicyActivation,
    #[serde(default)]
    pub signals: Vec<WorkflowGovernanceSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPolicyActivation {
    Required,
    WhenApplicable,
    OnSignal,
}

/// Closed signals that durable trusted state may activate. Free-form agent
/// interpretations never become routing authority.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceSignal {
    ContextRecoveryRequired,
    CourseCorrectionRequired,
    AdversarialReviewRequested,
    ReadinessRequested,
    BuildCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPrerequisite {
    pub policy_ref: StableId,
    pub requirement: WorkflowPrerequisiteRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPrerequisiteRequirement {
    Always,
    WhenApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationPolicy {
    pub id: StableId,
    pub description: String,
    pub criticality: ObligationCriticality,
    pub required_before: ReadinessTarget,
    pub claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimPolicy {
    pub id: StableId,
    pub statement: String,
    pub evaluator_ref: StableId,
    /// Universal assurance dimensions covered by this policy claim.
    ///
    /// Historical releases predate durable Assurance Case lenses and
    /// therefore deserialize to an empty set. An empty set is compatibility
    /// data, not proof that a claim covers every lens; release admission owns
    /// the stronger completeness invariant for lens-aware successors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assurance_lenses: Vec<UniversalAssuranceLens>,
    /// Closed semantic role used only by lens-aware releases. Historical
    /// claims omit it and retain byte-identical serialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assurance_role: Option<WorkflowAssuranceClaimRole>,
    pub waiver: WorkflowClaimWaiverPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAssuranceClaimRole {
    LensEvidence,
    RepresentativeSliceDefinition,
    RepresentativeSliceExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowClaimWaiverPolicy {
    NotWaivable,
    Authorized {
        max_target: ReadinessTarget,
        authority_scope: StableId,
        max_age_seconds: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvaluatorBinding {
    pub id: StableId,
    pub provider: WorkflowEvaluatorProvider,
    pub accepted_evidence_kinds: Vec<WorkflowEvidenceKind>,
    pub minimum_strength: WorkflowEvidenceStrength,
    pub minimum_passing_observations: usize,
    pub minimum_distinct_principals: usize,
    pub max_age_seconds: u64,
    pub freshness: WorkflowFreshnessRequirement,
    pub disproof_policy: WorkflowDisproofPolicy,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvaluatorProvider {
    RepositoryInspector,
    DeterministicTool,
    RepresentativeRuntime,
    IndependentReviewer,
    AuthorizedHuman,
    ExternalAuthority,
    ResearchSource,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceKind {
    ArtifactInspection,
    DeterministicCheck,
    RepresentativeExecution,
    IndependentReview,
    HumanAcceptance,
    ExternalAuthority,
    Research,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// Ordered weakest-to-strongest. Declaration order is part of the evaluator
/// contract and must not change without a schema version change.
pub enum WorkflowEvidenceStrength {
    ArtifactPresence,
    InspectedArtifact,
    DeterministicVerification,
    RepresentativeExecution,
    IndependentConfirmation,
    AuthoritativeAcceptance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFreshnessRequirement {
    CurrentOnly,
    StaleAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDisproofPolicy {
    RejectAnyDisproof,
    RequireUncontestedSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCapabilityRequirement {
    pub id: StableId,
    pub kind: CapabilityGapKind,
    pub probe_kind: WorkflowCapabilityProbeKind,
    pub description: String,
    pub affected_claim_refs: Vec<StableId>,
    pub resolution_options: Vec<String>,
    pub blocks_before: ReadinessTarget,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCapabilityProbeKind {
    StaticRegistry,
    LocalCommand,
    RuntimeHandshake,
    CredentialCheck,
    HumanAttestation,
    ExternalVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDecisionRule {
    pub id: StableId,
    pub activation: WorkflowDecisionActivation,
    #[serde(default)]
    pub claim_ref: Option<StableId>,
    pub question: String,
    pub reason: HumanDecisionReason,
    pub alternatives: Vec<DecisionAlternative>,
    pub recommended_alternative_ref: StableId,
    pub blocking: bool,
    pub blocks_before: ReadinessTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDecisionActivation {
    ObservedNeed,
    ClaimUnresolved,
    ClaimDisproven,
    /// The agent has verified the referenced claim and the remaining choice is
    /// genuinely a human judgment. Rules using this activation must carry a
    /// `claim_ref`; runtime admission enforces that binding.
    ClaimVerified,
    /// Every claim in the selected policy is freshly verified. This supports
    /// a human product/value choice only after the agent-owned evidence work
    /// is complete, without a caller-authored decision-need shortcut.
    AllClaimsVerified,
}

/// Non-authoritative strategy projection. No eligibility, completion, or
/// mutation field exists here by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryWorkflowPlaybook {
    pub id: StableId,
    #[serde(default)]
    pub steps: Vec<String>,
}

/// Durable ordered governance history. Each record is independently usable as
/// a receipt and is bound to the project, admitted bundle digest, and state
/// version at which it was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedgerDocument {
    pub schema_version: String,
    pub workflow_governance_ledger: WorkflowGovernanceLedger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedger {
    pub records: Vec<WorkflowGovernanceLedgerRecord>,
}

/// Single-record projection for APIs and stores that exchange one governance
/// receipt at a time. It has the identical hash-chain envelope as ledger items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReceiptDocument {
    pub schema_version: String,
    pub workflow_governance_receipt: WorkflowGovernanceLedgerRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedgerRecord {
    pub record_id: StableId,
    pub sequence: u64,
    pub project_id: StableId,
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub state_version: u64,
    #[serde(default)]
    pub previous_record_digest: Option<String>,
    pub record_digest: String,
    pub recorded_at_unix: u64,
    pub event: WorkflowGovernanceEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WorkflowGovernanceEvent {
    ProjectImported(ProjectImportedEvent),
    LegacySoloProfileAdopted(LegacySoloProfileAdoptedEvent),
    HumanIntentRevisionAccepted(HumanIntentRevisionAcceptedEvent),
    CooperativeObjectiveAccepted(CooperativeObjectiveAcceptedEvent),
    CooperativeEvidenceObserved(WorkflowCooperativeEvidenceObservedEvent),
    WorkFocusRecorded(crate::current_work::WorkflowWorkFocusRecordedEvent),
    ReleaseUpgraded(ReleaseUpgradedEvent),
    DomainPackGenerationTransitioned(DomainPackGenerationTransitionedEvent),
    CoreDomainPackRebased(Box<CoreDomainPackRebasedEvent>),
    PhaseAdvanced(PhaseAdvancedEvent),
    PostBuildVerifyEpisodeApplied(PostBuildVerifyEpisodeAppliedEvent),
    CoordinationStateApplied(CoordinationStateAppliedEvent),
    ApplicabilityAssessed(ApplicabilityAssessedEvent),
    SignalChanged(SignalChangedEvent),
    CapabilityProbed(CapabilityProbedEvent),
    DecisionNeedRaised(DecisionNeedRaisedEvent),
    DecisionResolved(DecisionResolvedEvent),
    EvaluatorObserved(EvaluatorObservedEvent),
    WaiverAuthorized(WaiverAuthorizedEvent),
    BrokerOriginApplied(BrokerOriginAppliedEvent),
    PolicyCompleted(PolicyCompletedEvent),
    ReceiptRevoked(ReceiptRevokedEvent),
    ContinuityRecorded(ContinuityRecordedEvent),
}

/// Closed authority basis for an objective carried by the same owner-operated
/// host agent. It is neither verified human origin nor an external-broker
/// attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeAuthorityBasis {
    CooperativeSameOwner,
}

/// Why a cooperative objective record exists in its immutable revision chain.
///
/// `Initial` is omitted from frozen 0.10 JSON so existing bytes and record
/// hashes remain unchanged. Every successor record carries an explicit kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeObjectiveRevisionKind {
    #[default]
    Initial,
    MaterialSupersession,
    NonMaterialClarification,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // Serde's predicate API passes the field by reference.
fn cooperative_revision_kind_is_initial(value: &WorkflowCooperativeObjectiveRevisionKind) -> bool {
    *value == WorkflowCooperativeObjectiveRevisionKind::Initial
}

/// Bounded semantic proposal materialized by a host from ordinary chat.
///
/// No readiness, evidence, evaluator, issuer, signature, or identity-
/// separation claim can be supplied through this shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeObjectiveProposal {
    pub outcome: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub unacceptable_outcomes: Vec<String>,
    #[serde(default)]
    pub open_uncertainties: Vec<String>,
}

/// Content-free, caller-carried host coordinates for audit and continuity.
///
/// These values identify where a proposal was materialized; they do not prove
/// that the named host, principal, or human interaction existed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeHostProvenance {
    pub host_id: StableId,
    pub host_version: String,
    pub session_ref: String,
    pub interaction_ref: String,
    pub conversation_digest: String,
    pub observed_at_unix: u64,
}

/// Closed host input for cooperative objective admission.
///
/// `DecisionRequired` is deliberately non-authorizing. Forge returns its one
/// typed question without reading or mutating governance state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowCooperativeObjectiveInput {
    /// Initial chat-derived objective for a project with no cooperative history.
    Unambiguous {
        proposal: WorkflowCooperativeObjectiveProposal,
        carrying_principal: PrincipalId,
        host_provenance: WorkflowCooperativeHostProvenance,
    },
    /// A material correction that replaces the active product direction while
    /// preserving the previous objective as immutable history.
    MaterialSupersession {
        proposal: WorkflowCooperativeObjectiveProposal,
        supersession_reason: String,
        carrying_principal: PrincipalId,
        host_provenance: WorkflowCooperativeHostProvenance,
    },
    /// Additive-only detail. The kernel retains the outcome and every existing
    /// list entry, then appends these bounded values without duplication.
    NonMaterialClarification {
        #[serde(default)]
        added_constraints: Vec<String>,
        #[serde(default)]
        added_unacceptable_outcomes: Vec<String>,
        #[serde(default)]
        added_open_uncertainties: Vec<String>,
        clarification_reason: String,
        carrying_principal: PrincipalId,
        host_provenance: WorkflowCooperativeHostProvenance,
    },
    DecisionRequired {
        decision_request: DecisionRequest,
    },
}

/// One immutable objective revision durably admitted for a Solo Cooperative workflow.
///
/// The kernel derives identity, revision, epoch, digest, predecessor, packet,
/// snapshot, head, and commit time. The carrying principal and generic host
/// provenance are audit coordinates only: this event makes no signature,
/// issuer-verification, human-origin, or identity-separation claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CooperativeObjectiveAcceptedEvent {
    pub objective_id: StableId,
    pub revision: u64,
    pub assurance_epoch: u64,
    pub proposal: WorkflowCooperativeObjectiveProposal,
    pub objective_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_objective_digest: Option<String>,
    #[serde(default, skip_serializing_if = "cooperative_revision_kind_is_initial")]
    pub revision_kind: WorkflowCooperativeObjectiveRevisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_reason: Option<String>,
    /// Canonical digest of the exact typed successor input accepted by the
    /// kernel. Frozen 0.10 initial records omit this field entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_input_digest: Option<String>,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub acceptance_action_packet_digest: String,
    pub carrying_principal: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
    pub authority_basis: WorkflowCooperativeAuthorityBasis,
    pub accepted_at_unix: u64,
}

/// One bounded human intent revision admitted by the workflow mutation TCB.
///
/// The event contains the accepted semantic content plus the exact epoch,
/// predecessor, project-state, action-packet, and authority bindings derived at
/// admission. It intentionally has no claim status, readiness, or evaluator
/// field for a host to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HumanIntentRevisionAcceptedEvent {
    pub assurance_epoch: u64,
    pub intent: WorkflowHumanIntentRevision,
    pub intent_digest: String,
    #[serde(default)]
    pub previous_intent_digest: Option<String>,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub acceptance_action_packet_digest: String,
    pub accepted_by: PrincipalId,
    pub accepted_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerHostInteractionKind {
    NativeHumanMessage,
    NativeHumanConfirmation,
    NativeReviewerConfirmation,
    AttestedRuntimeObservation,
}

/// Signed, content-free native-host provenance. Handles are opaque host-local
/// identifiers; the descriptor digest never commits raw transcript bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerNativeHostProvenance {
    pub host_kind: RuntimeKind,
    pub host_version: String,
    pub adapter_id: StableId,
    pub adapter_version: String,
    pub interaction_kind: WorkflowBrokerHostInteractionKind,
    pub host_event_ref: String,
    pub host_session_ref: String,
    pub host_interaction_ref: String,
    pub host_event_descriptor_digest: String,
    pub host_observed_at_unix: u64,
}

/// Durable provenance companion for one action produced from a separately
/// verified external broker event. This is audit data only; deserializing it
/// never grants broker, principal, or workflow mutation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrokerOriginAppliedEvent {
    pub action_packet_digest: String,
    pub broker_event_digest: String,
    pub action_record_digest: String,
    pub origin_principal_id: PrincipalId,
    pub separation_domain: StableId,
    pub nonce_fingerprint: String,
    pub issuer_id: StableId,
    pub issuer_profile: WorkflowBrokerOriginProfile,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub enrollment_ceremony_digest: String,
    pub broker_registry_digest: String,
    /// Rotation-stable strict-control-plane identity for one native host
    /// interaction. Historical `0.5` companions omit this field; new strict
    /// companions must carry it under the `0.6` ledger wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_interaction_replay_digest: Option<String>,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_host_provenance: Option<WorkflowBrokerNativeHostProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerOriginProfile {
    Human,
    Reviewer,
    Runtime,
}

/// Exact, project-local Domain Pack generation bound into one workflow
/// effective-bundle epoch. This is durable identity/audit data, not admission
/// authority; only the Domain Pack TCB can admit the active generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDomainPackGenerationIdentity {
    pub generation: u64,
    pub active_lock_digest: String,
    pub composition_digest: String,
    /// JCS digest of the sealed inner core `WorkflowGovernanceBundle`. The
    /// core release's runtime digest separately identifies the enclosing
    /// `WorkflowGovernanceBundleDocument`.
    pub base_core_bundle_digest: String,
    pub supply_chain_registry_digest: String,
    pub reviewer_registry_digest: String,
    pub reviewed_registry_digest: String,
}

/// Runtime identity used for policy evaluation without mutating the universal
/// core release registry. `core_runtime_bundle` remains the exact P5 release
/// pin; `effective_runtime_bundle` identifies core plus admitted pack policies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEffectiveBundleIdentity {
    pub core_runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub effective_runtime_bundle: WorkflowRuntimeBundleIdentity,
    #[serde(default)]
    pub domain_pack_generation: Option<WorkflowDomainPackGenerationIdentity>,
    /// Canonical digest of all receipt-relevant effective semantics. The
    /// kernel, never a caller-authored event, derives this value.
    pub receipt_context_digest: String,
}

/// Dedicated effective-bundle epoch transition. Raw event DTOs cannot be
/// appended through the generic ledger API; the workflow TCB validates source,
/// target, monotonic generation, prior head, and deterministic carryover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackGenerationTransitionedEvent {
    pub from_effective_bundle: WorkflowEffectiveBundleIdentity,
    pub to_effective_bundle: WorkflowEffectiveBundleIdentity,
    pub receipt_carryover: WorkflowReceiptCarryover,
    pub prior_ledger_head_digest: String,
}

/// One joined epoch transition that advances the admitted core release and an
/// independently committed Domain Pack generation together. The lifecycle TCB
/// remains the authority for the target generation; this event only makes the
/// already-admitted pair active in the workflow ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoreDomainPackRebasedEvent {
    pub release_transition: ReleaseUpgradedEvent,
    pub from_effective_bundle: WorkflowEffectiveBundleIdentity,
    pub to_effective_bundle: WorkflowEffectiveBundleIdentity,
    pub receipt_carryover: WorkflowReceiptCarryover,
    pub prior_ledger_head_digest: String,
}

/// Auditable result proposed for one atomic release upgrade. Raw event DTOs
/// grant no upgrade or admission authority; trusted kernel code must construct
/// and append the event under the project lock after verifying every binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseUpgradedEvent {
    pub from_release: WorkflowGovernanceReleaseIdentity,
    pub to_release: WorkflowGovernanceReleaseIdentity,
    pub from_runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub to_runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub registry_provenance: WorkflowReleaseRegistryProvenance,
    pub admission_proof: WorkflowReleaseAdmissionProof,
    pub receipt_carryover: WorkflowReceiptCarryover,
    pub prior_ledger_head_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseRegistryProvenance {
    pub registry_id: StableId,
    pub registry_version: String,
    pub registry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseAdmissionProof {
    pub proof_id: StableId,
    pub proof_digest: String,
    pub snapshot_digest: String,
    pub from_policy_set_digest: String,
    pub to_policy_set_digest: String,
}

/// Durable project-local workflow readiness posture.
///
/// Historical ledgers omit this value and are projected as
/// [`Self::StrictExternal`] without changing their bytes or record digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReadinessProfile {
    SoloCooperative,
    StrictExternal,
}

impl WorkflowReadinessProfile {
    /// Stable serialized name used in CLI diagnostics and other wire-visible
    /// projections. This must stay aligned with the serde representation.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::SoloCooperative => "solo_cooperative",
            Self::StrictExternal => "strict_external",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportedEvent {
    pub source_ref: String,
    pub source_digest: String,
    pub snapshot_digest: String,
    pub initial_phase: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_profile: Option<WorkflowReadinessProfile>,
}

/// One explicit, one-way adoption of Solo Cooperative by a historical project
/// whose genesis predates readiness profiles.
///
/// This records cooperative same-owner provenance only. It does not claim a
/// verified human interaction, signature, external reviewer, or broker event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegacySoloProfileAdoptedEvent {
    pub legacy_project_import_record_digest: String,
    pub prior_ledger_head_digest: String,
    pub snapshot_digest: String,
    pub authority_basis: WorkflowCooperativeAuthorityBasis,
}

impl ProjectImportedEvent {
    /// Legacy profile-less imports retain the historical strict posture.
    #[must_use]
    pub fn effective_readiness_profile(&self) -> WorkflowReadinessProfile {
        self.readiness_profile
            .unwrap_or(WorkflowReadinessProfile::StrictExternal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhaseAdvancedEvent {
    #[serde(default)]
    pub from_phase: Option<StableId>,
    pub to_phase: StableId,
    pub snapshot_digest: String,
}

/// Closed C5.2 route admitted by the kernel after replaying a candidate-only
/// C5.1 episode against exact current workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyEpisodeOutcome {
    AdvancedToReadyOperate,
    AdvancedToEvolve,
    RollbackAssessmentOpened,
    EvolutionTriageOpened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostBuildVerifyGateKind {
    Readiness,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyAdmittedGateResult {
    pub kind: PostBuildVerifyGateKind,
    pub status: crate::gate::GateStatus,
    pub effective_bundle_digest: String,
}

/// Durable audit projection for one C5.2 route. Deserializing this event never
/// recreates the kernel admission that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PostBuildVerifyEpisodeAppliedEvent {
    pub episode_id: StableId,
    pub generation: u64,
    #[serde(default)]
    pub previous_episode_digest: Option<String>,
    pub episode_digest: String,
    pub release_subject: WorkflowGovernanceReleaseIdentity,
    pub decision_digest: String,
    pub from_phase: StableId,
    #[serde(default)]
    pub to_phase: Option<StableId>,
    pub outcome: PostBuildVerifyEpisodeOutcome,
    pub snapshot_digest: String,
    pub prior_ledger_head_digest: String,
    pub prior_state_version: u64,
    #[serde(default)]
    pub admitted_gate: Option<PostBuildVerifyAdmittedGateResult>,
    /// Complete C5.1 candidate bytes retained for C5.3 fresh-process recovery.
    /// Historical `0.7` records omit this field. Deserialization remains audit
    /// projection only and never recreates the consumed kernel admission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_snapshot: Option<PostBuildVerifyEpisodeDocument>,
}

/// One append-only coordination-state projection admitted under the same retained
/// workflow-ledger lock as its exact state-version and head bindings. These values
/// remain audit/recovery data; they do not grant claim, mutation, phase, release,
/// signing, private-key, trust, install, activation, or lifecycle authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationStateAppliedEvent {
    pub prior_ledger_head_digest: String,
    pub prior_state_version: u64,
    pub state: CoordinationStateRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CoordinationStateRecord {
    Request(CoordinationRequestState),
    Completion(CoordinationCompletionState),
    HealthRecovery(CoordinationHealthRecoveryState),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationRequestState {
    pub request: RequestContractDocument,
    #[serde(default)]
    pub previous_status: Option<RequestStatus>,
    pub actor_agent_id: StableId,
    #[serde(default)]
    pub response_evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_handoff: Option<CoordinationMutationHandoff>,
}

/// Evidence-only binding showing which already-authorized driver/effect lane must
/// perform a requested mutation. The request and this handoff cannot apply it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationMutationHandoff {
    pub driver_agent_id: StableId,
    pub requested_operation: StableId,
    pub claim_contract_ref: crate::common::RepoPath,
    pub authority_refs: Vec<String>,
    pub effect_contract_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationCompletionState {
    pub completion: CompletionContractDocument,
    pub applied_claim_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationHealthRecoveryState {
    pub recovery: HealthRecoveryContractDocument,
    pub actor_agent_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplicabilityAssessedEvent {
    pub policy_ref: StableId,
    pub applicable: bool,
    pub assessed_by: PrincipalId,
    pub evaluator_ref: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowContentAddressedReference {
    pub subject_ref: String,
    pub subject_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalChangedEvent {
    pub signal: WorkflowGovernanceSignal,
    pub active: bool,
    pub episode_id: StableId,
    pub generation: u64,
    pub changed_by: PrincipalId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbedEvent {
    pub policy_ref: StableId,
    pub capability_ref: StableId,
    pub probe_kind: WorkflowCapabilityProbeKind,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub available: bool,
    pub probe_ref: String,
    pub probe_digest: String,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionNeedRaisedEvent {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub authority_scope: StableId,
    pub question_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionResolvedEvent {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub selected_alternative_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub authorization_intent_digest: String,
    pub signature_fingerprint: String,
    pub resolved_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorObservedEvent {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub provider: WorkflowEvaluatorProvider,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub outcome: WorkflowEvidenceOutcome,
    pub provenance: WorkflowEvidenceProvenance,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceProvenance {
    pub source_ref: String,
    pub source_digest: String,
    pub scenario_digest: String,
    pub semantic_identity: StableId,
    pub producer_ref: StableId,
    #[serde(default)]
    pub principal: Option<PrincipalId>,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceSubject {
    pub kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceSubjectKind {
    Artifact,
    RepositoryState,
    Runtime,
    ExternalSystem,
    HumanDecision,
    ProjectSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaiverAuthorizedEvent {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub max_target: ReadinessTarget,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub authorization_intent_digest: String,
    pub signature_fingerprint: String,
    pub consequences_digest: String,
    pub authorized_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyCompletedEvent {
    pub policy_ref: StableId,
    pub target: ReadinessTarget,
    pub phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub subject: WorkflowEvidenceSubject,
    pub dependency_receipt_digests: Vec<String>,
    pub evidence_receipt_digests: Vec<String>,
    /// Exact durable state anchors used for non-evidence grounding. Historical
    /// records omit this field and remain snapshot-bound.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grounding_anchor_digests: Vec<String>,
    pub unresolved_deferred_obligation_refs: Vec<StableId>,
    pub unresolved_deferred_capability_refs: Vec<StableId>,
    pub completed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReceiptRevokedEvent {
    pub revoked_record_id: StableId,
    pub revoked_record_digest: String,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub reason: String,
    pub revoked_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContinuityRecordedEvent {
    #[serde(default)]
    pub from_principal: Option<PrincipalId>,
    pub to_principal: PrincipalId,
    pub snapshot_digest: String,
    pub context_digest: String,
    pub next_policy_ref: StableId,
    pub next_action: String,
    pub continuity_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceEvaluationDocument {
    pub schema_version: String,
    pub workflow_governance_evaluation: WorkflowGovernanceEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceEvaluation {
    pub observation_set_id: StableId,
    pub state_version: u64,
    /// Trusted Adapter evaluation clock; caller-authored values remain
    /// simulation-only in raw documents.
    pub observed_at_unix: u64,
    pub bundle_id: StableId,
    pub policy_id: StableId,
    pub current_phase: StableId,
    pub target: ReadinessTarget,
    #[serde(default)]
    pub completed_policy_refs: Vec<StableId>,
    /// Policies proven not applicable by trusted applicability receipts. Raw
    /// simulation callers may propose this list, but it remains candidate-only.
    #[serde(default)]
    pub not_applicable_policy_refs: Vec<StableId>,
    #[serde(default)]
    pub available_capability_refs: Vec<StableId>,
    #[serde(default)]
    pub decision_need_refs: Vec<StableId>,
    #[serde(default)]
    pub resolved_decision_refs: Vec<StableId>,
    #[serde(default)]
    pub waivers: Vec<WorkflowClaimWaiverObservation>,
    /// Typed state grounding supplied by a trusted adapter. In raw evaluation
    /// documents this remains simulation-only, just like caller-authored
    /// evidence; it becomes authoritative only after kernel derivation from a
    /// current, non-revoked ledger anchor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groundings: Vec<WorkflowClaimGroundingObservation>,
    #[serde(default)]
    pub evidence: Vec<WorkflowEvidenceObservation>,
    pub completion_assertion: WorkflowCompletionAssertion,
}

/// A claim grounded by current durable project direction rather than by an
/// evaluator observation. Grounding carries no human-origin, independent
/// review, or evaluator-provider assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimGroundingObservation {
    pub grounding_ref: String,
    pub claim_ref: StableId,
    pub kind: WorkflowClaimGroundingKind,
    pub anchor_record_digest: String,
    #[serde(default)]
    pub principal: Option<PrincipalId>,
    pub authority_basis: WorkflowCooperativeAuthorityBasis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowClaimGroundingKind {
    /// The same owner and carrying agent asserted that the objective was
    /// unambiguous enough to guide work. This is not independent semantic
    /// review and does not verify that a human authored the text.
    CooperativeSameOwnerObjective,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceObservation {
    pub evidence_ref: String,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    #[serde(default)]
    pub principal: Option<PrincipalId>,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub freshness: WorkflowEvidenceFreshness,
    pub outcome: WorkflowEvidenceOutcome,
}

/// Raw projection of an authorized waiver receipt. In the simulation lane it
/// remains caller-authored and non-authoritative; the trusted Adapter derives
/// the identical shape only from verified ledger records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimWaiverObservation {
    pub claim_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub max_target: ReadinessTarget,
    pub authorization_intent_digest: String,
    pub authorized_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceFreshness {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceOutcome {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompletionAssertion {
    NotAsserted,
    Asserted,
}

/// Durable audit record for a kernel-adjudicated cooperative evidence offer.
/// A rejected offer is intentionally retained but never contributes support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceObservedEvent {
    /// Digest of canonical parsed bytes, or of the exact bounded/unparseable
    /// input. Rejections retain no hostile caller-controlled body.
    pub offer_digest: String,
    /// Retained only when the parsed id is bounded and nonblank. This permits
    /// durable conflicting-reuse detection without retaining the raw offer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted_evidence: Option<crate::WorkflowAdmittedCooperativeEvidence>,
    pub disposition: crate::WorkflowCooperativeEvidenceDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection: Option<crate::WorkflowCooperativeEvidenceRejection>,
    pub admission_snapshot_digest: String,
    pub admission_ledger_head_digest: String,
    pub admission_state_version: u64,
    /// Kernel readback/append observation. Evidence freshness is evaluated
    /// from the execution and readback times, never from a later reprojection.
    pub observed_at_unix: u64,
}
