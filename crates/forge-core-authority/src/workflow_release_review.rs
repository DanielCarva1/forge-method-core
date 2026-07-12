//! Independent cryptographic authorization for reviewed workflow releases.
//!
//! The wire documents in `forge-core-contracts` are deliberately inert. This
//! module is the only promotion boundary that resolves reviewer keys from the
//! repository-owned registry, verifies both required signatures, proves role
//! separation, and returns an opaque capability.

use std::collections::BTreeSet;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::{
    WorkflowGovernanceReleaseIdentity, WorkflowReleaseAdmissionAuthorizationDocument,
    WorkflowReleaseAdmissionAuthorizationPayload, WorkflowReleaseAdmissionSignature,
    WorkflowReleaseAdmissionSignatureAlgorithm, WorkflowReleaseReviewerCredential,
    WorkflowReleaseReviewerCredentialStatus, WorkflowReleaseReviewerRegistryDocument,
    WorkflowReleaseReviewerRole, WorkflowRuntimeBundleIdentity,
    WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Domain separator prepended to every P5d.4 admission signature.
pub const WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:workflow-release-admission:v1\0";

/// Result of an admission authorization verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowReleaseAdmissionAuthorityError {
    InvalidContract {
        document: &'static str,
        issue: String,
    },
    WrongAudience {
        expected: String,
        found: String,
    },
    WrongDomain {
        found: String,
    },
    BlockingReviewDecision,
    RegistryBindingMismatch {
        field: &'static str,
    },
    PayloadDigestMismatch {
        credential_id: String,
    },
    CredentialNotFound {
        credential_id: String,
    },
    CredentialNotActive {
        credential_id: String,
    },
    CredentialRoleMismatch {
        credential_id: String,
    },
    CredentialPrincipalMismatch {
        credential_id: String,
    },
    CredentialOutsideValidity {
        credential_id: String,
        signed_at_unix: u64,
    },
    AuthorizationOutsideValidity {
        credential_id: String,
        signed_at_unix: u64,
    },
    PublicKeyDecode {
        credential_id: String,
    },
    PublicKeyFingerprintMismatch {
        credential_id: String,
    },
    SignatureDecode {
        credential_id: String,
    },
    SignatureInvalid {
        credential_id: String,
    },
    ReviewerSeparationViolation {
        dimension: &'static str,
    },
    DuplicateSignature,
    MissingRequiredRole {
        role: WorkflowReleaseReviewerRole,
    },
    Canonicalization(String),
}

impl fmt::Display for WorkflowReleaseAdmissionAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { document, issue } => {
                write!(formatter, "invalid {document}: {issue}")
            }
            Self::WrongAudience { expected, found } => {
                write!(formatter, "wrong admission audience '{found}', expected '{expected}'")
            }
            Self::WrongDomain { found } => write!(formatter, "wrong admission domain '{found}'"),
            Self::BlockingReviewDecision => {
                formatter.write_str("admission contains a rejected or changes-required review")
            }
            Self::RegistryBindingMismatch { field } => {
                write!(formatter, "reviewer registry binding mismatch at '{field}'")
            }
            Self::PayloadDigestMismatch { credential_id } => {
                write!(formatter, "payload digest mismatch for credential '{credential_id}'")
            }
            Self::CredentialNotFound { credential_id } => {
                write!(formatter, "reviewer credential '{credential_id}' not found")
            }
            Self::CredentialNotActive { credential_id } => {
                write!(formatter, "reviewer credential '{credential_id}' is not active")
            }
            Self::CredentialRoleMismatch { credential_id } => {
                write!(formatter, "reviewer credential '{credential_id}' lacks the signed role")
            }
            Self::CredentialPrincipalMismatch { credential_id } => {
                write!(formatter, "reviewer credential '{credential_id}' has another principal")
            }
            Self::CredentialOutsideValidity {
                credential_id,
                signed_at_unix,
            } => write!(
                formatter,
                "credential '{credential_id}' is not valid at signed_at {signed_at_unix}"
            ),
            Self::AuthorizationOutsideValidity {
                credential_id,
                signed_at_unix,
            } => write!(
                formatter,
                "authorization for credential '{credential_id}' is not valid at signed_at {signed_at_unix}"
            ),
            Self::PublicKeyDecode { credential_id } => {
                write!(formatter, "invalid public key for credential '{credential_id}'")
            }
            Self::PublicKeyFingerprintMismatch { credential_id } => write!(
                formatter,
                "public key fingerprint mismatch for credential '{credential_id}'"
            ),
            Self::SignatureDecode { credential_id } => {
                write!(formatter, "invalid signature encoding for credential '{credential_id}'")
            }
            Self::SignatureInvalid { credential_id } => {
                write!(formatter, "invalid signature for credential '{credential_id}'")
            }
            Self::ReviewerSeparationViolation { dimension } => {
                write!(formatter, "reviewer separation violated for {dimension}")
            }
            Self::DuplicateSignature => formatter.write_str("duplicate reviewer signature"),
            Self::MissingRequiredRole { role } => {
                write!(formatter, "missing required reviewer role {role:?}")
            }
            Self::Canonicalization(message) => write!(formatter, "canonicalization failed: {message}"),
        }
    }
}

