#![allow(clippy::missing_errors_doc)]

//! Pure C6.2 candidate-byte remote Domain Pack acquisition decisions.
//!
//! This module plans and verifies immutable candidate bytes only. It does not
//! verify catalog signatures, establish an anchor, trust a package, mutate a
//! cache, install a package, advance lifecycle state, or activate anything.
//! Catalog verification and host time are represented as caller-supplied facts
//! so this pure layer can fail closed without treating those facts as authority.

use crate::{plan_domain_pack_acquisition, verify_domain_pack_acquisition_plan};
use forge_core_contracts::{
    domain_pack_remote_signed_mirror_order, DomainPackAcquisitionPlanDocument,
    DomainPackCandidateAuthority, DomainPackRegistryMirrorTransport,
    DomainPackRemoteAcquisitionBlock, DomainPackRemoteAcquisitionPlan,
    DomainPackRemoteAcquisitionPlanDocument, DomainPackRemoteAcquisitionPlanOutcome,
    DomainPackRemoteAcquisitionRequestDocument, DomainPackRemoteArtifactDescriptor,
    DomainPackRemoteArtifactLocationBinding, DomainPackRemoteArtifactMediaType,
    DomainPackRemoteCacheEntry, DomainPackRemoteCacheEntryDocument,
    DomainPackRemoteCacheProjectionDocument, DomainPackRemoteFetchEvidence,
    DomainPackRemoteFetchEvidenceDocument, DomainPackRemoteFetchOutcome,
    DomainPackRemoteFetchReceipt, DomainPackRemoteFetchReceiptDocument,
    DomainPackRemoteFetchSource, DomainPackRemoteFetchedArtifactReceipt,
    DomainPackRemoteNetworkMode, DomainPackRemoteUntrustedFetchObservation,
    DomainPackRemoteUntrustedFetchObservationDocument, StableId,
    DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// A caller-supplied result from the catalog signature/anchor and host-time
/// boundary. It is intentionally an opaque fact to this module: representing
/// `CurrentAnchored` here never supplies the signing or anchoring authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainPackRemoteCatalogAvailability {
    CurrentAnchored,
    Unavailable,
    NotAnchored,
    Stale,
    Revoked,
    SignatureTamper,
}

/// Non-authoritative catalog facts bound to the exact requested snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteCatalogFacts {
    pub snapshot_digest: String,
    pub availability: DomainPackRemoteCatalogAvailability,
    /// A time obtained at the host/TCB boundary, not from the catalog or cache.
    pub host_checked_at_unix: u64,
}

/// Full pure input for remote candidate-byte planning. `acquisition` is kept
/// alongside its expected plan so discovery is replayed instead of trusting
/// copied discovery digest strings in the remote request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteAcquisitionPlanningInput {
    pub acquisition: forge_core_contracts::DomainPackAcquisitionPlanningInput,
    pub acquisition_plan: DomainPackAcquisitionPlanDocument,
    pub request: DomainPackRemoteAcquisitionRequestDocument,
    /// A cache projection is only a raw-digest availability hint. Invalid or
    /// mismatched cache data becomes a miss/tamper disposition; it is never
    /// promoted into catalog or package authority.
    pub cache_projection: Option<DomainPackRemoteCacheProjectionDocument>,
    pub catalog_facts: DomainPackRemoteCatalogFacts,
}

/// Per-artifact cache result computed against the exact current catalog record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackRemoteCacheDisposition {
    Hit { cache_key_raw_sha256: String },
    Miss,
    Tampered,
}

/// A cache read that still needs byte verification. This is not a cache hit in
/// the cryptographic sense: it merely names a raw-digest-keyed candidate byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemotePlannedCacheRead {
    pub location: DomainPackRemoteArtifactLocationBinding,
    pub source: DomainPackRemoteFetchSource,
}

/// One signed transport location in the sole permitted mirror order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemotePlannedTransportAttempt {
    pub location: DomainPackRemoteArtifactLocationBinding,
    pub source: DomainPackRemoteFetchSource,
}

/// The deterministic planning result. `plan` is the portable contract document;
/// the cache and transport lists make its caller-specific candidate-byte work
/// explicit without serializing new authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteAcquisitionPlanningDecision {
    pub plan: DomainPackRemoteAcquisitionPlanDocument,
    pub cache_dispositions: Vec<DomainPackRemoteCacheDisposition>,
    pub cache_reads: Vec<DomainPackRemotePlannedCacheRead>,
    /// All fallback locations in contract-defined signed mirror order.
    pub transport_attempts: Vec<DomainPackRemotePlannedTransportAttempt>,
    /// Only actual network-mirror reads appear here. Offline plans always have
    /// an empty list; cache reads are separately represented above.
    pub network_fetches: Vec<DomainPackRemotePlannedTransportAttempt>,
    /// Operator-local reads are separately visible so an offline caller cannot
    /// mistake an opaque local location for a network endpoint.
    pub local_mirror_reads: Vec<DomainPackRemotePlannedTransportAttempt>,
}

/// A physical acquisition attempt supplied to the byte verifier. The bytes are
/// transient input to this pure function and are not serialized into contract
/// evidence; verified bytes are carried only in the returned handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackRemoteFetchAttempt {
    TransportFailure {
        location: DomainPackRemoteArtifactLocationBinding,
        source: DomainPackRemoteFetchSource,
    },
    Observation {
        observation: DomainPackRemoteUntrustedFetchObservationDocument,
        raw_bytes: Vec<u8>,
    },
}

