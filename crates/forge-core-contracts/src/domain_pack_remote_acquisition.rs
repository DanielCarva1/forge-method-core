#![allow(clippy::missing_errors_doc)]

//! Closed C6.2 remote Domain Pack acquisition contracts.
//!
//! Every value here is candidate-only evidence or a deterministic request/plan.
//! It deliberately represents neither trust nor installation/activation authority.
//! The existing `DomainPackSupplyChainRegistryDocument` remains the sole signed
//! catalog; remote metadata extends its signed subject rather than creating a
//! second catalog authority.

use std::collections::BTreeSet;

use crate::{
    DomainPackCandidateAuthority, DomainPackExactLockDocument, DomainPackLifecycleReceiptDocument,
    DomainPackPackageBinding, DomainPackRegistryArtifactSet, DomainPackRegistryMirror,
    DomainPackRegistryMirrorTransport, DomainPackRegistryPackageRecord,
    DomainPackRemoteArtifactDescriptor, DomainPackRemoteArtifactKind,
    DomainPackRemoteArtifactMediaType, DomainPackSupplyChainRegistryDocument, RepoPath, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION: &str = "0.1";
pub const MAX_DOMAIN_PACK_REMOTE_MIRRORS: usize = 32;
pub const MAX_DOMAIN_PACK_REMOTE_ARTIFACTS_PER_PACKAGE: usize = 256;
pub const MAX_DOMAIN_PACK_REMOTE_FETCH_OBSERVATIONS: usize = 256;
pub const MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES: u32 = 4_096;
pub const MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRY_BYTES: u64 = 1 << 34;
pub const MAX_DOMAIN_PACK_REMOTE_CACHE_TOTAL_BYTES: u64 = 1 << 40;

/// Return the only permitted mirror attempt order: the signed priority followed
/// by lexical stable mirror-ID tie breaking. A caller cannot supply an alternate
/// order and an implementation must still apply current anchor/signature policy
/// before a returned mirror becomes usable.
pub fn domain_pack_remote_signed_mirror_order(
    registry: &DomainPackSupplyChainRegistryDocument,
) -> Result<Vec<&DomainPackRegistryMirror>, DomainPackRemoteAcquisitionContractError> {
    registry.validate_remote_acquisition_metadata()?;
    let mut mirrors = registry
        .domain_pack_supply_chain_registry
        .mirrors
        .iter()
        .collect::<Vec<_>>();
    mirrors.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.mirror_id.cmp(&right.mirror_id))
    });
    Ok(mirrors)
}

/// Network availability policy selected before any transport is attempted.
/// There is intentionally no serde default: omitting this value denies network
/// behavior rather than silently selecting a convenient mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteNetworkMode {
    OnlineRequired,
    PreferCache,
    OfflineOnly,
}

/// Mirrors are selected only by the signed catalog ordering. A caller may not
/// serialize a preferred endpoint or reorder equivalent mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteMirrorPolicy {
    SignedPriorityThenMirrorId,
}

/// Bounded non-evicting cache policy. There is no eviction, overwrite, or
/// implicit expansion variant: an admission that would exceed a limit rejects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackRemoteCachePolicy {
    RejectOnFull {
        max_entry_bytes: u64,
        max_entries: u32,
        max_total_bytes: u64,
    },
}

/// Exact discovery evidence and explicit operator candidate selection. This is
/// still only a selection input, not supply-chain/trust/lifecycle authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCandidateDiscoveryBinding {
    pub acquisition_id: StableId,
    pub discovery_projection_digest: String,
    pub demand_digest: String,
    pub candidate_id: StableId,
    pub requirement_ref: StableId,
    pub selection: DomainPackRemoteOperatorSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteOperatorSelection {
    ExplicitCandidateApprovalRequired,
}

/// Full signed catalog snapshot plus the exact snapshot identity a consumer
/// must re-verify and anchor. It cannot substitute for an opaque anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCatalogSnapshotBinding {
    pub registry: DomainPackSupplyChainRegistryDocument,
    pub snapshot_digest: String,
}

/// Exact record and package sidecars selected from the signed catalog.
/// `package_digest` remains the established package-level identity; cache keys
/// are deliberately the individual artifact raw SHA-256 values instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemotePackageAcquisitionBinding {
    pub record: DomainPackRegistryPackageRecord,
    pub package: DomainPackPackageBinding,
}

/// One exact artifact object selected under one signed mirror. The physical
/// transport base never appears here; it is resolved solely from `mirror_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteArtifactLocationBinding {
    pub artifact: DomainPackRemoteArtifactDescriptor,
    pub mirror_id: StableId,
    pub object_path: RepoPath,
}

/// A locally retained catalog head provisioned and protected by the operator.
/// It contains no independent freshness deadline: `expires_at_unix` must equal
/// the signed catalog snapshot expiry, so using it offline can never extend
/// freshness. The TCB supplies and checks current host time before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteOperatorAnchoredLocalHead {
    pub registry_id: StableId,
    pub audience: StableId,
    pub generation: u64,
    pub snapshot_digest: String,
    pub expires_at_unix: u64,
    pub anchored_at_unix: u64,
}

/// Candidate-only request that binds discovery, one exact signed catalog
/// snapshot/record/package, and explicit network/cache constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteAcquisitionRequestDocument {
    pub schema_version: String,
    pub domain_pack_remote_acquisition_request: DomainPackRemoteAcquisitionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteAcquisitionRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub discovery: DomainPackRemoteCandidateDiscoveryBinding,
    pub catalog: DomainPackRemoteCatalogSnapshotBinding,
    pub package: DomainPackRemotePackageAcquisitionBinding,
    pub network_mode: DomainPackRemoteNetworkMode,
    pub mirror_policy: DomainPackRemoteMirrorPolicy,
    pub cache_policy: DomainPackRemoteCachePolicy,
    pub operator_anchored_local_head: Option<DomainPackRemoteOperatorAnchoredLocalHead>,
    pub request_digest: String,
}