impl std::error::Error for WorkflowReleaseAdmissionAuthorityError {}

/// Opaque admission capability. It intentionally implements neither `Clone`
/// nor serde traits and cannot be constructed outside this module.
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowReleaseAdmissionAuthorization;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedWorkflowReleaseAdmissionAuthorization>();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedWorkflowReleaseAdmissionAuthorization;
/// let _: VerifiedWorkflowReleaseAdmissionAuthorization = serde_json::from_str("{}").unwrap();
/// ```
pub struct VerifiedWorkflowReleaseAdmissionAuthorization {
    authorization_id: String,
    payload_digest: String,
    reviewer_registry_digest: String,
    candidate_release: WorkflowGovernanceReleaseIdentity,
    candidate_runtime_bundle: WorkflowRuntimeBundleIdentity,
    promoted_runtime_bundle: WorkflowRuntimeBundleIdentity,
    semantic_reviewer: VerifiedReviewer,
    release_authorizer: VerifiedReviewer,
}

impl fmt::Debug for VerifiedWorkflowReleaseAdmissionAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowReleaseAdmissionAuthorization")
            .field("authorization_id", &self.authorization_id)
            .field("payload_digest", &self.payload_digest)
            .field("reviewer_registry_digest", &self.reviewer_registry_digest)
            .field("candidate_release", &self.candidate_release)
            .field("candidate_runtime_bundle", &self.candidate_runtime_bundle)
            .field("promoted_runtime_bundle", &self.promoted_runtime_bundle)
            .finish_non_exhaustive()
    }
}

impl VerifiedWorkflowReleaseAdmissionAuthorization {
    #[must_use]
    pub fn authorization_id(&self) -> &str {
        &self.authorization_id
    }

    #[must_use]
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    #[must_use]
    pub fn candidate_release(&self) -> &WorkflowGovernanceReleaseIdentity {
        &self.candidate_release
    }

    #[must_use]
    pub fn candidate_runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.candidate_runtime_bundle
    }

    /// Exact final bundle identity admitted by the kernel, distinct from the
    /// P5d.3 candidate/shadow envelope.
    #[must_use]
    pub fn promoted_runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.promoted_runtime_bundle
    }

    /// Return a reporting-only projection. This value cannot recreate the
    /// opaque capability and therefore confers no admission authority.
    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowReleaseAdmissionAuthorizationAudit {
        VerifiedWorkflowReleaseAdmissionAuthorizationAudit {
            authority: WorkflowReleaseAdmissionAuditAuthority::NonAuthoritative,
            authorization_id: self.authorization_id.clone(),
            payload_digest: self.payload_digest.clone(),
            reviewer_registry_digest: self.reviewer_registry_digest.clone(),
            candidate_release_id: self.candidate_release.release_id.0.clone(),
            candidate_release_digest: self.candidate_release.release_digest.clone(),
            candidate_runtime_bundle_id: self.candidate_runtime_bundle.bundle_id.0.clone(),
            candidate_runtime_bundle_digest: self.candidate_runtime_bundle.bundle_digest.clone(),
            promoted_runtime_bundle_id: self.promoted_runtime_bundle.bundle_id.0.clone(),
            promoted_runtime_bundle_digest: self.promoted_runtime_bundle.bundle_digest.clone(),
            semantic_reviewer: self.semantic_reviewer.audit(),
            release_authorizer: self.release_authorizer.audit(),
        }
    }
}