/// Input that replays planning before accepting observations. A persisted plan
/// alone is insufficient because cache dispositions and fallback locations must
/// be reconstructed from the exact request/catalog/discovery bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteFetchVerificationInput {
    pub planning: DomainPackRemoteAcquisitionPlanningInput,
    pub plan: DomainPackRemoteAcquisitionPlanDocument,
    pub attempts: Vec<DomainPackRemoteFetchAttempt>,
}

/// One retained verified candidate artifact. It deliberately repeats only the
/// immutable descriptor binding, receipt metadata, and raw bytes; it does not
/// contain a trust, install, commit, or activation field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteVerifiedArtifact {
    pub receipt: DomainPackRemoteFetchedArtifactReceipt,
    pub raw_bytes: Vec<u8>,
}

/// Explicit handoff for the existing acquisition-derivation and later TCB
/// owners. Consumers must independently parse/admit these bytes and apply their
/// own signature, trust, review, preflight, and lifecycle controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteAcquisitionHandoff {
    pub plan: DomainPackRemoteAcquisitionPlanDocument,
    pub evidence: DomainPackRemoteFetchEvidenceDocument,
    pub receipt: DomainPackRemoteFetchReceiptDocument,
    /// Every supplied attempt, including transport failures and rejected byte
    /// observations, remains available as candidate-only diagnostics.
    pub attempts: Vec<DomainPackRemoteFetchAttempt>,
    /// This is non-empty only for a complete verified artifact set.
    pub verified_artifacts: Vec<DomainPackRemoteVerifiedArtifact>,
    /// Stable descriptor-set identity independent of the mirror that supplied
    /// identical pinned bytes.
    pub artifact_set_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DomainPackRemoteAcquisitionIssueCode {
    InvalidAcquisitionPlan,
    DiscoveryReplayMismatch,
    InvalidRemoteRequest,
    CandidateAuthorityViolation,
    DiscoveryBindingMismatch,
    CatalogPackageBindingMismatch,
    InvalidPlan,
    PlanReplayMismatch,
    CanonicalizationFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteAcquisitionIssue {
    pub code: DomainPackRemoteAcquisitionIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRemoteAcquisitionRejection {
    pub issues: Vec<DomainPackRemoteAcquisitionIssue>,
}

/// Plan deterministic, candidate-only cache reads and signed transport attempts.
///
/// Expected catalog availability, expiry, revocation, absent anchors, and absent
/// offline bytes are represented as closed contract blocks in the returned plan.
/// Structural request or discovery replay failures return typed rejections.
pub fn plan_domain_pack_remote_acquisition(
    input: &DomainPackRemoteAcquisitionPlanningInput,
) -> Result<DomainPackRemoteAcquisitionPlanningDecision, DomainPackRemoteAcquisitionRejection> {
    validate_planning_input(input)?;

    let request = &input.request.domain_pack_remote_acquisition_request;
    let descriptors = record_descriptors(&request.package.record.artifacts)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let ordered_mirrors = domain_pack_remote_signed_mirror_order(&request.catalog.registry)
        .map_err(|error| {
            rejection(
                DomainPackRemoteAcquisitionIssueCode::InvalidRemoteRequest,
                "request.catalog.registry",
                format!("remote catalog metadata failed validation: {error:?}"),
            )
        })?;

    let primary_mirrors = eligible_mirrors(request.network_mode, &ordered_mirrors, request);
    let fallback_mirrors = fallback_mirrors(request.network_mode, &ordered_mirrors, request);
    let locations = descriptors
        .iter()
        .map(|artifact| DomainPackRemoteArtifactLocationBinding {
            artifact: artifact.clone(),
            mirror_id: primary_mirrors
                .first()
                .or_else(|| ordered_mirrors.first())
                .map_or_else(
                    || StableId("remote.no-signed-mirror".to_owned()),
                    |mirror| mirror.mirror_id.clone(),
                ),
            object_path: artifact.object_path.clone(),
        })
        .collect::<Vec<_>>();

    let cache_dispositions = cache_dispositions(input, &descriptors);
    let mut blocks = catalog_blocks(input, request);
    if request.network_mode == DomainPackRemoteNetworkMode::OfflineOnly
        && input.catalog_facts.host_checked_at_unix
            >= request
                .operator_anchored_local_head
                .as_ref()
                .map_or(0, |head| head.expires_at_unix)
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::OfflineLocalHeadStale);
        blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogExpired);
    }

    let all_cache_hits = cache_dispositions
        .iter()
        .all(|disposition| matches!(disposition, DomainPackRemoteCacheDisposition::Hit { .. }));
    // OnlineRequired deliberately does not schedule a cache read, even when a
    // cache projection happens to contain the exact raw digest. The disposition
    // remains diagnostic-only in that mode; only PreferCache and OfflineOnly may
    // present cache bytes to the verifier.
    let mut cache_reads = if request.network_mode == DomainPackRemoteNetworkMode::OnlineRequired {
        Vec::new()
    } else {
        locations
            .iter()
            .zip(&cache_dispositions)
            .filter_map(|(location, disposition)| match disposition {
                DomainPackRemoteCacheDisposition::Hit {
                    cache_key_raw_sha256,
                } => Some(DomainPackRemotePlannedCacheRead {
                    location: location.clone(),
                    source: DomainPackRemoteFetchSource::Cache {
                        cache_key_raw_sha256: cache_key_raw_sha256.clone(),
                    },
                }),
                DomainPackRemoteCacheDisposition::Miss
                | DomainPackRemoteCacheDisposition::Tampered => None,
            })
            .collect::<Vec<_>>()
    };

    if request.network_mode == DomainPackRemoteNetworkMode::OfflineOnly && !all_cache_hits {
        blocks.insert(DomainPackRemoteAcquisitionBlock::OfflineExactBytesMissing);
        blocks.insert(DomainPackRemoteAcquisitionBlock::CacheMiss);
        if cache_dispositions
            .iter()
            .any(|disposition| matches!(disposition, DomainPackRemoteCacheDisposition::Tampered))
        {
            blocks.insert(DomainPackRemoteAcquisitionBlock::CacheTamper);
        }
    }

    let mut transport_attempts = Vec::new();
    let mut network_fetches = Vec::new();
    let mut local_mirror_reads = Vec::new();
    if blocks.is_empty() {
        let needs_transport = match request.network_mode {
            DomainPackRemoteNetworkMode::OnlineRequired => vec![true; descriptors.len()],
            DomainPackRemoteNetworkMode::PreferCache => cache_dispositions
                .iter()
                .map(|disposition| {
                    !matches!(disposition, DomainPackRemoteCacheDisposition::Hit { .. })
                })
                .collect(),
            DomainPackRemoteNetworkMode::OfflineOnly => vec![false; descriptors.len()],
        };
        for (descriptor, needs_transport) in descriptors.iter().zip(needs_transport) {
            if !needs_transport {
                continue;
            }
            for mirror in &fallback_mirrors {
                let location = DomainPackRemoteArtifactLocationBinding {
                    artifact: descriptor.clone(),
                    mirror_id: mirror.mirror_id.clone(),
                    object_path: descriptor.object_path.clone(),
                };
                let source = source_for_mirror(mirror, request);
                let planned = DomainPackRemotePlannedTransportAttempt { location, source };
                transport_attempts.push(planned.clone());
                match &planned.source {
                    DomainPackRemoteFetchSource::NetworkMirror { .. } => {
                        network_fetches.push(planned);
                    }
                    DomainPackRemoteFetchSource::OperatorAnchoredLocalMirror { .. } => {
                        local_mirror_reads.push(planned);
                    }
                    DomainPackRemoteFetchSource::Cache { .. } => {
                        unreachable!("mirror sources are never cache")
                    }
                }
            }
        }
        if request.network_mode == DomainPackRemoteNetworkMode::OnlineRequired
            && network_fetches.is_empty()
        {
            blocks.insert(DomainPackRemoteAcquisitionBlock::NetworkDenied);
        }
        if request.network_mode == DomainPackRemoteNetworkMode::PreferCache
            && !all_cache_hits
            && network_fetches.is_empty()
            && local_mirror_reads.is_empty()
        {
            blocks.insert(DomainPackRemoteAcquisitionBlock::NetworkDenied);
        }
    }

    // A blocked plan exposes dispositions as diagnostics only. It must not leak
    // any cache or transport work as a schedulable byte-acquisition action.
    if !blocks.is_empty() {
        cache_reads.clear();
        transport_attempts.clear();
        network_fetches.clear();
        local_mirror_reads.clear();
    }

    let (outcome, blocks) = if blocks.is_empty() {
        (
            if request.network_mode == DomainPackRemoteNetworkMode::OfflineOnly
                || (request.network_mode == DomainPackRemoteNetworkMode::PreferCache
                    && all_cache_hits)
            {
                DomainPackRemoteAcquisitionPlanOutcome::CacheOnlyCandidateBytesRequired
            } else {
                DomainPackRemoteAcquisitionPlanOutcome::CandidateBytesRequired
            },
            Vec::new(),
        )
    } else {
        (
            DomainPackRemoteAcquisitionPlanOutcome::Blocked,
            blocks.into_iter().collect(),
        )
    };

    let mut plan = DomainPackRemoteAcquisitionPlan {
        plan_id: derived_id("plan", &request.request_digest),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        request_digest: request.request_digest.clone(),
        catalog: request.catalog.clone(),
        discovery: request.discovery.clone(),
        package: request.package.clone(),
        network_mode: request.network_mode,
        mirror_policy: request.mirror_policy,
        cache_policy: request.cache_policy.clone(),
        operator_anchored_local_head: request.operator_anchored_local_head.clone(),
        artifacts: locations,
        outcome,
        blocks,
        plan_digest: String::new(),
    };
    let mut document = DomainPackRemoteAcquisitionPlanDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_acquisition_plan: plan.clone(),
    };
    plan.plan_digest = document.plan_digest().map_err(|error| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::CanonicalizationFailure,
            "plan",
            format!("could not canonicalize remote plan: {error:?}"),
        )
    })?;
    document.domain_pack_remote_acquisition_plan = plan;
    document.validate().map_err(|error| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidPlan,
            "plan",
            format!("derived remote plan failed validation: {error:?}"),
        )
    })?;

    Ok(DomainPackRemoteAcquisitionPlanningDecision {
        plan: document,
        cache_dispositions,
        cache_reads,
        transport_attempts,
        network_fetches,
        local_mirror_reads,
    })
}

