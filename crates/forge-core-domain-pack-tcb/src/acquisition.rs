//! Candidate-only remote artifact admission and confined immutable cache.
//!
//! This module admits bytes only after joining them to an opaque current supply-
//! chain selection. Its cache is intentionally outside the lifecycle namespace:
//! it retains untrusted candidate bytes and has no route to lifecycle commit
//! authority, installation, or activation.

#![allow(clippy::manual_let_else)]

use crate::{
    verify_immutable_artifact_bytes, DomainPackImmutableArtifact, ImmutableArtifactByteSemantics,
    ImmutableArtifactByteValidationError, DOMAIN_PACK_MAX_DOCUMENT_BYTES,
};
use forge_core_authority::{
    AnchoredDomainPackSupplyChainSnapshot, SelectedDomainPackSupplyChainRecord,
};
use forge_core_contracts::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackRegistryMirrorTransport,
    DomainPackRegistryPackageRecord, DomainPackRemoteArtifactDescriptor,
    DomainPackRemoteArtifactLocationBinding, DomainPackRemoteCacheEntry,
    DomainPackRemoteCachePolicy, DomainPackRemoteCacheProjection,
    DomainPackRemoteCacheProjectionDocument, DomainPackRemoteCacheProjectionOutcome,
    DomainPackRemoteFetchOutcome, DomainPackRemoteFetchReceiptDocument,
    DomainPackRemoteFetchSource, StableId, DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION,
    MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES, MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRY_BYTES,
    MAX_DOMAIN_PACK_REMOTE_CACHE_TOTAL_BYTES,
};
use forge_core_store::{acquire_effect_store_lock, RetainedEffectStoreIo};
use std::fmt;
use std::path::Path;

/// State-root-relative cache namespace. It is deliberately not under
/// `domain-packs`, so lifecycle object inventory and active-state recovery never
/// consume candidate cache data.
pub const DOMAIN_PACK_REMOTE_CACHE_RELATIVE_ROOT: &str = "domain-pack-remote-cache";
/// A dedicated root-level effect lock keeps the cache independent of lifecycle
/// state and yields descriptor-relative authority only for its own namespace.
pub const DOMAIN_PACK_REMOTE_CACHE_LOCK_RELATIVE_PATH: &str = "domain-pack-remote-cache.lock";

const CACHE_OBJECTS_IO_RELATIVE_PATH: &str = "objects";
const CACHE_PROJECTION_IO_RELATIVE_PATH: &str = "projection.json";
const CACHE_PROJECTION_MAX_BYTES: u64 = DOMAIN_PACK_MAX_DOCUMENT_BYTES;
const CACHE_ID: &str = "domain-pack-remote-cache.v1";

/// Current anchored selection and signed location needed to admit one remote
/// artifact. All fields are borrows: the opaque authorities remain owned by the
/// caller and candidate bytes do not gain lifecycle authority.
pub struct DomainPackRemoteArtifactAdmissionContext<'a> {
    pub anchored_snapshot: &'a AnchoredDomainPackSupplyChainSnapshot,
    pub selected_record: &'a SelectedDomainPackSupplyChainRecord,
    pub record: &'a DomainPackRegistryPackageRecord,
    pub receipt: &'a DomainPackRemoteFetchReceiptDocument,
    pub location: &'a DomainPackRemoteArtifactLocationBinding,
    /// Host time checked by the caller against the current anchored snapshot.
    pub checked_at_unix: u64,
}

/// Current anchored selection needed to read one cache entry. Cache lookup has
/// no transport input and never performs a fetch.
pub struct DomainPackRemoteCacheLookupContext<'a> {
    pub anchored_snapshot: &'a AnchoredDomainPackSupplyChainSnapshot,
    pub selected_record: &'a SelectedDomainPackSupplyChainRecord,
    pub record: &'a DomainPackRegistryPackageRecord,
    pub location: &'a DomainPackRemoteArtifactLocationBinding,
    /// Host time checked by the caller against the current anchored snapshot.
    pub checked_at_unix: u64,
}

