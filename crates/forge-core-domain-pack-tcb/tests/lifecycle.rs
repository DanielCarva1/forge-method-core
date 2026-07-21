use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_package_record_digest, domain_pack_promotion_reviewer_key_fingerprint,
    domain_pack_publisher_signing_bytes, domain_pack_registry_signing_bytes,
    domain_pack_registry_snapshot_digest, domain_pack_reviewed_registry_digest,
    domain_pack_reviewed_registry_entry_digest, domain_pack_reviewed_registry_signing_bytes,
    domain_pack_reviewer_registry_digest, verify_domain_pack_supply_chain_snapshot,
    AnchoredDomainPackSupplyChainSnapshot, AnchoredReviewedDomainPackRegistrySnapshot,
    DomainPackRegistryAnchor, DomainPackRegistryAnchorAdvance, DomainPackReviewerRegistryAnchor,
    ReviewedDomainPackRegistryAnchor,
};
use forge_core_contracts::*;
use forge_core_decisions::{
    compose_domain_packs, domain_pack_resolution_projection_digest,
    evaluate_domain_pack_compatibility, resolve_domain_packs, DomainPackCandidateMaterial,
    DomainPackCapabilityDemand, DomainPackCompatibilityInput, DomainPackTrustEvaluationInput,
    DomainPackTrustSelectedPackage,
};
use forge_core_domain_pack_tcb::{
    authorize_prepared_domain_pack_lifecycle, lock_domain_pack_lifecycle,
    verify_domain_pack_project_snapshot, DomainPackImmutableArtifact,
    DomainPackLifecycleAuthorizationContext, DomainPackLifecycleStoreError,
    DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn pid(value: &str) -> PrincipalId {
    PrincipalId(value.to_owned())
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
        output
    })
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn remote_descriptor(
    kind: DomainPackRemoteArtifactKind,
    binding: DomainPackArtifactBinding,
    raw_bytes: &[u8],
) -> DomainPackRemoteArtifactDescriptor {
    let token = binding
        .raw_sha256
        .strip_prefix("sha256:")
        .expect("fixture binding has sha256 prefix");
    DomainPackRemoteArtifactDescriptor {
        kind,
        object_path: RepoPath(format!("objects/sha256/{token}")),
        binding,
        byte_length: u64::try_from(raw_bytes.len()).expect("fixture byte length"),
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

fn learning_digest(label: &str) -> String {
    format!("{:x}", Sha256::digest(label.as_bytes()))
}

fn learning_full_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical learning JSON");
    format!("{:x}", Sha256::digest(bytes))
}

fn temp_state_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-tcb-{label}-{}-{nonce}",
        std::process::id()
    ));
    let state = root.join(".forge-method");
    fs::create_dir_all(&state).expect("create state root");
    fs::write(root.join("project.txt"), b"stable project input\n").expect("project input");
    state
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Clone)]
struct Fixture {
    policy: DomainPackTrustPolicyDocument,
    snapshot: DomainPackSupplyChainRegistryDocument,
    resolved: DomainPackResolvedPackage,
    reviewer_registry: DomainPackReviewerRegistryDocument,
    reviewed_registry: DomainPackReviewedRegistryDocument,
}

impl Fixture {
    #[allow(clippy::too_many_lines, clippy::similar_names)] // One cohesive signed fixture keeps every binding identical.
    fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs();
        let request_path =
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml");
        let composition_request: DomainPackCompositionRequestDocument = yaml_serde::from_str(
            &fs::read_to_string(request_path).expect("composition request fixture"),
        )
        .expect("typed composition request");
        let authored = &composition_request
            .domain_pack_composition_request
            .candidates[0];
        let manifest = &authored.manifest.domain_pack_manifest;
        let package_digest = digest('1');
        let namespace_grant_id = id("grant.forge.fixture");
        let credential_id = id("credential.forge.fixture");
        let registry_id = id("registry.domain-pack.test");
        let audience = id("project.domain-pack.test");
        let registry_keys = [
            (id("registry.key.a"), SigningKey::from_bytes(&[1_u8; 32])),
            (id("registry.key.b"), SigningKey::from_bytes(&[2_u8; 32])),
        ];
        let publisher_key = SigningKey::from_bytes(&[3_u8; 32]);
        let policy = DomainPackTrustPolicyDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_trust_policy: DomainPackTrustPolicy {
                policy_id: id("policy.domain-pack.test"),
                policy_version: "1.0.0".to_owned(),
                audience: audience.clone(),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                registry_keys: registry_keys
                    .iter()
                    .map(|(key_id, key)| DomainPackRegistryTrustKey {
                        key_id: key_id.clone(),
                        role: DomainPackRegistryTrustRole::RegistrySigner,
                        public_key_hex: hex(&key.verifying_key().to_bytes()),
                        status: DomainPackCredentialStatus::Active,
                        valid_from_unix: now.saturating_sub(3_600),
                        valid_until_unix: now + 3_600,
                    })
                    .collect(),
                required_registry_signature_threshold: 2,
                minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
                rules: vec![DomainPackTrustRule {
                    rule_id: id("rule.foundation.exact"),
                    pack: DomainPackCoordinate {
                        publisher: manifest.identity.publisher.clone(),
                        name: manifest.identity.name.clone(),
                    },
                    package_digest: Some(package_digest.clone()),
                    content_digest: Some(manifest.content.canonical_sha256.clone()),
                    disposition: DomainPackTrustDisposition::ActivateDeclarativeKnowledge,
                }],
                default_disposition: DomainPackTrustDisposition::Reject,
            },
        };
        let manifest_raw =
            fs::read(repo_root().join("docs/fixtures/domain-pack-v0/manifests/foundation.yaml"))
                .expect("raw manifest");
        let content_raw =
            fs::read(repo_root().join("docs/fixtures/domain-pack-v0/content/foundation.yaml"))
                .expect("raw content");
        let license_raw = fs::read(
            repo_root().join("docs/fixtures/domain-pack-v0/artifacts/license-notice.yaml"),
        )
        .expect("raw license");
        let representative_raw = fs::read(
            repo_root()
                .join("docs/fixtures/domain-pack-v0/artifacts/foundation-representative.yaml"),
        )
        .expect("raw representative fixture");
        let content_binding = DomainPackArtifactBinding {
            artifact_ref: manifest.content.content_ref.clone(),
            raw_sha256: manifest.content.raw_sha256.clone(),
            canonical_sha256: manifest.content.canonical_sha256.clone(),
        };
        let artifacts = DomainPackRegistryArtifactSet {
            manifest: remote_descriptor(
                DomainPackRemoteArtifactKind::Manifest,
                authored.manifest_binding.clone(),
                &manifest_raw,
            ),
            content: remote_descriptor(
                DomainPackRemoteArtifactKind::Content,
                content_binding.clone(),
                &content_raw,
            ),
            license: remote_descriptor(
                DomainPackRemoteArtifactKind::License,
                manifest.provenance.license_text.clone(),
                &license_raw,
            ),
            fixtures: authored
                .content
                .domain_pack_content
                .fixtures
                .iter()
                .zip([representative_raw.as_slice()])
                .map(|(fixture, raw)| {
                    remote_descriptor(
                        DomainPackRemoteArtifactKind::Fixture,
                        fixture.artifact.clone(),
                        raw,
                    )
                })
                .collect(),
        };
        let mut record = DomainPackRegistryPackageRecord {
            identity: manifest.identity.clone(),
            package_digest: package_digest.clone(),
            manifest_digest: artifacts.manifest.binding.raw_sha256.clone(),
            content_digest: artifacts.content.binding.raw_sha256.clone(),
            license_digest: artifacts.license.binding.raw_sha256.clone(),
            fixture_digests: artifacts
                .fixtures
                .iter()
                .map(|fixture| fixture.binding.raw_sha256.clone())
                .collect(),
            artifacts,
            namespace_grant_id: namespace_grant_id.clone(),
            publisher_credential_id: credential_id.clone(),
            publisher_signature_hex: "00".repeat(64),
            record_digest: digest('0'),
        };
        record.record_digest =
            domain_pack_package_record_digest(&record).expect("package record digest");
        let publisher_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, &record)
            .expect("publisher signing bytes");
        record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());

        let mut snapshot = DomainPackSupplyChainRegistryDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
                registry_id,
                registry_version: "1.0.0".to_owned(),
                audience,
                authority: DomainPackCandidateAuthority::CandidateOnly,
                generation: 1,
                previous_snapshot_digest: None,
                issued_at_unix: now.saturating_sub(60),
                expires_at_unix: now + 3_600,
                publisher_credentials: vec![DomainPackPublisherCredential {
                    credential_id,
                    publisher: manifest.identity.publisher.clone(),
                    public_key_hex: hex(&publisher_key.verifying_key().to_bytes()),
                    status: DomainPackCredentialStatus::Active,
                    valid_from_unix: now.saturating_sub(3_600),
                    valid_until_unix: now + 3_600,
                }],
                namespace_grants: vec![DomainPackNamespaceGrant {
                    grant_id: namespace_grant_id.clone(),
                    publisher: manifest.identity.publisher.clone(),
                    namespace_prefix: id("sample"),
                    valid_from_unix: now.saturating_sub(3_600),
                    valid_until_unix: now + 3_600,
                }],
                mirrors: vec![DomainPackRegistryMirror {
                    mirror_id: id("mirror.operator.fixture"),
                    priority: 0,
                    transport: DomainPackRegistryMirrorTransport::OperatorProvisionedLocal {
                        location_id: id("location.operator.fixture"),
                    },
                }],
                packages: vec![record.clone()],
                revocations: Vec::new(),
                snapshot_digest: digest('0'),
                signatures: Vec::new(),
            },
        };
        snapshot.domain_pack_supply_chain_registry.snapshot_digest =
            domain_pack_registry_snapshot_digest(&snapshot).expect("snapshot digest");
        for (key_id, key) in &registry_keys {
            let bytes = domain_pack_registry_signing_bytes(
                &snapshot,
                key_id,
                DomainPackRegistryTrustRole::RegistrySigner,
            )
            .expect("registry signing bytes");
            snapshot.domain_pack_supply_chain_registry.signatures.push(
                DomainPackRegistrySignature {
                    signer_key_id: key_id.clone(),
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                    signature_hex: hex(&key.sign(&bytes).to_bytes()),
                },
            );
        }

        let package = DomainPackPackageBinding {
            package_ref: RepoPath("domain-packs/forge.fixture.foundation.pack".to_owned()),
            package_digest,
            manifest: authored.manifest_binding.clone(),
            content: manifest.content.clone(),
            license: manifest.provenance.license_text.clone(),
            fixtures: authored
                .content
                .domain_pack_content
                .fixtures
                .iter()
                .map(|fixture| fixture.artifact.clone())
                .collect(),
        };
        let semantic_key = SigningKey::from_bytes(&[4_u8; 32]);
        let authorizer_key = SigningKey::from_bytes(&[5_u8; 32]);
        let reviewer_trust_policy_digest = learning_digest("operator-reviewer-trust");
        let reviewer_entries = [
            (
                "principal.semantic",
                "credential.semantic",
                DomainPackReviewerRole::DomainExpert,
                "domain.semantic",
                &semantic_key,
            ),
            (
                "principal.authorizer",
                "credential.authorizer",
                DomainPackReviewerRole::RegistryAuthorizer,
                "domain.registry",
                &authorizer_key,
            ),
        ];
        let mut reviewer_registry = DomainPackReviewerRegistryDocument {
            schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
            domain_pack_reviewer_registry: DomainPackReviewerRegistry {
                registry_id: id("reviewers.domain-pack.test"),
                audience: "forge-domain-pack-runtime".to_owned(),
                generation: 0,
                previous_registry_digest: None,
                trust_policy_digest: reviewer_trust_policy_digest,
                signature_threshold: 2,
                reviewers: reviewer_entries
                    .iter()
                    .map(|(principal, credential, role, domain, key)| {
                        DomainPackReviewerRegistryEntry {
                            reviewer_id: pid(principal),
                            credential_id: id(credential),
                            public_key_hex: hex(&key.verifying_key().to_bytes()),
                            public_key_fingerprint: domain_pack_promotion_reviewer_key_fingerprint(
                                &key.verifying_key().to_bytes(),
                            ),
                            algorithm: DomainPackPromotionSignatureAlgorithm::Ed25519,
                            roles: vec![*role],
                            independence_domains: vec![id(domain)],
                            status: DomainPackReviewerStatus::Active,
                            valid_from_unix: now.saturating_sub(3_600),
                            valid_until_unix: now + 3_600,
                        }
                    })
                    .collect(),
                rotation_signatures: reviewer_entries
                    .iter()
                    .map(
                        |(principal, credential, _, _, _)| DomainPackReviewerRegistrySignature {
                            signer_id: pid(principal),
                            credential_id: id(credential),
                            predecessor_registry_digest: None,
                            payload_digest: learning_digest("operator-genesis"),
                            algorithm: DomainPackPromotionSignatureAlgorithm::Ed25519,
                            signature: "00".repeat(64),
                            signed_at_unix: now,
                        },
                    )
                    .collect(),
                registry_digest: learning_digest("pending-reviewers"),
            },
        };
        reviewer_registry
            .domain_pack_reviewer_registry
            .registry_digest = domain_pack_reviewer_registry_digest(&reviewer_registry)
            .expect("reviewer registry digest");

        let mut reviewed_entry = DomainPackReviewedRegistryEntry {
            pack: DomainPackVersionReference {
                publisher: manifest.identity.publisher.clone(),
                name: manifest.identity.name.clone(),
                version: manifest.identity.version.clone(),
            },
            package_digest: package.package_digest.clone(),
            supply_chain_record_digest: record.record_digest.clone(),
            manifest_digest: package.manifest.canonical_sha256.clone(),
            content_digest: package.content.canonical_sha256.clone(),
            license_digest: package.license.canonical_sha256.clone(),
            fixture_digests: package
                .fixtures
                .iter()
                .map(|fixture| fixture.canonical_sha256.clone())
                .collect(),
            stage: DomainPackPromotionStage::Reviewed,
            eligibility: DomainPackReviewedEligibility::EligibleReviewed,
            promotion_decision_digest: learning_digest("promotion-decision"),
            authorization_digest: learning_digest("promotion-authorization"),
            independent_review_digests: vec![
                learning_digest("review-semantic"),
                learning_digest("review-authorizer"),
            ],
            compatibility: DomainPackReviewedCompatibility {
                forge_core_requirement: ">=0.7.0".to_owned(),
                pack_schema_requirement: "^0.1".to_owned(),
                evaluator_protocol_versions: vec!["1".to_owned()],
                predecessor_content_digests: Vec::new(),
                breaking_change: false,
                migration_evidence_refs: Vec::new(),
            },
            deprecation: None,
            revocation: None,
            supersession: None,
            entry_digest: learning_digest("pending-entry"),
        };
        reviewed_entry.entry_digest = domain_pack_reviewed_registry_entry_digest(&reviewed_entry)
            .expect("reviewed entry digest");
        let signature_stubs = reviewer_entries
            .iter()
            .map(
                |(principal, credential, role, _, _)| DomainPackReviewedRegistrySignature {
                    reviewer_id: pid(principal),
                    credential_id: id(credential),
                    role: *role,
                    algorithm: DomainPackPromotionSignatureAlgorithm::Ed25519,
                    payload_digest: learning_digest("pending-reviewed-registry"),
                    signature: "00".repeat(64),
                    signed_at_unix: now,
                },
            )
            .collect();
        let mut reviewed_registry = DomainPackReviewedRegistryDocument {
            schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
            domain_pack_reviewed_registry: DomainPackReviewedRegistry {
                registry_id: id("reviewed.domain-pack.test"),
                audience: "forge-domain-pack-runtime".to_owned(),
                generation: 0,
                previous_registry_digest: None,
                entries: vec![reviewed_entry],
                snapshot_signatures: signature_stubs,
                registry_digest: learning_digest("pending-reviewed-registry"),
            },
        };
        reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest = domain_pack_reviewed_registry_digest(&reviewed_registry)
            .expect("reviewed registry digest");
        let reviewed_digest = reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest
            .clone();
        for (index, key) in [&semantic_key, &authorizer_key].into_iter().enumerate() {
            reviewed_registry
                .domain_pack_reviewed_registry
                .snapshot_signatures[index]
                .payload_digest
                .clone_from(&reviewed_digest);
            let signature = reviewed_registry
                .domain_pack_reviewed_registry
                .snapshot_signatures[index]
                .clone();
            let bytes = domain_pack_reviewed_registry_signing_bytes(&reviewed_registry, &signature)
                .expect("reviewed registry signing bytes");
            reviewed_registry
                .domain_pack_reviewed_registry
                .snapshot_signatures[index]
                .signature = hex(&key.sign(&bytes).to_bytes());
        }
        let resolved = DomainPackResolvedPackage {
            identity: manifest.identity.clone(),
            package,
            registry_record_digest: record.record_digest,
            namespace_grant_id,
            source_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            semantic_assurance: DomainPackSemanticAssurance::Reviewed,
            reviewed_entry_digest: Some(
                reviewed_registry.domain_pack_reviewed_registry.entries[0]
                    .entry_digest
                    .clone(),
            ),
            promotion_authorization_digest: Some(
                reviewed_registry.domain_pack_reviewed_registry.entries[0]
                    .authorization_digest
                    .clone(),
            ),
            dependencies: manifest.dependencies.clone(),
            deterministic_order: 0,
        };
        Self {
            policy,
            snapshot,
            resolved,
            reviewer_registry,
            reviewed_registry,
        }
    }

    fn anchored(
        &self,
        verified: forge_core_authority::VerifiedDomainPackSupplyChainSnapshot,
    ) -> AnchoredDomainPackSupplyChainSnapshot {
        let registry = &self.snapshot.domain_pack_supply_chain_registry;
        let mut anchor = DomainPackRegistryAnchor::new_trust_on_first_use(
            registry.registry_id.clone(),
            registry.audience.clone(),
        )
        .expect("trust-on-first-use registry anchor");
        let version = anchor.version();
        match anchor
            .compare_and_advance(&version, verified)
            .expect("advance registry anchor")
        {
            DomainPackRegistryAnchorAdvance::Advanced(capability) => capability,
            DomainPackRegistryAnchorAdvance::Replay { .. } => {
                panic!("fresh trust-on-first-use anchor cannot replay")
            }
        }
    }

    #[allow(clippy::similar_names)] // Reviewer authority and reviewed content are independent axes.
    fn reviewed_anchored(&self, now: u64) -> AnchoredReviewedDomainPackRegistrySnapshot {
        let reviewer_trust = &self
            .reviewer_registry
            .domain_pack_reviewer_registry
            .trust_policy_digest;
        let reviewer_full = learning_full_digest(&self.reviewer_registry);
        let reviewer_anchor = DomainPackReviewerRegistryAnchor::from_operator_protected_genesis(
            self.reviewer_registry.clone(),
            reviewer_trust,
            &reviewer_full,
        )
        .expect("operator-protected reviewer registry");
        let reviewed_digest = self
            .reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest
            .clone();
        let mut reviewed_anchor = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
            &reviewer_anchor,
            self.reviewed_registry.clone(),
            &reviewed_digest,
            now,
        )
        .expect("operator-protected reviewed registry");
        reviewed_anchor
            .verify_exact_replay(&reviewer_anchor, self.reviewed_registry.clone(), now)
            .expect("fresh reviewed registry replay")
    }

    fn with_reviewed_registry(&self, mutate: impl FnOnce(&mut DomainPackReviewedRegistry)) -> Self {
        let mut changed = self.clone();
        mutate(&mut changed.reviewed_registry.domain_pack_reviewed_registry);
        for entry in &mut changed
            .reviewed_registry
            .domain_pack_reviewed_registry
            .entries
        {
            entry.entry_digest = domain_pack_reviewed_registry_entry_digest(entry)
                .expect("changed reviewed entry digest");
        }
        resign_reviewed_registry(&mut changed.reviewed_registry);
        changed
    }

    /// Produce a freshly signed current catalog that carries an exact revocation
    /// fact for the otherwise byte-identical historical record. Tests use a fresh
    /// anchor for this head, so it remains a valid generation-one current view.
    fn with_current_supply_chain_revocation(&self) -> Self {
        let mut changed = self.clone();
        let record_digest = changed.resolved.registry_record_digest.clone();
        {
            let registry = &mut changed.snapshot.domain_pack_supply_chain_registry;
            registry.revocations = vec![DomainPackPackageRevocation {
                record_digest,
                reason: DomainPackRevocationReason::PackageTamper,
                explanation: "current signed package revocation".to_owned(),
                revoked_at_unix: registry.issued_at_unix,
            }];
            registry.snapshot_digest = digest('0');
            registry.signatures.clear();
        }
        changed
            .snapshot
            .domain_pack_supply_chain_registry
            .snapshot_digest = domain_pack_registry_snapshot_digest(&changed.snapshot)
            .expect("revoked supply-chain snapshot digest");
        for (key_id, key) in [
            (id("registry.key.a"), SigningKey::from_bytes(&[1_u8; 32])),
            (id("registry.key.b"), SigningKey::from_bytes(&[2_u8; 32])),
        ] {
            let bytes = domain_pack_registry_signing_bytes(
                &changed.snapshot,
                &key_id,
                DomainPackRegistryTrustRole::RegistrySigner,
            )
            .expect("revoked registry signing bytes");
            changed
                .snapshot
                .domain_pack_supply_chain_registry
                .signatures
                .push(DomainPackRegistrySignature {
                    signer_key_id: key_id,
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                    signature_hex: hex(&key.sign(&bytes).to_bytes()),
                });
        }
        changed
    }
}

