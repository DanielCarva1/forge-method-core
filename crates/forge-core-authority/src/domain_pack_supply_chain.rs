//! P6b supply-chain verification for Domain Pack registry snapshots.
//!
//! Wire documents remain inert. This module verifies one operator-policy-bound
//! registry snapshot, its independent registry signatures, every publisher
//! signature, namespace ownership, revocations, and coordinate/version
//! equivocation before constructing an opaque cryptographic candidate. A
//! registry/audience-bound monotonic anchor must accept that candidate before
//! this module mints lifecycle-consumable supply-chain authority. Neither form
//! is semantic review, runtime capability, execution permission, or Core
//! admission.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCredentialStatus, DomainPackIdentity,
    DomainPackNamespaceGrant, DomainPackRegistryPackageRecord, DomainPackRegistrySignature,
    DomainPackRegistryTrustKey, DomainPackRegistryTrustRole, DomainPackSourceAssurance,
    DomainPackSupplyChainRegistry, DomainPackSupplyChainRegistryDocument,
    DomainPackTrustPolicyDocument, StableId, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Domain separator for publisher signatures over one exact package record.
pub const DOMAIN_PACK_PUBLISHER_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:domain-pack-publisher:v1\0";
/// Domain separator for independent signatures over one exact registry snapshot.
pub const DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:domain-pack-registry-snapshot:v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackSupplyChainError {
    UnsupportedSchemaVersion {
        document: &'static str,
        found: String,
    },
    InvalidPolicy {
        path: String,
        message: String,
    },
    InvalidSnapshot {
        path: String,
        message: String,
    },
    AudienceMismatch {
        expected: String,
        found: String,
    },
    SnapshotNotYetValid {
        issued_at_unix: u64,
        now_unix: u64,
    },
    SnapshotExpired {
        expires_at_unix: u64,
        now_unix: u64,
    },
    SnapshotDigestMismatch {
        expected: String,
        actual: String,
    },
    RegistryKeyNotFound {
        key_id: String,
    },
    RegistryKeyNotActive {
        key_id: String,
    },
    RegistryKeyRoleMismatch {
        key_id: String,
    },
    RegistryKeyOutsideValidity {
        key_id: String,
    },
    RegistrySignatureDuplicate {
        key_id: String,
    },
    RegistrySignatureInvalid {
        key_id: String,
    },
    RegistrySignatureThresholdNotMet {
        required: u16,
        verified: usize,
    },
    PublisherCredentialNotFound {
        credential_id: String,
    },
    PublisherCredentialNotActive {
        credential_id: String,
    },
    PublisherCredentialOutsideValidity {
        credential_id: String,
    },
    PublisherIdentityMismatch {
        credential_id: String,
        expected: String,
        found: String,
    },
    PublisherSignatureInvalid {
        credential_id: String,
        record_digest: String,
    },
    RecordDigestMismatch {
        record_digest: String,
        actual: String,
    },
    NamespaceGrantNotFound {
        grant_id: String,
    },
    NamespaceGrantOutsideValidity {
        grant_id: String,
    },
    NamespacePublisherMismatch {
        grant_id: String,
        publisher: String,
    },
    NamespaceNotGranted {
        grant_id: String,
        namespace: String,
    },
    ReservedCoreNamespace {
        namespace: String,
    },
    RevokedRecord {
        record_digest: String,
    },
    DuplicateRecord {
        coordinate_version: String,
    },
    PackageEquivocation {
        coordinate_version: String,
        first_record_digest: String,
        second_record_digest: String,
    },
    InvalidRegistryAnchor {
        message: String,
    },
    RegistryAnchorIdentityMismatch {
        expected_registry_id: String,
        found_registry_id: String,
        expected_audience: String,
        found_audience: String,
    },
    RegistryAnchorTrustPolicyMismatch {
        anchored_trust_policy_digest: String,
        candidate_trust_policy_digest: String,
    },
    RegistryAnchorCompareAndSwapConflict,
    RegistrySnapshotStale {
        anchored_generation: u64,
        candidate_generation: u64,
    },
    RegistrySnapshotFork {
        generation: u64,
        anchored_digest: String,
        candidate_digest: String,
    },
    RegistrySnapshotGenerationSkip {
        anchored_generation: u64,
        candidate_generation: u64,
    },
    RegistrySnapshotPredecessorMismatch {
        generation: u64,
        expected_digest: String,
        found_digest: Option<String>,
    },
    RegistryAnchorGenerationOverflow,
    InvalidPublicKey {
        subject_id: String,
    },
    InvalidSignatureEncoding {
        subject_id: String,
    },
    Canonicalization(String),
}

