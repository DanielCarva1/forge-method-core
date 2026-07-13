//! P6d real-process proof that one reviewed Domain Pack becomes transparent
//! workflow authority without caller-selected pack or bundle flags.

use super::*;
use forge_core_authority::{
    AttestationInput, CanonicalIntent, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_decisions::{
    compose_domain_packs, domain_pack_resolution_projection_digest,
    evaluate_domain_pack_compatibility, evaluate_domain_pack_trust, resolve_domain_packs,
    DomainPackCandidateMaterial, DomainPackCapabilityDemand, DomainPackCompatibilityInput,
    DomainPackTrustEvaluationInput, DomainPackTrustSelectedPackage,
};
use forge_core_store::sha256_content_hash;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::process::Output;

const REFERENCE_ROOT: &str = "docs/fixtures/domain-pack-reference-v0";
const WORKFLOW_AUDIENCE: &str = "forge-core:workflow:p6d-reference-e2e";
const HUMAN_CREDENTIAL: &str = "credential.workflow.p6d-human";
const HUMAN_TWO_CREDENTIAL: &str = "credential.workflow.p6d-human-two";
const WORKER_CREDENTIAL: &str = "credential.workflow.p6d-worker";
const WORKER_TWO_CREDENTIAL: &str = "credential.workflow.p6d-worker-two";
const RUNTIME_CREDENTIAL: &str = "credential.workflow.p6d-runtime";
const RUNTIME_TWO_CREDENTIAL: &str = "credential.workflow.p6d-runtime-two";

#[derive(Debug)]
struct ReferenceProject {
    root: PathBuf,
    app: PathBuf,
    state: PathBuf,
    operator: PathBuf,
    artifacts: PathBuf,
    inputs: PathBuf,
}

impl ReferenceProject {
    fn new() -> Self {
        let root = fresh_temp("p6d-reference-workflow");
        let app = root.join("app");
        let sidecar = root.join("forge-app");
        let state = sidecar.join(".forge-method");
        let operator = sidecar.join("operator");
        let artifacts = root.join("artifacts");
        let inputs = root.join("inputs");
        for directory in [&app, &state, &operator, &artifacts, &inputs] {
            fs::create_dir_all(directory).expect("reference journey directory");
        }
        fs::write(app.join("README.md"), "agent-built game project\n").expect("project artifact");
        fs::write(
            app.join(".forge-method.yaml"),
            "schema_version: forge_project_link_v1\nproject_id: project.agent-built-game\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
        )
        .expect("project link");
        let destination = artifacts.join(REFERENCE_ROOT);
        copy_tree(&repo_root().join(REFERENCE_ROOT), &destination);
        Self {
            root,
            app,
            state,
            operator,
            artifacts,
            inputs,
        }
    }

    fn workflow(&self, subcommand: &str, tail: &[String]) -> Output {
        let mut args = vec![
            "workflow".to_owned(),
            subcommand.to_owned(),
            "--root".to_owned(),
            self.app.display().to_string(),
            "--json".to_owned(),
        ];
        args.extend_from_slice(tail);
        run(&args)
    }
}

impl Drop for ReferenceProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct ReferenceSupply {
    trust_policy: PathBuf,
    registry: PathBuf,
    package_digest: String,
    registry_record_digest: String,
}

#[derive(Debug)]
struct LifecycleFiles {
    preflight: PathBuf,
    resolution: PathBuf,
    composition: PathBuf,
    trust_input: PathBuf,
}

fn typed<T: DeserializeOwned>(path: &Path) -> T {
    yaml_serde::from_str(&fs::read_to_string(path).expect("typed fixture bytes"))
        .unwrap_or_else(|error| panic!("invalid typed fixture {}: {error}", path.display()))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    sha256_content_hash(&serde_json_canonicalizer::to_vec(value).expect("canonical JSON"))
}

fn reference_request(project: &ReferenceProject) -> DomainPackCompositionRequestDocument {
    typed(
        &project
            .artifacts
            .join(format!("{REFERENCE_ROOT}/requests/agent-built-game.yaml")),
    )
}

fn reference_material<'a>(
    project: &'a ReferenceProject,
    candidate: &'a DomainPackCandidateInput,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let manifest = &candidate.manifest.domain_pack_manifest;
    (
        fs::read(
            project
                .artifacts
                .join(&candidate.manifest_binding.artifact_ref.0),
        )
        .expect("reference manifest"),
        fs::read(project.artifacts.join(&manifest.content.content_ref.0))
            .expect("reference content"),
        fs::read(
            project
                .artifacts
                .join(&manifest.provenance.license_text.artifact_ref.0),
        )
        .expect("reference license"),
    )
}

/// Reuse the P6b signer/key topology, replacing only the exact package subject
/// and then re-sealing every dependent digest and signature.
fn write_signed_reference_supply(
    project: &ReferenceProject,
    request: &DomainPackCompositionRequestDocument,
) -> ReferenceSupply {
    let (policy_path, registry_path) = write_signed_supply_chain(&project.operator);
    let mut policy: DomainPackTrustPolicyDocument = typed(&policy_path);
    let mut registry: DomainPackSupplyChainRegistryDocument = typed(&registry_path);
    let candidate = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate.manifest.domain_pack_manifest;
    let package_digest = canonical_digest(candidate);

    let rule = &mut policy.domain_pack_trust_policy.rules[0];
    rule.pack = DomainPackCoordinate {
        publisher: manifest.identity.publisher.clone(),
        name: manifest.identity.name.clone(),
    };
    rule.package_digest = Some(package_digest.clone());
    rule.content_digest = Some(manifest.content.canonical_sha256.clone());
    rule.disposition = DomainPackTrustDisposition::ActivateDeclarativeKnowledgeAndBoundBuiltIns;

    let registry_record_digest = {
        let snapshot = &mut registry.domain_pack_supply_chain_registry;
        snapshot.publisher_credentials[0].publisher = manifest.identity.publisher.clone();
        snapshot.namespace_grants[0].publisher = manifest.identity.publisher.clone();
        snapshot.namespace_grants[0].namespace_prefix = StableId("reference".to_owned());
        let record = &mut snapshot.packages[0];
        record.identity = manifest.identity.clone();
        record.package_digest.clone_from(&package_digest);
        record
            .manifest_digest
            .clone_from(&candidate.manifest_binding.canonical_sha256);
        record
            .content_digest
            .clone_from(&manifest.content.canonical_sha256);
        record
            .license_digest
            .clone_from(&manifest.provenance.license_text.canonical_sha256);
        record.fixture_digests = candidate
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.canonical_sha256.clone())
            .collect();
        record.record_digest = domain_pack_package_record_digest(record).expect("package digest");
        let publisher_key = SigningKey::from_bytes(&[42_u8; 32]);
        let publisher_bytes =
            domain_pack_publisher_signing_bytes(&snapshot.registry_id, &snapshot.audience, record)
                .expect("publisher signing bytes");
        record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());
        record.record_digest.clone()
    };
    registry.domain_pack_supply_chain_registry.snapshot_digest =
        domain_pack_registry_snapshot_digest(&registry)
            .expect("reference registry snapshot digest");
    registry
        .domain_pack_supply_chain_registry
        .signatures
        .clear();
    let registry_key = SigningKey::from_bytes(&[41_u8; 32]);
    let key_id = policy.domain_pack_trust_policy.registry_keys[0]
        .key_id
        .clone();
    let bytes = domain_pack_registry_signing_bytes(
        &registry,
        &key_id,
        DomainPackRegistryTrustRole::RegistrySigner,
    )
    .expect("registry signing bytes");
    registry
        .domain_pack_supply_chain_registry
        .signatures
        .push(DomainPackRegistrySignature {
            signer_key_id: key_id,
            role: DomainPackRegistryTrustRole::RegistrySigner,
            signature_hex: hex(&registry_key.sign(&bytes).to_bytes()),
        });
    fs::write(
        &policy_path,
        yaml_serde::to_string(&policy).expect("reference trust policy YAML"),
    )
    .expect("reference trust policy");
    fs::write(
        &registry_path,
        yaml_serde::to_string(&registry).expect("reference registry YAML"),
    )
    .expect("reference registry");
    ReferenceSupply {
        trust_policy: policy_path,
        registry: registry_path,
        package_digest,
        registry_record_digest,
    }
}

