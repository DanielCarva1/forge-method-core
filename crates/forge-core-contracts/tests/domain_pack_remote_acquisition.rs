use forge_core_contracts::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackContentBinding,
    DomainPackIdentity, DomainPackPackageBinding, DomainPackRegistryArtifactSet,
    DomainPackRegistryMirror, DomainPackRegistryMirrorTransport, DomainPackRegistryPackageRecord,
    DomainPackRemoteAcquisitionRequest, DomainPackRemoteAcquisitionRequestDocument,
    DomainPackRemoteArtifactDescriptor, DomainPackRemoteArtifactKind,
    DomainPackRemoteArtifactMediaType, DomainPackRemoteCacheEntry,
    DomainPackRemoteCacheEntryDocument, DomainPackRemoteCachePolicy,
    DomainPackRemoteCacheProjection, DomainPackRemoteCacheProjectionDocument,
    DomainPackRemoteCacheProjectionOutcome, DomainPackRemoteCandidateDiscoveryBinding,
    DomainPackRemoteCatalogSnapshotBinding, DomainPackRemoteMirrorPolicy,
    DomainPackRemoteNetworkMode, DomainPackRemoteOperatorSelection,
    DomainPackRemotePackageAcquisitionBinding, DomainPackSupplyChainRegistry,
    DomainPackSupplyChainRegistryDocument, RepoPath, StableId,
    DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION,
};
use schemars::schema_for;

fn digest(hex: char) -> String {
    format!("sha256:{}", hex.to_string().repeat(64))
}

