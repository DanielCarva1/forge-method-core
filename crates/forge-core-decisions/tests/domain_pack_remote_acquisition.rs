use forge_core_contracts::*;
use forge_core_decisions::{
    discover_domain_packs, plan_domain_pack_acquisition, plan_domain_pack_remote_acquisition,
    verify_domain_pack_remote_acquisition_plan, verify_domain_pack_remote_fetches,
    DomainPackRemoteAcquisitionPlanningInput, DomainPackRemoteCacheDisposition,
    DomainPackRemoteCatalogAvailability, DomainPackRemoteCatalogFacts,
    DomainPackRemoteFetchAttempt, DomainPackRemoteFetchVerificationInput,
    DomainPackRemotePlannedTransportAttempt,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn digest(byte: u8) -> String {
    format!("sha256:{}", char::from(byte).to_string().repeat(64))
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_yaml_digest(bytes: &[u8]) -> String {
    let text = std::str::from_utf8(bytes).expect("fixture YAML is UTF-8");
    let value: serde_json::Value = yaml_serde::from_str(text).expect("fixture YAML parses");
    canonical_digest(&value)
}

#[derive(Clone)]
struct Fixture {
    planning: DomainPackRemoteAcquisitionPlanningInput,
    bytes: BTreeMap<String, Vec<u8>>,
}

fn acquisition() -> (
    DomainPackAcquisitionPlanningInput,
    DomainPackAcquisitionPlanDocument,
) {
    let request: DomainPackDiscoveryRequestDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml"
    ))
    .expect("discovery fixture");
    let discovery = discover_domain_packs(&request).expect("discovery projection");
    let projection = &discovery.domain_pack_discovery_projection;
    let selected = &projection.matches[0];
    let planning = DomainPackAcquisitionPlanningInput {
        intent: DomainPackAcquisitionIntentDocument {
            schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
            domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
                acquisition_id: StableId("acquisition.remote.fixture".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                assurance_binding: projection.assurance_binding.clone(),
                discovery_projection_digest: projection.projection_digest.clone(),
                demand_digest: projection.demand_digest.clone(),
                candidate_id: selected.candidate_id.clone(),
                requirement_ref: selected.requirement_ref.clone(),
                operation: DomainPackAcquisitionOperation::Install,
                expected_project_snapshot_digest: projection
                    .assurance_binding
                    .snapshot_digest
                    .clone(),
            },
        },
        request,
        discovery,
    };
    let plan = plan_domain_pack_acquisition(&planning).expect("candidate acquisition plan");
    (planning, plan)
}