fn resign_reviewed_registry(registry: &mut DomainPackReviewedRegistryDocument) {
    let keys = [
        SigningKey::from_bytes(&[4_u8; 32]),
        SigningKey::from_bytes(&[5_u8; 32]),
    ];
    registry.domain_pack_reviewed_registry.registry_digest =
        domain_pack_reviewed_registry_digest(registry).expect("changed reviewed registry digest");
    let subject_digest = registry
        .domain_pack_reviewed_registry
        .registry_digest
        .clone();
    for (index, key) in keys.iter().enumerate() {
        registry.domain_pack_reviewed_registry.snapshot_signatures[index]
            .payload_digest
            .clone_from(&subject_digest);
        registry.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            "00".repeat(64);
        let signature = registry.domain_pack_reviewed_registry.snapshot_signatures[index].clone();
        let bytes = domain_pack_reviewed_registry_signing_bytes(registry, &signature)
            .expect("changed reviewed registry signing bytes");
        registry.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }
}

fn expected_from_projection(
    projection: &forge_core_domain_pack_tcb::DomainPackLifecycleStateProjection,
    project_snapshot_digest: &str,
) -> DomainPackExpectedLifecycleState {
    match &projection.active_pointer {
        None => DomainPackExpectedLifecycleState::Uninitialized {
            project_snapshot_digest: project_snapshot_digest.to_owned(),
        },
        Some(document) => {
            let pointer = &document.domain_pack_active_pointer;
            DomainPackExpectedLifecycleState::Initialized {
                generation: pointer.generation,
                active_lock_digest: pointer.active_lock_digest.clone(),
                lifecycle_head_digest: pointer.lifecycle_head_digest.clone(),
                project_snapshot_digest: project_snapshot_digest.to_owned(),
            }
        }
    }
}

fn current_project_snapshot_digest(project_root: &Path) -> String {
    match verify_domain_pack_project_snapshot(project_root, &digest('0')) {
        Err(DomainPackLifecycleStoreError::StaleExpectedState { actual, .. }) => actual,
        Ok(_) => digest('0'),
        Err(error) => panic!("compute project snapshot: {error}"),
    }
}

#[derive(Clone)]
struct IntegratedInstallInputs {
    resolution_request: DomainPackResolutionRequestDocument,
    composition_request: DomainPackCompositionRequestDocument,
    trust_input: DomainPackTrustEvaluationInput,
    trust_policy: DomainPackTrustPolicyDocument,
}

fn locked_package(package: &DomainPackResolvedPackage) -> DomainPackLockedPackage {
    DomainPackLockedPackage {
        identity: package.identity.clone(),
        package_digest: package.package.package_digest.clone(),
        manifest_binding: package.package.manifest.clone(),
        content_binding: package.package.content.clone(),
        license_binding: package.package.license.clone(),
        fixture_bindings: package.package.fixtures.clone(),
        namespace_grant_id: package.namespace_grant_id.clone(),
        registry_record_digest: package.registry_record_digest.clone(),
        source_assurance: package.source_assurance,
        semantic_assurance: package.semantic_assurance,
        reviewed_entry_digest: package.reviewed_entry_digest.clone(),
        promotion_authorization_digest: package.promotion_authorization_digest.clone(),
        dependencies: package.dependencies.clone(),
        deterministic_order: package.deterministic_order,
    }
}

#[allow(clippy::too_many_lines)] // Explicit construction is the adversarial cross-boundary oracle.
fn integrated_install_preflight(
    fixture: &Fixture,
    expected: DomainPackExpectedLifecycleState,
    material: &DomainPackCandidateMaterial<'_>,
) -> (
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let mut composition_request: DomainPackCompositionRequestDocument = yaml_serde::from_str(
        &fs::read_to_string(
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"),
        )
        .expect("composition fixture"),
    )
    .expect("typed composition fixture");
    let sealed_core_policy = composition_request
        .domain_pack_composition_request
        .candidates[1]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies[0]
        .clone();
    let core = &mut composition_request.domain_pack_composition_request.core;
    core.bundle.policies = vec![sealed_core_policy];
    core.bundle_digest = canonical_digest(&core.bundle);
    core.policy_set_digest = canonical_digest(&core.bundle.policies);
    composition_request
        .domain_pack_composition_request
        .request_id = id("composition.integrated");
    composition_request
        .domain_pack_composition_request
        .candidates
        .truncate(1);
    let manifest: DomainPackManifestDocument = yaml_serde::from_str(
        std::str::from_utf8(material.manifest_raw).expect("integrated manifest is UTF-8 YAML"),
    )
    .expect("typed integrated manifest");
    let candidate = &mut composition_request
        .domain_pack_composition_request
        .candidates[0];
    candidate.manifest = manifest;
    candidate.manifest_binding = fixture.resolved.package.manifest.clone();
    candidate.content = yaml_serde::from_str(
        std::str::from_utf8(material.content_raw).expect("integrated content is UTF-8 YAML"),
    )
    .expect("typed integrated content");
    composition_request
        .domain_pack_composition_request
        .requirements
        .required_domains
        .truncate(1);
    ">=1,<3".clone_into(
        &mut composition_request
            .domain_pack_composition_request
            .requirements
            .required_domains[0]
            .pack_version_requirement,
    );
    composition_request
        .domain_pack_composition_request
        .requirements
        .project_id = id("project.domain-pack.test");
    let materials = [*material];
    let composition = compose_domain_packs(&composition_request, &materials);
    assert_eq!(
        composition.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Composable,
        "issues={:?} gaps={:?}",
        composition.domain_pack_composition_projection.issues,
        composition.domain_pack_composition_projection.gaps
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let mut trust_policy = fixture.policy.clone();
    trust_policy.domain_pack_trust_policy.rules[0].disposition =
        DomainPackTrustDisposition::ActivateDeclarativeKnowledgeAndBoundBuiltIns;
    let verified = verify_domain_pack_supply_chain_snapshot(&trust_policy, &fixture.snapshot, now)
        .expect("integrated verified snapshot");
    let requirements = DomainPackProjectRequirementsDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_project_requirements: composition_request
            .domain_pack_composition_request
            .requirements
            .clone(),
    };
    let root = DomainPackResolutionRoot {
        pack: coordinate(fixture),
        version_requirement: format!("={}", fixture.resolved.identity.version),
        required_content_digest: Some(fixture.resolved.package.content.canonical_sha256.clone()),
        reason: DomainPackResolutionRootReason::InstallIntent,
    };
    let candidate = DomainPackResolutionCandidate {
        input: composition_request
            .domain_pack_composition_request
            .candidates[0]
            .clone(),
        package: fixture.resolved.package.clone(),
        registry_record_digest: Some(fixture.resolved.registry_record_digest.clone()),
    };
    let resolution_request = DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: id("resolution.integrated"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            forge_core_version: composition_request
                .domain_pack_composition_request
                .forge_core_version
                .clone(),
            core: composition_request
                .domain_pack_composition_request
                .core
                .clone(),
            requirements: requirements.clone(),
            roots: vec![root.clone()],
            current_lock: None,
            policy: DomainPackResolutionPolicy {
                selection: DomainPackVersionSelectionPolicy::MinimalChangeThenHighestCompatible,
                prerelease: DomainPackPrereleasePolicy::ExplicitOnly,
                duplicate_version: DomainPackDuplicateVersionPolicy::RejectDivergentContent,
                dependency_source: DomainPackDependencySourcePolicy::ExactPublisherOnly,
                unrelated_updates: DomainPackUnrelatedUpdatePolicy::PreserveLocked,
            },
            registry_snapshot_digest: verified.snapshot_digest().to_owned(),
            candidates: vec![candidate],
        },
    };
    let mut resolution = resolve_domain_packs(&resolution_request, &fixture.snapshot);
    assert_eq!(
        resolution.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved,
        "{:?}",
        resolution.domain_pack_resolution_projection.issues
    );
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(
            fixture
                .reviewed_registry
                .domain_pack_reviewed_registry
                .entries[0]
                .entry_digest
                .clone(),
        );
        selected.promotion_authorization_digest = Some(
            fixture
                .reviewed_registry
                .domain_pack_reviewed_registry
                .entries[0]
                .authorization_digest
                .clone(),
        );
    }
    resolution
        .domain_pack_resolution_projection
        .resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        verified.snapshot_digest(),
        &resolution.domain_pack_resolution_projection,
    );
    let selected = resolution.domain_pack_resolution_projection.selected[0].clone();
    let assessment = DomainPackSupplyChainAssessment {
        package_digest: selected.package.package_digest.clone(),
        registry_record_digest: selected.registry_record_digest.clone(),
        publisher_signature_verified: true,
        registry_signature_threshold_verified: true,
        namespace_grant_verified: true,
        revoked: false,
    };
    let capability_ref = id("sample.foundation.capability.repository-inspection");
    let subjects = [
        id("sample.foundation.transition.verify"),
        id("sample.foundation.adapter.repository-view"),
        composition_request
            .domain_pack_composition_request
            .requirements
            .required_domains[0]
            .id
            .clone(),
    ];
    let demands = subjects
        .iter()
        .cloned()
        .map(|subject_ref| DomainPackCapabilityDemand {
            subject_ref,
            capability_ref: capability_ref.clone(),
            kind: DomainPackCapabilityKind::Evaluator,
        })
        .collect::<Vec<_>>();
    let binding_ids = (0..subjects.len())
        .map(|index| id(&format!("binding.integrated.repository-inspector.{index}")))
        .collect::<Vec<_>>();
    let bindings = subjects
        .iter()
        .zip(&binding_ids)
        .map(
            |(subject_ref, binding_id)| DomainPackRuntimeCapabilityBinding {
                binding_id: binding_id.clone(),
                pack: DomainPackVersionReference {
                    publisher: selected.identity.publisher.clone(),
                    name: selected.identity.name.clone(),
                    version: selected.identity.version.clone(),
                },
                package_digest: selected.package.package_digest.clone(),
                subject_ref: subject_ref.clone(),
                capability_ref: capability_ref.clone(),
                kind: DomainPackCapabilityKind::Evaluator,
                provider: DomainPackRuntimeProvider::CoreBuiltin {
                    provider_id: id("core.repository-inspector"),
                },
                implementation_digest: digest('a'),
                status: DomainPackRuntimeCapabilityStatus::Available,
                evidence: selected.package.fixtures[0].clone(),
            },
        )
        .collect();
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: id("project.domain-pack.test"),
        selected: vec![DomainPackTrustSelectedPackage {
            package: selected.clone(),
            structurally_valid: true,
            supply_chain: assessment.clone(),
            capability_demands: demands,
        }],
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: DomainPackRuntimeCapabilityRegistry {
            registry_id: id("registry.runtime.integrated"),
            registry_version: "1.0.0".to_owned(),
            project_id: id("project.domain-pack.test"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            bindings,
        },
        sandbox_policy: DomainPackCapabilitySandboxPolicy {
            policy_id: id("policy.sandbox.integrated"),
            policy_version: "1.0.0".to_owned(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            default_decision: DomainPackSandboxDefaultDecision::Deny,
            allowed_builtin_binding_ids: binding_ids,
            external_execution: DomainPackExternalExecutionPolicy::DenyAll,
        },
    };
    let trust_evaluation = forge_core_decisions::evaluate_domain_pack_trust(&trust_input);
    assert_eq!(
        trust_evaluation.status,
        forge_core_decisions::DomainPackTrustEvaluationStatus::Approved,
        "{:?}",
        trust_evaluation.issues
    );
    let capability_registry_document = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry.clone(),
    };
    let sandbox_policy_document = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: trust_input.sandbox_policy.clone(),
    };
    let payload = DomainPackExactLockPayload {
        project_id: id("project.domain-pack.test"),
        core: composition_request
            .domain_pack_composition_request
            .core
            .clone(),
        requirements_digest: canonical_digest(&requirements),
        roots: vec![root],
        registry_snapshot_digest: verified.snapshot_digest().to_owned(),
        reviewer_registry_digest: fixture
            .reviewer_registry
            .domain_pack_reviewer_registry
            .registry_digest
            .clone(),
        reviewed_registry_digest: fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest
            .clone(),
        trust_policy_digest: verified.trust_policy_digest().to_owned(),
        capability_registry_digest: canonical_digest(&capability_registry_document),
        sandbox_policy_digest: canonical_digest(&sandbox_policy_document),
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: vec![locked_package(&selected)],
        verified_capability_bindings: trust_evaluation.verified_capability_bindings.clone(),
        unresolved_composition_gaps: Vec::new(),
        unresolved_capability_gaps: Vec::new(),
    };
    let proposed_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&payload),
            payload,
        },
    };
    let operation = DomainPackLifecycleOperation::Install {
        root: coordinate(fixture),
    };
    let compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: id("compatibility.integrated"),
        operation: operation.clone(),
        sealed_core: composition_request
            .domain_pack_composition_request
            .core
            .clone(),
        from_lock: None,
        to_lock: proposed_lock.clone(),
    });
    assert_eq!(
        compatibility_report.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Compatible
    );
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: id("lifecycle.integrated"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            principal_id: id("principal.integrated"),
            operation,
            expected_state: expected.clone(),
            resolution_request_digest: canonical_digest(&resolution_request),
            project_snapshot_digest: match &expected {
                DomainPackExpectedLifecycleState::Uninitialized {
                    project_snapshot_digest,
                }
                | DomainPackExpectedLifecycleState::Initialized {
                    project_snapshot_digest,
                    ..
                } => project_snapshot_digest.clone(),
            },
        },
    };
    let content_binding = &selected.package.content;
    let mut staged_artifacts = vec![
        selected.package.manifest.clone(),
        DomainPackArtifactBinding {
            artifact_ref: content_binding.content_ref.clone(),
            raw_sha256: content_binding.raw_sha256.clone(),
            canonical_sha256: content_binding.canonical_sha256.clone(),
        },
        selected.package.license.clone(),
    ];
    staged_artifacts.extend(selected.package.fixtures.clone());
    let mut document = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: id("preflight.integrated"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request),
            request: lifecycle_request,
            observed_state: expected,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: vec![assessment],
            trust_decisions: trust_evaluation.trust_decisions,
            capability_gaps: Vec::new(),
            compatibility_report,
            staged_artifacts,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: String::new(),
        },
    };
    document.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&document);
    (
        document,
        IntegratedInstallInputs {
            resolution_request,
            composition_request,
            trust_input,
            trust_policy,
        },
    )
}