impl fmt::Display for DomainPackSupplyChainError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { document, found } => {
                write!(formatter, "unsupported {document} schema version '{found}'")
            }
            Self::InvalidPolicy { path, message } => {
                write!(formatter, "invalid trust policy at '{path}': {message}")
            }
            Self::InvalidSnapshot { path, message } => {
                write!(formatter, "invalid registry snapshot at '{path}': {message}")
            }
            Self::AudienceMismatch { expected, found } => {
                write!(formatter, "registry audience '{found}' does not match policy '{expected}'")
            }
            Self::SnapshotNotYetValid {
                issued_at_unix,
                now_unix,
            } => write!(
                formatter,
                "registry snapshot issued at {issued_at_unix} is in the future at {now_unix}"
            ),
            Self::SnapshotExpired {
                expires_at_unix,
                now_unix,
            } => write!(
                formatter,
                "registry snapshot expired at {expires_at_unix} before {now_unix}"
            ),
            Self::SnapshotDigestMismatch { expected, actual } => write!(
                formatter,
                "registry snapshot digest mismatch: expected '{expected}', recomputed '{actual}'"
            ),
            Self::RegistryKeyNotFound { key_id } => {
                write!(formatter, "registry signer key '{key_id}' is not in operator policy")
            }
            Self::RegistryKeyNotActive { key_id } => {
                write!(formatter, "registry signer key '{key_id}' is not active")
            }
            Self::RegistryKeyRoleMismatch { key_id } => {
                write!(formatter, "registry key '{key_id}' is not a registry signer")
            }
            Self::RegistryKeyOutsideValidity { key_id } => {
                write!(formatter, "registry signer key '{key_id}' is outside validity")
            }
            Self::RegistrySignatureDuplicate { key_id } => {
                write!(formatter, "registry signer key '{key_id}' occurs more than once")
            }
            Self::RegistrySignatureInvalid { key_id } => {
                write!(formatter, "registry signature from '{key_id}' is invalid")
            }
            Self::RegistrySignatureThresholdNotMet { required, verified } => write!(
                formatter,
                "registry signature threshold {required} not met; verified {verified}"
            ),
            Self::PublisherCredentialNotFound { credential_id } => {
                write!(formatter, "publisher credential '{credential_id}' is absent")
            }
            Self::PublisherCredentialNotActive { credential_id } => {
                write!(formatter, "publisher credential '{credential_id}' is not active")
            }
            Self::PublisherCredentialOutsideValidity { credential_id } => {
                write!(formatter, "publisher credential '{credential_id}' is outside validity")
            }
            Self::PublisherIdentityMismatch {
                credential_id,
                expected,
                found,
            } => write!(
                formatter,
                "publisher credential '{credential_id}' belongs to '{expected}', not '{found}'"
            ),
            Self::PublisherSignatureInvalid {
                credential_id,
                record_digest,
            } => write!(
                formatter,
                "publisher signature '{credential_id}' is invalid for '{record_digest}'"
            ),
            Self::RecordDigestMismatch {
                record_digest,
                actual,
            } => write!(
                formatter,
                "package record digest '{record_digest}' recomputes to '{actual}'"
            ),
            Self::NamespaceGrantNotFound { grant_id } => {
                write!(formatter, "namespace grant '{grant_id}' is absent")
            }
            Self::NamespaceGrantOutsideValidity { grant_id } => {
                write!(formatter, "namespace grant '{grant_id}' is outside validity")
            }
            Self::NamespacePublisherMismatch {
                grant_id,
                publisher,
            } => write!(
                formatter,
                "namespace grant '{grant_id}' does not belong to publisher '{publisher}'"
            ),
            Self::NamespaceNotGranted {
                grant_id,
                namespace,
            } => write!(
                formatter,
                "namespace '{namespace}' is outside grant '{grant_id}'"
            ),
            Self::ReservedCoreNamespace { namespace } => {
                write!(formatter, "namespace '{namespace}' is reserved for sealed Core")
            }
            Self::RevokedRecord { record_digest } => {
                write!(formatter, "package record '{record_digest}' is revoked")
            }
            Self::DuplicateRecord { coordinate_version } => {
                write!(formatter, "package record '{coordinate_version}' occurs more than once")
            }
            Self::PackageEquivocation {
                coordinate_version,
                first_record_digest,
                second_record_digest,
            } => write!(
                formatter,
                "package '{coordinate_version}' equivocates between '{first_record_digest}' and '{second_record_digest}'"
            ),
            Self::InvalidRegistryAnchor { message } => {
                write!(formatter, "invalid registry anchor: {message}")
            }
            Self::RegistryAnchorIdentityMismatch {
                expected_registry_id,
                found_registry_id,
                expected_audience,
                found_audience,
            } => write!(
                formatter,
                "registry anchor '{expected_registry_id}'/'{expected_audience}' does not admit snapshot '{found_registry_id}'/'{found_audience}'"
            ),
            Self::RegistryAnchorTrustPolicyMismatch {
                anchored_trust_policy_digest,
                candidate_trust_policy_digest,
            } => write!(
                formatter,
                "registry anchor pins trust policy '{anchored_trust_policy_digest}', not candidate policy '{candidate_trust_policy_digest}'"
            ),
            Self::RegistryAnchorCompareAndSwapConflict => write!(
                formatter,
                "registry anchor changed after the caller captured its compare-and-swap version"
            ),
            Self::RegistrySnapshotStale {
                anchored_generation,
                candidate_generation,
            } => write!(
                formatter,
                "registry snapshot generation {candidate_generation} is older than anchored generation {anchored_generation}"
            ),
            Self::RegistrySnapshotFork {
                generation,
                anchored_digest,
                candidate_digest,
            } => write!(
                formatter,
                "registry snapshot generation {generation} forks anchored digest '{anchored_digest}' with '{candidate_digest}'"
            ),
            Self::RegistrySnapshotGenerationSkip {
                anchored_generation,
                candidate_generation,
            } => write!(
                formatter,
                "registry snapshot generation {candidate_generation} is not the direct successor of anchored generation {anchored_generation}"
            ),
            Self::RegistrySnapshotPredecessorMismatch {
                generation,
                expected_digest,
                found_digest,
            } => write!(
                formatter,
                "registry snapshot generation {generation} names predecessor {found_digest:?}, not anchored digest '{expected_digest}'"
            ),
            Self::RegistryAnchorGenerationOverflow => {
                write!(formatter, "registry anchor generation overflow")
            }
            Self::InvalidPublicKey { subject_id } => {
                write!(formatter, "'{subject_id}' has an invalid Ed25519 public key")
            }
            Self::InvalidSignatureEncoding { subject_id } => {
                write!(formatter, "'{subject_id}' has an invalid Ed25519 signature")
            }
            Self::Canonicalization(message) => {
                write!(formatter, "canonicalization failed: {message}")
            }
        }
    }
}

impl std::error::Error for DomainPackSupplyChainError {}

/// One registry package whose exact publisher signature and ownership were
/// verified. It is read-only and intentionally not serializable.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedDomainPackSupplyChainEntry {
    record: DomainPackRegistryPackageRecord,
    publisher_key_fingerprint: String,
}

impl VerifiedDomainPackSupplyChainEntry {
    #[must_use]
    pub const fn record(&self) -> &DomainPackRegistryPackageRecord {
        &self.record
    }

    #[must_use]
    pub fn publisher_key_fingerprint(&self) -> &str {
        &self.publisher_key_fingerprint
    }
}

/// Opaque cryptographic candidate for one exact operator-policy-bound snapshot.
///
/// It deliberately implements neither `Clone` nor serde traits. Its audit
/// projection cannot recreate the proof. This type alone has no monotonic
/// registry authority; callers must pass it through [`DomainPackRegistryAnchor`].
///
/// ```compile_fail
/// use forge_core_authority::VerifiedDomainPackSupplyChainSnapshot;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedDomainPackSupplyChainSnapshot>();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedDomainPackSupplyChainSnapshot;
/// let _: VerifiedDomainPackSupplyChainSnapshot = serde_json::from_str("{}").unwrap();
/// ```
pub struct VerifiedDomainPackSupplyChainSnapshot {
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    previous_snapshot_digest: Option<String>,
    issued_at_unix: u64,
    expires_at_unix: u64,
    verified_at_unix: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
    registry_signers: Vec<VerifiedDomainPackRegistrySignerAudit>,
    entries: Vec<VerifiedDomainPackSupplyChainEntry>,
    grants: Vec<DomainPackNamespaceGrant>,
}

impl fmt::Debug for VerifiedDomainPackSupplyChainSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDomainPackSupplyChainSnapshot")
            .field("registry_id", &self.registry_id)
            .field("audience", &self.audience)
            .field("generation", &self.generation)
            .field("snapshot_digest", &self.snapshot_digest)
            .field("entry_count", &self.entries.len())
            .field("grant_count", &self.grants.len())
            .finish_non_exhaustive()
    }
}

impl VerifiedDomainPackSupplyChainSnapshot {
    #[must_use]
    pub const fn registry_id(&self) -> &StableId {
        &self.registry_id
    }

    #[must_use]
    pub const fn audience(&self) -> &StableId {
        &self.audience
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn previous_snapshot_digest(&self) -> Option<&str> {
        self.previous_snapshot_digest.as_deref()
    }

    #[must_use]
    pub const fn issued_at_unix(&self) -> u64 {
        self.issued_at_unix
    }

    #[must_use]
    pub const fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }

    #[must_use]
    pub const fn verified_at_unix(&self) -> u64 {
        self.verified_at_unix
    }

    #[must_use]
    pub fn snapshot_digest(&self) -> &str {
        &self.snapshot_digest
    }

    #[must_use]
    pub fn trust_policy_digest(&self) -> &str {
        &self.trust_policy_digest
    }

    #[must_use]
    pub fn entries(&self) -> &[VerifiedDomainPackSupplyChainEntry] {
        &self.entries
    }

