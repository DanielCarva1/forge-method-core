use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_cumulative_revocation_digest, domain_pack_package_record_digest,
    domain_pack_publisher_signing_bytes, domain_pack_registry_signing_bytes,
    domain_pack_registry_snapshot_digest, select_domain_pack_supply_chain_record,
    verify_domain_pack_supply_chain_snapshot, DomainPackRegistryAnchor,
    DomainPackRegistryAnchorAdvance, DomainPackSupplyChainAuditAuthority,
    DomainPackSupplyChainError,
};
use forge_core_contracts::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCoordinate,
    DomainPackCredentialStatus, DomainPackIdentity, DomainPackNamespaceGrant,
    DomainPackPackageRevocation, DomainPackPublisherCredential, DomainPackRegistryArtifactSet,
    DomainPackRegistryMirror, DomainPackRegistryMirrorTransport, DomainPackRegistryPackageRecord,
    DomainPackRegistrySignature, DomainPackRegistryTrustKey, DomainPackRegistryTrustRole,
    DomainPackRemoteArtifactDescriptor, DomainPackRemoteArtifactKind,
    DomainPackRemoteArtifactMediaType, DomainPackRevocationReason, DomainPackSourceAssurance,
    DomainPackSupplyChainRegistry, DomainPackSupplyChainRegistryDocument,
    DomainPackTrustDisposition, DomainPackTrustPolicy, DomainPackTrustPolicyDocument,
    DomainPackTrustRule, RepoPath, StableId, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use std::fmt::Write as _;