fn integrated_upgrade_preflight(
    source: &Fixture,
    target: &Fixture,
    expected: DomainPackExpectedLifecycleState,
    previous_lock: DomainPackExactLockDocument,
    material: &DomainPackCandidateMaterial<'_>,
) -> (
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let (mut document, mut inputs) =
        integrated_install_preflight(target, expected.clone(), material);
    let target_requirement = format!("={}", target.resolved.identity.version);
    let required_content_digest = Some(target.resolved.package.content.canonical_sha256.clone());
    let operation = DomainPackLifecycleOperation::Upgrade {
        pack: coordinate(target),
        expected_from: source.resolved.identity.version.clone(),
        target_requirement: target_requirement.clone(),
        required_content_digest: required_content_digest.clone(),
    };

    let resolution_request = &mut inputs.resolution_request.domain_pack_resolution_request;
    resolution_request.request_id = id("resolution.integrated-upgrade");
    resolution_request.current_lock = Some(previous_lock.clone());
    resolution_request.roots[0].version_requirement = target_requirement;
    resolution_request.roots[0].required_content_digest = required_content_digest;
    resolution_request.roots[0].reason = DomainPackResolutionRootReason::UpgradeIntent;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let verified =
        verify_domain_pack_supply_chain_snapshot(&inputs.trust_policy, &target.snapshot, now)
            .expect("versioned upgrade supply-chain proof");
    let mut resolution = resolve_domain_packs(&inputs.resolution_request, &target.snapshot);
    assert_eq!(
        resolution.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved,
        "{:?}",
        resolution.domain_pack_resolution_projection.issues
    );
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected
            .reviewed_entry_digest
            .clone_from(&target.resolved.reviewed_entry_digest);
        selected
            .promotion_authorization_digest
            .clone_from(&target.resolved.promotion_authorization_digest);
    }
    resolution
        .domain_pack_resolution_projection
        .resolution_digest = domain_pack_resolution_projection_digest(
        &inputs.resolution_request,
        verified.snapshot_digest(),
        &resolution.domain_pack_resolution_projection,
    );

    let body = &mut document.domain_pack_lifecycle_preflight;
    body.preflight_id = id("preflight.integrated-upgrade");
    body.observed_state = expected.clone();
    body.resolution = resolution;
    let payload = &mut body.proposed_lock.domain_pack_exact_lock.payload;
    payload.roots.clone_from(
        &inputs
            .resolution_request
            .domain_pack_resolution_request
            .roots,
    );
    payload.resolution_digest.clone_from(
        &body
            .resolution
            .domain_pack_resolution_projection
            .resolution_digest,
    );
    body.proposed_lock.domain_pack_exact_lock.lock_digest = canonical_digest(payload);
    body.compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: id("compatibility.integrated-upgrade"),
        operation: operation.clone(),
        sealed_core: payload.core.clone(),
        from_lock: Some(previous_lock),
        to_lock: body.proposed_lock.clone(),
    });
    assert_eq!(
        body.compatibility_report
            .domain_pack_compatibility_report
            .status,
        DomainPackCompatibilityStatus::Compatible,
        "{:?}",
        body.compatibility_report
            .domain_pack_compatibility_report
            .issues
    );
    body.request.domain_pack_lifecycle_request.request_id = id("lifecycle.integrated-upgrade");
    body.request.domain_pack_lifecycle_request.operation = operation;
    body.request.domain_pack_lifecycle_request.expected_state = expected;
    body.request
        .domain_pack_lifecycle_request
        .resolution_request_digest = canonical_digest(&inputs.resolution_request);
    body.request_digest = canonical_digest(&body.request);
    body.preflight_digest.clear();
    let preflight_digest = canonical_digest(&document);
    document.domain_pack_lifecycle_preflight.preflight_digest = preflight_digest;
    (document, inputs)
}

fn coordinate(fixture: &Fixture) -> DomainPackCoordinate {
    DomainPackCoordinate {
        publisher: fixture.resolved.identity.publisher.clone(),
        name: fixture.resolved.identity.name.clone(),
    }
}

#[allow(clippy::too_many_lines)] // Removal must preserve the full exact prior-input matrix.
fn integrated_remove_preflight(
    fixture: &Fixture,
    base_inputs: &IntegratedInstallInputs,
    expected: DomainPackExpectedLifecycleState,
    previous_lock: DomainPackExactLockDocument,
) -> (
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let mut composition_request = base_inputs.composition_request.clone();
    composition_request
        .domain_pack_composition_request
        .request_id = id("composition.integrated-remove");
    composition_request
        .domain_pack_composition_request
        .candidates
        .clear();
    let composition = compose_domain_packs(&composition_request, &[]);
    assert!(
        matches!(
            composition.domain_pack_composition_projection.status,
            DomainPackCompositionStatus::Composable | DomainPackCompositionStatus::Blocked
        ),
        "{:?}",
        composition.domain_pack_composition_projection.issues
    );
    assert!(composition
        .domain_pack_composition_projection
        .issues
        .is_empty());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let registry_snapshot_digest =
        verify_domain_pack_supply_chain_snapshot(&base_inputs.trust_policy, &fixture.snapshot, now)
            .expect("fresh remove supply-chain proof")
            .snapshot_digest()
            .to_owned();
    let mut resolution_request = base_inputs.resolution_request.clone();
    let request = &mut resolution_request.domain_pack_resolution_request;
    request.request_id = id("resolution.integrated-remove");
    request
        .registry_snapshot_digest
        .clone_from(&registry_snapshot_digest);
    request.roots.clear();
    request.candidates.clear();
    request.current_lock = Some(previous_lock.clone());
    let resolution = resolve_domain_packs(&resolution_request, &fixture.snapshot);
    assert_eq!(
        resolution.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved,
        "{:?}",
        resolution.domain_pack_resolution_projection.issues
    );
    assert!(resolution
        .domain_pack_resolution_projection
        .selected
        .is_empty());

    let mut trust_input = base_inputs.trust_input.clone();
    trust_input.selected.clear();
    trust_input.capability_registry.bindings.clear();
    trust_input
        .sandbox_policy
        .allowed_builtin_binding_ids
        .clear();
    let trust_evaluation = forge_core_decisions::evaluate_domain_pack_trust(&trust_input);
    assert_eq!(
        trust_evaluation.status,
        forge_core_decisions::DomainPackTrustEvaluationStatus::Approved,
        "{:?}",
        trust_evaluation.issues
    );
    let capability_registry_document = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry.clone(),
    };
    let sandbox_policy_document = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: trust_input.sandbox_policy.clone(),
    };
    let requirements = DomainPackProjectRequirementsDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_project_requirements: composition_request
            .domain_pack_composition_request
            .requirements
            .clone(),
    };
    let mut payload = previous_lock.domain_pack_exact_lock.payload.clone();
    payload.registry_snapshot_digest = registry_snapshot_digest;
    payload.requirements_digest = canonical_digest(&requirements);
    payload.roots.clear();
    payload.reviewer_registry_digest.clone_from(
        &fixture
            .reviewer_registry
            .domain_pack_reviewer_registry
            .registry_digest,
    );
    payload.reviewed_registry_digest.clone_from(
        &fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest,
    );
    payload.capability_registry_digest = canonical_digest(&capability_registry_document);
    payload.sandbox_policy_digest = canonical_digest(&sandbox_policy_document);
    payload.resolution_digest.clone_from(
        &resolution
            .domain_pack_resolution_projection
            .resolution_digest,
    );
    payload.composition_digest.clone_from(
        &composition
            .domain_pack_composition_projection
            .composition_digest,
    );
    payload.packages.clear();
    payload.verified_capability_bindings.clear();
    payload
        .unresolved_composition_gaps
        .clone_from(&composition.domain_pack_composition_projection.gaps);
    payload.unresolved_capability_gaps.clear();
    let proposed_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&payload),
            payload,
        },
    };
    let operation = DomainPackLifecycleOperation::Remove {
        pack: coordinate(fixture),
    };
    let compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: id("compatibility.integrated-remove"),
        operation: operation.clone(),
        sealed_core: previous_lock.domain_pack_exact_lock.payload.core.clone(),
        from_lock: Some(previous_lock),
        to_lock: proposed_lock.clone(),
    });
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: id("lifecycle.integrated-remove"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            principal_id: id("principal.integrated"),
            operation,
            expected_state: expected.clone(),
            resolution_request_digest: canonical_digest(&resolution_request),
            project_snapshot_digest: match &expected {
                DomainPackExpectedLifecycleState::Uninitialized {
                    project_snapshot_digest,
                }
                | DomainPackExpectedLifecycleState::Initialized {
                    project_snapshot_digest,
                    ..
                } => project_snapshot_digest.clone(),
            },
        },
    };
    let mut document = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: id("preflight.integrated-remove"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request),
            request: lifecycle_request,
            observed_state: expected,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: Vec::new(),
            trust_decisions: trust_evaluation.trust_decisions,
            capability_gaps: Vec::new(),
            compatibility_report,
            staged_artifacts: Vec::new(),
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: String::new(),
        },
    };
    document.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&document);
    (
        document,
        IntegratedInstallInputs {
            resolution_request,
            composition_request,
            trust_input,
            trust_policy: base_inputs.trust_policy.clone(),
        },
    )
}

struct RawArtifactFixture {
    manifest: Vec<u8>,
    content: Vec<u8>,
    license: Vec<u8>,
    representative: Vec<u8>,
    content_binding: DomainPackArtifactBinding,
}

impl RawArtifactFixture {
    fn new(fixture: &Fixture) -> Self {
        Self {
            manifest: fs::read(
                repo_root().join("docs/fixtures/domain-pack-v0/manifests/foundation.yaml"),
            )
            .expect("raw manifest"),
            content: fs::read(
                repo_root().join("docs/fixtures/domain-pack-v0/content/foundation.yaml"),
            )
            .expect("raw content"),
            license: fs::read(
                repo_root().join("docs/fixtures/domain-pack-v0/artifacts/license-notice.yaml"),
            )
            .expect("raw license"),
            representative: fs::read(
                repo_root()
                    .join("docs/fixtures/domain-pack-v0/artifacts/foundation-representative.yaml"),
            )
            .expect("raw representative fixture"),
            content_binding: DomainPackArtifactBinding {
                artifact_ref: fixture.resolved.package.content.content_ref.clone(),
                raw_sha256: fixture.resolved.package.content.raw_sha256.clone(),
                canonical_sha256: fixture.resolved.package.content.canonical_sha256.clone(),
            },
        }
    }

    fn material<'a>(&'a self, fixture: &'a Fixture) -> DomainPackCandidateMaterial<'a> {
        DomainPackCandidateMaterial {
            publisher: &fixture.resolved.identity.publisher.0,
            name: &fixture.resolved.identity.name.0,
            version: &fixture.resolved.identity.version,
            manifest_raw: &self.manifest,
            content_raw: &self.content,
            license_raw: &self.license,
        }
    }

    fn immutable<'a>(&'a self, fixture: &'a Fixture) -> [DomainPackImmutableArtifact<'a>; 4] {
        [
            DomainPackImmutableArtifact {
                binding: &fixture.resolved.package.manifest,
                raw_bytes: &self.manifest,
            },
            DomainPackImmutableArtifact {
                binding: &self.content_binding,
                raw_bytes: &self.content,
            },
            DomainPackImmutableArtifact {
                binding: &fixture.resolved.package.license,
                raw_bytes: &self.license,
            },
            DomainPackImmutableArtifact {
                binding: &fixture.resolved.package.fixtures[0],
                raw_bytes: &self.representative,
            },
        ]
    }
}

/// Derive a second, independently content-addressed and fully signed version
/// from the repository fixture. Only the package version identity changes;
/// every derived content/manifest/package/registry/review binding is recomputed so the
/// upgrade exercises real admission rather than editing a version label in an
/// already trusted lock.
// This fixture intentionally keeps the complete content-addressed derivation in one
// linear narrative so changes cannot accidentally omit one of the linked bindings.
#[allow(clippy::too_many_lines)]
fn versioned_upgrade_fixture(base: &Fixture, version: &str) -> (Fixture, RawArtifactFixture) {
    let mut fixture = base.clone();
    let mut raw = RawArtifactFixture::new(base);
    let mut content: DomainPackContentDocument = yaml_serde::from_str(
        std::str::from_utf8(&raw.content).expect("base content is UTF-8 YAML"),
    )
    .expect("typed base content");
    version.clone_into(&mut content.domain_pack_content.pack.version);
    raw.content = yaml_serde::to_string(&content)
        .expect("serialize versioned content")
        .into_bytes();
    let content_raw_sha256 = format!("sha256:{:x}", Sha256::digest(&raw.content));
    let content_canonical_sha256 = canonical_digest(&content);
    let mut manifest: DomainPackManifestDocument = yaml_serde::from_str(
        std::str::from_utf8(&raw.manifest).expect("base manifest is UTF-8 YAML"),
    )
    .expect("typed base manifest");
    version.clone_into(&mut manifest.domain_pack_manifest.identity.version);
    manifest
        .domain_pack_manifest
        .content
        .raw_sha256
        .clone_from(&content_raw_sha256);
    manifest
        .domain_pack_manifest
        .content
        .canonical_sha256
        .clone_from(&content_canonical_sha256);
    raw.manifest = yaml_serde::to_string(&manifest)
        .expect("serialize versioned manifest")
        .into_bytes();
    let manifest_binding = DomainPackArtifactBinding {
        artifact_ref: fixture.resolved.package.manifest.artifact_ref.clone(),
        raw_sha256: format!("sha256:{:x}", Sha256::digest(&raw.manifest)),
        canonical_sha256: canonical_digest(&manifest),
    };
    let package_digest = format!(
        "sha256:{:x}",
        Sha256::digest(
            [
                raw.manifest.as_slice(),
                raw.content.as_slice(),
                raw.license.as_slice(),
                raw.representative.as_slice(),
            ]
            .concat()
        )
    );

    fixture.policy.domain_pack_trust_policy.rules[0].package_digest = Some(package_digest.clone());
    fixture.policy.domain_pack_trust_policy.rules[0].content_digest =
        Some(content_canonical_sha256.clone());
    version.clone_into(&mut fixture.resolved.identity.version);
    fixture
        .resolved
        .package
        .package_digest
        .clone_from(&package_digest);
    fixture.resolved.package.manifest = manifest_binding.clone();
    fixture
        .resolved
        .package
        .content
        .raw_sha256
        .clone_from(&content_raw_sha256);
    fixture
        .resolved
        .package
        .content
        .canonical_sha256
        .clone_from(&content_canonical_sha256);
    raw.content_binding
        .raw_sha256
        .clone_from(&content_raw_sha256);
    raw.content_binding
        .canonical_sha256
        .clone_from(&content_canonical_sha256);

    let registry = &mut fixture.snapshot.domain_pack_supply_chain_registry;
    let registry_id = registry.registry_id.clone();
    let audience = registry.audience.clone();
    let record = &mut registry.packages[0];
    version.clone_into(&mut record.identity.version);
    record.package_digest.clone_from(&package_digest);
    record
        .manifest_digest
        .clone_from(&manifest_binding.raw_sha256);
    record.content_digest.clone_from(&content_raw_sha256);
    record.artifacts.manifest = remote_descriptor(
        DomainPackRemoteArtifactKind::Manifest,
        manifest_binding.clone(),
        &raw.manifest,
    );
    record.artifacts.content = remote_descriptor(
        DomainPackRemoteArtifactKind::Content,
        raw.content_binding.clone(),
        &raw.content,
    );
    record.publisher_signature_hex = "00".repeat(64);
    record.record_digest = digest('0');
    record.record_digest =
        domain_pack_package_record_digest(record).expect("versioned package record digest");
    let publisher_key = SigningKey::from_bytes(&[3_u8; 32]);
    let publisher_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, record)
        .expect("versioned publisher signing bytes");
    record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());
    let record_digest = record.record_digest.clone();

    version.clone_into(&mut registry.registry_version);
    registry.snapshot_digest = digest('0');
    registry.signatures.clear();
    let snapshot_digest =
        domain_pack_registry_snapshot_digest(&fixture.snapshot).expect("versioned snapshot digest");
    fixture
        .snapshot
        .domain_pack_supply_chain_registry
        .snapshot_digest = snapshot_digest;
    for (key_id, key) in [
        (id("registry.key.a"), SigningKey::from_bytes(&[1_u8; 32])),
        (id("registry.key.b"), SigningKey::from_bytes(&[2_u8; 32])),
    ] {
        let bytes = domain_pack_registry_signing_bytes(
            &fixture.snapshot,
            &key_id,
            DomainPackRegistryTrustRole::RegistrySigner,
        )
        .expect("versioned registry signing bytes");
        fixture
            .snapshot
            .domain_pack_supply_chain_registry
            .signatures
            .push(DomainPackRegistrySignature {
                signer_key_id: key_id,
                role: DomainPackRegistryTrustRole::RegistrySigner,
                signature_hex: hex(&key.sign(&bytes).to_bytes()),
            });
    }

    let reviewed = &mut fixture
        .reviewed_registry
        .domain_pack_reviewed_registry
        .entries[0];
    version.clone_into(&mut reviewed.pack.version);
    reviewed.package_digest.clone_from(&package_digest);
    reviewed
        .supply_chain_record_digest
        .clone_from(&record_digest);
    reviewed
        .manifest_digest
        .clone_from(&manifest_binding.canonical_sha256);
    reviewed.content_digest = content_canonical_sha256;
    reviewed.promotion_decision_digest = learning_digest("promotion-decision-v2");
    reviewed.authorization_digest = learning_digest("promotion-authorization-v2");
    reviewed.independent_review_digests = vec![
        learning_digest("review-semantic-v2"),
        learning_digest("review-authorizer-v2"),
    ];
    reviewed.compatibility.predecessor_content_digests =
        vec![base.resolved.package.content.canonical_sha256.clone()];
    reviewed.entry_digest = domain_pack_reviewed_registry_entry_digest(reviewed)
        .expect("versioned reviewed entry digest");
    resign_reviewed_registry(&mut fixture.reviewed_registry);
    assert_eq!(
        domain_pack_reviewed_registry_entry_digest(
            &fixture
                .reviewed_registry
                .domain_pack_reviewed_registry
                .entries[0]
        )
        .expect("recheck versioned reviewed entry"),
        fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .entries[0]
            .entry_digest
    );

    fixture.resolved.registry_record_digest = record_digest;
    fixture.resolved.reviewed_entry_digest = Some(
        fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .entries[0]
            .entry_digest
            .clone(),
    );
    fixture.resolved.promotion_authorization_digest = Some(
        fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .entries[0]
            .authorization_digest
            .clone(),
    );
    (fixture, raw)
}