/// Rebind the already complete P6c graph to the exact reference package,
/// preserving the tested dual-review key topology and graph construction.
#[allow(clippy::too_many_lines)]
fn write_reference_promotion_graph(
    project: &ReferenceProject,
    reviewers: &Path,
    reviewed: &Path,
    request: &DomainPackCompositionRequestDocument,
    supply: &ReferenceSupply,
) -> PromotionGraphPaths {
    let graph = write_promotable_learning_graph(&project.operator, reviewers, reviewed);
    let candidate_input = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate_input.manifest.domain_pack_manifest;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let semantic_key = SigningKey::from_bytes(&[71_u8; 32]);
    let authorizer_key = SigningKey::from_bytes(&[72_u8; 32]);
    let reviewer_registry: DomainPackReviewerRegistryDocument = typed(reviewers);
    let current: DomainPackReviewedRegistryDocument = typed(reviewed);
    let mut local: DomainPackLocalLearningCandidateDocument = typed(&graph.candidate);
    local.domain_pack_local_learning_candidate.target.pack = DomainPackCoordinate {
        publisher: manifest.identity.publisher.clone(),
        name: manifest.identity.name.clone(),
    };
    local
        .domain_pack_local_learning_candidate
        .target
        .base_version = Some(manifest.identity.version.clone());
    local
        .domain_pack_local_learning_candidate
        .target
        .proposed_namespace = manifest.identity.namespace.clone();
    "the reviewed game-development pack prevents fluent false progress through representative evidence gates"
        .clone_into(&mut local.domain_pack_local_learning_candidate.assertion);
    local.domain_pack_local_learning_candidate.candidate_digest =
        candidate_self_digest(&local).expect("reference learning candidate digest");
    let candidate_digest = local
        .domain_pack_local_learning_candidate
        .candidate_digest
        .clone();

    let mut dossier: DomainPackPromotionDossierDocument = typed(&graph.dossier);
    let body = &mut dossier.domain_pack_promotion_dossier;
    body.pack = DomainPackVersionReference {
        publisher: manifest.identity.publisher.clone(),
        name: manifest.identity.name.clone(),
        version: manifest.identity.version.clone(),
    };
    body.package_digest.clone_from(&supply.package_digest);
    body.manifest_digest
        .clone_from(&candidate_input.manifest_binding.canonical_sha256);
    body.content_digest
        .clone_from(&manifest.content.canonical_sha256);
    body.license_digest
        .clone_from(&manifest.provenance.license_text.canonical_sha256);
    body.candidate_digests = vec![candidate_digest];
    body.fixture_bindings = candidate_input
        .content
        .domain_pack_content
        .fixtures
        .iter()
        .map(|fixture| DomainPackLearningFixtureBinding {
            fixture_id: fixture.id.clone(),
            fixture_ref: fixture.artifact.artifact_ref.clone(),
            producer: PrincipalId("forge.reference.fixture-author".to_owned()),
            raw_sha256: fixture.artifact.raw_sha256.clone(),
            canonical_sha256: fixture.artifact.canonical_sha256.clone(),
            expected_outcome_digest: learning_hash(&format!("expected.{}", fixture.id.0)),
            provenance_digest: learning_hash(&format!("provenance.{}", fixture.id.0)),
        })
        .collect();
    let dossier_digest =
        domain_pack_promotion_dossier_digest(&dossier).expect("reference promotion dossier digest");
    dossier
        .domain_pack_promotion_dossier
        .dossier_digest
        .clone_from(&dossier_digest);

    let mut reviews = graph
        .reviews
        .map(|path| typed::<DomainPackIndependentReviewDocument>(&path));
    for review in &mut reviews {
        let body = &mut review.domain_pack_independent_review;
        body.dossier_digest.clone_from(&dossier_digest);
        body.signed_subject_digest.clone_from(&dossier_digest);
        let digest =
            domain_pack_independent_review_digest(review).expect("reference review digest");
        review.domain_pack_independent_review.review_digest = digest;
    }
    let review_digests = reviews
        .iter()
        .map(|review| review.domain_pack_independent_review.review_digest.clone())
        .collect::<Vec<_>>();

    let mut proposed: DomainPackReviewedRegistryDocument = typed(&graph.proposed);
    let entry = &mut proposed.domain_pack_reviewed_registry.entries[0];
    entry.pack = DomainPackVersionReference {
        publisher: manifest.identity.publisher.clone(),
        name: manifest.identity.name.clone(),
        version: manifest.identity.version.clone(),
    };
    entry.package_digest.clone_from(&supply.package_digest);
    entry
        .supply_chain_record_digest
        .clone_from(&supply.registry_record_digest);
    entry
        .manifest_digest
        .clone_from(&candidate_input.manifest_binding.canonical_sha256);
    entry
        .content_digest
        .clone_from(&manifest.content.canonical_sha256);
    entry
        .license_digest
        .clone_from(&manifest.provenance.license_text.canonical_sha256);
    entry.fixture_digests = candidate_input
        .content
        .domain_pack_content
        .fixtures
        .iter()
        .map(|fixture| fixture.artifact.canonical_sha256.clone())
        .collect();
    entry.independent_review_digests.clone_from(&review_digests);
    entry
        .compatibility
        .forge_core_requirement
        .clone_from(&manifest.compatibility.forge_core_requirement);
    entry
        .compatibility
        .pack_schema_requirement
        .clone_from(&manifest.compatibility.pack_schema_requirement);
    entry.promotion_decision_digest.clear();
    entry.authorization_digest.clear();
    entry.entry_digest.clear();
    let proposed_binding_digest = domain_pack_reviewed_registry_proposal_digest(&proposed)
        .expect("reference reviewed proposal digest");

    let mut decision: DomainPackPromotionDecisionDocument = typed(&graph.decision);
    let decision_body = &mut decision.domain_pack_promotion_decision;
    decision_body.dossier_digest.clone_from(&dossier_digest);
    decision_body
        .independent_review_digests
        .clone_from(&review_digests);
    decision_body
        .registry_predecessor_digest
        .clone_from(&current.domain_pack_reviewed_registry.registry_digest);
    decision_body
        .proposed_registry_digest
        .clone_from(&proposed_binding_digest);
    let decision_digest =
        domain_pack_promotion_decision_digest(&decision).expect("reference decision digest");
    decision
        .domain_pack_promotion_decision
        .decision_digest
        .clone_from(&decision_digest);

    let mut authorization: DomainPackPromotionAuthorizationDocument = typed(&graph.authorization);
    let authorization_body = &mut authorization.domain_pack_promotion_authorization;
    authorization_body
        .payload
        .dossier_digest
        .clone_from(&dossier_digest);
    authorization_body
        .payload
        .decision_digest
        .clone_from(&decision_digest);
    authorization_body
        .payload
        .independent_review_digests
        .clone_from(&review_digests);
    authorization_body
        .payload
        .reviewer_registry_digest
        .clone_from(
            &reviewer_registry
                .domain_pack_reviewer_registry
                .registry_digest,
        );
    authorization_body
        .payload
        .current_reviewed_registry_digest
        .clone_from(&current.domain_pack_reviewed_registry.registry_digest);
    authorization_body.payload.proposed_reviewed_registry_digest = proposed_binding_digest;
    let payload_digest = domain_pack_promotion_payload_digest(&authorization_body.payload)
        .expect("reference authorization payload digest");
    for (index, signed) in authorization_body.signatures.iter_mut().enumerate() {
        signed.payload_digest.clone_from(&payload_digest);
        signed.signed_at_unix = now;
        let bytes = domain_pack_promotion_signing_bytes(&authorization_body.payload, signed)
            .expect("reference promotion signing bytes");
        signed.signature = hex(&[&semantic_key, &authorizer_key][index]
            .sign(&bytes)
            .to_bytes());
    }

    let entry = &mut proposed.domain_pack_reviewed_registry.entries[0];
    entry.promotion_decision_digest.clone_from(&decision_digest);
    entry.authorization_digest.clone_from(&payload_digest);
    entry.entry_digest =
        domain_pack_reviewed_registry_entry_digest(entry).expect("reference reviewed entry digest");
    let final_digest =
        domain_pack_reviewed_registry_digest(&proposed).expect("reference registry digest");
    proposed
        .domain_pack_reviewed_registry
        .registry_digest
        .clone_from(&final_digest);
    for (index, key) in [&semantic_key, &authorizer_key].into_iter().enumerate() {
        proposed.domain_pack_reviewed_registry.snapshot_signatures[index]
            .payload_digest
            .clone_from(&final_digest);
        proposed.domain_pack_reviewed_registry.snapshot_signatures[index].signed_at_unix = now;
        let copy = proposed.domain_pack_reviewed_registry.snapshot_signatures[index].clone();
        let bytes = domain_pack_reviewed_registry_signing_bytes(&proposed, &copy)
            .expect("reference reviewed registry signing bytes");
        proposed.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }

    let proposed_path =
        write_typed_yaml(&project.operator, "reference-reviewed-next.yaml", &proposed);
    let candidate_path = write_typed_yaml(&project.inputs, "reference-candidate.yaml", &local);
    let dossier_path = write_typed_yaml(&project.inputs, "reference-dossier.yaml", &dossier);
    let review_paths = [
        write_typed_yaml(
            &project.inputs,
            "reference-review-semantic.yaml",
            &reviews[0],
        ),
        write_typed_yaml(
            &project.inputs,
            "reference-review-authorizer.yaml",
            &reviews[1],
        ),
    ];
    let decision_path = write_typed_yaml(&project.inputs, "reference-decision.yaml", &decision);
    let authorization_path = write_typed_yaml(
        &project.inputs,
        "reference-authorization.yaml",
        &authorization,
    );
    PromotionGraphPaths {
        proposed: proposed_path,
        candidate: candidate_path,
        dossier: dossier_path,
        reviews: review_paths,
        decision: decision_path,
        authorization: authorization_path,
    }
}

fn project_snapshot_digest(project: &ReferenceProject) -> String {
    let mut entries = vec![
        (
            ".forge-method.yaml".to_owned(),
            sha256_content_hash(
                &fs::read(project.app.join(".forge-method.yaml")).expect("project link bytes"),
            ),
        ),
        (
            "README.md".to_owned(),
            sha256_content_hash(&fs::read(project.app.join("README.md")).expect("README bytes")),
        ),
    ];
    entries.sort();
    canonical_digest(&entries)
}

fn locked_package(selected: &DomainPackResolvedPackage) -> DomainPackLockedPackage {
    DomainPackLockedPackage {
        identity: selected.identity.clone(),
        package_digest: selected.package.package_digest.clone(),
        manifest_binding: selected.package.manifest.clone(),
        content_binding: selected.package.content.clone(),
        license_binding: selected.package.license.clone(),
        fixture_bindings: selected.package.fixtures.clone(),
        namespace_grant_id: selected.namespace_grant_id.clone(),
        registry_record_digest: selected.registry_record_digest.clone(),
        source_assurance: selected.source_assurance,
        semantic_assurance: selected.semantic_assurance,
        reviewed_entry_digest: selected.reviewed_entry_digest.clone(),
        promotion_authorization_digest: selected.promotion_authorization_digest.clone(),
        dependencies: selected.dependencies.clone(),
        deterministic_order: selected.deterministic_order,
    }
}