    #[must_use]
    pub fn grants(&self) -> &[DomainPackNamespaceGrant] {
        &self.grants
    }

    #[must_use]
    pub fn audit(&self) -> VerifiedDomainPackSupplyChainSnapshotAudit {
        VerifiedDomainPackSupplyChainSnapshotAudit {
            authority: DomainPackSupplyChainAuditAuthority::NonAuthoritative,
            source_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            registry_id: self.registry_id.clone(),
            audience: self.audience.clone(),
            generation: self.generation,
            previous_snapshot_digest: self.previous_snapshot_digest.clone(),
            issued_at_unix: self.issued_at_unix,
            expires_at_unix: self.expires_at_unix,
            verified_at_unix: self.verified_at_unix,
            snapshot_digest: self.snapshot_digest.clone(),
            trust_policy_digest: self.trust_policy_digest.clone(),
            registry_signers: self.registry_signers.clone(),
            entries: self
                .entries
                .iter()
                .map(|entry| VerifiedDomainPackSupplyChainEntryAudit {
                    identity: entry.record.identity.clone(),
                    package_digest: entry.record.package_digest.clone(),
                    manifest_digest: entry.record.manifest_digest.clone(),
                    content_digest: entry.record.content_digest.clone(),
                    license_digest: entry.record.license_digest.clone(),
                    fixture_digests: entry.record.fixture_digests.clone(),
                    namespace_grant_id: entry.record.namespace_grant_id.clone(),
                    publisher_credential_id: entry.record.publisher_credential_id.clone(),
                    publisher_key_fingerprint: entry.publisher_key_fingerprint.clone(),
                    record_digest: entry.record.record_digest.clone(),
                })
                .collect(),
            namespace_grant_ids: self
                .grants
                .iter()
                .map(|grant| grant.grant_id.clone())
                .collect(),
        }
    }
}

/// An in-process monotonic anchor for one exact registry and audience.
///
/// The anchor is intentionally neither cloneable nor serializable. A caller
/// that needs restart durability must restore it from an independently
/// operator-protected head; accepting project-controlled state as a trusted
/// head defeats rollback protection.
pub struct DomainPackRegistryAnchor {
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: Option<String>,
    trust_policy_digest: Option<String>,
}

impl fmt::Debug for DomainPackRegistryAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainPackRegistryAnchor")
            .field("registry_id", &self.registry_id)
            .field("audience", &self.audience)
            .field("generation", &self.generation)
            .field("snapshot_digest", &self.snapshot_digest)
            .field("trust_policy_digest", &self.trust_policy_digest)
            .finish()
    }
}

impl DomainPackRegistryAnchor {
    /// Establish an empty trust-on-first-use anchor.
    ///
    /// Only a valid generation-one snapshot may advance this anchor.
    ///
    /// # Errors
    ///
    /// Returns [`DomainPackSupplyChainError::InvalidRegistryAnchor`] when the
    /// registry id or audience is blank.
    pub fn new_trust_on_first_use(
        registry_id: StableId,
        audience: StableId,
    ) -> Result<Self, DomainPackSupplyChainError> {
        validate_anchor_identity(&registry_id, &audience)?;
        Ok(Self {
            registry_id,
            audience,
            generation: 0,
            snapshot_digest: None,
            trust_policy_digest: None,
        })
    }

    /// Restore a previously trusted head from an operator-protected source.
    ///
    /// This function validates shape, not provenance. The caller is the trust
    /// boundary and must not source these values from the registry snapshot,
    /// project tree, or another attacker-replaceable location.
    ///
    /// # Errors
    ///
    /// Returns an anchor error for a blank identity, generation zero, or a
    /// malformed snapshot or trust-policy digest.
    pub fn from_operator_protected_head(
        registry_id: StableId,
        audience: StableId,
        generation: u64,
        snapshot_digest: String,
        trust_policy_digest: String,
    ) -> Result<Self, DomainPackSupplyChainError> {
        validate_anchor_identity(&registry_id, &audience)?;
        if generation == 0 {
            return Err(DomainPackSupplyChainError::InvalidRegistryAnchor {
                message: "a restored head must have a non-zero generation".to_owned(),
            });
        }
        require_digest(&snapshot_digest, "registry_anchor.snapshot_digest", false)?;
        require_digest(
            &trust_policy_digest,
            "registry_anchor.trust_policy_digest",
            false,
        )?;
        Ok(Self {
            registry_id,
            audience,
            generation,
            snapshot_digest: Some(snapshot_digest),
            trust_policy_digest: Some(trust_policy_digest),
        })
    }

    /// Capture the exact compare-and-swap version of this anchor.
    #[must_use]
    pub fn version(&self) -> DomainPackRegistryAnchorVersion {
        DomainPackRegistryAnchorVersion {
            registry_id: self.registry_id.clone(),
            audience: self.audience.clone(),
            generation: self.generation,
            snapshot_digest: self.snapshot_digest.clone(),
            trust_policy_digest: self.trust_policy_digest.clone(),
        }
    }

    /// Compare-and-swap one cryptographically verified registry snapshot.
    ///
    /// Genesis and an exact direct successor issue a new opaque anchored
    /// capability. An exact replay is idempotent: after fresh cryptographic
    /// verification and an exact match with the protected head, it leaves the
    /// anchor intact and emits a revalidated capability for stateless process
    /// restarts. Older snapshots, same-generation forks, generation skips,
    /// predecessor drift, identity drift, and stale CAS observations fail
    /// closed without authority.
    ///
    /// # Errors
    ///
    /// Returns a typed anchor error when CAS, identity, generation, digest, or
    /// predecessor continuity does not exactly match the current anchor.
    pub fn compare_and_advance(
        &mut self,
        expected: &DomainPackRegistryAnchorVersion,
        candidate: VerifiedDomainPackSupplyChainSnapshot,
    ) -> Result<DomainPackRegistryAnchorAdvance, DomainPackSupplyChainError> {
        if !expected.matches(self) {
            return Err(DomainPackSupplyChainError::RegistryAnchorCompareAndSwapConflict);
        }
        if candidate.registry_id != self.registry_id || candidate.audience != self.audience {
            return Err(DomainPackSupplyChainError::RegistryAnchorIdentityMismatch {
                expected_registry_id: self.registry_id.0.clone(),
                found_registry_id: candidate.registry_id.0.clone(),
                expected_audience: self.audience.0.clone(),
                found_audience: candidate.audience.0.clone(),
            });
        }
        if let Some(anchored_trust_policy_digest) = self.trust_policy_digest.as_deref() {
            if candidate.trust_policy_digest != anchored_trust_policy_digest {
                return Err(
                    DomainPackSupplyChainError::RegistryAnchorTrustPolicyMismatch {
                        anchored_trust_policy_digest: anchored_trust_policy_digest.to_owned(),
                        candidate_trust_policy_digest: candidate.trust_policy_digest,
                    },
                );
            }
        }

        if candidate.generation < self.generation {
            return Err(DomainPackSupplyChainError::RegistrySnapshotStale {
                anchored_generation: self.generation,
                candidate_generation: candidate.generation,
            });
        }
        if candidate.generation == self.generation {
            let anchored_digest = self.snapshot_digest.as_deref().unwrap_or_default();
            if candidate.snapshot_digest != anchored_digest {
                return Err(DomainPackSupplyChainError::RegistrySnapshotFork {
                    generation: self.generation,
                    anchored_digest: anchored_digest.to_owned(),
                    candidate_digest: candidate.snapshot_digest,
                });
            }
            return Ok(DomainPackRegistryAnchorAdvance::Replay {
                capability: AnchoredDomainPackSupplyChainSnapshot {
                    verified: candidate,
                },
                audit: DomainPackRegistryAnchorReplayAudit {
                    registry_id: self.registry_id.clone(),
                    audience: self.audience.clone(),
                    generation: self.generation,
                    snapshot_digest: anchored_digest.to_owned(),
                    trust_policy_digest: self.trust_policy_digest.clone().unwrap_or_default(),
                },
            });
        }

        let direct_successor = self
            .generation
            .checked_add(1)
            .ok_or(DomainPackSupplyChainError::RegistryAnchorGenerationOverflow)?;
        if candidate.generation != direct_successor {
            return Err(DomainPackSupplyChainError::RegistrySnapshotGenerationSkip {
                anchored_generation: self.generation,
                candidate_generation: candidate.generation,
            });
        }
        if self.generation == 0 {
            if candidate.previous_snapshot_digest.is_some() {
                return Err(
                    DomainPackSupplyChainError::RegistrySnapshotPredecessorMismatch {
                        generation: candidate.generation,
                        expected_digest: "<genesis>".to_owned(),
                        found_digest: candidate.previous_snapshot_digest,
                    },
                );
            }
        } else if candidate.previous_snapshot_digest.as_deref() != self.snapshot_digest.as_deref() {
            return Err(
                DomainPackSupplyChainError::RegistrySnapshotPredecessorMismatch {
                    generation: candidate.generation,
                    expected_digest: self.snapshot_digest.clone().unwrap_or_default(),
                    found_digest: candidate.previous_snapshot_digest,
                },
            );
        }

        self.generation = candidate.generation;
        self.snapshot_digest = Some(candidate.snapshot_digest.clone());
        self.trust_policy_digest = Some(candidate.trust_policy_digest.clone());
        Ok(DomainPackRegistryAnchorAdvance::Advanced(
            AnchoredDomainPackSupplyChainSnapshot {
                verified: candidate,
            },
        ))
    }
}

