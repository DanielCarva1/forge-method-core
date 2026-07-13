use forge_core_contracts::*;
use forge_core_decisions::resolve_domain_packs;

fn digest(seed: u64) -> String {
    format!("sha256:{seed:064x}")
}

fn p6a_request() -> DomainPackCompositionRequestDocument {
    yaml_serde::from_str(include_str!(
        "../../../docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"
    ))
    .expect("P6a fixture parses")
}

fn candidate(version: &str, seed: u64) -> DomainPackResolutionCandidate {
    let mut input = p6a_request()
        .domain_pack_composition_request
        .candidates
        .into_iter()
        .next()
        .expect("foundation candidate");
    version.clone_into(&mut input.manifest.domain_pack_manifest.identity.version);
    version.clone_into(&mut input.content.domain_pack_content.pack.version);
    let package = DomainPackPackageBinding {
        package_ref: RepoPath(format!("packages/foundation-{version}.yaml")),
        package_digest: digest(seed),
        manifest: input.manifest_binding.clone(),
        content: input.manifest.domain_pack_manifest.content.clone(),
        license: input
            .manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .clone(),
        fixtures: input
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect(),
    };
    DomainPackResolutionCandidate {
        input,
        package,
        registry_record_digest: Some(digest(seed + 10_000)),
    }
}

fn record(candidate: &DomainPackResolutionCandidate) -> DomainPackRegistryPackageRecord {
    DomainPackRegistryPackageRecord {
        identity: candidate
            .input
            .manifest
            .domain_pack_manifest
            .identity
            .clone(),
        package_digest: candidate.package.package_digest.clone(),
        manifest_digest: candidate.package.manifest.canonical_sha256.clone(),
        content_digest: candidate.package.content.canonical_sha256.clone(),
        license_digest: candidate.package.license.canonical_sha256.clone(),
        fixture_digests: candidate
            .package
            .fixtures
            .iter()
            .map(|fixture| fixture.canonical_sha256.clone())
            .collect(),
        namespace_grant_id: StableId("grant.fixture".to_owned()),
        publisher_credential_id: StableId("credential.fixture".to_owned()),
        publisher_signature_hex: "00".repeat(64),
        record_digest: candidate.registry_record_digest.clone().unwrap(),
    }
}

fn registry(candidates: &[DomainPackResolutionCandidate]) -> DomainPackSupplyChainRegistryDocument {
    DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.fixture".to_owned()),
            registry_version: "1.0.0".to_owned(),
            audience: StableId("forge.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: 100,
            expires_at_unix: 200,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                public_key_hex: "00".repeat(32),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                namespace_prefix: StableId("sample".to_owned()),
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            packages: candidates.iter().map(record).collect(),
            revocations: Vec::new(),
            snapshot_digest: digest(9_000),
            signatures: Vec::new(),
        },
    }
}

fn root(requirement: &str, reason: DomainPackResolutionRootReason) -> DomainPackResolutionRoot {
    DomainPackResolutionRoot {
        pack: DomainPackCoordinate {
            publisher: StableId("forge.fixture".to_owned()),
            name: StableId("foundation".to_owned()),
        },
        version_requirement: requirement.to_owned(),
        required_content_digest: None,
        reason,
    }
}

fn request(
    candidates: Vec<DomainPackResolutionCandidate>,
    roots: Vec<DomainPackResolutionRoot>,
) -> DomainPackResolutionRequestDocument {
    let base = p6a_request().domain_pack_composition_request;
    DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: StableId("resolution.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: base.requirements.project_id.clone(),
            forge_core_version: base.forge_core_version,
            core: base.core,
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: base.requirements,
            },
            roots,
            current_lock: None,
            policy: DomainPackResolutionPolicy {
                selection: DomainPackVersionSelectionPolicy::MinimalChangeThenHighestCompatible,
                prerelease: DomainPackPrereleasePolicy::ExplicitOnly,
                duplicate_version: DomainPackDuplicateVersionPolicy::RejectDivergentContent,
                dependency_source: DomainPackDependencySourcePolicy::ExactPublisherOnly,
                unrelated_updates: DomainPackUnrelatedUpdatePolicy::PreserveLocked,
            },
            registry_snapshot_digest: digest(9_000),
            candidates,
        },
    }
}

