//! Independent cryptographic authorization for one generic V2 workflow release.
//!
//! V2 is intentionally separate from the frozen P5d.4a/V1 signature domain.
//! It verifies a release-specific payload against the exact review index and
//! evaluation selected by the kernel before returning a move-only capability.

use std::collections::BTreeSet;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::workflow_release_review_v2::{
    WorkflowReleaseAdmissionAuthorizationPayloadV2,
    WorkflowReleaseAdmissionAuthorizationV2Document, WorkflowReleaseAdmissionSignatureV2,
    WorkflowReleaseReviewIndexV2Document,
    WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION,
};
use forge_core_contracts::{
    WorkflowGovernanceReleaseIdentity, WorkflowReleaseAdmissionSignatureAlgorithm,
    WorkflowReleaseReviewDecision, WorkflowReleaseReviewerCredential,
    WorkflowReleaseReviewerCredentialStatus, WorkflowReleaseReviewerRegistry,
    WorkflowReleaseReviewerRegistryDocument, WorkflowReleaseReviewerRole,
    WorkflowRuntimeBundleIdentity, WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Domain separator prepended to every V2 release-admission signature.
pub const WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN_V2: &[u8] =
    b"forge-method:workflow-release-admission:v2\0";
/// Exact logical domain required inside every signed V2 payload.
pub const WORKFLOW_RELEASE_ADMISSION_PAYLOAD_DOMAIN_V2: &str =
    "forge-method:workflow-release-admission:v2";

/// Trusted, release-specific context selected by the kernel.
///
/// Passing the typed index and its exact bytes prevents a valid signature from
/// being replayed over a different predecessor, candidate, catalog, or review.
#[derive(Clone, Copy)]
pub struct WorkflowReleaseAdmissionExpectedContextV2<'a> {
    pub review_index: &'a WorkflowReleaseReviewIndexV2Document,
    pub review_index_raw_bytes: &'a [u8],
    pub evaluation_digest: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowReleaseAdmissionAuthorityErrorV2 {
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
    BindingMismatch {
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

impl fmt::Display for WorkflowReleaseAdmissionAuthorityErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { document, issue } => write!(f, "invalid {document}: {issue}"),
            Self::WrongAudience { expected, found } => write!(f, "wrong admission audience '{found}', expected '{expected}'"),
            Self::WrongDomain { found } => write!(f, "wrong V2 admission domain '{found}'"),
            Self::BlockingReviewDecision => f.write_str("admission contains a blocking review decision"),
            Self::BindingMismatch { field } => write!(f, "release-specific admission binding mismatch at '{field}'"),
            Self::PayloadDigestMismatch { credential_id } => write!(f, "payload digest mismatch for credential '{credential_id}'"),
            Self::CredentialNotFound { credential_id } => write!(f, "reviewer credential '{credential_id}' not found"),
            Self::CredentialNotActive { credential_id } => write!(f, "reviewer credential '{credential_id}' is not active"),
            Self::CredentialRoleMismatch { credential_id } => write!(f, "reviewer credential '{credential_id}' lacks the signed role or algorithm"),
            Self::CredentialPrincipalMismatch { credential_id } => write!(f, "reviewer credential '{credential_id}' has another principal"),
            Self::CredentialOutsideValidity { credential_id, signed_at_unix } => write!(f, "credential '{credential_id}' is not valid at signed_at {signed_at_unix}"),
            Self::AuthorizationOutsideValidity { credential_id, signed_at_unix } => write!(f, "authorization for credential '{credential_id}' is not valid at signed_at {signed_at_unix}"),
            Self::PublicKeyDecode { credential_id } => write!(f, "invalid public key for credential '{credential_id}'"),
            Self::PublicKeyFingerprintMismatch { credential_id } => write!(f, "public key fingerprint mismatch for credential '{credential_id}'"),
            Self::SignatureDecode { credential_id } => write!(f, "invalid signature encoding for credential '{credential_id}'"),
            Self::SignatureInvalid { credential_id } => write!(f, "invalid signature for credential '{credential_id}'"),
            Self::ReviewerSeparationViolation { dimension } => write!(f, "reviewer separation violated for {dimension}"),
            Self::DuplicateSignature => f.write_str("duplicate reviewer signature"),
            Self::MissingRequiredRole { role } => write!(f, "missing required reviewer role {role:?}"),
            Self::Canonicalization(message) => write!(f, "canonicalization failed: {message}"),
        }
    }
}
impl std::error::Error for WorkflowReleaseAdmissionAuthorityErrorV2 {}