const ISSUED: u64 = 200;
const EXPIRES: u64 = 800;
const NOW: u64 = 400;

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn artifact(
    kind: DomainPackRemoteArtifactKind,
    logical_path: &str,
    raw_digest: char,
    canonical_digest: char,
    byte_length: u64,
    media_type: DomainPackRemoteArtifactMediaType,
) -> DomainPackRemoteArtifactDescriptor {
    let raw_sha256 = digest(raw_digest);
    DomainPackRemoteArtifactDescriptor {
        kind,
        binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath(logical_path.to_owned()),
            raw_sha256: raw_sha256.clone(),
            canonical_sha256: digest(canonical_digest),
        },
        object_path: RepoPath(format!("objects/sha256/{}", &raw_sha256[7..])),
        byte_length,
        media_type,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

struct Fixture {
    policy: DomainPackTrustPolicyDocument,
    snapshot: DomainPackSupplyChainRegistryDocument,
    registry_keys: Vec<(StableId, SigningKey)>,
    publisher_key: SigningKey,
}

impl Fixture {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let registry_keys = vec![
            (id("registry.key.a"), SigningKey::from_bytes(&[1_u8; 32])),
            (id("registry.key.b"), SigningKey::from_bytes(&[2_u8; 32])),
        ];
        let publisher_key = SigningKey::from_bytes(&[3_u8; 32]);
        let policy = DomainPackTrustPolicyDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_trust_policy: DomainPackTrustPolicy {
                policy_id: id("policy.domain-pack.supply-chain"),
                policy_version: "1.0.0".to_owned(),
                audience: id("forge.domain-pack.project.test"),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                registry_keys: registry_keys
                    .iter()
                    .map(|(key_id, key)| DomainPackRegistryTrustKey {
                        key_id: key_id.clone(),
                        role: DomainPackRegistryTrustRole::RegistrySigner,
                        public_key_hex: hex(&key.verifying_key().to_bytes()),
                        status: DomainPackCredentialStatus::Active,
                        valid_from_unix: 100,
                        valid_until_unix: 900,
                    })
                    .collect(),
                required_registry_signature_threshold: 2,
                minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
                rules: vec![DomainPackTrustRule {
                    rule_id: id("rule.allow.fixture"),
                    pack: DomainPackCoordinate {
                        publisher: id("publisher.fixture"),
                        name: id("foundation"),
                    },
                    package_digest: None,
                    content_digest: None,
                    disposition: DomainPackTrustDisposition::InspectOnly,
                }],
                default_disposition: DomainPackTrustDisposition::Reject,
            },
        };
        let artifacts = DomainPackRegistryArtifactSet {
            manifest: artifact(
                DomainPackRemoteArtifactKind::Manifest,
                "packs/foundation/manifest.yaml",
                '2',
                'a',
                48,
                DomainPackRemoteArtifactMediaType::ApplicationYaml,
            ),
            content: artifact(
                DomainPackRemoteArtifactKind::Content,
                "packs/foundation/content.yaml",
                '3',
                'b',
                96,
                DomainPackRemoteArtifactMediaType::ApplicationYaml,
            ),
            license: artifact(
                DomainPackRemoteArtifactKind::License,
                "packs/foundation/LICENSE",
                '4',
                'c',
                24,
                DomainPackRemoteArtifactMediaType::TextPlain,
            ),
            fixtures: vec![artifact(
                DomainPackRemoteArtifactKind::Fixture,
                "packs/foundation/fixtures/base.json",
                '5',
                'd',
                64,
                DomainPackRemoteArtifactMediaType::ApplicationJson,
            )],
        };
        let record = DomainPackRegistryPackageRecord {
            identity: DomainPackIdentity {
                publisher: id("publisher.fixture"),
                name: id("foundation"),
                namespace: id("sample.foundation"),
                version: "1.0.0".to_owned(),
            },
            package_digest: digest('1'),
            manifest_digest: artifacts.manifest.binding.raw_sha256.clone(),
            content_digest: artifacts.content.binding.raw_sha256.clone(),
            license_digest: artifacts.license.binding.raw_sha256.clone(),
            fixture_digests: artifacts
                .fixtures
                .iter()
                .map(|fixture| fixture.binding.raw_sha256.clone())
                .collect(),
            artifacts,
            namespace_grant_id: id("grant.publisher.fixture"),
            publisher_credential_id: id("publisher.credential.fixture"),
            publisher_signature_hex: "00".repeat(64),
            record_digest: digest('0'),
        };
        let snapshot = DomainPackSupplyChainRegistryDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
                registry_id: id("registry.domain-pack.test"),
                registry_version: "1.0.0".to_owned(),
                audience: id("forge.domain-pack.project.test"),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                generation: 1,
                previous_snapshot_digest: None,
                issued_at_unix: ISSUED,
                expires_at_unix: EXPIRES,
                publisher_credentials: vec![DomainPackPublisherCredential {
                    credential_id: id("publisher.credential.fixture"),
                    publisher: id("publisher.fixture"),
                    public_key_hex: hex(&publisher_key.verifying_key().to_bytes()),
                    status: DomainPackCredentialStatus::Active,
                    valid_from_unix: 100,
                    valid_until_unix: 900,
                }],
                namespace_grants: vec![DomainPackNamespaceGrant {
                    grant_id: id("grant.publisher.fixture"),
                    publisher: id("publisher.fixture"),
                    namespace_prefix: id("sample"),
                    valid_from_unix: 100,
                    valid_until_unix: 900,
                }],
                mirrors: vec![
                    DomainPackRegistryMirror {
                        mirror_id: id("mirror.primary"),
                        priority: 0,
                        transport: DomainPackRegistryMirrorTransport::Https {
                            base_url: "https://registry.example.invalid/domain-packs".to_owned(),
                        },
                    },
                    DomainPackRegistryMirror {
                        mirror_id: id("mirror.operator-cache"),
                        priority: 10,
                        transport: DomainPackRegistryMirrorTransport::OperatorProvisionedLocal {
                            location_id: id("operator.registry.cache"),
                        },
                    },
                ],
                packages: vec![record],
                revocations: Vec::new(),
                snapshot_digest: digest('0'),
                signatures: Vec::new(),
            },
        };
        let mut fixture = Self {
            policy,
            snapshot,
            registry_keys,
            publisher_key,
        };
        fixture.seal_record(0);
        fixture.seal_snapshot();
        fixture
    }

    fn seal_record(&mut self, index: usize) {
        let registry = &mut self.snapshot.domain_pack_supply_chain_registry;
        let record = &mut registry.packages[index];
        record.record_digest = domain_pack_package_record_digest(record).expect("record digest");
        let bytes =
            domain_pack_publisher_signing_bytes(&registry.registry_id, &registry.audience, record)
                .expect("publisher signing bytes");
        record.publisher_signature_hex = hex(&self.publisher_key.sign(&bytes).to_bytes());
    }

    fn seal_snapshot(&mut self) {
        self.snapshot
            .domain_pack_supply_chain_registry
            .signatures
            .clear();
        self.snapshot
            .domain_pack_supply_chain_registry
            .snapshot_digest =
            domain_pack_registry_snapshot_digest(&self.snapshot).expect("snapshot digest");
        for (key_id, key) in &self.registry_keys {
            let bytes = domain_pack_registry_signing_bytes(
                &self.snapshot,
                key_id,
                DomainPackRegistryTrustRole::RegistrySigner,
            )
            .expect("registry signing bytes");
            self.snapshot
                .domain_pack_supply_chain_registry
                .signatures
                .push(DomainPackRegistrySignature {
                    signer_key_id: key_id.clone(),
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                    signature_hex: hex(&key.sign(&bytes).to_bytes()),
                });
        }
    }

    fn refresh_snapshot_digest_without_resigning(&mut self) {
        self.snapshot
            .domain_pack_supply_chain_registry
            .snapshot_digest =
            domain_pack_registry_snapshot_digest(&self.snapshot).expect("snapshot digest");
    }

    fn revoke_record(&mut self, index: usize, explanation: &str) {
        let record_digest = self.snapshot.domain_pack_supply_chain_registry.packages[index]
            .record_digest
            .clone();
        self.snapshot
            .domain_pack_supply_chain_registry
            .revocations
            .push(DomainPackPackageRevocation {
                record_digest,
                reason: DomainPackRevocationReason::PackageTamper,
                explanation: explanation.to_owned(),
                revoked_at_unix: 300,
            });
    }

    fn add_unrelated_record(&mut self) {
        let mut record = self.snapshot.domain_pack_supply_chain_registry.packages[0].clone();
        record.identity.name = id("unrelated");
        record.identity.namespace = id("sample.unrelated");
        "2.0.0".clone_into(&mut record.identity.version);
        self.snapshot
            .domain_pack_supply_chain_registry
            .packages
            .push(record);
        self.seal_record(1);
    }

    fn set_generation(
        &mut self,
        generation: u64,
        previous_snapshot_digest: Option<String>,
        registry_version: &str,
    ) {
        let registry = &mut self.snapshot.domain_pack_supply_chain_registry;
        registry.generation = generation;
        registry.previous_snapshot_digest = previous_snapshot_digest;
        registry_version.clone_into(&mut registry.registry_version);
        self.seal_snapshot();
    }

    fn verify(&self) -> forge_core_authority::VerifiedDomainPackSupplyChainSnapshot {
        verify_domain_pack_supply_chain_snapshot(&self.policy, &self.snapshot, NOW)
            .expect("valid signed snapshot")
    }
}