#[derive(Debug)]
struct VerifiedReviewer {
    principal_id: String,
    credential_id: String,
    public_key_fingerprint: String,
    signature_fingerprint: String,
    signed_at_unix: u64,
}

impl VerifiedReviewer {
    fn audit(&self) -> VerifiedWorkflowReleaseReviewerAudit {
        VerifiedWorkflowReleaseReviewerAudit {
            principal_id: self.principal_id.clone(),
            credential_id: self.credential_id.clone(),
            public_key_fingerprint: self.public_key_fingerprint.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
            signed_at_unix: self.signed_at_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionAuditAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowReleaseReviewerAudit {
    pub principal_id: String,
    pub credential_id: String,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowReleaseAdmissionAuthorizationAudit {
    pub authority: WorkflowReleaseAdmissionAuditAuthority,
    pub authorization_id: String,
    pub payload_digest: String,
    pub reviewer_registry_digest: String,
    pub candidate_release_id: String,
    pub candidate_release_digest: String,
    pub candidate_runtime_bundle_id: String,
    pub candidate_runtime_bundle_digest: String,
    pub promoted_runtime_bundle_id: String,
    pub promoted_runtime_bundle_digest: String,
    pub semantic_reviewer: VerifiedWorkflowReleaseReviewerAudit,
    pub release_authorizer: VerifiedWorkflowReleaseReviewerAudit,
}

/// Build the exact domain-separated bytes signed by one reviewer.
///
/// Exposing these deterministic bytes enables offline signing; it does not
/// resolve or authorize a key. All verification still uses the fixed registry.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityError::Canonicalization`] if the
/// closed payload cannot be represented as canonical JSON.
pub fn workflow_release_admission_signing_bytes(
    payload: &WorkflowReleaseAdmissionAuthorizationPayload,
    signature: &WorkflowReleaseAdmissionSignature,
) -> Result<Vec<u8>, WorkflowReleaseAdmissionAuthorityError> {
    #[derive(Serialize)]
    struct SignedEnvelope<'a> {
        authorization_id: &'a str,
        payload: &'a WorkflowReleaseAdmissionAuthorizationPayload,
        credential_id: &'a str,
        role: WorkflowReleaseReviewerRole,
        signed_at_unix: u64,
    }

    let envelope = SignedEnvelope {
        authorization_id: &payload.authorization_id.0,
        payload,
        credential_id: &signature.credential_id.0,
        role: signature.role,
        signed_at_unix: signature.signed_at_unix,
    };
    let value = serde_json::to_value(envelope).map_err(|error| {
        WorkflowReleaseAdmissionAuthorityError::Canonicalization(error.to_string())
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&value).map_err(|error| {
        WorkflowReleaseAdmissionAuthorityError::Canonicalization(error.to_string())
    })?;
    let mut bytes =
        Vec::with_capacity(WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN.len() + canonical.len());
    bytes.extend_from_slice(WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

/// Compute the canonical JCS identity of a signed admission payload.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityError::Canonicalization`] if the
/// closed payload cannot be represented as canonical JSON.
pub fn workflow_release_admission_payload_digest(
    payload: &WorkflowReleaseAdmissionAuthorizationPayload,
) -> Result<String, WorkflowReleaseAdmissionAuthorityError> {
    canonical_digest(payload)
}

/// Compute the registry key fingerprint used by the fixed credential registry.
#[must_use]
pub fn workflow_release_reviewer_key_fingerprint(public_key: &[u8; 32]) -> String {
    format!("sha256:{:x}", Sha256::digest(public_key))
}

/// Verify both independent signatures and return the opaque admission token.
///
/// `reviewer_registry_raw_bytes` must be the exact embedded registry bytes; its
/// raw digest and the typed registry JCS digest are both bound by the payload.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityError`] for any structural,
/// binding, credential, separation, time-window, decoding, or signature
/// failure. No partially verified capability is returned.
pub fn verify_workflow_release_admission_authorization(
    reviewer_registry: &WorkflowReleaseReviewerRegistryDocument,
    reviewer_registry_raw_bytes: &[u8],
    authorization: &WorkflowReleaseAdmissionAuthorizationDocument,
    expected_audience: &str,
) -> Result<VerifiedWorkflowReleaseAdmissionAuthorization, WorkflowReleaseAdmissionAuthorityError> {
    validate_contracts(reviewer_registry, authorization)?;
    let payload = &authorization
        .workflow_release_admission_authorization
        .payload;
    if payload.audience != expected_audience {
        return Err(WorkflowReleaseAdmissionAuthorityError::WrongAudience {
            expected: expected_audience.to_owned(),
            found: payload.audience.clone(),
        });
    }
    let expected_domain = "forge-method:workflow-release-admission:v1";
    if payload.domain != expected_domain {
        return Err(WorkflowReleaseAdmissionAuthorityError::WrongDomain {
            found: payload.domain.clone(),
        });
    }
    if payload.workflow_decisions.iter().any(|decision| {
        decision.decision != forge_core_contracts::WorkflowReleaseReviewDecision::Approved
    }) || payload.quarantine_decisions.iter().any(|decision| {
        decision.decision != forge_core_contracts::WorkflowReleaseReviewDecision::Approved
    }) || payload.dimension_decisions.iter().any(|decision| {
        decision.decision != forge_core_contracts::WorkflowReleaseReviewDecision::Approved
    }) {
        return Err(WorkflowReleaseAdmissionAuthorityError::BlockingReviewDecision);
    }

    let registry = &reviewer_registry.workflow_release_reviewer_registry;
    require_binding(
        payload.reviewer_registry_id == registry.registry_id,
        "reviewer_registry_id",
    )?;
    require_binding(
        payload.reviewer_registry_version == registry.registry_version,
        "reviewer_registry_version",
    )?;
    require_binding(
        payload.reviewer_registry_raw_digest == raw_digest(reviewer_registry_raw_bytes),
        "reviewer_registry_raw_digest",
    )?;
    let registry_digest = canonical_digest(reviewer_registry)?;
    require_binding(
        payload.reviewer_registry_canonical_digest == registry_digest,
        "reviewer_registry_canonical_digest",
    )?;

    let payload_digest = workflow_release_admission_payload_digest(payload)?;
    let signatures = &authorization
        .workflow_release_admission_authorization
        .signatures;
    let mut verified = Vec::with_capacity(signatures.len());
    let mut signature_values = BTreeSet::new();
    for signature in signatures {
        if !signature_values.insert(&signature.signature) {
            return Err(WorkflowReleaseAdmissionAuthorityError::DuplicateSignature);
        }
        if signature.payload_digest != payload_digest {
            return Err(
                WorkflowReleaseAdmissionAuthorityError::PayloadDigestMismatch {
                    credential_id: signature.credential_id.0.clone(),
                },
            );
        }
        verified.push(verify_signature(registry, payload, signature)?);
    }

    let semantic = verified
        .iter()
        .find(|(role, _)| *role == WorkflowReleaseReviewerRole::SemanticReviewer)
        .ok_or(
            WorkflowReleaseAdmissionAuthorityError::MissingRequiredRole {
                role: WorkflowReleaseReviewerRole::SemanticReviewer,
            },
        )?;
    let authorizer = verified
        .iter()
        .find(|(role, _)| *role == WorkflowReleaseReviewerRole::ReleaseAuthorizer)
        .ok_or(
            WorkflowReleaseAdmissionAuthorityError::MissingRequiredRole {
                role: WorkflowReleaseReviewerRole::ReleaseAuthorizer,
            },
        )?;
    require_separation(&semantic.1, &authorizer.1)?;

    Ok(VerifiedWorkflowReleaseAdmissionAuthorization {
        authorization_id: payload.authorization_id.0.clone(),
        payload_digest,
        reviewer_registry_digest: registry_digest,
        candidate_release: payload.promotion.candidate_release.clone(),
        candidate_runtime_bundle: payload.promotion.candidate_runtime_bundle.clone(),
        promoted_runtime_bundle: payload.promotion.promoted_runtime_bundle.clone(),
        semantic_reviewer: verified_reviewer(semantic),
        release_authorizer: verified_reviewer(authorizer),
    })
}

fn verified_reviewer(pair: &(WorkflowReleaseReviewerRole, VerifiedReviewer)) -> VerifiedReviewer {
    VerifiedReviewer {
        principal_id: pair.1.principal_id.clone(),
        credential_id: pair.1.credential_id.clone(),
        public_key_fingerprint: pair.1.public_key_fingerprint.clone(),
        signature_fingerprint: pair.1.signature_fingerprint.clone(),
        signed_at_unix: pair.1.signed_at_unix,
    }
}

fn validate_contracts(
    registry: &WorkflowReleaseReviewerRegistryDocument,
    authorization: &WorkflowReleaseAdmissionAuthorizationDocument,
) -> Result<(), WorkflowReleaseAdmissionAuthorityError> {
    if registry.schema_version != WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION {
        return Err(WorkflowReleaseAdmissionAuthorityError::InvalidContract {
            document: "reviewer registry",
            issue: "unsupported schema version".to_owned(),
        });
    }
    if authorization.schema_version != WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION {
        return Err(WorkflowReleaseAdmissionAuthorityError::InvalidContract {
            document: "admission authorization",
            issue: "unsupported schema version".to_owned(),
        });
    }
    if let Some(issue) = registry.validate().first() {
        return Err(WorkflowReleaseAdmissionAuthorityError::InvalidContract {
            document: "reviewer registry",
            issue: format!("{}: {}", issue.path, issue.message),
        });
    }
    if let Some(issue) = authorization.validate().first() {
        return Err(WorkflowReleaseAdmissionAuthorityError::InvalidContract {
            document: "admission authorization",
            issue: format!("{}: {}", issue.path, issue.message),
        });
    }
    Ok(())
}

fn verify_signature(
    registry: &forge_core_contracts::WorkflowReleaseReviewerRegistry,
    payload: &WorkflowReleaseAdmissionAuthorizationPayload,
    signature: &WorkflowReleaseAdmissionSignature,
) -> Result<(WorkflowReleaseReviewerRole, VerifiedReviewer), WorkflowReleaseAdmissionAuthorityError>
{
    let credential_id = signature.credential_id.0.clone();
    let credential = registry
        .credentials
        .iter()
        .find(|candidate| candidate.credential_id == signature.credential_id)
        .ok_or_else(
            || WorkflowReleaseAdmissionAuthorityError::CredentialNotFound {
                credential_id: credential_id.clone(),
            },
        )?;
    validate_credential(credential, payload, signature)?;
    let public_key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
        WorkflowReleaseAdmissionAuthorityError::PublicKeyDecode {
            credential_id: credential_id.clone(),
        }
    })?;
    if workflow_release_reviewer_key_fingerprint(&public_key_bytes)
        != credential.public_key_fingerprint
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::PublicKeyFingerprintMismatch { credential_id },
        );
    }
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
        WorkflowReleaseAdmissionAuthorityError::PublicKeyDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let signature_bytes = decode_fixed::<64>(&signature.signature).ok_or_else(|| {
        WorkflowReleaseAdmissionAuthorityError::SignatureDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let detached = Signature::from_bytes(&signature_bytes);
    let signing_bytes = workflow_release_admission_signing_bytes(payload, signature)?;
    verifying_key
        .verify_strict(&signing_bytes, &detached)
        .map_err(
            |_| WorkflowReleaseAdmissionAuthorityError::SignatureInvalid {
                credential_id: signature.credential_id.0.clone(),
            },
        )?;
    Ok((
        signature.role,
        VerifiedReviewer {
            principal_id: signature.principal_id.0.clone(),
            credential_id: signature.credential_id.0.clone(),
            public_key_fingerprint: credential.public_key_fingerprint.clone(),
            signature_fingerprint: raw_digest(&signature_bytes),
            signed_at_unix: signature.signed_at_unix,
        },
    ))
}

fn validate_credential(
    credential: &WorkflowReleaseReviewerCredential,
    payload: &WorkflowReleaseAdmissionAuthorizationPayload,
    signature: &WorkflowReleaseAdmissionSignature,
) -> Result<(), WorkflowReleaseAdmissionAuthorityError> {
    let credential_id = signature.credential_id.0.clone();
    if credential.status != WorkflowReleaseReviewerCredentialStatus::Active {
        return Err(WorkflowReleaseAdmissionAuthorityError::CredentialNotActive { credential_id });
    }
    if credential.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
        || signature.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::CredentialRoleMismatch { credential_id },
        );
    }
    if credential.principal_id != signature.principal_id {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::CredentialPrincipalMismatch { credential_id },
        );
    }
    if !credential.roles.contains(&signature.role) {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::CredentialRoleMismatch { credential_id },
        );
    }
    if signature.signed_at_unix < credential.valid_from_unix
        || signature.signed_at_unix > credential.valid_until_unix
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::CredentialOutsideValidity {
                credential_id,
                signed_at_unix: signature.signed_at_unix,
            },
        );
    }
    if signature.signed_at_unix < payload.issued_at_unix
        || signature.signed_at_unix > payload.expires_at_unix
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityError::AuthorizationOutsideValidity {
                credential_id,
                signed_at_unix: signature.signed_at_unix,
            },
        );
    }
    Ok(())
}

