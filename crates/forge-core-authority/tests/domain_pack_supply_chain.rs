use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_package_record_digest, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
    verify_domain_pack_supply_chain_snapshot, DomainPackRegistryAnchor,
    DomainPackRegistryAnchorAdvance, DomainPackSupplyChainAuditAuthority,
    DomainPackSupplyChainError,
};
use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCoordinate, DomainPackCredentialStatus,
    DomainPackIdentity, DomainPackNamespaceGrant, DomainPackPackageRevocation,
    DomainPackPublisherCredential, DomainPackRegistryPackageRecord, DomainPackRegistrySignature,
    DomainPackRegistryTrustKey, DomainPackRegistryTrustRole, DomainPackRevocationReason,
    DomainPackSourceAssurance, DomainPackSupplyChainRegistry,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustDisposition, DomainPackTrustPolicy,
    DomainPackTrustPolicyDocument, DomainPackTrustRule, StableId,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
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
        let record = DomainPackRegistryPackageRecord {
            identity: DomainPackIdentity {
                publisher: id("publisher.fixture"),
                name: id("foundation"),
                namespace: id("sample.foundation"),
                version: "1.0.0".to_owned(),
            },
            package_digest: digest('1'),
            manifest_digest: digest('2'),
            content_digest: digest('3'),
            license_digest: digest('4'),
            fixture_digests: vec![digest('5')],
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
    fixture.snapshot.domain_pack_supply_chain_registry.packages[0].license_digest = digest('a');
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
fn revocation_and_same_version_equivocation_are_hard_failures() {
    let mut fixture = Fixture::new();
    let record_digest = fixture.snapshot.domain_pack_supply_chain_registry.packages[0]
        .record_digest
        .clone();
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .revocations
        .push(DomainPackPackageRevocation {
            record_digest,
            reason: DomainPackRevocationReason::PackageTamper,
            explanation: "fixture revocation".to_owned(),
            revoked_at_unix: 300,
        });
    fixture.seal_snapshot();
    assert!(matches!(
        verify_domain_pack_supply_chain_snapshot(&fixture.policy, &fixture.snapshot, NOW),
        Err(DomainPackSupplyChainError::RevokedRecord { .. })
    ));

    let mut fixture = Fixture::new();
    let mut divergent = fixture.snapshot.domain_pack_supply_chain_registry.packages[0].clone();
    divergent.content_digest = digest('a');
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