fn descriptor(
    kind: DomainPackRemoteArtifactKind,
    artifact_ref: &str,
    bytes: &[u8],
) -> DomainPackRemoteArtifactDescriptor {
    let raw_sha256 = raw_digest(bytes);
    DomainPackRemoteArtifactDescriptor {
        kind,
        binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath(artifact_ref.to_owned()),
            raw_sha256: raw_sha256.clone(),
            canonical_sha256: canonical_yaml_digest(bytes),
        },
        object_path: RepoPath(format!("objects/sha256/{}", &raw_sha256["sha256:".len()..])),
        byte_length: u64::try_from(bytes.len()).expect("fixture length"),
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

#[allow(clippy::too_many_lines)]
fn fixture() -> Fixture {
    let (acquisition, acquisition_plan) = acquisition();
    let selected = &acquisition_plan.domain_pack_acquisition_plan.selected;
    let discovery_content =
        &acquisition.request.domain_pack_discovery_request.candidates[0].content;
    let manifest_bytes = b"schema_version: '0.1'\nmanifest: remote-fixture\n".to_vec();
    // Serialize content as JSON so its semantic SHA-256 is exactly the one
    // already replayed by discovery; manifest and license retain YAML coverage.
    let content_bytes = serde_json::to_vec(discovery_content).expect("content JSON");
    let license_bytes = b"license: MIT\n".to_vec();
    let manifest = descriptor(
        DomainPackRemoteArtifactKind::Manifest,
        "packs/remote/manifest.yaml",
        &manifest_bytes,
    );
    let content_raw_sha256 = raw_digest(&content_bytes);
    let content = DomainPackRemoteArtifactDescriptor {
        kind: DomainPackRemoteArtifactKind::Content,
        binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath("packs/remote/content.json".to_owned()),
            raw_sha256: content_raw_sha256.clone(),
            canonical_sha256: canonical_digest(discovery_content),
        },
        object_path: RepoPath(format!(
            "objects/sha256/{}",
            &content_raw_sha256["sha256:".len()..]
        )),
        byte_length: u64::try_from(content_bytes.len()).expect("fixture length"),
        media_type: DomainPackRemoteArtifactMediaType::ApplicationJson,
    };
    assert_eq!(content.binding.canonical_sha256, selected.content_digest);
    let license = descriptor(
        DomainPackRemoteArtifactKind::License,
        "packs/remote/LICENSE.yaml",
        &license_bytes,
    );
    let record = DomainPackRegistryPackageRecord {
        identity: DomainPackIdentity {
            publisher: selected.pack.publisher.clone(),
            name: selected.pack.name.clone(),
            namespace: discovery_content.domain_pack_content.namespace.clone(),
            version: selected.pack.version.clone(),
        },
        package_digest: selected.package_digest.clone(),
        manifest_digest: manifest.binding.raw_sha256.clone(),
        content_digest: content.binding.raw_sha256.clone(),
        license_digest: license.binding.raw_sha256.clone(),
        fixture_digests: Vec::new(),
        artifacts: DomainPackRegistryArtifactSet {
            manifest: manifest.clone(),
            content: content.clone(),
            license: license.clone(),
            fixtures: Vec::new(),
        },
        namespace_grant_id: StableId("grant.remote.fixture".to_owned()),
        publisher_credential_id: StableId("credential.remote.fixture".to_owned()),
        publisher_signature_hex: "00".to_owned(),
        record_digest: selected.supply_chain_record_digest.clone(),
    };
    let snapshot_digest = digest(b'e');
    let registry = DomainPackSupplyChainRegistryDocument {
        schema_version: "0.4".to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.remote.fixture".to_owned()),
            registry_version: "1".to_owned(),
            audience: StableId("audience.remote.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 7,
            previous_snapshot_digest: None,
            issued_at_unix: 10,
            expires_at_unix: 200,
            publisher_credentials: Vec::new(),
            namespace_grants: Vec::new(),
            // `mirror.alpha` and `mirror.beta` are intentionally aliases of
            // one transport identity. Stable mirror IDs still determine order.
            mirrors: vec![
                DomainPackRegistryMirror {
                    mirror_id: StableId("mirror.zulu".to_owned()),
                    priority: 2,
                    transport: DomainPackRegistryMirrorTransport::Https {
                        base_url: "https://mirror-z.example.invalid/domain-packs".to_owned(),
                    },
                },
                DomainPackRegistryMirror {
                    mirror_id: StableId("mirror.beta".to_owned()),
                    priority: 0,
                    transport: DomainPackRegistryMirrorTransport::Https {
                        base_url: "https://mirror-a.example.invalid/domain-packs".to_owned(),
                    },
                },
                DomainPackRegistryMirror {
                    mirror_id: StableId("mirror.local".to_owned()),
                    priority: 1,
                    transport: DomainPackRegistryMirrorTransport::OperatorProvisionedLocal {
                        location_id: StableId("operator-local.remote.fixture".to_owned()),
                    },
                },
                DomainPackRegistryMirror {
                    mirror_id: StableId("mirror.alpha".to_owned()),
                    priority: 0,
                    transport: DomainPackRegistryMirrorTransport::Https {
                        base_url: "https://mirror-a.example.invalid/domain-packs".to_owned(),
                    },
                },
            ],
            packages: vec![record.clone()],
            revocations: Vec::new(),
            snapshot_digest: snapshot_digest.clone(),
            signatures: Vec::new(),
        },
    };
    let package = DomainPackPackageBinding {
        package_ref: RepoPath("packs/remote/package.yaml".to_owned()),
        package_digest: selected.package_digest.clone(),
        manifest: manifest.binding.clone(),
        content: DomainPackContentBinding {
            content_ref: content.binding.artifact_ref.clone(),
            raw_sha256: content.binding.raw_sha256.clone(),
            canonical_sha256: content.binding.canonical_sha256.clone(),
        },
        license: license.binding.clone(),
        fixtures: Vec::new(),
    };
    let mut request = DomainPackRemoteAcquisitionRequestDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_acquisition_request: DomainPackRemoteAcquisitionRequest {
            request_id: StableId("request.remote.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            discovery: DomainPackRemoteCandidateDiscoveryBinding {
                acquisition_id: acquisition_plan
                    .domain_pack_acquisition_plan
                    .acquisition_id
                    .clone(),
                discovery_projection_digest: acquisition_plan
                    .domain_pack_acquisition_plan
                    .discovery_projection_digest
                    .clone(),
                demand_digest: acquisition_plan
                    .domain_pack_acquisition_plan
                    .demand_digest
                    .clone(),
                candidate_id: selected.candidate_id.clone(),
                requirement_ref: selected.requirement_ref.clone(),
                selection: DomainPackRemoteOperatorSelection::ExplicitCandidateApprovalRequired,
            },
            catalog: DomainPackRemoteCatalogSnapshotBinding {
                registry,
                snapshot_digest: snapshot_digest.clone(),
            },
            package: DomainPackRemotePackageAcquisitionBinding { record, package },
            network_mode: DomainPackRemoteNetworkMode::OnlineRequired,
            mirror_policy: DomainPackRemoteMirrorPolicy::SignedPriorityThenMirrorId,
            cache_policy: DomainPackRemoteCachePolicy::RejectOnFull {
                max_entry_bytes: 1_048_576,
                max_entries: 8,
                max_total_bytes: 4_194_304,
            },
            operator_anchored_local_head: None,
            request_digest: String::new(),
        },
    };
    rehash_request(&mut request);
    let bytes = [
        (manifest.binding.artifact_ref.0.clone(), manifest_bytes),
        (content.binding.artifact_ref.0.clone(), content_bytes),
        (license.binding.artifact_ref.0.clone(), license_bytes),
    ]
    .into_iter()
    .collect();
    Fixture {
        planning: DomainPackRemoteAcquisitionPlanningInput {
            acquisition,
            acquisition_plan,
            request,
            cache_projection: None,
            catalog_facts: DomainPackRemoteCatalogFacts {
                snapshot_digest,
                availability: DomainPackRemoteCatalogAvailability::CurrentAnchored,
                host_checked_at_unix: 100,
            },
        },
        bytes,
    }
}

