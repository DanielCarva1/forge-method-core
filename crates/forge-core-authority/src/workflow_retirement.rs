//! P5d.5 cryptographic authority for aggregate legacy-workflow retirement.
//!
//! Retirement is a product/binary checkpoint, not a per-project mutation. This
//! module authenticates an exact, evaluator-selected aggregate and returns an
//! opaque move-only capability. The kernel owns one-time process activation;
//! this stateless verifier intentionally performs no replay-store mutation.

use std::collections::BTreeSet;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::workflow_retirement::{
    WorkflowRetirementArtifactBinding, WorkflowRetirementAuthorizationV2Document,
    WorkflowRetirementAuthorizationV2Payload, WorkflowRetirementWorkflowBinding,
    WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION,
};
use forge_core_contracts::{
    WorkflowGovernanceReleaseIdentity, WorkflowReleaseAdmissionSignatureAlgorithm,
    WorkflowReleaseAdmissionSignatureV2, WorkflowReleaseReviewerCredential,
    WorkflowReleaseReviewerCredentialStatus, WorkflowReleaseReviewerRegistry,
    WorkflowReleaseReviewerRegistryDocument, WorkflowReleaseReviewerRole,
    WorkflowRuntimeBundleIdentity, WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const WORKFLOW_RETIREMENT_SIGNATURE_DOMAIN_V2: &[u8] = b"forge-method:workflow-retirement:v2\0";
pub const WORKFLOW_RETIREMENT_PAYLOAD_DOMAIN_V2: &str = "forge-method:workflow-retirement:v2";
pub const WORKFLOW_RETIREMENT_AGGREGATE_SIZE: usize = 42;

/// Exact trusted context selected after deterministic evidence evaluation.
#[derive(Clone, Copy)]
pub struct WorkflowRetirementExpectedContextV2<'a> {
    pub release: &'a WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: &'a WorkflowRuntimeBundleIdentity,
    pub legacy_catalog_digest: &'a str,
    pub retirements: &'a [WorkflowRetirementWorkflowBinding],
    pub release_manifest: &'a WorkflowRetirementArtifactBinding,
    pub runtime_bundle_artifact: &'a WorkflowRetirementArtifactBinding,
    pub snapshot_manifest: &'a WorkflowRetirementArtifactBinding,
    pub runtime_evidence: &'a WorkflowRetirementArtifactBinding,
    pub release_history: &'a WorkflowRetirementArtifactBinding,
    pub evidence_index: &'a WorkflowRetirementArtifactBinding,
    pub deletion_proof: &'a WorkflowRetirementArtifactBinding,
    pub consumer_report: &'a WorkflowRetirementArtifactBinding,
    pub tombstone_catalog: &'a WorkflowRetirementArtifactBinding,
    pub final_scorecard: &'a WorkflowRetirementArtifactBinding,
    pub reviewer_registry: &'a WorkflowRetirementArtifactBinding,
    /// Fixed checkpoint epoch compiled into the admitting binary, never wall-clock time.
    pub admission_epoch_unix: u64,
    pub consumer_observed_until_unix: u64,
    pub reviewer_registry_raw_digest: &'a str,
    pub evidence_reviewer_key_fingerprint: &'a str,
    pub retirement_authorizer_key_fingerprint: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRetirementAuthorityErrorV2 {
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
    BindingMismatch {
        field: &'static str,
    },
    InvalidAggregate {
        issue: &'static str,
    },
    AuthorizationNotYetValid,
    AuthorizationExpired,
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
    },
    SignatureOutsideValidity {
        credential_id: String,
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

impl fmt::Display for WorkflowRetirementAuthorityErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { document, issue } => write!(f, "invalid {document}: {issue}"),
            Self::WrongAudience { expected, found } => write!(
                f,
                "wrong retirement audience '{found}', expected '{expected}'"
            ),
            Self::WrongDomain { found } => write!(f, "wrong retirement domain '{found}'"),
            Self::BindingMismatch { field } => {
                write!(f, "retirement binding mismatch at '{field}'")
            }
            Self::InvalidAggregate { issue } => write!(f, "invalid retirement aggregate: {issue}"),
            Self::AuthorizationNotYetValid => {
                f.write_str("retirement authorization is not yet valid")
            }
            Self::AuthorizationExpired => f.write_str("retirement authorization has expired"),
            Self::PayloadDigestMismatch { credential_id } => write!(
                f,
                "payload digest mismatch for credential '{credential_id}'"
            ),
            Self::CredentialNotFound { credential_id } => {
                write!(f, "reviewer credential '{credential_id}' not found")
            }
            Self::CredentialNotActive { credential_id } => {
                write!(f, "reviewer credential '{credential_id}' is not active")
            }
            Self::CredentialRoleMismatch { credential_id } => write!(
                f,
                "credential '{credential_id}' lacks the signed retirement role or algorithm"
            ),
            Self::CredentialPrincipalMismatch { credential_id } => {
                write!(f, "credential '{credential_id}' has another principal")
            }
            Self::CredentialOutsideValidity { credential_id } => write!(
                f,
                "credential '{credential_id}' is outside its validity window"
            ),
            Self::SignatureOutsideValidity { credential_id } => write!(
                f,
                "signature from credential '{credential_id}' is outside the authorization window"
            ),
            Self::PublicKeyDecode { credential_id } => write!(
                f,
                "cannot decode public key for credential '{credential_id}'"
            ),
            Self::PublicKeyFingerprintMismatch { credential_id } => write!(
                f,
                "public-key fingerprint mismatch for credential '{credential_id}'"
            ),
            Self::SignatureDecode { credential_id } => write!(
                f,
                "cannot decode signature for credential '{credential_id}'"
            ),
            Self::SignatureInvalid { credential_id } => {
                write!(f, "invalid signature for credential '{credential_id}'")
            }
            Self::ReviewerSeparationViolation { dimension } => {
                write!(f, "retirement reviewer separation violated for {dimension}")
            }
            Self::DuplicateSignature => f.write_str("duplicate retirement signature"),
            Self::MissingRequiredRole { role } => {
                write!(f, "missing retirement reviewer role {role:?}")
            }
            Self::Canonicalization(message) => write!(f, "canonicalization failed: {message}"),
        }
    }
}
impl std::error::Error for WorkflowRetirementAuthorityErrorV2 {}