/// Deterministic candidate-byte acquisition plan. Its only non-blocked outcomes
/// ask for candidate bytes; neither carries trusted, installed, or active state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteAcquisitionPlanDocument {
    pub schema_version: String,
    pub domain_pack_remote_acquisition_plan: DomainPackRemoteAcquisitionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteAcquisitionPlan {
    pub plan_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub request_digest: String,
    pub catalog: DomainPackRemoteCatalogSnapshotBinding,
    pub discovery: DomainPackRemoteCandidateDiscoveryBinding,
    pub package: DomainPackRemotePackageAcquisitionBinding,
    pub network_mode: DomainPackRemoteNetworkMode,
    pub mirror_policy: DomainPackRemoteMirrorPolicy,
    pub cache_policy: DomainPackRemoteCachePolicy,
    pub operator_anchored_local_head: Option<DomainPackRemoteOperatorAnchoredLocalHead>,
    pub artifacts: Vec<DomainPackRemoteArtifactLocationBinding>,
    pub outcome: DomainPackRemoteAcquisitionPlanOutcome,
    pub blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    pub plan_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteAcquisitionPlanOutcome {
    CandidateBytesRequired,
    CacheOnlyCandidateBytesRequired,
    Blocked,
}

/// Source of untrusted bytes. A cache hit and a local operator mirror are
/// evidence sources only and never convey trust or freshness by themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackRemoteFetchSource {
    NetworkMirror {
        mirror_id: StableId,
    },
    Cache {
        cache_key_raw_sha256: String,
    },
    OperatorAnchoredLocalMirror {
        mirror_id: StableId,
        anchored_snapshot_digest: String,
    },
}

/// Untrusted observation of a single fetch or cache read before the byte pins
/// are rechecked. It is deliberately an observation rather than an admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteUntrustedFetchObservationDocument {
    pub schema_version: String,
    pub domain_pack_remote_untrusted_fetch_observation: DomainPackRemoteUntrustedFetchObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteUntrustedFetchObservation {
    pub observation_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub request_digest: String,
    pub source: DomainPackRemoteFetchSource,
    pub location: DomainPackRemoteArtifactLocationBinding,
    pub observed_raw_sha256: String,
    pub observed_canonical_sha256: String,
    pub observed_byte_length: u64,
    pub observed_media_type: DomainPackRemoteArtifactMediaType,
    pub observation_digest: String,
}

/// Collected untrusted observations and their closed byte-verification outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteFetchEvidenceDocument {
    pub schema_version: String,
    pub domain_pack_remote_fetch_evidence: DomainPackRemoteFetchEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteFetchEvidence {
    pub evidence_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub plan_digest: String,
    pub observations: Vec<DomainPackRemoteUntrustedFetchObservation>,
    pub outcome: DomainPackRemoteFetchOutcome,
    pub blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    pub evidence_digest: String,
}

/// Candidate-byte verification is intentionally not a trust decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteFetchOutcome {
    CandidateBytesVerified,
    Blocked,
}