fn rehash_request(document: &mut DomainPackRemoteAcquisitionRequestDocument) {
    document
        .domain_pack_remote_acquisition_request
        .request_digest = String::new();
    document
        .domain_pack_remote_acquisition_request
        .request_digest = document.request_digest().expect("canonical request digest");
}

fn rehash_plan(document: &mut DomainPackRemoteAcquisitionPlanDocument) {
    document.domain_pack_remote_acquisition_plan.plan_digest = String::new();
    document.domain_pack_remote_acquisition_plan.plan_digest =
        document.plan_digest().expect("canonical plan digest");
}

fn rehash_observation(document: &mut DomainPackRemoteUntrustedFetchObservationDocument) {
    document
        .domain_pack_remote_untrusted_fetch_observation
        .observation_digest = String::new();
    document
        .domain_pack_remote_untrusted_fetch_observation
        .observation_digest = document
        .observation_digest()
        .expect("canonical observation digest");
}

fn cache_projection(fixture: &Fixture) -> DomainPackRemoteCacheProjectionDocument {
    let request = &fixture
        .planning
        .request
        .domain_pack_remote_acquisition_request;
    let entries = record_descriptors(&request.package.record.artifacts)
        .into_iter()
        .map(|artifact| {
            let mut document = DomainPackRemoteCacheEntryDocument {
                schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
                domain_pack_remote_cache_entry: DomainPackRemoteCacheEntry {
                    cache_key_raw_sha256: artifact.binding.raw_sha256.clone(),
                    artifact: artifact.clone(),
                    byte_length: artifact.byte_length,
                    source_receipt_digest: digest(b'f'),
                    cached_at_unix: 20,
                    entry_digest: String::new(),
                },
            };
            document.domain_pack_remote_cache_entry.entry_digest =
                document.entry_digest().expect("cache entry digest");
            document.domain_pack_remote_cache_entry
        })
        .collect::<Vec<_>>();
    let mut document = DomainPackRemoteCacheProjectionDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_projection: DomainPackRemoteCacheProjection {
            cache_id: StableId("cache.remote.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            policy: request.cache_policy.clone(),
            total_bytes: entries.iter().map(|entry| entry.byte_length).sum(),
            entries,
            outcome: DomainPackRemoteCacheProjectionOutcome::CandidateBytesPresent,
            blocks: Vec::new(),
            projection_digest: String::new(),
        },
    };
    document
        .domain_pack_remote_cache_projection
        .projection_digest = document
        .projection_digest()
        .expect("cache projection digest");
    document
}

