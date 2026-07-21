use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_package_record_digest, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
    select_domain_pack_supply_chain_record, verify_domain_pack_supply_chain_snapshot,
    AnchoredDomainPackSupplyChainSnapshot, DomainPackRegistryAnchor,
    DomainPackRegistryAnchorAdvance, SelectedDomainPackSupplyChainRecord,
};
use forge_core_contracts::*;
use forge_core_domain_pack_tcb::acquisition::{
    admit_domain_pack_remote_artifact, cache_domain_pack_remote_artifact,
    lookup_cached_domain_pack_remote_artifact_offline, DomainPackRemoteArtifactAdmissionContext,
    DomainPackRemoteArtifactAdmissionOutcome, DomainPackRemoteCacheLookupContext,
    DomainPackRemoteCacheLookupOutcome, DomainPackRemoteCacheWriteOutcome,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    sha256(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("write hex");
        output
    })
}

fn yaml_binding(name: &str, raw: &[u8]) -> DomainPackArtifactBinding {
    let text = std::str::from_utf8(raw).expect("fixture YAML");
    let value: serde_json::Value = yaml_serde::from_str(text).expect("fixture YAML semantic value");
    DomainPackArtifactBinding {
        artifact_ref: RepoPath(format!("artifacts/{name}.yaml")),
        raw_sha256: sha256(raw),
        canonical_sha256: canonical_digest(&value),
    }
}