/// Move-only, non-serializable authority for exactly one reviewed release.
pub struct VerifiedWorkflowReleaseAdmissionAuthorizationV2 {
    authorization_id: String,
    payload_digest: String,
    review_index_raw_digest: String,
    review_index_canonical_digest: String,
    evaluation_digest: String,
    reviewer_registry_digest: String,
    predecessor_registry_digest: String,
    proposed_registry_digest: String,
    predecessor_release_id: String,
    predecessor_release_digest: String,
    candidate_release: WorkflowGovernanceReleaseIdentity,
    candidate_runtime_bundle: WorkflowRuntimeBundleIdentity,
    promoted_runtime_bundle: WorkflowRuntimeBundleIdentity,
    semantic_reviewer: VerifiedReviewerV2,
    release_authorizer: VerifiedReviewerV2,
}

impl fmt::Debug for VerifiedWorkflowReleaseAdmissionAuthorizationV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedWorkflowReleaseAdmissionAuthorizationV2")
            .field("authorization_id", &self.authorization_id)
            .field("payload_digest", &self.payload_digest)
            .field("evaluation_digest", &self.evaluation_digest)
            .field("predecessor_release_id", &self.predecessor_release_id)
            .field("candidate_release", &self.candidate_release)
            .field("promoted_runtime_bundle", &self.promoted_runtime_bundle)
            .finish_non_exhaustive()
    }
}

