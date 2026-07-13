//! Cryptographic authority for reviewed Domain Pack learning promotion.
//!
//! Candidate documents remain inert.  This module is the only boundary that
//! resolves reviewer keys, verifies exact graph bindings, and mints move-only
//! capabilities consumable by the monotonic reviewed-registry anchor.

#![allow(clippy::missing_errors_doc)]

use std::collections::BTreeSet;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::domain_pack_learning::{
    DomainPackIndependentReviewDocument, DomainPackLearningConflictDocument,
    DomainPackLocalLearningCandidateDocument, DomainPackPromotionAuthorizationDocument,
    DomainPackPromotionAuthorizationPayload, DomainPackPromotionDecisionDocument,
    DomainPackPromotionDecisionKind, DomainPackPromotionDossier,
    DomainPackPromotionDossierDocument, DomainPackPromotionSignature, DomainPackPromotionStage,
    DomainPackReviewDecision, DomainPackReviewedEligibility, DomainPackReviewedRegistry,
    DomainPackReviewedRegistryDocument, DomainPackReviewedRegistryEntry,
    DomainPackReviewedRegistrySignature, DomainPackReviewerIndependence,
    DomainPackReviewerRegistry, DomainPackReviewerRegistryDocument,
    DomainPackReviewerRegistryEntry, DomainPackReviewerRegistrySignature, DomainPackReviewerRole,
    DomainPackReviewerStatus, DOMAIN_PACK_LEARNING_SCHEMA_VERSION,
};
use forge_core_contracts::domain_pack_learning_conflict_digest;
use forge_core_contracts::{PrincipalId, StableId};
use forge_core_decisions::{
    evaluate_domain_pack_promotion, DomainPackPromotionEvaluationInput,
    DomainPackPromotionReadinessStatus,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Domain separator for promotion signatures. It is intentionally unrelated
/// to workflow release and P6b supply-chain signature domains.
pub const DOMAIN_PACK_PROMOTION_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:domain-pack-learning-promotion:v1\0";
/// Exact value required in every signed promotion payload.
pub const DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN: &str =
    "forge-method:domain-pack-learning-promotion:v1";
/// Domain separator for predecessor-authorized reviewer-registry rotation.
pub const DOMAIN_PACK_REVIEWER_REGISTRY_ROTATION_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:domain-pack-reviewer-registry-rotation:v1\0";
/// Domain separator for a freshly verifiable reviewed-registry snapshot.
pub const DOMAIN_PACK_REVIEWED_REGISTRY_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:domain-pack-reviewed-registry-snapshot:v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackPromotionAuthorityError {
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
    BlockingDecision,
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
    ReviewSignerMismatch {
        review_digest: String,
    },
    MissingSignedReview {
        role: DomainPackReviewerRole,
    },
    DuplicateSignature,
    InvalidReviewerRegistryAnchor {
        message: String,
    },
    ReviewerRegistryCompareAndSwapConflict,
    ReviewerRegistryIdentityMismatch,
    ReviewerRegistryTrustPolicyMismatch,
    ReviewerRegistryGenerationMismatch,
    ReviewerRegistryPredecessorMismatch,
    ReviewerRegistryThresholdNotMet,
    ReviewedRegistryCompareAndSwapConflict,
    ReviewedRegistryIdentityMismatch,
    ReviewedRegistryGenerationMismatch,
    ReviewedRegistryPredecessorMismatch,
    ReviewedRegistryEvolution {
        message: String,
    },
    Canonicalization(String),
}

impl fmt::Display for DomainPackPromotionAuthorityError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { document, issue } => {
                write!(formatter, "invalid {document}: {issue}")
            }
            Self::WrongAudience { expected, found } => {
                write!(formatter, "wrong audience '{found}', expected '{expected}'")
            }
            Self::WrongDomain { found } => write!(formatter, "wrong promotion domain '{found}'"),
            Self::BindingMismatch { field } => {
                write!(formatter, "promotion binding mismatch at '{field}'")
            }
            Self::BlockingDecision => {
                formatter.write_str("promotion contains a blocking decision or review")
            }
            Self::AuthorizationNotYetValid => {
                formatter.write_str("promotion authorization is not yet valid")
            }
            Self::AuthorizationExpired => {
                formatter.write_str("promotion authorization has expired")
            }
            Self::PayloadDigestMismatch { credential_id } => write!(
                formatter,
                "payload digest mismatch for credential '{credential_id}'"
            ),
            Self::CredentialNotFound { credential_id } => {
                write!(formatter, "reviewer credential '{credential_id}' not found")
            }
            Self::CredentialNotActive { credential_id } => write!(
                formatter,
                "reviewer credential '{credential_id}' is not active"
            ),
            Self::CredentialRoleMismatch { credential_id } => write!(
                formatter,
                "reviewer credential '{credential_id}' lacks the signed role or algorithm"
            ),
            Self::CredentialPrincipalMismatch { credential_id } => write!(
                formatter,
                "reviewer credential '{credential_id}' belongs to another principal"
            ),
            Self::CredentialOutsideValidity { credential_id } => write!(
                formatter,
                "reviewer credential '{credential_id}' is outside its validity window"
            ),
            Self::PublicKeyDecode { credential_id } => write!(
                formatter,
                "invalid public key for credential '{credential_id}'"
            ),
            Self::PublicKeyFingerprintMismatch { credential_id } => write!(
                formatter,
                "public key fingerprint mismatch for credential '{credential_id}'"
            ),
            Self::SignatureDecode { credential_id } => write!(
                formatter,
                "invalid signature encoding for credential '{credential_id}'"
            ),
            Self::SignatureInvalid { credential_id } => write!(
                formatter,
                "invalid signature for credential '{credential_id}'"
            ),
            Self::ReviewerSeparationViolation { dimension } => {
                write!(formatter, "reviewer separation violated for {dimension}")
            }
            Self::ReviewSignerMismatch { review_digest } => write!(
                formatter,
                "signed reviewer does not match independent review '{review_digest}'"
            ),
            Self::MissingSignedReview { role } => write!(
                formatter,
                "no approved independent review is signed for role {role:?}"
            ),
            Self::DuplicateSignature => formatter.write_str("duplicate promotion signature"),
            Self::InvalidReviewerRegistryAnchor { message } => {
                write!(formatter, "invalid reviewer registry anchor: {message}")
            }
            Self::ReviewerRegistryCompareAndSwapConflict => {
                formatter.write_str("reviewer registry anchor compare-and-swap conflict")
            }
            Self::ReviewerRegistryIdentityMismatch => {
                formatter.write_str("reviewer registry identity or audience changed")
            }
            Self::ReviewerRegistryTrustPolicyMismatch => {
                formatter.write_str("reviewer registry trust policy changed")
            }
            Self::ReviewerRegistryGenerationMismatch => {
                formatter.write_str("reviewer registry is not the direct successor")
            }
            Self::ReviewerRegistryPredecessorMismatch => {
                formatter.write_str("reviewer registry predecessor digest mismatch")
            }
            Self::ReviewerRegistryThresholdNotMet => formatter.write_str(
                "reviewer registry rotation threshold or independence requirement not met",
            ),
            Self::ReviewedRegistryCompareAndSwapConflict => {
                formatter.write_str("reviewed registry anchor compare-and-swap conflict")
            }
            Self::ReviewedRegistryIdentityMismatch => {
                formatter.write_str("reviewed registry identity or audience changed")
            }
            Self::ReviewedRegistryGenerationMismatch => {
                formatter.write_str("reviewed registry is not the direct successor")
            }
            Self::ReviewedRegistryPredecessorMismatch => {
                formatter.write_str("reviewed registry predecessor digest mismatch")
            }
            Self::ReviewedRegistryEvolution { message } => {
                write!(formatter, "invalid reviewed registry evolution: {message}")
            }
            Self::Canonicalization(message) => {
                write!(formatter, "canonicalization failed: {message}")
            }
        }
    }
}

impl std::error::Error for DomainPackPromotionAuthorityError {}

/// Exact candidate graph selected by the pure evaluator and TCB.
#[derive(Clone, Copy)]
pub struct DomainPackPromotionExpectedContext<'a> {
    pub dossier: &'a DomainPackPromotionDossierDocument,
    pub candidates: &'a [DomainPackLocalLearningCandidateDocument],
    pub decision: &'a DomainPackPromotionDecisionDocument,
    pub independent_reviews: &'a [DomainPackIndependentReviewDocument],
    pub conflicts: &'a [DomainPackLearningConflictDocument],
    pub current_reviewed_registry: &'a DomainPackReviewedRegistryDocument,
    pub proposed_reviewed_registry: &'a DomainPackReviewedRegistryDocument,
    /// Trusted TCB clock. Caller-authored timestamps are not accepted.
    pub verified_at_unix: u64,
}