/// Move-only admitted candidate bytes. It deliberately implements neither serde
/// trait nor `Clone`; bindings and bytes can only be observed by borrow. It is
/// still candidate evidence and cannot construct lifecycle commit authority.
pub struct DomainPackRemoteCandidateArtifact {
    descriptor: DomainPackRemoteArtifactDescriptor,
    raw_bytes: Vec<u8>,
    source_receipt_digest: String,
}

impl fmt::Debug for DomainPackRemoteCandidateArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainPackRemoteCandidateArtifact")
            .field("artifact_ref", &self.descriptor.binding.artifact_ref)
            .field("raw_sha256", &self.descriptor.binding.raw_sha256)
            .field("byte_length", &self.raw_bytes.len())
            .finish_non_exhaustive()
    }
}

impl DomainPackRemoteCandidateArtifact {
    #[must_use]
    pub fn binding(&self) -> &DomainPackArtifactBinding {
        &self.descriptor.binding
    }

    #[must_use]
    pub fn descriptor(&self) -> &DomainPackRemoteArtifactDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Borrow this candidate in the established lifecycle artifact shape. This
    /// does not prepare lifecycle state or mint commit authority.
    #[must_use]
    pub fn as_immutable_artifact(&self) -> DomainPackImmutableArtifact<'_> {
        DomainPackImmutableArtifact {
            binding: self.binding(),
            raw_bytes: self.raw_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum DomainPackRemoteArtifactAdmissionOutcome {
    Admitted(DomainPackRemoteCandidateArtifact),
    Stale,
    Revoked,
    DigestMismatch,
    CanonicalDigestMismatch,
    IntegrityFailure,
}

#[derive(Debug)]
pub enum DomainPackRemoteCacheLookupOutcome {
    Hit(DomainPackRemoteCandidateArtifact),
    Miss,
    Stale,
    Revoked,
    DigestMismatch,
    CanonicalDigestMismatch,
    MirrorEquivocation,
    CapacityExceeded,
    IntegrityFailure,
    StoreFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainPackRemoteCacheWriteOutcome {
    Stored,
    AlreadyPresent,
    MirrorEquivocation,
    CapacityExceeded,
    IntegrityFailure,
    StoreFailure,
}

/// Admit remote bytes as move-only candidate evidence.
///
/// Every exact catalog/selection/receipt/location join and the shared immutable
/// byte validator runs before `raw_bytes` is copied into the returned candidate.
#[must_use]
pub fn admit_domain_pack_remote_artifact(
    context: &DomainPackRemoteArtifactAdmissionContext<'_>,
    raw_bytes: &[u8],
) -> DomainPackRemoteArtifactAdmissionOutcome {
    match current_join(
        context.anchored_snapshot,
        context.selected_record,
        context.record,
        context.location,
        context.checked_at_unix,
    ) {
        CurrentJoin::Stale => return DomainPackRemoteArtifactAdmissionOutcome::Stale,
        CurrentJoin::Revoked => return DomainPackRemoteArtifactAdmissionOutcome::Revoked,
        CurrentJoin::Invalid => return DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure,
        CurrentJoin::Valid => {}
    }

    let receipt = &context.receipt.domain_pack_remote_fetch_receipt;
    if context.receipt.validate().is_err()
        || receipt.outcome != DomainPackRemoteFetchOutcome::CandidateBytesVerified
        || receipt.catalog_snapshot_digest != context.selected_record.snapshot_digest()
        || receipt.registry_record_digest != context.selected_record.record_digest()
        || receipt.package_digest != context.selected_record.package_digest()
        || receipt
            .artifacts
            .iter()
            .filter(|artifact| artifact.location == *context.location)
            .count()
            != 1
        || !receipt.artifacts.iter().any(|artifact| {
            artifact.location == *context.location
                && receipt_source_matches_current_location(
                    &artifact.source,
                    context.anchored_snapshot,
                    context.location,
                )
        })
    {
        return DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure;
    }

    match validate_descriptor_bytes(&context.location.artifact, raw_bytes) {
        Ok(()) => {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(DomainPackRemoteCandidateArtifact {
                descriptor: context.location.artifact.clone(),
                raw_bytes: raw_bytes.to_vec(),
                source_receipt_digest: receipt.receipt_digest.clone(),
            })
        }
        Err(ImmutableArtifactByteValidationError::RawDigest) => {
            DomainPackRemoteArtifactAdmissionOutcome::DigestMismatch
        }
        Err(ImmutableArtifactByteValidationError::CanonicalDigest) => {
            DomainPackRemoteArtifactAdmissionOutcome::CanonicalDigestMismatch
        }
        Err(_) => DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure,
    }
}

/// Store an admitted candidate in the separate immutable cache. Cache insertion
/// is create-new only for raw object bytes and replacement only for the
/// self-validating canonical inventory projection; neither operation touches
/// lifecycle state.
#[allow(clippy::too_many_lines)]
pub fn cache_domain_pack_remote_artifact(
    state_root: impl AsRef<Path>,
    policy: &DomainPackRemoteCachePolicy,
    artifact: &DomainPackRemoteCandidateArtifact,
) -> DomainPackRemoteCacheWriteOutcome {
    if !valid_cache_policy(policy)
        || validate_descriptor_bytes(&artifact.descriptor, artifact.raw_bytes()).is_err()
    {
        return DomainPackRemoteCacheWriteOutcome::IntegrityFailure;
    }

    let state_root = match crate::canonical_state_root(state_root.as_ref()) {
        Ok(root) => root,
        Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
    };
    let lock =
        match acquire_effect_store_lock(&state_root, DOMAIN_PACK_REMOTE_CACHE_LOCK_RELATIVE_PATH) {
            Ok(lock) => lock,
            Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
        };
    let root_io = match lock.retained_store_io() {
        Ok(io) => io,
        Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
    };
    let cache_io =
        match root_io.retain_subdirectory(Path::new(DOMAIN_PACK_REMOTE_CACHE_RELATIVE_ROOT)) {
            Ok(io) => io,
            Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
        };
    let objects_io = match cache_io.retain_subdirectory(Path::new(CACHE_OBJECTS_IO_RELATIVE_PATH)) {
        Ok(io) => io,
        Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
    };

    let mut projection_expected = match cache_io.retain_file_crash_safe_expected_leaf(
        Path::new(CACHE_PROJECTION_IO_RELATIVE_PATH),
        CACHE_PROJECTION_MAX_BYTES,
    ) {
        Ok(expected) => expected,
        Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
    };
    let existing_projection = match projection_expected.raw_bytes() {
        Some(raw) => parse_projection(raw),
        None => empty_projection(policy.clone()),
    };
    let mut projection = match existing_projection {
        Ok(projection) => projection,
        Err(()) => return DomainPackRemoteCacheWriteOutcome::IntegrityFailure,
    };
    if &projection.domain_pack_remote_cache_projection.policy != policy {
        return DomainPackRemoteCacheWriteOutcome::IntegrityFailure;
    }

    let key = &artifact.descriptor.binding.raw_sha256;
    let entries = &mut projection.domain_pack_remote_cache_projection.entries;
    if let Some(existing) = entries
        .iter()
        .find(|entry| entry.cache_key_raw_sha256 == *key)
    {
        if existing.artifact != artifact.descriptor {
            return DomainPackRemoteCacheWriteOutcome::MirrorEquivocation;
        }
        let token = match raw_digest_token(key) {
            Some(token) => token,
            None => return DomainPackRemoteCacheWriteOutcome::IntegrityFailure,
        };
        let existing_raw =
            match objects_io.read_optional_bounded(Path::new(token), existing.byte_length) {
                Ok(Some(witness)) => witness.raw_bytes().to_vec(),
                Ok(None) | Err(_) => {
                    return DomainPackRemoteCacheWriteOutcome::IntegrityFailure;
                }
            };
        if validate_descriptor_bytes(&existing.artifact, &existing_raw).is_err() {
            return DomainPackRemoteCacheWriteOutcome::IntegrityFailure;
        }
        return if existing_raw == artifact.raw_bytes() {
            DomainPackRemoteCacheWriteOutcome::AlreadyPresent
        } else {
            DomainPackRemoteCacheWriteOutcome::MirrorEquivocation
        };
    }

    let DomainPackRemoteCachePolicy::RejectOnFull {
        max_entry_bytes,
        max_entries,
        max_total_bytes,
    } = policy;
    let byte_length = u64::try_from(artifact.raw_bytes().len()).unwrap_or(u64::MAX);
    let total_bytes = projection.domain_pack_remote_cache_projection.total_bytes;
    if byte_length > *max_entry_bytes
        || entries.len() >= usize::try_from(*max_entries).unwrap_or(usize::MAX)
        || total_bytes
            .checked_add(byte_length)
            .is_none_or(|total| total > *max_total_bytes)
    {
        return DomainPackRemoteCacheWriteOutcome::CapacityExceeded;
    }

    let token = match raw_digest_token(key) {
        Some(token) => token,
        None => return DomainPackRemoteCacheWriteOutcome::IntegrityFailure,
    };
    match objects_io.read_optional_bounded(Path::new(token), byte_length) {
        Ok(Some(existing)) => {
            if existing.raw_bytes() != artifact.raw_bytes() {
                return DomainPackRemoteCacheWriteOutcome::MirrorEquivocation;
            }
        }
        Ok(None) => {
            if objects_io
                .write_new_file_synced(Path::new(token), artifact.raw_bytes(), byte_length)
                .is_err()
            {
                return DomainPackRemoteCacheWriteOutcome::StoreFailure;
            }
        }
        Err(_) => return DomainPackRemoteCacheWriteOutcome::StoreFailure,
    }

    let entry = match new_entry(artifact) {
        Ok(entry) => entry,
        Err(()) => return DomainPackRemoteCacheWriteOutcome::IntegrityFailure,
    };
    entries.push(entry);
    projection.domain_pack_remote_cache_projection.total_bytes =
        match total_bytes.checked_add(byte_length) {
            Some(total) => total,
            None => return DomainPackRemoteCacheWriteOutcome::CapacityExceeded,
        };
    let projection_raw = match finalize_projection(&mut projection) {
        Ok(raw) => raw,
        Err(()) => return DomainPackRemoteCacheWriteOutcome::IntegrityFailure,
    };
    if cache_io
        .replace_file_crash_safe(
            Path::new(CACHE_PROJECTION_IO_RELATIVE_PATH),
            &mut projection_expected,
            &projection_raw,
            CACHE_PROJECTION_MAX_BYTES,
        )
        .is_err()
    {
        return DomainPackRemoteCacheWriteOutcome::StoreFailure;
    }
    DomainPackRemoteCacheWriteOutcome::Stored
}

/// Read one exact artifact from the local cache and revalidate it against the
/// current opaque anchor and selected record. This function contains no network
/// operation; miss, stale, and revoked results cannot become a hit.
pub fn lookup_cached_domain_pack_remote_artifact(
    state_root: impl AsRef<Path>,
    context: &DomainPackRemoteCacheLookupContext<'_>,
) -> DomainPackRemoteCacheLookupOutcome {
    match current_join(
        context.anchored_snapshot,
        context.selected_record,
        context.record,
        context.location,
        context.checked_at_unix,
    ) {
        CurrentJoin::Stale => return DomainPackRemoteCacheLookupOutcome::Stale,
        CurrentJoin::Revoked => return DomainPackRemoteCacheLookupOutcome::Revoked,
        CurrentJoin::Invalid => return DomainPackRemoteCacheLookupOutcome::IntegrityFailure,
        CurrentJoin::Valid => {}
    }

    let state_root = match crate::canonical_state_root(state_root.as_ref()) {
        Ok(root) => root,
        Err(_) => return DomainPackRemoteCacheLookupOutcome::StoreFailure,
    };
    let lock =
        match acquire_effect_store_lock(&state_root, DOMAIN_PACK_REMOTE_CACHE_LOCK_RELATIVE_PATH) {
            Ok(lock) => lock,
            Err(_) => return DomainPackRemoteCacheLookupOutcome::StoreFailure,
        };
    let root_io = match lock.retained_store_io() {
        Ok(io) => io,
        Err(_) => return DomainPackRemoteCacheLookupOutcome::StoreFailure,
    };
    let cache_io =
        match root_io.retain_subdirectory(Path::new(DOMAIN_PACK_REMOTE_CACHE_RELATIVE_ROOT)) {
            Ok(io) => io,
            Err(_) => return DomainPackRemoteCacheLookupOutcome::StoreFailure,
        };
    let objects_io = match cache_io.retain_subdirectory(Path::new(CACHE_OBJECTS_IO_RELATIVE_PATH)) {
        Ok(io) => io,
        Err(_) => return DomainPackRemoteCacheLookupOutcome::StoreFailure,
    };
    let projection = match read_projection(&cache_io) {
        Ok(Some(projection)) => projection,
        Ok(None) => return DomainPackRemoteCacheLookupOutcome::Miss,
        Err(()) => return DomainPackRemoteCacheLookupOutcome::IntegrityFailure,
    };

    let key = &context.location.artifact.binding.raw_sha256;
    let Some(entry) = projection
        .domain_pack_remote_cache_projection
        .entries
        .iter()
        .find(|entry| entry.cache_key_raw_sha256 == *key)
    else {
        return DomainPackRemoteCacheLookupOutcome::Miss;
    };
    if entry.artifact != context.location.artifact {
        return DomainPackRemoteCacheLookupOutcome::MirrorEquivocation;
    }
    let Some(token) = raw_digest_token(key) else {
        return DomainPackRemoteCacheLookupOutcome::IntegrityFailure;
    };
    let raw_bytes = match objects_io.read_optional_bounded(Path::new(token), entry.byte_length) {
        Ok(Some(witness)) => witness.raw_bytes().to_vec(),
        Ok(None) | Err(_) => return DomainPackRemoteCacheLookupOutcome::IntegrityFailure,
    };
    match validate_descriptor_bytes(&entry.artifact, &raw_bytes) {
        Ok(()) => DomainPackRemoteCacheLookupOutcome::Hit(DomainPackRemoteCandidateArtifact {
            descriptor: entry.artifact.clone(),
            raw_bytes,
            source_receipt_digest: entry.source_receipt_digest.clone(),
        }),
        Err(ImmutableArtifactByteValidationError::RawDigest) => {
            DomainPackRemoteCacheLookupOutcome::DigestMismatch
        }
        Err(ImmutableArtifactByteValidationError::CanonicalDigest) => {
            DomainPackRemoteCacheLookupOutcome::CanonicalDigestMismatch
        }
        Err(_) => DomainPackRemoteCacheLookupOutcome::IntegrityFailure,
    }
}

/// Explicit offline spelling for callers that need a cache-only acquisition
/// boundary. It delegates to the no-network lookup above.
pub fn lookup_cached_domain_pack_remote_artifact_offline(
    state_root: impl AsRef<Path>,
    context: &DomainPackRemoteCacheLookupContext<'_>,
) -> DomainPackRemoteCacheLookupOutcome {
    lookup_cached_domain_pack_remote_artifact(state_root, context)
}

/// Network receipt evidence is admissible only from the exact signed HTTPS
/// mirror. Operator-local receipt evidence is deliberately rejected here because
/// this candidate-only context carries no exact operator-local-head capability.
fn receipt_source_matches_current_location(
    source: &DomainPackRemoteFetchSource,
    anchored: &AnchoredDomainPackSupplyChainSnapshot,
    location: &DomainPackRemoteArtifactLocationBinding,
) -> bool {
    let Some(mirror) = anchored
        .verified_snapshot()
        .mirrors()
        .iter()
        .find(|mirror| mirror.mirror_id == location.mirror_id)
    else {
        return false;
    };
    matches!(
        (&mirror.transport, source),
        (
            DomainPackRegistryMirrorTransport::Https { .. },
            DomainPackRemoteFetchSource::NetworkMirror { mirror_id },
        ) if mirror_id == &location.mirror_id
    )
}

enum CurrentJoin {
    Valid,
    Stale,
    Revoked,
    Invalid,
}

fn current_join(
    anchored: &AnchoredDomainPackSupplyChainSnapshot,
    selected: &SelectedDomainPackSupplyChainRecord,
    record: &DomainPackRegistryPackageRecord,
    location: &DomainPackRemoteArtifactLocationBinding,
    checked_at_unix: u64,
) -> CurrentJoin {
    let snapshot = anchored.verified_snapshot();
    if checked_at_unix >= snapshot.expires_at_unix() {
        return CurrentJoin::Stale;
    }
    // Evaluate cumulative revocation before considering any historical selected
    // capability. A stale selection cannot turn a currently revoked record into
    // a usable cache candidate.
    if anchored.is_currently_revoked(&record.record_digest) {
        return CurrentJoin::Revoked;
    }
    if selected.registry_id() != snapshot.registry_id()
        || selected.audience() != snapshot.audience()
        || selected.generation() != snapshot.generation()
        || selected.snapshot_digest() != snapshot.snapshot_digest()
        || selected.trust_policy_digest() != snapshot.trust_policy_digest()
        || selected.record_digest() != record.record_digest
        || selected.package_digest() != record.package_digest
        || selected.content_digest() != record.artifacts.content.binding.canonical_sha256
        || !snapshot
            .entries()
            .iter()
            .any(|entry| entry.record() == record)
        || !snapshot
            .mirrors()
            .iter()
            .any(|mirror| mirror.mirror_id == location.mirror_id)
        || location.object_path != location.artifact.object_path
        || !record_contains_descriptor(record, &location.artifact)
    {
        return CurrentJoin::Invalid;
    }
    CurrentJoin::Valid
}

fn record_contains_descriptor(
    record: &DomainPackRegistryPackageRecord,
    descriptor: &DomainPackRemoteArtifactDescriptor,
) -> bool {
    let descriptors = std::iter::once(&record.artifacts.manifest)
        .chain(std::iter::once(&record.artifacts.content))
        .chain(std::iter::once(&record.artifacts.license))
        .chain(record.artifacts.fixtures.iter());
    descriptors.clone().any(|candidate| candidate == descriptor)
        && record.artifacts.manifest.binding.raw_sha256 == record.manifest_digest
        && record.artifacts.content.binding.raw_sha256 == record.content_digest
        && record.artifacts.license.binding.raw_sha256 == record.license_digest
        && record.artifacts.fixtures.len() == record.fixture_digests.len()
        && record
            .artifacts
            .fixtures
            .iter()
            .zip(&record.fixture_digests)
            .all(|(fixture, digest)| fixture.binding.raw_sha256 == *digest)
}

fn validate_descriptor_bytes(
    descriptor: &DomainPackRemoteArtifactDescriptor,
    raw_bytes: &[u8],
) -> Result<(), ImmutableArtifactByteValidationError> {
    verify_immutable_artifact_bytes(
        &descriptor.binding,
        raw_bytes,
        Some(descriptor.byte_length),
        ImmutableArtifactByteSemantics::Remote(descriptor.media_type),
    )
}

fn raw_digest_token(digest: &str) -> Option<&str> {
    let token = digest.strip_prefix("sha256:")?;
    if token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Some(token)
    } else {
        None
    }
}

fn valid_cache_policy(policy: &DomainPackRemoteCachePolicy) -> bool {
    match policy {
        DomainPackRemoteCachePolicy::RejectOnFull {
            max_entry_bytes,
            max_entries,
            max_total_bytes,
        } => {
            *max_entry_bytes > 0
                && *max_entries > 0
                && *max_entries <= MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES
                && *max_total_bytes >= *max_entry_bytes
                && *max_entry_bytes <= MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRY_BYTES
                && *max_total_bytes <= MAX_DOMAIN_PACK_REMOTE_CACHE_TOTAL_BYTES
        }
    }
}

fn empty_projection(
    policy: DomainPackRemoteCachePolicy,
) -> Result<DomainPackRemoteCacheProjectionDocument, ()> {
    let mut projection = DomainPackRemoteCacheProjectionDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_projection: DomainPackRemoteCacheProjection {
            cache_id: StableId(CACHE_ID.to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            policy,
            entries: Vec::new(),
            total_bytes: 0,
            outcome: DomainPackRemoteCacheProjectionOutcome::CandidateBytesMissing,
            blocks: Vec::new(),
            projection_digest: String::new(),
        },
    };
    finalize_projection(&mut projection)?;
    Ok(projection)
}

fn new_entry(
    artifact: &DomainPackRemoteCandidateArtifact,
) -> Result<DomainPackRemoteCacheEntry, ()> {
    let mut document = forge_core_contracts::DomainPackRemoteCacheEntryDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_entry: DomainPackRemoteCacheEntry {
            cache_key_raw_sha256: artifact.descriptor.binding.raw_sha256.clone(),
            artifact: artifact.descriptor.clone(),
            byte_length: u64::try_from(artifact.raw_bytes().len()).map_err(|_| ())?,
            source_receipt_digest: artifact.source_receipt_digest.clone(),
            cached_at_unix: crate::trusted_now_unix().map_err(|_| ())?,
            entry_digest: String::new(),
        },
    };
    document.domain_pack_remote_cache_entry.entry_digest =
        document.entry_digest().map_err(|_| ())?;
    document.validate().map_err(|_| ())?;
    Ok(document.domain_pack_remote_cache_entry)
}

fn finalize_projection(
    projection: &mut DomainPackRemoteCacheProjectionDocument,
) -> Result<Vec<u8>, ()> {
    projection.domain_pack_remote_cache_projection.outcome = if projection
        .domain_pack_remote_cache_projection
        .entries
        .is_empty()
    {
        DomainPackRemoteCacheProjectionOutcome::CandidateBytesMissing
    } else {
        DomainPackRemoteCacheProjectionOutcome::CandidateBytesPresent
    };
    projection
        .domain_pack_remote_cache_projection
        .blocks
        .clear();
    projection
        .domain_pack_remote_cache_projection
        .projection_digest = projection.projection_digest().map_err(|_| ())?;
    validate_projection(projection)?;
    serde_json_canonicalizer::to_vec(projection).map_err(|_| ())
}

fn parse_projection(raw: &[u8]) -> Result<DomainPackRemoteCacheProjectionDocument, ()> {
    let projection: DomainPackRemoteCacheProjectionDocument =
        serde_json::from_slice(raw).map_err(|_| ())?;
    validate_projection(&projection)?;
    Ok(projection)
}

fn validate_projection(projection: &DomainPackRemoteCacheProjectionDocument) -> Result<(), ()> {
    projection.validate().map_err(|_| ())?;
    if !valid_cache_policy(&projection.domain_pack_remote_cache_projection.policy) {
        return Err(());
    }
    for entry in &projection.domain_pack_remote_cache_projection.entries {
        let document = forge_core_contracts::DomainPackRemoteCacheEntryDocument {
            schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
            domain_pack_remote_cache_entry: entry.clone(),
        };
        document.validate().map_err(|_| ())?;
    }
    Ok(())
}

fn read_projection(
    cache_io: &RetainedEffectStoreIo<'_>,
) -> Result<Option<DomainPackRemoteCacheProjectionDocument>, ()> {
    let session = cache_io
        .reconcile_file_crash_safe(
            Path::new(CACHE_PROJECTION_IO_RELATIVE_PATH),
            CACHE_PROJECTION_MAX_BYTES,
        )
        .map_err(|_| ())?;
    let Some(read) = session.read_exact().map_err(|_| ())? else {
        return Ok(None);
    };
    parse_projection(read.raw_bytes()).map(Some)
}