/// Immutable candidate-byte receipt used by later trust/preflight owners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteFetchReceiptDocument {
    pub schema_version: String,
    pub domain_pack_remote_fetch_receipt: DomainPackRemoteFetchReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteFetchReceipt {
    pub receipt_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub plan_digest: String,
    pub catalog_snapshot_digest: String,
    pub registry_record_digest: String,
    pub package_digest: String,
    pub artifacts: Vec<DomainPackRemoteFetchedArtifactReceipt>,
    pub outcome: DomainPackRemoteFetchOutcome,
    pub blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteFetchedArtifactReceipt {
    pub location: DomainPackRemoteArtifactLocationBinding,
    pub source: DomainPackRemoteFetchSource,
    pub raw_sha256: String,
    pub canonical_sha256: String,
    pub byte_length: u64,
    pub media_type: DomainPackRemoteArtifactMediaType,
}

/// Immutable candidate-byte cache entry. `cache_key_raw_sha256` must equal the
/// observed artifact raw hash; cache presence is explicitly non-authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCacheEntryDocument {
    pub schema_version: String,
    pub domain_pack_remote_cache_entry: DomainPackRemoteCacheEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCacheEntry {
    pub cache_key_raw_sha256: String,
    pub artifact: DomainPackRemoteArtifactDescriptor,
    pub byte_length: u64,
    pub source_receipt_digest: String,
    pub cached_at_unix: u64,
    pub entry_digest: String,
}

/// Non-authoritative cache inventory. It records only locally present candidate
/// bytes and deterministic reject-on-full results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCacheProjectionDocument {
    pub schema_version: String,
    pub domain_pack_remote_cache_projection: DomainPackRemoteCacheProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCacheProjection {
    pub cache_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub policy: DomainPackRemoteCachePolicy,
    pub entries: Vec<DomainPackRemoteCacheEntry>,
    pub total_bytes: u64,
    pub outcome: DomainPackRemoteCacheProjectionOutcome,
    pub blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    pub projection_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteCacheProjectionOutcome {
    CandidateBytesPresent,
    CandidateBytesMissing,
    RejectedOnFull,
    Blocked,
}

/// Exact retained material needed for a rollback. It intentionally has no
/// network mode, mirror, fetch, or cache dependency: a rollback never downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteRollbackInputDocument {
    pub schema_version: String,
    pub domain_pack_remote_rollback_input: DomainPackRemoteRollbackInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteRollbackInput {
    pub rollback_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub historical: DomainPackRemoteRetainedRollbackMaterial,
    /// Freshly verified/current anchored catalog material used only to evaluate
    /// current revocation and policy. It cannot replace historical objects.
    pub current_catalog_policy: DomainPackRemoteCurrentCatalogPolicyInput,
    pub outcome: DomainPackRemoteRollbackOutcome,
    pub blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    pub rollback_input_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteRetainedRollbackMaterial {
    pub receipt: DomainPackLifecycleReceiptDocument,
    pub receipt_digest: String,
    pub lock: DomainPackExactLockDocument,
    pub lock_digest: String,
    pub generation: u64,
    pub generation_digest: String,
    pub objects: Vec<DomainPackRemoteRetainedObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteRetainedObject {
    pub raw_sha256: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteCurrentCatalogPolicyInput {
    pub catalog: DomainPackRemoteCatalogSnapshotBinding,
    pub operator_anchored_head: DomainPackRemoteOperatorAnchoredLocalHead,
    pub host_checked_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteRollbackOutcome {
    CandidatePreflightRequired,
    Blocked,
}

/// Closed failure vocabulary for remote acquisition. Blocks remain evidence;
/// none implies trust, install, private commit authority, or activation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum DomainPackRemoteAcquisitionBlock {
    CatalogSignatureTamper,
    CatalogSnapshotMismatch,
    CatalogAnchorMissing,
    CatalogExpired,
    CatalogRevoked,
    DuplicateMirror,
    UnknownMirror,
    MirrorTransportInvalid,
    MirrorEquivocation,
    ArtifactDescriptorSetMismatch,
    ArtifactLocationInvalid,
    ArtifactRawDigestMismatch,
    ArtifactCanonicalDigestMismatch,
    ArtifactLengthMismatch,
    ArtifactMediaTypeMismatch,
    CacheMiss,
    CacheTamper,
    CacheFull,
    OfflineLocalHeadMissing,
    OfflineLocalHeadStale,
    OfflineExactBytesMissing,
    OfflineRevoked,
    NetworkDenied,
    RollbackDownloadAttempt,
    CumulativeRevocationMismatch,
    ImplicitTrustDenied,
    ImplicitInstallDenied,
    ImplicitActivationDenied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackRemoteAcquisitionContractError {
    UnsupportedSchemaVersion { found: String },
    InvalidDigest { path: String },
    InvalidIdentifier { path: String },
    InvalidLocation { path: String },
    InvalidCachePolicy { message: String },
    DuplicateMirror { mirror_id: String },
    DuplicateArtifact { path: String },
    UnknownMirror { mirror_id: String },
    ArtifactDescriptorSetMismatch { path: String },
    OfflineLocalHeadRequired,
    OfflineLocalHeadMismatch,
    OfflineLocalHeadExpired,
    InvalidOutcomeBlocks,
    Canonicalization { message: String },
    DigestMismatch { path: String },
}

impl DomainPackSupplyChainRegistryDocument {
    /// Validate the C6.2 signed remote metadata shape. Signature/anchor owners
    /// must call this before accepting a snapshot so mirrors and descriptors are
    /// covered by the same verified catalog authority as every other field.
    pub fn validate_remote_acquisition_metadata(
        &self,
    ) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        let registry = &self.domain_pack_supply_chain_registry;
        if registry.mirrors.len() > MAX_DOMAIN_PACK_REMOTE_MIRRORS {
            return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
                path: "domain_pack_supply_chain_registry.mirrors".to_owned(),
            });
        }
        let mut mirrors = BTreeSet::new();
        for (index, mirror) in registry.mirrors.iter().enumerate() {
            let path = format!("domain_pack_supply_chain_registry.mirrors[{index}]");
            required_id(&mirror.mirror_id, &format!("{path}.mirror_id"))?;
            if !mirrors.insert(mirror.mirror_id.0.as_str()) {
                return Err(DomainPackRemoteAcquisitionContractError::DuplicateMirror {
                    mirror_id: mirror.mirror_id.0.clone(),
                });
            }
            validate_mirror_transport(&mirror.transport, &format!("{path}.transport"))?;
        }
        for (index, record) in registry.packages.iter().enumerate() {
            let record_path = format!("domain_pack_supply_chain_registry.packages[{index}]");
            for (field, digest) in [
                ("package_digest", &record.package_digest),
                ("manifest_digest", &record.manifest_digest),
                ("content_digest", &record.content_digest),
                ("license_digest", &record.license_digest),
                ("record_digest", &record.record_digest),
            ] {
                require_digest(digest, &format!("{record_path}.{field}"))?;
            }
            for (fixture_index, digest) in record.fixture_digests.iter().enumerate() {
                require_digest(
                    digest,
                    &format!("{record_path}.fixture_digests[{fixture_index}]"),
                )?;
            }
            validate_artifact_set(
                &record.artifacts,
                record,
                &format!("{record_path}.artifacts"),
            )?;
        }
        Ok(())
    }
}

impl DomainPackRemoteAcquisitionRequestDocument {
    pub fn canonical_request_bytes(
        &self,
    ) -> Result<Vec<u8>, DomainPackRemoteAcquisitionContractError> {
        canonical_document_bytes(
            self,
            "domain_pack_remote_acquisition_request",
            "request_digest",
        )
    }

    pub fn request_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(
            self,
            "domain_pack_remote_acquisition_request",
            "request_digest",
        )
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        validate_request(&self.domain_pack_remote_acquisition_request)?;
        require_self_digest(
            &self.domain_pack_remote_acquisition_request.request_digest,
            self.request_digest()?,
            "request_digest",
        )
    }
}

impl DomainPackRemoteAcquisitionPlanDocument {
    pub fn canonical_plan_bytes(
        &self,
    ) -> Result<Vec<u8>, DomainPackRemoteAcquisitionContractError> {
        canonical_document_bytes(self, "domain_pack_remote_acquisition_plan", "plan_digest")
    }

    pub fn plan_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(self, "domain_pack_remote_acquisition_plan", "plan_digest")
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let plan = &self.domain_pack_remote_acquisition_plan;
        validate_plan(plan)?;
        require_self_digest(&plan.plan_digest, self.plan_digest()?, "plan_digest")
    }
}

impl DomainPackRemoteUntrustedFetchObservationDocument {
    pub fn observation_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(
            self,
            "domain_pack_remote_untrusted_fetch_observation",
            "observation_digest",
        )
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let observation = &self.domain_pack_remote_untrusted_fetch_observation;
        required_id(&observation.observation_id, "observation.observation_id")?;
        require_digest(&observation.request_digest, "observation.request_digest")?;
        validate_location_binding(&observation.location, None)?;
        require_digest(
            &observation.observed_raw_sha256,
            "observation.observed_raw_sha256",
        )?;
        require_digest(
            &observation.observed_canonical_sha256,
            "observation.observed_canonical_sha256",
        )?;
        require_self_digest(
            &observation.observation_digest,
            self.observation_digest()?,
            "observation.observation_digest",
        )
    }
}