/// Move-only authority for one exact promotion and one exact registry successor.
pub struct VerifiedDomainPackPromotionAuthorization {
    authorization_id: StableId,
    payload_digest: String,
    dossier_digest: String,
    decision_digest: String,
    reviewer_registry_digest: String,
    current_registry_digest: String,
    proposed_registry_digest: String,
    proposed_registry_full_digest: String,
    authorization_issued_at_unix: u64,
    authorization_expires_at_unix: u64,
    transition_from: DomainPackPromotionStage,
    transition_to: DomainPackPromotionStage,
    proposed_registry: DomainPackReviewedRegistryDocument,
    reviewers: Vec<VerifiedPromotionReviewer>,
}

impl fmt::Debug for VerifiedDomainPackPromotionAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDomainPackPromotionAuthorization")
            .field("authorization_id", &self.authorization_id)
            .field("payload_digest", &self.payload_digest)
            .field("transition_from", &self.transition_from)
            .field("transition_to", &self.transition_to)
            .field("proposed_registry_digest", &self.proposed_registry_digest)
            .finish_non_exhaustive()
    }
}

impl VerifiedDomainPackPromotionAuthorization {
    #[must_use]
    pub const fn authorization_id(&self) -> &StableId {
        &self.authorization_id
    }
    #[must_use]
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
    #[must_use]
    pub fn proposed_registry_digest(&self) -> &str {
        &self.proposed_registry_digest
    }
    #[must_use]
    pub fn audit(&self) -> VerifiedDomainPackPromotionAuthorizationAudit {
        VerifiedDomainPackPromotionAuthorizationAudit {
            authority: DomainPackPromotionAuditAuthority::NonAuthoritative,
            authorization_id: self.authorization_id.clone(),
            payload_digest: self.payload_digest.clone(),
            dossier_digest: self.dossier_digest.clone(),
            decision_digest: self.decision_digest.clone(),
            reviewer_registry_digest: self.reviewer_registry_digest.clone(),
            current_registry_digest: self.current_registry_digest.clone(),
            proposed_registry_digest: self.proposed_registry_digest.clone(),
            authorization_issued_at_unix: self.authorization_issued_at_unix,
            authorization_expires_at_unix: self.authorization_expires_at_unix,
            transition_from: self.transition_from,
            transition_to: self.transition_to,
            reviewers: self
                .reviewers
                .iter()
                .map(VerifiedPromotionReviewer::audit)
                .collect(),
        }
    }
}

#[derive(Debug)]
struct VerifiedPromotionReviewer {
    reviewer_id: PrincipalId,
    credential_id: StableId,
    role: DomainPackReviewerRole,
    independence_domains: Vec<StableId>,
    public_key_fingerprint: String,
    signature_fingerprint: String,
    signed_at_unix: u64,
}

impl VerifiedPromotionReviewer {
    fn audit(&self) -> VerifiedDomainPackPromotionReviewerAudit {
        VerifiedDomainPackPromotionReviewerAudit {
            reviewer_id: self.reviewer_id.clone(),
            credential_id: self.credential_id.clone(),
            role: self.role,
            independence_domains: self.independence_domains.clone(),
            public_key_fingerprint: self.public_key_fingerprint.clone(),
            signature_fingerprint: self.signature_fingerprint.clone(),
            signed_at_unix: self.signed_at_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPromotionAuditAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedDomainPackPromotionReviewerAudit {
    pub reviewer_id: PrincipalId,
    pub credential_id: StableId,
    pub role: DomainPackReviewerRole,
    pub independence_domains: Vec<StableId>,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub signed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedDomainPackPromotionAuthorizationAudit {
    pub authority: DomainPackPromotionAuditAuthority,
    pub authorization_id: StableId,
    pub payload_digest: String,
    pub dossier_digest: String,
    pub decision_digest: String,
    pub reviewer_registry_digest: String,
    pub current_registry_digest: String,
    pub proposed_registry_digest: String,
    pub authorization_issued_at_unix: u64,
    pub authorization_expires_at_unix: u64,
    pub transition_from: DomainPackPromotionStage,
    pub transition_to: DomainPackPromotionStage,
    pub reviewers: Vec<VerifiedDomainPackPromotionReviewerAudit>,
}

/// Operator-protected, predecessor-authorized reviewer-registry head.
///
/// Genesis trust is an explicit operator ceremony. Every later rotation is
/// verified only with credentials from the already anchored predecessor.
pub struct DomainPackReviewerRegistryAnchor {
    registry: DomainPackReviewerRegistryDocument,
    registry_digest: String,
    full_digest: String,
}

impl fmt::Debug for DomainPackReviewerRegistryAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let registry = &self.registry.domain_pack_reviewer_registry;
        formatter
            .debug_struct("DomainPackReviewerRegistryAnchor")
            .field("registry_id", &registry.registry_id)
            .field("audience", &registry.audience)
            .field("generation", &registry.generation)
            .field("registry_digest", &self.registry_digest)
            .finish_non_exhaustive()
    }
}

impl DomainPackReviewerRegistryAnchor {
    /// Establish a genesis head whose trust policy and exact full digest were
    /// approved outside the project tree.
    pub fn from_operator_protected_genesis(
        registry: DomainPackReviewerRegistryDocument,
        expected_trust_policy_digest: &str,
        expected_full_digest: &str,
    ) -> Result<Self, DomainPackPromotionAuthorityError> {
        if registry.domain_pack_reviewer_registry.generation != 0
            || registry
                .domain_pack_reviewer_registry
                .previous_registry_digest
                .is_some()
        {
            return Err(
                DomainPackPromotionAuthorityError::InvalidReviewerRegistryAnchor {
                    message: "operator-protected genesis must be generation zero".to_owned(),
                },
            );
        }
        Self::from_operator_protected_head(
            registry,
            expected_trust_policy_digest,
            expected_full_digest,
        )
    }

    /// Restore any exact generation from an operator-protected monotonic head.
    /// This validates shape and exact digests; the caller owns provenance.
    pub fn from_operator_protected_head(
        registry: DomainPackReviewerRegistryDocument,
        expected_trust_policy_digest: &str,
        expected_full_digest: &str,
    ) -> Result<Self, DomainPackPromotionAuthorityError> {
        validate_document("reviewer registry", registry.validate())?;
        let value = &registry.domain_pack_reviewer_registry;
        if value.trust_policy_digest != expected_trust_policy_digest {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryTrustPolicyMismatch);
        }
        let registry_digest = reviewer_registry_subject_digest(&registry)?;
        require_binding(
            value.registry_digest == registry_digest,
            "reviewer_registry.registry_digest",
        )?;
        let full_digest = canonical_digest(&registry)?;
        require_binding(
            full_digest == expected_full_digest,
            "operator_protected_reviewer_registry.full_digest",
        )?;
        Ok(Self {
            registry,
            registry_digest,
            full_digest,
        })
    }

    #[must_use]
    pub fn version(&self) -> DomainPackReviewerRegistryAnchorVersion {
        let value = &self.registry.domain_pack_reviewer_registry;
        DomainPackReviewerRegistryAnchorVersion {
            registry_id: value.registry_id.clone(),
            audience: value.audience.clone(),
            generation: value.generation,
            registry_digest: self.registry_digest.clone(),
            full_digest: self.full_digest.clone(),
            trust_policy_digest: value.trust_policy_digest.clone(),
        }
    }

    /// Verify and atomically advance one direct reviewer-registry successor.
    pub fn compare_and_advance(
        &mut self,
        expected: &DomainPackReviewerRegistryAnchorVersion,
        candidate: DomainPackReviewerRegistryDocument,
        verified_at_unix: u64,
    ) -> Result<DomainPackReviewerRegistryAdvanceAudit, DomainPackPromotionAuthorityError> {
        if expected != &self.version() {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryCompareAndSwapConflict);
        }
        validate_document("reviewer registry", candidate.validate())?;
        let current = &self.registry.domain_pack_reviewer_registry;
        let next = &candidate.domain_pack_reviewer_registry;
        if next.registry_id != current.registry_id || next.audience != current.audience {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryIdentityMismatch);
        }
        if next.trust_policy_digest != current.trust_policy_digest {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryTrustPolicyMismatch);
        }
        if next.signature_threshold < current.signature_threshold {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet);
        }
        if next.generation
            != current
                .generation
                .checked_add(1)
                .ok_or(DomainPackPromotionAuthorityError::ReviewerRegistryGenerationMismatch)?
        {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryGenerationMismatch);
        }
        if next.previous_registry_digest.as_deref() != Some(self.registry_digest.as_str()) {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryPredecessorMismatch);
        }
        let candidate_digest = reviewer_registry_subject_digest(&candidate)?;
        require_binding(
            next.registry_digest == candidate_digest,
            "reviewer_registry.registry_digest",
        )?;
        verify_reviewer_registry_rotation(current, next, &candidate_digest, verified_at_unix)?;
        let previous_digest = self.registry_digest.clone();
        self.full_digest = canonical_digest(&candidate)?;
        self.registry_digest.clone_from(&candidate_digest);
        self.registry = candidate;
        Ok(DomainPackReviewerRegistryAdvanceAudit {
            authority: DomainPackPromotionAuditAuthority::NonAuthoritative,
            previous_registry_digest: previous_digest,
            registry_digest: candidate_digest,
            generation: next_generation(self.registry.domain_pack_reviewer_registry.generation),
        })
    }

    fn registry(&self) -> &DomainPackReviewerRegistryDocument {
        &self.registry
    }
}