fn descriptor(
    kind: DomainPackRemoteArtifactKind,
    name: &str,
    raw: &[u8],
) -> DomainPackRemoteArtifactDescriptor {
    let binding = yaml_binding(name, raw);
    let token = binding.raw_sha256.trim_start_matches("sha256:").to_owned();
    DomainPackRemoteArtifactDescriptor {
        kind,
        binding,
        object_path: RepoPath(format!("objects/sha256/{token}")),
        byte_length: u64::try_from(raw.len()).expect("fixture bytes"),
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

fn policy(
    now: u64,
    registry_key: &SigningKey,
    audience: StableId,
    record: &DomainPackRegistryPackageRecord,
) -> DomainPackTrustPolicyDocument {
    DomainPackTrustPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_trust_policy: DomainPackTrustPolicy {
            policy_id: id("policy.remote-acquisition.test"),
            policy_version: "1".to_owned(),
            audience,
            authority: DomainPackCandidateAuthority::CandidateOnly,
            registry_keys: vec![DomainPackRegistryTrustKey {
                key_id: id("registry-key"),
                role: DomainPackRegistryTrustRole::RegistrySigner,
                public_key_hex: hex(&registry_key.verifying_key().to_bytes()),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: now - 60,
                valid_until_unix: now + 3_600,
            }],
            required_registry_signature_threshold: 1,
            minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            rules: vec![DomainPackTrustRule {
                rule_id: id("trust-rule"),
                pack: DomainPackCoordinate {
                    publisher: record.identity.publisher.clone(),
                    name: record.identity.name.clone(),
                },
                package_digest: Some(record.package_digest.clone()),
                content_digest: Some(record.artifacts.content.binding.canonical_sha256.clone()),
                disposition: DomainPackTrustDisposition::ActivateDeclarativeKnowledge,
            }],
            default_disposition: DomainPackTrustDisposition::Reject,
        },
    }
}

struct RemoteFixture {
    now: u64,
    policy: DomainPackTrustPolicyDocument,
    snapshot: DomainPackSupplyChainRegistryDocument,
    record: DomainPackRegistryPackageRecord,
    location: DomainPackRemoteArtifactLocationBinding,
    raw: Vec<u8>,
    receipt: DomainPackRemoteFetchReceiptDocument,
    anchor: DomainPackRegistryAnchor,
    anchored: AnchoredDomainPackSupplyChainSnapshot,
    selected: SelectedDomainPackSupplyChainRecord,
}

impl RemoteFixture {
    fn new(manifest_name: &str) -> Self {
        Self::with_manifest_descriptor(manifest_name, |_| {})
    }

    #[allow(clippy::too_many_lines)]
    fn with_manifest_descriptor(
        manifest_name: &str,
        mutate: impl FnOnce(&mut DomainPackRemoteArtifactDescriptor),
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs();
        let registry_key = SigningKey::from_bytes(&[11_u8; 32]);
        let publisher_key = SigningKey::from_bytes(&[12_u8; 32]);
        let registry_id = id("registry.remote-acquisition.test");
        let audience = id("audience.remote-acquisition.test");
        let raw = b"name: manifest\nversion: 1\n".to_vec();
        let content_raw = b"content: declarative\n".to_vec();
        let license_raw = b"license: MIT\n".to_vec();
        let mut manifest = descriptor(DomainPackRemoteArtifactKind::Manifest, manifest_name, &raw);
        mutate(&mut manifest);
        let content = descriptor(
            DomainPackRemoteArtifactKind::Content,
            "content",
            &content_raw,
        );
        let license = descriptor(
            DomainPackRemoteArtifactKind::License,
            "license",
            &license_raw,
        );
        let artifacts = DomainPackRegistryArtifactSet {
            manifest: manifest.clone(),
            content: content.clone(),
            license: license.clone(),
            fixtures: Vec::new(),
        };
        let mut record = DomainPackRegistryPackageRecord {
            identity: DomainPackIdentity {
                publisher: id("publisher.remote"),
                name: id("pack.remote"),
                namespace: id("remote"),
                version: "1.0.0".to_owned(),
            },
            package_digest: sha256(b"package.remote"),
            manifest_digest: manifest.binding.raw_sha256.clone(),
            content_digest: content.binding.raw_sha256.clone(),
            license_digest: license.binding.raw_sha256.clone(),
            fixture_digests: Vec::new(),
            artifacts,
            namespace_grant_id: id("grant.remote"),
            publisher_credential_id: id("credential.remote"),
            publisher_signature_hex: "00".repeat(64),
            record_digest: sha256(b"placeholder.record"),
        };
        record.record_digest = domain_pack_package_record_digest(&record).expect("record digest");
        let publisher_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, &record)
            .expect("publisher signing bytes");
        record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());
        let policy = policy(now, &registry_key, audience.clone(), &record);
        let mut snapshot = DomainPackSupplyChainRegistryDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
                registry_id: registry_id.clone(),
                registry_version: "1".to_owned(),
                audience: audience.clone(),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                generation: 1,
                previous_snapshot_digest: None,
                issued_at_unix: now - 60,
                expires_at_unix: now + 3_600,
                publisher_credentials: vec![DomainPackPublisherCredential {
                    credential_id: id("credential.remote"),
                    publisher: record.identity.publisher.clone(),
                    public_key_hex: hex(&publisher_key.verifying_key().to_bytes()),
                    status: DomainPackCredentialStatus::Active,
                    valid_from_unix: now - 60,
                    valid_until_unix: now + 3_600,
                }],
                namespace_grants: vec![DomainPackNamespaceGrant {
                    grant_id: id("grant.remote"),
                    publisher: record.identity.publisher.clone(),
                    namespace_prefix: id("remote"),
                    valid_from_unix: now - 60,
                    valid_until_unix: now + 3_600,
                }],
                mirrors: vec![
                    DomainPackRegistryMirror {
                        mirror_id: id("mirror.remote"),
                        priority: 0,
                        transport: DomainPackRegistryMirrorTransport::Https {
                            base_url: "https://mirror.remote-acquisition.test/domain-packs"
                                .to_owned(),
                        },
                    },
                    DomainPackRegistryMirror {
                        mirror_id: id("mirror.remote.secondary"),
                        priority: 1,
                        transport: DomainPackRegistryMirrorTransport::Https {
                            base_url:
                                "https://mirror-secondary.remote-acquisition.test/domain-packs"
                                    .to_owned(),
                        },
                    },
                ],
                packages: vec![record.clone()],
                revocations: Vec::new(),
                snapshot_digest: sha256(b"placeholder.snapshot"),
                signatures: Vec::new(),
            },
        };
        Self::resign_snapshot(&mut snapshot, &registry_key);
        let verified = verify_domain_pack_supply_chain_snapshot(&policy, &snapshot, now)
            .expect("verified snapshot");
        let mut anchor = DomainPackRegistryAnchor::new_trust_on_first_use(registry_id, audience)
            .expect("registry anchor");
        let version = anchor.version();
        let anchored = match anchor
            .compare_and_advance(&version, verified)
            .expect("advance anchor")
        {
            DomainPackRegistryAnchorAdvance::Advanced(anchored) => anchored,
            DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("fresh anchor replayed"),
        };
        let selected = select_domain_pack_supply_chain_record(&anchored, &record, &policy, now)
            .expect("select record");
        let location = DomainPackRemoteArtifactLocationBinding {
            artifact: manifest,
            mirror_id: id("mirror.remote"),
            object_path: record.artifacts.manifest.object_path.clone(),
        };
        let mut receipt = DomainPackRemoteFetchReceiptDocument {
            schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
            domain_pack_remote_fetch_receipt: DomainPackRemoteFetchReceipt {
                receipt_id: id("receipt.remote"),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                // This direct-admission fixture deliberately has no acquisition
                // plan. Cross-plan integration must bind this to the plan's
                // computed self-digest rather than reuse this isolated value.
                plan_digest: sha256(b"plan.remote"),
                catalog_snapshot_digest: snapshot
                    .domain_pack_supply_chain_registry
                    .snapshot_digest
                    .clone(),
                registry_record_digest: record.record_digest.clone(),
                package_digest: record.package_digest.clone(),
                artifacts: vec![DomainPackRemoteFetchedArtifactReceipt {
                    location: location.clone(),
                    source: DomainPackRemoteFetchSource::NetworkMirror {
                        mirror_id: id("mirror.remote"),
                    },
                    raw_sha256: location.artifact.binding.raw_sha256.clone(),
                    canonical_sha256: location.artifact.binding.canonical_sha256.clone(),
                    byte_length: location.artifact.byte_length,
                    media_type: location.artifact.media_type,
                }],
                outcome: DomainPackRemoteFetchOutcome::CandidateBytesVerified,
                blocks: Vec::new(),
                receipt_digest: String::new(),
            },
        };
        receipt.domain_pack_remote_fetch_receipt.receipt_digest =
            receipt.receipt_digest().expect("receipt digest");
        receipt.validate().expect("valid receipt");
        Self {
            now,
            policy,
            snapshot,
            record,
            location,
            raw,
            receipt,
            anchor,
            anchored,
            selected,
        }
    }

    fn resign_snapshot(snapshot: &mut DomainPackSupplyChainRegistryDocument, key: &SigningKey) {
        {
            let registry = &mut snapshot.domain_pack_supply_chain_registry;
            registry.signatures.clear();
            registry.snapshot_digest = sha256(b"placeholder.snapshot");
        }
        snapshot.domain_pack_supply_chain_registry.snapshot_digest =
            domain_pack_registry_snapshot_digest(snapshot).expect("snapshot digest");
        let bytes = domain_pack_registry_signing_bytes(
            snapshot,
            &id("registry-key"),
            DomainPackRegistryTrustRole::RegistrySigner,
        )
        .expect("registry signing bytes");
        snapshot
            .domain_pack_supply_chain_registry
            .signatures
            .push(DomainPackRegistrySignature {
                signer_key_id: id("registry-key"),
                role: DomainPackRegistryTrustRole::RegistrySigner,
                signature_hex: hex(&key.sign(&bytes).to_bytes()),
            });
    }

    fn admission_context(&self) -> DomainPackRemoteArtifactAdmissionContext<'_> {
        DomainPackRemoteArtifactAdmissionContext {
            anchored_snapshot: &self.anchored,
            selected_record: &self.selected,
            record: &self.record,
            receipt: &self.receipt,
            location: &self.location,
            checked_at_unix: self.now,
        }
    }

    fn lookup_context(&self) -> DomainPackRemoteCacheLookupContext<'_> {
        DomainPackRemoteCacheLookupContext {
            anchored_snapshot: &self.anchored,
            selected_record: &self.selected,
            record: &self.record,
            location: &self.location,
            checked_at_unix: self.now,
        }
    }

    fn advance_revoked_anchor(&mut self) -> AnchoredDomainPackSupplyChainSnapshot {
        let registry_key = SigningKey::from_bytes(&[11_u8; 32]);
        let previous = self
            .snapshot
            .domain_pack_supply_chain_registry
            .snapshot_digest
            .clone();
        let registry = &mut self.snapshot.domain_pack_supply_chain_registry;
        registry.generation = 2;
        registry.previous_snapshot_digest = Some(previous);
        registry.revocations = vec![DomainPackPackageRevocation {
            record_digest: self.record.record_digest.clone(),
            reason: DomainPackRevocationReason::PackageTamper,
            explanation: "fixture revocation".to_owned(),
            revoked_at_unix: self.now,
        }];
        Self::resign_snapshot(&mut self.snapshot, &registry_key);
        let verified =
            verify_domain_pack_supply_chain_snapshot(&self.policy, &self.snapshot, self.now)
                .expect("verified revoked snapshot");
        let version = self.anchor.version();
        match self
            .anchor
            .compare_and_advance(&version, verified)
            .expect("advance revoked anchor")
        {
            DomainPackRegistryAnchorAdvance::Advanced(anchored) => anchored,
            DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("revocation replay"),
        }
    }
}