/// Move-only, non-serializable authority for the exact 42-workflow checkpoint.
pub struct VerifiedWorkflowRetirementAuthorizationV2 {
    authorization_id: String,
    payload_digest: String,
    release: WorkflowGovernanceReleaseIdentity,
    runtime_bundle: WorkflowRuntimeBundleIdentity,
    legacy_catalog_digest: String,
    retirement_set_digest: String,
    final_scorecard_digest: String,
    nonce: String,
    evidence_reviewer: VerifiedRetirementReviewer,
    retirement_authorizer: VerifiedRetirementReviewer,
}

impl fmt::Debug for VerifiedWorkflowRetirementAuthorizationV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedWorkflowRetirementAuthorizationV2")
            .field("authorization_id", &self.authorization_id)
            .field("payload_digest", &self.payload_digest)
            .field("release", &self.release)
            .field("retirement_set_digest", &self.retirement_set_digest)
            .finish_non_exhaustive()
    }
}

impl VerifiedWorkflowRetirementAuthorizationV2 {
    #[must_use]
    pub fn authorization_id(&self) -> &str {
        &self.authorization_id
    }
    #[must_use]
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
    #[must_use]
    pub fn release(&self) -> &WorkflowGovernanceReleaseIdentity {
        &self.release
    }
    #[must_use]
    pub fn runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.runtime_bundle
    }
    #[must_use]
    pub fn legacy_catalog_digest(&self) -> &str {
        &self.legacy_catalog_digest
    }
    #[must_use]
    pub fn retirement_set_digest(&self) -> &str {
        &self.retirement_set_digest
    }
    #[must_use]
    pub fn final_scorecard_digest(&self) -> &str {
        &self.final_scorecard_digest
    }
    #[must_use]
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
    #[must_use]
    pub fn audit(&self) -> VerifiedWorkflowRetirementAuthorizationAuditV2 {
        VerifiedWorkflowRetirementAuthorizationAuditV2 {
            authority: WorkflowRetirementAuditAuthorityV2::NonAuthoritative,
            authorization_id: self.authorization_id.clone(),
            payload_digest: self.payload_digest.clone(),
            release_id: self.release.release_id.0.clone(),
            release_digest: self.release.release_digest.clone(),
            runtime_bundle_id: self.runtime_bundle.bundle_id.0.clone(),
            runtime_bundle_digest: self.runtime_bundle.bundle_digest.clone(),
            legacy_catalog_digest: self.legacy_catalog_digest.clone(),
            retirement_set_digest: self.retirement_set_digest.clone(),
            final_scorecard_digest: self.final_scorecard_digest.clone(),
            nonce_digest: raw_digest(self.nonce.as_bytes()),
            evidence_reviewer: self.evidence_reviewer.audit(),
            retirement_authorizer: self.retirement_authorizer.audit(),
        }
    }
}