fn authorize_integrated(
    project_root: &Path,
    fixture: &Fixture,
    raw: &RawArtifactFixture,
    inputs: &IntegratedInstallInputs,
    prepared: &forge_core_domain_pack_tcb::PreparedDomainPackLifecycleTransaction,
) -> Result<
    forge_core_domain_pack_tcb::DomainPackLifecycleCommitAuthority,
    DomainPackLifecycleStoreError,
> {
    let project_digest = current_project_snapshot_digest(project_root);
    let project_snapshot = verify_domain_pack_project_snapshot(project_root, &project_digest)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let verified =
        verify_domain_pack_supply_chain_snapshot(&inputs.trust_policy, &fixture.snapshot, now)
            .expect("fresh integrated supply-chain proof");
    let anchored = fixture.anchored(verified);
    let reviewed_anchored = fixture.reviewed_anchored(now);
    let materials = [raw.material(fixture)];
    let artifacts = raw.immutable(fixture);
    authorize_prepared_domain_pack_lifecycle(
        prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
            anchored_reviewed_snapshot: &reviewed_anchored,
            project_snapshot: &project_snapshot,
            trust_policy_document: &inputs.trust_policy,
            registry_document: &fixture.snapshot,
            resolution_request: &inputs.resolution_request,
            composition_request: &inputs.composition_request,
            materials: &materials,
            artifacts: &artifacts,
            trust_input: &inputs.trust_input,
        },
    )
}

fn authorize_without_artifacts(
    project_root: &Path,
    fixture: &Fixture,
    inputs: &IntegratedInstallInputs,
    prepared: &forge_core_domain_pack_tcb::PreparedDomainPackLifecycleTransaction,
) -> Result<
    forge_core_domain_pack_tcb::DomainPackLifecycleCommitAuthority,
    DomainPackLifecycleStoreError,
> {
    let project_digest = current_project_snapshot_digest(project_root);
    let project_snapshot = verify_domain_pack_project_snapshot(project_root, &project_digest)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let verified =
        verify_domain_pack_supply_chain_snapshot(&inputs.trust_policy, &fixture.snapshot, now)
            .expect("fresh supply-chain proof");
    let anchored = fixture.anchored(verified);
    let reviewed_anchored = fixture.reviewed_anchored(now);
    authorize_prepared_domain_pack_lifecycle(
        prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
            anchored_reviewed_snapshot: &reviewed_anchored,
            project_snapshot: &project_snapshot,
            trust_policy_document: &inputs.trust_policy,
            registry_document: &fixture.snapshot,
            resolution_request: &inputs.resolution_request,
            composition_request: &inputs.composition_request,
            materials: &[],
            artifacts: &[],
            trust_input: &inputs.trust_input,
        },
    )
}

fn commit_integrated_install(
    state_root: &Path,
    fixture: &Fixture,
    raw: &RawArtifactFixture,
) -> (
    DomainPackLifecycleReceiptDocument,
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root).expect("lock fresh lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let material = raw.material(fixture);
    let (document, inputs) = integrated_install_preflight(fixture, expected, &material);
    let prepared = locked
        .prepare_candidate(document.clone())
        .expect("prepare integrated install");
    let authority = authorize_integrated(project_root, fixture, raw, &inputs, &prepared)
        .expect("authorize integrated install");
    let receipt = locked
        .commit(prepared, authority)
        .expect("commit integrated install");
    (receipt, document, inputs)
}

fn commit_integrated_upgrade(
    state_root: &Path,
    source: &Fixture,
    target: &Fixture,
    raw: &RawArtifactFixture,
) -> (
    DomainPackLifecycleReceiptDocument,
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root).expect("lock installed lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let previous_lock = locked
        .projection()
        .active_lock
        .clone()
        .expect("active source lock");
    let material = raw.material(target);
    let (document, inputs) =
        integrated_upgrade_preflight(source, target, expected, previous_lock, &material);
    let prepared = locked
        .prepare_candidate(document.clone())
        .expect("prepare integrated upgrade");
    let authority = authorize_integrated(project_root, target, raw, &inputs, &prepared)
        .expect("authorize integrated upgrade");
    let receipt = locked
        .commit(prepared, authority)
        .expect("commit integrated upgrade");
    (receipt, document, inputs)
}

fn commit_integrated_remove(
    state_root: &Path,
    fixture: &Fixture,
    base_inputs: &IntegratedInstallInputs,
) -> DomainPackLifecycleReceiptDocument {
    commit_integrated_remove_with_preflight(state_root, fixture, base_inputs).0
}

fn commit_integrated_remove_with_preflight(
    state_root: &Path,
    fixture: &Fixture,
    base_inputs: &IntegratedInstallInputs,
) -> (
    DomainPackLifecycleReceiptDocument,
    DomainPackLifecyclePreflightDocument,
    IntegratedInstallInputs,
) {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root).expect("lock installed lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let previous_lock = locked
        .projection()
        .active_lock
        .clone()
        .expect("active installed lock");
    let (document, inputs) =
        integrated_remove_preflight(fixture, base_inputs, expected, previous_lock);
    let prepared = locked
        .prepare_candidate(document.clone())
        .expect("prepare integrated remove");
    let authority = authorize_without_artifacts(project_root, fixture, &inputs, &prepared)
        .expect("authorize integrated remove");
    let receipt = locked
        .commit(prepared, authority)
        .expect("commit integrated remove");
    (receipt, document, inputs)
}

fn commit_integrated_reinstall(
    state_root: &Path,
    fixture: &Fixture,
    raw: &RawArtifactFixture,
) -> DomainPackLifecycleReceiptDocument {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root).expect("lock removed lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let previous_lock = locked
        .projection()
        .active_lock
        .clone()
        .expect("active empty lock");
    let material = raw.material(fixture);
    let (mut document, inputs) = integrated_install_preflight(fixture, expected, &material);
    let body = &mut document.domain_pack_lifecycle_preflight;
    body.compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: id("compatibility.integrated-reinstall"),
        operation: body.request.domain_pack_lifecycle_request.operation.clone(),
        sealed_core: body
            .proposed_lock
            .domain_pack_exact_lock
            .payload
            .core
            .clone(),
        from_lock: Some(previous_lock),
        to_lock: body.proposed_lock.clone(),
    });
    body.preflight_digest.clear();
    let preflight_digest = canonical_digest(&document);
    document.domain_pack_lifecycle_preflight.preflight_digest = preflight_digest;
    let prepared = locked
        .prepare_candidate(document)
        .expect("prepare integrated reinstall");
    let authority = authorize_integrated(project_root, fixture, raw, &inputs, &prepared)
        .expect("authorize integrated reinstall");
    locked
        .commit(prepared, authority)
        .expect("commit integrated reinstall")
}

fn lifecycle_variant(
    base: &DomainPackLifecyclePreflightDocument,
    expected: DomainPackExpectedLifecycleState,
    previous_lock: DomainPackExactLockDocument,
    operation: DomainPackLifecycleOperation,
    marker: &str,
) -> DomainPackLifecyclePreflightDocument {
    let mut document = base.clone();
    let body = &mut document.domain_pack_lifecycle_preflight;
    body.preflight_id = id(&format!("preflight.{marker}"));
    body.request.domain_pack_lifecycle_request.request_id = id(&format!("lifecycle.{marker}"));
    body.request.domain_pack_lifecycle_request.operation = operation.clone();
    body.request.domain_pack_lifecycle_request.expected_state = expected.clone();
    body.request
        .domain_pack_lifecycle_request
        .project_snapshot_digest = match &expected {
        DomainPackExpectedLifecycleState::Uninitialized {
            project_snapshot_digest,
        }
        | DomainPackExpectedLifecycleState::Initialized {
            project_snapshot_digest,
            ..
        } => project_snapshot_digest.clone(),
    };
    body.observed_state = expected;
    body.compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: id(&format!("compatibility.{marker}")),
        operation,
        sealed_core: body
            .proposed_lock
            .domain_pack_exact_lock
            .payload
            .core
            .clone(),
        from_lock: Some(previous_lock),
        to_lock: body.proposed_lock.clone(),
    });
    body.request_digest = canonical_digest(&body.request);
    body.preflight_digest.clear();
    let preflight_digest = canonical_digest(&document);
    document.domain_pack_lifecycle_preflight.preflight_digest = preflight_digest;
    document
}

fn rebind_preflight_to_reviewed_registry(
    mut document: DomainPackLifecyclePreflightDocument,
    resolution_request: &DomainPackResolutionRequestDocument,
    fixture: &Fixture,
) -> DomainPackLifecyclePreflightDocument {
    let body = &mut document.domain_pack_lifecycle_preflight;
    if let Some(entry) = fixture
        .reviewed_registry
        .domain_pack_reviewed_registry
        .entries
        .first()
    {
        for selected in &mut body.resolution.domain_pack_resolution_projection.selected {
            selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
            selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        }
        for package in &mut body.proposed_lock.domain_pack_exact_lock.payload.packages {
            package.reviewed_entry_digest = Some(entry.entry_digest.clone());
            package.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        }
    }
    let projection = &mut body.resolution.domain_pack_resolution_projection;
    projection.resolution_digest = domain_pack_resolution_projection_digest(
        resolution_request,
        &resolution_request
            .domain_pack_resolution_request
            .registry_snapshot_digest,
        projection,
    );
    let lock = &mut body.proposed_lock.domain_pack_exact_lock;
    lock.payload.reviewer_registry_digest.clone_from(
        &fixture
            .reviewer_registry
            .domain_pack_reviewer_registry
            .registry_digest,
    );
    lock.payload.reviewed_registry_digest.clone_from(
        &fixture
            .reviewed_registry
            .domain_pack_reviewed_registry
            .registry_digest,
    );
    lock.payload
        .resolution_digest
        .clone_from(&projection.resolution_digest);
    lock.lock_digest = canonical_digest(&lock.payload);
    let operation = body.request.domain_pack_lifecycle_request.operation.clone();
    body.compatibility_report = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: body
            .compatibility_report
            .domain_pack_compatibility_report
            .report_id
            .clone(),
        operation,
        sealed_core: lock.payload.core.clone(),
        from_lock: None,
        to_lock: body.proposed_lock.clone(),
    });
    body.preflight_digest.clear();
    let preflight_digest = canonical_digest(&document);
    document.domain_pack_lifecycle_preflight.preflight_digest = preflight_digest;
    document
}

fn attempt_variant(
    state_root: &Path,
    fixture: &Fixture,
    raw: &RawArtifactFixture,
    base: &DomainPackLifecyclePreflightDocument,
    inputs: &IntegratedInstallInputs,
    operation: DomainPackLifecycleOperation,
    marker: &str,
) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root)?;
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let previous_lock = locked
        .projection()
        .active_lock
        .clone()
        .expect("initialized exact lock");
    let document = lifecycle_variant(base, expected, previous_lock, operation, marker);
    let prepared = locked.prepare_candidate(document)?;
    let authority = authorize_integrated(project_root, fixture, raw, inputs, &prepared)?;
    locked.commit(prepared, authority)
}

fn attempt_empty_variant(
    state_root: &Path,
    fixture: &Fixture,
    base: &DomainPackLifecyclePreflightDocument,
    inputs: &IntegratedInstallInputs,
    operation: DomainPackLifecycleOperation,
    marker: &str,
) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
    let project_root = state_root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let mut locked = lock_domain_pack_lifecycle(state_root)?;
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let previous_lock = locked
        .projection()
        .active_lock
        .clone()
        .expect("initialized exact lock");
    let document = lifecycle_variant(base, expected, previous_lock, operation, marker);
    let prepared = locked.prepare_candidate(document)?;
    let authority = authorize_without_artifacts(project_root, fixture, inputs, &prepared)?;
    locked.commit(prepared, authority)
}

