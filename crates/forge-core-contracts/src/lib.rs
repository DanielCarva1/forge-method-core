pub mod catalog;
pub mod claim;
pub mod command;
pub mod common;
pub mod envelope;
pub mod typed_failure;

pub mod agent_run;
pub mod assurance;
pub mod autonomy_policy;
pub mod backup_manifest;
pub mod bootstrap_recovery;
pub mod checkpoint;
pub mod completion;
pub mod coordination_eval;
pub mod decision;
pub mod domain_pack;
pub mod domain_pack_acquisition;
pub mod domain_pack_authoring;
pub mod domain_pack_discovery;
pub mod domain_pack_learning;
pub mod domain_pack_lifecycle;
pub mod domain_pack_policy;
pub mod domain_pack_publication;
pub mod domain_pack_rebase;
pub mod domain_pack_remote_acquisition;
pub mod domain_pack_resolution;
pub mod eval_run;
pub mod evidence;
pub mod execution_principal;
pub mod funnel_autonomy;
pub mod gate;
pub mod governance;
pub mod guide_decision;
pub mod guide_protocol;
pub mod host_capability_result;
pub mod host_conformance;
pub mod host_support_matrix;
pub mod inventory;
pub mod isolation;
pub mod markdown_authority;
pub mod memory;
pub mod operation;
pub mod operation_reference;
pub mod phase;
pub mod post_build_verify_episode;
pub mod product_lifecycle;
pub mod product_lifecycle_verification;
pub mod project_link;
pub mod project_reinitialize;
pub mod recovery;
pub mod request;
pub mod research;
pub mod reserved_state_paths;
pub mod runtime;
pub mod telemetry;
pub mod tool_effect;
pub mod verification_goal;
pub mod workflow;
pub mod workflow_behavior;
pub mod workflow_broker;
pub mod workflow_governance;
pub mod workflow_migration;
pub mod workflow_release;
pub mod workflow_release_review;
pub mod workflow_release_review_v2;
pub mod workflow_retirement;
pub mod workspace_crate_boundary;

pub(crate) fn is_exact_host_version(value: &str) -> bool {
    if value.is_empty()
        || value != value.trim()
        || value.contains(['*', '<', '>', '=', '^', '~', '|', ' '])
    {
        return false;
    }
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "unknown"
            | "unobserved"
            | "not_observed"
            | "not-observed"
            | "latest"
            | "current"
            | "unspecified"
            | "none"
            | "n/a"
    )
}

pub(crate) fn is_lowercase_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