fn cache_policy(max_entry_bytes: u64) -> DomainPackRemoteCachePolicy {
    DomainPackRemoteCachePolicy::RejectOnFull {
        max_entry_bytes,
        max_entries: 8,
        max_total_bytes: 1024 * 1024,
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let project_root = std::env::temp_dir().join(format!(
        "forge-domain-pack-remote-cache-{label}-{}-{nonce}",
        std::process::id()
    ));
    let state_root = project_root.join(".forge-method");
    fs::create_dir_all(&state_root).expect("state root");
    state_root
}

fn remove_temp_root(state_root: &Path) {
    fs::remove_dir_all(state_root.parent().expect("temporary project root")).expect("cleanup");
}

#[test]
fn admits_caches_and_reads_candidate_bytes_without_lifecycle_mutation() {
    let fixture = RemoteFixture::new("manifest");
    let root = temp_root("hit");
    let candidate =
        match admit_domain_pack_remote_artifact(&fixture.admission_context(), &fixture.raw) {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(candidate) => candidate,
            other => panic!("expected admission, got {other:?}"),
        };
    assert_eq!(candidate.binding(), &fixture.location.artifact.binding);
    assert_eq!(
        candidate.as_immutable_artifact().raw_bytes,
        fixture.raw.as_slice()
    );
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &candidate),
        DomainPackRemoteCacheWriteOutcome::Stored
    );
    assert!(
        !root.join("domain-packs").exists(),
        "candidate cache must not create lifecycle state"
    );
    match lookup_cached_domain_pack_remote_artifact_offline(&root, &fixture.lookup_context()) {
        DomainPackRemoteCacheLookupOutcome::Hit(hit) => {
            assert_eq!(hit.binding(), candidate.binding());
            assert_eq!(hit.raw_bytes(), candidate.raw_bytes());
        }
        other => panic!("expected cache hit, got {other:?}"),
    }
    remove_temp_root(&root);
}