fn generation_directories(state_root: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(state_root.join("domain-packs/generations"))
        .expect("generation directory")
        .map(|entry| entry.expect("generation entry").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[cfg(unix)]
fn create_directory_link(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create directory symlink");
}

#[cfg(windows)]
fn create_directory_link(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()
        .expect("invoke mklink");
    assert!(status.success(), "create directory junction");
}

#[cfg(unix)]
fn remove_directory_link(link: &Path) {
    fs::remove_file(link).expect("remove directory symlink");
}

#[cfg(windows)]
fn remove_directory_link(link: &Path) {
    fs::remove_dir(link).expect("remove directory junction");
}

#[test]
fn install_upgrade_remove_and_rollback_reject_semantically_incompatible_intent() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("semantic-operations");
    let (install, base, inputs) = commit_integrated_install(&root, &fixture, &raw);

    let cases = [
        (
            "install-initialized",
            DomainPackLifecycleOperation::Install {
                root: coordinate(&fixture),
            },
        ),
        (
            "upgrade-no-version-change",
            DomainPackLifecycleOperation::Upgrade {
                pack: coordinate(&fixture),
                expected_from: fixture.resolved.identity.version.clone(),
                target_requirement: format!("^{}", fixture.resolved.identity.version),
                required_content_digest: None,
            },
        ),
        (
            "remove-retains-pack",
            DomainPackLifecycleOperation::Remove {
                pack: coordinate(&fixture),
            },
        ),
        (
            "rollback-wrong-lock",
            DomainPackLifecycleOperation::Rollback {
                target_receipt_digest: install.domain_pack_lifecycle_receipt.receipt_digest.clone(),
                target_lock_digest: digest('b'),
            },
        ),
        (
            "rebase-core-unchanged",
            DomainPackLifecycleOperation::RebaseCore {
                target_release_id: id("release.core.unchanged"),
                expected_from_core_digest: base
                    .domain_pack_lifecycle_preflight
                    .proposed_lock
                    .domain_pack_exact_lock
                    .payload
                    .core
                    .bundle_digest
                    .clone(),
                target_core_digest: base
                    .domain_pack_lifecycle_preflight
                    .proposed_lock
                    .domain_pack_exact_lock
                    .payload
                    .core
                    .bundle_digest
                    .clone(),
            },
        ),
    ];
    for (marker, operation) in cases {
        let error = attempt_variant(&root, &fixture, &raw, &base, &inputs, operation, marker)
            .expect_err("semantic operation mismatch must fail closed");
        assert!(
            matches!(
                error,
                DomainPackLifecycleStoreError::PreflightBlocked { .. }
            ),
            "{marker}: {error}"
        );
    }

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
#[allow(clippy::too_many_lines)] // One persisted history covers all four successful lifecycle operations.
fn install_upgrade_rollback_and_remove_persist_exact_recoverable_history() {
    let v1 = Fixture::new();
    let raw_v1 = RawArtifactFixture::new(&v1);
    let (v2, raw_v2) = versioned_upgrade_fixture(&v1, "2.0.0");
    let root = temp_state_root("successful-operation-history");

    let (install, install_preflight, install_inputs) =
        commit_integrated_install(&root, &v1, &raw_v1);
    let install_state = install.domain_pack_lifecycle_receipt.to_state.clone();
    assert_eq!(install_state.generation, 0);
    let install_receipt_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        install
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .trim_start_matches("sha256:")
    ));
    let install_receipt_bytes = fs::read(&install_receipt_path).expect("persisted install receipt");
    let install_generation = generation_directories(&root)
        .into_iter()
        .next()
        .expect("install generation");
    let install_lock_bytes =
        fs::read(install_generation.join("lock.yaml")).expect("persisted install lock");
    let install_preflight_bytes =
        fs::read(install_generation.join("preflight.yaml")).expect("persisted install preflight");
    {
        let recovered = lock_domain_pack_lifecycle(&root).expect("recover install");
        assert_eq!(recovered.projection().ledger_records.len(), 1);
        assert_eq!(
            recovered
                .projection()
                .active_pointer
                .as_ref()
                .map(|pointer| &pointer.domain_pack_active_pointer),
            Some(&install_state)
        );
        assert_eq!(
            recovered
                .projection()
                .active_lock
                .as_ref()
                .unwrap()
                .domain_pack_exact_lock
                .payload
                .packages[0]
                .identity
                .version,
            "1.0.0"
        );
        assert!(matches!(
            recovered.projection().ledger_records[0].operation,
            DomainPackLifecycleOperation::Install { .. }
        ));
    }

    let (upgrade, _, _) = commit_integrated_upgrade(&root, &v1, &v2, &raw_v2);
    let upgrade_state = upgrade.domain_pack_lifecycle_receipt.to_state.clone();
    assert_eq!(upgrade_state.generation, 1);
    let upgrade_receipt_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        upgrade
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .trim_start_matches("sha256:")
    ));
    let upgrade_receipt_bytes = fs::read(&upgrade_receipt_path).expect("persisted upgrade receipt");
    let upgrade_generation = generation_directories(&root)[1].clone();
    let upgrade_lock_bytes =
        fs::read(upgrade_generation.join("lock.yaml")).expect("persisted upgrade lock");
    let upgrade_preflight_bytes =
        fs::read(upgrade_generation.join("preflight.yaml")).expect("persisted upgrade preflight");
    {
        let recovered = lock_domain_pack_lifecycle(&root).expect("recover upgrade");
        assert_eq!(recovered.projection().ledger_records.len(), 2);
        assert_eq!(
            recovered
                .projection()
                .active_pointer
                .as_ref()
                .map(|pointer| &pointer.domain_pack_active_pointer),
            Some(&upgrade_state)
        );
        assert_eq!(
            recovered
                .projection()
                .active_lock
                .as_ref()
                .unwrap()
                .domain_pack_exact_lock
                .payload
                .packages[0]
                .identity
                .version,
            "2.0.0"
        );
        assert!(matches!(
            recovered.projection().ledger_records[1].operation,
            DomainPackLifecycleOperation::Upgrade { .. }
        ));
    }

    let rollback = attempt_variant(
        &root,
        &v1,
        &raw_v1,
        &install_preflight,
        &install_inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: install.domain_pack_lifecycle_receipt.receipt_digest.clone(),
            target_lock_digest: install_state.active_lock_digest.clone(),
        },
        "rollback-after-upgrade",
    )
    .expect("rollback exact v1 generation");
    let rollback_state = rollback.domain_pack_lifecycle_receipt.to_state.clone();
    assert_eq!(rollback_state.generation, 2);
    let rollback_receipt_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        rollback
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .trim_start_matches("sha256:")
    ));
    let rollback_receipt_bytes =
        fs::read(&rollback_receipt_path).expect("persisted rollback receipt");
    let rollback_generation = generation_directories(&root)[2].clone();
    let rollback_lock_bytes =
        fs::read(rollback_generation.join("lock.yaml")).expect("persisted rollback lock");
    let rollback_preflight_bytes =
        fs::read(rollback_generation.join("preflight.yaml")).expect("persisted rollback preflight");
    {
        let recovered = lock_domain_pack_lifecycle(&root).expect("recover rollback");
        assert_eq!(recovered.projection().ledger_records.len(), 3);
        assert_eq!(
            recovered
                .projection()
                .active_pointer
                .as_ref()
                .map(|pointer| &pointer.domain_pack_active_pointer),
            Some(&rollback_state)
        );
        assert_eq!(
            recovered
                .projection()
                .active_lock
                .as_ref()
                .unwrap()
                .domain_pack_exact_lock
                .payload
                .packages[0]
                .identity
                .version,
            "1.0.0"
        );
        assert!(matches!(
            recovered.projection().ledger_records[2].operation,
            DomainPackLifecycleOperation::Rollback { .. }
        ));
    }

    let remove = commit_integrated_remove(&root, &v1, &install_inputs);
    let remove_state = remove.domain_pack_lifecycle_receipt.to_state.clone();
    assert_eq!(remove_state.generation, 3);
    let remove_receipt_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        remove
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .trim_start_matches("sha256:")
    ));
    let remove_receipt_bytes = fs::read(&remove_receipt_path).expect("persisted remove receipt");

    assert_eq!(generation_directories(&root).len(), 4);
    assert_eq!(
        fs::read(&install_receipt_path).unwrap(),
        install_receipt_bytes
    );
    assert_eq!(
        fs::read(&upgrade_receipt_path).unwrap(),
        upgrade_receipt_bytes
    );
    assert_eq!(
        fs::read(&rollback_receipt_path).unwrap(),
        rollback_receipt_bytes
    );
    assert_eq!(
        fs::read(&remove_receipt_path).unwrap(),
        remove_receipt_bytes
    );
    assert_eq!(
        fs::read(install_generation.join("lock.yaml")).unwrap(),
        install_lock_bytes
    );
    assert_eq!(
        fs::read(install_generation.join("preflight.yaml")).unwrap(),
        install_preflight_bytes
    );
    assert_eq!(
        fs::read(upgrade_generation.join("lock.yaml")).unwrap(),
        upgrade_lock_bytes
    );
    assert_eq!(
        fs::read(upgrade_generation.join("preflight.yaml")).unwrap(),
        upgrade_preflight_bytes
    );
    assert_eq!(
        fs::read(rollback_generation.join("lock.yaml")).unwrap(),
        rollback_lock_bytes
    );
    assert_eq!(
        fs::read(rollback_generation.join("preflight.yaml")).unwrap(),
        rollback_preflight_bytes
    );

    let recovered = lock_domain_pack_lifecycle(&root).expect("recover final remove");
    assert_eq!(recovered.projection().ledger_records.len(), 4);
    assert_eq!(
        recovered
            .projection()
            .active_pointer
            .as_ref()
            .map(|pointer| &pointer.domain_pack_active_pointer),
        Some(&remove_state)
    );
    assert!(matches!(
        recovered.projection().ledger_records[3].operation,
        DomainPackLifecycleOperation::Remove { .. }
    ));
    assert!(recovered
        .projection()
        .active_lock
        .as_ref()
        .unwrap()
        .domain_pack_exact_lock
        .payload
        .packages
        .is_empty());
    let admitted = recovered
        .admit_active_generation()
        .expect("admit final degraded remove generation");
    let view = admitted.verified_view().expect("revalidate final remove");
    assert!(view.is_degraded_empty());
    assert!(!view.degraded_gaps().is_empty());
    drop(admitted);

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
#[allow(clippy::too_many_lines)] // One cohesive adversarial history/rollback scenario.
fn rollback_replays_exact_history_without_generation_collision_and_rejects_orphan_receipt() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("exact-rollback");
    let (install, base, inputs) = commit_integrated_install(&root, &fixture, &raw);
    let original_generation = generation_directories(&root)
        .into_iter()
        .next()
        .expect("install generation");
    let original_lock = fs::read(original_generation.join("lock.yaml")).expect("install lock");
    let original_preflight =
        fs::read(original_generation.join("preflight.yaml")).expect("install preflight");
    let target_receipt_digest = install.domain_pack_lifecycle_receipt.receipt_digest.clone();
    let target_lock_digest = install
        .domain_pack_lifecycle_receipt
        .to_state
        .active_lock_digest
        .clone();

    commit_integrated_remove(&root, &fixture, &inputs);
    let rollback = attempt_variant(
        &root,
        &fixture,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: target_receipt_digest.clone(),
            target_lock_digest: target_lock_digest.clone(),
        },
        "rollback-exact",
    )
    .expect("exact reachable historical rollback commits");
    assert_eq!(
        rollback
            .domain_pack_lifecycle_receipt
            .to_state
            .active_lock_digest,
        target_lock_digest
    );

    let generations = generation_directories(&root);
    assert_eq!(
        generations.len(),
        3,
        "remove plus exact rollback create unique immutable generations"
    );
    assert_eq!(
        fs::read(original_generation.join("lock.yaml")).expect("immutable old lock"),
        original_lock
    );
    assert_eq!(
        fs::read(original_generation.join("preflight.yaml")).expect("immutable old preflight"),
        original_preflight
    );

    // Keep every ledger-bound field reachable and internally consistent, but
    // mint a different pointer and receipt. A digest-valid receipt-shaped file
    // is not historical authority unless it is byte-for-byte the receipt in
    // the immutable generation envelope.
    let mut synthetic = install.clone();
    let synthetic_body = &mut synthetic.domain_pack_lifecycle_receipt;
    synthetic_body.receipt_id = id("domain-pack.lifecycle.receipt.synthetic-reachable");
    synthetic_body.to_state.project_id = id("project.domain-pack.forged-pointer");
    synthetic_body.to_state.pointer_digest.clear();
    synthetic_body.to_state.pointer_digest = canonical_digest(&synthetic_body.to_state);
    synthetic_body.receipt_digest.clear();
    synthetic_body.receipt_digest = canonical_digest(synthetic_body);
    let synthetic_digest = synthetic_body.receipt_digest.clone();
    let synthetic_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        synthetic_digest.trim_start_matches("sha256:")
    ));
    fs::write(
        &synthetic_path,
        yaml_serde::to_string(&synthetic).expect("serialize self-consistent synthetic receipt"),
    )
    .expect("publish self-consistent synthetic receipt");

    let error = attempt_variant(
        &root,
        &fixture,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: synthetic_digest,
            target_lock_digest: target_lock_digest.clone(),
        },
        "rollback-synthetic-reachable",
    )
    .expect_err("reachable record cannot bless a receipt absent from its immutable generation");
    match error {
        DomainPackLifecycleStoreError::PreflightBlocked { reason } => assert!(
            reason.contains("exact immutable historical generation"),
            "unexpected rejection boundary: {reason}"
        ),
        other => panic!("unexpected synthetic receipt error: {other}"),
    }

    let mut orphan = install.clone();
    let orphan_body = &mut orphan.domain_pack_lifecycle_receipt;
    orphan_body.receipt_id = id("domain-pack.lifecycle.receipt.orphan");
    orphan_body.new_ledger_head_digest = digest('e');
    orphan_body.to_state.lifecycle_head_digest = digest('e');
    orphan_body.receipt_digest.clear();
    orphan_body.receipt_digest = canonical_digest(orphan_body);
    let orphan_digest = orphan_body.receipt_digest.clone();
    let receipt_path = root.join("domain-packs/receipts").join(format!(
        "{}.yaml",
        orphan_digest.trim_start_matches("sha256:")
    ));
    fs::write(
        &receipt_path,
        yaml_serde::to_string(&orphan).expect("serialize orphan receipt"),
    )
    .expect("publish orphan receipt-shaped object");

    let error = attempt_variant(
        &root,
        &fixture,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: orphan_digest,
            target_lock_digest,
        },
        "rollback-orphan",
    )
    .expect_err("receipt-shaped file outside reachable ledger cannot authorize rollback");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn rollback_to_exact_historical_empty_lock_is_vacuously_reviewed() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("rollback-empty-lock");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    let (remove, empty_preflight, empty_inputs) =
        commit_integrated_remove_with_preflight(&root, &fixture, &install_inputs);
    let target_receipt_digest = remove.domain_pack_lifecycle_receipt.receipt_digest.clone();
    let target_lock_digest = remove
        .domain_pack_lifecycle_receipt
        .to_state
        .active_lock_digest
        .clone();

    commit_integrated_reinstall(&root, &fixture, &raw);
    let rollback = attempt_empty_variant(
        &root,
        &fixture,
        &empty_preflight,
        &empty_inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest,
            target_lock_digest: target_lock_digest.clone(),
        },
        "rollback-empty-exact",
    )
    .expect("exact historical remove-last lock is vacuously reviewed eligible");
    assert_eq!(
        rollback
            .domain_pack_lifecycle_receipt
            .to_state
            .active_lock_digest,
        target_lock_digest
    );
    let locked = lock_domain_pack_lifecycle(&root).expect("load rolled-back empty lock");
    assert!(locked
        .projection()
        .active_lock
        .as_ref()
        .expect("active empty lock")
        .domain_pack_exact_lock
        .payload
        .packages
        .is_empty());
    let admitted = locked
        .admit_active_generation()
        .expect("historical empty rollback is an admitted degraded generation");
    let view = admitted.verified_view().expect("revalidate empty rollback");
    assert!(view.is_degraded_empty());
    assert!(!view.degraded_gaps().is_empty());
    drop(admitted);

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn dual_axis_review_is_mandatory_and_exactly_bound_to_supply_chain_artifacts() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("dual-axis-adversarial");
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let locked = lock_domain_pack_lifecycle(&root).expect("lock fresh lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let material = raw.material(&fixture);
    let (base, inputs) = integrated_install_preflight(&fixture, expected, &material);

    let without_review = fixture.with_reviewed_registry(|registry| registry.entries.clear());
    let no_review = rebind_preflight_to_reviewed_registry(
        base.clone(),
        &inputs.resolution_request,
        &without_review,
    );
    let prepared = locked
        .prepare_candidate(no_review)
        .expect("structurally prepare supply-only candidate");
    let error = authorize_integrated(project_root, &without_review, &raw, &inputs, &prepared)
        .expect_err("supply-chain verification without reviewed semantics must fail closed");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    let mismatched_supply = fixture.with_reviewed_registry(|registry| {
        registry.entries[0].supply_chain_record_digest = digest('e');
    });
    let mismatched = rebind_preflight_to_reviewed_registry(
        base.clone(),
        &inputs.resolution_request,
        &mismatched_supply,
    );
    let prepared = locked
        .prepare_candidate(mismatched)
        .expect("structurally prepare mismatched reviewed record");
    let error = authorize_integrated(project_root, &mismatched_supply, &raw, &inputs, &prepared)
        .expect_err("review cannot substitute for the exact supply-chain record");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    let mismatched_fixture = fixture.with_reviewed_registry(|registry| {
        registry.entries[0].fixture_digests[0] = digest('f');
    });
    let mismatched = rebind_preflight_to_reviewed_registry(
        base,
        &inputs.resolution_request,
        &mismatched_fixture,
    );
    let prepared = locked
        .prepare_candidate(mismatched)
        .expect("structurally prepare mismatched fixture review");
    let error = authorize_integrated(project_root, &mismatched_fixture, &raw, &inputs, &prepared)
        .expect_err("review without exact fixture binding must fail closed");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    drop(locked);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn revoked_review_blocks_rollback_but_does_not_trap_removal() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let revoked = fixture.with_reviewed_registry(|registry| {
        let entry = &mut registry.entries[0];
        entry.stage = DomainPackPromotionStage::Revoked;
        entry.eligibility = DomainPackReviewedEligibility::IneligibleRevoked;
        entry.revocation = Some(DomainPackRevocationBinding {
            reason: "adversarial revocation".to_owned(),
            effective_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_secs(),
            authorization_digest: learning_digest("revocation-authorization"),
        });
    });
    let root = temp_state_root("revoked-lifecycle");
    let (install, base, inputs) = commit_integrated_install(&root, &fixture, &raw);
    let target_receipt_digest = install.domain_pack_lifecycle_receipt.receipt_digest.clone();
    let target_lock_digest = install
        .domain_pack_lifecycle_receipt
        .to_state
        .active_lock_digest
        .clone();

    commit_integrated_remove(&root, &revoked, &inputs);
    let error = attempt_variant(
        &root,
        &revoked,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest,
            target_lock_digest,
        },
        "rollback-revoked",
    )
    .expect_err("a revoked package cannot be reactivated by historical rollback evidence");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn current_supply_chain_revocation_allows_removal_but_blocks_reactivation_and_rollback() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let revoked = fixture.with_current_supply_chain_revocation();
    let root = temp_state_root("current-supply-chain-revocation");
    let (install, base, inputs) = commit_integrated_install(&root, &fixture, &raw);
    let target_receipt_digest = install.domain_pack_lifecycle_receipt.receipt_digest.clone();
    let target_lock_digest = install
        .domain_pack_lifecycle_receipt
        .to_state
        .active_lock_digest
        .clone();

    // A current supply-chain revocation must never trap an already-active package:
    // removal commits an empty current lock while preserving historical evidence.
    commit_integrated_remove(&root, &revoked, &inputs);

    let error = attempt_variant(
        &root,
        &revoked,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Install {
            root: coordinate(&fixture),
        },
        "reinstall-current-supply-chain-revoked",
    )
    .expect_err("current cumulative supply-chain revocation blocks reactivation");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    let error = attempt_variant(
        &root,
        &revoked,
        &raw,
        &base,
        &inputs,
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest,
            target_lock_digest,
        },
        "rollback-current-supply-chain-revoked",
    )
    .expect_err("current supply-chain revocation blocks historical rollback reactivation");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn persisted_generation_object_and_crosslink_tamper_block_state_load() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);

    let generation_root = temp_state_root("tamper-generation");
    commit_integrated_install(&generation_root, &fixture, &raw);
    let generation_path = generation_directories(&generation_root)[0].join("generation.yaml");
    let mut generation_bytes = fs::read(&generation_path).expect("generation manifest");
    generation_bytes.extend_from_slice(b"forged_field: true\n");
    fs::write(&generation_path, generation_bytes).expect("tamper generation manifest");
    lock_domain_pack_lifecycle(&generation_root)
        .expect_err("tampered generation manifest must block active load");
    fs::remove_dir_all(generation_root.parent().expect("project root")).expect("cleanup");

    let object_root = temp_state_root("tamper-object");
    commit_integrated_install(&object_root, &fixture, &raw);
    let object_path = fs::read_dir(object_root.join("domain-packs/objects"))
        .expect("object store")
        .next()
        .expect("persisted object")
        .expect("object entry")
        .path();
    let mut object_bytes = fs::read(&object_path).expect("object bytes");
    object_bytes.push(b'!');
    fs::write(&object_path, object_bytes).expect("tamper object bytes");
    lock_domain_pack_lifecycle(&object_root)
        .expect_err("tampered immutable object must block active load");
    fs::remove_dir_all(object_root.parent().expect("project root")).expect("cleanup");

    let crosslink_root = temp_state_root("tamper-crosslink");
    let (receipt, _, _) = commit_integrated_install(&crosslink_root, &fixture, &raw);
    let receipt_path = generation_directories(&crosslink_root)[0].join("receipt.yaml");
    let receipt_text = fs::read_to_string(&receipt_path).expect("generation receipt");
    let head = &receipt.domain_pack_lifecycle_receipt.new_ledger_head_digest;
    let crosslinked = receipt_text.replace(head, &digest('d'));
    assert_ne!(
        crosslinked, receipt_text,
        "receipt contains ledger crosslink"
    );
    fs::write(&receipt_path, crosslinked).expect("tamper receipt crosslink");
    lock_domain_pack_lifecycle(&crosslink_root)
        .expect_err("tampered generation crosslink must block active load");
    fs::remove_dir_all(crosslink_root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn directory_links_in_generations_and_objects_block_publication_before_pointer_flip() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);

    for leaf in ["generations", "objects"] {
        let root = temp_state_root(&format!("linked-{leaf}"));
        let project_root = root.parent().expect("project root");
        let project_digest = current_project_snapshot_digest(project_root);
        let mut locked = lock_domain_pack_lifecycle(&root).expect("lock fresh lifecycle");
        let expected = expected_from_projection(locked.projection(), &project_digest);
        let material = raw.material(&fixture);
        let (document, inputs) = integrated_install_preflight(&fixture, expected, &material);
        let prepared = locked
            .prepare_candidate(document)
            .expect("prepare integrated install");
        let authority = authorize_integrated(project_root, &fixture, &raw, &inputs, &prepared)
            .expect("authorize before adversarial link");

        let link = root.join("domain-packs").join(leaf);
        fs::create_dir_all(link.parent().expect("link parent")).expect("state layout parent");
        let outside = project_root.with_extension(format!("outside-{leaf}"));
        fs::create_dir_all(&outside).expect("outside link target");
        create_directory_link(&link, &outside);

        locked
            .commit(prepared, authority)
            .expect_err("directory link must block immutable publication");
        assert!(
            !root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH).exists(),
            "failed publication never flips the active pointer"
        );
        assert_eq!(
            fs::read_dir(&outside).expect("outside target").count(),
            0,
            "TCB never writes through the adversarial directory link"
        );

        remove_directory_link(&link);
        fs::remove_dir_all(&outside).expect("cleanup outside target");
        drop(locked);
        fs::remove_dir_all(project_root).expect("cleanup project");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One end-to-end tamper matrix shares expensive signed setup.