impl VerifiedWorkflowReleaseAdmissionAuthorizationV2 {
    #[must_use]
    pub fn authorization_id(&self) -> &str {
        &self.authorization_id
    }
    #[must_use]
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
    #[must_use]
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }
    #[must_use]
    pub fn candidate_release(&self) -> &WorkflowGovernanceReleaseIdentity {
        &self.candidate_release
    }
    #[must_use]
    pub fn candidate_runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.candidate_runtime_bundle
    }
    #[must_use]
    pub fn promoted_runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.promoted_runtime_bundle
    }
    /// Canonical predecessor-registry digest verified as part of this opaque
    /// authorization. Unlike [`Self::audit`], this getter remains inside the
    /// trusted capability boundary and is suitable for the kernel append gate.
    #[must_use]
    pub fn predecessor_registry_digest(&self) -> &str {
        &self.predecessor_registry_digest
    }
    /// Canonical proposed-registry digest verified for this exact successor.
    #[must_use]
    pub fn proposed_registry_digest(&self) -> &str {
        &self.proposed_registry_digest
    }
    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowReleaseAdmissionAuthorizationAuditV2 {
        VerifiedWorkflowReleaseAdmissionAuthorizationAuditV2 {
            authority: WorkflowReleaseAdmissionAuditAuthorityV2::NonAuthoritative,
            authorization_id: self.authorization_id.clone(),
            payload_digest: self.payload_digest.clone(),
            review_index_raw_digest: self.review_index_raw_digest.clone(),
            review_index_canonical_digest: self.review_index_canonical_digest.clone(),
            evaluation_digest: self.evaluation_digest.clone(),
            reviewer_registry_digest: self.reviewer_registry_digest.clone(),
            predecessor_registry_digest: self.predecessor_registry_digest.clone(),
            proposed_registry_digest: self.proposed_registry_digest.clone(),
            predecessor_release_id: self.predecessor_release_id.clone(),
            predecessor_release_digest: self.predecessor_release_digest.clone(),
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
struct VerifiedReviewerV2 {
    principal_id: String,
    credential_id: String,
    independence_domain: String,
    public_key_fingerprint: String,
    signature_fingerprint: String,
    signed_at_unix: u64,
}
impl VerifiedReviewerV2 {
    fn audit(&self) -> VerifiedWorkflowReleaseReviewerAuditV2 {
        VerifiedWorkflowReleaseReviewerAuditV2 {
            principal_id: self.principal_id.clone(),
            credential_id: self.credential_id.clone(),
            independence_domain: self.independence_domain.clone(),
            public_key_fingerprint: self.public_key_fingerprint.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
            signed_at_unix: self.signed_at_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseAdmissionAuditAuthorityV2 {
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowReleaseReviewerAuditV2 {
    pub principal_id: String,
    pub credential_id: String,
    pub independence_domain: String,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowReleaseAdmissionAuthorizationAuditV2 {
    pub authority: WorkflowReleaseAdmissionAuditAuthorityV2,
    pub authorization_id: String,
    pub payload_digest: String,
    pub review_index_raw_digest: String,
    pub review_index_canonical_digest: String,
    pub evaluation_digest: String,
    pub reviewer_registry_digest: String,
    pub predecessor_registry_digest: String,
    pub proposed_registry_digest: String,
    pub predecessor_release_id: String,
    pub predecessor_release_digest: String,
    pub candidate_release_id: String,
    pub candidate_release_digest: String,
    pub candidate_runtime_bundle_id: String,
    pub candidate_runtime_bundle_digest: String,
    pub promoted_runtime_bundle_id: String,
    pub promoted_runtime_bundle_digest: String,
    pub semantic_reviewer: VerifiedWorkflowReleaseReviewerAuditV2,
    pub release_authorizer: VerifiedWorkflowReleaseReviewerAuditV2,
}

/// Produces domain-separated canonical bytes for one V2 signature envelope.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization`] when
/// the typed envelope cannot be converted to canonical JSON.
pub fn workflow_release_admission_signing_bytes_v2(
    payload: &WorkflowReleaseAdmissionAuthorizationPayloadV2,
    signature: &WorkflowReleaseAdmissionSignatureV2,
) -> Result<Vec<u8>, WorkflowReleaseAdmissionAuthorityErrorV2> {
    #[derive(Serialize)]
    struct SignedEnvelope<'a> {
        authorization_id: &'a str,
        payload: &'a WorkflowReleaseAdmissionAuthorizationPayloadV2,
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
    let value = serde_json::to_value(envelope)
        .map_err(|e| WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization(e.to_string()))?;
    let canonical = serde_json_canonicalizer::to_vec(&value)
        .map_err(|e| WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization(e.to_string()))?;
    let mut bytes =
        Vec::with_capacity(WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN_V2.len() + canonical.len());
    bytes.extend_from_slice(WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN_V2);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

/// Computes the canonical, domain-stable identity of a V2 admission payload.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization`] when
/// canonical JSON serialization fails.
pub fn workflow_release_admission_payload_digest_v2(
    payload: &WorkflowReleaseAdmissionAuthorizationPayloadV2,
) -> Result<String, WorkflowReleaseAdmissionAuthorityErrorV2> {
    canonical_digest(payload)
}

#[must_use]
pub fn workflow_release_reviewer_key_fingerprint_v2(public_key: &[u8; 32]) -> String {
    format!("sha256:{:x}", Sha256::digest(public_key))
}

/// Verifies the complete V2 review, registry, role, and signature boundary.
///
/// # Errors
///
/// Returns [`WorkflowReleaseAdmissionAuthorityErrorV2`] for any structural,
/// binding, validity, separation, digest, key, or signature failure.
// Keeping the security checks in one linear gate makes the fail-closed order
// auditable; extracting stateful fragments would obscure which values have
// been authenticated at each return point.
#[allow(clippy::too_many_lines)]
pub fn verify_workflow_release_admission_authorization_v2(
    reviewer_registry: &WorkflowReleaseReviewerRegistryDocument,
    reviewer_registry_raw_bytes: &[u8],
    authorization: &WorkflowReleaseAdmissionAuthorizationV2Document,
    expected: WorkflowReleaseAdmissionExpectedContextV2<'_>,
    expected_audience: &str,
) -> Result<VerifiedWorkflowReleaseAdmissionAuthorizationV2, WorkflowReleaseAdmissionAuthorityErrorV2>
{
    validate_contracts(reviewer_registry, authorization, expected.review_index)?;
    let payload = &authorization
        .workflow_release_admission_authorization
        .payload;
    if payload.audience != expected_audience {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::WrongAudience {
            expected: expected_audience.to_owned(),
            found: payload.audience.clone(),
        });
    }
    if payload.domain != WORKFLOW_RELEASE_ADMISSION_PAYLOAD_DOMAIN_V2 {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::WrongDomain {
            found: payload.domain.clone(),
        });
    }
    if payload
        .workflow_decisions
        .iter()
        .any(|d| d.decision != WorkflowReleaseReviewDecision::Approved)
        || payload
            .quarantine_decisions
            .iter()
            .any(|d| d.decision != WorkflowReleaseReviewDecision::Approved)
        || payload
            .dimension_decisions
            .iter()
            .any(|d| d.decision != WorkflowReleaseReviewDecision::Approved)
    {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::BlockingReviewDecision);
    }

    let index = &expected.review_index.workflow_release_review_index;
    require_binding(payload.review_index_id == index.id, "review_index_id")?;
    require_binding(
        payload.review_index_version == index.index_version,
        "review_index_version",
    )?;
    require_binding(
        payload.review_index_raw_digest == raw_digest(expected.review_index_raw_bytes),
        "review_index_raw_digest",
    )?;
    let index_digest = canonical_digest(expected.review_index)?;
    require_binding(
        payload.review_index_canonical_digest == index_digest,
        "review_index_canonical_digest",
    )?;
    require_binding(
        payload.evaluation_digest == expected.evaluation_digest,
        "evaluation_digest",
    )?;
    require_binding(payload.promotion == index.promotion, "promotion")?;
    require_binding(
        payload.release_manifest == index.release_manifest,
        "release_manifest",
    )?;
    require_binding(
        payload.review_subject == index.review_subject,
        "review_subject",
    )?;
    require_binding(payload.full_catalog == index.full_catalog, "full_catalog")?;
    require_binding(
        payload.predecessor_registry == index.predecessor_registry,
        "predecessor_registry",
    )?;
    require_binding(
        payload.proposed_registry == index.proposed_registry,
        "proposed_registry",
    )?;
    require_binding(
        payload.workflow_decisions == index.workflow_decisions,
        "workflow_decisions",
    )?;
    require_binding(
        payload.quarantine_decisions == index.quarantine_decisions,
        "quarantine_decisions",
    )?;
    require_binding(
        payload.dimension_decisions == index.dimension_decisions,
        "dimension_decisions",
    )?;
    require_binding(payload.invalidate_all_receipts, "invalidate_all_receipts")?;

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

    let payload_digest = workflow_release_admission_payload_digest_v2(payload)?;
    let signatures = &authorization
        .workflow_release_admission_authorization
        .signatures;
    let mut verified = Vec::with_capacity(signatures.len());
    let mut signature_values = BTreeSet::new();
    for signature in signatures {
        if !signature_values.insert(&signature.signature) {
            return Err(WorkflowReleaseAdmissionAuthorityErrorV2::DuplicateSignature);
        }
        if signature.payload_digest != payload_digest {
            return Err(
                WorkflowReleaseAdmissionAuthorityErrorV2::PayloadDigestMismatch {
                    credential_id: signature.credential_id.0.clone(),
                },
            );
        }
        verified.push(verify_signature(registry, payload, signature)?);
    }
    let semantic = verified
        .iter()
        .find(|(r, _)| *r == WorkflowReleaseReviewerRole::SemanticReviewer)
        .ok_or(
            WorkflowReleaseAdmissionAuthorityErrorV2::MissingRequiredRole {
                role: WorkflowReleaseReviewerRole::SemanticReviewer,
            },
        )?;
    let authorizer = verified
        .iter()
        .find(|(r, _)| *r == WorkflowReleaseReviewerRole::ReleaseAuthorizer)
        .ok_or(
            WorkflowReleaseAdmissionAuthorityErrorV2::MissingRequiredRole {
                role: WorkflowReleaseReviewerRole::ReleaseAuthorizer,
            },
        )?;
    require_separation(&semantic.1, &authorizer.1)?;

    Ok(VerifiedWorkflowReleaseAdmissionAuthorizationV2 {
        authorization_id: payload.authorization_id.0.clone(),
        payload_digest,
        review_index_raw_digest: payload.review_index_raw_digest.clone(),
        review_index_canonical_digest: index_digest,
        evaluation_digest: payload.evaluation_digest.clone(),
        reviewer_registry_digest: registry_digest,
        predecessor_registry_digest: payload.predecessor_registry.canonical_digest.clone(),
        proposed_registry_digest: payload.proposed_registry.canonical_digest.clone(),
        predecessor_release_id: payload.promotion.predecessor.release_id.0.clone(),
        predecessor_release_digest: payload.promotion.predecessor.release_digest.clone(),
        candidate_release: payload.promotion.candidate_release.clone(),
        candidate_runtime_bundle: payload.promotion.candidate_runtime_bundle.clone(),
        promoted_runtime_bundle: payload.promotion.promoted_runtime_bundle.clone(),
        semantic_reviewer: copy_reviewer(semantic),
        release_authorizer: copy_reviewer(authorizer),
    })
}

fn validate_contracts(
    registry: &WorkflowReleaseReviewerRegistryDocument,
    authorization: &WorkflowReleaseAdmissionAuthorizationV2Document,
    review_index: &WorkflowReleaseReviewIndexV2Document,
) -> Result<(), WorkflowReleaseAdmissionAuthorityErrorV2> {
    if registry.schema_version != WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "reviewer registry",
            issue: "unsupported schema version".to_owned(),
        });
    }
    if authorization.schema_version != WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "V2 admission authorization",
            issue: "unsupported schema version".to_owned(),
        });
    }
    if review_index.schema_version != WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "V2 review index",
            issue: "unsupported schema version".to_owned(),
        });
    }
    if let Some(issue) = registry.validate().first() {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "reviewer registry",
            issue: format!("{}: {}", issue.path, issue.message),
        });
    }
    if let Some(issue) = authorization.validate().first() {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "V2 admission authorization",
            issue: format!("{}: {}", issue.path, issue.message),
        });
    }
    if let Some(issue) = review_index.validate().first() {
        return Err(WorkflowReleaseAdmissionAuthorityErrorV2::InvalidContract {
            document: "V2 review index",
            issue: format!("{}: {}", issue.path, issue.message),
        });
    }
    Ok(())
}

