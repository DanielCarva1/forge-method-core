use assert_cmd::Command;
use forge_core_contracts::*;
use forge_core_decisions::{
    discover_domain_packs, plan_domain_pack_acquisition, plan_domain_pack_remote_acquisition,
    DomainPackRemoteAcquisitionPlanningInput, DomainPackRemoteCatalogAvailability,
    DomainPackRemoteCatalogFacts,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn command(args: &[&str]) -> std::process::Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(args)
        .output()
        .expect("forge-core command")
}

fn write_yaml<T: serde::Serialize>(path: &Path, value: &T) {
    fs::write(path, yaml_serde::to_string(value).expect("YAML")).expect("fixture file");
}

fn raw_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    raw_sha256(&bytes)
}

fn canonical_yaml_digest(bytes: &[u8]) -> String {
    let text = std::str::from_utf8(bytes).expect("fixture YAML is UTF-8");
    let value: serde_json::Value = yaml_serde::from_str(text).expect("fixture YAML parses");
    canonical_digest(&value)
}

fn digest_character(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn rehash_remote_request(document: &mut DomainPackRemoteAcquisitionRequestDocument) {
    document
        .domain_pack_remote_acquisition_request
        .request_digest = String::new();
    document
        .domain_pack_remote_acquisition_request
        .request_digest = document.request_digest().expect("canonical request digest");
}

fn rehash_remote_plan(document: &mut DomainPackRemoteAcquisitionPlanDocument) {
    document.domain_pack_remote_acquisition_plan.plan_digest = String::new();
    document.domain_pack_remote_acquisition_plan.plan_digest =
        document.plan_digest().expect("canonical plan digest");
}

fn remote_descriptor(
    kind: DomainPackRemoteArtifactKind,
    binding: DomainPackArtifactBinding,
    bytes: &[u8],
    media_type: DomainPackRemoteArtifactMediaType,
) -> DomainPackRemoteArtifactDescriptor {
    DomainPackRemoteArtifactDescriptor {
        kind,
        object_path: RepoPath(format!(
            "objects/sha256/{}",
            &binding.raw_sha256["sha256:".len()..]
        )),
        binding,
        byte_length: u64::try_from(bytes.len()).expect("fixture byte length"),
        media_type,
    }
}

fn cache_projection(
    policy: &DomainPackRemoteCachePolicy,
    artifacts: &[DomainPackRemoteArtifactDescriptor],
) -> DomainPackRemoteCacheProjectionDocument {
    let entries = artifacts
        .iter()
        .map(|artifact| {
            let mut document = DomainPackRemoteCacheEntryDocument {
                schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
                domain_pack_remote_cache_entry: DomainPackRemoteCacheEntry {
                    cache_key_raw_sha256: artifact.binding.raw_sha256.clone(),
                    artifact: artifact.clone(),
                    byte_length: artifact.byte_length,
                    source_receipt_digest: digest_character('f'),
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
            cache_id: StableId("cache.cli.download".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            policy: policy.clone(),
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

struct CandidateByteDownloadFixture {
    root: PathBuf,
    intent_file: PathBuf,
    request_file: PathBuf,
    projection_file: PathBuf,
    catalog_file: PathBuf,
    remote_request_file: PathBuf,
    remote_plan_file: PathBuf,
    cache_projection_file: PathBuf,
    catalog_facts_file: PathBuf,
    artifact_root: PathBuf,
    manifest_file: PathBuf,
    artifact_files: BTreeMap<String, PathBuf>,
}

impl CandidateByteDownloadFixture {
    fn argv(&self) -> Vec<String> {
        vec![
            "domain-pack".to_owned(),
            "acquire".to_owned(),
            "download".to_owned(),
            "--intent-file".to_owned(),
            self.intent_file.display().to_string(),
            "--request-file".to_owned(),
            self.request_file.display().to_string(),
            "--projection-file".to_owned(),
            self.projection_file.display().to_string(),
            "--catalog-file".to_owned(),
            self.catalog_file.display().to_string(),
            "--remote-request-file".to_owned(),
            self.remote_request_file.display().to_string(),
            "--remote-plan-file".to_owned(),
            self.remote_plan_file.display().to_string(),
            "--cache-projection-file".to_owned(),
            self.cache_projection_file.display().to_string(),
            "--catalog-facts-file".to_owned(),
            self.catalog_facts_file.display().to_string(),
            "--artifact-root".to_owned(),
            self.artifact_root.display().to_string(),
            "--json".to_owned(),
        ]
    }

    fn output(&self) -> std::process::Output {
        let argv = self.argv();
        Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .args(argv)
            .output()
            .expect("forge-core download command")
    }
}

#[allow(clippy::too_many_lines)] // This one fixture binds C6.1 derivation and C6.2 cache-only bytes.
fn candidate_byte_download_fixture(label: &str) -> CandidateByteDownloadFixture {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-download-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("fixture root");

    let request: DomainPackDiscoveryRequestDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml"
    ))
    .expect("neutral discovery request");
    let discovery = discover_domain_packs(&request).expect("neutral discovery projection");
    let projection = &discovery.domain_pack_discovery_projection;
    let selected = projection.matches[0].clone();
    let planning = DomainPackAcquisitionPlanningInput {
        intent: DomainPackAcquisitionIntentDocument {
            schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
            domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
                acquisition_id: StableId("acquisition.cli.download".to_owned()),
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
    let acquisition_plan = plan_domain_pack_acquisition(&planning).expect("C6.1 plan");
    let discovery_candidate = &planning.request.domain_pack_discovery_request.candidates[0];
    let base: DomainPackCompositionRequestDocument = yaml_serde::from_str(include_str!(
        "../../../docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"
    ))
    .expect("base candidate fixture");
    let base = base.domain_pack_composition_request;
    let mut candidate_input = base.candidates[0].clone();
    candidate_input.manifest.domain_pack_manifest.identity = DomainPackIdentity {
        publisher: selected.pack.publisher.clone(),
        name: selected.pack.name.clone(),
        namespace: discovery_candidate
            .content
            .domain_pack_content
            .namespace
            .clone(),
        version: selected.pack.version.clone(),
    };
    candidate_input.manifest.domain_pack_manifest.compatibility = DomainPackCompatibility {
        pack_schema_requirement: "^0.1".to_owned(),
        forge_core_requirement: ">=0.12.0, <1.0.0".to_owned(),
    };
    candidate_input.content = discovery_candidate.content.clone();

    let content_bytes = serde_json::to_vec(&candidate_input.content).expect("content JSON");
    candidate_input.manifest.domain_pack_manifest.content = DomainPackContentBinding {
        content_ref: RepoPath("packages/neutral/content.json".to_owned()),
        raw_sha256: raw_sha256(&content_bytes),
        canonical_sha256: canonical_digest(&candidate_input.content),
    };
    assert_eq!(
        candidate_input
            .manifest
            .domain_pack_manifest
            .content
            .canonical_sha256,
        selected.content_digest
    );
    let license_bytes = b"Apache-2.0\n".to_vec();
    candidate_input
        .manifest
        .domain_pack_manifest
        .provenance
        .license_text = DomainPackArtifactBinding {
        artifact_ref: RepoPath("packages/neutral/LICENSE.txt".to_owned()),
        raw_sha256: raw_sha256(&license_bytes),
        canonical_sha256: canonical_digest(
            &std::str::from_utf8(&license_bytes).expect("fixture license UTF-8"),
        ),
    };
    let manifest_bytes = yaml_serde::to_string(&candidate_input.manifest)
        .expect("manifest YAML")
        .into_bytes();
    candidate_input.manifest_binding = DomainPackArtifactBinding {
        artifact_ref: RepoPath("packages/neutral/manifest.yaml".to_owned()),
        raw_sha256: raw_sha256(&manifest_bytes),
        canonical_sha256: canonical_yaml_digest(&manifest_bytes),
    };
    let package = DomainPackPackageBinding {
        package_ref: RepoPath("packages/neutral/package.yaml".to_owned()),
        package_digest: selected.package_digest.clone(),
        manifest: candidate_input.manifest_binding.clone(),
        content: candidate_input
            .manifest
            .domain_pack_manifest
            .content
            .clone(),
        license: candidate_input
            .manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .clone(),
        fixtures: Vec::new(),
    };
    let resolution_candidate = DomainPackResolutionCandidate {
        input: candidate_input,
        package: package.clone(),
        registry_record_digest: Some(selected.supply_chain_record_digest.clone()),
    };
    let manifest = remote_descriptor(
        DomainPackRemoteArtifactKind::Manifest,
        package.manifest.clone(),
        &manifest_bytes,
        DomainPackRemoteArtifactMediaType::ApplicationYaml,
    );
    let remote_package = package.clone();
    let content = remote_descriptor(
        DomainPackRemoteArtifactKind::Content,
        DomainPackArtifactBinding {
            artifact_ref: package.content.content_ref.clone(),
            raw_sha256: package.content.raw_sha256.clone(),
            canonical_sha256: package.content.canonical_sha256.clone(),
        },
        &content_bytes,
        DomainPackRemoteArtifactMediaType::ApplicationJson,
    );
    let license = remote_descriptor(
        DomainPackRemoteArtifactKind::License,
        package.license.clone(),
        &license_bytes,
        DomainPackRemoteArtifactMediaType::TextPlain,
    );
    let artifacts = DomainPackRegistryArtifactSet {
        manifest: manifest.clone(),
        content: content.clone(),
        license: license.clone(),
        fixtures: Vec::new(),
    };
    let snapshot_digest = digest_character('e');
    let registry = DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.cli.download".to_owned()),
            registry_version: "1.0.0".to_owned(),
            audience: StableId("audience.cli.download".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: 10,
            expires_at_unix: 200,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.cli.download".to_owned()),
                publisher: selected.pack.publisher.clone(),
                public_key_hex: "00".repeat(32),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.cli.download".to_owned()),
                publisher: selected.pack.publisher.clone(),
                namespace_prefix: selected.pack.publisher.clone(),
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            mirrors: vec![DomainPackRegistryMirror {
                mirror_id: StableId("mirror.cli.download".to_owned()),
                priority: 0,
                transport: DomainPackRegistryMirrorTransport::Https {
                    base_url: "https://registry.example.invalid/domain-packs".to_owned(),
                },
            }],
            packages: vec![DomainPackRegistryPackageRecord {
                identity: resolution_candidate
                    .input
                    .manifest
                    .domain_pack_manifest
                    .identity
                    .clone(),
                package_digest: package.package_digest.clone(),
                manifest_digest: manifest.binding.raw_sha256.clone(),
                content_digest: content.binding.raw_sha256.clone(),
                license_digest: license.binding.raw_sha256.clone(),
                fixture_digests: Vec::new(),
                artifacts: artifacts.clone(),
                namespace_grant_id: StableId("grant.cli.download".to_owned()),
                publisher_credential_id: StableId("credential.cli.download".to_owned()),
                publisher_signature_hex: "00".repeat(64),
                record_digest: selected.supply_chain_record_digest.clone(),
            }],
            revocations: Vec::new(),
            snapshot_digest: snapshot_digest.clone(),
            signatures: Vec::new(),
        },
    };
    let cache_policy = DomainPackRemoteCachePolicy::RejectOnFull {
        max_entry_bytes: 1_048_576,
        max_entries: 8,
        max_total_bytes: 4_194_304,
    };
    let mut remote_request = DomainPackRemoteAcquisitionRequestDocument {
        schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_remote_acquisition_request: DomainPackRemoteAcquisitionRequest {
            request_id: StableId("remote-request.cli.download".to_owned()),
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
                registry: registry.clone(),
                snapshot_digest: snapshot_digest.clone(),
            },
            package: DomainPackRemotePackageAcquisitionBinding {
                record: registry.domain_pack_supply_chain_registry.packages[0].clone(),
                package: remote_package,
            },
            network_mode: DomainPackRemoteNetworkMode::PreferCache,
            mirror_policy: DomainPackRemoteMirrorPolicy::SignedPriorityThenMirrorId,
            cache_policy: cache_policy.clone(),
            operator_anchored_local_head: None,
            request_digest: String::new(),
        },
    };
    rehash_remote_request(&mut remote_request);
    let manifest_object_path = manifest.object_path.0.clone();
    let artifact_descriptors = vec![manifest, content, license];
    let cache = cache_projection(&cache_policy, &artifact_descriptors);
    let remote_planning = DomainPackRemoteAcquisitionPlanningInput {
        acquisition: planning.clone(),
        acquisition_plan: acquisition_plan.clone(),
        request: remote_request.clone(),
        cache_projection: Some(cache.clone()),
        catalog_facts: DomainPackRemoteCatalogFacts {
            snapshot_digest,
            availability: DomainPackRemoteCatalogAvailability::CurrentAnchored,
            host_checked_at_unix: 100,
        },
    };
    let remote_plan = plan_domain_pack_remote_acquisition(&remote_planning)
        .expect("cache-only remote plan")
        .plan;
    assert_eq!(
        remote_plan.domain_pack_remote_acquisition_plan.outcome,
        DomainPackRemoteAcquisitionPlanOutcome::CacheOnlyCandidateBytesRequired
    );

    let intent_file = root.join("intent.yaml");
    let request_file = root.join("request.yaml");
    let projection_file = root.join("projection.yaml");
    let catalog_file = root.join("catalog.yaml");
    let remote_request_file = root.join("remote-request.yaml");
    let remote_plan_file = root.join("remote-plan.yaml");
    let cache_projection_file = root.join("cache-projection.yaml");
    let catalog_facts_file = root.join("catalog-facts.yaml");
    let artifact_root = root.join("artifacts");
    write_yaml(&intent_file, &planning.intent);
    write_yaml(&request_file, &planning.request);
    write_yaml(&projection_file, &planning.discovery);
    write_yaml(
        &catalog_file,
        &DomainPackAcquisitionCatalogDocument {
            schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
            forge_core_version: "0.12.0".to_owned(),
            core: base.core,
            registry,
            candidates: vec![resolution_candidate],
        },
    );
    write_yaml(&remote_request_file, &remote_request);
    write_yaml(&remote_plan_file, &remote_plan);
    write_yaml(&cache_projection_file, &cache);
    write_yaml(
        &catalog_facts_file,
        &serde_json::json!({
            "snapshot_digest": remote_planning.catalog_facts.snapshot_digest,
            "availability": "current_anchored",
            "host_checked_at_unix": remote_planning.catalog_facts.host_checked_at_unix,
        }),
    );

    let mut artifact_files = BTreeMap::new();
    for (descriptor, bytes) in
        artifact_descriptors
            .into_iter()
            .zip([manifest_bytes, content_bytes, license_bytes])
    {
        let path = artifact_root.join(&descriptor.object_path.0);
        fs::create_dir_all(path.parent().expect("artifact parent")).expect("artifact directories");
        fs::write(&path, bytes).expect("artifact bytes");
        artifact_files.insert(descriptor.object_path.0, path);
    }
    CandidateByteDownloadFixture {
        root,
        intent_file,
        request_file,
        projection_file,
        catalog_file,
        remote_request_file,
        remote_plan_file,
        cache_projection_file,
        catalog_facts_file,
        manifest_file: artifact_root.join(manifest_object_path),
        artifact_root,
        artifact_files,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One subprocess journey proves replay, selection, and no mutation.
fn acquisition_plan_selects_only_exact_current_candidate_and_requires_trust() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-acquisition-plan-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let request = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml");
    let search = command(&[
        "domain-pack",
        "search",
        "--request-file",
        &request.display().to_string(),
        "--json",
    ]);
    assert!(
        search.status.success(),
        "search failed: {}",
        String::from_utf8_lossy(&search.stderr)
    );
    let search: Value = serde_json::from_slice(&search.stdout).expect("search envelope");
    let discovery: DomainPackDiscoveryProjectionDocument =
        serde_json::from_value(search["data"].clone()).expect("typed discovery projection");
    let projection = &discovery.domain_pack_discovery_projection;
    let selected = &projection.matches[0];
    let intent = DomainPackAcquisitionIntentDocument {
        schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
            acquisition_id: StableId("acquisition.cli.neutral".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            assurance_binding: projection.assurance_binding.clone(),
            discovery_projection_digest: projection.projection_digest.clone(),
            demand_digest: projection.demand_digest.clone(),
            candidate_id: selected.candidate_id.clone(),
            requirement_ref: selected.requirement_ref.clone(),
            operation: DomainPackAcquisitionOperation::Install,
            expected_project_snapshot_digest: projection.assurance_binding.snapshot_digest.clone(),
        },
    };
    let intent_path = root.join("intent.yaml");
    let projection_path = root.join("projection.yaml");
    write_yaml(&intent_path, &intent);
    write_yaml(&projection_path, &discovery);

    let plan = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &request.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(
        plan.status.success(),
        "plan failed: stdout={} stderr={}",
        String::from_utf8_lossy(&plan.stdout),
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan: Value = serde_json::from_slice(&plan.stdout).expect("plan envelope");
    assert_eq!(plan["command"], "domain-pack acquire plan");
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["status"],
        "trust_ceremony_required"
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["selected"]["candidate_id"],
        selected.candidate_id.0
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["requirements"]["project_id"],
        projection.assurance_binding.project_id.0
    );
    assert!(
        plan["data"]["domain_pack_acquisition_plan"]["discovery_request_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["required_ceremonies"]
            .as_array()
            .expect("ceremonies")
            .len(),
        6
    );
    assert!(
        !root.join(".forge-method/domain-packs/active.yaml").exists(),
        "read-only planning must not create lifecycle state"
    );

    let mut stale_intent = intent.clone();
    stale_intent.domain_pack_acquisition_intent.candidate_id =
        StableId("candidate.absent".to_owned());
    write_yaml(&intent_path, &stale_intent);
    let rejected = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &request.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(!rejected.status.success());

    let mut stale_request: forge_core_contracts::DomainPackDiscoveryRequestDocument =
        yaml_serde::from_slice(&fs::read(&request).expect("request corpus"))
            .expect("typed request corpus");
    stale_request
        .domain_pack_discovery_request
        .uncertainties
        .push("changed after projection".to_owned());
    let stale_request_path = root.join("stale-request.yaml");
    write_yaml(&stale_request_path, &stale_request);
    write_yaml(&intent_path, &intent);
    let replay_rejected = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &stale_request_path.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(!replay_rejected.status.success());
}

#[test]
fn candidate_byte_download_emits_only_a_ready_candidate_receipt_without_lifecycle_effects() {
    let fixture = candidate_byte_download_fixture("ready");
    let before = fixture
        .artifact_files
        .iter()
        .map(|(object, path)| (object.clone(), fs::read(path).expect("artifact before")))
        .collect::<BTreeMap<_, _>>();

    let output = fixture.output();
    assert!(
        output.status.success(),
        "candidate byte download failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("download envelope");
    assert_eq!(envelope["command"], "domain-pack acquire download");
    assert_eq!(envelope["data"]["status"], "ready_for_trust_evaluation");
    assert_eq!(envelope["data"]["authority"], "candidate_only");
    assert_eq!(
        envelope["data"]["receipt"]["domain_pack_remote_fetch_receipt"]["outcome"],
        "candidate_bytes_verified"
    );
    for forbidden in ["trusted", "reviewed", "installed", "active", "generation"] {
        assert!(
            envelope["data"].get(forbidden).is_none(),
            "candidate-only output must not assert {forbidden} authority"
        );
    }
    let after = fixture
        .artifact_files
        .iter()
        .map(|(object, path)| (object.clone(), fs::read(path).expect("artifact after")))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        after, before,
        "download must not mutate local candidate bytes"
    );
    assert!(
        !fixture.root.join(".forge-method").exists(),
        "candidate-byte verification must not create lifecycle state"
    );
    assert!(
        !fixture.root.join("domain-packs").exists(),
        "candidate-byte verification must not create cache or installation state"
    );
    let _ = fs::remove_dir_all(fixture.root);
}

#[test]
fn candidate_byte_download_rejects_replayed_plan_and_incomplete_or_extra_local_artifacts() {
    let stale = candidate_byte_download_fixture("stale-plan");
    let mut plan: DomainPackRemoteAcquisitionPlanDocument =
        yaml_serde::from_slice(&fs::read(&stale.remote_plan_file).expect("remote plan"))
            .expect("typed remote plan");
    plan.domain_pack_remote_acquisition_plan.request_digest = digest_character('a');
    rehash_remote_plan(&mut plan);
    write_yaml(&stale.remote_plan_file, &plan);
    let stale_output = stale.output();
    assert!(
        !stale_output.status.success(),
        "replayed plan must be rejected"
    );
    assert!(
        !stale.root.join(".forge-method").exists(),
        "replayed plan must not create lifecycle state"
    );
    let _ = fs::remove_dir_all(stale.root);

    let traversal = candidate_byte_download_fixture("traversal");
    let mut traversal_plan: DomainPackRemoteAcquisitionPlanDocument =
        yaml_serde::from_slice(&fs::read(&traversal.remote_plan_file).expect("remote plan"))
            .expect("typed remote plan");
    traversal_plan.domain_pack_remote_acquisition_plan.artifacts[0].object_path =
        RepoPath("../outside-artifact".to_owned());
    rehash_remote_plan(&mut traversal_plan);
    write_yaml(&traversal.remote_plan_file, &traversal_plan);
    let traversal_output = traversal.output();
    assert!(
        !traversal_output.status.success(),
        "traversal object paths must be rejected before local artifact access"
    );
    assert!(!traversal.root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(traversal.root);

    let missing = candidate_byte_download_fixture("missing");
    let missing_path = missing
        .artifact_files
        .values()
        .next()
        .expect("selected artifact")
        .clone();
    fs::remove_file(missing_path).expect("remove selected artifact");
    let missing_output = missing.output();
    assert!(
        !missing_output.status.success(),
        "missing signed selected artifact must be rejected"
    );
    assert!(!missing.root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(missing.root);

    let extra = candidate_byte_download_fixture("extra");
    let extra_path = extra
        .artifact_root
        .join(format!("objects/sha256/{}", "e".repeat(64)));
    fs::create_dir_all(extra_path.parent().expect("extra parent")).expect("extra parent directory");
    fs::write(&extra_path, b"unselected local bytes\n").expect("extra artifact");
    let extra_output = extra.output();
    assert!(
        !extra_output.status.success(),
        "unselected artifact object must be rejected"
    );
    assert!(!extra.root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(extra.root);
}

#[test]
#[allow(clippy::too_many_lines)]
fn candidate_byte_download_blocks_bad_bytes_and_rejects_oversize_or_symlink_artifacts() {
    let raw_mismatch = candidate_byte_download_fixture("raw-mismatch");
    let original_manifest = fs::read(&raw_mismatch.manifest_file).expect("selected manifest");
    let mut raw_changed_manifest = original_manifest.clone();
    let terminal_newline = raw_changed_manifest
        .iter()
        .rposition(|byte| *byte == b'\n')
        .expect("serialized manifest terminator");
    raw_changed_manifest[terminal_newline] = b' ';
    fs::write(&raw_mismatch.manifest_file, raw_changed_manifest)
        .expect("alter selected manifest bytes without changing its canonical value");
    let mismatch_output = raw_mismatch.output();
    assert!(
        mismatch_output.status.success(),
        "a digest mismatch must become a closed candidate-only receipt"
    );
    let mismatch: Value =
        serde_json::from_slice(&mismatch_output.stdout).expect("blocked envelope");
    assert_eq!(mismatch["data"]["status"], "blocked");
    assert_eq!(
        mismatch["data"]["receipt"]["domain_pack_remote_fetch_receipt"]["outcome"],
        "blocked"
    );
    assert!(
        mismatch["data"]["evidence"]["domain_pack_remote_fetch_evidence"]["blocks"]
            .as_array()
            .expect("raw mismatch blocks")
            .iter()
            .any(|block| block == "artifact_raw_digest_mismatch"),
        "raw cache corruption must remain typed candidate evidence"
    );
    assert!(
        !raw_mismatch.root.join(".forge-method").exists(),
        "bad candidate bytes must not advance lifecycle state"
    );
    let _ = fs::remove_dir_all(raw_mismatch.root);

    let canonical_mismatch = candidate_byte_download_fixture("canonical-mismatch");
    let mut canonical_changed_manifest =
        String::from_utf8(fs::read(&canonical_mismatch.manifest_file).expect("selected manifest"))
            .expect("manifest UTF-8");
    let mut replaced = false;
    for (expected, replacement) in [("'0.1'", "'9.1'"), ("\"0.1\"", "\"9.1\""), ("0.1", "9.1")] {
        if let Some(offset) = canonical_changed_manifest.find(expected) {
            canonical_changed_manifest.replace_range(offset..offset + expected.len(), replacement);
            replaced = true;
            break;
        }
    }
    assert!(replaced, "serialized manifest has a mutable version value");
    fs::write(
        &canonical_mismatch.manifest_file,
        canonical_changed_manifest.as_bytes(),
    )
    .expect("alter selected manifest semantics without changing its byte length");
    let canonical_output = canonical_mismatch.output();
    assert!(
        canonical_output.status.success(),
        "a canonical digest mismatch must become a closed candidate-only receipt"
    );
    let canonical: Value =
        serde_json::from_slice(&canonical_output.stdout).expect("canonical blocked envelope");
    assert_eq!(canonical["data"]["status"], "blocked");
    assert!(
        canonical["data"]["evidence"]["domain_pack_remote_fetch_evidence"]["blocks"]
            .as_array()
            .expect("canonical mismatch blocks")
            .iter()
            .any(|block| block == "artifact_canonical_digest_mismatch"),
        "the exact canonical mismatch must remain typed candidate evidence"
    );
    assert!(!canonical_mismatch.root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(canonical_mismatch.root);

    let oversized = candidate_byte_download_fixture("oversized");
    let oversized_path = oversized.manifest_file.clone();
    fs::OpenOptions::new()
        .write(true)
        .open(&oversized_path)
        .expect("open selected artifact")
        .set_len(1_048_577)
        .expect("make selected artifact oversize");
    let oversized_output = oversized.output();
    assert!(
        !oversized_output.status.success(),
        "bounded local reading must reject an oversize artifact"
    );
    assert!(!oversized.root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(oversized.root);

    #[cfg(unix)]
    {
        let symlink = candidate_byte_download_fixture("symlink");
        let artifact = symlink.manifest_file.clone();
        let target = symlink.root.join("outside-artifact");
        fs::write(&target, b"outside bytes\n").expect("symlink target");
        fs::remove_file(&artifact).expect("remove selected artifact");
        std::os::unix::fs::symlink(&target, &artifact).expect("create artifact symlink");
        let symlink_output = symlink.output();
        assert!(
            !symlink_output.status.success(),
            "symbolic links in the artifact root must be rejected"
        );
        assert!(!symlink.root.join(".forge-method").exists());
        let _ = fs::remove_dir_all(symlink.root);

        let special = candidate_byte_download_fixture("special-file");
        let artifact = special.manifest_file.clone();
        fs::remove_file(&artifact).expect("remove selected artifact");
        // AF_UNIX bind paths are limited to SUN_LEN, while the canonical object
        // path is intentionally long. Bind at a short sibling temp path, then
        // rename the same socket inode into the selected artifact path.
        let socket_path = std::env::temp_dir().join(format!(
            "forge-dp-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let socket = std::os::unix::net::UnixListener::bind(&socket_path)
            .expect("create short special socket");
        fs::rename(&socket_path, &artifact).expect("move special socket into artifact path");
        let special_output = special.output();
        assert!(
            !special_output.status.success(),
            "non-regular special files in the artifact root must be rejected"
        );
        assert!(!special.root.join(".forge-method").exists());
        drop(socket);
        let _ = fs::remove_dir_all(special.root);
    }
}

#[test]
fn candidate_byte_download_surface_rejects_transport_and_ambiguous_authority_flags() {
    for args in [
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--url",
            "https://example.invalid",
        ],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--endpoint",
            "example.invalid",
        ],
        vec!["domain-pack", "acquire", "download", "--network", "online"],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--downloader-command",
            "curl",
        ],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--downloader-executable",
            "curl",
        ],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "https://example.invalid",
        ],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--artifact-root",
            "first",
            "--artifact-root",
            "second",
        ],
        vec![
            "domain-pack",
            "acquire",
            "download",
            "--intent-file",
            "first.yaml",
            "--intent-file",
            "second.yaml",
        ],
    ] {
        let output = command(&args);
        assert!(
            !output.status.success(),
            "candidate-only local download must reject unsupported or duplicate authority input: {args:?}"
        );
    }

    let missing_root = candidate_byte_download_fixture("missing-artifact-root");
    let mut missing_root_argv = missing_root.argv();
    let artifact_root_flag = missing_root_argv
        .iter()
        .position(|argument| argument == "--artifact-root")
        .expect("artifact-root flag");
    missing_root_argv.drain(artifact_root_flag..=artifact_root_flag + 1);
    let missing_root_output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(&missing_root_argv)
        .output()
        .expect("missing artifact-root command");
    assert!(
        !missing_root_output.status.success(),
        "--artifact-root is mandatory even when every other typed input is supplied"
    );
    assert!(
        !missing_root.root.join(".forge-method").exists(),
        "missing artifact-root rejection must not create lifecycle state"
    );
    assert!(
        !missing_root.root.join("domain-packs").exists(),
        "missing artifact-root rejection must not create cache or installation state"
    );
    let _ = fs::remove_dir_all(missing_root.root);
}

#[test]
fn candidate_byte_download_help_is_projected_from_command_surface() {
    let output = command(&["domain-pack", "acquire", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains(
        "forge-core domain-pack acquire download --intent-file <path> --request-file <path> --projection-file <path> --catalog-file <path> --remote-request-file <path> --remote-plan-file <path> --cache-projection-file <path> --catalog-facts-file <path> --artifact-root <path> [--json|--no-json]"
    ));
    assert!(help.contains("forge-core domain-pack acquire derive-initialized"));
}

fn initialized_install_intent() -> DomainPackInitializedProjectIntentDocument {
    let selection = DomainPackInitializedProjectCandidateSelection {
        acquisition_id: StableId("acquisition.initialized.cli".to_owned()),
        assurance_binding: DurableAssuranceEpochBinding {
            project_id: StableId("project.initialized.cli".to_owned()),
            assurance_epoch: 7,
            intent_id: StableId("intent.initialized.cli".to_owned()),
            intent_revision: 3,
            intent_digest: "sha256:intent".to_owned(),
            accepted_record_digest: "sha256:record".to_owned(),
            accepted_sequence: 11,
            accepted_state_version: 5,
            snapshot_digest: "sha256:snapshot".to_owned(),
            ledger_head_before_acceptance: "sha256:ledger".to_owned(),
        },
        discovery_projection_digest: "sha256:projection".to_owned(),
        demand_digest: "sha256:demand".to_owned(),
        candidate_id: StableId("candidate.initialized.cli".to_owned()),
        requirement_ref: StableId("requirement.initialized.cli".to_owned()),
        approval: DomainPackCandidateApprovalRequirement::ExplicitOperatorApprovalRequired,
    };
    DomainPackInitializedProjectIntentDocument {
        schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
        domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
            intent_id: StableId("intent.initialized.cli".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: StableId("project.initialized.cli".to_owned()),
            principal_id: StableId("principal.initialized.cli".to_owned()),
            expected_state: DomainPackInitializedProjectStateBinding {
                generation: 7,
                active_lock_digest: "sha256:active-lock".to_owned(),
                lifecycle_head_digest: "sha256:lifecycle-head".to_owned(),
                project_snapshot_digest: "sha256:project-snapshot".to_owned(),
            },
            operation: DomainPackInitializedProjectOperation::Install { selection },
        },
    }
}

#[test]
fn initialized_derivation_refuses_missing_or_mismatched_candidate_approval_before_state_access() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-initialized-approval-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let intent_path = root.join("initialized-intent.yaml");
    let state_root = root.join("retained-state");
    write_yaml(&intent_path, &initialized_install_intent());

    let missing = command(&[
        "domain-pack",
        "acquire",
        "derive-initialized",
        "--intent-file",
        &intent_path.display().to_string(),
        "--state-root",
        &state_root.display().to_string(),
        "--json",
    ]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("operator-approve-candidate"),
        "missing approval must fail at the candidate-only command boundary"
    );
    assert!(
        !state_root.exists(),
        "candidate-approval refusal must not initialize or modify lifecycle state"
    );

    let mismatched = command(&[
        "domain-pack",
        "acquire",
        "derive-initialized",
        "--intent-file",
        &intent_path.display().to_string(),
        "--operator-approve-candidate",
        "candidate.other",
        "--state-root",
        &state_root.display().to_string(),
        "--json",
    ]);
    assert!(!mismatched.status.success());
    assert!(
        String::from_utf8_lossy(&mismatched.stderr).contains("exact selected candidate"),
        "approval must name the selection's exact candidate identity"
    );
    assert!(
        !state_root.exists(),
        "mismatched approval must remain non-authoritative and non-mutating"
    );
}

#[test]
fn initialized_derivation_rejects_duplicate_authority_bearing_flags_before_state_access() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-initialized-duplicate-flags-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let intent_path = root.join("initialized-intent.yaml");
    write_yaml(&intent_path, &initialized_install_intent());

    for flag in [
        "--intent-file",
        "--candidate-input-file",
        "--target-catalog-file",
        "--operator-approve-candidate",
        "--state-root",
    ] {
        let mut args = vec![
            "domain-pack".to_owned(),
            "acquire".to_owned(),
            "derive-initialized".to_owned(),
            "--intent-file".to_owned(),
            intent_path.display().to_string(),
        ];
        args.extend([
            flag.to_owned(),
            root.join("first").display().to_string(),
            flag.to_owned(),
            root.join("second").display().to_string(),
            "--json".to_owned(),
        ]);
        let output = Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .args(&args)
            .output()
            .expect("duplicate initialized derivation command");
        assert!(
            !output.status.success(),
            "derive-initialized must reject duplicate {flag}"
        );
    }
    assert!(
        !root.join("first").exists() && !root.join("second").exists(),
        "duplicate parser refusal must precede retained-state access"
    );
    let _ = fs::remove_dir_all(root);
}