/// Replay a planning input and require the exact portable plan document.
#[must_use]
pub fn verify_domain_pack_remote_acquisition_plan(
    input: &DomainPackRemoteAcquisitionPlanningInput,
    document: &DomainPackRemoteAcquisitionPlanDocument,
) -> bool {
    document.validate().is_ok()
        && plan_domain_pack_remote_acquisition(input)
            .as_ref()
            .is_ok_and(|decision| &decision.plan == document)
}

/// Verify untrusted attempts against an exactly replayed plan and construct
/// candidate-only evidence, a candidate-only receipt, and explicit raw bytes for
/// later authority owners. Transport failure alone may advance to the next signed
/// location. Any binding/digest/length/media/canonical disagreement blocks the
/// entire candidate set and no rejected bytes enter `verified_artifacts`.
pub fn verify_domain_pack_remote_fetches(
    input: &DomainPackRemoteFetchVerificationInput,
) -> Result<DomainPackRemoteAcquisitionHandoff, DomainPackRemoteAcquisitionRejection> {
    input.plan.validate().map_err(|error| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidPlan,
            "plan",
            format!("remote plan failed validation: {error:?}"),
        )
    })?;
    let decision = plan_domain_pack_remote_acquisition(&input.planning)?;
    if decision.plan != input.plan {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::PlanReplayMismatch,
            "plan",
            "presented plan is not the exact deterministic result of the planning input",
        ));
    }

    let plan = &input.plan.domain_pack_remote_acquisition_plan;
    let artifact_set_digest = remote_artifact_set_digest(&plan.artifacts).map_err(|message| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::CanonicalizationFailure,
            "plan.artifacts",
            message,
        )
    })?;
    if plan.outcome == DomainPackRemoteAcquisitionPlanOutcome::Blocked {
        if !input.attempts.is_empty() {
            return Err(rejection(
                DomainPackRemoteAcquisitionIssueCode::PlanReplayMismatch,
                "attempts",
                "blocked remote plans cannot schedule candidate-byte attempts",
            ));
        }
        return blocked_handoff(
            input.plan.clone(),
            input.attempts.clone(),
            Vec::new(),
            plan.blocks.clone(),
            artifact_set_digest,
        );
    }

    let schedules = expected_schedules(&decision);
    let mut current_artifact = 0_usize;
    let mut current_source = 0_usize;
    let mut evidence_observations = Vec::new();
    let mut verified = Vec::new();
    let mut blocks = BTreeSet::new();
    let mut compromised = false;

    for attempt in &input.attempts {
        let Some(schedule) = schedules.get(current_artifact) else {
            blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
            collect_evidence_observation(attempt, &mut evidence_observations);
            continue;
        };
        let Some(expected) = schedule.sources.get(current_source) else {
            blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
            collect_evidence_observation(attempt, &mut evidence_observations);
            continue;
        };
        if !attempt_matches_expected(attempt, expected) {
            blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
            blocks.insert(DomainPackRemoteAcquisitionBlock::MirrorEquivocation);
            collect_evidence_observation(attempt, &mut evidence_observations);
            compromised = true;
            continue;
        }

        match attempt {
            DomainPackRemoteFetchAttempt::TransportFailure { .. } => {
                if current_source + 1 < schedule.sources.len() {
                    current_source += 1;
                } else {
                    blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
                    current_artifact += 1;
                    current_source = 0;
                }
            }
            DomainPackRemoteFetchAttempt::Observation {
                observation,
                raw_bytes,
            } => {
                // Contract evidence may contain only structurally valid portable
                // observations. Keep malformed untrusted documents solely in the
                // non-serialized attempt diagnostics, while still blocking the
                // handoff below.
                if observation.validate().is_ok() {
                    evidence_observations.push(
                        observation
                            .domain_pack_remote_untrusted_fetch_observation
                            .clone(),
                    );
                }
                let observation_blocks = observation_blocks(plan, expected, observation, raw_bytes);
                if observation_blocks.is_empty() && !compromised {
                    verified.push(DomainPackRemoteVerifiedArtifact {
                        receipt: DomainPackRemoteFetchedArtifactReceipt {
                            location: expected.location.clone(),
                            source: expected.source.clone(),
                            raw_sha256: expected.location.artifact.binding.raw_sha256.clone(),
                            canonical_sha256: expected
                                .location
                                .artifact
                                .binding
                                .canonical_sha256
                                .clone(),
                            byte_length: expected.location.artifact.byte_length,
                            media_type: expected.location.artifact.media_type,
                        },
                        raw_bytes: raw_bytes.clone(),
                    });
                    current_artifact += 1;
                    current_source = 0;
                } else {
                    compromised = true;
                    blocks.extend(observation_blocks);
                }
            }
        }
    }

    if current_artifact != schedules.len() {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
    }
    if verified.len() != schedules.len() || compromised {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
    }
    verified.sort_by(|left, right| {
        artifact_sort_key(&left.receipt.location).cmp(&artifact_sort_key(&right.receipt.location))
    });
    evidence_observations
        .sort_by(|left, right| observation_sort_key(left).cmp(&observation_sort_key(right)));

    if blocks.is_empty() {
        successful_handoff(
            input.plan.clone(),
            input.attempts.clone(),
            evidence_observations,
            verified,
            artifact_set_digest,
        )
    } else {
        blocked_handoff(
            input.plan.clone(),
            input.attempts.clone(),
            evidence_observations,
            blocks.into_iter().collect(),
            artifact_set_digest,
        )
    }
}