fn record_descriptors(
    artifacts: &DomainPackRegistryArtifactSet,
) -> Vec<DomainPackRemoteArtifactDescriptor> {
    let mut descriptors = vec![
        artifacts.manifest.clone(),
        artifacts.content.clone(),
        artifacts.license.clone(),
    ];
    descriptors.extend(artifacts.fixtures.clone());
    descriptors
}

fn observation(
    plan: &DomainPackRemoteAcquisitionPlanDocument,
    attempt: &DomainPackRemotePlannedTransportAttempt,
    bytes: Vec<u8>,
) -> DomainPackRemoteFetchAttempt {
    let mut document = DomainPackRemoteUntrustedFetchObservationDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_untrusted_fetch_observation: DomainPackRemoteUntrustedFetchObservation {
            observation_id: StableId(format!(
                "observation.{}",
                attempt
                    .location
                    .artifact
                    .binding
                    .artifact_ref
                    .0
                    .replace('/', "-")
            )),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: plan
                .domain_pack_remote_acquisition_plan
                .request_digest
                .clone(),
            source: attempt.source.clone(),
            location: attempt.location.clone(),
            observed_raw_sha256: attempt.location.artifact.binding.raw_sha256.clone(),
            observed_canonical_sha256: attempt.location.artifact.binding.canonical_sha256.clone(),
            observed_byte_length: attempt.location.artifact.byte_length,
            observed_media_type: attempt.location.artifact.media_type,
            observation_digest: String::new(),
        },
    };
    rehash_observation(&mut document);
    DomainPackRemoteFetchAttempt::Observation {
        observation: document,
        raw_bytes: bytes,
    }
}

fn observations_for_online(
    fixture: &Fixture,
) -> (
    forge_core_decisions::DomainPackRemoteAcquisitionPlanningDecision,
    Vec<DomainPackRemoteFetchAttempt>,
) {
    let decision = plan_domain_pack_remote_acquisition(&fixture.planning).expect("online plan");
    let attempts = decision
        .transport_attempts
        .iter()
        .step_by(3)
        .map(|attempt| {
            let bytes = fixture.bytes[&attempt.location.artifact.binding.artifact_ref.0].clone();
            observation(&decision.plan, attempt, bytes)
        })
        .collect();
    (decision, attempts)
}

fn offline(fixture: &mut Fixture) {
    let request = &mut fixture
        .planning
        .request
        .domain_pack_remote_acquisition_request;
    request.network_mode = DomainPackRemoteNetworkMode::OfflineOnly;
    let registry = &request.catalog.registry.domain_pack_supply_chain_registry;
    request.operator_anchored_local_head = Some(DomainPackRemoteOperatorAnchoredLocalHead {
        registry_id: registry.registry_id.clone(),
        audience: registry.audience.clone(),
        generation: registry.generation,
        snapshot_digest: registry.snapshot_digest.clone(),
        expires_at_unix: registry.expires_at_unix,
        anchored_at_unix: 50,
    });
    rehash_request(&mut fixture.planning.request);
}