fn validate_anchor_identity(
    registry_id: &StableId,
    audience: &StableId,
) -> Result<(), DomainPackSupplyChainError> {
    if registry_id.0.trim().is_empty() || audience.0.trim().is_empty() {
        return Err(DomainPackSupplyChainError::InvalidRegistryAnchor {
            message: "registry id and audience must not be blank".to_owned(),
        });
    }
    Ok(())
}

/// Opaque compare-and-swap observation for one exact anchor head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRegistryAnchorVersion {
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: Option<String>,
    trust_policy_digest: Option<String>,
}

impl DomainPackRegistryAnchorVersion {
    #[must_use]
    pub const fn registry_id(&self) -> &StableId {
        &self.registry_id
    }

    #[must_use]
    pub const fn audience(&self) -> &StableId {
        &self.audience
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn snapshot_digest(&self) -> Option<&str> {
        self.snapshot_digest.as_deref()
    }

    #[must_use]
    pub fn trust_policy_digest(&self) -> Option<&str> {
        self.trust_policy_digest.as_deref()
    }

    fn matches(&self, anchor: &DomainPackRegistryAnchor) -> bool {
        self.registry_id == anchor.registry_id
            && self.audience == anchor.audience
            && self.generation == anchor.generation
            && self.snapshot_digest == anchor.snapshot_digest
            && self.trust_policy_digest == anchor.trust_policy_digest
    }
}

/// Result of a monotonic anchor compare-and-swap operation.
pub enum DomainPackRegistryAnchorAdvance {
    /// A genesis or direct successor was accepted and minted authority.
    Advanced(AnchoredDomainPackSupplyChainSnapshot),
    /// The exact current head was freshly verified without advancing state.
    Replay {
        capability: AnchoredDomainPackSupplyChainSnapshot,
        audit: DomainPackRegistryAnchorReplayAudit,
    },
}

impl fmt::Debug for DomainPackRegistryAnchorAdvance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Advanced(capability) => {
                formatter.debug_tuple("Advanced").field(capability).finish()
            }
            Self::Replay { capability, audit } => formatter
                .debug_struct("Replay")
                .field("capability", capability)
                .field("audit", audit)
                .finish(),
        }
    }
}

/// Non-authoritative evidence for an idempotent exact-head replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistryAnchorReplayAudit {
    pub registry_id: StableId,
    pub audience: StableId,
    pub generation: u64,
    pub snapshot_digest: String,
    pub trust_policy_digest: String,
}

/// Opaque monotonic supply-chain capability minted only by anchor CAS.
///
/// It deliberately implements neither `Clone` nor serde traits.
///
/// ```compile_fail
/// use forge_core_authority::AnchoredDomainPackSupplyChainSnapshot;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<AnchoredDomainPackSupplyChainSnapshot>();
/// ```
pub struct AnchoredDomainPackSupplyChainSnapshot {
    verified: VerifiedDomainPackSupplyChainSnapshot,
}

impl fmt::Debug for AnchoredDomainPackSupplyChainSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnchoredDomainPackSupplyChainSnapshot")
            .field("registry_id", self.verified.registry_id())
            .field("audience", self.verified.audience())
            .field("generation", &self.verified.generation())
            .field("snapshot_digest", &self.verified.snapshot_digest())
            .finish_non_exhaustive()
    }
}