fn artifact(
    kind: DomainPackRemoteArtifactKind,
    logical_path: &str,
    raw_hex: char,
    canonical_hex: char,
) -> DomainPackRemoteArtifactDescriptor {
    let raw_sha256 = digest(raw_hex);
    DomainPackRemoteArtifactDescriptor {
        kind,
        binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath(logical_path.to_owned()),
            raw_sha256: raw_sha256.clone(),
            canonical_sha256: digest(canonical_hex),
        },
        object_path: RepoPath(format!("objects/sha256/{}", &raw_sha256[7..])),
        byte_length: 32,
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

#[allow(clippy::too_many_lines)]
fn request_document() -> DomainPackRemoteAcquisitionRequestDocument {
    let manifest = artifact(
        DomainPackRemoteArtifactKind::Manifest,
        "packs/example/manifest.yaml",
        'a',
        'b',
    );
    let content = artifact(
        DomainPackRemoteArtifactKind::Content,
        "packs/example/content.yaml",
        'c',
        'd',
    );
    let license = artifact(
        DomainPackRemoteArtifactKind::License,
        "packs/example/LICENSE",
        'e',
        'f',
    );
    let artifacts = DomainPackRegistryArtifactSet {
        manifest: manifest.clone(),
        content: content.clone(),
        license: license.clone(),
        fixtures: vec![],
    };
    let record = DomainPackRegistryPackageRecord {
        identity: DomainPackIdentity {
            publisher: StableId("forge.test".to_owned()),
            name: StableId("remote-example".to_owned()),
            namespace: StableId("remote_example".to_owned()),
            version: "1.0.0".to_owned(),
        },
        package_digest: digest('1'),
        manifest_digest: manifest.binding.raw_sha256.clone(),
        content_digest: content.binding.raw_sha256.clone(),
        license_digest: license.binding.raw_sha256.clone(),
        fixture_digests: vec![],
        artifacts,
        namespace_grant_id: StableId("grant.example".to_owned()),
        publisher_credential_id: StableId("credential.example".to_owned()),
        publisher_signature_hex: "00".to_owned(),
        record_digest: digest('2'),
    };
    let registry = DomainPackSupplyChainRegistryDocument {
        schema_version: "0.4".to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.example".to_owned()),
            registry_version: "1".to_owned(),
            audience: StableId("audience.example".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 7,
            previous_snapshot_digest: None,
            issued_at_unix: 10,
            expires_at_unix: 100,
            publisher_credentials: vec![],
            namespace_grants: vec![],
            mirrors: vec![DomainPackRegistryMirror {
                mirror_id: StableId("mirror.primary".to_owned()),
                priority: 0,
                transport: DomainPackRegistryMirrorTransport::Https {
                    base_url: "https://mirror.example.invalid/domain-packs".to_owned(),
                },
            }],
            packages: vec![record.clone()],
            revocations: vec![],
            snapshot_digest: digest('3'),
            signatures: vec![],
        },
    };
    let package = DomainPackPackageBinding {
        package_ref: RepoPath("packs/example".to_owned()),
        package_digest: record.package_digest.clone(),
        manifest: manifest.binding.clone(),
        content: DomainPackContentBinding {
            content_ref: content.binding.artifact_ref.clone(),
            raw_sha256: content.binding.raw_sha256.clone(),
            canonical_sha256: content.binding.canonical_sha256.clone(),
        },
        license: license.binding.clone(),
        fixtures: vec![],
    };
    let mut document = DomainPackRemoteAcquisitionRequestDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_acquisition_request: DomainPackRemoteAcquisitionRequest {
            request_id: StableId("request.example".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            discovery: DomainPackRemoteCandidateDiscoveryBinding {
                acquisition_id: StableId("acquisition.example".to_owned()),
                discovery_projection_digest: digest('4'),
                demand_digest: digest('5'),
                candidate_id: StableId("candidate.example".to_owned()),
                requirement_ref: StableId("requirement.example".to_owned()),
                selection: DomainPackRemoteOperatorSelection::ExplicitCandidateApprovalRequired,
            },
            catalog: DomainPackRemoteCatalogSnapshotBinding {
                snapshot_digest: registry
                    .domain_pack_supply_chain_registry
                    .snapshot_digest
                    .clone(),
                registry,
            },
            package: DomainPackRemotePackageAcquisitionBinding { record, package },
            network_mode: DomainPackRemoteNetworkMode::OnlineRequired,
            mirror_policy: DomainPackRemoteMirrorPolicy::SignedPriorityThenMirrorId,
            cache_policy: DomainPackRemoteCachePolicy::RejectOnFull {
                max_entry_bytes: 1024,
                max_entries: 3,
                max_total_bytes: 2048,
            },
            operator_anchored_local_head: None,
            request_digest: String::new(),
        },
    };
    document
        .domain_pack_remote_acquisition_request
        .request_digest = document.request_digest().expect("canonical request digest");
    document
}

#[test]
fn candidate_only_request_is_canonical_and_validates() {
    let document = request_document();
    document.validate().expect("valid request");
    assert_eq!(
        document.domain_pack_remote_acquisition_request.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );

    let mut changed = document.clone();
    changed
        .domain_pack_remote_acquisition_request
        .discovery
        .candidate_id = StableId("candidate.changed".to_owned());
    assert_ne!(
        document.request_digest().expect("original digest"),
        changed.request_digest().expect("changed digest")
    );
}

#[test]
fn serde_denies_unknown_fields_closed_enums_and_implicit_policy_defaults() {
    let document = request_document();
    let mut unknown = serde_json::to_value(&document).expect("request json");
    unknown["domain_pack_remote_acquisition_request"]["installed"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DomainPackRemoteAcquisitionRequestDocument>(unknown).is_err());

    let mut bad_mode = serde_json::to_value(&document).expect("request json");
    bad_mode["domain_pack_remote_acquisition_request"]["network_mode"] =
        serde_json::json!("trusted_network");
    assert!(
        serde_json::from_value::<DomainPackRemoteAcquisitionRequestDocument>(bad_mode).is_err()
    );

    let mut no_mode = serde_json::to_value(&document).expect("request json");
    no_mode["domain_pack_remote_acquisition_request"]
        .as_object_mut()
        .expect("request object")
        .remove("network_mode");
    assert!(serde_json::from_value::<DomainPackRemoteAcquisitionRequestDocument>(no_mode).is_err());

    let mut no_cache_policy = serde_json::to_value(&document).expect("request json");
    no_cache_policy["domain_pack_remote_acquisition_request"]
        .as_object_mut()
        .expect("request object")
        .remove("cache_policy");
    assert!(
        serde_json::from_value::<DomainPackRemoteAcquisitionRequestDocument>(no_cache_policy)
            .is_err()
    );
}

#[test]
fn registry_metadata_rejects_malformed_transports_and_descriptor_mismatch() {
    let mut document = request_document();
    document
        .domain_pack_remote_acquisition_request
        .catalog
        .registry
        .domain_pack_supply_chain_registry
        .mirrors[0]
        .transport = DomainPackRegistryMirrorTransport::Https {
        base_url: "https://credential@mirror.example.invalid/path#fragment".to_owned(),
    };
    assert!(document.validate().is_err());

    let mut document = request_document();
    document
        .domain_pack_remote_acquisition_request
        .catalog
        .registry
        .domain_pack_supply_chain_registry
        .packages[0]
        .artifacts
        .manifest
        .byte_length = 33;
    assert!(document.validate().is_err());

    let mut document = request_document();
    document
        .domain_pack_remote_acquisition_request
        .package
        .package
        .content
        .canonical_sha256 = digest('9');
    assert!(document.validate().is_err());

    let mut document = request_document();
    document.domain_pack_remote_acquisition_request.network_mode =
        DomainPackRemoteNetworkMode::OfflineOnly;
    assert!(document.validate().is_err());
}

#[test]
fn cache_tamper_and_reject_on_full_are_explicitly_rejected() {
    let request = request_document();
    let artifacts = &request
        .domain_pack_remote_acquisition_request
        .package
        .record
        .artifacts;
    let mut tampered = DomainPackRemoteCacheEntryDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_entry: DomainPackRemoteCacheEntry {
            cache_key_raw_sha256: digest('9'),
            artifact: artifacts.manifest.clone(),
            byte_length: artifacts.manifest.byte_length,
            source_receipt_digest: digest('6'),
            cached_at_unix: 20,
            entry_digest: String::new(),
        },
    };
    tampered.domain_pack_remote_cache_entry.entry_digest =
        tampered.entry_digest().expect("cache entry digest");
    assert!(tampered.validate().is_err());

    let entry = |artifact: DomainPackRemoteArtifactDescriptor| DomainPackRemoteCacheEntry {
        cache_key_raw_sha256: artifact.binding.raw_sha256.clone(),
        byte_length: artifact.byte_length,
        artifact,
        source_receipt_digest: digest('7'),
        cached_at_unix: 20,
        entry_digest: digest('8'),
    };
    let mut projection = DomainPackRemoteCacheProjectionDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_cache_projection: DomainPackRemoteCacheProjection {
            cache_id: StableId("cache.example".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            policy: DomainPackRemoteCachePolicy::RejectOnFull {
                max_entry_bytes: 32,
                max_entries: 1,
                max_total_bytes: 64,
            },
            entries: vec![
                entry(artifacts.manifest.clone()),
                entry(artifacts.content.clone()),
            ],
            total_bytes: 64,
            outcome: DomainPackRemoteCacheProjectionOutcome::RejectedOnFull,
            blocks: vec![forge_core_contracts::DomainPackRemoteAcquisitionBlock::CacheFull],
            projection_digest: String::new(),
        },
    };
    projection
        .domain_pack_remote_cache_projection
        .projection_digest = projection
        .projection_digest()
        .expect("cache projection digest");
    assert!(projection.validate().is_err());
}

#[test]
fn generated_schema_has_only_candidate_authority_and_no_lifecycle_shortcut() {
    let schema = serde_json::to_value(schema_for!(DomainPackRemoteAcquisitionRequestDocument))
        .expect("request schema");
    let definitions = schema["$defs"].as_object().expect("schema definitions");
    let authority_values = definitions["DomainPackCandidateAuthority"]["enum"]
        .as_array()
        .expect("candidate authority enum");
    assert_eq!(authority_values, &[serde_json::json!("candidate_only")]);

    let request_properties = definitions["DomainPackRemoteAcquisitionRequest"]["properties"]
        .as_object()
        .expect("request properties");
    for lifecycle_shortcut in ["trusted", "installed", "active"] {
        assert!(
            !request_properties.contains_key(lifecycle_shortcut),
            "remote acquisition request schema exposes lifecycle shortcut {lifecycle_shortcut}"
        );
    }

    let serialized = serde_json::to_string(&schema).expect("serialized request schema");
    assert!(serialized.contains("offline_only"));
    assert!(serialized.contains("reject_on_full"));
}