#[test]
fn rejects_invalid_bytes_and_never_caches_them() {
    let fixture = RemoteFixture::new("manifest");
    let root = temp_root("invalid");
    let mut altered = fixture.raw.clone();
    altered[6] = b'X';
    assert!(matches!(
        admit_domain_pack_remote_artifact(&fixture.admission_context(), &altered),
        DomainPackRemoteArtifactAdmissionOutcome::DigestMismatch
    ));
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &fixture.lookup_context()),
        DomainPackRemoteCacheLookupOutcome::Miss
    ));
    remove_temp_root(&root);
}

#[test]
fn validates_canonical_length_media_and_location_bindings() {
    let canonical = RemoteFixture::with_manifest_descriptor("canonical", |descriptor| {
        descriptor.binding.canonical_sha256 = sha256(b"incorrect canonical value");
    });
    assert!(matches!(
        admit_domain_pack_remote_artifact(&canonical.admission_context(), &canonical.raw),
        DomainPackRemoteArtifactAdmissionOutcome::CanonicalDigestMismatch
    ));

    let length = RemoteFixture::with_manifest_descriptor("length", |descriptor| {
        descriptor.byte_length = descriptor.byte_length.saturating_add(1);
    });
    assert!(matches!(
        admit_domain_pack_remote_artifact(&length.admission_context(), &length.raw),
        DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure
    ));

    let media = RemoteFixture::with_manifest_descriptor("media", |descriptor| {
        descriptor.media_type = DomainPackRemoteArtifactMediaType::ApplicationJson;
    });
    assert!(matches!(
        admit_domain_pack_remote_artifact(&media.admission_context(), &media.raw),
        DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure
    ));

    let fixture = RemoteFixture::new("location");
    let mut invalid_location = fixture.location.clone();
    invalid_location.object_path = RepoPath("objects/sha256/not-the-signed-object".to_owned());
    let context = DomainPackRemoteArtifactAdmissionContext {
        anchored_snapshot: &fixture.anchored,
        selected_record: &fixture.selected,
        record: &fixture.record,
        receipt: &fixture.receipt,
        location: &invalid_location,
        checked_at_unix: fixture.now,
    };
    assert!(matches!(
        admit_domain_pack_remote_artifact(&context, &fixture.raw),
        DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure
    ));

    let mut wrong_source_receipt = fixture.receipt.clone();
    wrong_source_receipt
        .domain_pack_remote_fetch_receipt
        .artifacts[0]
        .source = DomainPackRemoteFetchSource::NetworkMirror {
        mirror_id: id("mirror.unanchored"),
    };
    wrong_source_receipt
        .domain_pack_remote_fetch_receipt
        .receipt_digest = wrong_source_receipt
        .receipt_digest()
        .expect("receipt digest");
    let context = DomainPackRemoteArtifactAdmissionContext {
        anchored_snapshot: &fixture.anchored,
        selected_record: &fixture.selected,
        record: &fixture.record,
        receipt: &wrong_source_receipt,
        location: &fixture.location,
        checked_at_unix: fixture.now,
    };
    assert!(matches!(
        admit_domain_pack_remote_artifact(&context, &fixture.raw),
        DomainPackRemoteArtifactAdmissionOutcome::IntegrityFailure
    ));
}