fn empty_anchor() -> DomainPackRegistryAnchor {
    DomainPackRegistryAnchor::new_trust_on_first_use(
        id("registry.domain-pack.test"),
        id("forge.domain-pack.project.test"),
    )
    .expect("valid anchor identity")
}

fn assert_signed_mirror_metadata_rejects<F>(mutate: F)
where
    F: FnOnce(&mut DomainPackSupplyChainRegistry),
{
    let mut fixture = Fixture::new();
    let signer_key_id = fixture.registry_keys[0].0.clone();
    let verifying_key = fixture.registry_keys[0].1.verifying_key();
    let original_bytes = domain_pack_registry_signing_bytes(
        &fixture.snapshot,
        &signer_key_id,
        DomainPackRegistryTrustRole::RegistrySigner,
    )
    .expect("original registry signing bytes");
    let signature = fixture.registry_keys[0].1.sign(&original_bytes);

    mutate(&mut fixture.snapshot.domain_pack_supply_chain_registry);
    let tampered_bytes = domain_pack_registry_signing_bytes(
        &fixture.snapshot,
        &signer_key_id,
        DomainPackRegistryTrustRole::RegistrySigner,
    )
    .expect("tampered registry signing bytes");
    assert_ne!(original_bytes, tampered_bytes);
    assert!(verifying_key
        .verify_strict(&tampered_bytes, &signature)
        .is_err());
}

fn assert_signed_descriptor_metadata_rejects<F>(mutate: F)
where
    F: FnOnce(&mut DomainPackRegistryPackageRecord),
{
    let mut fixture = Fixture::new();
    let registry_id = fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .registry_id
        .clone();
    let audience = fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .audience
        .clone();
    let verifying_key = fixture.publisher_key.verifying_key();
    let original_record = fixture.snapshot.domain_pack_supply_chain_registry.packages[0].clone();
    let original_bytes =
        domain_pack_publisher_signing_bytes(&registry_id, &audience, &original_record)
            .expect("original publisher signing bytes");
    let signature = fixture.publisher_key.sign(&original_bytes);

    let record = &mut fixture.snapshot.domain_pack_supply_chain_registry.packages[0];
    mutate(record);
    let tampered_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, record)
        .expect("tampered publisher signing bytes");
    assert_ne!(original_bytes, tampered_bytes);
    assert!(verifying_key
        .verify_strict(&tampered_bytes, &signature)
        .is_err());
}

#[test]
fn exact_threshold_snapshot_yields_only_opaque_supply_chain_assurance() {
    let fixture = Fixture::new();
    let verified =
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW)
            .expect("exact supply-chain snapshot");

    assert_eq!(verified.registry_id().0, "registry.domain-pack.test");
    assert_eq!(verified.audience().0, "forge.domain-pack.project.test");
    assert_eq!(verified.generation(), 1);
    assert_eq!(verified.entries().len(), 1);
    assert_eq!(verified.grants().len(), 1);
    assert_eq!(verified.mirrors().len(), 2);
    assert_eq!(
        verified.entries()[0]
            .record()
            .artifacts
            .content
            .binding
            .canonical_sha256,
        digest('b')
    );
    assert_eq!(
        verified.entries()[0].record().record_digest,
        fixture.snapshot.domain_pack_supply_chain_registry.packages[0].record_digest
    );

    let audit = verified.audit();
    assert_eq!(
        audit.authority,
        DomainPackSupplyChainAuditAuthority::NonAuthoritative
    );
    assert_eq!(
        audit.source_assurance,
        DomainPackSourceAssurance::SupplyChainVerified
    );
    assert_eq!(audit.registry_signers.len(), 2);
    assert_eq!(audit.entries.len(), 1);
    let json = serde_json::to_string(&audit).expect("audit JSON");
    assert!(!json.contains("signature_hex"));
    assert!(!json.contains("publisher_signature"));
    assert!(json.contains("signature_fingerprint"));
}