impl DomainPackRemoteFetchEvidenceDocument {
    pub fn evidence_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(self, "domain_pack_remote_fetch_evidence", "evidence_digest")
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let evidence = &self.domain_pack_remote_fetch_evidence;
        required_id(&evidence.evidence_id, "evidence.evidence_id")?;
        require_digest(&evidence.plan_digest, "evidence.plan_digest")?;
        if evidence.observations.len() > MAX_DOMAIN_PACK_REMOTE_FETCH_OBSERVATIONS {
            return Err(
                DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                    path: "evidence.observations".to_owned(),
                },
            );
        }
        for observation in &evidence.observations {
            validate_observation(observation)?;
            if matches!(
                evidence.outcome,
                DomainPackRemoteFetchOutcome::CandidateBytesVerified
            ) {
                validate_verified_observation(observation)?;
            }
        }
        validate_fetch_outcome(evidence.outcome, &evidence.blocks)?;
        require_self_digest(
            &evidence.evidence_digest,
            self.evidence_digest()?,
            "evidence.evidence_digest",
        )
    }
}

impl DomainPackRemoteFetchReceiptDocument {
    pub fn receipt_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(self, "domain_pack_remote_fetch_receipt", "receipt_digest")
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let receipt = &self.domain_pack_remote_fetch_receipt;
        required_id(&receipt.receipt_id, "receipt.receipt_id")?;
        for (path, value) in [
            ("receipt.plan_digest", &receipt.plan_digest),
            (
                "receipt.catalog_snapshot_digest",
                &receipt.catalog_snapshot_digest,
            ),
            (
                "receipt.registry_record_digest",
                &receipt.registry_record_digest,
            ),
            ("receipt.package_digest", &receipt.package_digest),
        ] {
            require_digest(value, path)?;
        }
        for artifact in &receipt.artifacts {
            validate_fetched_artifact_receipt(artifact)?;
        }
        validate_fetch_outcome(receipt.outcome, &receipt.blocks)?;
        require_self_digest(
            &receipt.receipt_digest,
            self.receipt_digest()?,
            "receipt.receipt_digest",
        )
    }
}

impl DomainPackRemoteCacheEntryDocument {
    pub fn entry_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(self, "domain_pack_remote_cache_entry", "entry_digest")
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        validate_cache_entry(&self.domain_pack_remote_cache_entry)?;
        require_self_digest(
            &self.domain_pack_remote_cache_entry.entry_digest,
            self.entry_digest()?,
            "cache.entry_digest",
        )
    }
}

impl DomainPackRemoteCacheProjectionDocument {
    pub fn projection_digest(&self) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(
            self,
            "domain_pack_remote_cache_projection",
            "projection_digest",
        )
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let projection = &self.domain_pack_remote_cache_projection;
        required_id(&projection.cache_id, "cache.cache_id")?;
        validate_cache_policy(&projection.policy)?;
        let mut cache_keys = BTreeSet::new();
        let mut total = 0_u64;
        for entry in &projection.entries {
            validate_cache_entry(entry)?;
            if !cache_keys.insert(entry.cache_key_raw_sha256.as_str()) {
                return Err(
                    DomainPackRemoteAcquisitionContractError::DuplicateArtifact {
                        path: "cache.entries.cache_key_raw_sha256".to_owned(),
                    },
                );
            }
            total = total.checked_add(entry.byte_length).ok_or_else(|| {
                DomainPackRemoteAcquisitionContractError::InvalidCachePolicy {
                    message: "cache total byte count overflows u64".to_owned(),
                }
            })?;
        }
        if total != projection.total_bytes {
            return Err(
                DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                    path: "cache.total_bytes".to_owned(),
                },
            );
        }
        validate_cache_projection_limits(projection)?;
        require_self_digest(
            &projection.projection_digest,
            self.projection_digest()?,
            "cache.projection_digest",
        )
    }
}

impl DomainPackRemoteRollbackInputDocument {
    pub fn rollback_input_digest(
        &self,
    ) -> Result<String, DomainPackRemoteAcquisitionContractError> {
        canonical_document_digest(
            self,
            "domain_pack_remote_rollback_input",
            "rollback_input_digest",
        )
    }

