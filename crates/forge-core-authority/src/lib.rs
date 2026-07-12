//! Host-neutral verification and authority capabilities for governed execution.
//!
//! This crate owns operator-registry verification and the opaque
//! [`VerifiedExecutionAuthorization`] capability. Protocol adapters may produce
//! that capability after verifying untrusted wire input; the kernel may consume
//! it later without depending on MCP, CLI, serde input, or subprocess handoff.
//!
//! The crate deliberately contains no filesystem, transport, store, or mutation
//! code. Possessing a valid signature is not sufficient: only an
//! operator-configured [`AuthorizedPrincipalRegistry`] can construct execution
//! authorization.

pub mod attestation;
pub mod execution_handoff;
pub mod principal_registry;
pub mod workflow_authority;
pub mod workflow_release_review;
pub mod workflow_release_review_v2;

pub use attestation::{
    AttestationError, AttestationGateOutcome, AttestationInput, AttestationPolicy,
    AttestationVerifier, CanonicalIntent,
};
pub use execution_handoff::{
    ExecutionError, ExecutionExecutor, ExecutionPayloadBinding, ExecutionRequest, ExecutionResult,
    ExecutionStatus, VerifiedExecutionCall,
};
pub use principal_registry::{
    AuthorizedPrincipal, AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry,
    PrincipalAuthorizationError, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, PrincipalRegistryError,
    PrincipalRegistryIssue, PrincipalRegistryIssueCode, VerifiedExecutionAuthorization,
    VerifiedExecutionAuthorizationAudit, DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
    DEFAULT_MAX_FUTURE_SKEW_SECONDS, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
pub use workflow_authority::{
    VerifiedWorkflowApplicabilityAuthorization, VerifiedWorkflowApplicabilityAuthorizationAudit,
    VerifiedWorkflowCapabilityAuthorization, VerifiedWorkflowCapabilityAuthorizationAudit,
    VerifiedWorkflowDecisionAuthorization, VerifiedWorkflowDecisionAuthorizationAudit,
    VerifiedWorkflowEvidenceAuthorization, VerifiedWorkflowEvidenceAuthorizationAudit,
    VerifiedWorkflowSignalAuthorization, VerifiedWorkflowSignalAuthorizationAudit,
    VerifiedWorkflowWaiverAuthorization, VerifiedWorkflowWaiverAuthorizationAudit,
    WorkflowApplicabilityAuthorization, WorkflowApplicabilityAuthorizationRequest,
    WorkflowAuthorityError, WorkflowCapabilityAuthorization,
    WorkflowCapabilityAuthorizationRequest, WorkflowDecisionAuthorizationRequest,
    WorkflowEvidenceAuthorizationRequest, WorkflowSignalAuthorization,
    WorkflowSignalAuthorizationRequest, WorkflowWaiverAuthorizationRequest, WorkflowWaiverSubject,
};
pub use workflow_release_review::{
    verify_workflow_release_admission_authorization, workflow_release_admission_payload_digest,
    workflow_release_admission_signing_bytes, workflow_release_reviewer_key_fingerprint,
    VerifiedWorkflowReleaseAdmissionAuthorization,
    VerifiedWorkflowReleaseAdmissionAuthorizationAudit, VerifiedWorkflowReleaseReviewerAudit,
    WorkflowReleaseAdmissionAuditAuthority, WorkflowReleaseAdmissionAuthorityError,
    WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN,
};
pub use workflow_release_review_v2::{
    verify_workflow_release_admission_authorization_v2,
    workflow_release_admission_payload_digest_v2, workflow_release_admission_signing_bytes_v2,
    workflow_release_reviewer_key_fingerprint_v2,
    VerifiedWorkflowReleaseAdmissionAuthorizationAuditV2,
    VerifiedWorkflowReleaseAdmissionAuthorizationV2, VerifiedWorkflowReleaseReviewerAuditV2,
    WorkflowReleaseAdmissionAuditAuthorityV2, WorkflowReleaseAdmissionAuthorityErrorV2,
    WorkflowReleaseAdmissionExpectedContextV2, WORKFLOW_RELEASE_ADMISSION_PAYLOAD_DOMAIN_V2,
    WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN_V2,
};