fn non_empty_raw_sidecar_install_recomputes_every_boundary_and_blocks_tamper() {
    let fixture = Fixture::new();
    let root = temp_state_root("integrated-non-empty");
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let manifest_raw =
        fs::read(repo_root().join("docs/fixtures/domain-pack-v0/manifests/foundation.yaml"))
            .expect("raw manifest");
    let content_raw =
        fs::read(repo_root().join("docs/fixtures/domain-pack-v0/content/foundation.yaml"))
            .expect("raw content");
    let license_raw =
        fs::read(repo_root().join("docs/fixtures/domain-pack-v0/artifacts/license-notice.yaml"))
            .expect("raw license");
    let fixture_raw = fs::read(
        repo_root().join("docs/fixtures/domain-pack-v0/artifacts/foundation-representative.yaml"),
    )
    .expect("raw representative fixture");
    let content_artifact_binding = DomainPackArtifactBinding {
        artifact_ref: fixture.resolved.package.content.content_ref.clone(),
        raw_sha256: fixture.resolved.package.content.raw_sha256.clone(),
        canonical_sha256: fixture.resolved.package.content.canonical_sha256.clone(),
    };
    let immutable_artifacts = [
        DomainPackImmutableArtifact {
            binding: &fixture.resolved.package.manifest,
            raw_bytes: &manifest_raw,
        },
        DomainPackImmutableArtifact {
            binding: &content_artifact_binding,
            raw_bytes: &content_raw,
        },
        DomainPackImmutableArtifact {
            binding: &fixture.resolved.package.license,
            raw_bytes: &license_raw,
        },
        DomainPackImmutableArtifact {
            binding: &fixture.resolved.package.fixtures[0],
            raw_bytes: &fixture_raw,
        },
    ];
    let material = DomainPackCandidateMaterial {
        publisher: &fixture.resolved.identity.publisher.0,
        name: &fixture.resolved.identity.name.0,
        version: &fixture.resolved.identity.version,
        manifest_raw: &manifest_raw,
        content_raw: &content_raw,
        license_raw: &license_raw,
    };
    let mut locked = lock_domain_pack_lifecycle(&root).expect("lock fresh lifecycle");
    let expected = expected_from_projection(locked.projection(), &project_digest);
    let (document, inputs) = integrated_install_preflight(&fixture, expected, &material);
    let prepared = locked
        .prepare_candidate(document.clone())
        .expect("prepare integrated install");

    let mut tampered_staging = document;
    tampered_staging
        .domain_pack_lifecycle_preflight
        .staged_artifacts
        .pop();
    tampered_staging
        .domain_pack_lifecycle_preflight
        .preflight_digest
        .clear();
    tampered_staging
        .domain_pack_lifecycle_preflight
        .preflight_digest = canonical_digest(&tampered_staging);
    let staged_prepared = locked
        .prepare_candidate(tampered_staging)
        .expect("structurally prepare staged-binding tamper");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let verified =
        verify_domain_pack_supply_chain_snapshot(&inputs.trust_policy, &fixture.snapshot, now)
            .expect("fresh integrated supply-chain proof");
    let anchored = fixture.anchored(verified);
    let reviewed_anchored = fixture.reviewed_anchored(now);
    let project_snapshot = verify_domain_pack_project_snapshot(project_root, &project_digest)
        .expect("fresh project snapshot");
    let valid_materials = [material];

    let mut tampered_content = content_raw.clone();
    tampered_content.push(b'\n');
    let tampered_material = DomainPackCandidateMaterial {
        content_raw: &tampered_content,
        ..material
    };
    let tampered_materials = [tampered_material];
    let raw_error = authorize_prepared_domain_pack_lifecycle(
        &prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
            anchored_reviewed_snapshot: &reviewed_anchored,
            project_snapshot: &project_snapshot,
            trust_policy_document: &inputs.trust_policy,
            registry_document: &fixture.snapshot,
            resolution_request: &inputs.resolution_request,
            composition_request: &inputs.composition_request,
            materials: &tampered_materials,
            artifacts: &immutable_artifacts,
            trust_input: &inputs.trust_input,
        },
    )
    .expect_err("raw content tamper cannot mint commit authority");
    assert!(matches!(
        raw_error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    let staging_error = authorize_prepared_domain_pack_lifecycle(
        &staged_prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
            anchored_reviewed_snapshot: &reviewed_anchored,
            project_snapshot: &project_snapshot,
            trust_policy_document: &inputs.trust_policy,
            registry_document: &fixture.snapshot,
            resolution_request: &inputs.resolution_request,
            composition_request: &inputs.composition_request,
            materials: &valid_materials,
            artifacts: &immutable_artifacts,
            trust_input: &inputs.trust_input,
        },
    )
    .expect_err("incomplete staged artifact set cannot mint authority");
    assert!(matches!(
        staging_error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));

    let authority = authorize_prepared_domain_pack_lifecycle(
        &prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
            anchored_reviewed_snapshot: &reviewed_anchored,
            project_snapshot: &project_snapshot,
            trust_policy_document: &inputs.trust_policy,
            registry_document: &fixture.snapshot,
            resolution_request: &inputs.resolution_request,
            composition_request: &inputs.composition_request,
            materials: &valid_materials,
            artifacts: &immutable_artifacts,
            trust_input: &inputs.trust_input,
        },
    )
    .expect("exact raw sidecars and staged bindings authorize");
    let receipt = locked
        .commit(prepared, authority)
        .expect("commit non-empty integrated install");
    assert_eq!(receipt.domain_pack_lifecycle_receipt.to_state.generation, 0);
    assert_eq!(
        locked
            .projection()
            .active_lock
            .as_ref()
            .expect("active integrated lock")
            .domain_pack_exact_lock
            .payload
            .packages
            .len(),
        1
    );
    for artifact in &immutable_artifacts {
        let path = root
            .join("domain-packs/objects")
            .join(artifact.binding.raw_sha256.trim_start_matches("sha256:"));
        assert_eq!(
            fs::read(path).expect("persisted immutable raw object"),
            artifact.raw_bytes,
            "object store preserves the exact admitted bytes"
        );
    }
    drop(locked);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn active_generation_admission_exposes_only_exact_composed_inputs_under_retained_lock() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("admitted-active-generation");
    let (_, preflight, _) = commit_integrated_install(&root, &fixture, &raw);
    let expected = &preflight.domain_pack_lifecycle_preflight;
    let expected_lock = &expected.proposed_lock.domain_pack_exact_lock;
    let expected_composition = &expected.composition.domain_pack_composition_projection;

    let admitted = lock_domain_pack_lifecycle(&root)
        .expect("lock committed lifecycle")
        .admit_active_generation()
        .expect("admit exact active generation");
    {
        let view = admitted
            .verified_view()
            .expect("freshly revalidate admitted generation");
        assert_eq!(view.generation_id(), 0);
        assert_eq!(view.lock_digest(), expected_lock.lock_digest);
        assert_eq!(
            view.base_core_bundle_digest(),
            expected_composition.core_bundle_digest
        );
        assert_eq!(
            view.composition_digest(),
            expected_composition.composition_digest
        );
        assert_eq!(
            view.supply_chain_registry_digest(),
            expected_lock.payload.registry_snapshot_digest
        );
        assert_eq!(
            view.reviewer_registry_digest(),
            expected_lock.payload.reviewer_registry_digest
        );
        assert_eq!(
            view.reviewed_registry_digest(),
            expected_lock.payload.reviewed_registry_digest
        );
        assert_eq!(
            view.active_package_identities(),
            expected_composition.ordered_packs
        );
        assert_eq!(
            view.effective_bundle(),
            expected_composition
                .composed_bundle
                .as_ref()
                .expect("committed composition bundle")
        );
    }
    lock_domain_pack_lifecycle(&root)
        .expect_err("admitted generation must retain the lifecycle OS lock");

    drop(admitted);
    lock_domain_pack_lifecycle(&root).expect("dropping admission releases lifecycle lock");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn governed_remove_to_empty_is_typed_degraded_and_retains_blocking_gaps() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("admitted-degraded-empty-generation");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    let (_, remove_preflight, _) =
        commit_integrated_remove_with_preflight(&root, &fixture, &install_inputs);
    let expected = &remove_preflight
        .domain_pack_lifecycle_preflight
        .composition
        .domain_pack_composition_projection;
    assert_eq!(expected.status, DomainPackCompositionStatus::Blocked);
    assert!(!expected.gaps.is_empty());
    assert!(expected.ordered_packs.is_empty());

    let admitted = lock_domain_pack_lifecycle(&root)
        .expect("lock governed empty generation")
        .admit_active_generation()
        .expect("admit exact degraded empty generation");
    let view = admitted
        .verified_view()
        .expect("revalidate degraded empty generation");
    assert!(matches!(
        &view,
        forge_core_domain_pack_tcb::AdmittedActiveDomainPackGenerationView::DegradedEmpty(_)
    ));
    assert!(view.is_degraded_empty());
    assert_eq!(view.degraded_gaps(), expected.gaps);
    assert!(view.active_package_identities().is_empty());
    assert_eq!(
        view.effective_bundle(),
        expected
            .composed_bundle
            .as_ref()
            .expect("blocked remove preserves core-only composed bundle")
    );
    lock_domain_pack_lifecycle(&root).expect_err("degraded admission retains lifecycle lock");

    drop(admitted);
    lock_domain_pack_lifecycle(&root).expect("dropping degraded admission releases lock");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn tampered_pointer_generation_and_preflight_cannot_mint_active_admission() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);

    let pointer_root = temp_state_root("admission-tampered-pointer");
    commit_integrated_install(&pointer_root, &fixture, &raw);
    let pointer_path = pointer_root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);
    let mut pointer_raw = fs::read(&pointer_path).expect("active pointer");
    pointer_raw.extend_from_slice(b"forged_field: true\n");
    fs::write(&pointer_path, pointer_raw).expect("tamper active pointer");
    lock_domain_pack_lifecycle(&pointer_root)
        .expect_err("caller-authored pointer cannot mint active admission");
    fs::remove_dir_all(pointer_root.parent().expect("project root")).expect("cleanup");

    let generation_root = temp_state_root("admission-tampered-generation");
    commit_integrated_install(&generation_root, &fixture, &raw);
    let generation_path = generation_directories(&generation_root)[0].join("generation.yaml");
    let mut generation_raw = fs::read(&generation_path).expect("generation manifest");
    generation_raw.extend_from_slice(b"forged_field: true\n");
    fs::write(&generation_path, generation_raw).expect("tamper generation manifest");
    lock_domain_pack_lifecycle(&generation_root)
        .expect_err("caller-authored generation cannot mint active admission");
    fs::remove_dir_all(generation_root.parent().expect("project root")).expect("cleanup");

    let preflight_root = temp_state_root("admission-tampered-preflight");
    commit_integrated_install(&preflight_root, &fixture, &raw);
    let preflight_path = generation_directories(&preflight_root)[0].join("preflight.yaml");
    let mut preflight_raw = fs::read(&preflight_path).expect("generation preflight");
    preflight_raw.extend_from_slice(b"forged_field: true\n");
    fs::write(&preflight_path, preflight_raw).expect("tamper generation preflight");
    lock_domain_pack_lifecycle(&preflight_root)
        .expect_err("caller-authored preflight cannot mint active admission");
    fs::remove_dir_all(preflight_root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn admitted_generation_revalidation_blocks_stale_state_and_uninitialized_state() {
    let empty_root = temp_state_root("admission-no-active-generation");
    let error = lock_domain_pack_lifecycle(&empty_root)
        .expect("lock empty lifecycle")
        .admit_active_generation()
        .expect_err("uninitialized lifecycle has no active generation");
    assert!(matches!(
        error,
        DomainPackLifecycleStoreError::PreflightBlocked { .. }
    ));
    fs::remove_dir_all(empty_root.parent().expect("project root")).expect("cleanup");

    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("admission-stale-after-mint");
    commit_integrated_install(&root, &fixture, &raw);
    let admitted = lock_domain_pack_lifecycle(&root)
        .expect("lock committed lifecycle")
        .admit_active_generation()
        .expect("mint active admission");
    admitted
        .verified_view()
        .expect("active generation starts fresh");

    let preflight_path = generation_directories(&root)[0].join("preflight.yaml");
    let mut preflight_raw = fs::read(&preflight_path).expect("generation preflight");
    preflight_raw.extend_from_slice(b"forged_field: true\n");
    fs::write(&preflight_path, preflight_raw).expect("simulate non-cooperative state change");
    admitted
        .verified_view()
        .expect_err("stale admitted capability must fail closed before consumer join");

    drop(admitted);
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn retained_guard_projects_complete_exact_raw_lifecycle_inventory_without_relocking() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("raw-lifecycle-inventory");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);

    let lifecycle = lock_domain_pack_lifecycle(&root).expect("retain producer lock");
    let inventory = lifecycle.raw_inventory().expect("capture raw inventory");
    let paths = inventory
        .files()
        .iter()
        .map(forge_core_domain_pack_tcb::DomainPackRawLifecycleFile::relative_path)
        .collect::<Vec<_>>();
    assert!(paths.contains(&DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH));
    assert!(paths.iter().any(|path| path.contains("/ledger/")));
    assert!(paths.iter().any(|path| path.contains("/generations/")));
    assert!(paths.iter().any(|path| path.contains("/receipts/")));
    assert!(paths.iter().any(|path| path.contains("/objects/")));
    assert_eq!(
        paths
            .iter()
            .filter(|path| path.ends_with("/generation.yaml"))
            .count(),
        2
    );
    assert_eq!(
        paths
            .iter()
            .filter(|path| path.contains("/ledger/"))
            .count(),
        2
    );
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    for file in inventory.files() {
        assert_eq!(
            file.raw_bytes(),
            fs::read(root.join(file.relative_path()))
                .expect("inventory path exists")
                .as_slice(),
            "{} must preserve exact bytes",
            file.relative_path()
        );
    }
    lock_domain_pack_lifecycle(&root)
        .expect_err("raw inventory owner retains the exact producer lock");

    drop(lifecycle);
    lock_domain_pack_lifecycle(&root).expect("dropping owner releases lifecycle lock");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}