#[test]
fn cache_rejects_full_and_divergent_same_digest_observations() {
    let fixture = RemoteFixture::new("manifest-a");
    let root = temp_root("capacity-equivocation");
    let candidate =
        match admit_domain_pack_remote_artifact(&fixture.admission_context(), &fixture.raw) {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(candidate) => candidate,
            other => panic!("expected admission, got {other:?}"),
        };
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1), &candidate),
        DomainPackRemoteCacheWriteOutcome::CapacityExceeded
    );
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &candidate),
        DomainPackRemoteCacheWriteOutcome::Stored
    );
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &candidate),
        DomainPackRemoteCacheWriteOutcome::AlreadyPresent
    );
    let mut secondary_location = fixture.location.clone();
    secondary_location.mirror_id = id("mirror.remote.secondary");
    let mut secondary_receipt = fixture.receipt.clone();
    secondary_receipt.domain_pack_remote_fetch_receipt.artifacts[0].location =
        secondary_location.clone();
    secondary_receipt.domain_pack_remote_fetch_receipt.artifacts[0].source =
        DomainPackRemoteFetchSource::NetworkMirror {
            mirror_id: secondary_location.mirror_id.clone(),
        };
    secondary_receipt
        .domain_pack_remote_fetch_receipt
        .receipt_digest = secondary_receipt
        .receipt_digest()
        .expect("secondary receipt digest");
    let secondary_context = DomainPackRemoteArtifactAdmissionContext {
        anchored_snapshot: &fixture.anchored,
        selected_record: &fixture.selected,
        record: &fixture.record,
        receipt: &secondary_receipt,
        location: &secondary_location,
        checked_at_unix: fixture.now,
    };
    let secondary_candidate =
        match admit_domain_pack_remote_artifact(&secondary_context, &fixture.raw) {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(candidate) => candidate,
            other => panic!("expected secondary-mirror admission, got {other:?}"),
        };
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &secondary_candidate),
        DomainPackRemoteCacheWriteOutcome::AlreadyPresent
    );
    let divergent = RemoteFixture::new("manifest-b");
    let divergent_candidate =
        match admit_domain_pack_remote_artifact(&divergent.admission_context(), &divergent.raw) {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(candidate) => candidate,
            other => panic!("expected divergent admission, got {other:?}"),
        };
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &divergent_candidate),
        DomainPackRemoteCacheWriteOutcome::MirrorEquivocation
    );
    remove_temp_root(&root);
}