impl AnchoredDomainPackSupplyChainSnapshot {
    /// Borrow the exact cryptographically verified snapshot carried by this
    /// monotonic capability.
    #[must_use]
    pub const fn verified_snapshot(&self) -> &VerifiedDomainPackSupplyChainSnapshot {
        &self.verified
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSupplyChainAuditAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedDomainPackRegistrySignerAudit {
    pub key_id: StableId,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedDomainPackSupplyChainEntryAudit {
    pub identity: DomainPackIdentity,
    pub package_digest: String,
    pub manifest_digest: String,
    pub content_digest: String,
    pub license_digest: String,
    pub fixture_digests: Vec<String>,
    pub namespace_grant_id: StableId,
    pub publisher_credential_id: StableId,
    pub publisher_key_fingerprint: String,
    pub record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedDomainPackSupplyChainSnapshotAudit {
    pub authority: DomainPackSupplyChainAuditAuthority,
    pub source_assurance: DomainPackSourceAssurance,
    pub registry_id: StableId,
    pub audience: StableId,
    pub generation: u64,
    pub previous_snapshot_digest: Option<String>,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub verified_at_unix: u64,
    pub snapshot_digest: String,
    pub trust_policy_digest: String,
    pub registry_signers: Vec<VerifiedDomainPackRegistrySignerAudit>,
    pub entries: Vec<VerifiedDomainPackSupplyChainEntryAudit>,
    pub namespace_grant_ids: Vec<StableId>,
}

#[derive(Serialize)]
struct PackageRecordSubject<'a> {
    identity: &'a DomainPackIdentity,
    package_digest: &'a str,
    manifest_digest: &'a str,
    content_digest: &'a str,
    license_digest: &'a str,
    fixture_digests: &'a [String],
    namespace_grant_id: &'a StableId,
    publisher_credential_id: &'a StableId,
}

fn package_record_subject(record: &DomainPackRegistryPackageRecord) -> PackageRecordSubject<'_> {
    PackageRecordSubject {
        identity: &record.identity,
        package_digest: &record.package_digest,
        manifest_digest: &record.manifest_digest,
        content_digest: &record.content_digest,
        license_digest: &record.license_digest,
        fixture_digests: &record.fixture_digests,
        namespace_grant_id: &record.namespace_grant_id,
        publisher_credential_id: &record.publisher_credential_id,
    }
}

#[derive(Serialize)]
struct RegistrySnapshotSubject<'a> {
    schema_version: &'a str,
    registry_id: &'a StableId,
    registry_version: &'a str,
    audience: &'a StableId,
    authority: DomainPackCandidateAuthority,
    generation: u64,
    previous_snapshot_digest: &'a Option<String>,
    issued_at_unix: u64,
    expires_at_unix: u64,
    publisher_credentials: &'a [forge_core_contracts::DomainPackPublisherCredential],
    namespace_grants: &'a [DomainPackNamespaceGrant],
    packages: &'a [DomainPackRegistryPackageRecord],
    revocations: &'a [forge_core_contracts::DomainPackPackageRevocation],
}

fn registry_snapshot_subject(
    document: &DomainPackSupplyChainRegistryDocument,
) -> RegistrySnapshotSubject<'_> {
    let registry = &document.domain_pack_supply_chain_registry;
    RegistrySnapshotSubject {
        schema_version: &document.schema_version,
        registry_id: &registry.registry_id,
        registry_version: &registry.registry_version,
        audience: &registry.audience,
        authority: registry.authority,
        generation: registry.generation,
        previous_snapshot_digest: &registry.previous_snapshot_digest,
        issued_at_unix: registry.issued_at_unix,
        expires_at_unix: registry.expires_at_unix,
        publisher_credentials: &registry.publisher_credentials,
        namespace_grants: &registry.namespace_grants,
        packages: &registry.packages,
        revocations: &registry.revocations,
    }
}

/// Compute the canonical digest of a package record excluding its authored
/// `record_digest` and detached publisher signature.
///
/// # Errors
///
/// Returns [`DomainPackSupplyChainError::Canonicalization`] if canonical JSON
/// encoding fails.
pub fn domain_pack_package_record_digest(
    record: &DomainPackRegistryPackageRecord,
) -> Result<String, DomainPackSupplyChainError> {
    canonical_digest(&package_record_subject(record))
}

/// Build exact domain-separated publisher signing bytes for a package record.
///
/// # Errors
///
/// Returns [`DomainPackSupplyChainError::Canonicalization`] if canonical JSON
/// encoding fails.
pub fn domain_pack_publisher_signing_bytes(
    registry_id: &StableId,
    audience: &StableId,
    record: &DomainPackRegistryPackageRecord,
) -> Result<Vec<u8>, DomainPackSupplyChainError> {
    #[derive(Serialize)]
    struct PublisherEnvelope<'a> {
        schema_version: &'a str,
        registry_id: &'a StableId,
        audience: &'a StableId,
        record: PackageRecordSubject<'a>,
    }
    domain_separated_bytes(
        DOMAIN_PACK_PUBLISHER_SIGNATURE_DOMAIN,
        &PublisherEnvelope {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
            registry_id,
            audience,
            record: package_record_subject(record),
        },
    )
}

/// Compute the canonical registry snapshot digest. Detached registry
/// signatures and the authored `snapshot_digest` are deliberately excluded.
///
/// # Errors
///
/// Returns [`DomainPackSupplyChainError::Canonicalization`] if canonical JSON
/// encoding fails.
pub fn domain_pack_registry_snapshot_digest(
    document: &DomainPackSupplyChainRegistryDocument,
) -> Result<String, DomainPackSupplyChainError> {
    canonical_digest(&registry_snapshot_subject(document))
}

/// Build exact domain-separated signing bytes for one independent registry
/// signer. The signer identity and role are bound as well as the snapshot.
///
/// # Errors
///
/// Returns [`DomainPackSupplyChainError::Canonicalization`] if canonical JSON
/// encoding fails.
pub fn domain_pack_registry_signing_bytes(
    document: &DomainPackSupplyChainRegistryDocument,
    signer_key_id: &StableId,
    role: DomainPackRegistryTrustRole,
) -> Result<Vec<u8>, DomainPackSupplyChainError> {
    #[derive(Serialize)]
    struct RegistrySignatureEnvelope<'a> {
        signer_key_id: &'a StableId,
        role: DomainPackRegistryTrustRole,
        snapshot: RegistrySnapshotSubject<'a>,
    }
    domain_separated_bytes(
        DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN,
        &RegistrySignatureEnvelope {
            signer_key_id,
            role,
            snapshot: registry_snapshot_subject(document),
        },
    )
}

/// Verify one exact registry snapshot against an operator-owned trust policy.
///
/// The returned candidate proves supply-chain identity and integrity only. It
/// has no monotonic registry authority, does not semantically trust package
/// content, and cannot activate anything. Pass it through
/// [`DomainPackRegistryAnchor::compare_and_advance`] before authority use.
///
/// # Errors
///
/// Returns a typed supply-chain error whenever document shape, policy,
/// signatures, digests, ownership, validity, revocation, or equivocation fails
/// closed.
pub fn verify_domain_pack_supply_chain_snapshot(
    policy_document: &DomainPackTrustPolicyDocument,
    snapshot_document: &DomainPackSupplyChainRegistryDocument,
    now_unix: u64,
) -> Result<VerifiedDomainPackSupplyChainSnapshot, DomainPackSupplyChainError> {
    validate_document_headers(policy_document, snapshot_document, now_unix)?;
    validate_policy(policy_document)?;
    validate_snapshot_shape(&snapshot_document.domain_pack_supply_chain_registry)?;

    let snapshot_digest = domain_pack_registry_snapshot_digest(snapshot_document)?;
    let registry = &snapshot_document.domain_pack_supply_chain_registry;
    if registry.snapshot_digest != snapshot_digest {
        return Err(DomainPackSupplyChainError::SnapshotDigestMismatch {
            expected: registry.snapshot_digest.clone(),
            actual: snapshot_digest,
        });
    }

    let registry_signers = verify_registry_signatures(policy_document, snapshot_document)?;
    let entries = verify_package_records(snapshot_document, now_unix)?;
    let trust_policy_digest = canonical_digest(policy_document)?;

    Ok(VerifiedDomainPackSupplyChainSnapshot {
        registry_id: registry.registry_id.clone(),
        audience: registry.audience.clone(),
        generation: registry.generation,
        previous_snapshot_digest: registry.previous_snapshot_digest.clone(),
        issued_at_unix: registry.issued_at_unix,
        expires_at_unix: registry.expires_at_unix,
        verified_at_unix: now_unix,
        snapshot_digest: registry.snapshot_digest.clone(),
        trust_policy_digest,
        registry_signers,
        entries,
        grants: registry.namespace_grants.clone(),
    })
}