#[test]
fn historical_inventory_rejects_manifest_object_omission_after_two_generations() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("historical-object-omission");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);

    let manifest_path = generation_directories(&root)[0].join("generation.yaml");
    let text = fs::read_to_string(&manifest_path).expect("historical manifest");
    let mut manifest: serde_json::Value = yaml_serde::from_str(&text).expect("parse manifest");
    let objects = manifest
        .get_mut("object_raw_digests")
        .and_then(serde_json::Value::as_array_mut)
        .expect("object digest array");
    assert!(
        objects.pop().is_some(),
        "install generation references objects"
    );
    fs::write(
        &manifest_path,
        yaml_serde::to_string(&manifest).expect("encode omitted manifest"),
    )
    .expect("omit historical object digest");

    let lifecycle =
        lock_domain_pack_lifecycle(&root).expect("active generation remains independently valid");
    lifecycle
        .raw_inventory()
        .expect_err("historical manifest cannot redefine the complete object closure");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn historical_inventory_rejects_omitted_replay_input() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("historical-replay-input-omission");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);
    fs::remove_file(generation_directories(&root)[0].join("resolution-request.yaml"))
        .expect("omit retained replay input");

    lock_domain_pack_lifecycle(&root)
        .expect("active generation remains valid")
        .raw_inventory()
        .expect_err("historical authority requires complete deterministic replay inputs");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}
#[test]
fn historical_inventory_rejects_receipt_state_discontinuity_after_rehash() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("historical-receipt-continuity");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);
    commit_integrated_reinstall(&root, &fixture, &raw);

    let generation = generation_directories(&root)[1].clone();
    let receipt_path = generation.join("receipt.yaml");
    let mut receipt: DomainPackLifecycleReceiptDocument =
        yaml_serde::from_str(&fs::read_to_string(&receipt_path).expect("historical receipt"))
            .expect("parse receipt");
    let body = &mut receipt.domain_pack_lifecycle_receipt;
    let old_digest = body.receipt_digest.clone();
    body.from_state = None;
    body.receipt_digest.clear();
    body.receipt_digest = canonical_digest(body);
    let new_digest = body.receipt_digest.clone();
    fs::write(
        &receipt_path,
        yaml_serde::to_string(&receipt).expect("serialize rehashed receipt"),
    )
    .expect("write rehashed receipt");

    let manifest_path = generation.join("generation.yaml");
    let mut manifest: serde_json::Value =
        yaml_serde::from_str(&fs::read_to_string(&manifest_path).expect("manifest"))
            .expect("parse manifest");
    manifest["receipt_digest"] = serde_json::Value::String(new_digest.clone());
    fs::write(
        &manifest_path,
        yaml_serde::to_string(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let receipts = root.join("domain-packs/receipts");
    fs::remove_file(receipts.join(format!("{}.yaml", old_digest.trim_start_matches("sha256:"))))
        .expect("remove old committed receipt");
    fs::write(
        receipts.join(format!("{}.yaml", new_digest.trim_start_matches("sha256:"))),
        yaml_serde::to_string(&receipt).expect("serialize committed receipt"),
    )
    .expect("write rehashed committed receipt");

    lock_domain_pack_lifecycle(&root)
        .expect("active generation remains valid")
        .raw_inventory()
        .expect_err("a self-consistent receipt cannot sever prior to-state continuity");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}
#[test]
fn historical_inventory_rejects_every_corrupt_authority_sidecar() {
    for sidecar in ["lock.yaml", "preflight.yaml", "compatibility.yaml"] {
        let fixture = Fixture::new();
        let raw = RawArtifactFixture::new(&fixture);
        let root = temp_state_root(&format!("historical-corrupt-{sidecar}"));
        let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
        commit_integrated_remove(&root, &fixture, &install_inputs);

        let path = generation_directories(&root)[0].join(sidecar);
        let mut bytes = fs::read(&path).expect("historical sidecar");
        bytes.extend_from_slice(b"forged_field: true\n");
        fs::write(&path, bytes).expect("corrupt historical sidecar");

        let lifecycle = lock_domain_pack_lifecycle(&root)
            .expect("active generation remains independently valid");
        lifecycle
            .raw_inventory()
            .expect_err("every historical authority sidecar must be parsed and bound");
        fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn historical_inventory_rejects_field_by_field_derivation_input_tampering() {
    let cases = [
        (
            "catalog.yaml",
            "/schema_version",
            serde_json::json!("forge.domain-pack.acquisition.v999"),
        ),
        (
            "catalog.yaml",
            "/forge_core_version",
            serde_json::json!("99.0.0"),
        ),
        (
            "catalog.yaml",
            "/registry/domain_pack_supply_chain_registry/registry_version",
            serde_json::json!("999.0.0"),
        ),
        (
            "composition-request.yaml",
            "/domain_pack_composition_request/request_id",
            serde_json::json!("forged-request"),
        ),
        (
            "composition-request.yaml",
            "/domain_pack_composition_request/authority",
            serde_json::json!("forged_authority"),
        ),
        (
            "composition-request.yaml",
            "/domain_pack_composition_request/forge_core_version",
            serde_json::json!("99.0.0"),
        ),
        (
            "composition-request.yaml",
            "/domain_pack_composition_request/core/bundle_digest",
            serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        (
            "composition-request.yaml",
            "/domain_pack_composition_request/candidates/0/manifest_binding/raw_sha256",
            serde_json::json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        (
            "trust-input.yaml",
            "/project_id",
            serde_json::json!("forged-project"),
        ),
        (
            "trust-input.yaml",
            "/selected/0/structurally_valid",
            serde_json::json!(false),
        ),
        (
            "trust-input.yaml",
            "/selected/0/package/registry_record_digest",
            serde_json::json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"),
        ),
        (
            "trust-input.yaml",
            "/trust_policy/policy_id",
            serde_json::json!("forged-policy"),
        ),
        (
            "trust-input.yaml",
            "/capability_registry/registry_id",
            serde_json::json!("forged-registry"),
        ),
        (
            "trust-input.yaml",
            "/capability_registry/bindings/0/implementation_digest",
            serde_json::json!("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"),
        ),
        (
            "trust-input.yaml",
            "/selected/0/capability_demands/0/capability_ref",
            serde_json::json!("forged-capability"),
        ),
        (
            "trust-input.yaml",
            "/sandbox_policy/policy_id",
            serde_json::json!("forged-sandbox"),
        ),
    ];
    for (index, (sidecar, pointer, replacement)) in cases.into_iter().enumerate() {
        let fixture = Fixture::new();
        let raw = RawArtifactFixture::new(&fixture);
        let root = temp_state_root(&format!("historical-rebase-field-{index}"));
        let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
        commit_integrated_remove(&root, &fixture, &install_inputs);

        let path = generation_directories(&root)[0].join(sidecar);
        let text = fs::read_to_string(&path).expect("historical rebase sidecar");
        let mut value: serde_json::Value = yaml_serde::from_str(&text).expect("parse sidecar");
        *value
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("fixture field {pointer} must exist")) = replacement;
        fs::write(
            &path,
            yaml_serde::to_string(&value).expect("serialize tampered sidecar"),
        )
        .expect("write tampered sidecar");

        lock_domain_pack_lifecycle(&root)
            .expect("active generation remains independently valid")
            .raw_inventory()
            .expect_err("every historical rebase input field must remain cross-bound");
        fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
    }
}

#[cfg(unix)]
#[test]
fn historical_inventory_rejects_state_root_rename_and_replacement() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("raw-lifecycle-root-swap");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);
    let replacement_root = temp_state_root("raw-lifecycle-root-replacement");
    commit_integrated_install(&replacement_root, &fixture, &raw);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("pin original lifecycle root");
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).expect("rename guarded lifecycle root");
    fs::rename(&replacement_root, &root).expect("install well-formed replacement lifecycle root");

    lifecycle
        .raw_inventory()
        .expect_err("inventory must reject a replacement root outside the retained lock namespace");
    drop(lifecycle);
    fs::remove_dir_all(root.parent().expect("original project root")).expect("cleanup original");
    fs::remove_dir_all(replacement_root.parent().expect("replacement project root"))
        .expect("cleanup replacement project");
}

#[cfg(unix)]
#[test]
fn historical_inventory_rejects_rename_to_symlink_namespace_substitution() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("historical-sidecar-symlink");
    let (_, _, install_inputs) = commit_integrated_install(&root, &fixture, &raw);
    commit_integrated_remove(&root, &fixture, &install_inputs);

    let lock_path = generation_directories(&root)[0].join("lock.yaml");
    let moved = lock_path.with_extension("yaml.moved");
    fs::rename(&lock_path, &moved).expect("rename checked sidecar");
    std::os::unix::fs::symlink(&moved, &lock_path).expect("substitute symlink");

    let lifecycle =
        lock_domain_pack_lifecycle(&root).expect("active generation remains independently valid");
    lifecycle
        .raw_inventory()
        .expect_err("no-follow snapshot must reject renamed symlink substitution");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}