/// Stable identity for the complete immutable descriptor set. Mirror IDs and
/// transient fetch source are intentionally excluded: identical verified pinned
/// artifacts retain one candidate-set identity regardless of signed mirror alias.
pub fn domain_pack_remote_artifact_set_digest(
    artifacts: &[DomainPackRemoteArtifactLocationBinding],
) -> Result<String, String> {
    remote_artifact_set_digest(artifacts)
}

fn validate_planning_input(
    input: &DomainPackRemoteAcquisitionPlanningInput,
) -> Result<(), DomainPackRemoteAcquisitionRejection> {
    input.request.validate().map_err(|error| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidRemoteRequest,
            "request",
            format!("remote request failed contract validation: {error:?}"),
        )
    })?;
    let request = &input.request.domain_pack_remote_acquisition_request;
    if request.authority != DomainPackCandidateAuthority::CandidateOnly
        || request
            .catalog
            .registry
            .domain_pack_supply_chain_registry
            .authority
            != DomainPackCandidateAuthority::CandidateOnly
    {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::CandidateAuthorityViolation,
            "request.authority",
            "remote request and catalog material must remain candidate-only",
        ));
    }
    if request
        .catalog
        .registry
        .domain_pack_supply_chain_registry
        .mirrors
        .is_empty()
    {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidRemoteRequest,
            "request.catalog.registry.mirrors",
            "a remote acquisition plan requires at least one signed mirror location",
        ));
    }
    if !verify_domain_pack_acquisition_plan(&input.acquisition_plan) {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidAcquisitionPlan,
            "acquisition_plan",
            "candidate acquisition plan failed its integrity invariants",
        ));
    }
    let replayed = plan_domain_pack_acquisition(&input.acquisition).map_err(|_| {
        rejection(
            DomainPackRemoteAcquisitionIssueCode::DiscoveryReplayMismatch,
            "acquisition",
            "remote acquisition input does not replay its discovery-bound candidate plan",
        )
    })?;
    if replayed != input.acquisition_plan {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::DiscoveryReplayMismatch,
            "acquisition_plan",
            "presented candidate acquisition plan does not replay from discovery input",
        ));
    }
    validate_remote_candidate_binding(input)
}