const fn next_generation(value: u64) -> u64 {
    value
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackReviewerRegistryAnchorVersion {
    registry_id: StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
    full_digest: String,
    trust_policy_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReviewerRegistryAdvanceAudit {
    pub authority: DomainPackPromotionAuditAuthority,
    pub previous_registry_digest: String,
    pub registry_digest: String,
    pub generation: u64,
}

/// Monotonic anchor for semantically reviewed registry snapshots.
pub struct ReviewedDomainPackRegistryAnchor {
    registry: DomainPackReviewedRegistryDocument,
    registry_digest: String,
}

impl fmt::Debug for ReviewedDomainPackRegistryAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let registry = &self.registry.domain_pack_reviewed_registry;
        formatter
            .debug_struct("ReviewedDomainPackRegistryAnchor")
            .field("registry_id", &registry.registry_id)
            .field("audience", &registry.audience)
            .field("generation", &registry.generation)
            .field("registry_digest", &self.registry_digest)
            .finish_non_exhaustive()
    }
}

impl ReviewedDomainPackRegistryAnchor {
    /// Restore an exact protected head. Generation zero is the explicit empty
    /// genesis used by the first reviewed promotion.
    pub fn from_operator_protected_head(
        reviewer_anchor: &DomainPackReviewerRegistryAnchor,
        registry: DomainPackReviewedRegistryDocument,
        expected_registry_digest: &str,
        verified_at_unix: u64,
    ) -> Result<Self, DomainPackPromotionAuthorityError> {
        validate_document("reviewed registry", registry.validate())?;
        let registry_digest = reviewed_registry_subject_digest(&registry)?;
        require_binding(
            registry.domain_pack_reviewed_registry.registry_digest == registry_digest,
            "reviewed_registry.registry_digest",
        )?;
        require_binding(
            registry_digest == expected_registry_digest,
            "operator_protected_reviewed_registry.registry_digest",
        )?;
        verify_reviewed_registry_snapshot_signatures(
            reviewer_anchor.registry(),
            &registry,
            verified_at_unix,
        )?;
        Ok(Self {
            registry,
            registry_digest,
        })
    }

    #[must_use]
    pub fn version(&self) -> ReviewedDomainPackRegistryAnchorVersion {
        let value = &self.registry.domain_pack_reviewed_registry;
        ReviewedDomainPackRegistryAnchorVersion {
            registry_id: value.registry_id.clone(),
            audience: value.audience.clone(),
            generation: value.generation,
            registry_digest: self.registry_digest.clone(),
        }
    }

    /// Freshly reverify the exact anchored semantic snapshot. Signature bytes
    /// may be refreshed without changing the registry subject digest.
    pub fn verify_exact_replay(
        &mut self,
        reviewer_anchor: &DomainPackReviewerRegistryAnchor,
        candidate: DomainPackReviewedRegistryDocument,
        verified_at_unix: u64,
    ) -> Result<AnchoredReviewedDomainPackRegistrySnapshot, DomainPackPromotionAuthorityError> {
        validate_document("reviewed registry", candidate.validate())?;
        let candidate_digest = reviewed_registry_subject_digest(&candidate)?;
        if candidate_digest != self.registry_digest
            || candidate.domain_pack_reviewed_registry.registry_id
                != self.registry.domain_pack_reviewed_registry.registry_id
            || candidate.domain_pack_reviewed_registry.audience
                != self.registry.domain_pack_reviewed_registry.audience
            || candidate.domain_pack_reviewed_registry.generation
                != self.registry.domain_pack_reviewed_registry.generation
        {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryCompareAndSwapConflict);
        }
        verify_reviewed_registry_snapshot_signatures(
            reviewer_anchor.registry(),
            &candidate,
            verified_at_unix,
        )?;
        self.registry = candidate;
        Ok(AnchoredReviewedDomainPackRegistrySnapshot {
            registry: self.registry.clone(),
            registry_digest: self.registry_digest.clone(),
            reviewer_registry_digest: reviewer_anchor.registry_digest.clone(),
            authorization_audit: None,
        })
    }

    /// Consume one exact promotion capability and advance only a direct,
    /// append-preserving successor. Terminal entries can never be rewritten.
    #[allow(clippy::needless_pass_by_value)]
    pub fn compare_and_advance(
        &mut self,
        expected: &ReviewedDomainPackRegistryAnchorVersion,
        reviewer_anchor: &DomainPackReviewerRegistryAnchor,
        capability: VerifiedDomainPackPromotionAuthorization,
        verified_at_unix: u64,
    ) -> Result<AnchoredReviewedDomainPackRegistrySnapshot, DomainPackPromotionAuthorityError> {
        if expected != &self.version() {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryCompareAndSwapConflict);
        }
        require_binding(
            capability.reviewer_registry_digest == reviewer_anchor.registry_digest,
            "capability.reviewer_registry_digest",
        )?;
        if verified_at_unix < capability.authorization_issued_at_unix {
            return Err(DomainPackPromotionAuthorityError::AuthorizationNotYetValid);
        }
        if verified_at_unix > capability.authorization_expires_at_unix {
            return Err(DomainPackPromotionAuthorityError::AuthorizationExpired);
        }
        if capability.current_registry_digest != self.registry_digest {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryPredecessorMismatch);
        }
        let proposed = &capability.proposed_registry;
        let current = &self.registry.domain_pack_reviewed_registry;
        let next = &proposed.domain_pack_reviewed_registry;
        if next.registry_id != current.registry_id || next.audience != current.audience {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryIdentityMismatch);
        }
        if next.generation
            != current
                .generation
                .checked_add(1)
                .ok_or(DomainPackPromotionAuthorityError::ReviewedRegistryGenerationMismatch)?
        {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryGenerationMismatch);
        }
        if next.previous_registry_digest.as_deref() != Some(self.registry_digest.as_str()) {
            return Err(DomainPackPromotionAuthorityError::ReviewedRegistryPredecessorMismatch);
        }
        validate_reviewed_registry_evolution(
            current,
            next,
            capability.transition_from,
            capability.transition_to,
        )?;
        let proposed_digest = reviewed_registry_subject_digest(proposed)?;
        let proposed_full_digest = canonical_digest(proposed)?;
        if proposed_digest != capability.proposed_registry_digest
            || proposed_full_digest != capability.proposed_registry_full_digest
        {
            return Err(
                DomainPackPromotionAuthorityError::ReviewedRegistryEvolution {
                    message: "capability does not carry the exact proposed registry".to_owned(),
                },
            );
        }
        verify_reviewed_registry_snapshot_signatures(
            reviewer_anchor.registry(),
            proposed,
            verified_at_unix,
        )?;
        self.registry = proposed.clone();
        self.registry_digest = proposed_digest;
        Ok(AnchoredReviewedDomainPackRegistrySnapshot {
            registry: self.registry.clone(),
            registry_digest: self.registry_digest.clone(),
            reviewer_registry_digest: reviewer_anchor.registry_digest.clone(),
            authorization_audit: Some(capability.audit()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedDomainPackRegistryAnchorVersion {
    registry_id: StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
}

/// Opaque reviewed-registry authority. It deliberately has no serde or Clone.
pub struct AnchoredReviewedDomainPackRegistrySnapshot {
    registry: DomainPackReviewedRegistryDocument,
    registry_digest: String,
    reviewer_registry_digest: String,
    authorization_audit: Option<VerifiedDomainPackPromotionAuthorizationAudit>,
}

impl fmt::Debug for AnchoredReviewedDomainPackRegistrySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnchoredReviewedDomainPackRegistrySnapshot")
            .field("registry_digest", &self.registry_digest)
            .field(
                "generation",
                &self.registry.domain_pack_reviewed_registry.generation,
            )
            .finish_non_exhaustive()
    }
}

impl AnchoredReviewedDomainPackRegistrySnapshot {
    #[must_use]
    pub const fn registry(&self) -> &DomainPackReviewedRegistryDocument {
        &self.registry
    }
    #[must_use]
    pub fn registry_digest(&self) -> &str {
        &self.registry_digest
    }
    #[must_use]
    pub fn reviewer_registry_digest(&self) -> &str {
        &self.reviewer_registry_digest
    }
    #[must_use]
    pub const fn authorization_audit(
        &self,
    ) -> Option<&VerifiedDomainPackPromotionAuthorizationAudit> {
        self.authorization_audit.as_ref()
    }
}

/// Domain-separated bytes signed by one promotion reviewer.
pub fn domain_pack_promotion_signing_bytes(
    payload: &DomainPackPromotionAuthorizationPayload,
    signature: &DomainPackPromotionSignature,
) -> Result<Vec<u8>, DomainPackPromotionAuthorityError> {
    #[derive(Serialize)]
    struct Envelope<'a> {
        payload: &'a DomainPackPromotionAuthorizationPayload,
        reviewer_id: &'a PrincipalId,
        credential_id: &'a StableId,
        role: DomainPackReviewerRole,
        signed_at_unix: u64,
    }
    domain_separated(
        DOMAIN_PACK_PROMOTION_SIGNATURE_DOMAIN,
        &Envelope {
            payload,
            reviewer_id: &signature.reviewer_id,
            credential_id: &signature.credential_id,
            role: signature.role,
            signed_at_unix: signature.signed_at_unix,
        },
    )
}