    pub fn validate(&self) -> Result<(), DomainPackRemoteAcquisitionContractError> {
        validate_schema(&self.schema_version)?;
        let input = &self.domain_pack_remote_rollback_input;
        required_id(&input.rollback_id, "rollback.rollback_id")?;
        validate_retained_rollback_material(&input.historical)?;
        validate_current_catalog_policy(&input.current_catalog_policy)?;
        validate_rollback_outcome(input.outcome, &input.blocks)?;
        require_self_digest(
            &input.rollback_input_digest,
            self.rollback_input_digest()?,
            "rollback.rollback_input_digest",
        )
    }
}

fn validate_request(
    request: &DomainPackRemoteAcquisitionRequest,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    required_id(&request.request_id, "request.request_id")?;
    validate_discovery(&request.discovery)?;
    validate_catalog_binding(&request.catalog)?;
    validate_package_binding(&request.catalog, &request.package)?;
    validate_cache_policy(&request.cache_policy)?;
    validate_offline_requirement(
        request.network_mode,
        request.operator_anchored_local_head.as_ref(),
        &request.catalog,
    )?;
    require_digest(&request.request_digest, "request.request_digest")
}

fn validate_plan(
    plan: &DomainPackRemoteAcquisitionPlan,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    required_id(&plan.plan_id, "plan.plan_id")?;
    require_digest(&plan.request_digest, "plan.request_digest")?;
    validate_discovery(&plan.discovery)?;
    validate_catalog_binding(&plan.catalog)?;
    validate_package_binding(&plan.catalog, &plan.package)?;
    validate_cache_policy(&plan.cache_policy)?;
    validate_offline_requirement(
        plan.network_mode,
        plan.operator_anchored_local_head.as_ref(),
        &plan.catalog,
    )?;
    let descriptors = package_descriptors(&plan.package.record.artifacts);
    if plan.artifacts.len() != descriptors.len() {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "plan.artifacts".to_owned(),
            },
        );
    }
    let mut object_paths = BTreeSet::new();
    for location in &plan.artifacts {
        validate_location_binding(location, Some(&plan.catalog.registry))?;
        if !descriptors.contains(&&location.artifact) {
            return Err(
                DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                    path: "plan.artifacts.artifact".to_owned(),
                },
            );
        }
        if !object_paths.insert(location.object_path.0.as_str()) {
            return Err(
                DomainPackRemoteAcquisitionContractError::DuplicateArtifact {
                    path: "plan.artifacts.object_path".to_owned(),
                },
            );
        }
        if matches!(plan.network_mode, DomainPackRemoteNetworkMode::OfflineOnly)
            && !is_operator_local_mirror(&plan.catalog.registry, &location.mirror_id)
        {
            return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadRequired);
        }
    }
    validate_plan_outcome(plan.outcome, &plan.blocks)?;
    require_digest(&plan.plan_digest, "plan.plan_digest")
}

fn validate_discovery(
    discovery: &DomainPackRemoteCandidateDiscoveryBinding,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    for (path, value) in [
        ("discovery.acquisition_id", &discovery.acquisition_id),
        ("discovery.candidate_id", &discovery.candidate_id),
        ("discovery.requirement_ref", &discovery.requirement_ref),
    ] {
        required_id(value, path)?;
    }
    require_digest(
        &discovery.discovery_projection_digest,
        "discovery.discovery_projection_digest",
    )?;
    require_digest(&discovery.demand_digest, "discovery.demand_digest")
}

fn validate_catalog_binding(
    catalog: &DomainPackRemoteCatalogSnapshotBinding,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    catalog.registry.validate_remote_acquisition_metadata()?;
    let registry = &catalog.registry.domain_pack_supply_chain_registry;
    require_digest(&catalog.snapshot_digest, "catalog.snapshot_digest")?;
    if catalog.snapshot_digest != registry.snapshot_digest {
        return Err(DomainPackRemoteAcquisitionContractError::DigestMismatch {
            path: "catalog.snapshot_digest".to_owned(),
        });
    }
    Ok(())
}

fn validate_package_binding(
    catalog: &DomainPackRemoteCatalogSnapshotBinding,
    package: &DomainPackRemotePackageAcquisitionBinding,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    let record = &package.record;
    if !catalog
        .registry
        .domain_pack_supply_chain_registry
        .packages
        .iter()
        .any(|catalog_record| catalog_record == record)
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "package.record".to_owned(),
            },
        );
    }
    if package.package.package_digest != record.package_digest
        || package.package.manifest != record.artifacts.manifest.binding
        || package.package.content.content_ref != record.artifacts.content.binding.artifact_ref
        || package.package.content.raw_sha256 != record.artifacts.content.binding.raw_sha256
        || package.package.content.canonical_sha256
            != record.artifacts.content.binding.canonical_sha256
        || package.package.license != record.artifacts.license.binding
        || package.package.fixtures.len() != record.artifacts.fixtures.len()
        || package
            .package
            .fixtures
            .iter()
            .zip(&record.artifacts.fixtures)
            .any(|(binding, descriptor)| binding != &descriptor.binding)
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "package".to_owned(),
            },
        );
    }
    Ok(())
}