fn validate_remote_candidate_binding(
    input: &DomainPackRemoteAcquisitionPlanningInput,
) -> Result<(), DomainPackRemoteAcquisitionRejection> {
    let request = &input.request.domain_pack_remote_acquisition_request;
    let acquisition = &input.acquisition_plan.domain_pack_acquisition_plan;
    let selected = &acquisition.selected;
    let record = &request.package.record;
    let package = &request.package.package;
    if request.discovery.acquisition_id != acquisition.acquisition_id
        || request.discovery.discovery_projection_digest != acquisition.discovery_projection_digest
        || request.discovery.demand_digest != acquisition.demand_digest
        || request.discovery.candidate_id != selected.candidate_id
        || request.discovery.requirement_ref != selected.requirement_ref
    {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::DiscoveryBindingMismatch,
            "request.discovery",
            "remote request discovery fields do not bind the replayed candidate acquisition plan",
        ));
    }
    if record.record_digest != selected.supply_chain_record_digest
        || record.package_digest != selected.package_digest
        || package.package_digest != selected.package_digest
        || package.content.canonical_sha256 != selected.content_digest
        || record.identity.publisher != selected.pack.publisher
        || record.identity.name != selected.pack.name
        || record.identity.version != selected.pack.version
    {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::CatalogPackageBindingMismatch,
            "request.package",
            "remote record/package material does not join the selected candidate exactly",
        ));
    }
    if input.catalog_facts.snapshot_digest
        != request
            .catalog
            .registry
            .domain_pack_supply_chain_registry
            .snapshot_digest
    {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::CatalogPackageBindingMismatch,
            "catalog_facts.snapshot_digest",
            "catalog facts refer to a different signed catalog snapshot",
        ));
    }
    Ok(())
}

fn catalog_blocks(
    input: &DomainPackRemoteAcquisitionPlanningInput,
    request: &forge_core_contracts::DomainPackRemoteAcquisitionRequest,
) -> BTreeSet<DomainPackRemoteAcquisitionBlock> {
    let mut blocks = BTreeSet::new();
    match input.catalog_facts.availability {
        DomainPackRemoteCatalogAvailability::CurrentAnchored => {}
        DomainPackRemoteCatalogAvailability::Unavailable
        | DomainPackRemoteCatalogAvailability::NotAnchored => {
            blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogAnchorMissing);
        }
        DomainPackRemoteCatalogAvailability::Stale => {
            blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogExpired);
        }
        DomainPackRemoteCatalogAvailability::Revoked => {
            blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogRevoked);
            if request.network_mode == DomainPackRemoteNetworkMode::OfflineOnly {
                blocks.insert(DomainPackRemoteAcquisitionBlock::OfflineRevoked);
            }
        }
        DomainPackRemoteCatalogAvailability::SignatureTamper => {
            blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogSignatureTamper);
        }
    }
    let registry = &request.catalog.registry.domain_pack_supply_chain_registry;
    if input.catalog_facts.host_checked_at_unix >= registry.expires_at_unix {
        blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogExpired);
    }
    if registry
        .revocations
        .iter()
        .any(|revocation| revocation.record_digest == request.package.record.record_digest)
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::CatalogRevoked);
        if request.network_mode == DomainPackRemoteNetworkMode::OfflineOnly {
            blocks.insert(DomainPackRemoteAcquisitionBlock::OfflineRevoked);
        }
    }
    blocks
}

fn cache_dispositions(
    input: &DomainPackRemoteAcquisitionPlanningInput,
    descriptors: &[DomainPackRemoteArtifactDescriptor],
) -> Vec<DomainPackRemoteCacheDisposition> {
    let Some(projection) = &input.cache_projection else {
        return vec![DomainPackRemoteCacheDisposition::Miss; descriptors.len()];
    };
    let request = &input.request.domain_pack_remote_acquisition_request;
    let projection_valid = projection.validate().is_ok()
        && projection.domain_pack_remote_cache_projection.authority
            == DomainPackCandidateAuthority::CandidateOnly
        && projection.domain_pack_remote_cache_projection.policy == request.cache_policy;
    if !projection_valid {
        return vec![DomainPackRemoteCacheDisposition::Tampered; descriptors.len()];
    }
    descriptors
        .iter()
        .map(|descriptor| {
            let matching = projection
                .domain_pack_remote_cache_projection
                .entries
                .iter()
                .find(|entry| entry.cache_key_raw_sha256 == descriptor.binding.raw_sha256);
            match matching {
                None => DomainPackRemoteCacheDisposition::Miss,
                Some(entry) if cache_entry_matches(entry, descriptor) => {
                    DomainPackRemoteCacheDisposition::Hit {
                        cache_key_raw_sha256: entry.cache_key_raw_sha256.clone(),
                    }
                }
                Some(_) => DomainPackRemoteCacheDisposition::Tampered,
            }
        })
        .collect()
}