/// Canonical identity of the signed promotion payload.
pub fn domain_pack_promotion_payload_digest(
    payload: &DomainPackPromotionAuthorizationPayload,
) -> Result<String, DomainPackPromotionAuthorityError> {
    canonical_digest(payload)
}

/// Domain-separated bytes signed by one reviewed-registry snapshot signer.
pub fn domain_pack_reviewed_registry_signing_bytes(
    registry: &DomainPackReviewedRegistryDocument,
    signature: &DomainPackReviewedRegistrySignature,
) -> Result<Vec<u8>, DomainPackPromotionAuthorityError> {
    #[derive(Serialize)]
    struct Envelope<'a> {
        registry_subject_digest: String,
        reviewer_id: &'a PrincipalId,
        credential_id: &'a StableId,
        role: DomainPackReviewerRole,
        signed_at_unix: u64,
    }
    domain_separated(
        DOMAIN_PACK_REVIEWED_REGISTRY_SIGNATURE_DOMAIN,
        &Envelope {
            registry_subject_digest: reviewed_registry_subject_digest(registry)?,
            reviewer_id: &signature.reviewer_id,
            credential_id: &signature.credential_id,
            role: signature.role,
            signed_at_unix: signature.signed_at_unix,
        },
    )
}

/// Canonical bytes signed for reviewer-registry rotation.
pub fn domain_pack_reviewer_registry_rotation_signing_bytes(
    registry: &DomainPackReviewerRegistryDocument,
    signature: &DomainPackReviewerRegistrySignature,
) -> Result<Vec<u8>, DomainPackPromotionAuthorityError> {
    #[derive(Serialize)]
    struct Envelope<'a> {
        registry_subject_digest: String,
        predecessor_registry_digest: &'a Option<String>,
        signer_id: &'a PrincipalId,
        credential_id: &'a StableId,
        signed_at_unix: u64,
    }
    domain_separated(
        DOMAIN_PACK_REVIEWER_REGISTRY_ROTATION_SIGNATURE_DOMAIN,
        &Envelope {
            registry_subject_digest: reviewer_registry_subject_digest(registry)?,
            predecessor_registry_digest: &signature.predecessor_registry_digest,
            signer_id: &signature.signer_id,
            credential_id: &signature.credential_id,
            signed_at_unix: signature.signed_at_unix,
        },
    )
}

#[must_use]
pub fn domain_pack_promotion_reviewer_key_fingerprint(public_key: &[u8; 32]) -> String {
    raw_digest(public_key)
}