#[test]
fn signed_mirror_order_and_candidate_plan_are_deterministic() {
    let fixture = fixture();
    let first = plan_domain_pack_remote_acquisition(&fixture.planning).expect("first plan");
    let mut reordered = fixture.planning.clone();
    reordered
        .request
        .domain_pack_remote_acquisition_request
        .catalog
        .registry
        .domain_pack_supply_chain_registry
        .mirrors
        .reverse();
    rehash_request(&mut reordered.request);
    let second = plan_domain_pack_remote_acquisition(&reordered).expect("reordered plan");

    let order = first
        .transport_attempts
        .iter()
        .take(3)
        .map(|attempt| match &attempt.source {
            DomainPackRemoteFetchSource::NetworkMirror { mirror_id } => mirror_id.0.as_str(),
            _ => panic!("online plans use network mirrors"),
        })
        .collect::<Vec<_>>();
    assert_eq!(order, ["mirror.alpha", "mirror.beta", "mirror.zulu"]);
    assert_eq!(first.transport_attempts, second.transport_attempts);
    assert_eq!(
        first
            .plan
            .domain_pack_remote_acquisition_plan
            .artifacts
            .len(),
        3
    );
    assert!(verify_domain_pack_remote_acquisition_plan(
        &fixture.planning,
        &first.plan
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn cache_hit_miss_stale_revoked_and_offline_modes_fail_closed() {
    let mut online = fixture();
    let online_projection = cache_projection(&online);
    online.planning.cache_projection = Some(online_projection);
    let online_plan = plan_domain_pack_remote_acquisition(&online.planning).expect("online plan");
    assert!(online_plan.cache_reads.is_empty());
    assert_eq!(online_plan.network_fetches.len(), 9);

    let mut cached = fixture();
    cached
        .planning
        .request
        .domain_pack_remote_acquisition_request
        .network_mode = DomainPackRemoteNetworkMode::PreferCache;
    rehash_request(&mut cached.planning.request);
    let cached_projection = cache_projection(&cached);
    cached.planning.cache_projection = Some(cached_projection);
    let cache_hit = plan_domain_pack_remote_acquisition(&cached.planning).expect("cache plan");
    assert!(cache_hit
        .cache_dispositions
        .iter()
        .all(|disposition| matches!(disposition, DomainPackRemoteCacheDisposition::Hit { .. })));
    assert_eq!(cache_hit.network_fetches.len(), 0);
    assert_eq!(
        cache_hit.plan.domain_pack_remote_acquisition_plan.outcome,
        DomainPackRemoteAcquisitionPlanOutcome::CacheOnlyCandidateBytesRequired
    );
    let cache_attempts = cache_hit
        .cache_reads
        .iter()
        .map(|attempt| {
            observation(
                &cache_hit.plan,
                &DomainPackRemotePlannedTransportAttempt {
                    location: attempt.location.clone(),
                    source: attempt.source.clone(),
                },
                cached.bytes[&attempt.location.artifact.binding.artifact_ref.0].clone(),
            )
        })
        .collect();
    let cache_handoff =
        verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
            planning: cached.planning.clone(),
            plan: cache_hit.plan.clone(),
            attempts: cache_attempts,
        })
        .expect("cache bytes still require and pass byte verification");
    assert_eq!(cache_handoff.verified_artifacts.len(), 3);

    let mut missed = cached.clone();
    missed.planning.cache_projection = None;
    let cache_miss =
        plan_domain_pack_remote_acquisition(&missed.planning).expect("cache miss plan");
    assert!(cache_miss
        .cache_dispositions
        .iter()
        .all(|disposition| matches!(disposition, DomainPackRemoteCacheDisposition::Miss)));
    assert_eq!(cache_miss.network_fetches.len(), 9);

    let mut stale = cached.clone();
    stale.planning.catalog_facts.availability = DomainPackRemoteCatalogAvailability::Stale;
    let stale_plan = plan_domain_pack_remote_acquisition(&stale.planning).expect("stale plan");
    assert_eq!(
        stale_plan.plan.domain_pack_remote_acquisition_plan.outcome,
        DomainPackRemoteAcquisitionPlanOutcome::Blocked
    );
    assert!(stale_plan
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CatalogExpired));

    let mut revoked = cached.clone();
    let record_digest = revoked
        .planning
        .request
        .domain_pack_remote_acquisition_request
        .package
        .record
        .record_digest
        .clone();
    revoked
        .planning
        .request
        .domain_pack_remote_acquisition_request
        .catalog
        .registry
        .domain_pack_supply_chain_registry
        .revocations
        .push(DomainPackPackageRevocation {
            record_digest,
            reason: DomainPackRevocationReason::PackageTamper,
            explanation: "fixture revocation".to_owned(),
            revoked_at_unix: 99,
        });
    rehash_request(&mut revoked.planning.request);
    let revoked_plan =
        plan_domain_pack_remote_acquisition(&revoked.planning).expect("revoked plan");
    assert!(revoked_plan
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CatalogRevoked));

    let mut offline_miss = fixture();
    offline(&mut offline_miss);
    let offline_plan =
        plan_domain_pack_remote_acquisition(&offline_miss.planning).expect("offline plan");
    assert_eq!(offline_plan.network_fetches.len(), 0);
    assert_eq!(offline_plan.local_mirror_reads.len(), 0);
    assert!(offline_plan
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::OfflineExactBytesMissing));
    assert!(offline_plan
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CacheMiss));

    let mut offline_not_anchored = offline_miss.clone();
    offline_not_anchored.planning.catalog_facts.availability =
        DomainPackRemoteCatalogAvailability::NotAnchored;
    let not_anchored = plan_domain_pack_remote_acquisition(&offline_not_anchored.planning)
        .expect("unanchored offline plan");
    assert!(not_anchored
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CatalogAnchorMissing));
}