fn validate_artifact_set(
    artifacts: &DomainPackRegistryArtifactSet,
    record: &DomainPackRegistryPackageRecord,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    let descriptors = package_descriptors(artifacts);
    if descriptors.len() > MAX_DOMAIN_PACK_REMOTE_ARTIFACTS_PER_PACKAGE {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: path.to_owned(),
            },
        );
    }
    if artifacts.manifest.kind != DomainPackRemoteArtifactKind::Manifest
        || artifacts.content.kind != DomainPackRemoteArtifactKind::Content
        || artifacts.license.kind != DomainPackRemoteArtifactKind::License
        || artifacts
            .fixtures
            .iter()
            .any(|fixture| fixture.kind != DomainPackRemoteArtifactKind::Fixture)
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: path.to_owned(),
            },
        );
    }
    let mut logical_refs = BTreeSet::new();
    let mut object_paths = BTreeSet::new();
    for (index, descriptor) in descriptors.iter().enumerate() {
        validate_artifact_descriptor(descriptor, &format!("{path}[{index}]"))?;
        if !logical_refs.insert(descriptor.binding.artifact_ref.0.as_str()) {
            return Err(
                DomainPackRemoteAcquisitionContractError::DuplicateArtifact {
                    path: format!("{path}.binding.artifact_ref"),
                },
            );
        }
        if !object_paths.insert(descriptor.object_path.0.as_str()) {
            return Err(
                DomainPackRemoteAcquisitionContractError::DuplicateArtifact {
                    path: format!("{path}.object_path"),
                },
            );
        }
    }
    if record.manifest_digest != artifacts.manifest.binding.raw_sha256
        || record.content_digest != artifacts.content.binding.raw_sha256
        || record.license_digest != artifacts.license.binding.raw_sha256
        || record.fixture_digests.len() != artifacts.fixtures.len()
        || record
            .fixture_digests
            .iter()
            .zip(&artifacts.fixtures)
            .any(|(digest, fixture)| digest != &fixture.binding.raw_sha256)
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: path.to_owned(),
            },
        );
    }
    Ok(())
}

fn validate_artifact_descriptor(
    descriptor: &DomainPackRemoteArtifactDescriptor,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_repo_path(
        &descriptor.binding.artifact_ref,
        &format!("{path}.binding.artifact_ref"),
    )?;
    require_digest(
        &descriptor.binding.raw_sha256,
        &format!("{path}.binding.raw_sha256"),
    )?;
    require_digest(
        &descriptor.binding.canonical_sha256,
        &format!("{path}.binding.canonical_sha256"),
    )?;
    validate_content_addressed_object_path(
        &descriptor.object_path,
        &descriptor.binding.raw_sha256,
        &format!("{path}.object_path"),
    )
}

fn validate_location_binding(
    location: &DomainPackRemoteArtifactLocationBinding,
    registry: Option<&DomainPackSupplyChainRegistryDocument>,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_artifact_descriptor(&location.artifact, "location.artifact")?;
    validate_content_addressed_object_path(
        &location.object_path,
        &location.artifact.binding.raw_sha256,
        "location.object_path",
    )?;
    if location.object_path != location.artifact.object_path {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "location.object_path".to_owned(),
            },
        );
    }
    required_id(&location.mirror_id, "location.mirror_id")?;
    if let Some(registry) = registry {
        if !registry
            .domain_pack_supply_chain_registry
            .mirrors
            .iter()
            .any(|mirror| mirror.mirror_id == location.mirror_id)
        {
            return Err(DomainPackRemoteAcquisitionContractError::UnknownMirror {
                mirror_id: location.mirror_id.0.clone(),
            });
        }
    }
    Ok(())
}

fn validate_mirror_transport(
    transport: &DomainPackRegistryMirrorTransport,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    match transport {
        DomainPackRegistryMirrorTransport::Https { base_url } => {
            validate_https_base_url(base_url, &format!("{path}.base_url"))
        }
        DomainPackRegistryMirrorTransport::OperatorProvisionedLocal { location_id } => {
            required_id(location_id, &format!("{path}.location_id"))
        }
    }
}

fn validate_https_base_url(
    value: &str,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    let Some(remainder) = value.strip_prefix("https://") else {
        return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
            path: path.to_owned(),
        });
    };
    if remainder.is_empty()
        || value.bytes().any(|byte| byte.is_ascii_control())
        || value.contains(['@', '#', '?', '\\'])
    {
        return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
            path: path.to_owned(),
        });
    }
    let mut parts = remainder.split('/');
    if parts.next().is_none_or(str::is_empty)
        || parts.any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn validate_repo_path(
    path: &RepoPath,
    field: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    if path.0.is_empty()
        || path.0.starts_with('/')
        || path.0.starts_with('\\')
        || path.0.contains('\\')
        || path.0.bytes().any(|byte| byte.is_ascii_control())
        || path
            .0
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
            path: field.to_owned(),
        });
    }
    Ok(())
}

fn validate_content_addressed_object_path(
    path: &RepoPath,
    raw_sha256: &str,
    field: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_repo_path(path, field)?;
    require_digest(raw_sha256, field)?;
    let expected = format!("objects/sha256/{}", &raw_sha256["sha256:".len()..]);
    if path.0 != expected {
        return Err(DomainPackRemoteAcquisitionContractError::InvalidLocation {
            path: field.to_owned(),
        });
    }
    Ok(())
}

fn validate_cache_policy(
    policy: &DomainPackRemoteCachePolicy,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    match policy {
        DomainPackRemoteCachePolicy::RejectOnFull {
            max_entry_bytes,
            max_entries,
            max_total_bytes,
        } if *max_entry_bytes > 0
            && *max_entries > 0
            && *max_entries <= MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES
            && *max_total_bytes >= *max_entry_bytes
            && *max_entry_bytes <= MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRY_BYTES
            && *max_total_bytes <= MAX_DOMAIN_PACK_REMOTE_CACHE_TOTAL_BYTES =>
        {
            Ok(())
        }
        DomainPackRemoteCachePolicy::RejectOnFull { .. } => Err(
            DomainPackRemoteAcquisitionContractError::InvalidCachePolicy {
                message: "reject-on-full limits must be nonzero, bounded, and permit one entry"
                    .to_owned(),
            },
        ),
    }
}