#[test]
fn missing_active_pointer_fails_closed_when_lifecycle_residue_exists() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("missing-active-pointer-residue");
    commit_integrated_install(&root, &fixture, &raw);
    fs::remove_file(root.join("domain-packs/active.lock.yaml")).expect("remove only pointer");

    lock_domain_pack_lifecycle(&root)
        .expect_err("ledger, receipt, generation, and object residue cannot become core-only");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn absent_pointer_residue_aba_is_rejected_without_touching_displaced_namespace() {
    let root = temp_state_root("absent-pointer-residue-aba");
    fs::create_dir_all(root.join("domain-packs")).expect("create empty authority A");
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock empty authority A");
    let authority_a = root.join("domain-packs");
    let snapshot = |directory: &Path| {
        let mut entries = fs::read_dir(directory)
            .expect("read authority A")
            .map(|entry| {
                let entry = entry.expect("authority A entry");
                let path = entry.path();
                (entry.file_name(), fs::read(path).expect("authority A file"))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    };
    let before_displacement = snapshot(&authority_a);
    let displaced = root.join("domain-packs-a");
    fs::rename(&authority_a, &displaced).expect("displace authority A");
    fs::create_dir_all(root.join("domain-packs/generations")).expect("create replacement B");
    fs::write(root.join("domain-packs/generations/residue"), b"hostile-b")
        .expect("write replacement residue");

    lifecycle
        .verified_core_only_view()
        .expect_err("replacement namespace residue cannot be accepted as core-only");
    assert_eq!(
        snapshot(&displaced),
        before_displacement,
        "descriptor-relative validation must not modify displaced authority A"
    );
    assert_eq!(
        fs::read(root.join("domain-packs/generations/residue")).expect("replacement unchanged"),
        b"hostile-b"
    );

    drop(lifecycle);
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}
#[test]
fn verified_core_only_view_borrows_the_retained_lifecycle_lock() {
    let root = temp_state_root("verified-core-only-lifecycle");
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock empty lifecycle");
    let view = lifecycle
        .verified_core_only_view()
        .expect("admit exact core-only observation");
    assert!(format!("{view:?}").contains("AdmittedCoreOnlyDomainPackLifecycleView"));
    lock_domain_pack_lifecycle(&root).expect_err("core-only view owner retains lifecycle lock");

    drop(lifecycle);
    lock_domain_pack_lifecycle(&root).expect("dropping core-only owner releases lock");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
#[allow(clippy::too_many_lines)]
fn independent_completion_selector_binds_exact_record_parent_and_pointer() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("independent-lifecycle-completion");
    let project_root = root.parent().expect("project root");
    fs::write(
        project_root.join("governed-project-input.txt"),
        b"exact governed project bytes",
    )
    .expect("write governed input");

    commit_integrated_install(&root, &fixture, &raw);
    let generation = generation_directories(&root)
        .into_iter()
        .next()
        .expect("committed generation");
    let record_path = generation.join("completion.record");
    let selector_path = generation.join("completion.selector");
    let record_bytes = fs::read(&record_path).expect("canonical completion record");
    let selector_bytes = fs::read(&selector_path).expect("canonical completion selector");
    let record: serde_json::Value =
        serde_json::from_slice(&record_bytes).expect("parse completion record");
    let selector: serde_json::Value =
        serde_json::from_slice(&selector_bytes).expect("parse completion selector");
    assert_eq!(
        selector["record_digest"],
        serde_json::Value::String(format!("sha256:{}", hex(&Sha256::digest(&record_bytes))))
    );
    assert_eq!(
        selector["record_byte_length"],
        serde_json::Value::from(u64::try_from(record_bytes.len()).expect("record length fits u64"))
    );
    for field in ["record_anchor", "parent_anchor"] {
        assert!(selector[field]["anchor_relative_path"]
            .as_str()
            .is_some_and(|path| path.starts_with(".forge-lifecycle-anchors/")));
        assert!(selector[field]["nonce"]
            .as_str()
            .is_some_and(|nonce| nonce.len() == 64));
    }
    assert_eq!(record["previous_pointer"], serde_json::Value::Null);
    assert!(record.get("completion_record_digest").is_none());
    assert!(record.get("completion_file_identity_digest").is_none());
    assert!(record.get("completion_parent_identity_digest").is_none());
    assert_eq!(
        record["operation_nonce"]
            .as_str()
            .expect("operation nonce")
            .len(),
        64
    );
    assert!(record["project_snapshot_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    let pointer_bytes = fs::read(root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH))
        .expect("installed active pointer");
    assert_eq!(
        record["installed_pointer"]["raw_digest"],
        serde_json::Value::String(format!("sha256:{}", hex(&Sha256::digest(&pointer_bytes))))
    );
    assert_eq!(
        record["installed_pointer"]["anchor"]["content_digest"],
        record["installed_pointer"]["raw_digest"]
    );
    assert!(record["materials"]
        .as_array()
        .expect("immutable material inventory")
        .iter()
        .all(|entry| entry["anchor"]["content_digest"] == entry["raw_digest"]));

    let displaced = record_path.with_extension("record.displaced");
    fs::rename(&record_path, &displaced).expect("displace selected completion record");
    fs::write(&record_path, &record_bytes).expect("install byte-identical record replacement");
    lock_domain_pack_lifecycle(&root)
        .expect_err("byte-identical completion record replacement must fail closed");

    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn hidden_or_tampered_completion_selector_fails_closed() {
    for mode in ["hidden", "tampered"] {
        let fixture = Fixture::new();
        let raw = RawArtifactFixture::new(&fixture);
        let root = temp_state_root(&format!("completion-selector-{mode}"));
        commit_integrated_install(&root, &fixture, &raw);
        let selector_path = generation_directories(&root)[0].join("completion.selector");
        if mode == "hidden" {
            fs::remove_file(&selector_path).expect("hide completion selector");
        } else {
            let mut selector = fs::read(&selector_path).expect("completion selector");
            selector.push(b' ');
            fs::write(&selector_path, selector).expect("tamper completion selector");
        }
        lock_domain_pack_lifecycle(&root)
            .expect_err("hidden or non-canonical completion selector must fail closed");
        fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
    }
}

#[test]
fn completion_selector_rollback_and_byte_identical_pointer_replacement_fail_closed() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);

    let rollback_root = temp_state_root("completion-selector-rollback");
    let (_, _, install_inputs) = commit_integrated_install(&rollback_root, &fixture, &raw);
    commit_integrated_remove(&rollback_root, &fixture, &install_inputs);
    let generations = generation_directories(&rollback_root);
    let old_selector =
        fs::read(generations[0].join("completion.selector")).expect("old completion selector");
    let current_selector = generations[1].join("completion.selector");
    fs::rename(
        &current_selector,
        current_selector.with_extension("selector.displaced"),
    )
    .expect("displace current selector");
    fs::write(&current_selector, old_selector).expect("roll back selector bytes");
    lock_domain_pack_lifecycle(&rollback_root)
        .expect_err("an old selector cannot authorize the current active generation");
    fs::remove_dir_all(rollback_root.parent().expect("project root")).expect("cleanup");

    let pointer_root = temp_state_root("completion-pointer-identical-replacement");
    commit_integrated_install(&pointer_root, &fixture, &raw);
    let pointer_path = pointer_root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);
    let pointer_bytes = fs::read(&pointer_path).expect("active pointer bytes");
    fs::rename(&pointer_path, pointer_path.with_extension("yaml.displaced"))
        .expect("displace active pointer");
    fs::write(&pointer_path, pointer_bytes).expect("install byte-identical active pointer");
    lock_domain_pack_lifecycle(&pointer_root)
        .expect_err("byte-identical active-pointer replacement must fail its lifetime anchor");
    fs::remove_dir_all(pointer_root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn retained_completion_guard_rejects_byte_identical_selector_substitution() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("retained-completion-selector-substitution");
    let project_file = root
        .parent()
        .expect("project root")
        .join("retained-project-input.txt");
    fs::write(&project_file, b"retained exact project bytes").expect("write project input");
    commit_integrated_install(&root, &fixture, &raw);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("retain selected completion");
    let selector_path = generation_directories(&root)[0].join("completion.selector");
    let selector_bytes = fs::read(&selector_path).expect("completion selector bytes");
    fs::rename(
        &selector_path,
        selector_path.with_extension("selector.displaced"),
    )
    .expect("displace retained selector");
    fs::write(&selector_path, selector_bytes).expect("install byte-identical selector");

    lifecycle
        .raw_inventory()
        .expect_err("retained selector handle must reject byte-identical substitution");
    drop(lifecycle);
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn lifecycle_absence_revalidation_rejects_create_delete_aba() {
    let root = temp_state_root("lifecycle-absence-aba");
    let lock =
        forge_core_store::acquire_effect_store_lock(&root, "locks/domain-packs.lifecycle.lock")
            .expect("lifecycle effect lock");
    let store = lock
        .into_domain_pack_lifecycle_store()
        .expect("retained lifecycle Store");
    let session = store
        .reconcile_active_pointer(64 * 1024)
        .expect("reconcile absent active pointer");
    let expected = store
        .consume_reconciled_active_pointer(session)
        .expect("transfer exact absence claim");
    let pointer = root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);

    fs::remove_file(&pointer).expect("remove Store absence placeholder");
    fs::write(&pointer, b"transient attacker pointer\n").expect("create transient pointer");
    fs::remove_file(&pointer).expect("delete transient pointer");

    store
        .revalidate_expected_active_pointer(&expected)
        .expect_err("create-delete ABA must invalidate lifecycle absence authority");
    drop(expected);
    drop(store);
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn lifecycle_load_transfers_the_original_reconciliation_session_authority() {
    let source = include_str!("../src/lib.rs");
    let start = source
        .find("fn load_current_state_under_lock(")
        .expect("lifecycle state loader source");
    let end = source[start..]
        .find("fn load_state_under_lock(")
        .map(|offset| start + offset)
        .expect("lifecycle state loader boundary");
    let loader = &source[start..end];
    assert!(loader.contains("store.consume_reconciled_active_pointer(session)?"));
    assert!(!loader.contains("retain_expected_active_pointer"));
    assert!(!loader.contains("read_optional("));
    assert!(!loader.contains("session.read_exact()"));

    let lock_start = source
        .find("pub fn lock_domain_pack_lifecycle(")
        .expect("lifecycle lock source");
    let lock_end = source[lock_start..]
        .find("impl LockedDomainPackLifecycle")
        .map(|offset| lock_start + offset)
        .expect("lifecycle lock boundary");
    let lock = &source[lock_start..lock_end];
    assert!(lock.contains("let (loaded, recovery, active_pointer_authority) ="));
    assert!(!lock.contains("retain_expected_active_pointer"));
    assert!(!lock.contains("take_active_pointer_witness"));

    let store = include_str!("../../forge-core-store/src/retained_lifecycle.rs");
    assert!(store.contains("session.consume_reconciled_leaf("));
    assert!(store.contains("reconciled_binding: Some(reconciled_binding)"));
    let crash = include_str!("../../forge-core-store/src/retained_crash_replace.rs");
    let transfer_start = crash
        .find("pub(crate) fn consume_reconciled_leaf(")
        .expect("crash reconciliation transfer source");
    let transfer_end = crash[transfer_start..]
        .find("/// Consume this session as one exact read.")
        .map(|offset| transfer_start + offset)
        .expect("crash reconciliation transfer boundary");
    let transfer = &crash[transfer_start..transfer_end];
    assert!(transfer.contains("target: self.target"));
    assert!(!transfer.contains("open_leaf_"));
    assert!(!transfer.contains("read_file_"));
}

#[test]
fn fresh_lifecycle_load_rejects_byte_identical_project_replacement() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("fresh-project-identical-replacement");
    let project_file = root
        .parent()
        .expect("project root")
        .join("fresh-project-input.txt");
    fs::write(&project_file, b"fresh exact project bytes").expect("write project input");
    commit_integrated_install(&root, &fixture, &raw);
    fs::rename(&project_file, project_file.with_extension("txt.displaced"))
        .expect("displace committed project file");
    fs::write(&project_file, b"fresh exact project bytes")
        .expect("install byte-identical project replacement");

    lock_domain_pack_lifecycle(&root)
        .expect_err("fresh lifecycle load must reject byte-identical project replacement");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn retained_project_guard_rejects_byte_identical_project_replacement() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("retained-project-identical-replacement");
    let project_file = root
        .parent()
        .expect("project root")
        .join("retained-project-input.txt");
    fs::write(&project_file, b"retained exact project bytes").expect("write project input");
    commit_integrated_install(&root, &fixture, &raw);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("retain exact project tree");
    fs::rename(&project_file, project_file.with_extension("txt.displaced"))
        .expect("displace retained project file");
    fs::write(&project_file, b"retained exact project bytes")
        .expect("install byte-identical project replacement");

    lifecycle
        .raw_inventory()
        .expect_err("retained project capability must reject byte-identical replacement");
    drop(lifecycle);
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn completion_record_tamper_fails_closed() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("completion-record-tamper");
    commit_integrated_install(&root, &fixture, &raw);
    let record_path = generation_directories(&root)[0].join("completion.record");
    let mut record = fs::read(&record_path).expect("completion record");
    record.push(b' ');
    fs::write(&record_path, record).expect("tamper canonical record bytes");

    lock_domain_pack_lifecycle(&root)
        .expect_err("non-canonical completion record must fail closed");
    fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
}

#[test]
fn initialized_candidate_rejects_stale_generation_active_lock_and_lifecycle_head_independently() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("initialized-stale-exact-cas");
    commit_integrated_install(&root, &fixture, &raw);
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock initialized lifecycle");
    let current = expected_from_projection(lifecycle.projection(), &project_digest);
    let DomainPackExpectedLifecycleState::Initialized {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    } = current
    else {
        panic!("installed lifecycle must project initialized state");
    };
    let current = DomainPackInitializedProjectStateBinding {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    };
    let exact_intent = |expected_state| DomainPackInitializedProjectIntentDocument {
        schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
        domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
            intent_id: id("intent.initialized.remove"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            principal_id: id("principal.operator"),
            expected_state,
            operation: DomainPackInitializedProjectOperation::Remove {
                pack: coordinate(&fixture),
            },
        },
    };
    let source = lifecycle
        .initialized_project_source(&exact_intent(current.clone()))
        .expect("exact initialized state derives retained candidate source");
    assert_eq!(source.expected_state, current);
    let stale_cases = [
        (
            "generation",
            DomainPackInitializedProjectStateBinding {
                generation: current.generation + 1,
                ..current.clone()
            },
        ),
        (
            "active-lock",
            DomainPackInitializedProjectStateBinding {
                active_lock_digest: digest('d'),
                ..current.clone()
            },
        ),
        (
            "lifecycle-head",
            DomainPackInitializedProjectStateBinding {
                lifecycle_head_digest: digest('e'),
                ..current.clone()
            },
        ),
    ];

    for (label, stale) in stale_cases {
        let error = lifecycle
            .initialized_project_source(&exact_intent(stale))
            .expect_err("stale initialized CAS must fail before derivation");
        assert!(
            matches!(
                error,
                DomainPackLifecycleStoreError::StaleExpectedState { .. }
            ),
            "{label} drift returned {error}"
        );
    }

    drop(lifecycle);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn initialized_candidate_source_rejects_tampered_retained_acquisition_catalog() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("initialized-tampered-catalog");
    commit_integrated_install(&root, &fixture, &raw);
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock initialized lifecycle");
    let expected = expected_from_projection(lifecycle.projection(), &project_digest);
    let DomainPackExpectedLifecycleState::Initialized {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    } = expected
    else {
        panic!("installed lifecycle must project initialized state");
    };
    let intent = DomainPackInitializedProjectIntentDocument {
        schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
        domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
            intent_id: id("intent.initialized.catalog-tamper"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            principal_id: id("principal.operator"),
            expected_state: DomainPackInitializedProjectStateBinding {
                generation,
                active_lock_digest,
                lifecycle_head_digest,
                project_snapshot_digest,
            },
            operation: DomainPackInitializedProjectOperation::Remove {
                pack: coordinate(&fixture),
            },
        },
    };
    let catalog_path = generation_directories(&root)[0].join("catalog.yaml");
    let mut catalog: serde_json::Value = yaml_serde::from_str(
        &fs::read_to_string(&catalog_path).expect("retained acquisition catalog"),
    )
    .expect("parse retained acquisition catalog");
    catalog["schema_version"] = serde_json::json!("forge.domain-pack.acquisition.v999");
    fs::write(
        &catalog_path,
        yaml_serde::to_string(&catalog).expect("serialize tampered catalog"),
    )
    .expect("tamper retained acquisition catalog");

    let error = lifecycle
        .initialized_project_source(&intent)
        .expect_err("tampered retained catalog must not provide initialized derivation source");
    assert!(
        matches!(
            error,
            DomainPackLifecycleStoreError::PreflightBlocked { .. }
        ),
        "unexpected retained catalog result: {error}"
    );

    drop(lifecycle);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn initialized_candidate_source_rejects_project_snapshot_drift() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("initialized-stale-project-snapshot");
    commit_integrated_install(&root, &fixture, &raw);
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock initialized lifecycle");
    let expected = expected_from_projection(lifecycle.projection(), &project_digest);
    let DomainPackExpectedLifecycleState::Initialized {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    } = expected
    else {
        panic!("installed lifecycle must project initialized state");
    };
    let intent = DomainPackInitializedProjectIntentDocument {
        schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
        domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
            intent_id: id("intent.initialized.snapshot-drift"),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: id("project.domain-pack.test"),
            principal_id: id("principal.operator"),
            expected_state: DomainPackInitializedProjectStateBinding {
                generation,
                active_lock_digest,
                lifecycle_head_digest,
                project_snapshot_digest,
            },
            operation: DomainPackInitializedProjectOperation::Remove {
                pack: coordinate(&fixture),
            },
        },
    };

    fs::write(
        project_root.join("project.txt"),
        b"project changed after initialized derivation\n",
    )
    .expect("mutate project after intent capture");
    let error = lifecycle
        .initialized_project_source(&intent)
        .expect_err("project drift must invalidate initialized derivation source");
    assert!(
        matches!(
            error,
            DomainPackLifecycleStoreError::StaleExpectedState { .. }
        ),
        "unexpected project drift result: {error}"
    );

    drop(lifecycle);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
#[allow(clippy::too_many_lines)]
fn initialized_project_source_exposes_all_five_operations_without_granting_authority() {
    let fixture = Fixture::new();
    let raw = RawArtifactFixture::new(&fixture);
    let root = temp_state_root("initialized-all-operation-sources");
    let (install, _, inputs) = commit_integrated_install(&root, &fixture, &raw);
    let _remove = commit_integrated_remove(&root, &fixture, &inputs);
    let project_root = root.parent().expect("project root");
    let project_digest = current_project_snapshot_digest(project_root);
    let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock initialized lifecycle");
    let expected = expected_from_projection(lifecycle.projection(), &project_digest);
    let DomainPackExpectedLifecycleState::Initialized {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    } = expected
    else {
        panic!("installed lifecycle must project initialized state");
    };
    let expected_state = DomainPackInitializedProjectStateBinding {
        generation,
        active_lock_digest,
        lifecycle_head_digest,
        project_snapshot_digest,
    };
    let selection = DomainPackInitializedProjectCandidateSelection {
        acquisition_id: id("acquisition.initialized.source"),
        assurance_binding: DurableAssuranceEpochBinding {
            project_id: id("project.domain-pack.test"),
            assurance_epoch: 1,
            intent_id: id("assurance.intent.initialized.source"),
            intent_revision: 1,
            intent_digest: digest('1'),
            accepted_record_digest: digest('2'),
            accepted_sequence: 1,
            accepted_state_version: 1,
            snapshot_digest: expected_state.project_snapshot_digest.clone(),
            ledger_head_before_acceptance: digest('3'),
        },
        discovery_projection_digest: digest('4'),
        demand_digest: digest('5'),
        candidate_id: id("candidate.initialized.source"),
        requirement_ref: id("requirement.initialized.source"),
        approval: DomainPackCandidateApprovalRequirement::ExplicitOperatorApprovalRequired,
    };
    let core_digest = lifecycle
        .projection()
        .active_lock
        .as_ref()
        .expect("active lock")
        .domain_pack_exact_lock
        .payload
        .core
        .bundle_digest
        .clone();
    let operations = [
        DomainPackInitializedProjectOperation::Install {
            selection: selection.clone(),
        },
        DomainPackInitializedProjectOperation::Upgrade {
            pack: coordinate(&fixture),
            expected_from: fixture.resolved.identity.version.clone(),
            target_requirement: "=2.0.0".to_owned(),
            required_content_digest: None,
            selection,
        },
        DomainPackInitializedProjectOperation::Rollback {
            target_receipt_digest: install.domain_pack_lifecycle_receipt.receipt_digest,
            target_lock_digest: install
                .domain_pack_lifecycle_receipt
                .to_state
                .active_lock_digest,
        },
        DomainPackInitializedProjectOperation::Remove {
            pack: coordinate(&fixture),
        },
        DomainPackInitializedProjectOperation::RebaseCore {
            target_release_id: id("release.core.source"),
            expected_from_core_digest: core_digest.clone(),
            target_core_digest: core_digest,
        },
    ];

    for (index, operation) in operations.into_iter().enumerate() {
        let intent = DomainPackInitializedProjectIntentDocument {
            schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
            domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
                intent_id: id(&format!("intent.initialized.source.{index}")),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                project_id: id("project.domain-pack.test"),
                principal_id: id("principal.operator"),
                expected_state: expected_state.clone(),
                operation: operation.clone(),
            },
        };
        let source = lifecycle
            .initialized_project_source(&intent)
            .expect("exact high-level operation exposes derivation source");
        assert_eq!(source.expected_state, expected_state);
        assert_eq!(
            source.active_lock.domain_pack_exact_lock.lock_digest,
            expected_state.active_lock_digest
        );
        assert_eq!(
            intent.domain_pack_initialized_project_intent.operation,
            operation
        );
    }

    drop(lifecycle);
    fs::remove_dir_all(project_root).expect("cleanup");
}

#[test]
fn initialized_project_intent_wire_preserves_all_operations_exact_cas_and_explicit_approval() {
    let expected_state = DomainPackInitializedProjectStateBinding {
        generation: 7,
        active_lock_digest: digest('a'),
        lifecycle_head_digest: digest('b'),
        project_snapshot_digest: digest('c'),
    };
    let selection = DomainPackInitializedProjectCandidateSelection {
        acquisition_id: id("acquisition.initialized"),
        assurance_binding: DurableAssuranceEpochBinding {
            project_id: id("project.domain-pack.test"),
            assurance_epoch: 3,
            intent_id: id("assurance.intent.initialized"),
            intent_revision: 2,
            intent_digest: digest('1'),
            accepted_record_digest: digest('2'),
            accepted_sequence: 9,
            accepted_state_version: 11,
            snapshot_digest: expected_state.project_snapshot_digest.clone(),
            ledger_head_before_acceptance: digest('3'),
        },
        discovery_projection_digest: digest('4'),
        demand_digest: digest('5'),
        candidate_id: id("candidate.initialized.exact"),
        requirement_ref: id("requirement.initialized.exact"),
        approval: DomainPackCandidateApprovalRequirement::ExplicitOperatorApprovalRequired,
    };
    let pack = DomainPackCoordinate {
        publisher: id("forge.fixture"),
        name: id("foundation.pack"),
    };
    let operations = [
        DomainPackInitializedProjectOperation::Install {
            selection: selection.clone(),
        },
        DomainPackInitializedProjectOperation::Upgrade {
            pack: pack.clone(),
            expected_from: "1.0.0".to_owned(),
            target_requirement: "=2.0.0".to_owned(),
            required_content_digest: Some(digest('6')),
            selection,
        },
        DomainPackInitializedProjectOperation::Rollback {
            target_receipt_digest: digest('7'),
            target_lock_digest: digest('8'),
        },
        DomainPackInitializedProjectOperation::Remove { pack },
        DomainPackInitializedProjectOperation::RebaseCore {
            target_release_id: id("release.core.next"),
            expected_from_core_digest: digest('9'),
            target_core_digest: digest('d'),
        },
    ];

    for (index, operation) in operations.into_iter().enumerate() {
        let document = DomainPackInitializedProjectIntentDocument {
            schema_version: DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION.to_owned(),
            domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent {
                intent_id: id(&format!("intent.initialized.{index}")),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                project_id: id("project.domain-pack.test"),
                principal_id: id("principal.operator"),
                expected_state: expected_state.clone(),
                operation,
            },
        };
        let encoded = yaml_serde::to_string(&document).expect("serialize initialized intent");
        let decoded: DomainPackInitializedProjectIntentDocument =
            yaml_serde::from_str(&encoded).expect("deserialize initialized intent");
        assert_eq!(decoded, document);
        assert_eq!(
            decoded.domain_pack_initialized_project_intent.authority,
            DomainPackCandidateAuthority::CandidateOnly
        );
        assert_eq!(
            decoded
                .domain_pack_initialized_project_intent
                .expected_state,
            expected_state
        );
        match &decoded.domain_pack_initialized_project_intent.operation {
            DomainPackInitializedProjectOperation::Install { selection }
            | DomainPackInitializedProjectOperation::Upgrade { selection, .. } => assert_eq!(
                selection.approval,
                DomainPackCandidateApprovalRequirement::ExplicitOperatorApprovalRequired
            ),
            DomainPackInitializedProjectOperation::Rollback { .. }
            | DomainPackInitializedProjectOperation::Remove { .. }
            | DomainPackInitializedProjectOperation::RebaseCore { .. } => {}
        }
    }
}

#[test]
fn initialized_project_state_wire_keeps_each_stale_binding_distinct() {
    let current = DomainPackInitializedProjectStateBinding {
        generation: 12,
        active_lock_digest: digest('a'),
        lifecycle_head_digest: digest('b'),
        project_snapshot_digest: digest('c'),
    };
    let stale = [
        DomainPackInitializedProjectStateBinding {
            generation: current.generation + 1,
            ..current.clone()
        },
        DomainPackInitializedProjectStateBinding {
            active_lock_digest: digest('d'),
            ..current.clone()
        },
        DomainPackInitializedProjectStateBinding {
            lifecycle_head_digest: digest('e'),
            ..current.clone()
        },
        DomainPackInitializedProjectStateBinding {
            project_snapshot_digest: digest('f'),
            ..current.clone()
        },
    ];

    for binding in stale {
        assert_ne!(binding, current);
        let encoded = yaml_serde::to_string(&binding).expect("serialize stale binding");
        let decoded: DomainPackInitializedProjectStateBinding =
            yaml_serde::from_str(&encoded).expect("deserialize stale binding");
        assert_eq!(decoded, binding);
    }
}