#[test]
fn offline_never_schedules_network_and_stale_or_revoked_cache_cannot_revive_catalog() {
    let mut fixture = fixture();
    offline(&mut fixture);
    let cached_projection = cache_projection(&fixture);
    fixture.planning.cache_projection = Some(cached_projection);
    fixture.planning.catalog_facts.availability = DomainPackRemoteCatalogAvailability::Stale;
    let stale = plan_domain_pack_remote_acquisition(&fixture.planning).expect("offline stale plan");
    assert!(stale.cache_reads.is_empty());
    assert!(stale.network_fetches.is_empty());
    assert!(stale.local_mirror_reads.is_empty());
    assert!(stale
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CatalogExpired));

    fixture.planning.catalog_facts.availability = DomainPackRemoteCatalogAvailability::Revoked;
    let revoked =
        plan_domain_pack_remote_acquisition(&fixture.planning).expect("offline revoked plan");
    assert!(revoked.network_fetches.is_empty());
    assert!(revoked
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::OfflineRevoked));
}

#[test]
fn opaque_catalog_facts_can_only_block_candidate_byte_plans() {
    for (availability, expected_block) in [
        (
            DomainPackRemoteCatalogAvailability::Unavailable,
            DomainPackRemoteAcquisitionBlock::CatalogAnchorMissing,
        ),
        (
            DomainPackRemoteCatalogAvailability::SignatureTamper,
            DomainPackRemoteAcquisitionBlock::CatalogSignatureTamper,
        ),
    ] {
        let mut fixture = fixture();
        fixture.planning.catalog_facts.availability = availability;
        let decision = plan_domain_pack_remote_acquisition(&fixture.planning)
            .expect("catalog facts are opaque decision input");
        assert_eq!(
            decision.plan.domain_pack_remote_acquisition_plan.outcome,
            DomainPackRemoteAcquisitionPlanOutcome::Blocked
        );
        assert!(decision
            .plan
            .domain_pack_remote_acquisition_plan
            .blocks
            .contains(&expected_block));
        assert!(decision.cache_reads.is_empty());
        assert!(decision.transport_attempts.is_empty());
    }
}