fn validate_document_headers(
    policy: &DomainPackTrustPolicyDocument,
    snapshot: &DomainPackSupplyChainRegistryDocument,
    now_unix: u64,
) -> Result<(), DomainPackSupplyChainError> {
    if policy.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        return Err(DomainPackSupplyChainError::UnsupportedSchemaVersion {
            document: "domain pack trust policy",
            found: policy.schema_version.clone(),
        });
    }
    if snapshot.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        return Err(DomainPackSupplyChainError::UnsupportedSchemaVersion {
            document: "domain pack supply-chain registry",
            found: snapshot.schema_version.clone(),
        });
    }
    let policy = &policy.domain_pack_trust_policy;
    let registry = &snapshot.domain_pack_supply_chain_registry;
    if policy.audience != registry.audience {
        return Err(DomainPackSupplyChainError::AudienceMismatch {
            expected: policy.audience.0.clone(),
            found: registry.audience.0.clone(),
        });
    }
    if registry.issued_at_unix >= registry.expires_at_unix {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry.expires_at_unix".to_owned(),
            message: "expiry must be later than issuance".to_owned(),
        });
    }
    if now_unix < registry.issued_at_unix {
        return Err(DomainPackSupplyChainError::SnapshotNotYetValid {
            issued_at_unix: registry.issued_at_unix,
            now_unix,
        });
    }
    if now_unix >= registry.expires_at_unix {
        return Err(DomainPackSupplyChainError::SnapshotExpired {
            expires_at_unix: registry.expires_at_unix,
            now_unix,
        });
    }
    if registry.generation == 0 {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry.generation".to_owned(),
            message: "generation must be greater than zero".to_owned(),
        });
    }
    if registry.generation == 1 && registry.previous_snapshot_digest.is_some() {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry.previous_snapshot_digest".to_owned(),
            message: "generation one cannot name a predecessor".to_owned(),
        });
    }
    if registry.generation > 1 && registry.previous_snapshot_digest.is_none() {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry.previous_snapshot_digest".to_owned(),
            message: "later generations require an exact predecessor digest".to_owned(),
        });
    }
    if let Some(digest) = &registry.previous_snapshot_digest {
        require_digest(
            digest,
            "domain_pack_supply_chain_registry.previous_snapshot_digest",
            false,
        )?;
    }
    Ok(())
}

fn validate_policy(
    document: &DomainPackTrustPolicyDocument,
) -> Result<(), DomainPackSupplyChainError> {
    let policy = &document.domain_pack_trust_policy;
    if policy.policy_id.0.trim().is_empty()
        || policy.policy_version.trim().is_empty()
        || policy.audience.0.trim().is_empty()
    {
        return Err(DomainPackSupplyChainError::InvalidPolicy {
            path: "domain_pack_trust_policy".to_owned(),
            message: "policy id, version, and audience must not be blank".to_owned(),
        });
    }
    if policy.required_registry_signature_threshold == 0 {
        return Err(DomainPackSupplyChainError::InvalidPolicy {
            path: "domain_pack_trust_policy.required_registry_signature_threshold".to_owned(),
            message: "signature threshold must be greater than zero".to_owned(),
        });
    }
    let mut key_ids = BTreeSet::new();
    let mut signer_key_material = BTreeSet::new();
    let mut signer_count = 0usize;
    for key in &policy.registry_keys {
        if !key_ids.insert(key.key_id.0.clone()) {
            return Err(DomainPackSupplyChainError::InvalidPolicy {
                path: format!("domain_pack_trust_policy.registry_keys.{}", key.key_id.0),
                message: "registry key id occurs more than once".to_owned(),
            });
        }
        if key.valid_from_unix >= key.valid_until_unix {
            return Err(DomainPackSupplyChainError::InvalidPolicy {
                path: format!("domain_pack_trust_policy.registry_keys.{}", key.key_id.0),
                message: "registry key validity window is empty".to_owned(),
            });
        }
        let bytes = decode_fixed::<32>(&key.public_key_hex).ok_or_else(|| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: key.key_id.0.clone(),
            }
        })?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: key.key_id.0.clone(),
            }
        })?;
        if key.role == DomainPackRegistryTrustRole::RegistrySigner {
            signer_count += 1;
            if !signer_key_material.insert(bytes) {
                return Err(DomainPackSupplyChainError::InvalidPolicy {
                    path: format!("domain_pack_trust_policy.registry_keys.{}", key.key_id.0),
                    message: "distinct registry signer ids must use distinct keys".to_owned(),
                });
            }
        }
    }
    if usize::from(policy.required_registry_signature_threshold) > signer_count {
        return Err(DomainPackSupplyChainError::InvalidPolicy {
            path: "domain_pack_trust_policy.required_registry_signature_threshold".to_owned(),
            message: "signature threshold exceeds configured registry signer count".to_owned(),
        });
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &policy.rules {
        if !rule_ids.insert(rule.rule_id.0.clone()) {
            return Err(DomainPackSupplyChainError::InvalidPolicy {
                path: format!("domain_pack_trust_policy.rules.{}", rule.rule_id.0),
                message: "trust rule id occurs more than once".to_owned(),
            });
        }
        if let Some(digest) = &rule.package_digest {
            require_digest(
                digest,
                "domain_pack_trust_policy.rules.package_digest",
                true,
            )?;
        }
        if let Some(digest) = &rule.content_digest {
            require_digest(
                digest,
                "domain_pack_trust_policy.rules.content_digest",
                true,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_snapshot_shape(
    registry: &DomainPackSupplyChainRegistry,
) -> Result<(), DomainPackSupplyChainError> {
    if registry.registry_id.0.trim().is_empty()
        || registry.registry_version.trim().is_empty()
        || registry.audience.0.trim().is_empty()
    {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry".to_owned(),
            message: "registry id, version, and audience must not be blank".to_owned(),
        });
    }
    require_digest(
        &registry.snapshot_digest,
        "domain_pack_supply_chain_registry.snapshot_digest",
        false,
    )?;
    if registry.publisher_credentials.is_empty()
        || registry.namespace_grants.is_empty()
        || registry.packages.is_empty()
        || registry.signatures.is_empty()
    {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry".to_owned(),
            message: "credentials, grants, packages, and signatures must not be empty".to_owned(),
        });
    }

    let mut credential_ids = BTreeSet::new();
    for credential in &registry.publisher_credentials {
        if !credential_ids.insert(credential.credential_id.0.clone()) {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.publisher_credentials.{}",
                    credential.credential_id.0
                ),
                message: "publisher credential id occurs more than once".to_owned(),
            });
        }
        if credential.valid_from_unix >= credential.valid_until_unix {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.publisher_credentials.{}",
                    credential.credential_id.0
                ),
                message: "publisher credential validity window is empty".to_owned(),
            });
        }
        let bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: credential.credential_id.0.clone(),
            }
        })?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: credential.credential_id.0.clone(),
            }
        })?;
    }

    let mut grant_ids = BTreeSet::new();
    let mut grants_by_prefix = BTreeMap::<String, String>::new();
    for grant in &registry.namespace_grants {
        if !grant_ids.insert(grant.grant_id.0.clone()) {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.namespace_grants.{}",
                    grant.grant_id.0
                ),
                message: "namespace grant id occurs more than once".to_owned(),
            });
        }
        if grant.valid_from_unix >= grant.valid_until_unix {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.namespace_grants.{}",
                    grant.grant_id.0
                ),
                message: "namespace grant validity window is empty".to_owned(),
            });
        }
        for (prefix, publisher) in &grants_by_prefix {
            if publisher != &grant.publisher.0
                && namespace_prefixes_overlap(prefix, &grant.namespace_prefix.0)
            {
                return Err(DomainPackSupplyChainError::InvalidSnapshot {
                    path: format!(
                        "domain_pack_supply_chain_registry.namespace_grants.{}",
                        grant.grant_id.0
                    ),
                    message: format!(
                        "namespace prefix '{}' overlaps publisher '{}'",
                        grant.namespace_prefix.0, publisher
                    ),
                });
            }
        }
        if grants_by_prefix
            .insert(grant.namespace_prefix.0.clone(), grant.publisher.0.clone())
            .is_some()
        {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.namespace_grants.{}",
                    grant.grant_id.0
                ),
                message: "namespace prefix occurs more than once".to_owned(),
            });
        }
    }

    let mut revocations = BTreeSet::new();
    for revocation in &registry.revocations {
        require_digest(
            &revocation.record_digest,
            "domain_pack_supply_chain_registry.revocations.record_digest",
            false,
        )?;
        if !revocations.insert(revocation.record_digest.clone()) {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.revocations.{}",
                    revocation.record_digest
                ),
                message: "record revocation occurs more than once".to_owned(),
            });
        }
        if revocation.explanation.trim().is_empty() {
            return Err(DomainPackSupplyChainError::InvalidSnapshot {
                path: format!(
                    "domain_pack_supply_chain_registry.revocations.{}",
                    revocation.record_digest
                ),
                message: "revocation explanation must not be blank".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_registry_signatures(
    policy: &DomainPackTrustPolicyDocument,
    snapshot: &DomainPackSupplyChainRegistryDocument,
) -> Result<Vec<VerifiedDomainPackRegistrySignerAudit>, DomainPackSupplyChainError> {
    let policy = &policy.domain_pack_trust_policy;
    let registry = &snapshot.domain_pack_supply_chain_registry;
    let keys = policy
        .registry_keys
        .iter()
        .map(|key| (key.key_id.0.as_str(), key))
        .collect::<BTreeMap<_, _>>();
    let mut seen_key_ids = BTreeSet::new();
    let mut verified_key_material = BTreeSet::new();
    let mut audits = Vec::new();
    for signature in &registry.signatures {
        let key_id = signature.signer_key_id.0.clone();
        if !seen_key_ids.insert(key_id.clone()) {
            return Err(DomainPackSupplyChainError::RegistrySignatureDuplicate { key_id });
        }
        let key = keys.get(key_id.as_str()).copied().ok_or_else(|| {
            DomainPackSupplyChainError::RegistryKeyNotFound {
                key_id: key_id.clone(),
            }
        })?;
        validate_registry_signer_key(key, signature, registry)?;
        let public_key_bytes = decode_fixed::<32>(&key.public_key_hex).ok_or_else(|| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: key_id.clone(),
            }
        })?;
        if !verified_key_material.insert(public_key_bytes) {
            return Err(DomainPackSupplyChainError::RegistrySignatureDuplicate { key_id });
        }
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: key_id.clone(),
            }
        })?;
        let signature_bytes = decode_fixed::<64>(&signature.signature_hex).ok_or_else(|| {
            DomainPackSupplyChainError::InvalidSignatureEncoding {
                subject_id: key_id.clone(),
            }
        })?;
        let detached = Signature::from_bytes(&signature_bytes);
        let signing_bytes =
            domain_pack_registry_signing_bytes(snapshot, &signature.signer_key_id, signature.role)?;
        verifying_key
            .verify_strict(&signing_bytes, &detached)
            .map_err(|_| DomainPackSupplyChainError::RegistrySignatureInvalid {
                key_id: key_id.clone(),
            })?;
        audits.push(VerifiedDomainPackRegistrySignerAudit {
            key_id: signature.signer_key_id.clone(),
            public_key_fingerprint: raw_digest(&public_key_bytes),
            signature_fingerprint: raw_digest(&signature_bytes),
        });
    }
    if audits.len() < usize::from(policy.required_registry_signature_threshold) {
        return Err(
            DomainPackSupplyChainError::RegistrySignatureThresholdNotMet {
                required: policy.required_registry_signature_threshold,
                verified: audits.len(),
            },
        );
    }
    audits.sort_by(|left, right| left.key_id.0.cmp(&right.key_id.0));
    Ok(audits)
}

