//! Human-authorized workflow decision and waiver capabilities.
//!
//! Callers provide a closed request plus wire attestation. They never provide
//! the [`CanonicalIntent`]: this module constructs the exact `workflow` intent,
//! verifies it against the operator-owned registry, and only then promotes a
//! human principal with the required grant into an opaque capability.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{
    ReadinessTarget, StableId, WorkflowCapabilityProbeKind, WorkflowEvaluatorProvider,
    WorkflowEvidenceKind, WorkflowEvidenceOutcome, WorkflowEvidenceStrength,
    WorkflowEvidenceSubjectKind, WorkflowGovernanceSignal,
};
use serde::{Deserialize, Serialize};

use crate::attestation::{
    attestation_fingerprints, AttestationInput, AttestationVerifier, CanonicalIntent,
};
use crate::principal_registry::{
    AuthorizedPrincipal, AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry,
    PrincipalAuthorizationError,
};

const WORKFLOW_TOOL: &str = "workflow";

/// Closed workflow observation kinds that may be signed by an operator-owned
/// credential bridge.
///
/// The serialized labels are intentionally shorter than the canonical action
/// strings. Hosts select a semantic kind; Forge alone maps it to the exact
/// action that the authority verifier binds into [`CanonicalIntent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuthorizationKind {
    Applicability,
    Capability,
    Decision,
    Evidence,
    IntentRevision,
    Signal,
    Waiver,
}

impl WorkflowAuthorizationKind {
    /// Canonical action bound into the signed `workflow` intent.
    #[must_use]
    pub const fn canonical_action(self) -> &'static str {
        match self {
            Self::Applicability => "applicability_assess",
            Self::Capability => "capability_authorize",
            Self::Decision => "decision_resolve",
            Self::Evidence => "evidence_authorize",
            Self::IntentRevision => "intent_revision_accept",
            Self::Signal => "signal_authorize",
            Self::Waiver => "waiver_authorize",
        }
    }
}

#[cfg(test)]
const DECISION_ACTION: &str = WorkflowAuthorizationKind::Decision.canonical_action();
#[cfg(test)]
const WAIVER_ACTION: &str = WorkflowAuthorizationKind::Waiver.canonical_action();
#[cfg(test)]
const EVIDENCE_ACTION: &str = WorkflowAuthorizationKind::Evidence.canonical_action();
#[cfg(test)]
const APPLICABILITY_ACTION: &str = WorkflowAuthorizationKind::Applicability.canonical_action();
#[cfg(test)]
const CAPABILITY_ACTION: &str = WorkflowAuthorizationKind::Capability.canonical_action();
#[cfg(test)]
const SIGNAL_ACTION: &str = WorkflowAuthorizationKind::Signal.canonical_action();
const DECISION_GRANT: &str = "workflow.decision.resolve";
const WAIVER_GRANT: &str = "workflow.waiver.authorize";
const EVIDENCE_HUMAN_GRANT: &str = "workflow.evidence.authorize_human";
const EVIDENCE_REVIEW_GRANT: &str = "workflow.evidence.authorize_review";
const EVIDENCE_RUNTIME_GRANT: &str = "workflow.evidence.authorize_runtime";
const EVIDENCE_EXTERNAL_GRANT: &str = "workflow.evidence.authorize_external";
const APPLICABILITY_GRANT: &str = "workflow.applicability.assess";
const CAPABILITY_GRANT: &str = "workflow.capability.authorize";
const SIGNAL_GRANT: &str = "workflow.signal.authorize";
/// Closed evaluator identity for signed applicability assessments.
///
/// This is kernel-owned authority metadata, not a caller-selected label.
pub const WORKFLOW_APPLICABILITY_EVALUATOR_REF: &str = "evaluator.workflow.applicability.human";
/// Closed authority scope for signed applicability assessments.
pub const WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE: &str = "workflow.applicability.assess";
/// Closed authority scope for signed capability observations.
pub const WORKFLOW_CAPABILITY_AUTHORITY_SCOPE: &str = "workflow.capability.authorize";
const HUMAN_ROLES: &[CallerRole] = &[CallerRole::Human];
const REVIEW_ROLES: &[CallerRole] = &[CallerRole::Worker, CallerRole::Driver];
const RUNTIME_ROLES: &[CallerRole] = &[CallerRole::Runtime];
const EXTERNAL_ROLES: &[CallerRole] = &[CallerRole::Worker, CallerRole::Runtime];

/// Exact decision selection a human is asked to authorize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDecisionAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub selected_alternative_ref: StableId,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub readiness_target: String,
    pub consequences_ack_digest: String,
}

/// The one governance requirement waived by a human authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum WorkflowWaiverSubject {
    Claim { claim_ref: StableId },
    Obligation { obligation_ref: StableId },
}

/// Exact, bounded waiver a human is asked to authorize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWaiverAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub policy_ref: StableId,
    pub subject: WorkflowWaiverSubject,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub maximum_readiness_target: String,
    pub reason: String,
    pub consequences_ack_digest: String,
    pub expires_at_unix: i64,
}

/// Exact evaluator observation a trusted human principal is asked to authorize.
///
/// This request deliberately binds the semantic evidence classification and
/// the subject digest as well as the project snapshot. It is not a generic
/// signed statement and cannot be replayed as evidence for another claim,
/// evaluator, phase, target, or repository state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub provider: WorkflowEvaluatorProvider,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub outcome: WorkflowEvidenceOutcome,
    pub subject_kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
    /// Stable identity of the exercised scenario or inspection protocol.
    /// Repeating the same scenario at a new ledger head must not inflate the
    /// number of independent observations.
    pub scenario_digest: String,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub readiness_target: ReadinessTarget,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

/// Exact applicability result a trusted principal is asked to authorize.
///
/// The request binds both the project snapshot and the current ledger head so
/// an authorization cannot be replayed after governance state advances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowApplicabilityAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub policy_ref: StableId,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub applicable: bool,
    pub evaluator_ref: StableId,
    pub authority_scope: StableId,
    pub basis_refs: Vec<String>,
    pub basis_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

/// Exact capability probe result a trusted principal is asked to authorize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCapabilityAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub policy_ref: StableId,
    pub capability_ref: StableId,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub probe_kind: WorkflowCapabilityProbeKind,
    pub available: bool,
    pub authority_scope: StableId,
    pub probe_ref: String,
    pub probe_digest: String,
    pub subject_kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

/// Exact closed governance-signal transition authorized by an operator-owned
/// runtime or worker identity. Episode and generation make repeated signal
/// cycles distinguishable without letting a caller overwrite history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSignalAuthorizationRequest {
    pub project_id: StableId,
    pub policy_bundle_digest: String,
    pub state_version: u64,
    pub current_phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub signal: WorkflowGovernanceSignal,
    pub active: bool,
    pub episode_id: StableId,
    pub generation: u64,
    pub basis_refs: Vec<String>,
    pub basis_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowAuthorityError {
    InvalidRequest {
        field: &'static str,
        reason: &'static str,
    },
    Principal(PrincipalAuthorizationError),
    RoleRequired {
        required: &'static str,
        found: CallerRole,
    },
    MissingAuthorityGrant {
        credential_id: String,
        grant: &'static str,
    },
    WaiverExpired {
        expires_at_unix: i64,
        now_unix: i64,
    },
    UnsupportedEvidenceProvider {
        provider: WorkflowEvaluatorProvider,
    },
    EvidenceClassificationMismatch {
        provider: WorkflowEvaluatorProvider,
        kind: WorkflowEvidenceKind,
        strength: WorkflowEvidenceStrength,
    },
    EvidenceTimestampOutOfBounds {
        observed_at_unix: u64,
        expires_at_unix: Option<u64>,
        now_unix: i64,
    },
    AuthorizationTimestampOutOfBounds {
        observed_at_unix: u64,
        expires_at_unix: Option<u64>,
        now_unix: i64,
    },
    SystemClockBeforeUnixEpoch,
}