pub use agent_run::{AgentRunContract, AgentRunContractDocument};
pub use assurance::{
    AssuranceCase, AssuranceCaseDocument, AssuranceClaim, AssuranceClaimStatus, AssuranceWaiver,
    CapabilityGap, CapabilityGapKind, DecisionAlternative, DecisionRequest,
    DurableAssuranceCapabilityBinding, DurableAssuranceClaimBinding,
    DurableAssuranceDecisionBinding, DurableAssuranceEpistemicState, DurableAssuranceEpochBinding,
    DurableAssuranceEvidenceBinding, DurableAssuranceLensProjection, DurableAssuranceNextAction,
    DurableAssuranceProjection, DurableAssuranceReadinessState, DurableAssuranceWaiverBinding,
    HumanDecisionReason, IntentProposal, NextAction, NextActionKind, Obligation,
    ObligationCriticality, ObligationStatus, ProjectSnapshot, ReadinessAssessment, ReadinessTarget,
    ReadinessVerdict, UniversalAssuranceLens, WorkflowHumanIntentRevision,
    WorkflowRepresentativeEnvironment, WorkflowRepresentativeFailureMode,
    WorkflowRepresentativeScenarioReference, WorkflowRepresentativeSliceDefinition,
    WorkflowRepresentativeSliceDefinitionDocument, ASSURANCE_CASE_SCHEMA_VERSION,
    MAX_DURABLE_ASSURANCE_NEXT_ACTIONS, MAX_REPRESENTATIVE_SLICE_ITEMS,
    MAX_REPRESENTATIVE_SLICE_ITEM_BYTES, MAX_REPRESENTATIVE_SLICE_TEXT_BYTES,
    MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES, MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    MAX_WORKFLOW_INTENT_ITEM_BYTES, MAX_WORKFLOW_INTENT_LIST_ITEMS,
    MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES, MAX_WORKFLOW_INTENT_TOTAL_BYTES,
    WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION,
};
pub use autonomy_policy::{AutonomyPolicyContract, AutonomyPolicyContractDocument};
pub use backup_manifest::*;
pub use bootstrap_recovery::{
    BootstrapRecoveryAction, BootstrapRecoveryAuthorityEffect, BootstrapRecoveryAvailability,
    BootstrapRecoveryChoice, BootstrapRecoveryChoices, BootstrapRecoveryRequirement,
    BootstrapRecoveryValidationError, BootstrapStateLossDiagnostic, StateLossCause, StateLossKind,
    StateLossReleaseStatus, BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION,
};
pub use checkpoint::{CheckpointContract, CheckpointContractDocument};
pub use claim::{ClaimContract, ClaimContractDocument};
pub use command::{CommandContract, CommandContractDocument};
pub use common::{ClaimId, PrincipalId, RepoPath, ScopeId, SourceId, StableId};
pub use completion::{CompletionContract, CompletionContractDocument};
pub use coordination_eval::{
    CoordinationDimension, CoordinationEvalContract, CoordinationEvalContractDocument,
    CoordinationMetricKind,
};
pub use decision::{
    DecisionCloseContract, DecisionCloseContractDocument, DecisionEvidenceKind, DecisionKind,
    DecisionStatus,
};
pub use domain_pack::*;
pub use domain_pack_acquisition::*;
pub use domain_pack_authoring::*;
pub use domain_pack_discovery::*;
pub use domain_pack_learning::*;
pub use domain_pack_lifecycle::*;
pub use domain_pack_policy::*;
pub use domain_pack_publication::*;
pub use domain_pack_rebase::*;
pub use domain_pack_remote_acquisition::*;
pub use domain_pack_resolution::*;
pub use eval_run::{EvalRunContract, EvalRunContractDocument};
pub use evidence::{EvidenceSource, FieldEvidenceRegistry};
pub use execution_principal::ExecutionPrincipal;
pub use funnel_autonomy::*;
pub use gate::{GateContract, GateContractDocument};
pub use governance::{
    ConflictContract, ConflictDetectionReason, ConflictPolicy, ConflictResolutionState,
    GovernancePolicy, IntentContract, IntentScope, IntentScopeKind, ResolutionDecision,
};
pub use host_capability_result::*;
pub use host_conformance::*;
pub use host_support_matrix::*;
pub use inventory::{ContractFamily, ContractFamilyInventory, ContractFamilyInventoryDocument};
pub use isolation::{
    GitAction, IsolationContract, IsolationContractDocument, IsolationError, IsolationStatus,
    MergePlan, MergePolicy, MergeStep,
};
pub use markdown_authority::{
    authorize_markdown_load, is_markdown_repo_path, is_typed_authority_reference,
    validate_markdown_allowlist_entry, validate_markdown_policy, MarkdownAllowlistEntry,
    MarkdownAuthorityBoundary, MarkdownDebtDisposition, MarkdownDebtEntry, MarkdownLoadAudience,
    MarkdownLoadDecision, MarkdownLoadError, MarkdownProvenance, MarkdownRetirementDocument,
    MarkdownRole, MARKDOWN_RETIREMENT_POLICY_ID, MARKDOWN_RETIREMENT_SCHEMA_VERSION,
};
pub use memory::{
    AdmissionDecision, AdmissionDenialReason, AdmissionEvidence, ApprovalState, AuthorityLevel,
    EvidenceField, Freshness, MemoryContract, MemoryContractDocument, MemoryEntry, MemoryKind,
    MemoryPolicy, MemoryProvenance, MemoryScope, MemoryScopeKind, ReviewState,
};
pub use operation::{OperationContract, OperationContractDocument};
pub use operation_reference::OperationCrossReferencePolicyDocument;
pub use project_link::{ProjectLinkDocument, PROJECT_LINK_FILE_NAME, PROJECT_LINK_SCHEMA_VERSION};
pub use project_reinitialize::*;
pub use recovery::{
    HealthRecoveryContract, HealthRecoveryContractDocument, HealthStatus, RecoveryAction,
};
pub use request::{RequestContract, RequestContractDocument};
pub use reserved_state_paths::{
    classify_reserved_state_path, crash_replace_siblings, is_reserved_state_path,
    normalize_state_relative_path, ReservedStateArtifact, ReservedStatePath,
    StateRelativePathError, LEGACY_STATE_ROOT_COMPONENT, RESERVED_STATE_ARTIFACTS,
};
pub use runtime::{
    RuntimeBlockedReason, RuntimeCapability, RuntimeCapabilityDocument, RuntimeCapabilityKind,
    RuntimeHandoffContract, RuntimeHandoffContractDocument, RuntimeHandoffStatus, RuntimeKind,
    RuntimeRegistryEntryDocument,
};
pub use telemetry::{TelemetryContract, TelemetryContractDocument};
pub use tool_effect::{ToolEffectContract, ToolEffectContractDocument};
pub use verification_goal::{VerificationGoalContract, VerificationGoalContractDocument};