#[test]
fn invalid_cache_projection_is_only_a_tampered_byte_hint() {
    let mut fixture = fixture();
    offline(&mut fixture);
    let mut projection = cache_projection(&fixture);
    projection.domain_pack_remote_cache_projection.total_bytes += 1;
    // Deliberately do not recompute the self-digest: cache integrity is a hint
    // boundary, so either invalid projection form is cache tamper, not authority.
    fixture.planning.cache_projection = Some(projection);
    let decision = plan_domain_pack_remote_acquisition(&fixture.planning).expect("tamper plan");
    assert!(decision
        .cache_dispositions
        .iter()
        .all(|disposition| matches!(disposition, DomainPackRemoteCacheDisposition::Tampered)));
    assert!(decision
        .plan
        .domain_pack_remote_acquisition_plan
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::CacheTamper));
    assert!(decision.network_fetches.is_empty());
}

#[test]
fn raw_length_media_canonical_and_yaml_mismatches_are_explicit_and_never_handed_off() {
    let fixture = fixture();
    let (decision, attempts) = observations_for_online(&fixture);
    let cases = [
        (
            {
                let mut value = attempts.clone();
                if let DomainPackRemoteFetchAttempt::Observation { raw_bytes, .. } = &mut value[0] {
                    *raw_bytes = b"manifest: altered\n".to_vec();
                }
                value
            },
            DomainPackRemoteAcquisitionBlock::ArtifactRawDigestMismatch,
        ),
        (
            {
                let mut value = attempts.clone();
                if let DomainPackRemoteFetchAttempt::Observation { observation, .. } = &mut value[0]
                {
                    observation
                        .domain_pack_remote_untrusted_fetch_observation
                        .observed_byte_length += 1;
                    rehash_observation(observation);
                }
                value
            },
            DomainPackRemoteAcquisitionBlock::ArtifactLengthMismatch,
        ),
        (
            {
                let mut value = attempts.clone();
                if let DomainPackRemoteFetchAttempt::Observation { observation, .. } = &mut value[0]
                {
                    observation
                        .domain_pack_remote_untrusted_fetch_observation
                        .observed_media_type = DomainPackRemoteArtifactMediaType::TextPlain;
                    rehash_observation(observation);
                }
                value
            },
            DomainPackRemoteAcquisitionBlock::ArtifactMediaTypeMismatch,
        ),
        (
            {
                let mut value = attempts.clone();
                if let DomainPackRemoteFetchAttempt::Observation { observation, .. } = &mut value[0]
                {
                    observation
                        .domain_pack_remote_untrusted_fetch_observation
                        .observed_canonical_sha256 = digest(b'9');
                    rehash_observation(observation);
                }
                value
            },
            DomainPackRemoteAcquisitionBlock::ArtifactCanonicalDigestMismatch,
        ),
        (
            {
                let mut value = attempts.clone();
                if let DomainPackRemoteFetchAttempt::Observation { raw_bytes, .. } = &mut value[0] {
                    *raw_bytes = b"\xffnot-valid-yaml".to_vec();
                }
                value
            },
            DomainPackRemoteAcquisitionBlock::ArtifactCanonicalDigestMismatch,
        ),
    ];
    for (attempts, expected_block) in cases {
        let handoff = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
            planning: fixture.planning.clone(),
            plan: decision.plan.clone(),
            attempts,
        })
        .expect("mismatch remains typed evidence");
        assert!(handoff
            .evidence
            .domain_pack_remote_fetch_evidence
            .blocks
            .contains(&expected_block));
        assert_eq!(handoff.verified_artifacts.len(), 0);
        assert_eq!(
            handoff.receipt.domain_pack_remote_fetch_receipt.outcome,
            DomainPackRemoteFetchOutcome::Blocked
        );
    }
}