fn exact_lock(
    request: &DomainPackResolutionRequestDocument,
    candidate: &DomainPackResolutionCandidate,
) -> DomainPackExactLockDocument {
    let identity = candidate
        .input
        .manifest
        .domain_pack_manifest
        .identity
        .clone();
    DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            payload: DomainPackExactLockPayload {
                project_id: request.domain_pack_resolution_request.project_id.clone(),
                core: request.domain_pack_resolution_request.core.clone(),
                requirements_digest: digest(101),
                roots: request.domain_pack_resolution_request.roots.clone(),
                registry_snapshot_digest: digest(9_000),
                reviewer_registry_digest: digest(9_001),
                reviewed_registry_digest: digest(9_002),
                trust_policy_digest: digest(102),
                capability_registry_digest: digest(103),
                sandbox_policy_digest: digest(104),
                resolution_digest: digest(105),
                composition_digest: digest(106),
                unresolved_composition_gaps: vec![],
                packages: vec![DomainPackLockedPackage {
                    identity,
                    package_digest: candidate.package.package_digest.clone(),
                    manifest_binding: candidate.package.manifest.clone(),
                    content_binding: candidate.package.content.clone(),
                    license_binding: candidate.package.license.clone(),
                    fixture_bindings: candidate.package.fixtures.clone(),
                    namespace_grant_id: StableId("grant.fixture".to_owned()),
                    registry_record_digest: candidate.registry_record_digest.clone().unwrap(),
                    source_assurance: DomainPackSourceAssurance::SupplyChainVerified,
                    semantic_assurance: forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance::Reviewed,
                    reviewed_entry_digest: Some(digest(9_003)),
                    promotion_authorization_digest: Some(digest(9_004)),
                    dependencies: candidate
                        .input
                        .manifest
                        .domain_pack_manifest
                        .dependencies
                        .clone(),
                    deterministic_order: 0,
                }],
                verified_capability_bindings: Vec::new(),
                unresolved_capability_gaps: Vec::new(),
            },
            lock_digest: digest(107),
        },
    }
}

#[test]
fn resolution_is_input_order_independent_and_selects_highest_stable() {
    let one = candidate("1.0.0", 1);
    let two = candidate("2.0.0", 2);
    let candidates = vec![one.clone(), two.clone()];
    let registry = registry(&candidates);
    let forward = request(
        candidates.clone(),
        vec![root(
            ">=1,<3",
            DomainPackResolutionRootReason::InstallIntent,
        )],
    );
    let mut reverse = forward.clone();
    reverse.domain_pack_resolution_request.candidates.reverse();
    let mut reverse_registry = registry.clone();
    reverse_registry
        .domain_pack_supply_chain_registry
        .packages
        .reverse();

    let first = resolve_domain_packs(&forward, &registry);
    let second = resolve_domain_packs(&reverse, &reverse_registry);
    assert_eq!(first, second);
    assert_eq!(
        first.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved
    );
    assert_eq!(
        first.domain_pack_resolution_projection.selected[0]
            .identity
            .version,
        "2.0.0"
    );
    let selected = &first.domain_pack_resolution_projection.selected[0];
    assert_eq!(
        selected.semantic_assurance,
        forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance::Unreviewed
    );
    assert!(selected.reviewed_entry_digest.is_none());
    assert!(selected.promotion_authorization_digest.is_none());
    assert!(first.domain_pack_resolution_projection.issues.is_empty());
}