/// Verify an exact promotion graph and mint move-only authority.
#[allow(clippy::too_many_lines)]
pub fn verify_domain_pack_promotion_authorization(
    reviewer_anchor: &DomainPackReviewerRegistryAnchor,
    authorization: &DomainPackPromotionAuthorizationDocument,
    expected: DomainPackPromotionExpectedContext<'_>,
    expected_audience: &str,
) -> Result<VerifiedDomainPackPromotionAuthorization, DomainPackPromotionAuthorityError> {
    validate_document("promotion dossier", expected.dossier.validate())?;
    validate_document("promotion decision", expected.decision.validate())?;
    validate_document("promotion authorization", authorization.validate())?;
    validate_document(
        "current reviewed registry",
        expected.current_reviewed_registry.validate(),
    )?;
    validate_document(
        "proposed reviewed registry",
        expected.proposed_reviewed_registry.validate(),
    )?;
    for review in expected.independent_reviews {
        validate_document("independent review", review.validate())?;
    }
    for candidate in expected.candidates {
        validate_document("local learning candidate", candidate.validate())?;
        require_binding(
            candidate
                .domain_pack_local_learning_candidate
                .candidate_digest
                == domain_pack_local_learning_candidate_digest(candidate)?,
            "candidate.candidate_digest",
        )?;
    }
    for conflict in expected.conflicts {
        validate_document("learning conflict", conflict.validate())?;
        let conflict_digest = domain_pack_learning_conflict_digest(conflict)
            .map_err(DomainPackPromotionAuthorityError::Canonicalization)?;
        require_binding(
            conflict.domain_pack_learning_conflict.conflict_digest == conflict_digest,
            "conflict.conflict_digest",
        )?;
    }
    let payload = &authorization.domain_pack_promotion_authorization.payload;
    if payload.audience != expected_audience {
        return Err(DomainPackPromotionAuthorityError::WrongAudience {
            expected: expected_audience.to_owned(),
            found: payload.audience.clone(),
        });
    }
    if payload.domain != DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN {
        return Err(DomainPackPromotionAuthorityError::WrongDomain {
            found: payload.domain.clone(),
        });
    }
    if expected.verified_at_unix < payload.issued_at_unix {
        return Err(DomainPackPromotionAuthorityError::AuthorizationNotYetValid);
    }
    if expected.verified_at_unix > payload.expires_at_unix {
        return Err(DomainPackPromotionAuthorityError::AuthorizationExpired);
    }

    let dossier_digest = domain_pack_promotion_dossier_digest(expected.dossier)?;
    let decision_digest = domain_pack_promotion_decision_digest(expected.decision)?;
    require_binding(
        expected
            .dossier
            .domain_pack_promotion_dossier
            .dossier_digest
            == dossier_digest,
        "dossier.dossier_digest",
    )?;
    require_binding(
        expected
            .decision
            .domain_pack_promotion_decision
            .decision_digest
            == decision_digest,
        "decision.decision_digest",
    )?;
    let dossier = &expected.dossier.domain_pack_promotion_dossier;
    let decision = &expected.decision.domain_pack_promotion_decision;
    if decision.decision != DomainPackPromotionDecisionKind::Approve {
        return Err(DomainPackPromotionAuthorityError::BlockingDecision);
    }
    require_binding(
        decision.dossier_digest == dossier_digest,
        "decision.dossier_digest",
    )?;
    require_binding(
        decision.transition == dossier.transition,
        "decision.transition",
    )?;
    require_exact_digest_graph(
        &dossier.candidate_digests,
        expected.candidates.iter().map(|document| {
            document
                .domain_pack_local_learning_candidate
                .candidate_digest
                .as_str()
        }),
        "dossier.candidate_digests",
    )?;
    require_exact_digest_graph(
        &dossier.conflict_record_digests,
        expected.conflicts.iter().map(|document| {
            document
                .domain_pack_learning_conflict
                .conflict_digest
                .as_str()
        }),
        "dossier.conflict_record_digests",
    )?;
    require_exact_digest_graph(
        &dossier.conflict_record_digests,
        decision
            .resolved_conflict_digests
            .iter()
            .map(String::as_str),
        "decision.resolved_conflict_digests",
    )?;
    require_binding(
        payload.dossier_digest == dossier_digest,
        "payload.dossier_digest",
    )?;
    require_binding(
        payload.decision_digest == decision_digest,
        "payload.decision_digest",
    )?;
    require_binding(
        payload.transition == dossier.transition,
        "payload.transition",
    )?;

    let reviewer_registry_digest = reviewer_anchor.registry_digest.clone();
    require_binding(
        payload.reviewer_registry_digest == reviewer_registry_digest,
        "payload.reviewer_registry_digest",
    )?;
    let current_registry_digest =
        reviewed_registry_subject_digest(expected.current_reviewed_registry)?;
    let proposed_registry_digest =
        reviewed_registry_subject_digest(expected.proposed_reviewed_registry)?;
    let proposed_registry_proposal_digest =
        domain_pack_reviewed_registry_proposal_digest(expected.proposed_reviewed_registry)?;
    require_binding(
        expected
            .current_reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest
            == current_registry_digest,
        "current_reviewed_registry.registry_digest",
    )?;
    require_binding(
        expected
            .proposed_reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest
            == proposed_registry_digest,
        "proposed_reviewed_registry.registry_digest",
    )?;
    require_binding(
        payload.current_reviewed_registry_digest == current_registry_digest,
        "payload.current_reviewed_registry_digest",
    )?;
    require_binding(
        payload.proposed_reviewed_registry_digest == proposed_registry_proposal_digest,
        "payload.proposed_reviewed_registry_digest",
    )?;
    require_binding(
        decision.registry_predecessor_digest == current_registry_digest,
        "decision.registry_predecessor_digest",
    )?;
    require_binding(
        decision.proposed_registry_digest == proposed_registry_proposal_digest,
        "decision.proposed_registry_digest",
    )?;

    let mut review_digests = Vec::with_capacity(expected.independent_reviews.len());
    for review in expected.independent_reviews {
        let digest = domain_pack_independent_review_digest(review)?;
        let value = &review.domain_pack_independent_review;
        require_binding(
            value.review_digest == digest,
            "independent_review.review_digest",
        )?;
        require_binding(
            value.dossier_digest == dossier_digest,
            "independent_review.dossier_digest",
        )?;
        require_binding(
            value.signed_subject_digest == dossier_digest,
            "independent_review.signed_subject_digest",
        )?;
        require_binding(
            value.reviewer_registry_digest == reviewer_registry_digest,
            "independent_review.reviewer_registry_digest",
        )?;
        if value.decision != DomainPackReviewDecision::Approve
            || !matches!(value.independence, DomainPackReviewerIndependence::Independent { .. })
            || value.findings.iter().any(|finding| {
                matches!(finding.severity, forge_core_contracts::domain_pack_learning::DomainPackReviewFindingSeverity::Blocking)
                    && !matches!(finding.disposition, forge_core_contracts::domain_pack_learning::DomainPackReviewFindingDisposition::Resolved)
            })
        {
            return Err(DomainPackPromotionAuthorityError::BlockingDecision);
        }
        if expected.verified_at_unix < value.issued_at_unix
            || expected.verified_at_unix > value.expires_at_unix
        {
            return Err(DomainPackPromotionAuthorityError::AuthorizationExpired);
        }
        review_digests.push(digest);
    }
    review_digests.sort();
    let mut payload_reviews = payload.independent_review_digests.clone();
    payload_reviews.sort();
    let mut decision_reviews = decision.independent_review_digests.clone();
    decision_reviews.sort();
    require_binding(
        payload_reviews == review_digests,
        "payload.independent_review_digests",
    )?;
    require_binding(
        decision_reviews == review_digests,
        "decision.independent_review_digests",
    )?;

    let payload_digest = domain_pack_promotion_payload_digest(payload)?;
    let registry = &reviewer_anchor.registry().domain_pack_reviewer_registry;
    let mut reviewers = Vec::with_capacity(
        authorization
            .domain_pack_promotion_authorization
            .signatures
            .len(),
    );
    let mut signatures = BTreeSet::new();
    for signature in &authorization.domain_pack_promotion_authorization.signatures {
        if !signatures.insert(&signature.signature) {
            return Err(DomainPackPromotionAuthorityError::DuplicateSignature);
        }
        if signature.payload_digest != payload_digest {
            return Err(DomainPackPromotionAuthorityError::PayloadDigestMismatch {
                credential_id: signature.credential_id.0.clone(),
            });
        }
        reviewers.push(verify_promotion_signature(
            registry,
            payload,
            signature,
            expected.verified_at_unix,
        )?);
    }
    require_promotion_reviewer_separation(&reviewers)?;
    require_reviews_signed(&reviewers, expected.independent_reviews)?;
    reject_participant_overlap(&reviewers, dossier)?;

    let evaluation = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: expected.dossier,
        candidates: expected.candidates,
        independent_reviews: expected.independent_reviews,
        conflicts: expected.conflicts,
    });
    if evaluation.status != DomainPackPromotionReadinessStatus::ReadyForTrustedReview
        || !evaluation.issues.is_empty()
    {
        return Err(DomainPackPromotionAuthorityError::BlockingDecision);
    }

    validate_reviewed_registry_evolution(
        &expected
            .current_reviewed_registry
            .domain_pack_reviewed_registry,
        &expected
            .proposed_reviewed_registry
            .domain_pack_reviewed_registry,
        dossier.transition.from,
        dossier.transition.to,
    )?;
    require_dossier_registry_binding(
        &expected
            .current_reviewed_registry
            .domain_pack_reviewed_registry,
        &expected
            .proposed_reviewed_registry
            .domain_pack_reviewed_registry,
        dossier,
        &decision_digest,
        &payload_digest,
        &review_digests,
    )?;
    Ok(VerifiedDomainPackPromotionAuthorization {
        authorization_id: payload.authorization_id.clone(),
        payload_digest,
        dossier_digest,
        decision_digest,
        reviewer_registry_digest,
        current_registry_digest,
        proposed_registry_digest,
        proposed_registry_full_digest: canonical_digest(expected.proposed_reviewed_registry)?,
        authorization_issued_at_unix: payload.issued_at_unix,
        authorization_expires_at_unix: payload.expires_at_unix,
        transition_from: dossier.transition.from,
        transition_to: dossier.transition.to,
        proposed_registry: expected.proposed_reviewed_registry.clone(),
        reviewers,
    })
}

fn verify_promotion_signature(
    registry: &DomainPackReviewerRegistry,
    payload: &DomainPackPromotionAuthorizationPayload,
    signature: &DomainPackPromotionSignature,
    verified_at_unix: u64,
) -> Result<VerifiedPromotionReviewer, DomainPackPromotionAuthorityError> {
    let credential_id = signature.credential_id.0.clone();
    let credential = registry
        .reviewers
        .iter()
        .find(|entry| entry.credential_id == signature.credential_id)
        .ok_or_else(|| DomainPackPromotionAuthorityError::CredentialNotFound {
            credential_id: credential_id.clone(),
        })?;
    validate_credential(
        credential,
        &signature.reviewer_id,
        signature.role,
        signature.signed_at_unix,
        verified_at_unix,
    )?;
    let key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
        DomainPackPromotionAuthorityError::PublicKeyDecode {
            credential_id: credential_id.clone(),
        }
    })?;
    if domain_pack_promotion_reviewer_key_fingerprint(&key_bytes)
        != credential.public_key_fingerprint
    {
        return Err(
            DomainPackPromotionAuthorityError::PublicKeyFingerprintMismatch { credential_id },
        );
    }
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| {
        DomainPackPromotionAuthorityError::PublicKeyDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    let signature_bytes = decode_fixed::<64>(&signature.signature).ok_or_else(|| {
        DomainPackPromotionAuthorityError::SignatureDecode {
            credential_id: signature.credential_id.0.clone(),
        }
    })?;
    key.verify_strict(
        &domain_pack_promotion_signing_bytes(payload, signature)?,
        &Signature::from_bytes(&signature_bytes),
    )
    .map_err(|_| DomainPackPromotionAuthorityError::SignatureInvalid {
        credential_id: signature.credential_id.0.clone(),
    })?;
    Ok(VerifiedPromotionReviewer {
        reviewer_id: signature.reviewer_id.clone(),
        credential_id: signature.credential_id.clone(),
        role: signature.role,
        independence_domains: credential.independence_domains.clone(),
        public_key_fingerprint: credential.public_key_fingerprint.clone(),
        signature_fingerprint: raw_digest(&signature_bytes),
        signed_at_unix: signature.signed_at_unix,
    })
}