impl fmt::Display for WorkflowAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { field, reason } => {
                write!(
                    formatter,
                    "invalid workflow authorization field '{field}': {reason}"
                )
            }
            Self::Principal(error) => write!(formatter, "{error}"),
            Self::RoleRequired { required, found } => {
                write!(
                    formatter,
                    "workflow authorization requires {required}, found {found:?}"
                )
            }
            Self::MissingAuthorityGrant {
                credential_id,
                grant,
            } => write!(
                formatter,
                "credential '{credential_id}' is missing required authority grant '{grant}'"
            ),
            Self::WaiverExpired {
                expires_at_unix,
                now_unix,
            } => write!(
                formatter,
                "workflow waiver expired at {expires_at_unix}; current time is {now_unix}"
            ),
            Self::UnsupportedEvidenceProvider { provider } => write!(
                formatter,
                "workflow evidence authority does not accept provider {provider:?}"
            ),
            Self::EvidenceClassificationMismatch {
                provider,
                kind,
                strength,
            } => write!(
                formatter,
                "workflow evidence classification {provider:?}/{kind:?}/{strength:?} is not an authoritative human observation"
            ),
            Self::EvidenceTimestampOutOfBounds {
                observed_at_unix,
                expires_at_unix,
                now_unix,
            } => write!(
                formatter,
                "workflow evidence timestamps are out of bounds: observed={observed_at_unix}, expires={expires_at_unix:?}, now={now_unix}"
            ),
            Self::AuthorizationTimestampOutOfBounds {
                observed_at_unix,
                expires_at_unix,
                now_unix,
            } => write!(
                formatter,
                "workflow authorization timestamps are out of bounds: observed={observed_at_unix}, expires={expires_at_unix:?}, now={now_unix}"
            ),
            Self::SystemClockBeforeUnixEpoch => {
                write!(formatter, "system clock is before the Unix epoch")
            }
        }
    }
}

impl std::error::Error for WorkflowAuthorityError {}

impl From<PrincipalAuthorizationError> for WorkflowAuthorityError {
    fn from(error: PrincipalAuthorizationError) -> Self {
        Self::Principal(error)
    }
}