fn reference_capability_demands(
    request: &DomainPackCompositionRequestDocument,
) -> Vec<DomainPackCapabilityDemand> {
    let content = &request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content;
    let kinds = content
        .provided_capabilities
        .iter()
        .map(|capability| (capability.id.0.as_str(), capability.kind))
        .collect::<BTreeMap<_, _>>();
    let mut demands = Vec::new();
    let mut add = |subject_ref: &StableId, capability_ref: &StableId| {
        demands.push(DomainPackCapabilityDemand {
            subject_ref: subject_ref.clone(),
            capability_ref: capability_ref.clone(),
            kind: *kinds
                .get(capability_ref.0.as_str())
                .unwrap_or_else(|| panic!("undeclared reference capability {capability_ref:?}")),
        });
    };
    for lifecycle in &content.lifecycle_models {
        for transition in &lifecycle.transitions {
            for capability_ref in &transition.required_capability_refs {
                add(&transition.id, capability_ref);
            }
        }
    }
    for adapter in &content.adapters {
        for capability_ref in &adapter.required_capability_refs {
            add(&adapter.id, capability_ref);
        }
    }
    for requirement in &request
        .domain_pack_composition_request
        .requirements
        .required_domains
    {
        for capability_ref in &requirement.required_capability_refs {
            if kinds.contains_key(capability_ref.0.as_str()) {
                add(&requirement.id, capability_ref);
            }
        }
    }
    demands.sort_by(|left, right| {
        left.subject_ref
            .0
            .cmp(&right.subject_ref.0)
            .then(left.capability_ref.0.cmp(&right.capability_ref.0))
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    demands.dedup();
    demands
}

#[allow(clippy::too_many_lines)]
fn write_reference_lifecycle(
    project: &ReferenceProject,
    request: &DomainPackCompositionRequestDocument,
    supply: &ReferenceSupply,
    reviewed_path: &Path,
) -> LifecycleFiles {
    let candidate = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate.manifest.domain_pack_manifest;
    let (manifest_raw, content_raw, license_raw) = reference_material(project, candidate);
    let material = DomainPackCandidateMaterial {
        publisher: &manifest.identity.publisher.0,
        name: &manifest.identity.name.0,
        version: &manifest.identity.version,
        manifest_raw: &manifest_raw,
        content_raw: &content_raw,
        license_raw: &license_raw,
    };
    let composition = compose_domain_packs(request, &[material]);
    assert_eq!(
        composition.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Composable,
        "reference composition issues={:?}",
        composition.domain_pack_composition_projection.issues
    );

    let trust_policy: DomainPackTrustPolicyDocument = typed(&supply.trust_policy);
    let registry: DomainPackSupplyChainRegistryDocument = typed(&supply.registry);
    let reviewed: DomainPackReviewedRegistryDocument = typed(reviewed_path);
    let reviewer_registry: DomainPackReviewerRegistryDocument =
        typed(&project.operator.join("reviewers.yaml"));
    let requirements = DomainPackProjectRequirementsDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_project_requirements: request
            .domain_pack_composition_request
            .requirements
            .clone(),
    };
    let root = DomainPackResolutionRoot {
        pack: DomainPackCoordinate {
            publisher: manifest.identity.publisher.clone(),
            name: manifest.identity.name.clone(),
        },
        version_requirement: "^1.0".to_owned(),
        required_content_digest: Some(manifest.content.canonical_sha256.clone()),
        reason: DomainPackResolutionRootReason::InstallIntent,
    };
    let package = DomainPackPackageBinding {
        package_ref: RepoPath("packages/reference-game-development-1.0.0.yaml".to_owned()),
        package_digest: supply.package_digest.clone(),
        manifest: candidate.manifest_binding.clone(),
        content: manifest.content.clone(),
        license: manifest.provenance.license_text.clone(),
        fixtures: candidate
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect(),
    };
    let resolution_request = DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: StableId("resolution.agent-built-game.reference".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: request
                .domain_pack_composition_request
                .requirements
                .project_id
                .clone(),
            forge_core_version: request
                .domain_pack_composition_request
                .forge_core_version
                .clone(),
            core: request.domain_pack_composition_request.core.clone(),
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
            registry_snapshot_digest: registry
                .domain_pack_supply_chain_registry
                .snapshot_digest
                .clone(),
            candidates: vec![DomainPackResolutionCandidate {
                input: candidate.clone(),
                package,
                registry_record_digest: Some(supply.registry_record_digest.clone()),
            }],
        },
    };
    let mut resolution = resolve_domain_packs(&resolution_request, &registry);
    assert_eq!(
        resolution.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved,
        "reference resolution issues={:?}",
        resolution.domain_pack_resolution_projection.issues
    );
    let reviewed_entry = &reviewed.domain_pack_reviewed_registry.entries[0];
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(reviewed_entry.entry_digest.clone());
        selected.promotion_authorization_digest = Some(reviewed_entry.authorization_digest.clone());
    }
    resolution
        .domain_pack_resolution_projection
        .resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        &registry.domain_pack_supply_chain_registry.snapshot_digest,
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
    let capability_demands = reference_capability_demands(request);
    let evidence = selected
        .package
        .fixtures
        .first()
        .expect("reference package has a capability evidence fixture")
        .clone();
    let binding_ids = (0..capability_demands.len())
        .map(|index| StableId(format!("binding.reference-game.{index}")))
        .collect::<Vec<_>>();
    let bindings = capability_demands
        .iter()
        .zip(&binding_ids)
        .enumerate()
        .map(
            |(index, (demand, binding_id))| DomainPackRuntimeCapabilityBinding {
                binding_id: binding_id.clone(),
                pack: DomainPackVersionReference {
                    publisher: selected.identity.publisher.clone(),
                    name: selected.identity.name.clone(),
                    version: selected.identity.version.clone(),
                },
                package_digest: selected.package.package_digest.clone(),
                subject_ref: demand.subject_ref.clone(),
                capability_ref: demand.capability_ref.clone(),
                kind: demand.kind,
                provider: DomainPackRuntimeProvider::CoreBuiltin {
                    provider_id: StableId(format!("core.reference-e2e.{index}")),
                },
                implementation_digest: sha256_content_hash(
                    format!("reference-e2e-implementation-{index}").as_bytes(),
                ),
                status: DomainPackRuntimeCapabilityStatus::Available,
                evidence: evidence.clone(),
            },
        )
        .collect::<Vec<_>>();
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        selected: vec![DomainPackTrustSelectedPackage {
            package: selected.clone(),
            structurally_valid: true,
            capability_demands,
            supply_chain: assessment.clone(),
        }],
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: DomainPackRuntimeCapabilityRegistry {
            registry_id: StableId("registry.runtime.reference-game".to_owned()),
            registry_version: "1.0.0".to_owned(),
            project_id: resolution_request
                .domain_pack_resolution_request
                .project_id
                .clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            bindings,
        },
        sandbox_policy: DomainPackCapabilitySandboxPolicy {
            policy_id: StableId("policy.sandbox.reference-game".to_owned()),
            policy_version: "1.0.0".to_owned(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            default_decision: DomainPackSandboxDefaultDecision::Deny,
            allowed_builtin_binding_ids: binding_ids,
            external_execution: DomainPackExternalExecutionPolicy::DenyAll,
        },
    };
    let trust = evaluate_domain_pack_trust(&trust_input);
    assert_eq!(
        trust.status,
        forge_core_decisions::DomainPackTrustEvaluationStatus::Approved,
        "reference trust issues={:?}",
        trust.issues
    );
    let capability_registry = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry.clone(),
    };
    let sandbox_policy = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: trust_input.sandbox_policy.clone(),
    };
    let lock_payload = DomainPackExactLockPayload {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        core: request.domain_pack_composition_request.core.clone(),
        requirements_digest: canonical_digest(&requirements),
        roots: vec![root.clone()],
        registry_snapshot_digest: registry
            .domain_pack_supply_chain_registry
            .snapshot_digest
            .clone(),
        reviewer_registry_digest: reviewer_registry
            .domain_pack_reviewer_registry
            .registry_digest,
        reviewed_registry_digest: reviewed.domain_pack_reviewed_registry.registry_digest,
        trust_policy_digest: canonical_digest(&trust_policy),
        capability_registry_digest: canonical_digest(&capability_registry),
        sandbox_policy_digest: canonical_digest(&sandbox_policy),
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: vec![locked_package(&selected)],
        verified_capability_bindings: trust.verified_capability_bindings,
        unresolved_composition_gaps: vec![],
        unresolved_capability_gaps: vec![],
    };
    let proposed_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&lock_payload),
            payload: lock_payload,
        },
    };
    let operation = DomainPackLifecycleOperation::Install {
        root: root.pack.clone(),
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId("compatibility.agent-built-game.reference".to_owned()),
        operation: operation.clone(),
        sealed_core: request.domain_pack_composition_request.core.clone(),
        from_lock: None,
        to_lock: proposed_lock.clone(),
    });
    assert_eq!(
        compatibility.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Compatible
    );
    let snapshot_digest = project_snapshot_digest(project);
    let expected = DomainPackExpectedLifecycleState::Uninitialized {
        project_snapshot_digest: snapshot_digest.clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId("lifecycle.agent-built-game.reference".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: resolution_request
                .domain_pack_resolution_request
                .project_id
                .clone(),
            principal_id: StableId("principal.reference-pack-installer".to_owned()),
            operation,
            expected_state: expected.clone(),
            resolution_request_digest: canonical_digest(&resolution_request),
            project_snapshot_digest: snapshot_digest,
        },
    };
    let mut staged = vec![
        candidate.manifest_binding.clone(),
        DomainPackArtifactBinding {
            artifact_ref: manifest.content.content_ref.clone(),
            raw_sha256: manifest.content.raw_sha256.clone(),
            canonical_sha256: manifest.content.canonical_sha256.clone(),
        },
        manifest.provenance.license_text.clone(),
    ];
    staged.extend(
        candidate
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone()),
    );
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId("preflight.agent-built-game.reference".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request),
            request: lifecycle_request,
            observed_state: expected,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: vec![assessment],
            trust_decisions: trust.trust_decisions,
            capability_gaps: vec![],
            compatibility_report: compatibility,
            staged_artifacts: staged,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: vec![],
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight);
    LifecycleFiles {
        preflight: write_typed_yaml(&project.inputs, "lifecycle-preflight.yaml", &preflight),
        resolution: write_typed_yaml(
            &project.inputs,
            "resolution-request.yaml",
            &resolution_request,
        ),
        composition: write_typed_yaml(&project.inputs, "composition-request.yaml", request),
        trust_input: write_typed_yaml(&project.inputs, "trust-input.yaml", &trust_input),
    }
}