fn validate_credential(
    credential: &DomainPackReviewerRegistryEntry,
    reviewer_id: &PrincipalId,
    role: DomainPackReviewerRole,
    signed_at_unix: u64,
    verified_at_unix: u64,
) -> Result<(), DomainPackPromotionAuthorityError> {
    let credential_id = credential.credential_id.0.clone();
    if credential.status != DomainPackReviewerStatus::Active {
        return Err(DomainPackPromotionAuthorityError::CredentialNotActive { credential_id });
    }
    if credential.algorithm != forge_core_contracts::domain_pack_learning::DomainPackPromotionSignatureAlgorithm::Ed25519
        || !credential.roles.contains(&role)
    {
        return Err(DomainPackPromotionAuthorityError::CredentialRoleMismatch { credential_id });
    }
    if &credential.reviewer_id != reviewer_id {
        return Err(
            DomainPackPromotionAuthorityError::CredentialPrincipalMismatch { credential_id },
        );
    }
    if signed_at_unix < credential.valid_from_unix
        || signed_at_unix > credential.valid_until_unix
        || verified_at_unix < credential.valid_from_unix
        || verified_at_unix > credential.valid_until_unix
    {
        return Err(DomainPackPromotionAuthorityError::CredentialOutsideValidity { credential_id });
    }
    Ok(())
}

fn require_promotion_reviewer_separation(
    reviewers: &[VerifiedPromotionReviewer],
) -> Result<(), DomainPackPromotionAuthorityError> {
    // Promotion is deliberately an exact two-role boundary, not an arbitrary
    // threshold assembled from many correlated signatures.
    if reviewers.len() != 2 {
        return Err(
            DomainPackPromotionAuthorityError::ReviewerSeparationViolation {
                dimension: "exact two reviewers",
            },
        );
    }
    let a = &reviewers[0];
    let b = &reviewers[1];
    let roles = [a.role, b.role].into_iter().collect::<BTreeSet<_>>();
    if !roles.contains(&DomainPackReviewerRole::RegistryAuthorizer)
        || !roles.iter().any(|role| is_semantic_reviewer_role(*role))
    {
        return Err(
            DomainPackPromotionAuthorityError::ReviewerSeparationViolation {
                dimension: "semantic reviewer and registry authorizer roles",
            },
        );
    }
    for (different, dimension) in [
        (a.reviewer_id != b.reviewer_id, "principal"),
        (a.credential_id != b.credential_id, "credential"),
        (a.role != b.role, "role"),
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
                DomainPackPromotionAuthorityError::ReviewerSeparationViolation { dimension },
            );
        }
    }
    if a.independence_domains
        .iter()
        .any(|domain| b.independence_domains.contains(domain))
    {
        return Err(
            DomainPackPromotionAuthorityError::ReviewerSeparationViolation {
                dimension: "independence domain",
            },
        );
    }
    Ok(())
}

const fn is_semantic_reviewer_role(role: DomainPackReviewerRole) -> bool {
    matches!(
        role,
        DomainPackReviewerRole::DomainExpert
            | DomainPackReviewerRole::EvidenceReviewer
            | DomainPackReviewerRole::SafetyReviewer
            | DomainPackReviewerRole::CompatibilityReviewer
    )
}

fn require_reviews_signed(
    reviewers: &[VerifiedPromotionReviewer],
    reviews: &[DomainPackIndependentReviewDocument],
) -> Result<(), DomainPackPromotionAuthorityError> {
    for reviewer in reviewers {
        let matched = reviews.iter().any(|document| {
            let review = &document.domain_pack_independent_review;
            review.reviewer_id == reviewer.reviewer_id
                && review.credential_id == reviewer.credential_id
                && review.reviewer_role == reviewer.role
        });
        if !matched {
            return Err(DomainPackPromotionAuthorityError::MissingSignedReview {
                role: reviewer.role,
            });
        }
    }
    for document in reviews {
        let review = &document.domain_pack_independent_review;
        if !reviewers.iter().any(|reviewer| {
            reviewer.reviewer_id == review.reviewer_id
                && reviewer.credential_id == review.credential_id
                && reviewer.role == review.reviewer_role
        }) {
            return Err(DomainPackPromotionAuthorityError::ReviewSignerMismatch {
                review_digest: review.review_digest.clone(),
            });
        }
    }
    Ok(())
}

fn reject_participant_overlap(
    reviewers: &[VerifiedPromotionReviewer],
    dossier: &forge_core_contracts::domain_pack_learning::DomainPackPromotionDossier,
) -> Result<(), DomainPackPromotionAuthorityError> {
    for reviewer in reviewers {
        let evaluator_overlap = dossier
            .evaluator_runs
            .iter()
            .any(|run| run.evaluator_principal == reviewer.reviewer_id);
        let fixture_overlap = dossier
            .fixture_bindings
            .iter()
            .any(|fixture| fixture.producer == reviewer.reviewer_id);
        let judge_overlap = dossier.evaluator_runs.iter().any(|run| {
            run.strong_judge_proof
                .as_ref()
                .is_some_and(|proof| proof.judge_principal == reviewer.reviewer_id)
        });
        if dossier
            .provenance
            .authored_by
            .contains(&reviewer.reviewer_id)
            || evaluator_overlap
            || fixture_overlap
            || judge_overlap
        {
            return Err(
                DomainPackPromotionAuthorityError::ReviewerSeparationViolation {
                    dimension: "author/evaluator/fixture-producer/judge/reviewer identity",
                },
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_reviewer_registry_rotation(
    predecessor: &DomainPackReviewerRegistry,
    candidate: &DomainPackReviewerRegistry,
    candidate_digest: &str,
    verified_at_unix: u64,
) -> Result<(), DomainPackPromotionAuthorityError> {
    let candidate_document = DomainPackReviewerRegistryDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_reviewer_registry: candidate.clone(),
    };
    let mut principals = BTreeSet::new();
    let mut credentials = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut signatures = BTreeSet::new();
    let mut domains = BTreeSet::new();
    let mut registry_authorizer_principals = BTreeSet::new();
    let mut semantic_reviewer_principals = BTreeSet::new();
    let mut verified = 0usize;
    for signed in &candidate.rotation_signatures {
        if signed.payload_digest != candidate_digest {
            return Err(DomainPackPromotionAuthorityError::PayloadDigestMismatch {
                credential_id: signed.credential_id.0.clone(),
            });
        }
        let credential = predecessor
            .reviewers
            .iter()
            .find(|entry| entry.credential_id == signed.credential_id)
            .ok_or_else(|| DomainPackPromotionAuthorityError::CredentialNotFound {
                credential_id: signed.credential_id.0.clone(),
            })?;
        if credential.reviewer_id != signed.signer_id {
            return Err(
                DomainPackPromotionAuthorityError::CredentialPrincipalMismatch {
                    credential_id: signed.credential_id.0.clone(),
                },
            );
        }
        if credential.status != DomainPackReviewerStatus::Active {
            return Err(DomainPackPromotionAuthorityError::CredentialNotActive {
                credential_id: signed.credential_id.0.clone(),
            });
        }
        if signed.signed_at_unix < credential.valid_from_unix
            || signed.signed_at_unix > credential.valid_until_unix
            || verified_at_unix < credential.valid_from_unix
            || verified_at_unix > credential.valid_until_unix
        {
            return Err(
                DomainPackPromotionAuthorityError::CredentialOutsideValidity {
                    credential_id: signed.credential_id.0.clone(),
                },
            );
        }
        let key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
            DomainPackPromotionAuthorityError::PublicKeyDecode {
                credential_id: signed.credential_id.0.clone(),
            }
        })?;
        if domain_pack_promotion_reviewer_key_fingerprint(&key_bytes)
            != credential.public_key_fingerprint
        {
            return Err(
                DomainPackPromotionAuthorityError::PublicKeyFingerprintMismatch {
                    credential_id: signed.credential_id.0.clone(),
                },
            );
        }
        let detached_bytes = decode_fixed::<64>(&signed.signature).ok_or_else(|| {
            DomainPackPromotionAuthorityError::SignatureDecode {
                credential_id: signed.credential_id.0.clone(),
            }
        })?;
        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| DomainPackPromotionAuthorityError::PublicKeyDecode {
                credential_id: signed.credential_id.0.clone(),
            })?
            .verify_strict(
                &domain_pack_reviewer_registry_rotation_signing_bytes(&candidate_document, signed)?,
                &Signature::from_bytes(&detached_bytes),
            )
            .map_err(|_| DomainPackPromotionAuthorityError::SignatureInvalid {
                credential_id: signed.credential_id.0.clone(),
            })?;
        if !principals.insert(&signed.signer_id.0)
            || !credentials.insert(&signed.credential_id.0)
            || !keys.insert(&credential.public_key_fingerprint)
            || !signatures.insert(&signed.signature)
        {
            return Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet);
        }
        for domain in &credential.independence_domains {
            if !domains.insert(&domain.0) {
                return Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet);
            }
        }
        if credential
            .roles
            .contains(&DomainPackReviewerRole::RegistryAuthorizer)
        {
            registry_authorizer_principals.insert(&signed.signer_id.0);
        }
        if credential
            .roles
            .iter()
            .any(|role| is_semantic_reviewer_role(*role))
        {
            semantic_reviewer_principals.insert(&signed.signer_id.0);
        }
        verified += 1;
    }
    let separated_roles = registry_authorizer_principals.iter().any(|authorizer| {
        semantic_reviewer_principals
            .iter()
            .any(|semantic| semantic != authorizer)
    });
    if verified < usize::from(predecessor.signature_threshold) || !separated_roles {
        return Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet);
    }
    Ok(())
}