#[test]
fn snapshot_digest_excludes_only_detached_registry_signatures_and_digest_field() {
    let mut fixture = Fixture::new();
    let expected = domain_pack_registry_snapshot_digest(&fixture.snapshot).unwrap();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest = digest('f');
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .signatures
        .clear();
    assert_eq!(
        domain_pack_registry_snapshot_digest(&fixture.snapshot).unwrap(),
        expected
    );

    fixture.snapshot.domain_pack_supply_chain_registry.packages[0].publisher_signature_hex =
        "11".repeat(64);
    assert_ne!(
        domain_pack_registry_snapshot_digest(&fixture.snapshot).unwrap(),
        expected,
        "publisher signature remains part of the registry snapshot subject"
    );
}

#[test]
fn schema_audience_and_snapshot_time_are_policy_bound() {
    let mut fixture = Fixture::new();
    fixture.policy.schema_version = "0.1".to_owned();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::UnsupportedSchemaVersion { .. })
    ));

    let mut fixture = Fixture::new();
    fixture.policy.domain_pack_trust_policy.audience = id("another.audience");
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::AudienceMismatch { .. })
    ));

    let fixture = Fixture::new();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, ISSUED - 1),
        Err(DomainPackSupplyChainError::SnapshotNotYetValid { .. })
    ));
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, EXPIRES),
        Err(DomainPackSupplyChainError::SnapshotExpired { .. })
    ));
}

#[test]
fn registry_threshold_requires_distinct_active_policy_selected_keys() {
    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .signatures
        .pop();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(
            DomainPackSupplyChainError::RegistrySignatureThresholdNotMet {
                required: 2,
                verified: 1
            }
        )
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .signatures
        .push(
            fixture
                .snapshot
                .domain_pack_supply_chain_registry
                .signatures[0]
                .clone(),
        );
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RegistrySignatureDuplicate { .. })
    ));

    let mut fixture = Fixture::new();
    fixture.policy.domain_pack_trust_policy.registry_keys[0].status =
        DomainPackCredentialStatus::Revoked;
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RegistryKeyNotActive { .. })
    ));
}

#[test]
fn caller_selected_or_tampered_registry_signature_cannot_pass() {
    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .signatures[0]
        .signer_key_id = id("attacker.caller.key");
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RegistryKeyNotFound { .. })
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .signatures[0]
        .signature_hex = "00".repeat(64);
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RegistrySignatureInvalid { .. })
    ));
}

#[test]
fn publisher_signature_binds_exact_record_and_registry_context() {
    let mut fixture = Fixture::new();
    fixture.snapshot.domain_pack_supply_chain_registry.packages[0].package_digest = digest('a');
    let record = &mut fixture.snapshot.domain_pack_supply_chain_registry.packages[0];
    record.record_digest = domain_pack_package_record_digest(record).unwrap();
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PublisherSignatureInvalid { .. })
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .registry_id = id("registry.replayed");
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PublisherSignatureInvalid { .. })
    ));
}

#[test]
fn publisher_credential_status_identity_and_validity_fail_closed() {
    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .publisher_credentials[0]
        .status = DomainPackCredentialStatus::Revoked;
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PublisherCredentialNotActive { .. })
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .publisher_credentials[0]
        .publisher = id("publisher.other");
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PublisherIdentityMismatch { .. })
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .publisher_credentials[0]
        .valid_until_unix = 300;
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PublisherCredentialOutsideValidity { .. })
    ));
}

#[test]
fn namespace_ownership_is_registry_granted_and_core_is_never_grantable() {
    let mut fixture = Fixture::new();
    fixture.snapshot.domain_pack_supply_chain_registry.packages[0]
        .identity
        .namespace = id("attacker.outside");
    fixture.seal_record(0);
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::NamespaceNotGranted { .. })
    ));

    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .namespace_grants[0]
        .namespace_prefix = id("forge.core");
    fixture.snapshot.domain_pack_supply_chain_registry.packages[0]
        .identity
        .namespace = id("forge.core.attacker");
    fixture.seal_record(0);
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::ReservedCoreNamespace { .. })
    ));
}

#[test]
fn overlapping_cross_publisher_namespace_grants_are_rejected() {
    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .namespace_grants
        .push(DomainPackNamespaceGrant {
            grant_id: id("grant.attacker"),
            publisher: id("publisher.attacker"),
            namespace_prefix: id("sample.foundation"),
            valid_from_unix: 100,
            valid_until_unix: 900,
        });
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::InvalidSnapshot { .. })
    ));
}