fn verify_signature(
    registry: &WorkflowReleaseReviewerRegistry,
    payload: &WorkflowReleaseAdmissionAuthorizationPayloadV2,
    signature: &WorkflowReleaseAdmissionSignatureV2,
) -> Result<
    (WorkflowReleaseReviewerRole, VerifiedReviewerV2),
    WorkflowReleaseAdmissionAuthorityErrorV2,
> {
    let credential_id = signature.credential_id.0.clone();
    let credential = registry
        .credentials
        .iter()
        .find(|c| c.credential_id == signature.credential_id)
        .ok_or_else(
            || WorkflowReleaseAdmissionAuthorityErrorV2::CredentialNotFound {
                credential_id: credential_id.clone(),
            },
        )?;
    validate_credential(credential, payload, signature)?;
    let public_key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
        WorkflowReleaseAdmissionAuthorityErrorV2::PublicKeyDecode {
            credential_id: credential_id.clone(),
        }
    })?;
    if workflow_release_reviewer_key_fingerprint_v2(&public_key_bytes)
        != credential.public_key_fingerprint
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::PublicKeyFingerprintMismatch {
                credential_id,
            },
        );
    }
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
        WorkflowReleaseAdmissionAuthorityErrorV2::PublicKeyDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let signature_bytes = decode_fixed::<64>(&signature.signature).ok_or_else(|| {
        WorkflowReleaseAdmissionAuthorityErrorV2::SignatureDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let detached = Signature::from_bytes(&signature_bytes);
    let signing_bytes = workflow_release_admission_signing_bytes_v2(payload, signature)?;
    verifying_key
        .verify_strict(&signing_bytes, &detached)
        .map_err(
            |_| WorkflowReleaseAdmissionAuthorityErrorV2::SignatureInvalid {
                credential_id: signature.credential_id.0.clone(),
            },
        )?;
    Ok((
        signature.role,
        VerifiedReviewerV2 {
            principal_id: signature.principal_id.0.clone(),
            credential_id: signature.credential_id.0.clone(),
            independence_domain: credential.independence_domain.clone(),
            public_key_fingerprint: credential.public_key_fingerprint.clone(),
            signature_fingerprint: raw_digest(&signature_bytes),
            signed_at_unix: signature.signed_at_unix,
        },
    ))
}

fn validate_credential(
    credential: &WorkflowReleaseReviewerCredential,
    payload: &WorkflowReleaseAdmissionAuthorizationPayloadV2,
    signature: &WorkflowReleaseAdmissionSignatureV2,
) -> Result<(), WorkflowReleaseAdmissionAuthorityErrorV2> {
    let credential_id = signature.credential_id.0.clone();
    if credential.status != WorkflowReleaseReviewerCredentialStatus::Active {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::CredentialNotActive { credential_id },
        );
    }
    if credential.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
        || signature.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
        || !credential.roles.contains(&signature.role)
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::CredentialRoleMismatch { credential_id },
        );
    }
    if credential.principal_id != signature.principal_id {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::CredentialPrincipalMismatch { credential_id },
        );
    }
    if signature.signed_at_unix < credential.valid_from_unix
        || signature.signed_at_unix > credential.valid_until_unix
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::CredentialOutsideValidity {
                credential_id,
                signed_at_unix: signature.signed_at_unix,
            },
        );
    }
    if signature.signed_at_unix < payload.issued_at_unix
        || signature.signed_at_unix > payload.expires_at_unix
    {
        return Err(
            WorkflowReleaseAdmissionAuthorityErrorV2::AuthorizationOutsideValidity {
                credential_id,
                signed_at_unix: signature.signed_at_unix,
            },
        );
    }
    Ok(())
}