#[test]
fn cache_rehashes_tampered_objects_before_returning_a_hit() {
    let fixture = RemoteFixture::new("tamper");
    let root = temp_root("tamper");
    let candidate =
        match admit_domain_pack_remote_artifact(&fixture.admission_context(), &fixture.raw) {
            DomainPackRemoteArtifactAdmissionOutcome::Admitted(candidate) => candidate,
            other => panic!("expected admission, got {other:?}"),
        };
    assert_eq!(
        cache_domain_pack_remote_artifact(&root, &cache_policy(1024), &candidate),
        DomainPackRemoteCacheWriteOutcome::Stored
    );
    let token = fixture
        .location
        .artifact
        .binding
        .raw_sha256
        .strip_prefix("sha256:")
        .expect("raw digest token");
    let mut tampered = fixture.raw.clone();
    tampered[6] = b'X';
    fs::write(
        root.join("domain-pack-remote-cache/objects").join(token),
        tampered,
    )
    .expect("tamper cached object");
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &fixture.lookup_context()),
        DomainPackRemoteCacheLookupOutcome::DigestMismatch
    ));
    fs::remove_file(root.join("domain-pack-remote-cache/objects").join(token))
        .expect("remove cached object");
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &fixture.lookup_context()),
        DomainPackRemoteCacheLookupOutcome::IntegrityFailure
    ));
    remove_temp_root(&root);
}

#[test]
fn offline_read_remains_miss_stale_or_revoked() {
    let mut fixture = RemoteFixture::new("manifest");
    let root = temp_root("offline");
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &fixture.lookup_context()),
        DomainPackRemoteCacheLookupOutcome::Miss
    ));
    let stale = DomainPackRemoteCacheLookupContext {
        checked_at_unix: fixture.anchored.verified_snapshot().expires_at_unix(),
        ..fixture.lookup_context()
    };
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &stale),
        DomainPackRemoteCacheLookupOutcome::Stale
    ));
    let revoked_anchor = fixture.advance_revoked_anchor();
    let revoked = DomainPackRemoteCacheLookupContext {
        anchored_snapshot: &revoked_anchor,
        selected_record: &fixture.selected,
        record: &fixture.record,
        location: &fixture.location,
        checked_at_unix: fixture.now,
    };
    assert!(matches!(
        lookup_cached_domain_pack_remote_artifact_offline(&root, &revoked),
        DomainPackRemoteCacheLookupOutcome::Revoked
    ));
    remove_temp_root(&root);
}