#[test]
fn transport_failure_can_advance_but_mirror_equivocation_cannot() {
    let fixture = fixture();
    let decision = plan_domain_pack_remote_acquisition(&fixture.planning).expect("online plan");
    let transport = &decision.transport_attempts;
    let mut fallback_attempts = vec![DomainPackRemoteFetchAttempt::TransportFailure {
        location: transport[0].location.clone(),
        source: transport[0].source.clone(),
    }];
    for index in [1_usize, 3, 6] {
        let attempt = &transport[index];
        fallback_attempts.push(observation(
            &decision.plan,
            attempt,
            fixture.bytes[&attempt.location.artifact.binding.artifact_ref.0].clone(),
        ));
    }
    let fallback = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts: fallback_attempts,
    })
    .expect("transport failure is candidate evidence");
    assert_eq!(fallback.verified_artifacts.len(), 3);
    assert_eq!(
        fallback.receipt.domain_pack_remote_fetch_receipt.outcome,
        DomainPackRemoteFetchOutcome::CandidateBytesVerified
    );

    let (_, mut attempts) = observations_for_online(&fixture);
    if let DomainPackRemoteFetchAttempt::Observation { observation, .. } = &mut attempts[0] {
        observation
            .domain_pack_remote_untrusted_fetch_observation
            .source = DomainPackRemoteFetchSource::NetworkMirror {
            mirror_id: StableId("mirror.beta".to_owned()),
        };
        rehash_observation(observation);
    }
    let equivocation = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts,
    })
    .expect("equivocation remains evidence");
    assert!(equivocation
        .evidence
        .domain_pack_remote_fetch_evidence
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::MirrorEquivocation));
    assert!(equivocation.verified_artifacts.is_empty());
}

#[test]
fn missing_extra_and_replayed_plan_artifacts_fail_closed() {
    let fixture = fixture();
    let (decision, attempts) = observations_for_online(&fixture);
    let missing = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts: attempts[..2].to_vec(),
    })
    .expect("missing artifact is evidence");
    assert!(missing
        .evidence
        .domain_pack_remote_fetch_evidence
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch));

    let mut extra_attempts = attempts.clone();
    extra_attempts.push(attempts[0].clone());
    let extra = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts: extra_attempts,
    })
    .expect("extra artifact is evidence");
    assert!(extra
        .evidence
        .domain_pack_remote_fetch_evidence
        .blocks
        .contains(&DomainPackRemoteAcquisitionBlock::ArtifactDescriptorSetMismatch));

    let mut replay_mismatch = decision.plan.clone();
    replay_mismatch
        .domain_pack_remote_acquisition_plan
        .request_digest = digest(b'8');
    rehash_plan(&mut replay_mismatch);
    let rejection = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: replay_mismatch,
        attempts,
    })
    .expect_err("a plan cannot be replayed under a different request binding");
    assert_eq!(
        rejection.issues[0].code,
        forge_core_decisions::DomainPackRemoteAcquisitionIssueCode::PlanReplayMismatch
    );
}

#[test]
fn identical_signed_mirror_aliases_normalize_to_one_artifact_identity_without_authority() {
    let fixture = fixture();
    let (decision, primary_attempts) = observations_for_online(&fixture);
    let primary = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts: primary_attempts,
    })
    .expect("primary mirror handoff");

    let transport = &decision.transport_attempts;
    let mut alias_attempts = vec![DomainPackRemoteFetchAttempt::TransportFailure {
        location: transport[0].location.clone(),
        source: transport[0].source.clone(),
    }];
    for index in [1_usize, 3, 6] {
        let attempt = &transport[index];
        alias_attempts.push(observation(
            &decision.plan,
            attempt,
            fixture.bytes[&attempt.location.artifact.binding.artifact_ref.0].clone(),
        ));
    }
    let alias = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: fixture.planning.clone(),
        plan: decision.plan.clone(),
        attempts: alias_attempts,
    })
    .expect("alias mirror handoff");
    assert_eq!(primary.artifact_set_digest, alias.artifact_set_digest);
    assert_ne!(
        primary
            .receipt
            .domain_pack_remote_fetch_receipt
            .receipt_digest,
        alias
            .receipt
            .domain_pack_remote_fetch_receipt
            .receipt_digest
    );
    assert_eq!(
        primary.receipt.domain_pack_remote_fetch_receipt.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );
    let receipt = serde_json::to_value(&primary.receipt).expect("receipt JSON");
    assert!(receipt.get("trusted").is_none());
    assert!(receipt.get("installed").is_none());
    assert!(receipt.get("active").is_none());
}
