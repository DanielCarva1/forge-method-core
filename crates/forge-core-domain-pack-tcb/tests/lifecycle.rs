use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_package_record_digest, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
    verify_domain_pack_supply_chain_snapshot, AnchoredDomainPackSupplyChainSnapshot,
    DomainPackRegistryAnchor, DomainPackRegistryAnchorAdvance,
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

struct Fixture {
    policy: DomainPackTrustPolicyDocument,
    snapshot: DomainPackSupplyChainRegistryDocument,
    resolved: DomainPackResolvedPackage,
}

impl Fixture {
    #[allow(clippy::too_many_lines)] // One cohesive signed fixture keeps every binding identical.
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
        let mut record = DomainPackRegistryPackageRecord {
            identity: manifest.identity.clone(),
            package_digest: package_digest.clone(),
            manifest_digest: authored.manifest_binding.canonical_sha256.clone(),
            content_digest: manifest.content.canonical_sha256.clone(),
            license_digest: manifest.provenance.license_text.canonical_sha256.clone(),
            fixture_digests: authored
                .content
                .domain_pack_content
                .fixtures
                .iter()
                .map(|fixture| fixture.artifact.canonical_sha256.clone())
                .collect(),
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
        let resolved = DomainPackResolvedPackage {
            identity: manifest.identity.clone(),
            package,
            registry_record_digest: record.record_digest,
            namespace_grant_id,
            source_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            dependencies: manifest.dependencies.clone(),
            deterministic_order: 0,
        };
        Self {
            policy,
            snapshot,
            resolved,
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
    composition_request
        .domain_pack_composition_request
        .requirements
        .required_domains
        .truncate(1);
    composition_request
        .domain_pack_composition_request
        .requirements
        .project_id = id("project.domain-pack.test");
    let materials = [*material];
    let composition = compose_domain_packs(&composition_request, &materials);
    assert_eq!(
        composition.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Composable,
        "{:?}",
        composition.domain_pack_composition_projection.issues
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
        version_requirement: "^1.0".to_owned(),
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

    let mut resolution_request = base_inputs.resolution_request.clone();
    let request = &mut resolution_request.domain_pack_resolution_request;
    request.request_id = id("resolution.integrated-remove");
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
    payload.requirements_digest = canonical_digest(&requirements);
    payload.roots.clear();
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
    let materials = [raw.material(fixture)];
    let artifacts = raw.immutable(fixture);
    authorize_prepared_domain_pack_lifecycle(
        prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
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
    authorize_prepared_domain_pack_lifecycle(
        prepared,
        &DomainPackLifecycleAuthorizationContext {
            anchored_snapshot: &anchored,
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

fn commit_integrated_remove(
    state_root: &Path,
    fixture: &Fixture,
    base_inputs: &IntegratedInstallInputs,
) -> DomainPackLifecycleReceiptDocument {
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
        .prepare_candidate(document)
        .expect("prepare integrated remove");
    let authority = authorize_without_artifacts(project_root, fixture, &inputs, &prepared)
        .expect("authorize integrated remove");
    locked
        .commit(prepared, authority)
        .expect("commit integrated remove")
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