fn require_separation(
    semantic: &VerifiedReviewer,
    authorizer: &VerifiedReviewer,
) -> Result<(), WorkflowReleaseAdmissionAuthorityError> {
    for (different, dimension) in [
        (
            semantic.principal_id != authorizer.principal_id,
            "principal",
        ),
        (
            semantic.credential_id != authorizer.credential_id,
            "credential",
        ),
        (
            semantic.public_key_fingerprint != authorizer.public_key_fingerprint,
            "public key fingerprint",
        ),
        (
            semantic.signature_fingerprint != authorizer.signature_fingerprint,
            "signature fingerprint",
        ),
    ] {
        if !different {
            return Err(
                WorkflowReleaseAdmissionAuthorityError::ReviewerSeparationViolation { dimension },
            );
        }
    }
    Ok(())
}

fn require_binding(
    matches: bool,
    field: &'static str,
) -> Result<(), WorkflowReleaseAdmissionAuthorityError> {
    if matches {
        Ok(())
    } else {
        Err(WorkflowReleaseAdmissionAuthorityError::RegistryBindingMismatch { field })
    }
}

fn canonical_digest<T: Serialize>(
    value: &T,
) -> Result<String, WorkflowReleaseAdmissionAuthorityError> {
    let json = serde_json::to_value(value).map_err(|error| {
        WorkflowReleaseAdmissionAuthorityError::Canonicalization(error.to_string())
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&json).map_err(|error| {
        WorkflowReleaseAdmissionAuthorityError::Canonicalization(error.to_string())
    })?;
    Ok(raw_digest(&canonical))
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}