fn validate_offline_requirement(
    network_mode: DomainPackRemoteNetworkMode,
    head: Option<&DomainPackRemoteOperatorAnchoredLocalHead>,
    catalog: &DomainPackRemoteCatalogSnapshotBinding,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    if !matches!(network_mode, DomainPackRemoteNetworkMode::OfflineOnly) {
        return Ok(());
    }
    let Some(head) = head else {
        return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadRequired);
    };
    let registry = &catalog.registry.domain_pack_supply_chain_registry;
    if head.registry_id != registry.registry_id
        || head.audience != registry.audience
        || head.generation != registry.generation
        || head.snapshot_digest != catalog.snapshot_digest
        || head.expires_at_unix != registry.expires_at_unix
    {
        return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadMismatch);
    }
    if head.anchored_at_unix >= head.expires_at_unix {
        return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadExpired);
    }
    if !registry.mirrors.iter().any(|mirror| {
        matches!(
            &mirror.transport,
            DomainPackRegistryMirrorTransport::OperatorProvisionedLocal { .. }
        )
    }) {
        return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadRequired);
    }
    Ok(())
}

fn validate_observation(
    observation: &DomainPackRemoteUntrustedFetchObservation,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    required_id(&observation.observation_id, "observation.observation_id")?;
    require_digest(&observation.request_digest, "observation.request_digest")?;
    validate_location_binding(&observation.location, None)?;
    for (path, value) in [
        (
            "observation.observed_raw_sha256",
            &observation.observed_raw_sha256,
        ),
        (
            "observation.observed_canonical_sha256",
            &observation.observed_canonical_sha256,
        ),
        (
            "observation.observation_digest",
            &observation.observation_digest,
        ),
    ] {
        require_digest(value, path)?;
    }
    Ok(())
}

fn validate_verified_observation(
    observation: &DomainPackRemoteUntrustedFetchObservation,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    let expected = &observation.location.artifact;
    if observation.observed_raw_sha256 != expected.binding.raw_sha256
        || observation.observed_canonical_sha256 != expected.binding.canonical_sha256
        || observation.observed_byte_length != expected.byte_length
        || observation.observed_media_type != expected.media_type
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "evidence.observations".to_owned(),
            },
        );
    }
    match &observation.source {
        DomainPackRemoteFetchSource::Cache {
            cache_key_raw_sha256,
        } if cache_key_raw_sha256 != &expected.binding.raw_sha256 => Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "evidence.observations.source.cache_key_raw_sha256".to_owned(),
            },
        ),
        _ => Ok(()),
    }
}

fn validate_fetched_artifact_receipt(
    artifact: &DomainPackRemoteFetchedArtifactReceipt,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_location_binding(&artifact.location, None)?;
    if artifact.raw_sha256 != artifact.location.artifact.binding.raw_sha256
        || artifact.canonical_sha256 != artifact.location.artifact.binding.canonical_sha256
        || artifact.byte_length != artifact.location.artifact.byte_length
        || artifact.media_type != artifact.location.artifact.media_type
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "receipt.artifacts".to_owned(),
            },
        );
    }
    Ok(())
}

fn validate_fetch_outcome(
    outcome: DomainPackRemoteFetchOutcome,
    blocks: &[DomainPackRemoteAcquisitionBlock],
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    match (outcome, blocks.is_empty()) {
        (DomainPackRemoteFetchOutcome::CandidateBytesVerified, true)
        | (DomainPackRemoteFetchOutcome::Blocked, false) => Ok(()),
        _ => Err(DomainPackRemoteAcquisitionContractError::InvalidOutcomeBlocks),
    }
}

fn validate_plan_outcome(
    outcome: DomainPackRemoteAcquisitionPlanOutcome,
    blocks: &[DomainPackRemoteAcquisitionBlock],
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    match (outcome, blocks.is_empty()) {
        (
            DomainPackRemoteAcquisitionPlanOutcome::CandidateBytesRequired
            | DomainPackRemoteAcquisitionPlanOutcome::CacheOnlyCandidateBytesRequired,
            true,
        )
        | (DomainPackRemoteAcquisitionPlanOutcome::Blocked, false) => Ok(()),
        _ => Err(DomainPackRemoteAcquisitionContractError::InvalidOutcomeBlocks),
    }
}

fn validate_cache_entry(
    entry: &DomainPackRemoteCacheEntry,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_artifact_descriptor(&entry.artifact, "cache.artifact")?;
    require_digest(&entry.cache_key_raw_sha256, "cache.cache_key_raw_sha256")?;
    require_digest(&entry.source_receipt_digest, "cache.source_receipt_digest")?;
    require_digest(&entry.entry_digest, "cache.entry_digest")?;
    if entry.cache_key_raw_sha256 != entry.artifact.binding.raw_sha256
        || entry.byte_length != entry.artifact.byte_length
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::ArtifactDescriptorSetMismatch {
                path: "cache.entry".to_owned(),
            },
        );
    }
    Ok(())
}

fn validate_cache_projection_limits(
    projection: &DomainPackRemoteCacheProjection,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    let DomainPackRemoteCachePolicy::RejectOnFull {
        max_entry_bytes,
        max_entries,
        max_total_bytes,
    } = &projection.policy;
    if projection.entries.len() > usize::try_from(*max_entries).unwrap_or(usize::MAX)
        || projection.total_bytes > *max_total_bytes
        || projection
            .entries
            .iter()
            .any(|entry| entry.byte_length > *max_entry_bytes)
    {
        return Err(
            DomainPackRemoteAcquisitionContractError::InvalidCachePolicy {
                message: "cache inventory exceeds reject-on-full policy".to_owned(),
            },
        );
    }
    match (projection.outcome, projection.blocks.is_empty()) {
        (
            DomainPackRemoteCacheProjectionOutcome::Blocked
            | DomainPackRemoteCacheProjectionOutcome::RejectedOnFull,
            false,
        )
        | (
            DomainPackRemoteCacheProjectionOutcome::CandidateBytesPresent
            | DomainPackRemoteCacheProjectionOutcome::CandidateBytesMissing,
            true,
        ) => Ok(()),
        _ => Err(DomainPackRemoteAcquisitionContractError::InvalidOutcomeBlocks),
    }
}