fn copy_reviewer(pair: &(WorkflowReleaseReviewerRole, VerifiedReviewerV2)) -> VerifiedReviewerV2 {
    VerifiedReviewerV2 {
        principal_id: pair.1.principal_id.clone(),
        credential_id: pair.1.credential_id.clone(),
        independence_domain: pair.1.independence_domain.clone(),
        public_key_fingerprint: pair.1.public_key_fingerprint.clone(),
        signature_fingerprint: pair.1.signature_fingerprint.clone(),
        signed_at_unix: pair.1.signed_at_unix,
    }
}
fn require_separation(
    a: &VerifiedReviewerV2,
    b: &VerifiedReviewerV2,
) -> Result<(), WorkflowReleaseAdmissionAuthorityErrorV2> {
    for (different, dimension) in [
        (a.principal_id != b.principal_id, "principal"),
        (a.credential_id != b.credential_id, "credential"),
        (
            a.independence_domain != b.independence_domain,
            "independence domain",
        ),
        (
            a.public_key_fingerprint != b.public_key_fingerprint,
            "public key fingerprint",
        ),
        (
            a.signature_fingerprint != b.signature_fingerprint,
            "signature fingerprint",
        ),
    ] {
        if !different {
            return Err(
                WorkflowReleaseAdmissionAuthorityErrorV2::ReviewerSeparationViolation { dimension },
            );
        }
    }
    Ok(())
}
fn require_binding(
    matches: bool,
    field: &'static str,
) -> Result<(), WorkflowReleaseAdmissionAuthorityErrorV2> {
    if matches {
        Ok(())
    } else {
        Err(WorkflowReleaseAdmissionAuthorityErrorV2::BindingMismatch { field })
    }
}
fn canonical_digest<T: Serialize>(
    value: &T,
) -> Result<String, WorkflowReleaseAdmissionAuthorityErrorV2> {
    let json = serde_json::to_value(value)
        .map_err(|e| WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization(e.to_string()))?;
    let canonical = serde_json_canonicalizer::to_vec(&json)
        .map_err(|e| WorkflowReleaseAdmissionAuthorityErrorV2::Canonicalization(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_contracts::workflow_release_review_v2::{
        WorkflowReleaseAdmissionAuthorizationPayloadV2, WorkflowReleaseAdmissionAuthorizationV2,
        WorkflowReleaseAdmissionAuthorizationV2Authority, WorkflowReleaseAdmissionSignatureV2,
        WorkflowReleaseReviewIndexV2Document,
    };
    use forge_core_contracts::{PrincipalId, StableId};
    use serde_json::json;
    use std::fmt::Write as _;

    const AUDIENCE: &str = "forge-runtime:workflow-governance-release";

    struct Fixture {
        registry: WorkflowReleaseReviewerRegistryDocument,
        registry_raw: Vec<u8>,
        index: WorkflowReleaseReviewIndexV2Document,
        index_raw: Vec<u8>,
        evaluation_digest: String,
        authorization: WorkflowReleaseAdmissionAuthorizationV2Document,
        semantic_key: SigningKey,
        authorizer_key: SigningKey,
    }

    fn hex(bytes: &[u8]) -> String {
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        encoded
    }
    fn digest(value: &[u8]) -> String {
        raw_digest(value)
    }
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }
    fn dimensions() -> Vec<serde_json::Value> {
        [
            "status",
            "eligibility",
            "progression",
            "completion",
            "obligations",
            "claims",
            "decisions",
            "capabilities",
            "issues",
            "next_actions",
        ]
        .into_iter()
        .map(|dimension| {
            json!({
                "dimension": dimension, "decision": "approved", "rationale": "reviewed",
                "finding_refs": [format!("finding.{dimension}")]
            })
        })
        .collect()
    }
    fn artifact(id: &str) -> serde_json::Value {
        json!({
            "artifact_id": id, "embedded_ref": format!("contracts/{id}.yaml"),
            "raw_digest": digest(format!("raw:{id}").as_bytes()),
            "canonical_digest": digest(format!("canonical:{id}").as_bytes())
        })
    }
    fn index(release: &str, predecessor: &str) -> WorkflowReleaseReviewIndexV2Document {
        serde_json::from_value(json!({
            "schema_version": "0.2",
            "workflow_release_review_index": {
                "id": format!("review.{release}"), "index_version": "2.0.0", "authority": "candidate_only",
                "promotion": {
                    "predecessor": { "release_id": predecessor, "release_digest": digest(predecessor.as_bytes()) },
                    "candidate_release": { "lineage_id": "lineage.core", "release_id": release,
                        "release_version": "0.4.0", "release_digest": digest(release.as_bytes()) },
                    "candidate_runtime_bundle": { "bundle_id": format!("bundle.{release}.shadow"),
                        "bundle_digest": digest(format!("shadow:{release}").as_bytes()),
                        "policy_set_digest": digest(format!("policies:{release}").as_bytes()) },
                    "promoted_runtime_bundle": { "bundle_id": format!("bundle.{release}.promoted"),
                        "bundle_digest": digest(format!("promoted:{release}").as_bytes()),
                        "policy_set_digest": digest(format!("policies:{release}").as_bytes()) }
                },
                "release_manifest": artifact("release"), "migration_batches": [artifact("batch")],
                "review_subject": artifact("subject"), "coverage_policy": artifact("coverage"),
                "full_catalog": artifact("catalog"), "corpus_set": artifact("corpus-set"),
                "representative_corpus": artifact("representative"), "adversarial_corpus": artifact("adversarial"),
                "shadow_report": artifact("report"), "candidate_runtime_bundle": artifact("candidate-bundle"),
                "promoted_runtime_bundle": artifact("promoted-bundle"), "predecessor_registry": artifact("predecessor-registry"),
                "proposed_registry": artifact("proposed-registry"), "evaluator_source": artifact("evaluator"),
                "frozen_history": artifact("history"),
                "workflow_decisions": [{ "workflow_id": "workflow.one", "decision": "approved",
                    "rationale": "reviewed", "finding_refs": ["finding.workflow"] }],
                "quarantine_decisions": [{ "workflow_id": "workflow.quarantined", "decision": "approved",
                    "rationale": "remains quarantined", "finding_refs": ["finding.quarantine"] }],
                "dimension_decisions": dimensions()
            }
        })).unwrap()
    }

    fn fixture() -> Fixture {
        let semantic_key = key(41);
        let authorizer_key = key(42);
        let registry: WorkflowReleaseReviewerRegistryDocument = serde_json::from_value(json!({
            "schema_version": "0.1", "workflow_release_reviewer_registry": {
                "registry_id": "reviewers.v2", "registry_version": "2.0.0", "authority": "candidate_only",
                "credentials": [
                    { "credential_id": "credential.semantic", "principal_id": "principal.semantic",
                      "public_key_fingerprint": workflow_release_reviewer_key_fingerprint_v2(semantic_key.verifying_key().as_bytes()),
                      "public_key_hex": hex(semantic_key.verifying_key().as_bytes()), "algorithm": "ed25519",
                      "roles": ["semantic_reviewer"], "status": "active", "valid_from_unix": 100,
                      "valid_until_unix": 1000, "independence_domain": "semantic" },
                    { "credential_id": "credential.authorizer", "principal_id": "principal.authorizer",
                      "public_key_fingerprint": workflow_release_reviewer_key_fingerprint_v2(authorizer_key.verifying_key().as_bytes()),
                      "public_key_hex": hex(authorizer_key.verifying_key().as_bytes()), "algorithm": "ed25519",
                      "roles": ["release_authorizer"], "status": "active", "valid_from_unix": 100,
                      "valid_until_unix": 1000, "independence_domain": "operations" }
                ]
            }
        })).unwrap();
        let registry_raw = serde_json::to_vec_pretty(&registry).unwrap();
        let index = index("release.assurance", "release.core");
        let index_raw = serde_json::to_vec_pretty(&index).unwrap();
        let evaluation_digest = digest(b"evaluation:release.assurance");
        let index_value = &index.workflow_release_review_index;
        let payload = WorkflowReleaseAdmissionAuthorizationPayloadV2 {
            authorization_id: StableId("authorization.release.assurance".to_owned()),
            review_index_id: index_value.id.clone(),
            review_index_version: index_value.index_version.clone(),
            review_index_raw_digest: digest(&index_raw),
            review_index_canonical_digest: canonical_digest(&index).unwrap(),
            evaluation_digest: evaluation_digest.clone(),
            reviewer_registry_id: registry
                .workflow_release_reviewer_registry
                .registry_id
                .clone(),
            reviewer_registry_version: registry
                .workflow_release_reviewer_registry
                .registry_version
                .clone(),
            reviewer_registry_raw_digest: digest(&registry_raw),
            reviewer_registry_canonical_digest: canonical_digest(&registry).unwrap(),
            promotion: index_value.promotion.clone(),
            release_manifest: index_value.release_manifest.clone(),
            review_subject: index_value.review_subject.clone(),
            full_catalog: index_value.full_catalog.clone(),
            predecessor_registry: index_value.predecessor_registry.clone(),
            proposed_registry: index_value.proposed_registry.clone(),
            invalidate_all_receipts: true,
            workflow_decisions: index_value.workflow_decisions.clone(),
            quarantine_decisions: index_value.quarantine_decisions.clone(),
            dimension_decisions: index_value.dimension_decisions.clone(),
            audience: AUDIENCE.to_owned(),
            domain: WORKFLOW_RELEASE_ADMISSION_PAYLOAD_DOMAIN_V2.to_owned(),
            nonce: "nonce:release.assurance".to_owned(),
            issued_at_unix: 200,
            expires_at_unix: 900,
        };
        let signature =
            |principal: &str, credential: &str, role| WorkflowReleaseAdmissionSignatureV2 {
                principal_id: PrincipalId(principal.to_owned()),
                credential_id: StableId(credential.to_owned()),
                role,
                algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
                payload_digest: digest(b"pending"),
                signature: "00".repeat(64),
                signed_at_unix: 500,
            };
        let authorization = WorkflowReleaseAdmissionAuthorizationV2Document {
            schema_version: WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION.to_owned(),
            workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorizationV2 {
                authority: WorkflowReleaseAdmissionAuthorizationV2Authority::CandidateAuthorization,
                payload,
                signatures: vec![
                    signature(
                        "principal.semantic",
                        "credential.semantic",
                        WorkflowReleaseReviewerRole::SemanticReviewer,
                    ),
                    signature(
                        "principal.authorizer",
                        "credential.authorizer",
                        WorkflowReleaseReviewerRole::ReleaseAuthorizer,
                    ),
                ],
            },
        };
        let mut fixture = Fixture {
            registry,
            registry_raw,
            index,
            index_raw,
            evaluation_digest,
            authorization,
            semantic_key,
            authorizer_key,
        };
        resign(&mut fixture);
        fixture
    }

    fn resign(fixture: &mut Fixture) {
        let authorization = &mut fixture
            .authorization
            .workflow_release_admission_authorization;
        let payload_digest =
            workflow_release_admission_payload_digest_v2(&authorization.payload).unwrap();
        for (position, signature) in authorization.signatures.iter_mut().enumerate() {
            signature.payload_digest.clone_from(&payload_digest);
            let bytes =
                workflow_release_admission_signing_bytes_v2(&authorization.payload, signature)
                    .unwrap();
            let key = if position == 0 {
                &fixture.semantic_key
            } else {
                &fixture.authorizer_key
            };
            signature.signature = hex(&key.sign(&bytes).to_bytes());
        }
    }
    fn verify(
        fixture: &Fixture,
    ) -> Result<
        VerifiedWorkflowReleaseAdmissionAuthorizationV2,
        WorkflowReleaseAdmissionAuthorityErrorV2,
    > {
        verify_workflow_release_admission_authorization_v2(
            &fixture.registry,
            &fixture.registry_raw,
            &fixture.authorization,
            WorkflowReleaseAdmissionExpectedContextV2 {
                review_index: &fixture.index,
                review_index_raw_bytes: &fixture.index_raw,
                evaluation_digest: &fixture.evaluation_digest,
            },
            AUDIENCE,
        )
    }

    #[test]
    fn verifies_release_specific_move_only_capability() {
        let fixture = fixture();
        let capability = verify(&fixture).expect("valid V2 authorization");
        assert_eq!(
            capability.candidate_release().release_id.0,
            "release.assurance"
        );
        assert_eq!(capability.audit().predecessor_release_id, "release.core");
        assert_eq!(
            capability.audit().evaluation_digest,
            fixture.evaluation_digest
        );
    }

    #[test]
    fn rejects_signed_payload_tamper() {
        let mut fixture = fixture();
        fixture
            .authorization
            .workflow_release_admission_authorization
            .payload
            .nonce
            .push_str(":tampered");
        assert!(matches!(
            verify(&fixture),
            Err(WorkflowReleaseAdmissionAuthorityErrorV2::PayloadDigestMismatch { .. })
        ));
    }

    #[test]
    fn authorization_for_one_release_cannot_authorize_its_successor() {
        let mut fixture = fixture();
        fixture.index = index("release.next", "release.assurance");
        fixture.index_raw = serde_json::to_vec_pretty(&fixture.index).unwrap();
        fixture.evaluation_digest = digest(b"evaluation:release.next");
        assert!(matches!(
            verify(&fixture),
            Err(WorkflowReleaseAdmissionAuthorityErrorV2::BindingMismatch { .. })
        ));
    }

    #[test]
    fn v2_signature_domain_is_distinct_from_frozen_v1() {
        assert_ne!(
            WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN_V2,
            crate::workflow_release_review::WORKFLOW_RELEASE_ADMISSION_SIGNATURE_DOMAIN
        );
    }
}