fn cache_entry_matches(
    entry: &DomainPackRemoteCacheEntry,
    descriptor: &DomainPackRemoteArtifactDescriptor,
) -> bool {
    let document = DomainPackRemoteCacheEntryDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_entry: entry.clone(),
    };
    document.validate().is_ok()
        && entry.artifact == *descriptor
        && entry.cache_key_raw_sha256 == descriptor.binding.raw_sha256
        && entry.byte_length == descriptor.byte_length
}

fn eligible_mirrors<'a>(
    mode: DomainPackRemoteNetworkMode,
    mirrors: &[&'a forge_core_contracts::DomainPackRegistryMirror],
    request: &forge_core_contracts::DomainPackRemoteAcquisitionRequest,
) -> Vec<&'a forge_core_contracts::DomainPackRegistryMirror> {
    match mode {
        DomainPackRemoteNetworkMode::OnlineRequired => mirrors
            .iter()
            .copied()
            .filter(|mirror| {
                matches!(
                    &mirror.transport,
                    DomainPackRegistryMirrorTransport::Https { .. }
                )
            })
            .collect(),
        DomainPackRemoteNetworkMode::PreferCache => mirrors
            .iter()
            .copied()
            .filter(|mirror| {
                matches!(
                    &mirror.transport,
                    DomainPackRegistryMirrorTransport::Https { .. }
                ) || request.operator_anchored_local_head.is_some()
            })
            .collect(),
        DomainPackRemoteNetworkMode::OfflineOnly => mirrors
            .iter()
            .copied()
            .filter(|mirror| {
                matches!(
                    &mirror.transport,
                    DomainPackRegistryMirrorTransport::OperatorProvisionedLocal { .. }
                )
            })
            .collect(),
    }
}

fn fallback_mirrors<'a>(
    mode: DomainPackRemoteNetworkMode,
    mirrors: &[&'a forge_core_contracts::DomainPackRegistryMirror],
    request: &forge_core_contracts::DomainPackRemoteAcquisitionRequest,
) -> Vec<&'a forge_core_contracts::DomainPackRegistryMirror> {
    eligible_mirrors(mode, mirrors, request)
}

fn source_for_mirror(
    mirror: &forge_core_contracts::DomainPackRegistryMirror,
    request: &forge_core_contracts::DomainPackRemoteAcquisitionRequest,
) -> DomainPackRemoteFetchSource {
    match &mirror.transport {
        DomainPackRegistryMirrorTransport::Https { .. } => {
            DomainPackRemoteFetchSource::NetworkMirror {
                mirror_id: mirror.mirror_id.clone(),
            }
        }
        DomainPackRegistryMirrorTransport::OperatorProvisionedLocal { .. } => {
            DomainPackRemoteFetchSource::OperatorAnchoredLocalMirror {
                mirror_id: mirror.mirror_id.clone(),
                anchored_snapshot_digest: request
                    .operator_anchored_local_head
                    .as_ref()
                    .map_or_else(String::new, |head| head.snapshot_digest.clone()),
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ArtifactSchedule {
    sources: Vec<ExpectedAttempt>,
}

#[derive(Debug, Clone)]
struct ExpectedAttempt {
    location: DomainPackRemoteArtifactLocationBinding,
    source: DomainPackRemoteFetchSource,
}

fn expected_schedules(
    decision: &DomainPackRemoteAcquisitionPlanningDecision,
) -> Vec<ArtifactSchedule> {
    let plan = &decision.plan.domain_pack_remote_acquisition_plan;
    let mut transport = BTreeMap::<String, Vec<ExpectedAttempt>>::new();
    for attempt in &decision.transport_attempts {
        transport
            .entry(artifact_key(&attempt.location))
            .or_default()
            .push(ExpectedAttempt {
                location: attempt.location.clone(),
                source: attempt.source.clone(),
            });
    }
    plan.artifacts
        .iter()
        .zip(&decision.cache_dispositions)
        .map(|(location, cache)| {
            let mut sources = Vec::new();
            if !matches!(
                plan.network_mode,
                DomainPackRemoteNetworkMode::OnlineRequired
            ) {
                if let DomainPackRemoteCacheDisposition::Hit {
                    cache_key_raw_sha256,
                } = cache
                {
                    sources.push(ExpectedAttempt {
                        location: location.clone(),
                        source: DomainPackRemoteFetchSource::Cache {
                            cache_key_raw_sha256: cache_key_raw_sha256.clone(),
                        },
                    });
                }
            }
            sources.extend(
                transport
                    .remove(&artifact_key(location))
                    .unwrap_or_default(),
            );
            ArtifactSchedule { sources }
        })
        .collect()
}

fn attempt_matches_expected(
    attempt: &DomainPackRemoteFetchAttempt,
    expected: &ExpectedAttempt,
) -> bool {
    match attempt {
        DomainPackRemoteFetchAttempt::TransportFailure { location, source } => {
            location == &expected.location && source == &expected.source
        }
        DomainPackRemoteFetchAttempt::Observation { observation, .. } => {
            let observation = &observation.domain_pack_remote_untrusted_fetch_observation;
            observation.location == expected.location && observation.source == expected.source
        }
    }
}

fn observation_blocks(
    plan: &forge_core_contracts::DomainPackRemoteAcquisitionPlan,
    expected: &ExpectedAttempt,
    observation: &DomainPackRemoteUntrustedFetchObservationDocument,
    raw_bytes: &[u8],
) -> BTreeSet<DomainPackRemoteAcquisitionBlock> {
    let mut blocks = BTreeSet::new();
    let observed = &observation.domain_pack_remote_untrusted_fetch_observation;
    if observation.validate().is_err()
        || observed.authority != DomainPackCandidateAuthority::CandidateOnly
        || observed.request_digest != plan.request_digest
        || observed.location != expected.location
        || observed.source != expected.source
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch);
    }
    let raw_digest = sha256_digest(raw_bytes);
    if raw_digest != observed.observed_raw_sha256
        || raw_digest != expected.location.artifact.binding.raw_sha256
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactRawDigestMismatch);
    }
    if u64::try_from(raw_bytes.len()).unwrap_or(u64::MAX) != observed.observed_byte_length
        || observed.observed_byte_length != expected.location.artifact.byte_length
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactLengthMismatch);
    }
    if observed.observed_media_type != expected.location.artifact.media_type {
        blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactMediaTypeMismatch);
    }
    match canonical_digest_for_media(expected.location.artifact.media_type, raw_bytes) {
        Ok(canonical_digest)
            if canonical_digest == observed.observed_canonical_sha256
                && canonical_digest == expected.location.artifact.binding.canonical_sha256 => {}
        Ok(_) | Err(()) => {
            blocks.insert(DomainPackRemoteAcquisitionBlock::ArtifactCanonicalDigestMismatch);
        }
    }
    if !blocks.is_empty()
        && matches!(
            &expected.source,
            DomainPackRemoteFetchSource::NetworkMirror { .. }
                | DomainPackRemoteFetchSource::OperatorAnchoredLocalMirror { .. }
        )
    {
        blocks.insert(DomainPackRemoteAcquisitionBlock::MirrorEquivocation);
    }
    blocks
}