#[test]
fn pure_resolution_never_semantically_promotes_a_transitive_dependency() {
    let mut transitive = candidate("1.0.0", 41);
    transitive.input.manifest.domain_pack_manifest.identity.name =
        StableId("transitive".to_owned());
    transitive.input.content.domain_pack_content.pack.name = StableId("transitive".to_owned());
    transitive.package.package_ref = RepoPath("packages/transitive-1.0.0.yaml".to_owned());

    let mut root_candidate = candidate("1.0.0", 40);
    root_candidate
        .input
        .manifest
        .domain_pack_manifest
        .dependencies = vec![DomainPackDependency {
        pack: DomainPackCoordinate {
            publisher: StableId("forge.fixture".to_owned()),
            name: StableId("transitive".to_owned()),
        },
        version_requirement: "^1.0".to_owned(),
        required_content_digest: Some(transitive.package.content.canonical_sha256.clone()),
    }];
    let candidates = vec![root_candidate, transitive];
    let registry = registry(&candidates);
    let request = request(
        candidates,
        vec![root("^1.0", DomainPackResolutionRootReason::InstallIntent)],
    );
    let projection = resolve_domain_packs(&request, &registry);
    assert_eq!(
        projection.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved
    );
    assert_eq!(
        projection.domain_pack_resolution_projection.selected.len(),
        2
    );
    assert!(projection
        .domain_pack_resolution_projection
        .selected
        .iter()
        .all(|selected| selected.semantic_assurance
            == forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance::Unreviewed
            && selected.reviewed_entry_digest.is_none()
            && selected.promotion_authorization_digest.is_none()));
}

#[test]
fn compatible_lock_is_preserved_except_for_explicit_upgrade() {
    let one = candidate("1.0.0", 1);
    let two = candidate("2.0.0", 2);
    let candidates = vec![one.clone(), two.clone()];
    let registry = registry(&candidates);
    let mut preserved = request(
        candidates.clone(),
        vec![root(
            ">=1,<3",
            DomainPackResolutionRootReason::ExistingProjectRoot,
        )],
    );
    preserved.domain_pack_resolution_request.current_lock = Some(exact_lock(&preserved, &one));
    let kept = resolve_domain_packs(&preserved, &registry);
    assert_eq!(
        kept.domain_pack_resolution_projection.selected[0]
            .identity
            .version,
        "1.0.0"
    );

    let mut upgraded = preserved;
    upgraded.domain_pack_resolution_request.roots[0].reason =
        DomainPackResolutionRootReason::UpgradeIntent;
    let changed = resolve_domain_packs(&upgraded, &registry);
    assert_eq!(
        changed.domain_pack_resolution_projection.selected[0]
            .identity
            .version,
        "2.0.0"
    );
}

#[test]
fn prerelease_requires_an_explicit_prerelease_requirement() {
    let prerelease = candidate("2.0.0-alpha.1", 3);
    let candidates = vec![prerelease.clone()];
    let registry = registry(&candidates);
    let implicit = request(
        candidates.clone(),
        vec![root(
            ">=1,<3",
            DomainPackResolutionRootReason::InstallIntent,
        )],
    );
    let blocked = resolve_domain_packs(&implicit, &registry);
    assert_eq!(
        blocked.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Blocked
    );
    assert_eq!(
        blocked.domain_pack_resolution_projection.issues[0].code,
        DomainPackResolutionIssueCode::MissingRoot
    );

    let explicit = request(
        candidates,
        vec![root(
            ">=2.0.0-alpha.1,<2.0.0",
            DomainPackResolutionRootReason::InstallIntent,
        )],
    );
    let resolved = resolve_domain_packs(&explicit, &registry);
    assert_eq!(
        resolved.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved
    );
}