fn verify_reviewed_registry_snapshot_signatures(
    reviewer_registry: &DomainPackReviewerRegistryDocument,
    snapshot: &DomainPackReviewedRegistryDocument,
    verified_at_unix: u64,
) -> Result<(), DomainPackPromotionAuthorityError> {
    for entry in &snapshot.domain_pack_reviewed_registry.entries {
        if entry.entry_digest != domain_pack_reviewed_registry_entry_digest(entry)? {
            return evolution("reviewed registry entry digest is not canonical");
        }
    }
    let digest = reviewed_registry_subject_digest(snapshot)?;
    let registry = &reviewer_registry.domain_pack_reviewer_registry;
    let mut verified = Vec::with_capacity(
        snapshot
            .domain_pack_reviewed_registry
            .snapshot_signatures
            .len(),
    );
    let mut values = BTreeSet::new();
    for signed in &snapshot.domain_pack_reviewed_registry.snapshot_signatures {
        if !values.insert(&signed.signature) {
            return Err(DomainPackPromotionAuthorityError::DuplicateSignature);
        }
        if signed.payload_digest != digest {
            return Err(DomainPackPromotionAuthorityError::PayloadDigestMismatch {
                credential_id: signed.credential_id.0.clone(),
            });
        }
        let credential = registry
            .reviewers
            .iter()
            .find(|entry| entry.credential_id == signed.credential_id)
            .ok_or_else(|| DomainPackPromotionAuthorityError::CredentialNotFound {
                credential_id: signed.credential_id.0.clone(),
            })?;
        validate_credential(
            credential,
            &signed.reviewer_id,
            signed.role,
            signed.signed_at_unix,
            verified_at_unix,
        )?;
        let key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
            DomainPackPromotionAuthorityError::PublicKeyDecode {
                credential_id: signed.credential_id.0.clone(),
            }
        })?;
        if domain_pack_promotion_reviewer_key_fingerprint(&key_bytes)
            != credential.public_key_fingerprint
        {
            return Err(
                DomainPackPromotionAuthorityError::PublicKeyFingerprintMismatch {
                    credential_id: signed.credential_id.0.clone(),
                },
            );
        }
        let signature_bytes = decode_fixed::<64>(&signed.signature).ok_or_else(|| {
            DomainPackPromotionAuthorityError::SignatureDecode {
                credential_id: signed.credential_id.0.clone(),
            }
        })?;
        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| DomainPackPromotionAuthorityError::PublicKeyDecode {
                credential_id: signed.credential_id.0.clone(),
            })?
            .verify_strict(
                &domain_pack_reviewed_registry_signing_bytes(snapshot, signed)?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| DomainPackPromotionAuthorityError::SignatureInvalid {
                credential_id: signed.credential_id.0.clone(),
            })?;
        verified.push(VerifiedPromotionReviewer {
            reviewer_id: signed.reviewer_id.clone(),
            credential_id: signed.credential_id.clone(),
            role: signed.role,
            independence_domains: credential.independence_domains.clone(),
            public_key_fingerprint: credential.public_key_fingerprint.clone(),
            signature_fingerprint: raw_digest(&signature_bytes),
            signed_at_unix: signed.signed_at_unix,
        });
    }
    require_promotion_reviewer_separation(&verified)
}

fn validate_reviewed_registry_evolution(
    current: &DomainPackReviewedRegistry,
    proposed: &DomainPackReviewedRegistry,
    transition_from: DomainPackPromotionStage,
    transition_to: DomainPackPromotionStage,
) -> Result<(), DomainPackPromotionAuthorityError> {
    if proposed.registry_id != current.registry_id || proposed.audience != current.audience {
        return Err(DomainPackPromotionAuthorityError::ReviewedRegistryIdentityMismatch);
    }
    if proposed.generation
        != current
            .generation
            .checked_add(1)
            .ok_or(DomainPackPromotionAuthorityError::ReviewedRegistryGenerationMismatch)?
    {
        return Err(DomainPackPromotionAuthorityError::ReviewedRegistryGenerationMismatch);
    }
    if proposed.previous_registry_digest.as_deref() != Some(current.registry_digest.as_str()) {
        return Err(DomainPackPromotionAuthorityError::ReviewedRegistryPredecessorMismatch);
    }
    if proposed.entries.len() < current.entries.len()
        || proposed.entries.len() > current.entries.len() + 1
    {
        return evolution(
            "a successor must preserve all entries and append at most one new identity",
        );
    }
    let mut changed = Vec::new();
    for old in &current.entries {
        let candidates = proposed
            .entries
            .iter()
            .filter(|entry| same_reviewed_identity(entry, old))
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return evolution(
                "every predecessor identity must occur exactly once in the successor",
            );
        }
        let new = candidates[0];
        if new != old {
            if matches!(
                old.stage,
                DomainPackPromotionStage::Revoked | DomainPackPromotionStage::Superseded
            ) {
                return evolution("revoked and superseded records are terminal tombstones");
            }
            require_immutable_reviewed_identity(old, new)?;
            changed.push((old, new));
        }
    }
    let appended = proposed
        .entries
        .iter()
        .filter(|entry| {
            !current
                .entries
                .iter()
                .any(|old| same_reviewed_identity(entry, old))
        })
        .collect::<Vec<_>>();
    if (transition_from, transition_to)
        == (
            DomainPackPromotionStage::Validated,
            DomainPackPromotionStage::Reviewed,
        )
    {
        if !changed.is_empty()
            || appended.len() != 1
            || appended[0].stage != DomainPackPromotionStage::Reviewed
            || appended[0].eligibility != DomainPackReviewedEligibility::EligibleReviewed
        {
            return evolution(
                "validated-to-reviewed must append exactly one eligible reviewed identity",
            );
        }
    } else {
        if changed.len() != 1 || !appended.is_empty() {
            return evolution(
                "a reviewed lifecycle transition must change exactly one preserved identity",
            );
        }
        let (old, new) = changed[0];
        if old.stage != transition_from || new.stage != transition_to {
            return evolution("changed entry does not match the authorized transition");
        }
    }
    for entry in &proposed.entries {
        let expected_entry_digest = domain_pack_reviewed_registry_entry_digest(entry)?;
        if entry.entry_digest != expected_entry_digest {
            return evolution("reviewed registry entry digest is not canonical");
        }
        if matches!(
            entry.stage,
            DomainPackPromotionStage::Revoked | DomainPackPromotionStage::Superseded
        ) && matches!(
            entry.eligibility,
            DomainPackReviewedEligibility::EligibleReviewed
        ) {
            return evolution("a terminal tombstone cannot be eligible");
        }
    }
    Ok(())
}