fn canonical_digest_for_media(
    media_type: DomainPackRemoteArtifactMediaType,
    raw_bytes: &[u8],
) -> Result<String, ()> {
    match media_type {
        DomainPackRemoteArtifactMediaType::ApplicationYaml => {
            let text = std::str::from_utf8(raw_bytes).map_err(|_| ())?;
            let value: serde_json::Value = yaml_serde::from_str(text).map_err(|_| ())?;
            canonical_digest(&value).map_err(|_| ())
        }
        DomainPackRemoteArtifactMediaType::ApplicationJson => {
            let value: serde_json::Value = serde_json::from_slice(raw_bytes).map_err(|_| ())?;
            canonical_digest(&value).map_err(|_| ())
        }
        DomainPackRemoteArtifactMediaType::TextPlain => {
            let text = std::str::from_utf8(raw_bytes).map_err(|_| ())?;
            canonical_digest(&text).map_err(|_| ())
        }
        DomainPackRemoteArtifactMediaType::ApplicationOctetStream => Ok(sha256_digest(raw_bytes)),
    }
}

fn successful_handoff(
    plan: DomainPackRemoteAcquisitionPlanDocument,
    attempts: Vec<DomainPackRemoteFetchAttempt>,
    observations: Vec<DomainPackRemoteUntrustedFetchObservation>,
    verified_artifacts: Vec<DomainPackRemoteVerifiedArtifact>,
    artifact_set_digest: String,
) -> Result<DomainPackRemoteAcquisitionHandoff, DomainPackRemoteAcquisitionRejection> {
    let evidence = fetch_evidence_document(
        &plan,
        observations,
        DomainPackRemoteFetchOutcome::CandidateBytesVerified,
        Vec::new(),
    )?;
    let receipt = fetch_receipt_document(
        &plan,
        verified_artifacts
            .iter()
            .map(|artifact| artifact.receipt.clone())
            .collect(),
        DomainPackRemoteFetchOutcome::CandidateBytesVerified,
        Vec::new(),
    )?;
    if evidence.validate().is_err() || receipt.validate().is_err() {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidPlan,
            "handoff",
            "derived verified evidence or receipt failed its contract invariants",
        ));
    }
    Ok(DomainPackRemoteAcquisitionHandoff {
        plan,
        evidence,
        receipt,
        attempts,
        verified_artifacts,
        artifact_set_digest,
    })
}

fn blocked_handoff(
    plan: DomainPackRemoteAcquisitionPlanDocument,
    attempts: Vec<DomainPackRemoteFetchAttempt>,
    observations: Vec<DomainPackRemoteUntrustedFetchObservation>,
    mut blocks: Vec<DomainPackRemoteAcquisitionBlock>,
    artifact_set_digest: String,
) -> Result<DomainPackRemoteAcquisitionHandoff, DomainPackRemoteAcquisitionRejection> {
    blocks.sort();
    blocks.dedup();
    let evidence = fetch_evidence_document(
        &plan,
        observations,
        DomainPackRemoteFetchOutcome::Blocked,
        blocks.clone(),
    )?;
    let receipt = fetch_receipt_document(
        &plan,
        Vec::new(),
        DomainPackRemoteFetchOutcome::Blocked,
        blocks,
    )?;
    if evidence.validate().is_err() || receipt.validate().is_err() {
        return Err(rejection(
            DomainPackRemoteAcquisitionIssueCode::InvalidPlan,
            "handoff",
            "derived blocked evidence or receipt failed its contract invariants",
        ));
    }
    Ok(DomainPackRemoteAcquisitionHandoff {
        plan,
        evidence,
        receipt,
        attempts,
        verified_artifacts: Vec::new(),
        artifact_set_digest,
    })
}