fn validate_registry_signer_key(
    key: &DomainPackRegistryTrustKey,
    signature: &DomainPackRegistrySignature,
    registry: &DomainPackSupplyChainRegistry,
) -> Result<(), DomainPackSupplyChainError> {
    let key_id = key.key_id.0.clone();
    if key.status != DomainPackCredentialStatus::Active {
        return Err(DomainPackSupplyChainError::RegistryKeyNotActive { key_id });
    }
    if key.role != DomainPackRegistryTrustRole::RegistrySigner
        || signature.role != DomainPackRegistryTrustRole::RegistrySigner
        || signature.role != key.role
    {
        return Err(DomainPackSupplyChainError::RegistryKeyRoleMismatch { key_id });
    }
    if !time_inclusive(
        registry.issued_at_unix,
        key.valid_from_unix,
        key.valid_until_unix,
    ) || !time_inclusive(
        registry.expires_at_unix,
        key.valid_from_unix,
        key.valid_until_unix,
    ) {
        return Err(DomainPackSupplyChainError::RegistryKeyOutsideValidity { key_id });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_package_records(
    snapshot: &DomainPackSupplyChainRegistryDocument,
    now_unix: u64,
) -> Result<Vec<VerifiedDomainPackSupplyChainEntry>, DomainPackSupplyChainError> {
    let registry = &snapshot.domain_pack_supply_chain_registry;
    let credentials = registry
        .publisher_credentials
        .iter()
        .map(|credential| (credential.credential_id.0.as_str(), credential))
        .collect::<BTreeMap<_, _>>();
    let grants = registry
        .namespace_grants
        .iter()
        .map(|grant| (grant.grant_id.0.as_str(), grant))
        .collect::<BTreeMap<_, _>>();
    let revoked = registry
        .revocations
        .iter()
        .map(|revocation| revocation.record_digest.as_str())
        .collect::<BTreeSet<_>>();
    let mut coordinate_versions = BTreeMap::<String, String>::new();
    let mut entries = Vec::new();
    for record in &registry.packages {
        validate_record_digests(record)?;
        let actual_record_digest = domain_pack_package_record_digest(record)?;
        if actual_record_digest != record.record_digest {
            return Err(DomainPackSupplyChainError::RecordDigestMismatch {
                record_digest: record.record_digest.clone(),
                actual: actual_record_digest,
            });
        }
        let coordinate_version = format!(
            "{}::{}@{}",
            record.identity.publisher.0, record.identity.name.0, record.identity.version
        );
        if let Some(previous) = coordinate_versions.get(&coordinate_version) {
            if previous == &record.record_digest {
                return Err(DomainPackSupplyChainError::DuplicateRecord { coordinate_version });
            }
            return Err(DomainPackSupplyChainError::PackageEquivocation {
                coordinate_version,
                first_record_digest: previous.clone(),
                second_record_digest: record.record_digest.clone(),
            });
        }
        coordinate_versions.insert(coordinate_version, record.record_digest.clone());
        if revoked.contains(record.record_digest.as_str()) {
            return Err(DomainPackSupplyChainError::RevokedRecord {
                record_digest: record.record_digest.clone(),
            });
        }

        let credential_id = record.publisher_credential_id.0.clone();
        let credential = credentials
            .get(credential_id.as_str())
            .copied()
            .ok_or_else(|| DomainPackSupplyChainError::PublisherCredentialNotFound {
                credential_id: credential_id.clone(),
            })?;
        if credential.status != DomainPackCredentialStatus::Active {
            return Err(DomainPackSupplyChainError::PublisherCredentialNotActive { credential_id });
        }
        if !time_inclusive(
            registry.issued_at_unix,
            credential.valid_from_unix,
            credential.valid_until_unix,
        ) || !time_inclusive(
            now_unix,
            credential.valid_from_unix,
            credential.valid_until_unix,
        ) || !time_inclusive(
            registry.expires_at_unix,
            credential.valid_from_unix,
            credential.valid_until_unix,
        ) {
            return Err(
                DomainPackSupplyChainError::PublisherCredentialOutsideValidity { credential_id },
            );
        }
        if credential.publisher != record.identity.publisher {
            return Err(DomainPackSupplyChainError::PublisherIdentityMismatch {
                credential_id,
                expected: credential.publisher.0.clone(),
                found: record.identity.publisher.0.clone(),
            });
        }
        let grant = grants
            .get(record.namespace_grant_id.0.as_str())
            .copied()
            .ok_or_else(|| DomainPackSupplyChainError::NamespaceGrantNotFound {
                grant_id: record.namespace_grant_id.0.clone(),
            })?;
        validate_record_namespace(record, grant, registry, now_unix)?;

        let public_key_bytes = decode_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: credential.credential_id.0.clone(),
            }
        })?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
            DomainPackSupplyChainError::InvalidPublicKey {
                subject_id: credential.credential_id.0.clone(),
            }
        })?;
        let signature_bytes =
            decode_fixed::<64>(&record.publisher_signature_hex).ok_or_else(|| {
                DomainPackSupplyChainError::InvalidSignatureEncoding {
                    subject_id: credential.credential_id.0.clone(),
                }
            })?;
        let detached = Signature::from_bytes(&signature_bytes);
        let signing_bytes =
            domain_pack_publisher_signing_bytes(&registry.registry_id, &registry.audience, record)?;
        verifying_key
            .verify_strict(&signing_bytes, &detached)
            .map_err(|_| DomainPackSupplyChainError::PublisherSignatureInvalid {
                credential_id: credential.credential_id.0.clone(),
                record_digest: record.record_digest.clone(),
            })?;
        entries.push(VerifiedDomainPackSupplyChainEntry {
            record: record.clone(),
            publisher_key_fingerprint: raw_digest(&public_key_bytes),
        });
    }
    entries.sort_by(|left, right| {
        identity_key(&left.record.identity).cmp(&identity_key(&right.record.identity))
    });
    Ok(entries)
}