/// Opaque proof that a registry-authorized human selected one exact decision.
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowDecisionAuthorization;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedWorkflowDecisionAuthorization>();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowDecisionAuthorization;
/// let _: VerifiedWorkflowDecisionAuthorization = serde_json::from_str("{}").unwrap();
/// ```
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowDecisionAuthorization {
    request: WorkflowDecisionAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowDecisionAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowDecisionAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowDecisionAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowDecisionAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthorizedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }

    #[must_use]
    pub fn attestation_digest(&self) -> &str {
        &self.attestation_digest
    }

    #[must_use]
    pub fn signature_fingerprint(&self) -> &str {
        &self.signature_fingerprint
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowDecisionAuthorizationAudit {
        VerifiedWorkflowDecisionAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowDecisionAuthorizationAudit {
    pub request: WorkflowDecisionAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque proof that a registry-authorized human granted one exact waiver.
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowWaiverAuthorization;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedWorkflowWaiverAuthorization>();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowWaiverAuthorization;
/// let _: VerifiedWorkflowWaiverAuthorization = serde_json::from_str("{}").unwrap();
/// ```
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowWaiverAuthorization {
    request: WorkflowWaiverAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowWaiverAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowWaiverAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowWaiverAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowWaiverAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthorizedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }

    #[must_use]
    pub fn attestation_digest(&self) -> &str {
        &self.attestation_digest
    }

    #[must_use]
    pub fn signature_fingerprint(&self) -> &str {
        &self.signature_fingerprint
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowWaiverAuthorizationAudit {
        VerifiedWorkflowWaiverAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowWaiverAuthorizationAudit {
    pub request: WorkflowWaiverAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque proof that a registry-authorized human authorized one exact
/// evaluator observation.
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowEvidenceAuthorization;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedWorkflowEvidenceAuthorization>();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowEvidenceAuthorization;
/// let _: VerifiedWorkflowEvidenceAuthorization = serde_json::from_str("{}").unwrap();
/// ```
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowEvidenceAuthorization {
    request: WorkflowEvidenceAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowEvidenceAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowEvidenceAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowEvidenceAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowEvidenceAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthorizedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }

    #[must_use]
    pub fn attestation_digest(&self) -> &str {
        &self.attestation_digest
    }

    #[must_use]
    pub fn signature_fingerprint(&self) -> &str {
        &self.signature_fingerprint
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowEvidenceAuthorizationAudit {
        VerifiedWorkflowEvidenceAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowEvidenceAuthorizationAudit {
    pub request: WorkflowEvidenceAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque proof for one exact applicability assessment.
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowApplicabilityAuthorization {
    request: WorkflowApplicabilityAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowApplicabilityAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowApplicabilityAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowApplicabilityAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowApplicabilityAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthorizedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowApplicabilityAuthorizationAudit {
        VerifiedWorkflowApplicabilityAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowApplicabilityAuthorizationAudit {
    pub request: WorkflowApplicabilityAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque applicability capability returned by the authorization factory.
pub type WorkflowApplicabilityAuthorization = VerifiedWorkflowApplicabilityAuthorization;

/// Opaque proof for one exact capability probe result.
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowCapabilityAuthorization {
    request: WorkflowCapabilityAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowCapabilityAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowCapabilityAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowCapabilityAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowCapabilityAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthorizedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowCapabilityAuthorizationAudit {
        VerifiedWorkflowCapabilityAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowCapabilityAuthorizationAudit {
    pub request: WorkflowCapabilityAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque capability-probe authorization returned by the factory.
pub type WorkflowCapabilityAuthorization = VerifiedWorkflowCapabilityAuthorization;

/// Opaque proof for one exact closed governance-signal transition.
#[derive(PartialEq, Eq)]
pub struct VerifiedWorkflowSignalAuthorization {
    request: WorkflowSignalAuthorizationRequest,
    principal: AuthorizedPrincipal,
    intent_digest: String,
    attestation_digest: String,
    signature_fingerprint: String,
}

impl fmt::Debug for VerifiedWorkflowSignalAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowSignalAuthorization")
            .field("request", &self.request)
            .field("principal", &self.principal.audit())
            .field("intent_digest", &self.intent_digest)
            .field("attestation_digest", &self.attestation_digest)
            .field("signature_fingerprint", &self.signature_fingerprint)
            .finish()
    }
}

impl VerifiedWorkflowSignalAuthorization {
    #[must_use]
    pub const fn request(&self) -> &WorkflowSignalAuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowSignalAuthorizationAudit {
        VerifiedWorkflowSignalAuthorizationAudit {
            request: self.request.clone(),
            principal: self.principal.audit(),
            intent_digest: self.intent_digest.clone(),
            attestation_digest: self.attestation_digest.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct VerifiedWorkflowSignalAuthorizationAudit {
    pub request: WorkflowSignalAuthorizationRequest,
    pub principal: AuthorizedPrincipalAudit,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature_fingerprint: String,
}

/// Opaque signal authorization returned by the factory.
pub type WorkflowSignalAuthorization = VerifiedWorkflowSignalAuthorization;

impl AuthorizedPrincipalRegistry {
    /// Authorize an exact human decision using the trusted system clock and
    /// crate-owned attestation freshness policy.
    ///
    /// # Errors
    ///
    /// Fails closed when the request, clock, signature, registry identity,
    /// role, or authority grant is invalid.
    pub fn authorize_workflow_decision(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowDecisionAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowDecisionAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_decision_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    fn authorize_workflow_decision_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowDecisionAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowDecisionAuthorization, WorkflowAuthorityError> {
        validate_decision_request(&request)?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Decision.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(
            &principal,
            &[CallerRole::Human],
            "human role",
            DECISION_GRANT,
        )?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowDecisionAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    /// Authorize an exact, bounded waiver using the trusted system clock.
    ///
    /// # Errors
    ///
    /// Fails closed when the request, clock, signature, registry identity,
    /// role, authority grant, or waiver lifetime is invalid.
    pub fn authorize_workflow_waiver(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowWaiverAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowWaiverAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_waiver_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    fn authorize_workflow_waiver_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowWaiverAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowWaiverAuthorization, WorkflowAuthorityError> {
        validate_waiver_request(&request, now_unix)?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Waiver.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(&principal, &[CallerRole::Human], "human role", WAIVER_GRANT)?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowWaiverAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    /// Authorize one provider-classified evaluator observation. Provider,
    /// evidence kind, strength, registry role, and authority grant are a
    /// closed matrix; no generic evidence grant is accepted.
    ///
    /// # Errors
    ///
    /// Fails closed for any invalid classification, timestamp, signature,
    /// registry identity, provider role, or provider-specific grant.
    pub fn authorize_workflow_evidence(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowEvidenceAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowEvidenceAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_evidence_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    fn authorize_workflow_evidence_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowEvidenceAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowEvidenceAuthorization, WorkflowAuthorityError> {
        validate_evidence_request(
            &request,
            now_unix,
            crate::principal_registry::DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        )?;
        let (roles, role_description, grant) = evidence_authority(&request)?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Evidence.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(&principal, roles, role_description, grant)?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowEvidenceAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    /// Authorize one exact applicability result against a specific trusted
    /// snapshot and ledger head.
    ///
    /// # Errors
    ///
    /// Fails closed for invalid bindings, timestamps, signature, registry
    /// identity, human role, or applicability grant.
    pub fn authorize_workflow_applicability(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowApplicabilityAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowApplicabilityAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_applicability_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    fn authorize_workflow_applicability_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowApplicabilityAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowApplicabilityAuthorization, WorkflowAuthorityError> {
        validate_applicability_request(
            &request,
            now_unix,
            crate::principal_registry::DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        )?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Applicability.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(
            &principal,
            &[CallerRole::Human],
            "human role",
            APPLICABILITY_GRANT,
        )?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowApplicabilityAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    /// Authorize one exact capability result against a specific trusted
    /// snapshot and ledger head.
    ///
    /// # Errors
    ///
    /// Fails closed for invalid bindings, timestamps, signature, registry
    /// identity, runtime role, or capability grant.
    pub fn authorize_workflow_capability(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowCapabilityAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowCapabilityAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_capability_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    /// Authorize one exact governance-signal transition using the trusted
    /// system clock and a closed runtime/worker role plus grant matrix.
    ///
    /// # Errors
    /// Fails closed for invalid binding, episode/generation, timestamps,
    /// signature, registry identity, role, or authority grant.
    pub fn authorize_workflow_signal(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowSignalAuthorizationRequest,
        attestation: &AttestationInput,
    ) -> Result<VerifiedWorkflowSignalAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_signal_with_clock(
            verifier,
            request,
            attestation,
            system_now_unix()?,
        )
    }

    fn authorize_workflow_signal_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowSignalAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowSignalAuthorization, WorkflowAuthorityError> {
        validate_signal_request(
            &request,
            now_unix,
            crate::principal_registry::DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        )?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Signal.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(
            &principal,
            &[CallerRole::Runtime, CallerRole::Worker, CallerRole::Driver],
            "runtime, worker, or driver role",
            SIGNAL_GRANT,
        )?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowSignalAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    fn authorize_workflow_capability_with_clock(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowCapabilityAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowCapabilityAuthorization, WorkflowAuthorityError> {
        validate_capability_request(
            &request,
            now_unix,
            crate::principal_registry::DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        )?;
        let intent = workflow_intent(
            WorkflowAuthorizationKind::Capability.canonical_action(),
            &request,
            attestation,
        )?;
        let principal =
            authorize_with_default_policy(self, verifier, &intent, attestation, now_unix)?;
        require_role_grant(
            &principal,
            &[CallerRole::Runtime],
            "runtime role",
            CAPABILITY_GRANT,
        )?;
        let (intent_digest, attestation_digest, signature_fingerprint) =
            authorization_fingerprints(&intent, attestation)?;
        Ok(VerifiedWorkflowCapabilityAuthorization {
            request,
            principal,
            intent_digest,
            attestation_digest,
            signature_fingerprint,
        })
    }

    #[cfg(test)]
    fn authorize_workflow_decision_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowDecisionAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
        _max_age_seconds: u64,
        _max_future_skew_seconds: u64,
    ) -> Result<VerifiedWorkflowDecisionAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_decision_with_clock(verifier, request, attestation, now_unix)
    }

    #[cfg(test)]
    fn authorize_workflow_waiver_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowWaiverAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
        _max_age_seconds: u64,
        _max_future_skew_seconds: u64,
    ) -> Result<VerifiedWorkflowWaiverAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_waiver_with_clock(verifier, request, attestation, now_unix)
    }

    #[cfg(test)]
    fn authorize_workflow_evidence_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowEvidenceAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
        _max_age_seconds: u64,
        _max_future_skew_seconds: u64,
    ) -> Result<VerifiedWorkflowEvidenceAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_evidence_with_clock(verifier, request, attestation, now_unix)
    }

    #[cfg(test)]
    fn authorize_workflow_applicability_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowApplicabilityAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowApplicabilityAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_applicability_with_clock(verifier, request, attestation, now_unix)
    }

    #[cfg(test)]
    fn authorize_workflow_capability_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowCapabilityAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowCapabilityAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_capability_with_clock(verifier, request, attestation, now_unix)
    }

    #[cfg(test)]
    fn authorize_workflow_signal_at(
        &self,
        verifier: &AttestationVerifier,
        request: WorkflowSignalAuthorizationRequest,
        attestation: &AttestationInput,
        now_unix: i64,
    ) -> Result<VerifiedWorkflowSignalAuthorization, WorkflowAuthorityError> {
        self.authorize_workflow_signal_with_clock(verifier, request, attestation, now_unix)
    }
}

fn workflow_intent<T: Serialize>(
    action: &'static str,
    request: &T,
    attestation: &AttestationInput,
) -> Result<CanonicalIntent, WorkflowAuthorityError> {
    let request =
        serde_json::to_value(request).map_err(|_| WorkflowAuthorityError::InvalidRequest {
            field: "request",
            reason: "must be serializable as canonical JSON",
        })?;
    Ok(CanonicalIntent {
        tool: WORKFLOW_TOOL.to_owned(),
        arguments: serde_json::json!({"action": action, "request": request}),
        credential_id: attestation.credential_id.clone(),
        audience: attestation.audience.clone(),
        execution_intent_digest: attestation.execution_intent_digest.clone(),
        nonce: attestation.nonce.clone(),
        ts: attestation.ts,
    })
}

fn require_role_grant(
    principal: &AuthorizedPrincipal,
    accepted_roles: &[CallerRole],
    required: &'static str,
    grant: &'static str,
) -> Result<(), WorkflowAuthorityError> {
    if !accepted_roles.contains(&principal.role()) {
        return Err(WorkflowAuthorityError::RoleRequired {
            required,
            found: principal.role(),
        });
    }
    if !principal
        .authority_grants()
        .iter()
        .any(|candidate| candidate.0 == grant)
    {
        return Err(WorkflowAuthorityError::MissingAuthorityGrant {
            credential_id: principal.credential_id().to_owned(),
            grant,
        });
    }
    Ok(())
}

fn authorize_with_default_policy(
    registry: &AuthorizedPrincipalRegistry,
    verifier: &AttestationVerifier,
    intent: &CanonicalIntent,
    attestation: &AttestationInput,
    now_unix: i64,
) -> Result<AuthorizedPrincipal, WorkflowAuthorityError> {
    registry
        .authorize(
            verifier,
            intent,
            attestation,
            now_unix,
            crate::principal_registry::DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
            crate::principal_registry::DEFAULT_MAX_FUTURE_SKEW_SECONDS,
        )
        .map_err(Into::into)
}

fn authorization_fingerprints(
    intent: &CanonicalIntent,
    attestation: &AttestationInput,
) -> Result<(String, String, String), WorkflowAuthorityError> {
    let intent_digest = intent
        .digest()
        .map_err(PrincipalAuthorizationError::Attestation)?;
    let (attestation_digest, signature_fingerprint) = attestation_fingerprints(intent, attestation)
        .map_err(PrincipalAuthorizationError::Attestation)?;
    Ok((intent_digest, attestation_digest, signature_fingerprint))
}

fn system_now_unix() -> Result<i64, WorkflowAuthorityError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WorkflowAuthorityError::SystemClockBeforeUnixEpoch)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| WorkflowAuthorityError::SystemClockBeforeUnixEpoch)
}

fn evidence_authority(
    request: &WorkflowEvidenceAuthorizationRequest,
) -> Result<(&'static [CallerRole], &'static str, &'static str), WorkflowAuthorityError> {
    let authority = match (request.provider, request.kind, request.strength) {
        (
            WorkflowEvaluatorProvider::AuthorizedHuman,
            WorkflowEvidenceKind::HumanAcceptance,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ) => (HUMAN_ROLES, "human role", EVIDENCE_HUMAN_GRANT),
        (
            WorkflowEvaluatorProvider::IndependentReviewer,
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ) => (REVIEW_ROLES, "worker or driver role", EVIDENCE_REVIEW_GRANT),
        (
            WorkflowEvaluatorProvider::RepositoryInspector,
            WorkflowEvidenceKind::ArtifactInspection,
            WorkflowEvidenceStrength::InspectedArtifact,
        )
        | (
            WorkflowEvaluatorProvider::DeterministicTool,
            WorkflowEvidenceKind::DeterministicCheck,
            WorkflowEvidenceStrength::DeterministicVerification,
        )
        | (
            WorkflowEvaluatorProvider::RepresentativeRuntime,
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ) => (RUNTIME_ROLES, "runtime role", EVIDENCE_RUNTIME_GRANT),
        (
            WorkflowEvaluatorProvider::ExternalAuthority,
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        )
        | (
            WorkflowEvaluatorProvider::ResearchSource,
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ) => (
            EXTERNAL_ROLES,
            "worker or runtime role",
            EVIDENCE_EXTERNAL_GRANT,
        ),
        (provider, kind, strength) => {
            return Err(WorkflowAuthorityError::EvidenceClassificationMismatch {
                provider,
                kind,
                strength,
            });
        }
    };
    Ok(authority)
}

fn validate_decision_request(
    request: &WorkflowDecisionAuthorizationRequest,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable("project_id", &request.project_id)?;
    digest("policy_bundle_digest", &request.policy_bundle_digest)?;
    nonblank_stable("policy_ref", &request.policy_ref)?;
    nonblank_stable("decision_ref", &request.decision_ref)?;
    nonblank_stable(
        "selected_alternative_ref",
        &request.selected_alternative_ref,
    )?;
    nonblank_stable("current_phase", &request.current_phase)?;
    digest("snapshot_digest", &request.snapshot_digest)?;
    digest("ledger_head_digest", &request.ledger_head_digest)?;
    nonblank("readiness_target", &request.readiness_target)?;
    digest("consequences_ack_digest", &request.consequences_ack_digest)
}

fn validate_waiver_request(
    request: &WorkflowWaiverAuthorizationRequest,
    now_unix: i64,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable("project_id", &request.project_id)?;
    digest("policy_bundle_digest", &request.policy_bundle_digest)?;
    nonblank_stable("policy_ref", &request.policy_ref)?;
    match &request.subject {
        WorkflowWaiverSubject::Claim { claim_ref } => nonblank_stable("claim_ref", claim_ref)?,
        WorkflowWaiverSubject::Obligation { obligation_ref } => {
            nonblank_stable("obligation_ref", obligation_ref)?;
        }
    }
    nonblank_stable("current_phase", &request.current_phase)?;
    digest("snapshot_digest", &request.snapshot_digest)?;
    digest("ledger_head_digest", &request.ledger_head_digest)?;
    nonblank(
        "maximum_readiness_target",
        &request.maximum_readiness_target,
    )?;
    nonblank("reason", &request.reason)?;
    digest("consequences_ack_digest", &request.consequences_ack_digest)?;
    if request.expires_at_unix <= now_unix {
        return Err(WorkflowAuthorityError::WaiverExpired {
            expires_at_unix: request.expires_at_unix,
            now_unix,
        });
    }
    Ok(())
}

fn validate_evidence_request(
    request: &WorkflowEvidenceAuthorizationRequest,
    now_unix: i64,
    max_future_skew_seconds: u64,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable("project_id", &request.project_id)?;
    digest("policy_bundle_digest", &request.policy_bundle_digest)?;
    nonblank_stable("policy_ref", &request.policy_ref)?;
    nonblank_stable("claim_ref", &request.claim_ref)?;
    nonblank_stable("evaluator_ref", &request.evaluator_ref)?;
    nonblank("subject_ref", &request.subject_ref)?;
    digest("subject_digest", &request.subject_digest)?;
    digest("scenario_digest", &request.scenario_digest)?;
    nonblank_stable("current_phase", &request.current_phase)?;
    digest("snapshot_digest", &request.snapshot_digest)?;
    digest("ledger_head_digest", &request.ledger_head_digest)?;

    evidence_authority(request)?;

    let now = u64::try_from(now_unix).unwrap_or(0);
    let future_limit = now.saturating_add(max_future_skew_seconds);
    let invalid_expiry = request
        .expires_at_unix
        .is_some_and(|expires| expires <= request.observed_at_unix || expires <= now);
    if request.observed_at_unix == 0 || request.observed_at_unix > future_limit || invalid_expiry {
        return Err(WorkflowAuthorityError::EvidenceTimestampOutOfBounds {
            observed_at_unix: request.observed_at_unix,
            expires_at_unix: request.expires_at_unix,
            now_unix,
        });
    }
    Ok(())
}

fn validate_applicability_request(
    request: &WorkflowApplicabilityAuthorizationRequest,
    now_unix: i64,
    max_future_skew_seconds: u64,
) -> Result<(), WorkflowAuthorityError> {
    validate_snapshot_binding(
        &request.project_id,
        &request.policy_bundle_digest,
        &request.policy_ref,
        &request.current_phase,
        &request.snapshot_digest,
        &request.ledger_head_digest,
    )?;
    exact_stable(
        "evaluator_ref",
        &request.evaluator_ref,
        WORKFLOW_APPLICABILITY_EVALUATOR_REF,
    )?;
    exact_stable(
        "authority_scope",
        &request.authority_scope,
        WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE,
    )?;
    if request.basis_refs.is_empty()
        || request
            .basis_refs
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(WorkflowAuthorityError::InvalidRequest {
            field: "basis_refs",
            reason: "must contain only non-blank basis references",
        });
    }
    digest("basis_digest", &request.basis_digest)?;
    validate_authorization_timestamps(
        request.observed_at_unix,
        Some(request.expires_at_unix),
        now_unix,
        max_future_skew_seconds,
    )
}

fn validate_capability_request(
    request: &WorkflowCapabilityAuthorizationRequest,
    now_unix: i64,
    max_future_skew_seconds: u64,
) -> Result<(), WorkflowAuthorityError> {
    validate_snapshot_binding(
        &request.project_id,
        &request.policy_bundle_digest,
        &request.policy_ref,
        &request.current_phase,
        &request.snapshot_digest,
        &request.ledger_head_digest,
    )?;
    nonblank_stable("capability_ref", &request.capability_ref)?;
    exact_stable(
        "authority_scope",
        &request.authority_scope,
        WORKFLOW_CAPABILITY_AUTHORITY_SCOPE,
    )?;
    nonblank("probe_ref", &request.probe_ref)?;
    digest("probe_digest", &request.probe_digest)?;
    nonblank("subject_ref", &request.subject_ref)?;
    digest("subject_digest", &request.subject_digest)?;
    validate_authorization_timestamps(
        request.observed_at_unix,
        request.expires_at_unix,
        now_unix,
        max_future_skew_seconds,
    )
}

fn validate_signal_request(
    request: &WorkflowSignalAuthorizationRequest,
    now_unix: i64,
    max_future_skew_seconds: u64,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable("project_id", &request.project_id)?;
    digest("policy_bundle_digest", &request.policy_bundle_digest)?;
    nonblank_stable("current_phase", &request.current_phase)?;
    digest("snapshot_digest", &request.snapshot_digest)?;
    digest("ledger_head_digest", &request.ledger_head_digest)?;
    nonblank_stable("episode_id", &request.episode_id)?;
    if request.generation == 0 {
        return Err(WorkflowAuthorityError::InvalidRequest {
            field: "generation",
            reason: "must be greater than zero",
        });
    }
    if request.basis_refs.is_empty()
        || request
            .basis_refs
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(WorkflowAuthorityError::InvalidRequest {
            field: "basis_refs",
            reason: "must contain only non-blank basis references",
        });
    }
    digest("basis_digest", &request.basis_digest)?;
    validate_authorization_timestamps(
        request.observed_at_unix,
        Some(request.expires_at_unix),
        now_unix,
        max_future_skew_seconds,
    )
}

fn validate_snapshot_binding(
    project_id: &StableId,
    policy_bundle_digest: &str,
    policy_ref: &StableId,
    current_phase: &StableId,
    snapshot_digest: &str,
    ledger_head_digest: &str,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable("project_id", project_id)?;
    digest("policy_bundle_digest", policy_bundle_digest)?;
    nonblank_stable("policy_ref", policy_ref)?;
    nonblank_stable("current_phase", current_phase)?;
    digest("snapshot_digest", snapshot_digest)?;
    digest("ledger_head_digest", ledger_head_digest)
}

fn validate_authorization_timestamps(
    observed_at_unix: u64,
    expires_at_unix: Option<u64>,
    now_unix: i64,
    max_future_skew_seconds: u64,
) -> Result<(), WorkflowAuthorityError> {
    let now = u64::try_from(now_unix).unwrap_or(0);
    let future_limit = now.saturating_add(max_future_skew_seconds);
    let invalid_expiry =
        expires_at_unix.is_some_and(|expires| expires <= observed_at_unix || expires <= now);
    if observed_at_unix == 0 || observed_at_unix > future_limit || invalid_expiry {
        return Err(WorkflowAuthorityError::AuthorizationTimestampOutOfBounds {
            observed_at_unix,
            expires_at_unix,
            now_unix,
        });
    }
    Ok(())
}

fn nonblank_stable(field: &'static str, value: &StableId) -> Result<(), WorkflowAuthorityError> {
    nonblank(field, &value.0)
}

fn exact_stable(
    field: &'static str,
    value: &StableId,
    expected: &'static str,
) -> Result<(), WorkflowAuthorityError> {
    nonblank_stable(field, value)?;
    if value.0 == expected {
        Ok(())
    } else {
        Err(WorkflowAuthorityError::InvalidRequest {
            field,
            reason: "must match the closed workflow authority value",
        })
    }
}

fn nonblank(field: &'static str, value: &str) -> Result<(), WorkflowAuthorityError> {
    if value.trim().is_empty() {
        Err(WorkflowAuthorityError::InvalidRequest {
            field,
            reason: "must not be blank",
        })
    } else {
        Ok(())
    }
}

fn digest(field: &'static str, value: &str) -> Result<(), WorkflowAuthorityError> {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if valid {
        Ok(())
    } else {
        Err(WorkflowAuthorityError::InvalidRequest {
            field,
            reason: "must be a sha256: digest with 64 hexadecimal characters",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::{hex_encode, AttestationError, AttestationPolicy};
    use crate::principal_registry::{
        PrincipalCredentialStatus, PrincipalRegistryContract, PrincipalRegistryDocument,
        PrincipalRegistryEntry, DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
        DEFAULT_MAX_FUTURE_SKEW_SECONDS, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_contracts::PrincipalId;

    const NOW: i64 = 1_800_000_000;
    const AUDIENCE: &str = "forge-core:workflow:test-project";
    const CREDENTIAL: &str = "key.human.2026-01";

    fn decision_request() -> WorkflowDecisionAuthorizationRequest {
        WorkflowDecisionAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            policy_ref: StableId("policy.release".to_owned()),
            decision_ref: StableId("decision.scope".to_owned()),
            selected_alternative_ref: StableId("alternative.safe".to_owned()),
            state_version: 12,
            current_phase: StableId("phase.delivery".to_owned()),
            snapshot_digest: format!("sha256:{}", "c".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "d".repeat(64)),
            readiness_target: "release".to_owned(),
            consequences_ack_digest: format!("sha256:{}", "b".repeat(64)),
        }
    }

    fn waiver_request() -> WorkflowWaiverAuthorizationRequest {
        WorkflowWaiverAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            policy_ref: StableId("policy.release".to_owned()),
            subject: WorkflowWaiverSubject::Obligation {
                obligation_ref: StableId("obligation.accessibility".to_owned()),
            },
            state_version: 12,
            current_phase: StableId("phase.delivery".to_owned()),
            snapshot_digest: format!("sha256:{}", "d".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "e".repeat(64)),
            maximum_readiness_target: "execute".to_owned(),
            reason: "Temporary external test environment outage".to_owned(),
            consequences_ack_digest: format!("sha256:{}", "c".repeat(64)),
            expires_at_unix: NOW + 3_600,
        }
    }

    fn evidence_request(
        provider: WorkflowEvaluatorProvider,
    ) -> WorkflowEvidenceAuthorizationRequest {
        let (kind, strength) = match provider {
            WorkflowEvaluatorProvider::AuthorizedHuman => (
                WorkflowEvidenceKind::HumanAcceptance,
                WorkflowEvidenceStrength::AuthoritativeAcceptance,
            ),
            WorkflowEvaluatorProvider::IndependentReviewer => (
                WorkflowEvidenceKind::IndependentReview,
                WorkflowEvidenceStrength::IndependentConfirmation,
            ),
            WorkflowEvaluatorProvider::RepositoryInspector => (
                WorkflowEvidenceKind::ArtifactInspection,
                WorkflowEvidenceStrength::InspectedArtifact,
            ),
            WorkflowEvaluatorProvider::DeterministicTool => (
                WorkflowEvidenceKind::DeterministicCheck,
                WorkflowEvidenceStrength::DeterministicVerification,
            ),
            WorkflowEvaluatorProvider::RepresentativeRuntime => (
                WorkflowEvidenceKind::RepresentativeExecution,
                WorkflowEvidenceStrength::RepresentativeExecution,
            ),
            WorkflowEvaluatorProvider::ExternalAuthority => (
                WorkflowEvidenceKind::ExternalAuthority,
                WorkflowEvidenceStrength::AuthoritativeAcceptance,
            ),
            WorkflowEvaluatorProvider::ResearchSource => (
                WorkflowEvidenceKind::Research,
                WorkflowEvidenceStrength::IndependentConfirmation,
            ),
        };
        WorkflowEvidenceAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            policy_ref: StableId("policy.release".to_owned()),
            claim_ref: StableId("claim.usable".to_owned()),
            evaluator_ref: StableId("evaluator.human-acceptance".to_owned()),
            provider,
            kind,
            strength,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
            subject_ref: "project:test@state:12".to_owned(),
            subject_digest: format!("sha256:{}", "d".repeat(64)),
            scenario_digest: format!("sha256:{}", "9".repeat(64)),
            state_version: 12,
            current_phase: StableId("phase.delivery".to_owned()),
            snapshot_digest: format!("sha256:{}", "e".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "f".repeat(64)),
            readiness_target: ReadinessTarget::Release,
            observed_at_unix: u64::try_from(NOW - 5).expect("positive fixture timestamp"),
            expires_at_unix: Some(u64::try_from(NOW + 3_600).expect("positive fixture timestamp")),
        }
    }

    fn applicability_request() -> WorkflowApplicabilityAuthorizationRequest {
        WorkflowApplicabilityAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            policy_ref: StableId("policy.domain-scan".to_owned()),
            state_version: 12,
            current_phase: StableId("phase.discovery".to_owned()),
            snapshot_digest: format!("sha256:{}", "b".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "c".repeat(64)),
            applicable: true,
            evaluator_ref: StableId(WORKFLOW_APPLICABILITY_EVALUATOR_REF.to_owned()),
            authority_scope: StableId(WORKFLOW_APPLICABILITY_AUTHORITY_SCOPE.to_owned()),
            basis_refs: vec!["artifact:brief".to_owned()],
            basis_digest: format!("sha256:{}", "d".repeat(64)),
            observed_at_unix: u64::try_from(NOW - 5).expect("fixture time"),
            expires_at_unix: u64::try_from(NOW + 3_600).expect("fixture time"),
        }
    }

    fn capability_request() -> WorkflowCapabilityAuthorizationRequest {
        WorkflowCapabilityAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            policy_ref: StableId("policy.build-story".to_owned()),
            capability_ref: StableId("capability.test-runner".to_owned()),
            state_version: 12,
            current_phase: StableId("phase.delivery".to_owned()),
            snapshot_digest: format!("sha256:{}", "b".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "c".repeat(64)),
            probe_kind: WorkflowCapabilityProbeKind::RuntimeHandshake,
            available: true,
            authority_scope: StableId(WORKFLOW_CAPABILITY_AUTHORITY_SCOPE.to_owned()),
            probe_ref: "runtime:test-runner".to_owned(),
            probe_digest: format!("sha256:{}", "d".repeat(64)),
            subject_kind: WorkflowEvidenceSubjectKind::Runtime,
            subject_ref: "runtime:test-runner@12".to_owned(),
            subject_digest: format!("sha256:{}", "e".repeat(64)),
            observed_at_unix: u64::try_from(NOW - 5).expect("fixture time"),
            expires_at_unix: Some(u64::try_from(NOW + 3_600).expect("fixture time")),
        }
    }

    fn signal_request() -> WorkflowSignalAuthorizationRequest {
        WorkflowSignalAuthorizationRequest {
            project_id: StableId("project.test".to_owned()),
            policy_bundle_digest: format!("sha256:{}", "a".repeat(64)),
            state_version: 12,
            current_phase: StableId("phase.delivery".to_owned()),
            snapshot_digest: format!("sha256:{}", "b".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "c".repeat(64)),
            signal: WorkflowGovernanceSignal::CourseCorrectionRequired,
            active: true,
            episode_id: StableId("episode.course-correction.1".to_owned()),
            generation: 1,
            basis_refs: vec!["evidence/disproof.json".to_owned()],
            basis_digest: format!("sha256:{}", "d".repeat(64)),
            observed_at_unix: u64::try_from(NOW - 5).expect("fixture time"),
            expires_at_unix: u64::try_from(NOW + 3_600).expect("fixture time"),
        }
    }

    fn registry(
        key: &SigningKey,
        role: CallerRole,
        grants: &[&str],
        status: PrincipalCredentialStatus,
    ) -> AuthorizedPrincipalRegistry {
        AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: AUDIENCE.to_owned(),
                principals: vec![PrincipalRegistryEntry {
                    credential_id: CREDENTIAL.to_owned(),
                    principal_id: PrincipalId("principal.human".to_owned()),
                    agent_id: StableId("human-console".to_owned()),
                    role,
                    public_key_hex: hex_encode(&key.verifying_key().to_bytes()),
                    allowed_tools: vec![StableId(WORKFLOW_TOOL.to_owned())],
                    authority_grants: grants
                        .iter()
                        .map(|grant| StableId((*grant).to_owned()))
                        .collect(),
                    status,
                }],
            },
        })
        .expect("valid test registry")
    }

    fn metadata() -> AttestationInput {
        AttestationInput {
            credential_id: Some(CREDENTIAL.to_owned()),
            audience: Some(AUDIENCE.to_owned()),
            execution_intent_digest: None,
            nonce: "workflow-authority-nonce-0001".to_owned(),
            ts: NOW - 5,
            signature: String::new(),
            public_key_hex: String::new(),
        }
    }

    fn sign<T: Serialize>(
        key: &SigningKey,
        action: &'static str,
        request: &T,
        mut attestation: AttestationInput,
    ) -> AttestationInput {
        attestation.public_key_hex = hex_encode(&key.verifying_key().to_bytes());
        let intent = workflow_intent(action, request, &attestation).expect("workflow intent");
        attestation.signature = hex_encode(
            &key.sign(&intent.canonical_bytes().expect("canonical intent"))
                .to_bytes(),
        );
        attestation
    }

    fn verifier() -> AttestationVerifier {
        AttestationVerifier::new(AttestationPolicy::Default)
    }

    #[test]
    fn decision_and_waiver_happy_paths_produce_opaque_audits() {
        let key = SigningKey::from_bytes(&[21; 32]);
        let registry = registry(
            &key,
            CallerRole::Human,
            &[DECISION_GRANT, WAIVER_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let decision = decision_request();
        let decision_attestation = sign(&key, DECISION_ACTION, &decision, metadata());
        let authorization = registry
            .authorize_workflow_decision_at(
                &verifier(),
                decision.clone(),
                &decision_attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect("human decision authorization");
        assert_eq!(authorization.request(), &decision);
        assert_eq!(authorization.principal().role(), CallerRole::Human);
        assert!(authorization.intent_digest().starts_with("sha256:"));
        let audit = serde_json::to_value(authorization.audit()).expect("decision audit");
        assert_eq!(
            audit["request"]["selected_alternative_ref"],
            "alternative.safe"
        );
        assert!(!audit.to_string().contains(&decision_attestation.signature));

        let waiver = waiver_request();
        let waiver_attestation = sign(&key, WAIVER_ACTION, &waiver, metadata());
        let authorization = registry
            .authorize_workflow_waiver_at(
                &verifier(),
                waiver.clone(),
                &waiver_attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect("human waiver authorization");
        assert_eq!(authorization.request(), &waiver);
        assert_eq!(authorization.principal().role(), CallerRole::Human);
        assert!(authorization.signature_fingerprint().starts_with("sha256:"));
    }

    #[test]
    fn wrong_role_and_missing_grants_fail_closed() {
        let key = SigningKey::from_bytes(&[22; 32]);
        let decision = decision_request();
        let attestation = sign(&key, DECISION_ACTION, &decision, metadata());
        let wrong_role = registry(
            &key,
            CallerRole::Driver,
            &[DECISION_GRANT],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            wrong_role.authorize_workflow_decision_at(
                &verifier(),
                decision.clone(),
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::RoleRequired {
                found: CallerRole::Driver,
                ..
            })
        ));

        let no_grant = registry(
            &key,
            CallerRole::Human,
            &["workflow.read"],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            no_grant.authorize_workflow_decision_at(
                &verifier(),
                decision,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::MissingAuthorityGrant {
                grant: DECISION_GRANT,
                ..
            })
        ));
    }

    #[test]
    fn revoked_credential_and_request_tampering_fail_closed() {
        let key = SigningKey::from_bytes(&[23; 32]);
        let decision = decision_request();
        let attestation = sign(&key, DECISION_ACTION, &decision, metadata());
        let revoked = registry(
            &key,
            CallerRole::Human,
            &[DECISION_GRANT],
            PrincipalCredentialStatus::Revoked,
        );
        assert!(matches!(
            revoked.authorize_workflow_decision_at(
                &verifier(),
                decision.clone(),
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::CredentialRevoked(_)
            ))
        ));

        let active = registry(
            &key,
            CallerRole::Human,
            &[DECISION_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let mut tampered = decision;
        tampered.selected_alternative_ref = StableId("alternative.unsafe".to_owned());
        assert!(matches!(
            active.authorize_workflow_decision_at(
                &verifier(),
                tampered,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));
    }

    #[test]
    fn stale_audience_and_key_mismatch_fail_closed() {
        let key = SigningKey::from_bytes(&[24; 32]);
        let attacker = SigningKey::from_bytes(&[25; 32]);
        let registry = registry(
            &key,
            CallerRole::Human,
            &[WAIVER_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let request = waiver_request();

        let mut stale_meta = metadata();
        stale_meta.ts = NOW - 301;
        let stale = sign(&key, WAIVER_ACTION, &request, stale_meta);
        assert!(matches!(
            registry.authorize_workflow_waiver_at(
                &verifier(),
                request.clone(),
                &stale,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Expired)
            ))
        ));

        let mut wrong_audience_meta = metadata();
        wrong_audience_meta.audience = Some("forge-core:workflow:other".to_owned());
        let wrong_audience = sign(&key, WAIVER_ACTION, &request, wrong_audience_meta);
        assert!(matches!(
            registry.authorize_workflow_waiver_at(
                &verifier(),
                request.clone(),
                &wrong_audience,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::AudienceMismatch { .. }
            ))
        ));

        let wrong_key = sign(&attacker, WAIVER_ACTION, &request, metadata());
        assert!(matches!(
            registry.authorize_workflow_waiver_at(&verifier(), request, &wrong_key, NOW, 300, 30),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::PublicKeyMismatch(_)
            ))
        ));
    }

    #[test]
    fn waiver_is_bounded_and_closed_requests_reject_unknown_fields() {
        let key = SigningKey::from_bytes(&[26; 32]);
        let registry = registry(
            &key,
            CallerRole::Human,
            &[WAIVER_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let mut request = waiver_request();
        request.expires_at_unix = NOW;
        let attestation = sign(&key, WAIVER_ACTION, &request, metadata());
        assert!(matches!(
            registry.authorize_workflow_waiver_at(&verifier(), request, &attestation, NOW, 300, 30),
            Err(WorkflowAuthorityError::WaiverExpired { .. })
        ));

        let mut value = serde_json::to_value(decision_request()).expect("decision JSON");
        value["caller_injected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<WorkflowDecisionAuthorizationRequest>(value).is_err());
    }

    #[test]
    fn evidence_provider_role_grant_matrix_is_closed() {
        let cases = [
            (
                WorkflowEvaluatorProvider::AuthorizedHuman,
                CallerRole::Human,
                EVIDENCE_HUMAN_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::IndependentReviewer,
                CallerRole::Worker,
                EVIDENCE_REVIEW_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::RepositoryInspector,
                CallerRole::Runtime,
                EVIDENCE_RUNTIME_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::DeterministicTool,
                CallerRole::Runtime,
                EVIDENCE_RUNTIME_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::RepresentativeRuntime,
                CallerRole::Runtime,
                EVIDENCE_RUNTIME_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::ExternalAuthority,
                CallerRole::Worker,
                EVIDENCE_EXTERNAL_GRANT,
            ),
            (
                WorkflowEvaluatorProvider::ResearchSource,
                CallerRole::Runtime,
                EVIDENCE_EXTERNAL_GRANT,
            ),
        ];

        for (index, (provider, role, grant)) in cases.into_iter().enumerate() {
            let key = SigningKey::from_bytes(&[u8::try_from(31 + index).expect("fixture key"); 32]);
            let registry = registry(&key, role, &[grant], PrincipalCredentialStatus::Active);
            let request = evidence_request(provider);
            let attestation = sign(&key, EVIDENCE_ACTION, &request, metadata());
            let authorization = registry
                .authorize_workflow_evidence_at(
                    &verifier(),
                    request.clone(),
                    &attestation,
                    NOW,
                    300,
                    30,
                )
                .expect("matrix-authorized evidence");
            assert_eq!(authorization.request(), &request);
            assert_eq!(authorization.principal().role(), role);
            let audit = serde_json::to_string(&authorization.audit()).expect("audit");
            assert!(!audit.contains(&attestation.signature));
        }
    }

    #[test]
    fn evidence_rejects_generic_grant_wrong_role_and_wrong_specific_grant() {
        let key = SigningKey::from_bytes(&[41; 32]);
        let request = evidence_request(WorkflowEvaluatorProvider::IndependentReviewer);
        let attestation = sign(&key, EVIDENCE_ACTION, &request, metadata());

        for (role, grants) in [
            (CallerRole::Human, vec![EVIDENCE_REVIEW_GRANT]),
            (CallerRole::Worker, vec!["workflow.evidence.authorize"]),
            (CallerRole::Worker, vec![EVIDENCE_RUNTIME_GRANT]),
        ] {
            let result = registry(&key, role, &grants, PrincipalCredentialStatus::Active)
                .authorize_workflow_evidence_at(
                    &verifier(),
                    request.clone(),
                    &attestation,
                    NOW,
                    300,
                    30,
                );
            assert!(matches!(
                result,
                Err(WorkflowAuthorityError::RoleRequired { .. }
                    | WorkflowAuthorityError::MissingAuthorityGrant { .. })
            ));
        }
    }

    #[test]
    fn evidence_pairing_tampering_and_cross_action_replay_fail_closed() {
        let key = SigningKey::from_bytes(&[42; 32]);
        let registry = registry(
            &key,
            CallerRole::Runtime,
            &[EVIDENCE_RUNTIME_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let request = evidence_request(WorkflowEvaluatorProvider::DeterministicTool);
        let attestation = sign(&key, EVIDENCE_ACTION, &request, metadata());

        let mut wrong_strength = request.clone();
        wrong_strength.strength = WorkflowEvidenceStrength::AuthoritativeAcceptance;
        assert!(matches!(
            registry.authorize_workflow_evidence_at(
                &verifier(),
                wrong_strength,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::EvidenceClassificationMismatch { .. })
        ));

        let mut tampered = request.clone();
        tampered.subject_digest = format!("sha256:{}", "f".repeat(64));
        assert!(matches!(
            registry.authorize_workflow_evidence_at(
                &verifier(),
                tampered,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));

        let cross_action = sign(&key, CAPABILITY_ACTION, &request, metadata());
        assert!(matches!(
            registry.authorize_workflow_evidence_at(
                &verifier(),
                request,
                &cross_action,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));
    }

    #[test]
    fn applicability_and_capability_are_snapshot_bound_opaque_authorizations() {
        let human_key = SigningKey::from_bytes(&[43; 32]);
        let human_registry = registry(
            &human_key,
            CallerRole::Human,
            &[APPLICABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let applicability = applicability_request();
        let applicability_attestation =
            sign(&human_key, APPLICABILITY_ACTION, &applicability, metadata());
        let authorized = human_registry
            .authorize_workflow_applicability_at(
                &verifier(),
                applicability.clone(),
                &applicability_attestation,
                NOW,
            )
            .expect("applicability authorization");
        assert_eq!(authorized.request(), &applicability);
        assert!(authorized.audit().intent_digest.starts_with("sha256:"));

        let runtime_key = SigningKey::from_bytes(&[44; 32]);
        let runtime_registry = registry(
            &runtime_key,
            CallerRole::Runtime,
            &[CAPABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let capability = capability_request();
        let capability_attestation = sign(&runtime_key, CAPABILITY_ACTION, &capability, metadata());
        let authorized = runtime_registry
            .authorize_workflow_capability_at(
                &verifier(),
                capability.clone(),
                &capability_attestation,
                NOW,
            )
            .expect("capability authorization");
        assert_eq!(authorized.request(), &capability);
        assert_eq!(authorized.principal().role(), CallerRole::Runtime);
    }

    #[test]
    fn applicability_and_capability_reject_replay_role_grant_and_timestamp_attacks() {
        let human_key = SigningKey::from_bytes(&[45; 32]);
        let request = applicability_request();
        let attestation = sign(&human_key, APPLICABILITY_ACTION, &request, metadata());

        let mut advanced_head = request.clone();
        advanced_head.ledger_head_digest = format!("sha256:{}", "9".repeat(64));
        let applicability_registry = registry(
            &human_key,
            CallerRole::Human,
            &[APPLICABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            applicability_registry.authorize_workflow_applicability_at(
                &verifier(),
                advanced_head,
                &attestation,
                NOW
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));

        let wrong_role = registry(
            &human_key,
            CallerRole::Runtime,
            &[APPLICABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            wrong_role.authorize_workflow_applicability_at(
                &verifier(),
                request.clone(),
                &attestation,
                NOW
            ),
            Err(WorkflowAuthorityError::RoleRequired { .. })
        ));

        let runtime_key = SigningKey::from_bytes(&[46; 32]);
        let mut capability = capability_request();
        capability.expires_at_unix = Some(u64::try_from(NOW).expect("fixture time"));
        let capability_attestation = sign(&runtime_key, CAPABILITY_ACTION, &capability, metadata());
        let missing_grant = registry(
            &runtime_key,
            CallerRole::Runtime,
            &["workflow.capability.read"],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            missing_grant.authorize_workflow_capability_at(
                &verifier(),
                capability,
                &capability_attestation,
                NOW
            ),
            Err(WorkflowAuthorityError::AuthorizationTimestampOutOfBounds { .. })
        ));

        let valid_capability = capability_request();
        let valid_attestation = sign(
            &runtime_key,
            CAPABILITY_ACTION,
            &valid_capability,
            metadata(),
        );
        assert!(matches!(
            missing_grant.authorize_workflow_capability_at(
                &verifier(),
                valid_capability.clone(),
                &valid_attestation,
                NOW
            ),
            Err(WorkflowAuthorityError::MissingAuthorityGrant {
                grant: CAPABILITY_GRANT,
                ..
            })
        ));

        let cross_action = sign(
            &runtime_key,
            APPLICABILITY_ACTION,
            &valid_capability,
            metadata(),
        );
        let authorized_runtime = registry(
            &runtime_key,
            CallerRole::Runtime,
            &[CAPABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            authorized_runtime.authorize_workflow_capability_at(
                &verifier(),
                valid_capability,
                &cross_action,
                NOW
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));
    }

    #[test]
    fn decision_waiver_and_evidence_reject_snapshot_or_head_drift() {
        let key = SigningKey::from_bytes(&[48; 32]);
        let registry = registry(
            &key,
            CallerRole::Human,
            &[DECISION_GRANT, WAIVER_GRANT, EVIDENCE_HUMAN_GRANT],
            PrincipalCredentialStatus::Active,
        );

        let decision = decision_request();
        let attestation = sign(&key, DECISION_ACTION, &decision, metadata());
        let mut advanced = decision;
        advanced.ledger_head_digest = format!("sha256:{}", "1".repeat(64));
        assert!(matches!(
            registry.authorize_workflow_decision_at(
                &verifier(),
                advanced,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));

        let waiver = waiver_request();
        let attestation = sign(&key, WAIVER_ACTION, &waiver, metadata());
        let mut drifted = waiver;
        drifted.snapshot_digest = format!("sha256:{}", "2".repeat(64));
        assert!(matches!(
            registry.authorize_workflow_waiver_at(&verifier(), drifted, &attestation, NOW, 300, 30),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));

        let evidence = evidence_request(WorkflowEvaluatorProvider::AuthorizedHuman);
        let attestation = sign(&key, EVIDENCE_ACTION, &evidence, metadata());
        let mut advanced = evidence;
        advanced.ledger_head_digest = format!("sha256:{}", "3".repeat(64));
        assert!(matches!(
            registry.authorize_workflow_evidence_at(
                &verifier(),
                advanced,
                &attestation,
                NOW,
                300,
                30
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));
    }

    #[test]
    fn signal_authorization_is_closed_role_granted_and_head_bound() {
        let key = SigningKey::from_bytes(&[49; 32]);
        let request = signal_request();
        let attestation = sign(&key, SIGNAL_ACTION, &request, metadata());
        let authorized_registry = registry(
            &key,
            CallerRole::Runtime,
            &[SIGNAL_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let authorized = authorized_registry
            .authorize_workflow_signal_at(&verifier(), request.clone(), &attestation, NOW)
            .expect("signal authorization");
        assert_eq!(authorized.request(), &request);

        let mut advanced = request.clone();
        advanced.ledger_head_digest = format!("sha256:{}", "7".repeat(64));
        assert!(matches!(
            authorized_registry.authorize_workflow_signal_at(
                &verifier(),
                advanced,
                &attestation,
                NOW
            ),
            Err(WorkflowAuthorityError::Principal(
                PrincipalAuthorizationError::Attestation(AttestationError::Invalid)
            ))
        ));

        let wrong_role = registry(
            &key,
            CallerRole::Human,
            &[SIGNAL_GRANT],
            PrincipalCredentialStatus::Active,
        );
        assert!(matches!(
            wrong_role.authorize_workflow_signal_at(&verifier(), request, &attestation, NOW),
            Err(WorkflowAuthorityError::RoleRequired { .. })
        ));
    }

    #[test]
    fn new_requests_are_closed_and_require_digest_shaped_snapshot_bindings() {
        let mut applicability =
            serde_json::to_value(applicability_request()).expect("applicability JSON");
        applicability["caller_intent"] = serde_json::json!("forged");
        assert!(
            serde_json::from_value::<WorkflowApplicabilityAuthorizationRequest>(applicability)
                .is_err()
        );

        let key = SigningKey::from_bytes(&[47; 32]);
        let registry = registry(
            &key,
            CallerRole::Runtime,
            &[CAPABILITY_GRANT],
            PrincipalCredentialStatus::Active,
        );
        let mut request = capability_request();
        request.snapshot_digest = "not-a-digest".to_owned();
        let attestation = sign(&key, CAPABILITY_ACTION, &request, metadata());
        assert!(matches!(
            registry.authorize_workflow_capability_at(&verifier(), request, &attestation, NOW),
            Err(WorkflowAuthorityError::InvalidRequest {
                field: "snapshot_digest",
                ..
            })
        ));

        let mut applicability = applicability_request();
        applicability.evaluator_ref = StableId("evaluator.caller-selected".to_owned());
        assert!(matches!(
            validate_applicability_request(&applicability, NOW, DEFAULT_MAX_FUTURE_SKEW_SECONDS,),
            Err(WorkflowAuthorityError::InvalidRequest {
                field: "evaluator_ref",
                ..
            })
        ));

        let mut applicability = applicability_request();
        applicability.authority_scope = StableId("workflow.caller-selected".to_owned());
        assert!(matches!(
            validate_applicability_request(&applicability, NOW, DEFAULT_MAX_FUTURE_SKEW_SECONDS,),
            Err(WorkflowAuthorityError::InvalidRequest {
                field: "authority_scope",
                ..
            })
        ));

        let mut capability = capability_request();
        capability.authority_scope = StableId("workflow.caller-selected".to_owned());
        assert!(matches!(
            validate_capability_request(&capability, NOW, DEFAULT_MAX_FUTURE_SKEW_SECONDS),
            Err(WorkflowAuthorityError::InvalidRequest {
                field: "authority_scope",
                ..
            })
        ));
    }
}