/// Build the governed remove-last transition from the exact committed install
/// inputs. The empty generation deliberately retains the composition gaps that
/// tell an agent how to restore the removed domain authority.
#[allow(clippy::too_many_lines)]
fn write_reference_remove_lifecycle(
    project: &ReferenceProject,
    install: &LifecycleFiles,
    supply: &ReferenceSupply,
    install_receipt: &DomainPackLifecycleReceiptDocument,
) -> LifecycleFiles {
    let install_preflight: DomainPackLifecyclePreflightDocument = typed(&install.preflight);
    let previous_lock = install_preflight
        .domain_pack_lifecycle_preflight
        .proposed_lock
        .clone();
    let project_snapshot_digest = install_preflight
        .domain_pack_lifecycle_preflight
        .request
        .domain_pack_lifecycle_request
        .project_snapshot_digest
        .clone();
    let mut composition_request: DomainPackCompositionRequestDocument = typed(&install.composition);
    composition_request
        .domain_pack_composition_request
        .request_id = StableId("composition.agent-built-game.remove-last".to_owned());
    composition_request
        .domain_pack_composition_request
        .candidates
        .clear();
    let composition = compose_domain_packs(&composition_request, &[]);
    assert!(composition
        .domain_pack_composition_projection
        .issues
        .is_empty());
    assert!(
        !composition
            .domain_pack_composition_projection
            .gaps
            .is_empty(),
        "remove-last must preserve actionable composition gaps"
    );

    let registry: DomainPackSupplyChainRegistryDocument = typed(&supply.registry);
    let mut resolution_request: DomainPackResolutionRequestDocument = typed(&install.resolution);
    let resolution_payload = &mut resolution_request.domain_pack_resolution_request;
    resolution_payload.request_id = StableId("resolution.agent-built-game.remove-last".to_owned());
    resolution_payload.roots.clear();
    resolution_payload.candidates.clear();
    resolution_payload.current_lock = Some(previous_lock.clone());
    let resolution = resolve_domain_packs(&resolution_request, &registry);
    assert_eq!(
        resolution.domain_pack_resolution_projection.status,
        DomainPackResolutionStatus::Resolved,
        "remove resolution issues={:?}",
        resolution.domain_pack_resolution_projection.issues
    );
    assert!(resolution
        .domain_pack_resolution_projection
        .selected
        .is_empty());

    let mut trust_input: DomainPackTrustEvaluationInput = typed(&install.trust_input);
    trust_input.selected.clear();
    trust_input.capability_registry.bindings.clear();
    trust_input
        .sandbox_policy
        .allowed_builtin_binding_ids
        .clear();
    let trust = evaluate_domain_pack_trust(&trust_input);
    assert_eq!(
        trust.status,
        forge_core_decisions::DomainPackTrustEvaluationStatus::Approved
    );
    let capability_registry = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry.clone(),
    };
    let sandbox_policy = DomainPackCapabilitySandboxPolicyDocument {
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
    payload.capability_registry_digest = canonical_digest(&capability_registry);
    payload.sandbox_policy_digest = canonical_digest(&sandbox_policy);
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
    let candidate = &reference_request(project)
        .domain_pack_composition_request
        .candidates[0];
    let operation = DomainPackLifecycleOperation::Remove {
        pack: DomainPackCoordinate {
            publisher: candidate
                .manifest
                .domain_pack_manifest
                .identity
                .publisher
                .clone(),
            name: candidate
                .manifest
                .domain_pack_manifest
                .identity
                .name
                .clone(),
        },
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId("compatibility.agent-built-game.remove-last".to_owned()),
        operation: operation.clone(),
        sealed_core: previous_lock.domain_pack_exact_lock.payload.core.clone(),
        from_lock: Some(previous_lock),
        to_lock: proposed_lock.clone(),
    });
    assert_eq!(
        compatibility.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Degraded,
        "removing the last required pack must preserve an explicit degraded compatibility result"
    );
    let committed = &install_receipt.domain_pack_lifecycle_receipt;
    let expected = DomainPackExpectedLifecycleState::Initialized {
        generation: committed.to_state.generation,
        active_lock_digest: committed.to_state.active_lock_digest.clone(),
        lifecycle_head_digest: committed.new_ledger_head_digest.clone(),
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId("lifecycle.agent-built-game.remove-last".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: composition_request
                .domain_pack_composition_request
                .requirements
                .project_id
                .clone(),
            principal_id: StableId("principal.reference-pack-installer".to_owned()),
            operation,
            expected_state: expected.clone(),
            resolution_request_digest: canonical_digest(&resolution_request),
            project_snapshot_digest,
        },
    };
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId("preflight.agent-built-game.remove-last".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request),
            request: lifecycle_request,
            observed_state: expected,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: vec![],
            trust_decisions: trust.trust_decisions,
            capability_gaps: vec![],
            compatibility_report: compatibility,
            staged_artifacts: vec![],
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: vec![],
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight);
    LifecycleFiles {
        preflight: write_typed_yaml(&project.inputs, "remove-preflight.yaml", &preflight),
        resolution: write_typed_yaml(
            &project.inputs,
            "remove-resolution-request.yaml",
            &resolution_request,
        ),
        composition: write_typed_yaml(
            &project.inputs,
            "remove-composition-request.yaml",
            &composition_request,
        ),
        trust_input: write_typed_yaml(&project.inputs, "remove-trust-input.yaml", &trust_input),
    }
}