pub use catalog::{Catalog, CatalogDocument, CatalogEntry};
pub use envelope::{CliEnvelope, CliError, ExitReason, ENVELOPE_SCHEMA_VERSION};
pub use guide_decision::{GuideDecision, GuideDecisionDocument};
pub use guide_protocol::{GuideProtocol, GuideProtocolDocument, GUIDE_PROTOCOL_SCHEMA_VERSION};
pub use phase::Phase;
pub use post_build_verify_episode::*;
pub use product_lifecycle::*;
pub use product_lifecycle_verification::*;
pub use research::{
    ResearchAdmissionDecision, ResearchAdmissionDenialReason, ResearchContract, ResearchPolicy,
    ResearchSource, ResearchSourceKind,
};
pub use typed_failure::TypedFailure;
pub use workflow::{Workflow, WorkflowDocument};
pub use workflow_behavior::*;
pub use workflow_broker::*;
pub use workflow_governance::{
    AdvisoryWorkflowPlaybook, ApplicabilityAssessedEvent, BrokerOriginAppliedEvent,
    CapabilityProbedEvent, ContinuityRecordedEvent, CoordinationCompletionState,
    CoordinationHealthRecoveryState, CoordinationMutationHandoff, CoordinationRequestState,
    CoordinationStateAppliedEvent, CoordinationStateRecord, CoreDomainPackRebasedEvent,
    DecisionNeedRaisedEvent, DecisionResolvedEvent, DomainPackGenerationTransitionedEvent,
    EvaluatorObservedEvent, HumanIntentRevisionAcceptedEvent, PhaseAdvancedEvent,
    PolicyCompletedEvent, PostBuildVerifyAdmittedGateResult, PostBuildVerifyEpisodeAppliedEvent,
    PostBuildVerifyEpisodeOutcome, PostBuildVerifyGateKind, ProjectImportedEvent,
    ReceiptRevokedEvent, ReleaseUpgradedEvent, SignalChangedEvent, WaiverAuthorizedEvent,
    WorkflowAssuranceClaimRole, WorkflowBrokerHostInteractionKind,
    WorkflowBrokerNativeHostProvenance, WorkflowBrokerOriginProfile, WorkflowCapabilityProbeKind,
    WorkflowCapabilityRequirement, WorkflowClaimPolicy, WorkflowClaimWaiverObservation,
    WorkflowClaimWaiverPolicy, WorkflowCompletionAssertion, WorkflowContentAddressedReference,
    WorkflowDecisionActivation, WorkflowDecisionRule, WorkflowDisproofPolicy,
    WorkflowDomainPackGenerationIdentity, WorkflowEffectiveBundleIdentity,
    WorkflowEvaluatorBinding, WorkflowEvaluatorProvider, WorkflowEvidenceFreshness,
    WorkflowEvidenceKind, WorkflowEvidenceObservation, WorkflowEvidenceOutcome,
    WorkflowEvidenceProvenance, WorkflowEvidenceStrength, WorkflowEvidenceSubject,
    WorkflowEvidenceSubjectKind, WorkflowFreshnessRequirement, WorkflowGovernanceBundle,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluation,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent, WorkflowGovernanceLedger,
    WorkflowGovernanceLedgerDocument, WorkflowGovernanceLedgerRecord, WorkflowGovernancePolicy,
    WorkflowGovernancePolicyOverlay, WorkflowGovernancePolicyOverlayDocument,
    WorkflowGovernanceReceiptDocument, WorkflowGovernanceSignal, WorkflowObligationPolicy,
    WorkflowPolicyActivation, WorkflowPolicyRouting, WorkflowPrerequisite,
    WorkflowPrerequisiteRequirement, WorkflowReleaseAdmissionProof,
    WorkflowReleaseRegistryProvenance, WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION,
};
pub use workflow_migration::{
    LegacyWorkflowField, LegacyWorkflowFieldMapping, LegacyWorkflowFieldRole,
    WorkflowCompatibilityField, WorkflowCompatibilityProjectionPolicy, WorkflowGoldenPathCoverage,
    WorkflowGoldenPathSelection, WorkflowMigrationAuthority, WorkflowMigrationDisposition,
    WorkflowMigrationPlan, WorkflowMigrationPlanDocument, WorkflowMigrationQuarantine,
    WorkflowMigrationTargetNamespaces, WorkflowRetirementGate, WorkflowRetirementPolicy,
    WorkflowSelectionTier, WorkflowShadowMode, WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
};
pub use workflow_release::{
    WorkflowCompatibilityLifecycle, WorkflowCompatibilityReason, WorkflowCompatibilityReasonCode,
    WorkflowConsumerDiagnosticsPolicy, WorkflowDomainPackCandidate,
    WorkflowDomainPackDeferralReason, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceReleaseManifest, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistry, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowGovernanceReleaseRegistryEntry, WorkflowLegacyCompatibilityAuthority,
    WorkflowMigrationBatch, WorkflowMigrationBatchAuthority, WorkflowMigrationBatchBinding,
    WorkflowMigrationBatchDocument, WorkflowMigrationBatchEvidence,
    WorkflowMigrationEvidenceReference, WorkflowQuarantine, WorkflowQuarantineReasonCode,
    WorkflowQuarantineRiskTier, WorkflowReceiptCarryover, WorkflowReleaseBatchReference,
    WorkflowReleaseCompatibilityPolicy, WorkflowReleaseCompatibilityProjectionMode,
    WorkflowReleaseDispositionIntent, WorkflowReleasePredecessorReference,
    WorkflowReleaseRegistryAuthority, WorkflowReleaseRegistrySource, WorkflowReleaseWorkflowEntry,
    WorkflowRetirementAdmissionPolicy, WorkflowRetirementAuthorization,
    WorkflowRetirementAuthorizationDocument, WorkflowRetirementAuthorizationReference,
    WorkflowRetirementCompatibilityWindow, WorkflowRetirementEvidenceBinding,
    WorkflowRetirementReviewer, WorkflowRetirementSignatureAlgorithm,
    WorkflowRetirementSignatureEnvelope, WorkflowRuntimeBundleIdentity,
    WorkflowRuntimeBundleReference, WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION, WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION,
};
pub use workflow_release_review::*;
pub use workflow_release_review_v2::*;
pub use workflow_retirement::*;
pub use workspace_crate_boundary::*;