fn validate_record_digests(
    record: &DomainPackRegistryPackageRecord,
) -> Result<(), DomainPackSupplyChainError> {
    for (path, digest) in [
        ("package_digest", &record.package_digest),
        ("manifest_digest", &record.manifest_digest),
        ("content_digest", &record.content_digest),
        ("license_digest", &record.license_digest),
        ("record_digest", &record.record_digest),
    ] {
        require_digest(digest, path, false)?;
    }
    for digest in &record.fixture_digests {
        require_digest(digest, "fixture_digests", false)?;
    }
    if record.identity.publisher.0.trim().is_empty()
        || record.identity.name.0.trim().is_empty()
        || record.identity.namespace.0.trim().is_empty()
        || record.identity.version.trim().is_empty()
    {
        return Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: "domain_pack_supply_chain_registry.packages.identity".to_owned(),
            message: "package identity fields must not be blank".to_owned(),
        });
    }
    Ok(())
}

fn validate_record_namespace(
    record: &DomainPackRegistryPackageRecord,
    grant: &DomainPackNamespaceGrant,
    registry: &DomainPackSupplyChainRegistry,
    now_unix: u64,
) -> Result<(), DomainPackSupplyChainError> {
    let namespace = &record.identity.namespace.0;
    if namespace == "core"
        || namespace.starts_with("core.")
        || namespace == "forge.core"
        || namespace.starts_with("forge.core.")
    {
        return Err(DomainPackSupplyChainError::ReservedCoreNamespace {
            namespace: namespace.clone(),
        });
    }
    if !time_inclusive(
        registry.issued_at_unix,
        grant.valid_from_unix,
        grant.valid_until_unix,
    ) || !time_inclusive(now_unix, grant.valid_from_unix, grant.valid_until_unix)
    {
        return Err(DomainPackSupplyChainError::NamespaceGrantOutsideValidity {
            grant_id: grant.grant_id.0.clone(),
        });
    }
    if grant.publisher != record.identity.publisher {
        return Err(DomainPackSupplyChainError::NamespacePublisherMismatch {
            grant_id: grant.grant_id.0.clone(),
            publisher: record.identity.publisher.0.clone(),
        });
    }
    if !namespace_is_within(namespace, &grant.namespace_prefix.0) {
        return Err(DomainPackSupplyChainError::NamespaceNotGranted {
            grant_id: grant.grant_id.0.clone(),
            namespace: namespace.clone(),
        });
    }
    Ok(())
}

fn namespace_is_within(namespace: &str, prefix: &str) -> bool {
    namespace == prefix
        || namespace
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn namespace_prefixes_overlap(left: &str, right: &str) -> bool {
    namespace_is_within(left, right) || namespace_is_within(right, left)
}

fn identity_key(identity: &DomainPackIdentity) -> String {
    format!(
        "{}::{}@{}#{}",
        identity.publisher.0, identity.name.0, identity.version, identity.namespace.0
    )
}

fn time_inclusive(value: u64, from: u64, until: u64) -> bool {
    value >= from && value <= until
}

fn require_digest(value: &str, path: &str, policy: bool) -> Result<(), DomainPackSupplyChainError> {
    if is_sha256_token(value) {
        return Ok(());
    }
    if policy {
        Err(DomainPackSupplyChainError::InvalidPolicy {
            path: path.to_owned(),
            message: "digest must be sha256: plus 64 lowercase hexadecimal characters".to_owned(),
        })
    } else {
        Err(DomainPackSupplyChainError::InvalidSnapshot {
            path: path.to_owned(),
            message: "digest must be sha256: plus 64 lowercase hexadecimal characters".to_owned(),
        })
    }
}

fn is_sha256_token(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, DomainPackSupplyChainError> {
    let bytes = canonical_bytes(value)?;
    Ok(raw_digest(&bytes))
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, DomainPackSupplyChainError> {
    let value = serde_json::to_value(value)
        .map_err(|error| DomainPackSupplyChainError::Canonicalization(error.to_string()))?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| DomainPackSupplyChainError::Canonicalization(error.to_string()))
}

fn domain_separated_bytes<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<Vec<u8>, DomainPackSupplyChainError> {
    let canonical = canonical_bytes(value)?;
    let mut bytes = Vec::with_capacity(domain.len() + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?;
    }
    Some(bytes)
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