fn run(args: &[String]) -> Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(args)
        .output()
        .expect("fresh forge-core process")
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON envelope: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn ok(output: &Output, command: &str) -> Value {
    assert!(
        output.status.success(),
        "{command} failed status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value = envelope(output);
    assert_eq!(value["ok"], true, "{value:#}");
    assert_eq!(value["command"], command);
    value
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

fn command_args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn required_str<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string {pointer}: {value:#}"))
}

fn required_u64(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing integer {pointer}: {value:#}"))
}

struct WorkflowAuthority {
    human: SigningKey,
    human_two: SigningKey,
    worker: SigningKey,
    worker_two: SigningKey,
    runtime: SigningKey,
    runtime_two: SigningKey,
}

impl WorkflowAuthority {
    // Keep the complete credential/key/grant matrix together so this security fixture is auditable.
    #[allow(clippy::too_many_lines)]
    fn install(project: &ReferenceProject) -> Self {
        let human = SigningKey::from_bytes(&[91_u8; 32]);
        let human_two = SigningKey::from_bytes(&[94_u8; 32]);
        let worker = SigningKey::from_bytes(&[93_u8; 32]);
        let worker_two = SigningKey::from_bytes(&[95_u8; 32]);
        let runtime = SigningKey::from_bytes(&[92_u8; 32]);
        let runtime_two = SigningKey::from_bytes(&[96_u8; 32]);
        let principal = |credential: &str,
                         principal: &str,
                         agent: &str,
                         role: CallerRole,
                         key: &SigningKey,
                         grants: &[&str]| {
            PrincipalRegistryEntry {
                credential_id: credential.to_owned(),
                principal_id: PrincipalId(principal.to_owned()),
                agent_id: StableId(agent.to_owned()),
                role,
                public_key_hex: hex(&key.verifying_key().to_bytes()),
                allowed_tools: vec![StableId("workflow".to_owned())],
                authority_grants: grants
                    .iter()
                    .map(|grant| StableId((*grant).to_owned()))
                    .collect(),
                status: PrincipalCredentialStatus::Active,
            }
        };
        let registry = PrincipalRegistryDocument {
            schema_version: forge_core_authority::PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: WORKFLOW_AUDIENCE.to_owned(),
                principals: vec![
                    principal(
                        HUMAN_CREDENTIAL,
                        "principal.workflow.p6d-human",
                        "agent.workflow.p6d-human-console",
                        CallerRole::Human,
                        &human,
                        &[
                            "workflow.applicability.assess",
                            "workflow.evidence.authorize_human",
                            "workflow.decision.resolve",
                        ],
                    ),
                    principal(
                        HUMAN_TWO_CREDENTIAL,
                        "principal.workflow.p6d-human-two",
                        "agent.workflow.p6d-human-console-two",
                        CallerRole::Human,
                        &human_two,
                        &[
                            "workflow.applicability.assess",
                            "workflow.evidence.authorize_human",
                            "workflow.decision.resolve",
                        ],
                    ),
                    principal(
                        WORKER_CREDENTIAL,
                        "principal.workflow.p6d-worker",
                        "agent.workflow.p6d-independent-reviewer",
                        CallerRole::Worker,
                        &worker,
                        &[
                            "workflow.evidence.authorize_review",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                    principal(
                        WORKER_TWO_CREDENTIAL,
                        "principal.workflow.p6d-worker-two",
                        "agent.workflow.p6d-independent-reviewer-two",
                        CallerRole::Worker,
                        &worker_two,
                        &[
                            "workflow.evidence.authorize_review",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                    principal(
                        RUNTIME_CREDENTIAL,
                        "principal.workflow.p6d-runtime",
                        "agent.workflow.p6d-runtime",
                        CallerRole::Runtime,
                        &runtime,
                        &[
                            "workflow.capability.authorize",
                            "workflow.evidence.authorize_runtime",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                    principal(
                        RUNTIME_TWO_CREDENTIAL,
                        "principal.workflow.p6d-runtime-two",
                        "agent.workflow.p6d-runtime-two",
                        CallerRole::Runtime,
                        &runtime_two,
                        &[
                            "workflow.capability.authorize",
                            "workflow.evidence.authorize_runtime",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                ],
            },
        };
        fs::write(
            project.operator.join("workflow-principal-registry.yaml"),
            yaml_serde::to_string(&registry).expect("workflow registry YAML"),
        )
        .expect("workflow registry");
        Self {
            human,
            human_two,
            worker,
            worker_two,
            runtime,
            runtime_two,
        }
    }

    fn attestation<T: Serialize>(
        &self,
        credential: &str,
        action: &str,
        request: &T,
    ) -> AttestationInput {
        let key = match credential {
            HUMAN_CREDENTIAL => &self.human,
            HUMAN_TWO_CREDENTIAL => &self.human_two,
            WORKER_CREDENTIAL => &self.worker,
            WORKER_TWO_CREDENTIAL => &self.worker_two,
            RUNTIME_CREDENTIAL => &self.runtime,
            RUNTIME_TWO_CREDENTIAL => &self.runtime_two,
            _ => panic!("unknown P6d workflow credential {credential}"),
        };
        let issued = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_secs(),
        )
        .expect("clock fits i64");
        let mut attestation = AttestationInput {
            credential_id: Some(credential.to_owned()),
            audience: Some(WORKFLOW_AUDIENCE.to_owned()),
            execution_intent_digest: None,
            nonce: format!(
                "p6d-{action}-{issued}-{}-{}",
                learning_hash(credential),
                learning_hash(&canonical_digest(request))
            ),
            ts: issued,
            signature: String::new(),
            public_key_hex: hex(&key.verifying_key().to_bytes()),
        };
        let intent = CanonicalIntent {
            tool: "workflow".to_owned(),
            arguments: serde_json::json!({
                "action": action,
                "request": serde_json::to_value(request).expect("workflow request JSON")
            }),
            credential_id: attestation.credential_id.clone(),
            audience: attestation.audience.clone(),
            execution_intent_digest: None,
            nonce: attestation.nonce.clone(),
            ts: attestation.ts,
        };
        attestation.signature = hex(&key
            .sign(&intent.canonical_bytes().expect("canonical intent"))
            .to_bytes());
        attestation
    }

    fn write_authorization<T: Serialize>(
        &self,
        project: &ReferenceProject,
        label: &str,
        credential: &str,
        action: &str,
        request: &T,
    ) -> (PathBuf, PathBuf) {
        let request_path =
            write_typed_json(&project.inputs, &format!("{label}-request.json"), request);
        let attestation = self.attestation(credential, action, request);
        let attestation_path = write_typed_json(
            &project.inputs,
            &format!("{label}-attestation.json"),
            &attestation,
        );
        (request_path, attestation_path)
    }
}

fn write_typed_json<T: Serialize>(root: &Path, name: &str, value: &T) -> PathBuf {
    let path = root.join(name);
    fs::write(
        &path,
        serde_json::to_vec_pretty(value).expect("typed JSON fixture"),
    )
    .expect("typed JSON fixture write");
    path
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn guidance_after_record(guidance: &Value, record: &Value) -> Value {
    let mut current = guidance.clone();
    current["data"]["ledger_head_digest"] = record["data"]["record_digest"].clone();
    current["data"]["state_version"] = record["data"]["state_version"].clone();
    current
}

fn authorize_applicability(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    label: &str,
) -> Value {
    let data = &guidance["data"];
    let basis_refs = vec!["README.md".to_owned()];
    let basis = vec![WorkflowContentAddressedReference {
        subject_ref: "README.md".to_owned(),
        subject_digest: sha256_content_hash(
            &fs::read(project.app.join("README.md")).expect("basis bytes"),
        ),
    }];
    let observed = now();
    let request = forge_core_authority::WorkflowApplicabilityAuthorizationRequest {
        project_id: StableId(required_str(data, "/project_id").to_owned()),
        policy_bundle_digest: required_str(data, "/bundle_digest").to_owned(),
        policy_ref: StableId(required_str(data, "/selected_policy_ref").to_owned()),
        state_version: required_u64(data, "/state_version"),
        current_phase: StableId(required_str(data, "/current_phase").to_owned()),
        snapshot_digest: required_str(data, "/snapshot_digest").to_owned(),
        ledger_head_digest: required_str(data, "/ledger_head_digest").to_owned(),
        applicable: true,
        evaluator_ref: StableId("evaluator.workflow.applicability.human".to_owned()),
        authority_scope: StableId("workflow.applicability.assess".to_owned()),
        basis_refs,
        basis_digest: canonical_digest(&basis),
        observed_at_unix: observed,
        expires_at_unix: observed + 3_600,
    };
    let (request_path, attestation_path) = authority.write_authorization(
        project,
        label,
        HUMAN_CREDENTIAL,
        "applicability_assess",
        &request,
    );
    let record = ok(
        &project.workflow(
            "applicability-authorize",
            &command_args(&[
                "--request-file",
                &path_arg(&request_path),
                "--attestation-file",
                &path_arg(&attestation_path),
            ]),
        ),
        "workflow.applicability_authorize",
    );
    guidance_after_record(guidance, &record)
}

#[allow(clippy::too_many_arguments)]
fn write_evidence_authorization(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    label: &str,
    credential: &str,
    claim_ref: &str,
    evaluator_ref: &str,
    provider: WorkflowEvaluatorProvider,
    kind: WorkflowEvidenceKind,
    strength: WorkflowEvidenceStrength,
) -> (PathBuf, PathBuf) {
    let data = &guidance["data"];
    let observed = now();
    let request = forge_core_authority::WorkflowEvidenceAuthorizationRequest {
        project_id: StableId(required_str(data, "/project_id").to_owned()),
        policy_bundle_digest: required_str(data, "/bundle_digest").to_owned(),
        policy_ref: StableId(required_str(data, "/selected_policy_ref").to_owned()),
        claim_ref: StableId(claim_ref.to_owned()),
        evaluator_ref: StableId(evaluator_ref.to_owned()),
        provider,
        kind,
        strength,
        outcome: WorkflowEvidenceOutcome::Pass,
        subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
        subject_ref: required_str(data, "/project_id").to_owned(),
        subject_digest: required_str(data, "/snapshot_digest").to_owned(),
        scenario_digest: sha256_content_hash(format!("p6d:{label}").as_bytes()),
        state_version: required_u64(data, "/state_version"),
        current_phase: StableId(required_str(data, "/current_phase").to_owned()),
        snapshot_digest: required_str(data, "/snapshot_digest").to_owned(),
        ledger_head_digest: required_str(data, "/ledger_head_digest").to_owned(),
        readiness_target: serde_json::from_value(data["target"].clone())
            .expect("guidance readiness target"),
        observed_at_unix: observed,
        expires_at_unix: Some(observed + 3_600),
    };
    authority.write_authorization(project, label, credential, "evidence_authorize", &request)
}

#[allow(clippy::too_many_arguments)]
fn authorize_evidence(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    label: &str,
    credential: &str,
    claim_ref: &str,
    evaluator_ref: &str,
    provider: WorkflowEvaluatorProvider,
    kind: WorkflowEvidenceKind,
    strength: WorkflowEvidenceStrength,
) -> Value {
    let (request_path, attestation_path) = write_evidence_authorization(
        project,
        authority,
        guidance,
        label,
        credential,
        claim_ref,
        evaluator_ref,
        provider,
        kind,
        strength,
    );
    let record = ok(
        &project.workflow(
            "evidence-authorize",
            &command_args(&[
                "--request-file",
                &path_arg(&request_path),
                "--attestation-file",
                &path_arg(&attestation_path),
            ]),
        ),
        "workflow.evidence_authorize",
    );
    guidance_after_record(guidance, &record)
}

/// Append every required observation under a fresh ledger head. Different
/// evaluator providers deliberately use different operator principals,
/// preventing an omnibus observation or one actor from satisfying independent
/// disciplines added to the reference pack.
fn evidence_authority(
    provider: WorkflowEvaluatorProvider,
) -> (
    &'static [&'static str],
    WorkflowEvidenceKind,
    WorkflowEvidenceStrength,
) {
    match provider {
        WorkflowEvaluatorProvider::ResearchSource => (
            &[RUNTIME_CREDENTIAL, WORKER_CREDENTIAL],
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        WorkflowEvaluatorProvider::IndependentReviewer => (
            &[WORKER_CREDENTIAL, WORKER_TWO_CREDENTIAL],
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        WorkflowEvaluatorProvider::RepositoryInspector => (
            &[RUNTIME_CREDENTIAL, RUNTIME_TWO_CREDENTIAL],
            WorkflowEvidenceKind::ArtifactInspection,
            WorkflowEvidenceStrength::InspectedArtifact,
        ),
        WorkflowEvaluatorProvider::DeterministicTool => (
            &[RUNTIME_CREDENTIAL, RUNTIME_TWO_CREDENTIAL],
            WorkflowEvidenceKind::DeterministicCheck,
            WorkflowEvidenceStrength::DeterministicVerification,
        ),
        WorkflowEvaluatorProvider::RepresentativeRuntime => (
            &[RUNTIME_CREDENTIAL, RUNTIME_TWO_CREDENTIAL],
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ),
        WorkflowEvaluatorProvider::AuthorizedHuman => (
            &[HUMAN_CREDENTIAL, HUMAN_TWO_CREDENTIAL],
            WorkflowEvidenceKind::HumanAcceptance,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
        WorkflowEvaluatorProvider::ExternalAuthority => (
            &[RUNTIME_CREDENTIAL, WORKER_CREDENTIAL],
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
    }
}

fn authorize_policy_claim_evidence(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    policy: &WorkflowGovernancePolicy,
    claim_ref: &str,
    mut guidance: Value,
) -> Value {
    let claim = policy
        .claims
        .iter()
        .find(|claim| claim.id.0 == claim_ref)
        .expect("declared policy claim");
    let evaluator = policy
        .evaluators
        .iter()
        .find(|evaluator| evaluator.id == claim.evaluator_ref)
        .expect("claim-bound evaluator");
    let (credentials, kind, strength) = evidence_authority(evaluator.provider);
    assert!(evaluator.accepted_evidence_kinds.contains(&kind));
    assert_eq!(evaluator.minimum_strength, strength);
    assert!(evaluator.minimum_distinct_principals <= credentials.len());
    for observation in 0..evaluator.minimum_passing_observations.max(1) {
        let label = format!("{}-{claim_ref}-observation-{observation}", policy.id.0);
        guidance = authorize_evidence(
            project,
            authority,
            &guidance,
            &label,
            credentials[observation % credentials.len()],
            claim_ref,
            &evaluator.id.0,
            evaluator.provider,
            kind,
            strength,
        );
    }
    guidance
}

fn authorize_declared_policy_evidence(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    request: &DomainPackCompositionRequestDocument,
    expected_policy: &str,
    mut guidance: Value,
) -> Value {
    let policy = request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies
        .iter()
        .find(|policy| policy.id.0 == expected_policy)
        .expect("declared reference policy");
    for (ordinal, claim) in policy.claims.iter().enumerate() {
        let evaluator = policy
            .evaluators
            .iter()
            .find(|evaluator| evaluator.id == claim.evaluator_ref)
            .expect("claim-bound reference evaluator");
        let (credentials, kind, strength) = evidence_authority(evaluator.provider);
        assert!(evaluator.accepted_evidence_kinds.contains(&kind));
        assert_eq!(evaluator.minimum_strength, strength);
        assert!(
            evaluator.minimum_distinct_principals <= credentials.len(),
            "P6d fixture needs more independently signed authority principals"
        );
        for observation in 0..evaluator.minimum_passing_observations.max(1) {
            assert_eq!(
                guidance["data"]["selected_policy_ref"], expected_policy,
                "evidence subject changed before {}: {guidance:#}",
                claim.id.0
            );
            let label = format!(
                "reference-policy-{ordinal}-{}-observation-{observation}",
                claim.id.0
            );
            guidance = authorize_evidence(
                project,
                authority,
                &guidance,
                &label,
                credentials[observation % credentials.len()],
                &claim.id.0,
                &evaluator.id.0,
                evaluator.provider,
                kind,
                strength,
            );
            if ordinal == 0 && observation == 0 && evaluator.minimum_passing_observations > 1 {
                guidance = ok(&project.workflow("next", &[]), "workflow.next");
                assert_eq!(
                    guidance["data"]["selected_policy_ref"], expected_policy,
                    "one observation incorrectly satisfied plural evidence"
                );
                assert!(
                    guidance["data"]["simulation"]["candidate_decision_requests"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
                );
            }
        }
    }
    guidance
}

fn authorize_policy_capabilities(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    policy: &WorkflowGovernancePolicy,
    guidance: Value,
) -> Value {
    authorize_policy_capabilities_matching(project, authority, policy, guidance, |_| true)
}

fn authorize_policy_capabilities_matching(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    policy: &WorkflowGovernancePolicy,
    mut guidance: Value,
    include: impl Fn(&StableId) -> bool,
) -> Value {
    for requirement in &policy.capability_requirements {
        if !include(&requirement.id) {
            continue;
        }
        assert_eq!(guidance["data"]["selected_policy_ref"], policy.id.0);
        let data = &guidance["data"];
        let observed = now();
        let request = forge_core_authority::WorkflowCapabilityAuthorizationRequest {
            project_id: StableId(required_str(data, "/project_id").to_owned()),
            policy_bundle_digest: required_str(data, "/bundle_digest").to_owned(),
            policy_ref: StableId(required_str(data, "/selected_policy_ref").to_owned()),
            capability_ref: requirement.id.clone(),
            state_version: required_u64(data, "/state_version"),
            current_phase: StableId(required_str(data, "/current_phase").to_owned()),
            snapshot_digest: required_str(data, "/snapshot_digest").to_owned(),
            ledger_head_digest: required_str(data, "/ledger_head_digest").to_owned(),
            probe_kind: requirement.probe_kind,
            available: true,
            authority_scope: StableId("workflow.capability.authorize".to_owned()),
            probe_ref: format!("runtime:p6d:{}", requirement.id.0),
            probe_digest: sha256_content_hash(requirement.id.0.as_bytes()),
            subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
            subject_ref: required_str(data, "/project_id").to_owned(),
            subject_digest: required_str(data, "/snapshot_digest").to_owned(),
            observed_at_unix: observed,
            expires_at_unix: Some(observed + 3_600),
        };
        let label = format!("reference-capability-{}", requirement.id.0);
        let (request_path, attestation_path) = authority.write_authorization(
            project,
            &label,
            RUNTIME_CREDENTIAL,
            "capability_authorize",
            &request,
        );
        let record = ok(
            &project.workflow(
                "capability-authorize",
                &command_args(&[
                    "--request-file",
                    &path_arg(&request_path),
                    "--attestation-file",
                    &path_arg(&attestation_path),
                ]),
            ),
            "workflow.capability_authorize",
        );
        guidance = guidance_after_record(&guidance, &record);
    }
    guidance
}

fn reference_policy<'a>(
    request: &'a DomainPackCompositionRequestDocument,
    policy_ref: &str,
) -> &'a WorkflowGovernancePolicy {
    request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies
        .iter()
        .find(|policy| policy.id.0 == policy_ref)
        .expect("reference policy")
}

fn effective_policy<'a>(
    core: &'a WorkflowGovernanceBundleDocument,
    request: &'a DomainPackCompositionRequestDocument,
    policy_ref: &str,
) -> &'a WorkflowGovernancePolicy {
    core.workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == policy_ref)
        .unwrap_or_else(|| reference_policy(request, policy_ref))
}

/// Re-authorize only the evidence/capabilities that the TCB projects as
/// missing when crossing an Explore -> Execute or Execute -> Release boundary.
/// Each synthetic view changes request binding fields only; every write still
/// passes through `require_active_policy` and advances the real CAS record.
fn close_boundary_rechecks(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    core: &WorkflowGovernanceBundleDocument,
    request: &DomainPackCompositionRequestDocument,
    mut guidance: Value,
) -> Value {
    let bound = core.workflow_governance_bundle.policies.len()
        + request.domain_pack_composition_request.candidates[0]
            .content
            .domain_pack_content
            .workflow_overlay
            .policies
            .len()
        + 1;
    for _ in 0..bound {
        let boundaries = guidance["data"]["boundary_rechecks"]
            .as_array()
            .cloned()
            .expect("typed boundary rechecks");
        if boundaries.is_empty() {
            return guidance;
        }
        let mut progressed = false;
        for boundary in boundaries {
            let policy_ref = required_str(&boundary, "/policy_ref");
            let policy = effective_policy(core, request, policy_ref);
            let mut view = guidance.clone();
            view["data"]["selected_policy_ref"] = Value::String(policy_ref.to_owned());
            view["data"]["target"] = boundary["requested_target"].clone();
            view["data"]["simulation"] = boundary["simulation"].clone();

            let missing_capabilities = boundary["simulation"]["candidate_capability_gaps"]
                .as_array()
                .expect("typed capability gaps")
                .iter()
                .filter_map(|gap| gap["id"].as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>();
            if !missing_capabilities.is_empty() {
                view = authorize_policy_capabilities_matching(
                    project,
                    authority,
                    policy,
                    view,
                    |capability_ref| missing_capabilities.contains(&capability_ref.0),
                );
                progressed = true;
            }

            let missing_claims = boundary["simulation"]["candidate_claim_results"]
                .as_array()
                .expect("typed claim results")
                .iter()
                .filter(|claim| !matches!(claim["status"].as_str(), Some("verified" | "waived")))
                .filter_map(|claim| claim["claim_id"].as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>();
            for claim_ref in missing_claims {
                view =
                    authorize_policy_claim_evidence(project, authority, policy, &claim_ref, view);
                progressed = true;
            }
            if !boundary["simulation"]["candidate_decision_requests"]
                .as_array()
                .is_none_or(Vec::is_empty)
            {
                view = authorize_decision(project, authority, &view, policy);
                progressed = true;
            }
            guidance = view;
        }
        assert!(
            progressed,
            "boundary rechecks made no CAS progress: {guidance:#}"
        );
        guidance = ok(&project.workflow("next", &[]), "workflow.next");
    }
    panic!("boundary rechecks did not converge within the policy bound")
}

fn authorize_decision(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    policy: &WorkflowGovernancePolicy,
) -> Value {
    let rule = policy
        .decision_rules
        .iter()
        .find(|rule| {
            guidance["data"]["simulation"]["candidate_decision_requests"]
                .as_array()
                .is_some_and(|requests| requests.iter().any(|request| request["id"] == rule.id.0))
        })
        .expect("active decision rule");
    let alternative = rule
        .alternatives
        .iter()
        .find(|alternative| alternative.id == rule.recommended_alternative_ref)
        .expect("recommended decision alternative");
    let data = &guidance["data"];
    let request = forge_core_authority::WorkflowDecisionAuthorizationRequest {
        project_id: StableId(required_str(data, "/project_id").to_owned()),
        policy_bundle_digest: required_str(data, "/bundle_digest").to_owned(),
        policy_ref: policy.id.clone(),
        decision_ref: rule.id.clone(),
        selected_alternative_ref: alternative.id.clone(),
        state_version: required_u64(data, "/state_version"),
        current_phase: StableId(required_str(data, "/current_phase").to_owned()),
        snapshot_digest: required_str(data, "/snapshot_digest").to_owned(),
        ledger_head_digest: required_str(data, "/ledger_head_digest").to_owned(),
        readiness_target: required_str(data, "/target").to_owned(),
        consequences_ack_digest: canonical_digest(&alternative.consequences),
    };
    let (request_path, attestation_path) = authority.write_authorization(
        project,
        &format!("decision-{}", rule.id.0),
        HUMAN_CREDENTIAL,
        "decision_resolve",
        &request,
    );
    let record = ok(
        &project.workflow(
            "decision-resolve",
            &command_args(&[
                "--request-file",
                &path_arg(&request_path),
                "--attestation-file",
                &path_arg(&attestation_path),
            ]),
        ),
        "workflow.decision_resolve",
    );
    guidance_after_record(guidance, &record)
}

fn assert_fresh_resume(project: &ReferenceProject, guidance: &Value) {
    let resumed = ok(&project.workflow("resume", &[]), "workflow.resume");
    assert_eq!(resumed["data"], guidance["data"]);
}

fn assert_rejected_without_state_mutation(
    project: &ReferenceProject,
    output: &Output,
    before: &BTreeMap<String, String>,
    expected_message: &str,
) {
    assert_eq!(output.status.code(), Some(2));
    let failure = envelope(output);
    assert_eq!(failure["exit_reason"], "rejected_by_gate");
    assert!(failure["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains(expected_message)));
    assert_eq!(&snapshot(&project.state), before);
}

fn reject_artifact_only_for_representative_claim(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    policy: &WorkflowGovernancePolicy,
    claim_ref: &str,
) {
    let claim = policy
        .claims
        .iter()
        .find(|claim| claim.id.0 == claim_ref)
        .expect("representative claim");
    let (request_path, attestation_path) = write_evidence_authorization(
        project,
        authority,
        guidance,
        &format!("artifact-only-{claim_ref}"),
        RUNTIME_CREDENTIAL,
        claim_ref,
        &claim.evaluator_ref.0,
        WorkflowEvaluatorProvider::RepositoryInspector,
        WorkflowEvidenceKind::ArtifactInspection,
        WorkflowEvidenceStrength::InspectedArtifact,
    );
    let before = snapshot(&project.state);
    let rejected = project.workflow(
        "evidence-authorize",
        &command_args(&[
            "--request-file",
            &path_arg(&request_path),
            "--attestation-file",
            &path_arg(&attestation_path),
        ]),
    );
    assert_rejected_without_state_mutation(
        project,
        &rejected,
        &before,
        "does not match current governance state",
    );
}

fn complete_ready(project: &ReferenceProject, guidance: &Value) -> Value {
    assert_eq!(
        guidance["data"]["status"], "ready_to_complete",
        "{guidance:#}"
    );
    complete_selected(project, guidance)
}

fn reject_incomplete_completion(project: &ReferenceProject, guidance: &Value) {
    let before = snapshot(&project.state);
    let rejected = project.workflow(
        "complete",
        &command_args(&[
            "--if-snapshot",
            required_str(&guidance["data"], "/snapshot_digest"),
            "--principal",
            "principal.workflow.p6d-runtime",
        ]),
    );
    assert_rejected_without_state_mutation(project, &rejected, &before, "not ready");
}

fn advance_core_until(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    core: &WorkflowGovernanceBundleDocument,
    target_policy: &str,
) -> Value {
    for _ in 0..core.workflow_governance_bundle.policies.len() {
        let mut guidance = ok(&project.workflow("next", &[]), "workflow.next");
        let selected = required_str(&guidance["data"], "/selected_policy_ref").to_owned();
        if selected == target_policy {
            return guidance;
        }
        let policy = core
            .workflow_governance_bundle
            .policies
            .iter()
            .find(|policy| policy.id.0 == selected)
            .unwrap_or_else(|| {
                panic!("unexpected non-core policy before {target_policy}: {selected}")
            });
        if guidance["data"]["status"] == "applicability_required" {
            guidance = authorize_applicability(
                project,
                authority,
                &guidance,
                &format!("core-auto-applicability-{selected}"),
            );
        }
        guidance = authorize_policy_capabilities(project, authority, policy, guidance);
        for claim in &policy.claims {
            guidance =
                authorize_policy_claim_evidence(project, authority, policy, &claim.id.0, guidance);
        }
        if policy.decision_rules.is_empty() {
            // Completion performs the same late TCB recheck; avoid a redundant
            // read-only subprocess for ordinary core policies.
            complete_selected(project, &guidance);
            continue;
        }
        guidance = ok(&project.workflow("next", &[]), "workflow.next");
        if !guidance["data"]["simulation"]["candidate_decision_requests"]
            .as_array()
            .is_none_or(Vec::is_empty)
        {
            let _decision_record = authorize_decision(project, authority, &guidance, policy);
            guidance = ok(&project.workflow("next", &[]), "workflow.next");
        }
        complete_ready(project, &guidance);
    }
    panic!("core policies did not route to {target_policy}")
}

fn complete_selected(project: &ReferenceProject, guidance: &Value) -> Value {
    let snapshot = required_str(&guidance["data"], "/snapshot_digest").to_owned();
    ok(
        &project.workflow(
            "complete",
            &command_args(&[
                "--if-snapshot",
                &snapshot,
                "--principal",
                "principal.workflow.p6d-runtime",
            ]),
        ),
        "workflow.complete",
    )
}

#[test]
#[allow(clippy::too_many_lines)]
fn p6d_reference_pack_real_journey() {
    let project = ReferenceProject::new();
    let request = reference_request(&project);
    let valid_manifest = project
        .artifacts
        .join(format!("{REFERENCE_ROOT}/manifests/game-development.yaml"));
    let valid_content = project
        .artifacts
        .join(format!("{REFERENCE_ROOT}/content/game-development.yaml"));
    let valid = ok(
        &run(&command_args(&[
            "domain-pack",
            "validate",
            "--manifest-file",
            &path_arg(&valid_manifest),
            "--content-file",
            &path_arg(&valid_content),
            "--artifact-root",
            &path_arg(&project.artifacts),
            "--forge-core-version",
            "0.9.0",
            "--json",
        ])),
        "domain-pack validate",
    );
    assert_eq!(valid["data"]["structurally_valid"], true, "{valid:#}");

    // A digest-correct core-shadow attempt is projected as structurally
    // invalid before any lifecycle, learning, workflow, or trust state exists.
    let hostile_manifest = project.artifacts.join(format!(
        "{REFERENCE_ROOT}/hostile/manifest-core-shadow.invalid.yaml"
    ));
    let hostile_content = project.artifacts.join(format!(
        "{REFERENCE_ROOT}/hostile/content-core-shadow.invalid.yaml"
    ));
    let before_hostile = snapshot(&project.root);
    let hostile = run(&command_args(&[
        "domain-pack",
        "validate",
        "--manifest-file",
        &path_arg(&hostile_manifest),
        "--content-file",
        &path_arg(&hostile_content),
        "--artifact-root",
        &path_arg(&project.artifacts),
        "--forge-core-version",
        "0.9.0",
        "--json",
    ]));
    assert!(hostile.status.success());
    let hostile = envelope(&hostile);
    assert_eq!(hostile["data"]["structurally_valid"], false);
    assert!(hostile["data"]["issues"]
        .as_array()
        .is_some_and(|issues| issues.iter().any(|issue| issue["code"] == "core_shadow")));
    assert_eq!(snapshot(&project.root), before_hostile);

    let supply = write_signed_reference_supply(&project, &request);
    let (reviewers, reviewed, _) = write_signed_learning_roots(&project.operator);
    let graph = write_reference_promotion_graph(&project, &reviewers, &reviewed, &request, &supply);
    ok(
        &run(&command_args(&[
            "domain-pack",
            "learning",
            "trust-provision",
            "--operator-root",
            &path_arg(&project.operator),
            "--reviewer-registry-file",
            &path_arg(&reviewers),
            "--reviewed-registry-file",
            &path_arg(&reviewed),
            "--project-root",
            &path_arg(&project.app),
            "--state-root",
            &path_arg(&project.state),
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE",
            "--json",
        ])),
        "domain-pack learning trust-provision",
    );
    ok(
        &run(&command_args(&[
            "domain-pack",
            "learning",
            "capture",
            "--candidate-file",
            &path_arg(&graph.candidate),
            "--state-root",
            &path_arg(&project.state),
            "--json",
        ])),
        "domain-pack learning capture",
    );
    for subcommand in ["evaluate", "conflict-check"] {
        ok(
            &run(&command_args(&[
                "domain-pack",
                "learning",
                subcommand,
                "--dossier-file",
                &path_arg(&graph.dossier),
                "--candidate-file",
                &path_arg(&graph.candidate),
                "--review-file",
                &path_arg(&graph.reviews[0]),
                "--review-file",
                &path_arg(&graph.reviews[1]),
                "--json",
            ])),
            &format!("domain-pack learning {subcommand}"),
        );
    }
    let promoted = ok(
        &run(&command_args(&[
            "domain-pack",
            "learning",
            "promote",
            "--operator-root",
            &path_arg(&project.operator),
            "--reviewer-registry-file",
            &path_arg(&reviewers),
            "--reviewed-registry-file",
            &path_arg(&reviewed),
            "--proposed-registry-file",
            &path_arg(&graph.proposed),
            "--dossier-file",
            &path_arg(&graph.dossier),
            "--candidate-file",
            &path_arg(&graph.candidate),
            "--decision-file",
            &path_arg(&graph.decision),
            "--authorization-file",
            &path_arg(&graph.authorization),
            "--review-file",
            &path_arg(&graph.reviews[0]),
            "--review-file",
            &path_arg(&graph.reviews[1]),
            "--project-root",
            &path_arg(&project.app),
            "--state-root",
            &path_arg(&project.state),
            "--json",
        ])),
        "domain-pack learning promote",
    );
    assert_eq!(promoted["data"]["generation"], 1);

    ok(
        &run(&command_args(&[
            "domain-pack",
            "trust-provision",
            "--operator-root",
            &path_arg(&project.operator),
            "--trust-policy-file",
            &path_arg(&supply.trust_policy),
            "--registry-file",
            &path_arg(&supply.registry),
            "--project-root",
            &path_arg(&project.app),
            "--artifact-root",
            &path_arg(&project.artifacts),
            "--state-root",
            &path_arg(&project.state),
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_TRUST_ON_FIRST_USE",
            "--json",
        ])),
        "domain-pack trust-provision",
    );
    let lifecycle = write_reference_lifecycle(&project, &request, &supply, &graph.proposed);
    let lifecycle_tail = command_args(&[
        "--preflight-file",
        &path_arg(&lifecycle.preflight),
        "--trust-policy-file",
        &path_arg(&supply.trust_policy),
        "--registry-file",
        &path_arg(&supply.registry),
        "--reviewer-registry-file",
        &path_arg(&reviewers),
        "--reviewed-registry-file",
        &path_arg(&graph.proposed),
        "--resolution-request-file",
        &path_arg(&lifecycle.resolution),
        "--composition-request-file",
        &path_arg(&lifecycle.composition),
        "--trust-input-file",
        &path_arg(&lifecycle.trust_input),
        "--project-root",
        &path_arg(&project.app),
        "--artifact-root",
        &path_arg(&project.artifacts),
        "--state-root",
        &path_arg(&project.state),
        "--json",
    ]);
    let mut install_receipt = None;
    for (subcommand, command) in [
        ("preflight", "domain-pack preflight"),
        ("apply", "domain-pack apply"),
    ] {
        let mut args = vec!["domain-pack".to_owned(), subcommand.to_owned()];
        args.extend(lifecycle_tail.clone());
        let result = ok(&run(&args), command);
        if subcommand == "apply" {
            assert_eq!(
                result["data"]["domain_pack_lifecycle_receipt"]["to_state"]["generation"], 0,
                "{result:#}"
            );
            install_receipt = Some(
                serde_json::from_value(result["data"].clone())
                    .expect("typed install lifecycle receipt"),
            );
        }
    }
    let install_receipt = install_receipt.expect("install apply receipt");

    let authority = WorkflowAuthority::install(&project);
    let initialized = ok(&project.workflow("init", &[]), "workflow.init");
    assert_eq!(
        initialized["data"]["effective"]["domain_pack_generation"]["generation"],
        0
    );
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");

    // Core ordering remains sealed. Progress its three discovery policies
    // honestly before the appended reference discovery policy can run.
    let discover = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(
        discover["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let discover = authorize_evidence(
        &project,
        &authority,
        &discover,
        "core-discover-intent",
        HUMAN_CREDENTIAL,
        "claim.workflow.discover-intent.intent-grounded",
        "evaluator.workflow.discover-intent.intent-review",
        WorkflowEvaluatorProvider::AuthorizedHuman,
        WorkflowEvidenceKind::HumanAcceptance,
        WorkflowEvidenceStrength::AuthoritativeAcceptance,
    );
    complete_selected(&project, &discover);

    let domain_scan = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(
        domain_scan["data"]["selected_policy_ref"],
        "policy.workflow.domain-scan"
    );
    assert_eq!(domain_scan["data"]["status"], "applicability_required");
    let domain_scan = authorize_applicability(
        &project,
        &authority,
        &domain_scan,
        "core-domain-scan-applicability",
    );
    let domain_scan = authorize_evidence(
        &project,
        &authority,
        &domain_scan,
        "core-domain-scan",
        RUNTIME_CREDENTIAL,
        "claim.workflow.domain-scan.domain-risks-bounded",
        "evaluator.workflow.domain-scan.domain-review",
        WorkflowEvaluatorProvider::ExternalAuthority,
        WorkflowEvidenceKind::ExternalAuthority,
        WorkflowEvidenceStrength::AuthoritativeAcceptance,
    );
    complete_selected(&project, &domain_scan);

    let feasibility = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(
        feasibility["data"]["selected_policy_ref"],
        "policy.workflow.technical-feasibility-scan"
    );
    assert_eq!(feasibility["data"]["status"], "applicability_required");
    let feasibility = authorize_applicability(
        &project,
        &authority,
        &feasibility,
        "core-feasibility-applicability",
    );
    let feasibility = authorize_evidence(
        &project,
        &authority,
        &feasibility,
        "core-technical-feasibility",
        RUNTIME_CREDENTIAL,
        "claim.workflow.technical-feasibility-scan.feasibility-bounded",
        "evaluator.workflow.technical-feasibility-scan.feasibility-review",
        WorkflowEvaluatorProvider::DeterministicTool,
        WorkflowEvidenceKind::DeterministicCheck,
        WorkflowEvidenceStrength::DeterministicVerification,
    );
    complete_selected(&project, &feasibility);

    let reference = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(
        reference["data"]["selected_policy_ref"],
        "reference.game-development.policy.discovery"
    );
    assert!(
        reference["data"]["simulation"]["candidate_decision_requests"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(reference["data"]["simulation"]["candidate_next_actions"]
        .as_array()
        .is_some_and(|actions| actions.iter().all(|action| action["kind"] != "ask_human")));
    let discovery_policy =
        reference_policy(&request, "reference.game-development.policy.discovery");
    let reference =
        authorize_policy_capabilities(&project, &authority, discovery_policy, reference);
    let _reference = authorize_declared_policy_evidence(
        &project,
        &authority,
        &request,
        "reference.game-development.policy.discovery",
        reference,
    );
    let decision = ok(&project.workflow("next", &[]), "workflow.next");
    let requests = decision["data"]["simulation"]["candidate_decision_requests"]
        .as_array()
        .expect("reference decision requests");
    assert_eq!(requests.len(), 1, "{decision:#}");
    assert_eq!(
        requests[0]["id"],
        "reference.game-development.decision.discovery.product-direction"
    );
    assert_eq!(
        requests[0]["recommended_alternative_ref"],
        "reference.game-development.alternative.discovery.single-platform-vertical-slice"
    );
    assert!(decision["data"]["simulation"]["candidate_next_actions"]
        .as_array()
        .is_some_and(|actions| actions.iter().any(|action| action["kind"] == "ask_human")));
    assert_fresh_resume(&project, &decision);
    let _decision_record = authorize_decision(&project, &authority, &decision, discovery_policy);
    let discovery_ready = ok(&project.workflow("next", &[]), "workflow.next");
    assert_fresh_resume(&project, &discovery_ready);
    complete_ready(&project, &discovery_ready);

    // The universal execute/release policies remain ahead of pack policies.
    // Advance them through the same admitted capabilities/evidence/decisions,
    // then exercise every reference policy to its declared readiness target.
    let core: WorkflowGovernanceBundleDocument =
        typed(&repo_root().join("contracts/workflow-governance/golden-path-v0.yaml"));
    let playable_policy =
        reference_policy(&request, "reference.game-development.policy.playable-loop");
    let playable = advance_core_until(
        &project,
        &authority,
        &core,
        "reference.game-development.policy.playable-loop",
    );
    assert_eq!(playable["data"]["target"], "execute");

    // A capability probe advances the CAS head. An otherwise valid evidence
    // authorization signed against the prior head must fail without mutation.
    let playable_claim = &playable_policy.claims[0];
    let playable_evaluator = &playable_policy.evaluators[0];
    let (credentials, kind, strength) = evidence_authority(playable_evaluator.provider);
    let (stale_request, stale_attestation) = write_evidence_authorization(
        &project,
        &authority,
        &playable,
        "stale-playable-evidence",
        credentials[0],
        &playable_claim.id.0,
        &playable_evaluator.id.0,
        playable_evaluator.provider,
        kind,
        strength,
    );
    let playable = authorize_policy_capabilities(&project, &authority, playable_policy, playable);
    let state_after_capability = snapshot(&project.state);
    let stale = project.workflow(
        "evidence-authorize",
        &command_args(&[
            "--request-file",
            &path_arg(&stale_request),
            "--attestation-file",
            &path_arg(&stale_attestation),
        ]),
    );
    assert_rejected_without_state_mutation(
        &project,
        &stale,
        &state_after_capability,
        "does not match current governance state",
    );
    let _playable_evidence = authorize_policy_claim_evidence(
        &project,
        &authority,
        playable_policy,
        &playable_claim.id.0,
        playable,
    );
    let playable_ready = ok(&project.workflow("next", &[]), "workflow.next");
    let playable_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, playable_ready);
    assert_fresh_resume(&project, &playable_ready);
    complete_ready(&project, &playable_ready);

    let first_use_policy = reference_policy(
        &request,
        "reference.game-development.policy.first-use-playtest",
    );
    let first_use = advance_core_until(
        &project,
        &authority,
        &core,
        "reference.game-development.policy.first-use-playtest",
    );
    assert_eq!(first_use["data"]["target"], "execute");
    let first_use =
        authorize_policy_capabilities(&project, &authority, first_use_policy, first_use);
    reject_artifact_only_for_representative_claim(
        &project,
        &authority,
        &first_use,
        first_use_policy,
        "reference.game-development.claim.first-use-playtest.representative-session",
    );
    let _first_use_independent_evidence = authorize_policy_claim_evidence(
        &project,
        &authority,
        first_use_policy,
        "reference.game-development.claim.first-use-playtest.independent-review",
        first_use,
    );
    let first_use_partial = ok(&project.workflow("next", &[]), "workflow.next");
    assert_ne!(first_use_partial["data"]["status"], "ready_to_complete");
    reject_incomplete_completion(&project, &first_use_partial);
    assert_fresh_resume(&project, &first_use_partial);
    let _first_use_representative_evidence = authorize_policy_claim_evidence(
        &project,
        &authority,
        first_use_policy,
        "reference.game-development.claim.first-use-playtest.representative-session",
        first_use_partial,
    );
    let first_use_ready = ok(&project.workflow("next", &[]), "workflow.next");
    let first_use_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, first_use_ready);
    assert_fresh_resume(&project, &first_use_ready);
    complete_ready(&project, &first_use_ready);

    let packaging_policy = reference_policy(
        &request,
        "reference.game-development.policy.packaging-readiness",
    );
    let packaging = advance_core_until(
        &project,
        &authority,
        &core,
        "reference.game-development.policy.packaging-readiness",
    );
    assert_eq!(packaging["data"]["target"], "release");
    let packaging =
        authorize_policy_capabilities(&project, &authority, packaging_policy, packaging);
    let _packaging_clean_evidence = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.clean-package-identity",
        packaging,
    );
    let packaging_partial = ok(&project.workflow("next", &[]), "workflow.next");
    assert_ne!(packaging_partial["data"]["status"], "ready_to_complete");
    reject_incomplete_completion(&project, &packaging_partial);
    assert_fresh_resume(&project, &packaging_partial);
    let packaging = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.installed-runtime-behavior",
        packaging_partial,
    );
    let _packaging_final_evidence = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.release-audit",
        packaging,
    );
    let packaging_ready = ok(&project.workflow("next", &[]), "workflow.next");
    let packaging_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, packaging_ready);
    assert_fresh_resume(&project, &packaging_ready);
    complete_ready(&project, &packaging_ready);

    let release_status = ok(
        &project.workflow("release-status", &[]),
        "workflow.release_status",
    );
    assert_eq!(release_status["data"]["domain_pack_rebase_required"], true);
    assert!(release_status["data"]["upgrade_argv"].is_null());
    let target = required_str(&release_status, "/data/available_successor/release_id");
    let current = required_str(&release_status, "/data/active/release/release_digest");
    let head = required_str(&release_status, "/data/ledger_head_digest");
    let snapshot_digest = required_str(&release_status, "/data/snapshot_digest");
    let state_before_rebase = snapshot(&project.state);
    let rejected = project.workflow(
        "release-upgrade",
        &command_args(&[
            "--target-release-id",
            target,
            "--expected-current-release-digest",
            current,
            "--expected-head-digest",
            head,
            "--expected-snapshot-digest",
            snapshot_digest,
        ]),
    );
    assert_eq!(rejected.status.code(), Some(2));
    let rejected = envelope(&rejected);
    assert_eq!(rejected["exit_reason"], "rejected_by_gate");
    assert!(rejected["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("rebase")));
    assert_eq!(snapshot(&project.state), state_before_rebase);

    // The agent-facing workflow surface remains zero-config: neither pack nor
    // effective bundle identity can be selected by a caller.
    let state_before_injection = snapshot(&project.state);
    let injected = project.workflow("next", &command_args(&["--pack-file", "attacker.yaml"]));
    assert_eq!(injected.status.code(), Some(3));
    assert_eq!(snapshot(&project.state), state_before_injection);

    // Removing the last pack is governed and reversible, but never silently
    // resumes core-only authority: the empty generation is explicitly degraded
    // and exposes the exact restoration gaps on both next and resume.
    let removal = write_reference_remove_lifecycle(&project, &lifecycle, &supply, &install_receipt);
    let removal_tail = command_args(&[
        "--preflight-file",
        &path_arg(&removal.preflight),
        "--trust-policy-file",
        &path_arg(&supply.trust_policy),
        "--registry-file",
        &path_arg(&supply.registry),
        "--reviewer-registry-file",
        &path_arg(&reviewers),
        "--reviewed-registry-file",
        &path_arg(&graph.proposed),
        "--resolution-request-file",
        &path_arg(&removal.resolution),
        "--composition-request-file",
        &path_arg(&removal.composition),
        "--trust-input-file",
        &path_arg(&removal.trust_input),
        "--project-root",
        &path_arg(&project.app),
        "--artifact-root",
        &path_arg(&project.artifacts),
        "--state-root",
        &path_arg(&project.state),
        "--json",
    ]);
    for (subcommand, command) in [
        ("preflight", "domain-pack preflight"),
        ("apply", "domain-pack apply"),
    ] {
        let mut args = vec!["domain-pack".to_owned(), subcommand.to_owned()];
        args.extend(removal_tail.clone());
        let result = ok(&run(&args), command);
        if subcommand == "apply" {
            assert_eq!(
                result["data"]["domain_pack_lifecycle_receipt"]["to_state"]["generation"], 1,
                "{result:#}"
            );
        }
    }
    let degraded_next = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(degraded_next["data"]["status"], "blocked");
    assert_eq!(degraded_next["data"]["domain_pack_degraded"], true);
    let gaps = degraded_next["data"]["domain_pack_gaps"]
        .as_array()
        .expect("typed degraded gaps");
    assert!(!gaps.is_empty());
    for required_code in ["missing_domain", "missing_capability"] {
        assert!(
            gaps.iter().any(|gap| gap["code"] == required_code),
            "remove-last omitted required {required_code} recovery gap: {gaps:#?}"
        );
    }
    assert!(gaps.iter().all(|gap| {
        gap["requirement_ref"]
            .as_str()
            .is_some_and(|requirement| !requirement.is_empty())
            && gap["subject_ref"]
                .as_str()
                .is_some_and(|subject| !subject.is_empty())
            && gap["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty())
    }));
    assert_fresh_resume(&project, &degraded_next);

    let clean_claim = packaging_policy
        .claims
        .iter()
        .find(|claim| {
            claim.id.0 == "reference.game-development.claim.packaging.clean-package-identity"
        })
        .expect("clean package claim");
    let clean_evaluator = packaging_policy
        .evaluators
        .iter()
        .find(|evaluator| evaluator.id == clean_claim.evaluator_ref)
        .expect("clean package evaluator");
    let (credentials, kind, strength) = evidence_authority(clean_evaluator.provider);
    let (blocked_request, blocked_attestation) = write_evidence_authorization(
        &project,
        &authority,
        &degraded_next,
        "degraded-evidence-mutation",
        credentials[0],
        &clean_claim.id.0,
        &clean_evaluator.id.0,
        clean_evaluator.provider,
        kind,
        strength,
    );
    let degraded_state = snapshot(&project.state);
    let blocked_evidence = project.workflow(
        "evidence-authorize",
        &command_args(&[
            "--request-file",
            &path_arg(&blocked_request),
            "--attestation-file",
            &path_arg(&blocked_attestation),
        ]),
    );
    assert_rejected_without_state_mutation(
        &project,
        &blocked_evidence,
        &degraded_state,
        "Domain Pack gaps block workflow mutation",
    );
    let blocked_complete = project.workflow(
        "complete",
        &command_args(&[
            "--if-snapshot",
            required_str(&degraded_next["data"], "/snapshot_digest"),
            "--principal",
            "principal.workflow.p6d-runtime",
        ]),
    );
    assert_rejected_without_state_mutation(
        &project,
        &blocked_complete,
        &degraded_state,
        "Domain Pack gaps block workflow mutation",
    );
}