#[test]
fn revoked_history_is_verified_but_same_version_equivocation_is_rejected() {
    let mut fixture = Fixture::new();
    fixture.revoke_record(0, "fixture revocation");
    fixture.seal_snapshot();
    let verified = fixture.verify();
    assert_eq!(verified.entries().len(), 1);
    assert!(verified.is_currently_revoked(
        &fixture.snapshot.domain_pack_supply_chain_registry.packages[0].record_digest
    ));
    assert_eq!(verified.audit().current_revocations.len(), 1);

    let mut fixture = Fixture::new();
    let mut divergent = fixture.snapshot.domain_pack_supply_chain_registry.packages[0].clone();
    divergent.content_digest = digest('e');
    divergent.artifacts.content.binding.raw_sha256 = digest('e');
    divergent.artifacts.content.object_path = RepoPath(format!(
        "objects/sha256/{}",
        &divergent.artifacts.content.binding.raw_sha256[7..]
    ));
    divergent.record_digest = domain_pack_package_record_digest(&divergent).unwrap();
    let registry = &fixture.snapshot.domain_pack_supply_chain_registry;
    let bytes =
        domain_pack_publisher_signing_bytes(&registry.registry_id, &registry.audience, &divergent)
            .unwrap();
    divergent.publisher_signature_hex = hex(&fixture.publisher_key.sign(&bytes).to_bytes());
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .packages
        .push(divergent);
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::PackageEquivocation { .. })
    ));
}

#[test]
fn snapshot_or_record_digest_tampering_fails_before_authority_construction() {
    let mut fixture = Fixture::new();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest = digest('f');
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::SnapshotDigestMismatch { .. })
    ));

    let mut fixture = Fixture::new();
    fixture.snapshot.domain_pack_supply_chain_registry.packages[0].record_digest = digest('f');
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RecordDigestMismatch { .. })
    ));
}

#[test]
fn anchor_accepts_only_genesis_then_an_exact_direct_successor() {
    let genesis = Fixture::new();
    let genesis_digest = genesis
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let mut anchor = empty_anchor();
    let empty_version = anchor.version();
    let first = anchor
        .compare_and_advance(&empty_version, genesis.verify())
        .expect("accept genesis");
    let DomainPackRegistryAnchorAdvance::Advanced(first) = first else {
        panic!("genesis must mint anchored capability");
    };
    assert_eq!(first.verified_snapshot().generation(), 1);

    let mut successor = Fixture::new();
    successor.set_generation(2, Some(genesis_digest.clone()), "1.0.1");
    assert!(matches!(
        anchor.compare_and_advance(&empty_version, successor.verify()),
        Err(DomainPackSupplyChainError::RegistryAnchorCompareAndSwapConflict)
    ));

    let mut wrong_predecessor = Fixture::new();
    wrong_predecessor.set_generation(2, Some(digest('e')), "1.0.1");
    let current = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&current, wrong_predecessor.verify()),
        Err(DomainPackSupplyChainError::RegistrySnapshotPredecessorMismatch { .. })
    ));

    let current = anchor.version();
    let advanced = anchor
        .compare_and_advance(&current, successor.verify())
        .expect("accept exact direct successor");
    let DomainPackRegistryAnchorAdvance::Advanced(advanced) = advanced else {
        panic!("successor must mint anchored capability");
    };
    assert_eq!(advanced.verified_snapshot().generation(), 2);
    assert_eq!(
        advanced.verified_snapshot().previous_snapshot_digest(),
        Some(genesis_digest.as_str())
    );
}

#[test]
fn anchor_rejects_an_older_still_cryptographically_valid_snapshot() {
    let old = Fixture::new();
    let predecessor = old
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let mut anchor = empty_anchor();
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, old.verify())
        .expect("accept genesis");

    let mut current = Fixture::new();
    current.set_generation(2, Some(predecessor), "1.0.1");
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, current.verify())
        .expect("accept successor");

    let replayed_old = Fixture::new();
    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, replayed_old.verify()),
        Err(DomainPackSupplyChainError::RegistrySnapshotStale {
            anchored_generation: 2,
            candidate_generation: 1
        })
    ));
}

#[test]
fn anchor_rejects_a_same_generation_fork() {
    let genesis = Fixture::new();
    let predecessor = genesis
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let mut anchor = empty_anchor();
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, genesis.verify())
        .expect("accept genesis");

    let mut accepted = Fixture::new();
    accepted.set_generation(2, Some(predecessor.clone()), "1.0.1");
    let mut fork = Fixture::new();
    fork.set_generation(2, Some(predecessor), "1.0.2-fork");
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, accepted.verify())
        .expect("accept first successor");

    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, fork.verify()),
        Err(DomainPackSupplyChainError::RegistrySnapshotFork { generation: 2, .. })
    ));
}

#[test]
fn anchor_rejects_generation_skip_even_with_valid_signatures() {
    let genesis = Fixture::new();
    let predecessor = genesis
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let mut anchor = empty_anchor();
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, genesis.verify())
        .expect("accept genesis");

    let mut skipped = Fixture::new();
    skipped.set_generation(3, Some(predecessor), "1.0.2");
    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, skipped.verify()),
        Err(DomainPackSupplyChainError::RegistrySnapshotGenerationSkip {
            anchored_generation: 1,
            candidate_generation: 3
        })
    ));
}

