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
