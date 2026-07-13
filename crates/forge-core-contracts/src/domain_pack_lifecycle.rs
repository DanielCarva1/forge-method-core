//! Closed P6b compatibility, lifecycle, ledger, receipt, and recovery wire types.
//!
//! Every value in this module is serializable evidence. Only the trusted
//! lifecycle boundary can turn a clean, freshly rechecked preflight into an
//! active generation.

use crate::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority,
    DomainPackCompositionProjectionDocument, DomainPackCoordinate, DomainPackExactLockDocument,
    DomainPackResolutionProjectionDocument, DomainPackRuntimeCapabilityGap,
    DomainPackSandboxDecision, DomainPackTrustDisposition, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompatibilityReportDocument {
    pub schema_version: String,
    pub domain_pack_compatibility_report: DomainPackCompatibilityReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompatibilityReport {
    pub report_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub operation: DomainPackLifecycleOperation,
    pub from_lock_digest: Option<String>,
    pub to_lock_digest: String,
    pub from_composition_digest: Option<String>,
    pub to_composition_digest: String,
    pub changes: Vec<DomainPackSemanticChange>,
    pub requirement_impacts: Vec<DomainPackRequirementImpact>,
    pub capability_impacts: Vec<DomainPackCapabilityImpact>,
    pub receipt_policy: DomainPackReceiptMigrationPolicy,
    pub universal_core_unchanged: bool,
    pub status: DomainPackCompatibilityStatus,
    pub issues: Vec<DomainPackCompatibilityIssue>,
    pub report_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackSemanticChange {
    pub kind: DomainPackSemanticChangeKind,
    pub subject_ref: StableId,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub explanation: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSemanticChangeKind {
    PackAdded,
    PackRemoved,
    PackVersionChanged,
    PackContentChanged,
    PolicyChanged,
    ObligationChanged,
    ClaimChanged,
    HazardChanged,
    LifecycleChanged,
    EvaluatorChanged,
    AdapterChanged,
    CapabilityChanged,
    DomainChanged,
    PlaybookChanged,
    FixtureChanged,
    DependencyChanged,
    ReplacementChanged,
    TrustChanged,
    SandboxChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRequirementImpact {
    pub requirement_ref: StableId,
    pub subject_ref: StableId,
    pub status: DomainPackRequirementImpactStatus,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRequirementImpactStatus {
    Satisfied,
    NewlySatisfied,
    StillMissing,
    NewlyMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCapabilityImpact {
    pub capability_ref: StableId,
    pub before: Option<DomainPackSandboxDecision>,
    pub after: Option<DomainPackSandboxDecision>,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackReceiptMigrationPolicy {
    PreserveExactEquivalent,
    InvalidateAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCompatibilityStatus {
    Compatible,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompatibilityIssue {
    pub code: DomainPackCompatibilityIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCompatibilityIssueCode {
    CoreChanged,
    RequirementsChangedWithoutIntent,
    RegistryChangedWithoutResolution,
    TrustDegraded,
    RevokedTarget,
    NamespaceChanged,
    MissingRequiredDomain,
    MissingRequiredCapability,
    ExecutableCapabilityDenied,
    ReceiptInvalidationRequired,
    InvalidLockDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleRequestDocument {
    pub schema_version: String,
    pub domain_pack_lifecycle_request: DomainPackLifecycleRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub project_id: StableId,
    pub principal_id: StableId,
    pub operation: DomainPackLifecycleOperation,
    pub expected_state: DomainPackExpectedLifecycleState,
    pub resolution_request_digest: String,
    pub project_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackLifecycleOperation {
    Install {
        root: DomainPackCoordinate,
    },
    Upgrade {
        pack: DomainPackCoordinate,
        expected_from: String,
        target_requirement: String,
        required_content_digest: Option<String>,
    },
    Remove {
        pack: DomainPackCoordinate,
    },
    Rollback {
        target_receipt_digest: String,
        target_lock_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackExpectedLifecycleState {
    Uninitialized {
        project_snapshot_digest: String,
    },
    Initialized {
        generation: u64,
        active_lock_digest: String,
        lifecycle_head_digest: String,
        project_snapshot_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecyclePreflightDocument {
    pub schema_version: String,
    pub domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecyclePreflight {
    pub preflight_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub request: DomainPackLifecycleRequestDocument,
    pub request_digest: String,
    pub observed_state: DomainPackExpectedLifecycleState,
    pub resolution: DomainPackResolutionProjectionDocument,
    pub proposed_lock: DomainPackExactLockDocument,
    pub composition: DomainPackCompositionProjectionDocument,
    pub supply_chain_assessments: Vec<DomainPackSupplyChainAssessment>,
    pub trust_decisions: Vec<DomainPackLifecycleTrustDecision>,
    pub capability_gaps: Vec<DomainPackRuntimeCapabilityGap>,
    pub compatibility_report: DomainPackCompatibilityReportDocument,
    pub staged_artifacts: Vec<DomainPackArtifactBinding>,
    pub status: DomainPackLifecyclePreflightStatus,
    pub issues: Vec<DomainPackLifecycleIssue>,
    pub preflight_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// These independent signed-wire observations must remain individually
// addressable; collapsing them into one state enum would permit invalid
// combinations to disappear from governance evidence and break the schema.
#[allow(clippy::struct_excessive_bools)]
pub struct DomainPackSupplyChainAssessment {
    pub package_digest: String,
    pub registry_record_digest: String,
    pub publisher_signature_verified: bool,
    pub registry_signature_threshold_verified: bool,
    pub namespace_grant_verified: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleTrustDecision {
    pub package_digest: String,
    pub disposition: DomainPackTrustDisposition,
    pub rule_ref: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLifecyclePreflightStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleIssue {
    pub code: DomainPackLifecycleIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackLifecycleIssueCode {
    UnsupportedSchemaVersion,
    InvalidDigest,
    StaleExpectedState,
    ProjectSnapshotDrift,
    ResolutionBlocked,
    CompositionBlocked,
    SupplyChainRejected,
    TrustRejected,
    CapabilityUnavailable,
    SandboxDenied,
    CompatibilityBlocked,
    ArtifactBindingMismatch,
    SemanticReviewRejected,
    ResourceLimitExceeded,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackActivePointerDocument {
    pub schema_version: String,
    pub domain_pack_active_pointer: DomainPackActivePointer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackActivePointer {
    pub project_id: StableId,
    pub generation: u64,
    pub active_lock_digest: String,
    pub lifecycle_head_digest: String,
    pub pointer_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleLedgerDocument {
    pub schema_version: String,
    pub domain_pack_lifecycle_ledger: DomainPackLifecycleLedger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleLedger {
    pub project_id: StableId,
    pub records: Vec<DomainPackLifecycleLedgerRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleLedgerRecord {
    pub sequence: u64,
    pub previous_record_digest: Option<String>,
    pub operation: DomainPackLifecycleOperation,
    pub request_digest: String,
    pub preflight_digest: String,
    pub from_pointer_digest: Option<String>,
    pub to_generation: u64,
    pub active_lock_digest: String,
    pub compatibility_report_digest: String,
    pub principal_id: StableId,
    pub observed_at_unix: u64,
    pub record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleReceiptDocument {
    pub schema_version: String,
    pub domain_pack_lifecycle_receipt: DomainPackLifecycleReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLifecycleReceipt {
    pub receipt_id: StableId,
    pub operation: DomainPackLifecycleOperation,
    pub principal_id: StableId,
    pub request_digest: String,
    pub preflight_digest: String,
    pub resolution_digest: String,
    pub composition_digest: String,
    pub compatibility_report_digest: String,
    pub trust_policy_digest: String,
    pub reviewer_registry_digest: String,
    pub reviewed_registry_digest: String,
    pub capability_registry_digest: String,
    pub sandbox_policy_digest: String,
    pub from_state: Option<DomainPackActivePointer>,
    pub to_state: DomainPackActivePointer,
    pub prior_ledger_head_digest: Option<String>,
    pub new_ledger_head_digest: String,
    pub applied_object_digests: Vec<String>,
    pub observed_at_unix: u64,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRecoveryReportDocument {
    pub schema_version: String,
    pub domain_pack_recovery_report: DomainPackRecoveryReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRecoveryReport {
    pub authority: DomainPackCandidateAuthority,
    pub status: DomainPackRecoveryStatus,
    pub active_state: Option<DomainPackActivePointer>,
    pub lifecycle_head_digest: Option<String>,
    pub repaired_artifact_refs: Vec<String>,
    pub issues: Vec<DomainPackLifecycleIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRecoveryStatus {
    Clean,
    RecoveredPrior,
    RecoveredTarget,
    BlockedAmbiguous,
}