#[test]
fn exact_head_replay_is_idempotent_and_revalidates_authority() {
    let genesis = Fixture::new();
    let mut anchor = empty_anchor();
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, genesis.verify())
        .expect("accept genesis");
    let current = anchor.version();

    let exact_replay = Fixture::new();
    let outcome = anchor
        .compare_and_advance(&current, exact_replay.verify())
        .expect("exact replay is idempotent");
    let DomainPackRegistryAnchorAdvance::Replay { capability, audit } = outcome else {
        panic!("exact current replay must revalidate authority");
    };
    assert_eq!(audit.generation, 1);
    assert_eq!(capability.verified_snapshot().generation(), 1);
    assert_eq!(
        capability.verified_snapshot().snapshot_digest(),
        audit.snapshot_digest
    );
    assert_eq!(anchor.version(), current);
}

#[test]
fn anchor_rejects_replay_and_successor_verified_under_a_swapped_trust_policy() {
    let genesis = Fixture::new();
    let genesis_digest = genesis
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let genesis_verified = genesis.verify();
    let anchored_policy_digest = genesis_verified.trust_policy_digest().to_owned();
    let mut anchor = empty_anchor();
    let expected = anchor.version();
    anchor
        .compare_and_advance(&expected, genesis_verified)
        .expect("accept genesis and pin its exact policy");
    assert_eq!(
        anchor.version().trust_policy_digest(),
        Some(anchored_policy_digest.as_str())
    );

    let mut swapped_replay = Fixture::new();
    swapped_replay
        .policy
        .domain_pack_trust_policy
        .policy_version = "attacker-policy".to_owned();
    let swapped_replay = swapped_replay.verify();
    let swapped_policy_digest = swapped_replay.trust_policy_digest().to_owned();
    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, swapped_replay),
        Err(DomainPackSupplyChainError::RegistryAnchorTrustPolicyMismatch {
            anchored_trust_policy_digest,
            candidate_trust_policy_digest,
        }) if anchored_trust_policy_digest == anchored_policy_digest
            && candidate_trust_policy_digest == swapped_policy_digest
    ));

    let mut swapped_successor = Fixture::new();
    swapped_successor
        .policy
        .domain_pack_trust_policy
        .policy_version = "attacker-policy".to_owned();
    swapped_successor.set_generation(2, Some(genesis_digest.clone()), "1.0.1");
    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, swapped_successor.verify()),
        Err(DomainPackSupplyChainError::RegistryAnchorTrustPolicyMismatch { .. })
    ));

    let mut legitimate_successor = Fixture::new();
    legitimate_successor.set_generation(2, Some(genesis_digest), "1.0.1");
    let expected = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&expected, legitimate_successor.verify()),
        Ok(DomainPackRegistryAnchorAdvance::Advanced(_))
    ));
}

#[test]
fn every_signed_mirror_and_descriptor_field_invalidates_its_signature() {
    assert_signed_mirror_metadata_rejects(|registry| {
        registry.mirrors[0].mirror_id = id("mirror.renamed");
    });
    assert_signed_mirror_metadata_rejects(|registry| {
        registry.mirrors.swap(0, 1);
    });
    assert_signed_mirror_metadata_rejects(|registry| {
        registry.mirrors[0].priority = 1;
    });
    assert_signed_mirror_metadata_rejects(|registry| {
        registry.mirrors[0].transport = DomainPackRegistryMirrorTransport::Https {
            base_url: "https://mirror.example.invalid/domain-packs".to_owned(),
        };
    });
    assert_signed_mirror_metadata_rejects(|registry| {
        registry.mirrors[1].transport =
            DomainPackRegistryMirrorTransport::OperatorProvisionedLocal {
                location_id: id("operator.registry.alternate-cache"),
            };
    });

    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.kind = DomainPackRemoteArtifactKind::Fixture;
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.binding.artifact_ref =
            RepoPath("packs/foundation/content-renamed.yaml".to_owned());
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.object_path = RepoPath("objects/sha256/alternate".to_owned());
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.binding.raw_sha256 = digest('e');
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.binding.canonical_sha256 = digest('e');
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.byte_length += 1;
    });
    assert_signed_descriptor_metadata_rejects(|record| {
        record.artifacts.content.media_type = DomainPackRemoteArtifactMediaType::TextPlain;
    });
}