fn fetch_evidence_document(
    plan: &DomainPackRemoteAcquisitionPlanDocument,
    observations: Vec<DomainPackRemoteUntrustedFetchObservation>,
    outcome: DomainPackRemoteFetchOutcome,
    blocks: Vec<DomainPackRemoteAcquisitionBlock>,
) -> Result<DomainPackRemoteFetchEvidenceDocument, DomainPackRemoteAcquisitionRejection> {
    let mut document = DomainPackRemoteFetchEvidenceDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_fetch_evidence: DomainPackRemoteFetchEvidence {
            evidence_id: derived_id(
                "evidence",
                &plan.domain_pack_remote_acquisition_plan.plan_digest,
            ),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            plan_digest: plan.domain_pack_remote_acquisition_plan.plan_digest.clone(),
            observations,
            outcome,
            blocks,
            evidence_digest: String::new(),
        },
    };
    document.domain_pack_remote_fetch_evidence.evidence_digest =
        document.evidence_digest().map_err(|error| {
            rejection(
                DomainPackRemoteAcquisitionIssueCode::CanonicalizationFailure,
                "evidence",
                format!("could not canonicalize fetch evidence: {error:?}"),
            )
        })?;
    Ok(document)
}

fn fetch_receipt_document(
    plan: &DomainPackRemoteAcquisitionPlanDocument,
    artifacts: Vec<DomainPackRemoteFetchedArtifactReceipt>,
    outcome: DomainPackRemoteFetchOutcome,
    blocks: Vec<DomainPackRemoteAcquisitionBlock>,
) -> Result<DomainPackRemoteFetchReceiptDocument, DomainPackRemoteAcquisitionRejection> {
    let remote_plan = &plan.domain_pack_remote_acquisition_plan;
    let mut document = DomainPackRemoteFetchReceiptDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_fetch_receipt: DomainPackRemoteFetchReceipt {
            receipt_id: derived_id("receipt", &remote_plan.plan_digest),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            plan_digest: remote_plan.plan_digest.clone(),
            catalog_snapshot_digest: remote_plan.catalog.snapshot_digest.clone(),
            registry_record_digest: remote_plan.package.record.record_digest.clone(),
            package_digest: remote_plan.package.package.package_digest.clone(),
            artifacts,
            outcome,
            blocks,
            receipt_digest: String::new(),
        },
    };
    document.domain_pack_remote_fetch_receipt.receipt_digest =
        document.receipt_digest().map_err(|error| {
            rejection(
                DomainPackRemoteAcquisitionIssueCode::CanonicalizationFailure,
                "receipt",
                format!("could not canonicalize fetch receipt: {error:?}"),
            )
        })?;
    Ok(document)
}

fn collect_evidence_observation(
    attempt: &DomainPackRemoteFetchAttempt,
    observations: &mut Vec<DomainPackRemoteUntrustedFetchObservation>,
) {
    if let DomainPackRemoteFetchAttempt::Observation { observation, .. } = attempt {
        // A malformed untrusted document is retained in `handoff.attempts`, not
        // injected into canonical contract evidence where it would invalidate
        // the entire blocked evidence receipt.
        if observation.validate().is_ok() {
            observations.push(
                observation
                    .domain_pack_remote_untrusted_fetch_observation
                    .clone(),
            );
        }
    }
}

fn record_descriptors(
    artifacts: &forge_core_contracts::DomainPackRegistryArtifactSet,
) -> Vec<&DomainPackRemoteArtifactDescriptor> {
    let mut descriptors = Vec::with_capacity(3 + artifacts.fixtures.len());
    descriptors.push(&artifacts.manifest);
    descriptors.push(&artifacts.content);
    descriptors.push(&artifacts.license);
    descriptors.extend(&artifacts.fixtures);
    descriptors
}

fn remote_artifact_set_digest(
    artifacts: &[DomainPackRemoteArtifactLocationBinding],
) -> Result<String, String> {
    let mut descriptors = artifacts
        .iter()
        .map(|location| location.artifact.clone())
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| {
        left.binding
            .artifact_ref
            .0
            .cmp(&right.binding.artifact_ref.0)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    canonical_digest(&descriptors)
}

fn artifact_key(location: &DomainPackRemoteArtifactLocationBinding) -> String {
    format!(
        "{}\u{1f}{}",
        location.artifact.binding.artifact_ref.0, location.artifact.binding.raw_sha256
    )
}

fn artifact_sort_key(location: &DomainPackRemoteArtifactLocationBinding) -> (&str, &str) {
    (
        location.artifact.binding.artifact_ref.0.as_str(),
        location.artifact.binding.raw_sha256.as_str(),
    )
}

fn observation_sort_key(
    observation: &DomainPackRemoteUntrustedFetchObservation,
) -> (&str, &str, &str) {
    (
        observation
            .location
            .artifact
            .binding
            .artifact_ref
            .0
            .as_str(),
        observation.location.mirror_id.0.as_str(),
        observation.observation_id.0.as_str(),
    )
}

fn sha256_digest(raw_bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(raw_bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn derived_id(kind: &str, digest: &str) -> StableId {
    let mut hasher = Sha256::new();
    hasher.update((kind.len() as u64).to_be_bytes());
    hasher.update(kind.as_bytes());
    hasher.update((digest.len() as u64).to_be_bytes());
    hasher.update(digest.as_bytes());
    StableId(format!("remote-acquisition.{kind}.{:x}", hasher.finalize()))
}

fn rejection(
    code: DomainPackRemoteAcquisitionIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> DomainPackRemoteAcquisitionRejection {
    DomainPackRemoteAcquisitionRejection {
        issues: vec![DomainPackRemoteAcquisitionIssue {
            code,
            path: path.into(),
            message: message.into(),
        }],
    }
}