fn require_dossier_registry_binding(
    current: &DomainPackReviewedRegistry,
    proposed: &DomainPackReviewedRegistry,
    dossier: &DomainPackPromotionDossier,
    decision_digest: &str,
    authorization_payload_digest: &str,
    independent_review_digests: &[String],
) -> Result<(), DomainPackPromotionAuthorityError> {
    let promoted_entry = if (dossier.transition.from, dossier.transition.to)
        == (
            DomainPackPromotionStage::Validated,
            DomainPackPromotionStage::Reviewed,
        ) {
        proposed.entries.iter().find(|entry| {
            !current
                .entries
                .iter()
                .any(|old| same_reviewed_identity(entry, old))
        })
    } else {
        current.entries.iter().find_map(|old| {
            proposed
                .entries
                .iter()
                .find(|entry| same_reviewed_identity(entry, old) && *entry != old)
        })
    }
    .ok_or_else(
        || DomainPackPromotionAuthorityError::ReviewedRegistryEvolution {
            message: "the promoted registry entry is absent".to_owned(),
        },
    )?;

    let dossier_fixture_digests = dossier
        .fixture_bindings
        .iter()
        .map(|fixture| fixture.canonical_sha256.clone())
        .collect::<Vec<_>>();
    if promoted_entry.pack != dossier.pack
        || promoted_entry.package_digest != dossier.package_digest
        || promoted_entry.manifest_digest != dossier.manifest_digest
        || promoted_entry.content_digest != dossier.content_digest
        || promoted_entry.license_digest != dossier.license_digest
        || promoted_entry.fixture_digests != dossier_fixture_digests
        || promoted_entry.stage != dossier.transition.to
        || promoted_entry.promotion_decision_digest != decision_digest
        || promoted_entry.authorization_digest != authorization_payload_digest
    {
        return evolution(
            "the promoted registry entry does not exactly bind the authorized dossier package",
        );
    }
    require_exact_digest_graph(
        independent_review_digests,
        promoted_entry
            .independent_review_digests
            .iter()
            .map(String::as_str),
        "reviewed_registry_entry.independent_review_digests",
    )?;
    Ok(())
}

fn same_reviewed_identity(
    left: &DomainPackReviewedRegistryEntry,
    right: &DomainPackReviewedRegistryEntry,
) -> bool {
    left.pack == right.pack && left.package_digest == right.package_digest
}

fn require_immutable_reviewed_identity(
    old: &DomainPackReviewedRegistryEntry,
    new: &DomainPackReviewedRegistryEntry,
) -> Result<(), DomainPackPromotionAuthorityError> {
    if old.pack != new.pack
        || old.package_digest != new.package_digest
        || old.supply_chain_record_digest != new.supply_chain_record_digest
        || old.manifest_digest != new.manifest_digest
        || old.content_digest != new.content_digest
        || old.license_digest != new.license_digest
        || old.fixture_digests != new.fixture_digests
        || old.compatibility != new.compatibility
    {
        return evolution("a lifecycle disposition cannot rewrite reviewed package semantics");
    }
    Ok(())
}

/// Canonical identity of one reviewed entry with `entry_digest` blanked.
pub fn domain_pack_reviewed_registry_entry_digest(
    entry: &DomainPackReviewedRegistryEntry,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = entry.clone();
    subject.entry_digest.clear();
    canonical_digest(&subject)
}

fn evolution<T>(message: &str) -> Result<T, DomainPackPromotionAuthorityError> {
    Err(
        DomainPackPromotionAuthorityError::ReviewedRegistryEvolution {
            message: message.to_owned(),
        },
    )
}

#[allow(clippy::needless_pass_by_value)]
fn validate_document(
    document: &'static str,
    issues: Vec<forge_core_contracts::domain_pack_learning::DomainPackLearningContractIssue>,
) -> Result<(), DomainPackPromotionAuthorityError> {
    if let Some(issue) = issues.first() {
        Err(DomainPackPromotionAuthorityError::InvalidContract {
            document,
            issue: format!("{}: {}", issue.path, issue.message),
        })
    } else {
        Ok(())
    }
}

/// Canonical digest of a promotion dossier with its digest field blanked.
pub fn domain_pack_promotion_dossier_digest(
    document: &DomainPackPromotionDossierDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject.domain_pack_promotion_dossier.dossier_digest.clear();
    canonical_digest(&subject)
}

/// Canonical digest of a promotion decision with its digest field blanked.
pub fn domain_pack_promotion_decision_digest(
    document: &DomainPackPromotionDecisionDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject
        .domain_pack_promotion_decision
        .decision_digest
        .clear();
    canonical_digest(&subject)
}

/// Canonical digest of independent review evidence with its digest field blanked.
pub fn domain_pack_independent_review_digest(
    document: &DomainPackIndependentReviewDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject.domain_pack_independent_review.review_digest.clear();
    canonical_digest(&subject)
}

/// Digest of the reviewer registry signed during rotation. Signatures and the
/// digest field are blanked to avoid self-reference.
pub fn domain_pack_reviewer_registry_digest(
    document: &DomainPackReviewerRegistryDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    reviewer_registry_subject_digest(document)
}

fn reviewer_registry_subject_digest(
    document: &DomainPackReviewerRegistryDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject
        .domain_pack_reviewer_registry
        .registry_digest
        .clear();
    subject
        .domain_pack_reviewer_registry
        .rotation_signatures
        .clear();
    canonical_digest(&subject)
}

/// Canonical semantic digest of one reviewed registry. Authorization backlink
/// fields are included; callers must construct the exact graph before signing.
pub fn domain_pack_reviewed_registry_digest(
    document: &DomainPackReviewedRegistryDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    reviewed_registry_subject_digest(document)
}

/// Non-circular commitment signed by the decision and authorization. It
/// retains all package, lifecycle, compatibility, and independent-review
/// semantics while removing only the backlinks that do not exist until after
/// the decision/payload digests are known, plus the entry digests derived from
/// those backlinks.
pub fn domain_pack_reviewed_registry_proposal_digest(
    document: &DomainPackReviewedRegistryDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject
        .domain_pack_reviewed_registry
        .registry_digest
        .clear();
    subject
        .domain_pack_reviewed_registry
        .snapshot_signatures
        .clear();
    for entry in &mut subject.domain_pack_reviewed_registry.entries {
        entry.promotion_decision_digest.clear();
        entry.authorization_digest.clear();
        entry.entry_digest.clear();
    }
    canonical_digest(&subject)
}

fn reviewed_registry_subject_digest(
    document: &DomainPackReviewedRegistryDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut subject = document.clone();
    subject
        .domain_pack_reviewed_registry
        .registry_digest
        .clear();
    subject
        .domain_pack_reviewed_registry
        .snapshot_signatures
        .clear();
    canonical_digest(&subject)
}

fn require_binding(
    matches: bool,
    field: &'static str,
) -> Result<(), DomainPackPromotionAuthorityError> {
    if matches {
        Ok(())
    } else {
        Err(DomainPackPromotionAuthorityError::BindingMismatch { field })
    }
}

fn require_exact_digest_graph<'a>(
    expected: &[String],
    observed: impl IntoIterator<Item = &'a str>,
    field: &'static str,
) -> Result<(), DomainPackPromotionAuthorityError> {
    let mut expected = expected.iter().map(String::as_str).collect::<Vec<_>>();
    let mut observed = observed.into_iter().collect::<Vec<_>>();
    expected.sort_unstable();
    observed.sort_unstable();
    let has_duplicates = |values: &[&str]| values.windows(2).any(|pair| pair[0] == pair[1]);
    require_binding(
        !has_duplicates(&expected) && !has_duplicates(&observed) && expected == observed,
        field,
    )
}

fn domain_pack_local_learning_candidate_digest(
    document: &DomainPackLocalLearningCandidateDocument,
) -> Result<String, DomainPackPromotionAuthorityError> {
    let mut value = serde_json::to_value(document)
        .map_err(|error| DomainPackPromotionAuthorityError::Canonicalization(error.to_string()))?;
    value
        .get_mut("domain_pack_local_learning_candidate")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|candidate| candidate.remove("candidate_digest"))
        .ok_or_else(|| {
            DomainPackPromotionAuthorityError::Canonicalization(
                "candidate digest field is absent".to_owned(),
            )
        })?;
    canonical_digest(&value)
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, DomainPackPromotionAuthorityError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| DomainPackPromotionAuthorityError::Canonicalization(error.to_string()))?;
    Ok(raw_digest(&bytes))
}

fn domain_separated<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<Vec<u8>, DomainPackPromotionAuthorityError> {
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| DomainPackPromotionAuthorityError::Canonicalization(error.to_string()))?;
    let mut bytes = Vec::with_capacity(domain.len() + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