#[test]
fn signed_mirror_and_descriptor_metadata_tampering_fails_closed() {
    let mut mirror_tamper = Fixture::new();
    mirror_tamper
        .snapshot
        .domain_pack_supply_chain_registry
        .mirrors[0]
        .priority = 9;
    mirror_tamper.refresh_snapshot_digest_without_resigning();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(
            &mirror_tamper.policy,
            &mirror_tamper.snapshot,
            NOW
        ),
        Err(DomainPackSupplyChainError::RegistrySignatureInvalid { .. })
    ));

    let mut location_tamper = Fixture::new();
    location_tamper
        .snapshot
        .domain_pack_supply_chain_registry
        .mirrors[1]
        .transport = DomainPackRegistryMirrorTransport::OperatorProvisionedLocal {
        location_id: id("operator.registry.changed"),
    };
    location_tamper.refresh_snapshot_digest_without_resigning();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(
            &location_tamper.policy,
            &location_tamper.snapshot,
            NOW
        ),
        Err(DomainPackSupplyChainError::RegistrySignatureInvalid { .. })
    ));

    let mut descriptor_tamper = Fixture::new();
    let record = &mut descriptor_tamper
        .snapshot
        .domain_pack_supply_chain_registry
        .packages[0];
    record.artifacts.content.binding.canonical_sha256 = digest('f');
    record.artifacts.content.byte_length = 97;
    record.artifacts.content.media_type = DomainPackRemoteArtifactMediaType::ApplicationJson;
    record.record_digest = domain_pack_package_record_digest(record).expect("record digest");
    descriptor_tamper.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(
            &descriptor_tamper.policy,
            &descriptor_tamper.snapshot,
            NOW
        ),
        Err(DomainPackSupplyChainError::PublisherSignatureInvalid { .. })
    ));

    let mut raw_path_tamper = Fixture::new();
    let record = &mut raw_path_tamper
        .snapshot
        .domain_pack_supply_chain_registry
        .packages[0];
    record.artifacts.license.binding.raw_sha256 = digest('e');
    record.artifacts.license.object_path = RepoPath(format!(
        "objects/sha256/{}",
        &record.artifacts.license.binding.raw_sha256[7..]
    ));
    record.license_digest = record.artifacts.license.binding.raw_sha256.clone();
    record.record_digest = domain_pack_package_record_digest(record).expect("record digest");
    raw_path_tamper.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(
            &raw_path_tamper.policy,
            &raw_path_tamper.snapshot,
            NOW
        ),
        Err(DomainPackSupplyChainError::PublisherSignatureInvalid { .. })
    ));
}

#[test]
fn historical_revocations_remain_visible_but_only_block_revoked_selection() {
    let mut fixture = Fixture::new();
    fixture.add_unrelated_record();
    fixture.revoke_record(0, "historical tamper evidence");
    fixture
        .policy
        .domain_pack_trust_policy
        .rules
        .push(DomainPackTrustRule {
            rule_id: id("rule.allow.unrelated"),
            pack: DomainPackCoordinate {
                publisher: id("publisher.fixture"),
                name: id("unrelated"),
            },
            package_digest: None,
            content_digest: None,
            disposition: DomainPackTrustDisposition::InspectOnly,
        });
    fixture.seal_snapshot();
    let verified = fixture.verify();
    assert_eq!(verified.entries().len(), 2);
    assert_eq!(verified.current_revocations().len(), 1);

    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, verified)
        .expect("revoked history does not invalidate the whole snapshot")
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let revoked = &fixture.snapshot.domain_pack_supply_chain_registry.packages[0];
    assert!(matches!(
        select_domain_pack_supply_chain_record(&anchored, revoked, &fixture.policy, NOW),
        Err(DomainPackSupplyChainError::RevokedRecord { .. })
    ));
    let unrelated = &fixture.snapshot.domain_pack_supply_chain_registry.packages[1];
    let selected =
        select_domain_pack_supply_chain_record(&anchored, unrelated, &fixture.policy, NOW)
            .expect("unrelated current record remains selectable");
    assert_eq!(selected.record_digest(), unrelated.record_digest);
    assert_eq!(anchored.audit().cumulative_revocations.len(), 1);
}