#[test]
fn revoked_equivocated_or_self_granted_candidates_cannot_resolve() {
    let valid = candidate("1.0.0", 1);
    let mut divergent = valid.clone();
    divergent.package.package_digest = digest(55);
    divergent.registry_record_digest = Some(digest(55_000));
    let candidates = vec![valid.clone(), divergent.clone()];
    let equivocation_registry = registry(&candidates);
    let equivocation_request = request(
        candidates,
        vec![root("^1", DomainPackResolutionRootReason::InstallIntent)],
    );
    let blocked = resolve_domain_packs(&equivocation_request, &equivocation_registry);
    assert_eq!(
        blocked.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Blocked
    );
    assert!(blocked
        .domain_pack_resolution_projection
        .rejected
        .iter()
        .all(|candidate| candidate
            .reasons
            .contains(&DomainPackResolutionIssueCode::DuplicateVersionEquivocation)));

    let mut revoked_registry = registry(std::slice::from_ref(&valid));
    revoked_registry
        .domain_pack_supply_chain_registry
        .revocations
        .push(DomainPackPackageRevocation {
            record_digest: valid.registry_record_digest.clone().unwrap(),
            reason: DomainPackRevocationReason::PackageTamper,
            explanation: "fixture".to_owned(),
            revoked_at_unix: 120,
        });
    let revoked_request = request(
        vec![valid.clone()],
        vec![root("^1", DomainPackResolutionRootReason::InstallIntent)],
    );
    let revoked = resolve_domain_packs(&revoked_request, &revoked_registry);
    assert!(revoked.domain_pack_resolution_projection.rejected[0]
        .reasons
        .contains(&DomainPackResolutionIssueCode::RevokedPackage));

    let mut wrong_grant = registry(std::slice::from_ref(&valid));
    wrong_grant
        .domain_pack_supply_chain_registry
        .namespace_grants[0]
        .publisher = StableId("other.publisher".to_owned());
    let denied = resolve_domain_packs(&revoked_request, &wrong_grant);
    assert!(denied.domain_pack_resolution_projection.rejected[0]
        .reasons
        .contains(&DomainPackResolutionIssueCode::NamespaceNotGranted));
}

#[test]
fn revoked_publisher_credential_is_structurally_rejected() {
    let valid = candidate("1.0.0", 1);
    let mut revoked_credential_registry = registry(std::slice::from_ref(&valid));
    revoked_credential_registry
        .domain_pack_supply_chain_registry
        .publisher_credentials[0]
        .status = DomainPackCredentialStatus::Revoked;
    let resolution_request = request(
        vec![valid],
        vec![root("^1", DomainPackResolutionRootReason::InstallIntent)],
    );

    let denied = resolve_domain_packs(&resolution_request, &revoked_credential_registry);

    assert_eq!(
        denied.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Blocked
    );
    assert!(denied.domain_pack_resolution_projection.selected.is_empty());
    assert!(denied.domain_pack_resolution_projection.rejected[0]
        .reasons
        .contains(&DomainPackResolutionIssueCode::RegistryRecordMismatch));
}

#[test]
fn deterministic_backtracking_avoids_conflicting_highest_version() {
    let one = candidate("1.0.0", 1);
    let mut two = candidate("2.0.0", 2);
    let blocker = candidate("3.0.0", 3);
    two.input
        .manifest
        .domain_pack_manifest
        .conflicts
        .push(DomainPackConflict {
            pack: DomainPackCoordinate {
                publisher: StableId("forge.fixture".to_owned()),
                name: StableId("blocker".to_owned()),
            },
            version_requirement: "*".to_owned(),
            reason: DomainPackConflictReason::SemanticIncompatibility,
            explanation: "fixture conflict".to_owned(),
        });
    let mut blocker = blocker;
    blocker.input.manifest.domain_pack_manifest.identity.name = StableId("blocker".to_owned());
    blocker
        .input
        .manifest
        .domain_pack_manifest
        .identity
        .namespace = StableId("sample.blocker".to_owned());
    blocker.input.content.domain_pack_content.pack.name = StableId("blocker".to_owned());
    let candidates = vec![one, two, blocker.clone()];
    let registry = registry(&candidates);
    let mut blocker_root = root("^3", DomainPackResolutionRootReason::InstallIntent);
    blocker_root.pack.name = StableId("blocker".to_owned());
    let request = request(
        candidates,
        vec![
            root(">=1,<3", DomainPackResolutionRootReason::InstallIntent),
            blocker_root,
        ],
    );
    let resolved = resolve_domain_packs(&request, &registry);
    assert_eq!(
        resolved.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved
    );
    assert_eq!(
        resolved
            .domain_pack_resolution_projection
            .selected
            .iter()
            .find(|package| package.identity.name.0 == "foundation")
            .unwrap()
            .identity
            .version,
        "1.0.0"
    );
}