fn validate_retained_rollback_material(
    material: &DomainPackRemoteRetainedRollbackMaterial,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    require_digest(
        &material.receipt_digest,
        "rollback.historical.receipt_digest",
    )?;
    require_digest(&material.lock_digest, "rollback.historical.lock_digest")?;
    require_digest(
        &material.generation_digest,
        "rollback.historical.generation_digest",
    )?;
    if material.receipt_digest
        != material
            .receipt
            .domain_pack_lifecycle_receipt
            .receipt_digest
        || material.lock_digest != material.lock.domain_pack_exact_lock.lock_digest
    {
        return Err(DomainPackRemoteAcquisitionContractError::DigestMismatch {
            path: "rollback.historical".to_owned(),
        });
    }
    let mut objects = BTreeSet::new();
    for object in &material.objects {
        require_digest(&object.raw_sha256, "rollback.historical.objects.raw_sha256")?;
        if !objects.insert(object.raw_sha256.as_str()) {
            return Err(
                DomainPackRemoteAcquisitionContractError::DuplicateArtifact {
                    path: "rollback.historical.objects.raw_sha256".to_owned(),
                },
            );
        }
    }
    Ok(())
}

fn validate_current_catalog_policy(
    current: &DomainPackRemoteCurrentCatalogPolicyInput,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_catalog_binding(&current.catalog)?;
    validate_offline_head_against_catalog(&current.operator_anchored_head, &current.catalog)?;
    if current.host_checked_at_unix >= current.operator_anchored_head.expires_at_unix {
        return Err(DomainPackRemoteAcquisitionContractError::OfflineLocalHeadExpired);
    }
    Ok(())
}

fn validate_offline_head_against_catalog(
    head: &DomainPackRemoteOperatorAnchoredLocalHead,
    catalog: &DomainPackRemoteCatalogSnapshotBinding,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    validate_offline_requirement(
        DomainPackRemoteNetworkMode::OfflineOnly,
        Some(head),
        catalog,
    )
}

fn validate_rollback_outcome(
    outcome: DomainPackRemoteRollbackOutcome,
    blocks: &[DomainPackRemoteAcquisitionBlock],
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    match (outcome, blocks.is_empty()) {
        (DomainPackRemoteRollbackOutcome::CandidatePreflightRequired, true)
        | (DomainPackRemoteRollbackOutcome::Blocked, false) => Ok(()),
        _ => Err(DomainPackRemoteAcquisitionContractError::InvalidOutcomeBlocks),
    }
}

fn package_descriptors(
    artifacts: &DomainPackRegistryArtifactSet,
) -> Vec<&DomainPackRemoteArtifactDescriptor> {
    let mut descriptors = Vec::with_capacity(3 + artifacts.fixtures.len());
    descriptors.push(&artifacts.manifest);
    descriptors.push(&artifacts.content);
    descriptors.push(&artifacts.license);
    descriptors.extend(&artifacts.fixtures);
    descriptors
}

fn is_operator_local_mirror(
    registry: &DomainPackSupplyChainRegistryDocument,
    mirror_id: &StableId,
) -> bool {
    registry
        .domain_pack_supply_chain_registry
        .mirrors
        .iter()
        .any(|mirror| {
            mirror.mirror_id == *mirror_id
                && matches!(
                    &mirror.transport,
                    DomainPackRegistryMirrorTransport::OperatorProvisionedLocal { .. }
                )
        })
}

fn validate_schema(version: &str) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    if version == DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(
            DomainPackRemoteAcquisitionContractError::UnsupportedSchemaVersion {
                found: version.to_owned(),
            },
        )
    }
}

fn required_id(
    value: &StableId,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    if value.0.trim().is_empty() {
        Err(
            DomainPackRemoteAcquisitionContractError::InvalidIdentifier {
                path: path.to_owned(),
            },
        )
    } else {
        Ok(())
    }
}

fn require_digest(value: &str, path: &str) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(DomainPackRemoteAcquisitionContractError::InvalidDigest {
            path: path.to_owned(),
        })
    }
}

#[allow(clippy::needless_pass_by_value)]
fn require_self_digest(
    authored: &str,
    expected: String,
    path: &str,
) -> Result<(), DomainPackRemoteAcquisitionContractError> {
    require_digest(authored, path)?;
    if authored == expected {
        Ok(())
    } else {
        Err(DomainPackRemoteAcquisitionContractError::DigestMismatch {
            path: path.to_owned(),
        })
    }
}

fn canonical_document_bytes<T: Serialize>(
    document: &T,
    root_key: &str,
    digest_key: &str,
) -> Result<Vec<u8>, DomainPackRemoteAcquisitionContractError> {
    let mut value = serde_json::to_value(document).map_err(|error| {
        DomainPackRemoteAcquisitionContractError::Canonicalization {
            message: error.to_string(),
        }
    })?;
    value
        .get_mut(root_key)
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|root| root.remove(digest_key))
        .ok_or_else(
            || DomainPackRemoteAcquisitionContractError::Canonicalization {
                message: format!("{root_key}.{digest_key} is absent"),
            },
        )?;
    serde_json_canonicalizer::to_vec(&value).map_err(|error| {
        DomainPackRemoteAcquisitionContractError::Canonicalization {
            message: error.to_string(),
        }
    })
}

fn canonical_document_digest<T: Serialize>(
    document: &T,
    root_key: &str,
    digest_key: &str,
) -> Result<String, DomainPackRemoteAcquisitionContractError> {
    let canonical = canonical_document_bytes(document, root_key, digest_key)?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}