#[derive(Debug)]
struct VerifiedRetirementReviewer {
    principal_id: String,
    credential_id: String,
    independence_domain: String,
    public_key_fingerprint: String,
    signature_fingerprint: String,
    signed_at_unix: u64,
}
impl VerifiedRetirementReviewer {
    fn audit(&self) -> VerifiedWorkflowRetirementReviewerAuditV2 {
        VerifiedWorkflowRetirementReviewerAuditV2 {
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
pub enum WorkflowRetirementAuditAuthorityV2 {
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowRetirementReviewerAuditV2 {
    pub principal_id: String,
    pub credential_id: String,
    pub independence_domain: String,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowRetirementAuthorizationAuditV2 {
    pub authority: WorkflowRetirementAuditAuthorityV2,
    pub authorization_id: String,
    pub payload_digest: String,
    pub release_id: String,
    pub release_digest: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub legacy_catalog_digest: String,
    pub retirement_set_digest: String,
    pub final_scorecard_digest: String,
    pub nonce_digest: String,
    pub evidence_reviewer: VerifiedWorkflowRetirementReviewerAuditV2,
    pub retirement_authorizer: VerifiedWorkflowRetirementReviewerAuditV2,
}

/// Produces domain-separated JCS bytes for one detached retirement signature.
///
/// # Errors
///
/// Returns [`WorkflowRetirementAuthorityErrorV2::Canonicalization`] if the
/// closed payload or signature envelope cannot be encoded as canonical JSON.
pub fn workflow_retirement_signing_bytes_v2(
    payload: &WorkflowRetirementAuthorizationV2Payload,
    signature: &WorkflowReleaseAdmissionSignatureV2,
) -> Result<Vec<u8>, WorkflowRetirementAuthorityErrorV2> {
    #[derive(Serialize)]
    struct Envelope<'a> {
        authorization_id: &'a str,
        payload: &'a WorkflowRetirementAuthorizationV2Payload,
        credential_id: &'a str,
        role: WorkflowReleaseReviewerRole,
        signed_at_unix: u64,
    }
    let envelope = Envelope {
        authorization_id: &payload.authorization_id.0,
        payload,
        credential_id: &signature.credential_id.0,
        role: signature.role,
        signed_at_unix: signature.signed_at_unix,
    };
    let canonical = serde_json_canonicalizer::to_vec(
        &serde_json::to_value(envelope)
            .map_err(|e| WorkflowRetirementAuthorityErrorV2::Canonicalization(e.to_string()))?,
    )
    .map_err(|e| WorkflowRetirementAuthorityErrorV2::Canonicalization(e.to_string()))?;
    let mut bytes =
        Vec::with_capacity(WORKFLOW_RETIREMENT_SIGNATURE_DOMAIN_V2.len() + canonical.len());
    bytes.extend_from_slice(WORKFLOW_RETIREMENT_SIGNATURE_DOMAIN_V2);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

/// Computes the canonical content identity of an aggregate retirement payload.
///
/// # Errors
///
/// Returns [`WorkflowRetirementAuthorityErrorV2::Canonicalization`] if the
/// payload cannot be encoded as canonical JSON.
pub fn workflow_retirement_payload_digest_v2(
    payload: &WorkflowRetirementAuthorizationV2Payload,
) -> Result<String, WorkflowRetirementAuthorityErrorV2> {
    canonical_digest(payload)
}

#[must_use]
pub fn workflow_retirement_reviewer_key_fingerprint_v2(public_key: &[u8; 32]) -> String {
    raw_digest(public_key)
}

/// Verifies exact aggregate bindings, trusted time, registry authority, two
/// independent Ed25519 signatures, and returns an opaque retirement capability.
///
/// Repeated verification is idempotent. The kernel must consume the returned
/// move-only capability once through its process-wide activation boundary.
///
/// # Errors
/// Returns a fail-closed error for every structural, binding, time, registry,
/// separation, digest, key, or signature failure.
#[allow(clippy::too_many_lines)]
pub fn verify_workflow_retirement_authorization_v2(
    registry: &WorkflowReleaseReviewerRegistryDocument,
    registry_raw_bytes: &[u8],
    authorization: &WorkflowRetirementAuthorizationV2Document,
    expected: WorkflowRetirementExpectedContextV2<'_>,
    expected_audience: &str,
) -> Result<VerifiedWorkflowRetirementAuthorizationV2, WorkflowRetirementAuthorityErrorV2> {
    let payload = &authorization.workflow_retirement_authorization_v2.payload;
    validate_shape(
        registry,
        authorization,
        payload,
        expected.admission_epoch_unix,
    )?;
    if payload.audience != expected_audience {
        return Err(WorkflowRetirementAuthorityErrorV2::WrongAudience {
            expected: expected_audience.to_owned(),
            found: payload.audience.clone(),
        });
    }
    if payload.domain != WORKFLOW_RETIREMENT_PAYLOAD_DOMAIN_V2 {
        return Err(WorkflowRetirementAuthorityErrorV2::WrongDomain {
            found: payload.domain.clone(),
        });
    }
    require_binding(payload.release == *expected.release, "release")?;
    require_binding(
        payload.runtime_bundle == *expected.runtime_bundle,
        "runtime_bundle",
    )?;
    require_binding(
        payload.legacy_catalog_digest == expected.legacy_catalog_digest,
        "legacy_catalog_digest",
    )?;
    require_binding(payload.retirements == expected.retirements, "retirements")?;
    for (name, actual, wanted) in [
        (
            "release_manifest",
            &payload.release_manifest,
            expected.release_manifest,
        ),
        (
            "runtime_bundle_artifact",
            &payload.runtime_bundle_artifact,
            expected.runtime_bundle_artifact,
        ),
        (
            "snapshot_manifest",
            &payload.snapshot_manifest,
            expected.snapshot_manifest,
        ),
        (
            "runtime_evidence",
            &payload.runtime_evidence,
            expected.runtime_evidence,
        ),
        (
            "release_history",
            &payload.release_history,
            expected.release_history,
        ),
        (
            "evidence_index",
            &payload.evidence_index,
            expected.evidence_index,
        ),
        (
            "deletion_proof",
            &payload.deletion_proof,
            expected.deletion_proof,
        ),
        (
            "consumer_report",
            &payload.consumer_report,
            expected.consumer_report,
        ),
        (
            "tombstone_catalog",
            &payload.tombstone_catalog,
            expected.tombstone_catalog,
        ),
        (
            "final_scorecard",
            &payload.final_scorecard,
            expected.final_scorecard,
        ),
        (
            "reviewer_registry",
            &payload.reviewer_registry,
            expected.reviewer_registry,
        ),
    ] {
        require_binding(actual == wanted, name)?;
    }
    require_binding(
        raw_digest(registry_raw_bytes) == expected.reviewer_registry_raw_digest,
        "trusted_reviewer_registry.raw_digest",
    )?;
    require_binding(
        payload.reviewer_registry.raw_digest == raw_digest(registry_raw_bytes),
        "reviewer_registry.raw_digest",
    )?;
    require_binding(
        payload.reviewer_registry.canonical_digest == canonical_digest(registry)?,
        "reviewer_registry.canonical_digest",
    )?;

    let payload_digest = workflow_retirement_payload_digest_v2(payload)?;
    let signatures = &authorization
        .workflow_retirement_authorization_v2
        .signatures;
    let mut values = BTreeSet::new();
    let mut verified = Vec::with_capacity(2);
    for signature in signatures {
        if !values.insert(&signature.signature) {
            return Err(WorkflowRetirementAuthorityErrorV2::DuplicateSignature);
        }
        if signature.payload_digest != payload_digest {
            return Err(WorkflowRetirementAuthorityErrorV2::PayloadDigestMismatch {
                credential_id: signature.credential_id.0.clone(),
            });
        }
        verified.push(verify_signature(
            &registry.workflow_release_reviewer_registry,
            payload,
            signature,
            expected.admission_epoch_unix,
            expected.consumer_observed_until_unix,
            match signature.role {
                WorkflowReleaseReviewerRole::SemanticReviewer => {
                    expected.evidence_reviewer_key_fingerprint
                }
                WorkflowReleaseReviewerRole::ReleaseAuthorizer => {
                    expected.retirement_authorizer_key_fingerprint
                }
            },
        )?);
    }
    let semantic = verified
        .iter()
        .find(|(r, _)| *r == WorkflowReleaseReviewerRole::SemanticReviewer)
        .ok_or(WorkflowRetirementAuthorityErrorV2::MissingRequiredRole {
            role: WorkflowReleaseReviewerRole::SemanticReviewer,
        })?;
    let authorizer = verified
        .iter()
        .find(|(r, _)| *r == WorkflowReleaseReviewerRole::ReleaseAuthorizer)
        .ok_or(WorkflowRetirementAuthorityErrorV2::MissingRequiredRole {
            role: WorkflowReleaseReviewerRole::ReleaseAuthorizer,
        })?;
    require_separation(&semantic.1, &authorizer.1)?;
    Ok(VerifiedWorkflowRetirementAuthorizationV2 {
        authorization_id: payload.authorization_id.0.clone(),
        payload_digest,
        release: payload.release.clone(),
        runtime_bundle: payload.runtime_bundle.clone(),
        legacy_catalog_digest: payload.legacy_catalog_digest.clone(),
        retirement_set_digest: canonical_digest(&payload.retirements)?,
        final_scorecard_digest: payload.final_scorecard.canonical_digest.clone(),
        nonce: payload.nonce.clone(),
        evidence_reviewer: copy_reviewer(semantic),
        retirement_authorizer: copy_reviewer(authorizer),
    })
}

fn validate_shape(
    registry: &WorkflowReleaseReviewerRegistryDocument,
    authorization: &WorkflowRetirementAuthorizationV2Document,
    payload: &WorkflowRetirementAuthorizationV2Payload,
    admission_epoch: u64,
) -> Result<(), WorkflowRetirementAuthorityErrorV2> {
    if registry.schema_version != WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION {
        return invalid("reviewer registry", "unsupported schema version");
    }
    if authorization.schema_version != WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION {
        return invalid("retirement authorization", "unsupported schema version");
    }
    if let Some(issue) = registry.validate().first() {
        return invalid(
            "reviewer registry",
            &format!("{}: {}", issue.path, issue.message),
        );
    }
    if payload.authorization_id.0.trim().is_empty() || payload.nonce.trim().is_empty() {
        return invalid(
            "retirement authorization",
            "authorization id and nonce must be non-blank",
        );
    }
    if payload.issued_at_unix >= payload.expires_at_unix {
        return invalid(
            "retirement authorization",
            "expiry must be later than issuance",
        );
    }
    if admission_epoch < payload.issued_at_unix {
        return Err(WorkflowRetirementAuthorityErrorV2::AuthorizationNotYetValid);
    }
    if admission_epoch >= payload.expires_at_unix {
        return Err(WorkflowRetirementAuthorityErrorV2::AuthorizationExpired);
    }
    if payload.retirements.len() != WORKFLOW_RETIREMENT_AGGREGATE_SIZE {
        return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
            issue: "exactly 42 retirements are required",
        });
    }
    for digest in [
        &payload.release.release_digest,
        &payload.runtime_bundle.bundle_digest,
        &payload.runtime_bundle.policy_set_digest,
        &payload.legacy_catalog_digest,
    ] {
        if !valid_digest(digest) {
            return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
                issue: "invalid release, bundle, policy-set, or catalog digest",
            });
        }
    }
    validate_artifact_bindings(payload)?;
    let mut workflow_ids = BTreeSet::new();
    let mut policy_ids = BTreeSet::new();
    for retirement in &payload.retirements {
        if !workflow_ids.insert(&retirement.workflow_id) {
            return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
                issue: "duplicate workflow id",
            });
        }
        if !policy_ids.insert(&retirement.replacement_policy_ref) {
            return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
                issue: "duplicate replacement policy",
            });
        }
        if !valid_digest(&retirement.legacy_workflow_digest)
            || !valid_digest(&retirement.replacement_policy_digest)
        {
            return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
                issue: "invalid workflow or policy digest",
            });
        }
    }
    if authorization
        .workflow_retirement_authorization_v2
        .signatures
        .len()
        != 2
    {
        return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
            issue: "exactly two signatures are required",
        });
    }
    Ok(())
}