#[test]
fn revocation_facts_are_cumulative_across_advancement_replay_and_restoration() {
    let genesis = Fixture::new();
    let genesis_digest = genesis
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let mut anchor = empty_anchor();
    let version = anchor.version();
    anchor
        .compare_and_advance(&version, genesis.verify())
        .expect("accept genesis");

    let mut successor = Fixture::new();
    successor.set_generation(2, Some(genesis_digest.clone()), "1.0.1");
    successor.revoke_record(0, "new cumulative fact");
    successor.seal_snapshot();
    let successor_digest = successor
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest
        .clone();
    let version = anchor.version();
    anchor
        .compare_and_advance(&version, successor.verify())
        .expect("successor may add a revocation");
    assert_eq!(
        anchor.version().cumulative_revocation_digest(),
        domain_pack_cumulative_revocation_digest(
            &successor
                .snapshot
                .domain_pack_supply_chain_registry
                .revocations
        )
        .expect("cumulative digest")
    );

    let replay = {
        let mut fixture = Fixture::new();
        fixture.set_generation(2, Some(genesis_digest.clone()), "1.0.1");
        fixture.revoke_record(0, "new cumulative fact");
        fixture.seal_snapshot();
        fixture
    };
    let version = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&version, replay.verify()),
        Ok(DomainPackRegistryAnchorAdvance::Replay { .. })
    ));

    let mut removal = Fixture::new();
    removal.set_generation(3, Some(successor_digest.clone()), "1.0.2");
    let version = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&version, removal.verify()),
        Err(DomainPackSupplyChainError::RegistryAnchorCumulativeRevocationMismatch { .. })
    ));

    let mut mutation = Fixture::new();
    mutation.set_generation(3, Some(successor_digest), "1.0.2");
    mutation.revoke_record(0, "mutated explanation");
    mutation.seal_snapshot();
    let version = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&version, mutation.verify()),
        Err(DomainPackSupplyChainError::RegistryAnchorCumulativeRevocationMismatch { .. })
    ));

    let restored = DomainPackRegistryAnchor::from_operator_protected_head(
        id("registry.domain-pack.test"),
        id("forge.domain-pack.project.test"),
        2,
        replay
            .snapshot
            .domain_pack_supply_chain_registry
            .snapshot_digest
            .clone(),
        replay.verify().trust_policy_digest().to_owned(),
        replay
            .snapshot
            .domain_pack_supply_chain_registry
            .revocations
            .clone(),
        domain_pack_cumulative_revocation_digest(
            &replay
                .snapshot
                .domain_pack_supply_chain_registry
                .revocations,
        )
        .expect("canonical protected-head digest"),
    )
    .expect("operator head binds cumulative facts");
    assert_eq!(
        restored.version().cumulative_revocation_digest(),
        anchor.version().cumulative_revocation_digest()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn selected_record_uses_exact_policy_rules_defaults_and_ambiguous_rejection() {
    let mut pinned = Fixture::new();
    let record = pinned.snapshot.domain_pack_supply_chain_registry.packages[0].clone();
    pinned.policy.domain_pack_trust_policy.rules[0].package_digest =
        Some(record.package_digest.clone());
    pinned.policy.domain_pack_trust_policy.rules[0].content_digest =
        Some(record.artifacts.content.binding.canonical_sha256.clone());
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, pinned.verify())
        .expect("accept digest-pinned policy")
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let selected = select_domain_pack_supply_chain_record(&anchored, &record, &pinned.policy, NOW)
        .expect("matching allow rule selects record");
    assert_eq!(
        selected.disposition(),
        DomainPackTrustDisposition::InspectOnly
    );
    assert_eq!(selected.trust_rule_id().0, "rule.allow.fixture");

    let mut reject = Fixture::new();
    reject.policy.domain_pack_trust_policy.rules[0].disposition =
        DomainPackTrustDisposition::Reject;
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, reject.verify())
        .unwrap()
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let record = &reject.snapshot.domain_pack_supply_chain_registry.packages[0];
    assert!(matches!(
        select_domain_pack_supply_chain_record(&anchored, record, &reject.policy, NOW),
        Err(DomainPackSupplyChainError::SelectedRecordRejected { .. })
    ));

    let mut default = Fixture::new();
    default.policy.domain_pack_trust_policy.rules.clear();
    default.policy.domain_pack_trust_policy.default_disposition =
        DomainPackTrustDisposition::InspectOnly;
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, default.verify())
        .unwrap()
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let record = &default.snapshot.domain_pack_supply_chain_registry.packages[0];
    let selected = select_domain_pack_supply_chain_record(&anchored, record, &default.policy, NOW)
        .expect("permissive explicit default selects record");
    assert_eq!(
        selected.trust_rule_id().0,
        "domain-pack.trust.default-disposition"
    );

    let mut ambiguous = Fixture::new();
    let mut conflicting = ambiguous.policy.domain_pack_trust_policy.rules[0].clone();
    conflicting.rule_id = id("rule.conflicting");
    conflicting.disposition = DomainPackTrustDisposition::Reject;
    ambiguous
        .policy
        .domain_pack_trust_policy
        .rules
        .push(conflicting);
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, ambiguous.verify())
        .unwrap()
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let record = &ambiguous
        .snapshot
        .domain_pack_supply_chain_registry
        .packages[0];
    assert!(matches!(
        select_domain_pack_supply_chain_record(&anchored, record, &ambiguous.policy, NOW),
        Err(DomainPackSupplyChainError::SelectedRecordAmbiguousTrustRule { .. })
    ));

    let mut reordered = Fixture::new();
    let mut conflicting = reordered.policy.domain_pack_trust_policy.rules[0].clone();
    conflicting.rule_id = id("rule.conflicting");
    conflicting.disposition = DomainPackTrustDisposition::Reject;
    reordered
        .policy
        .domain_pack_trust_policy
        .rules
        .insert(0, conflicting);
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, reordered.verify())
        .unwrap()
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let record = &reordered
        .snapshot
        .domain_pack_supply_chain_registry
        .packages[0];
    assert!(matches!(
        select_domain_pack_supply_chain_record(&anchored, record, &reordered.policy, NOW),
        Err(DomainPackSupplyChainError::SelectedRecordAmbiguousTrustRule { .. })
    ));
}

#[test]
fn audit_evidence_is_non_authoritative_and_contains_no_artifact_bytes() {
    let fixture = Fixture::new();
    let mut anchor = empty_anchor();
    let version = anchor.version();
    let anchored = match anchor
        .compare_and_advance(&version, fixture.verify())
        .unwrap()
    {
        DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
        DomainPackRegistryAnchorAdvance::Replay { .. } => panic!("genesis cannot replay"),
    };
    let audit = anchored.audit();
    let json = serde_json::to_string(&audit).expect("audit JSON");
    assert!(json.contains("non_authoritative"));
    assert!(!json.contains("publisher_signature_hex"));
    assert!(!json.contains("https://"));
}
