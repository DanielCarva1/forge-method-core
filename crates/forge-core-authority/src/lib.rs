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
pub mod domain_pack_promotion;
pub mod domain_pack_supply_chain;
pub mod execution_handoff;
pub mod principal_registry;
pub mod workflow_authority;
pub mod workflow_release_review;
pub mod workflow_release_review_v2;
pub mod workflow_retirement;

pub use attestation::{
    AttestationError, AttestationGateOutcome, AttestationInput, AttestationPolicy,
    AttestationVerifier, CanonicalIntent,
};
pub use domain_pack_promotion::{
    domain_pack_independent_review_digest, domain_pack_promotion_decision_digest,
    domain_pack_promotion_dossier_digest, domain_pack_promotion_payload_digest,
    domain_pack_promotion_reviewer_key_fingerprint, domain_pack_promotion_signing_bytes,
    domain_pack_reviewed_registry_digest, domain_pack_reviewed_registry_entry_digest,
    domain_pack_reviewed_registry_proposal_digest, domain_pack_reviewed_registry_signing_bytes,
    domain_pack_reviewer_registry_digest, domain_pack_reviewer_registry_rotation_signing_bytes,
    verify_domain_pack_promotion_authorization, AnchoredReviewedDomainPackRegistrySnapshot,
    DomainPackPromotionAuditAuthority, DomainPackPromotionAuthorityError,
    DomainPackPromotionExpectedContext, DomainPackReviewerRegistryAdvanceAudit,
    DomainPackReviewerRegistryAnchor, DomainPackReviewerRegistryAnchorVersion,
    ReviewedDomainPackRegistryAnchor, ReviewedDomainPackRegistryAnchorVersion,
    VerifiedDomainPackPromotionAuthorization, VerifiedDomainPackPromotionAuthorizationAudit,
    VerifiedDomainPackPromotionReviewerAudit, DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN,
    DOMAIN_PACK_PROMOTION_SIGNATURE_DOMAIN, DOMAIN_PACK_REVIEWED_REGISTRY_SIGNATURE_DOMAIN,
    DOMAIN_PACK_REVIEWER_REGISTRY_ROTATION_SIGNATURE_DOMAIN,
};
pub use domain_pack_supply_chain::{
    domain_pack_package_record_digest, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
    verify_domain_pack_supply_chain_snapshot, AnchoredDomainPackSupplyChainSnapshot,
    DomainPackRegistryAnchor, DomainPackRegistryAnchorAdvance, DomainPackRegistryAnchorReplayAudit,
    DomainPackRegistryAnchorVersion, DomainPackSupplyChainAuditAuthority,
    DomainPackSupplyChainError, VerifiedDomainPackRegistrySignerAudit,
    VerifiedDomainPackSupplyChainEntry, VerifiedDomainPackSupplyChainEntryAudit,
    VerifiedDomainPackSupplyChainSnapshot, VerifiedDomainPackSupplyChainSnapshotAudit,
    DOMAIN_PACK_PUBLISHER_SIGNATURE_DOMAIN, DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN,
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
pub use workflow_retirement::{
    verify_workflow_retirement_authorization_v2, workflow_retirement_payload_digest_v2,
    workflow_retirement_reviewer_key_fingerprint_v2, workflow_retirement_signing_bytes_v2,
    VerifiedWorkflowRetirementAuthorizationAuditV2, VerifiedWorkflowRetirementAuthorizationV2,
    VerifiedWorkflowRetirementReviewerAuditV2, WorkflowRetirementAuditAuthorityV2,
    WorkflowRetirementAuthorityErrorV2, WorkflowRetirementExpectedContextV2,
    WORKFLOW_RETIREMENT_AGGREGATE_SIZE, WORKFLOW_RETIREMENT_PAYLOAD_DOMAIN_V2,
    WORKFLOW_RETIREMENT_SIGNATURE_DOMAIN_V2,
};