fn validate_artifact_bindings(
    payload: &WorkflowRetirementAuthorizationV2Payload,
) -> Result<(), WorkflowRetirementAuthorityErrorV2> {
    for binding in [
        &payload.release_manifest,
        &payload.runtime_bundle_artifact,
        &payload.snapshot_manifest,
        &payload.runtime_evidence,
        &payload.release_history,
        &payload.evidence_index,
        &payload.deletion_proof,
        &payload.consumer_report,
        &payload.tombstone_catalog,
        &payload.final_scorecard,
        &payload.reviewer_registry,
    ] {
        if binding.artifact_id.0.trim().is_empty()
            || binding.embedded_ref.0.trim().is_empty()
            || !valid_digest(&binding.raw_digest)
            || !valid_digest(&binding.canonical_digest)
        {
            return Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate {
                issue: "invalid evidence artifact binding",
            });
        }
    }
    Ok(())
}

fn verify_signature(
    registry: &WorkflowReleaseReviewerRegistry,
    payload: &WorkflowRetirementAuthorizationV2Payload,
    signature: &WorkflowReleaseAdmissionSignatureV2,
    admission_epoch: u64,
    consumer_observed_until: u64,
    expected_key_fingerprint: &str,
) -> Result<
    (WorkflowReleaseReviewerRole, VerifiedRetirementReviewer),
    WorkflowRetirementAuthorityErrorV2,
> {
    let id = signature.credential_id.0.clone();
    let credential = registry
        .credentials
        .iter()
        .find(|c| c.credential_id == signature.credential_id)
        .ok_or_else(|| WorkflowRetirementAuthorityErrorV2::CredentialNotFound {
            credential_id: id.clone(),
        })?;
    validate_credential(
        credential,
        payload,
        signature,
        admission_epoch,
        consumer_observed_until,
    )?;
    let key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
        WorkflowRetirementAuthorityErrorV2::PublicKeyDecode {
            credential_id: id.clone(),
        }
    })?;
    if credential.public_key_fingerprint != expected_key_fingerprint {
        return Err(
            WorkflowRetirementAuthorityErrorV2::PublicKeyFingerprintMismatch { credential_id: id },
        );
    }
    if workflow_retirement_reviewer_key_fingerprint_v2(&key_bytes)
        != credential.public_key_fingerprint
    {
        return Err(
            WorkflowRetirementAuthorityErrorV2::PublicKeyFingerprintMismatch { credential_id: id },
        );
    }
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| {
        WorkflowRetirementAuthorityErrorV2::PublicKeyDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let signature_bytes = decode_fixed::<64>(&signature.signature).ok_or_else(|| {
        WorkflowRetirementAuthorityErrorV2::SignatureDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    key.verify_strict(
        &workflow_retirement_signing_bytes_v2(payload, signature)?,
        &Signature::from_bytes(&signature_bytes),
    )
    .map_err(|_| WorkflowRetirementAuthorityErrorV2::SignatureInvalid {
        credential_id: signature.credential_id.0.clone(),
    })?;
    Ok((
        signature.role,
        VerifiedRetirementReviewer {
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
    payload: &WorkflowRetirementAuthorizationV2Payload,
    signature: &WorkflowReleaseAdmissionSignatureV2,
    admission_epoch: u64,
    consumer_observed_until: u64,
) -> Result<(), WorkflowRetirementAuthorityErrorV2> {
    let id = signature.credential_id.0.clone();
    if credential.status != WorkflowReleaseReviewerCredentialStatus::Active {
        return Err(WorkflowRetirementAuthorityErrorV2::CredentialNotActive { credential_id: id });
    }
    if credential.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
        || signature.algorithm != WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519
        || !credential.roles.contains(&signature.role)
    {
        return Err(WorkflowRetirementAuthorityErrorV2::CredentialRoleMismatch {
            credential_id: id,
        });
    }
    if credential.principal_id != signature.principal_id {
        return Err(
            WorkflowRetirementAuthorityErrorV2::CredentialPrincipalMismatch { credential_id: id },
        );
    }
    if signature.signed_at_unix < credential.valid_from_unix
        || signature.signed_at_unix > credential.valid_until_unix
        || admission_epoch < credential.valid_from_unix
        || admission_epoch > credential.valid_until_unix
    {
        return Err(
            WorkflowRetirementAuthorityErrorV2::CredentialOutsideValidity { credential_id: id },
        );
    }
    if signature.signed_at_unix < payload.issued_at_unix
        || signature.signed_at_unix >= payload.expires_at_unix
        || signature.signed_at_unix > admission_epoch
        || signature.signed_at_unix < consumer_observed_until
    {
        return Err(
            WorkflowRetirementAuthorityErrorV2::SignatureOutsideValidity { credential_id: id },
        );
    }
    Ok(())
}

fn require_separation(
    a: &VerifiedRetirementReviewer,
    b: &VerifiedRetirementReviewer,
) -> Result<(), WorkflowRetirementAuthorityErrorV2> {
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
                WorkflowRetirementAuthorityErrorV2::ReviewerSeparationViolation { dimension },
            );
        }
    }
    Ok(())
}
fn copy_reviewer(
    pair: &(WorkflowReleaseReviewerRole, VerifiedRetirementReviewer),
) -> VerifiedRetirementReviewer {
    VerifiedRetirementReviewer {
        principal_id: pair.1.principal_id.clone(),
        credential_id: pair.1.credential_id.clone(),
        independence_domain: pair.1.independence_domain.clone(),
        public_key_fingerprint: pair.1.public_key_fingerprint.clone(),
        signature_fingerprint: pair.1.signature_fingerprint.clone(),
        signed_at_unix: pair.1.signed_at_unix,
    }
}
fn require_binding(
    ok: bool,
    field: &'static str,
) -> Result<(), WorkflowRetirementAuthorityErrorV2> {
    if ok {
        Ok(())
    } else {
        Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch { field })
    }
}
fn invalid<T>(
    document: &'static str,
    issue: &str,
) -> Result<T, WorkflowRetirementAuthorityErrorV2> {
    Err(WorkflowRetirementAuthorityErrorV2::InvalidContract {
        document,
        issue: issue.to_owned(),
    })
}
fn canonical_digest<T: Serialize>(value: &T) -> Result<String, WorkflowRetirementAuthorityErrorV2> {
    let value = serde_json::to_value(value)
        .map_err(|e| WorkflowRetirementAuthorityErrorV2::Canonicalization(e.to_string()))?;
    let bytes = serde_json_canonicalizer::to_vec(&value)
        .map_err(|e| WorkflowRetirementAuthorityErrorV2::Canonicalization(e.to_string()))?;
    Ok(raw_digest(&bytes))
}
fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut out = [0; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}
