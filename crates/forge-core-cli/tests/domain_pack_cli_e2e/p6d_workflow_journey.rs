//! P6d real-process proof that one reviewed Domain Pack becomes transparent
//! workflow authority without caller-selected pack or bundle flags.

use super::*;
use forge_core_authority::{
    workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
    AuthorizedWorkflowBrokerControlPlane, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, WorkflowBrokerEventEnvelope,
    WorkflowBrokerIssuerProfile, WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_decisions::{
    compose_domain_packs, discover_domain_packs, domain_pack_resolution_projection_digest,
    evaluate_domain_pack_compatibility, evaluate_domain_pack_trust, resolve_domain_packs,
    DomainPackCandidateMaterial, DomainPackCapabilityDemand, DomainPackCompatibilityInput,
    DomainPackTrustEvaluationInput, DomainPackTrustSelectedPackage,
};
use forge_core_store::{
    sha256_content_hash, workflow_action_replay::WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH,
};
use forge_core_workflow_governance_tcb::WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::process::Output;
use std::time::{Duration, Instant};

const REFERENCE_ROOT: &str = "docs/fixtures/domain-pack-reference-v0";
const WORKFLOW_AUDIENCE: &str = "forge-core:workflow:p6d-reference-e2e";
const HUMAN_CREDENTIAL: &str = "credential.workflow.p6d-human";
const HUMAN_TWO_CREDENTIAL: &str = "credential.workflow.p6d-human-two";
const WORKER_CREDENTIAL: &str = "credential.workflow.p6d-worker";
const WORKER_TWO_CREDENTIAL: &str = "credential.workflow.p6d-worker-two";
const RUNTIME_CREDENTIAL: &str = "credential.workflow.p6d-runtime";
const RUNTIME_TWO_CREDENTIAL: &str = "credential.workflow.p6d-runtime-two";
const C1_BROKER_ISSUER_ID: &str = "broker.host.human.p6d-c1";
const C1_BROKER_PRINCIPAL_ID: &str = "principal.human.p6d-c1";
const C1_BROKER_SEPARATION_DOMAIN: &str = "human-session.p6d-c1";
const CLI_SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(300);

fn heartbeat(started: Instant, phase: &str) {
    eprintln!(
        "P6d heartbeat: phase={phase}; elapsed_seconds={:.3}",
        started.elapsed().as_secs_f64()
    );
}

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
        self.workflow_with_env(subcommand, tail, None)
    }
    fn workflow_nested(&self, group: &str, action: &str, tail: &[String]) -> Output {
        let mut args = vec![
            "workflow".to_owned(),
            group.to_owned(),
            action.to_owned(),
            "--root".to_owned(),
            self.app.display().to_string(),
            "--json".to_owned(),
        ];
        args.extend_from_slice(tail);
        Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .timeout(CLI_SUBPROCESS_TIMEOUT)
            .args(&args)
            .output()
            .expect("fresh nested forge-core process")
    }

    fn workflow_with_env(
        &self,
        subcommand: &str,
        tail: &[String],
        env: Option<(&str, &str)>,
    ) -> Output {
        let mut args = vec![
            "workflow".to_owned(),
            subcommand.to_owned(),
            "--root".to_owned(),
            self.app.display().to_string(),
            "--json".to_owned(),
        ];
        args.extend_from_slice(tail);
        let mut command = Command::cargo_bin("forge-core").expect("forge-core binary");
        command.timeout(CLI_SUBPROCESS_TIMEOUT).args(&args);
        if let Some((name, value)) = env {
            command.env(name, value);
        }
        command.output().expect("fresh forge-core process")
    }
}

impl Drop for ReferenceProject {
    fn drop(&mut self) {
        if std::env::var_os("FORGE_P6D_KEEP_TEMP").is_some() {
            eprintln!("preserving P6d diagnostic root: {}", self.root.display());
        } else {
            let _ = fs::remove_dir_all(&self.root);
        }
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
    acquisition_input: PathBuf,
    acquisition_intent: PathBuf,
    discovery_request: PathBuf,
    discovery_projection: PathBuf,
    acquisition_catalog: PathBuf,
    capability_registry: PathBuf,
    sandbox_policy: PathBuf,
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

fn reference_artifact_descriptor(
    kind: DomainPackRemoteArtifactKind,
    binding: DomainPackArtifactBinding,
    raw: &[u8],
) -> DomainPackRemoteArtifactDescriptor {
    let object_digest = binding
        .raw_sha256
        .strip_prefix("sha256:")
        .expect("reference raw digest prefix");
    let object_path = RepoPath(format!("objects/sha256/{object_digest}"));
    DomainPackRemoteArtifactDescriptor {
        kind,
        binding,
        object_path,
        byte_length: u64::try_from(raw.len()).expect("reference artifact byte length"),
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

/// Reuse the P6b signer/key topology, replacing only the exact package subject
/// and then re-sealing every dependent digest and signature.
#[allow(clippy::too_many_lines)]
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
    let (manifest_raw, content_raw, license_raw) = reference_material(project, candidate);
    let artifacts = DomainPackRegistryArtifactSet {
        manifest: reference_artifact_descriptor(
            DomainPackRemoteArtifactKind::Manifest,
            candidate.manifest_binding.clone(),
            &manifest_raw,
        ),
        content: reference_artifact_descriptor(
            DomainPackRemoteArtifactKind::Content,
            DomainPackArtifactBinding {
                artifact_ref: manifest.content.content_ref.clone(),
                raw_sha256: manifest.content.raw_sha256.clone(),
                canonical_sha256: manifest.content.canonical_sha256.clone(),
            },
            &content_raw,
        ),
        license: reference_artifact_descriptor(
            DomainPackRemoteArtifactKind::License,
            manifest.provenance.license_text.clone(),
            &license_raw,
        ),
        fixtures: candidate
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| {
                let raw = fs::read(project.artifacts.join(&fixture.artifact.artifact_ref.0))
                    .expect("reference fixture");
                reference_artifact_descriptor(
                    DomainPackRemoteArtifactKind::Fixture,
                    fixture.artifact.clone(),
                    &raw,
                )
            })
            .collect(),
    };
    let manifest_digest = artifacts.manifest.binding.raw_sha256.clone();
    let content_digest = artifacts.content.binding.raw_sha256.clone();
    let license_digest = artifacts.license.binding.raw_sha256.clone();
    let fixture_digests = artifacts
        .fixtures
        .iter()
        .map(|fixture| fixture.binding.raw_sha256.clone())
        .collect();

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
        record.manifest_digest = manifest_digest;
        record.content_digest = content_digest;
        record.license_digest = license_digest;
        record.fixture_digests = fixture_digests;
        record.artifacts = artifacts;
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
            .registry_digest
            .clone(),
        reviewed_registry_digest: reviewed
            .domain_pack_reviewed_registry
            .registry_digest
            .clone(),
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
            project_snapshot_digest: snapshot_digest.clone(),
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

    let assurance_binding = DurableAssuranceEpochBinding {
        project_id: requirements
            .domain_pack_project_requirements
            .project_id
            .clone(),
        assurance_epoch: 1,
        intent_id: StableId("intent.agent-built-game.reference".to_owned()),
        intent_revision: 1,
        intent_digest: sha256_content_hash(b"agent-built-game-reference-intent"),
        accepted_record_digest: sha256_content_hash(b"agent-built-game-reference-acceptance"),
        accepted_sequence: 1,
        accepted_state_version: 1,
        snapshot_digest: snapshot_digest.clone(),
        ledger_head_before_acceptance: sha256_content_hash(b"agent-built-game-reference-head"),
    };
    let discovery_request = DomainPackDiscoveryRequestDocument {
        schema_version: DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION.to_owned(),
        domain_pack_discovery_request: DomainPackDiscoveryRequest {
            request_id: StableId("discovery.agent-built-game.reference".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            assurance_binding: assurance_binding.clone(),
            requirements: requirements.domain_pack_project_requirements.clone(),
            provenance: DomainPackDemandProvenance {
                source: DomainPackDemandSource::HostProposal,
                source_ref: "host://p6d-reference-journey".to_owned(),
                source_digest: sha256_content_hash(b"p6d-reference-host-proposal"),
            },
            uncertainties: Vec::new(),
            candidates: vec![DomainPackDiscoveryCandidate {
                reviewed_entry: reviewed_entry.clone(),
                content: candidate.content.clone(),
            }],
        },
    };
    let discovery = discover_domain_packs(&discovery_request).expect("reference discovery");
    let matched = discovery.domain_pack_discovery_projection.matches[0].clone();
    let planning_input = DomainPackAcquisitionPlanningInput {
        intent: DomainPackAcquisitionIntentDocument {
            schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
            domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
                acquisition_id: StableId("acquisition.agent-built-game.reference".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                assurance_binding,
                discovery_projection_digest: discovery
                    .domain_pack_discovery_projection
                    .projection_digest
                    .clone(),
                demand_digest: discovery
                    .domain_pack_discovery_projection
                    .demand_digest
                    .clone(),
                candidate_id: matched.candidate_id,
                requirement_ref: matched.requirement_ref,
                operation: DomainPackAcquisitionOperation::Install,
                expected_project_snapshot_digest: snapshot_digest,
            },
        },
        request: discovery_request,
        discovery,
    };
    let acquisition_catalog = DomainPackAcquisitionCatalogDocument {
        schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
        forge_core_version: request
            .domain_pack_composition_request
            .forge_core_version
            .clone(),
        core: request.domain_pack_composition_request.core.clone(),
        registry: registry.clone(),
        candidates: resolution_request
            .domain_pack_resolution_request
            .candidates
            .clone(),
    };
    LifecycleFiles {
        preflight: write_typed_yaml(&project.inputs, "lifecycle-preflight.yaml", &preflight),
        resolution: write_typed_yaml(
            &project.inputs,
            "resolution-request.yaml",
            &resolution_request,
        ),
        composition: write_typed_yaml(&project.inputs, "composition-request.yaml", request),
        trust_input: write_typed_yaml(&project.inputs, "trust-input.yaml", &trust_input),
        acquisition_input: project.inputs.join("acquisition-input.yaml"),
        acquisition_intent: write_typed_yaml(
            &project.inputs,
            "acquisition-intent.yaml",
            &planning_input.intent,
        ),
        discovery_request: write_typed_yaml(
            &project.inputs,
            "acquisition-discovery-request.yaml",
            &planning_input.request,
        ),
        discovery_projection: write_typed_yaml(
            &project.inputs,
            "acquisition-discovery-projection.yaml",
            &planning_input.discovery,
        ),
        acquisition_catalog: write_typed_yaml(
            &project.inputs,
            "acquisition-catalog.yaml",
            &acquisition_catalog,
        ),
        capability_registry: write_typed_yaml(
            &project.operator,
            "runtime-capability-registry.yaml",
            &capability_registry,
        ),
        sandbox_policy: write_typed_yaml(
            &project.operator,
            "capability-sandbox-policy.yaml",
            &sandbox_policy,
        ),
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
    let committed_state = &install_receipt.domain_pack_lifecycle_receipt.to_state;
    let committed_head = committed_state
        .lifecycle_head_digest
        .strip_prefix("sha256:")
        .expect("committed lifecycle head is canonical SHA-256");
    let previous_lock: DomainPackExactLockDocument = typed(
        &project
            .state
            .join("domain-packs")
            .join("generations")
            .join(format!(
                "{:020}-{committed_head}",
                committed_state.generation
            ))
            .join("lock.yaml"),
    );
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
        acquisition_input: install.acquisition_input.clone(),
        acquisition_intent: install.acquisition_intent.clone(),
        discovery_request: install.discovery_request.clone(),
        discovery_projection: install.discovery_projection.clone(),
        acquisition_catalog: install.acquisition_catalog.clone(),
        capability_registry: install.capability_registry.clone(),
        sandbox_policy: install.sandbox_policy.clone(),
    }
}

fn publish_active_runtime_policy(lifecycle: &LifecycleFiles) {
    let trust_input: DomainPackTrustEvaluationInput = typed(&lifecycle.trust_input);
    let capability_registry = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry,
    };
    let sandbox_policy = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: trust_input.sandbox_policy,
    };
    fs::write(
        &lifecycle.capability_registry,
        yaml_serde::to_string(&capability_registry).expect("active capability registry YAML"),
    )
    .expect("publish active capability registry");
    fs::write(
        &lifecycle.sandbox_policy,
        yaml_serde::to_string(&sandbox_policy).expect("active sandbox policy YAML"),
    )
    .expect("publish active sandbox policy");
}

fn maybe_export_reference_catalog(
    lifecycle: &LifecycleFiles,
    supply: &ReferenceSupply,
    reviewers: &Path,
    reviewed: &Path,
) {
    let Some(root) = std::env::var_os("FORGE_EXPORT_REFERENCE_CATALOG") else {
        return;
    };
    let root = PathBuf::from(root);
    fs::create_dir_all(&root).expect("reference catalog export root");
    for (source, name) in [
        (&supply.trust_policy, "trust-policy.yaml"),
        (&supply.registry, "supply-chain-registry.yaml"),
        (&reviewers.to_path_buf(), "reviewer-registry.yaml"),
        (&reviewed.to_path_buf(), "reviewed-registry.yaml"),
        (
            &lifecycle.capability_registry,
            "runtime-capability-registry.yaml",
        ),
        (&lifecycle.sandbox_policy, "capability-sandbox-policy.yaml"),
        (&lifecycle.acquisition_catalog, "acquisition-catalog.yaml"),
    ] {
        fs::copy(source, root.join(name))
            .unwrap_or_else(|error| panic!("export {} as {name}: {error}", source.display()));
    }
}

fn run(args: &[String]) -> Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .timeout(CLI_SUBPROCESS_TIMEOUT)
        .args(args)
        .output()
        .expect("fresh bounded forge-core process")
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

struct WorkflowAuthority {
    human: SigningKey,
    human_two: SigningKey,
    worker: SigningKey,
    worker_two: SigningKey,
    runtime: SigningKey,
    runtime_two: SigningKey,
    c1_human: SigningKey,
    broker_audience: String,
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

        let c1_human = SigningKey::from_bytes(&[97_u8; 32]);
        let admin = SigningKey::from_bytes(&[98_u8; 32]);
        let project_id = StableId("project.agent-built-game".to_owned());
        let workflow_id = StableId("workflow.governance".to_owned());
        let broker_audience = workflow_broker_expected_audience(&project_id, &workflow_id);
        let host_binding = WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "0.12.0".to_owned(),
            adapter_id: StableId("adapter.forge-standalone.p6d".to_owned()),
            adapter_version: "0.1.0".to_owned(),
            host_installation_id: StableId("host.installation.p6d".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        };
        let enrollment_operation_id = StableId("admin.operation.p6d-genesis".to_owned());
        let enrolled_at = now().saturating_sub(60);
        let event_credential =
            |credential_id: &str,
             broker_id: &str,
             subject_id: &str,
             profile: WorkflowBrokerCredentialProfile,
             key: &SigningKey,
             allowed_operations: Vec<WorkflowBrokerBoundOperation>| {
                WorkflowBrokerPublicCredentialMetadata {
                    credential_id: StableId(credential_id.to_owned()),
                    broker_id: StableId(broker_id.to_owned()),
                    subject_id: StableId(subject_id.to_owned()),
                    purpose: WorkflowBrokerCredentialPurpose::EventIssuer,
                    profile,
                    algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
                    public_key_hex: hex(key.verifying_key().as_bytes()),
                    key_generation: 1,
                    status: WorkflowBrokerCredentialStatus::Active,
                    custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
                    host_binding: host_binding.clone(),
                    allowed_operations,
                    not_before_unix: enrolled_at,
                    revoked_at_unix: None,
                    predecessor_credential_id: None,
                    enrollment_operation_id: enrollment_operation_id.clone(),
                    revocation_operation_id: None,
                }
            };
        let human_operations = vec![
            WorkflowBrokerBoundOperation::Applicability,
            WorkflowBrokerBoundOperation::Decision,
            WorkflowBrokerBoundOperation::Evidence,
            WorkflowBrokerBoundOperation::Waiver,
        ];
        let reviewer_operations = vec![WorkflowBrokerBoundOperation::Evidence];
        let runtime_operations = vec![
            WorkflowBrokerBoundOperation::Capability,
            WorkflowBrokerBoundOperation::Evidence,
        ];
        let mut credentials = vec![
            WorkflowBrokerPublicCredentialMetadata {
                credential_id: StableId("credential.workflow.p6d-admin".to_owned()),
                broker_id: StableId("broker.workflow.p6d-admin".to_owned()),
                subject_id: StableId("administrator.workflow.p6d".to_owned()),
                purpose: WorkflowBrokerCredentialPurpose::RegistryAdministrator,
                profile: WorkflowBrokerCredentialProfile::Administrator,
                algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
                public_key_hex: hex(admin.verifying_key().as_bytes()),
                key_generation: 1,
                status: WorkflowBrokerCredentialStatus::Active,
                custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
                host_binding: host_binding.clone(),
                allowed_operations: Vec::new(),
                not_before_unix: enrolled_at,
                revoked_at_unix: None,
                predecessor_credential_id: None,
                enrollment_operation_id: enrollment_operation_id.clone(),
                revocation_operation_id: None,
            },
            event_credential(
                HUMAN_CREDENTIAL,
                "broker.workflow.p6d-human",
                HUMAN_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Human,
                &human,
                human_operations.clone(),
            ),
            event_credential(
                HUMAN_TWO_CREDENTIAL,
                "broker.workflow.p6d-human-two",
                HUMAN_TWO_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Human,
                &human_two,
                human_operations,
            ),
            event_credential(
                WORKER_CREDENTIAL,
                "broker.workflow.p6d-worker",
                WORKER_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Reviewer,
                &worker,
                reviewer_operations.clone(),
            ),
            event_credential(
                WORKER_TWO_CREDENTIAL,
                "broker.workflow.p6d-worker-two",
                WORKER_TWO_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Reviewer,
                &worker_two,
                reviewer_operations,
            ),
            event_credential(
                RUNTIME_CREDENTIAL,
                "broker.workflow.p6d-runtime",
                RUNTIME_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Runtime,
                &runtime,
                runtime_operations.clone(),
            ),
            event_credential(
                RUNTIME_TWO_CREDENTIAL,
                "broker.workflow.p6d-runtime-two",
                RUNTIME_TWO_CREDENTIAL,
                WorkflowBrokerCredentialProfile::Runtime,
                &runtime_two,
                runtime_operations,
            ),
            event_credential(
                "credential.workflow.p6d-c1-human",
                "broker.workflow.p6d-c1-human",
                C1_BROKER_ISSUER_ID,
                WorkflowBrokerCredentialProfile::Human,
                &c1_human,
                vec![WorkflowBrokerBoundOperation::IntentRevision],
            ),
        ];
        credentials.sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        let broker_registry = WorkflowBrokerPublicRegistryDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: broker_audience.clone(),
            project_id: project_id.clone(),
            workflow_id: workflow_id.clone(),
            registry_generation: 1,
            previous_registry_digest: None,
            required_event_schema_version: WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION.to_owned(),
            credentials,
        };
        AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            broker_registry.clone(),
            &broker_audience,
            &project_id,
            &workflow_id,
        )
        .expect("strict P6d broker registry fixture");
        fs::write(
            project.operator.join("workflow-broker-registry.yaml"),
            yaml_serde::to_string(&broker_registry).expect("strict P6d broker registry YAML"),
        )
        .expect("publish preconfigured external broker registry");
        Self {
            human,
            human_two,
            worker,
            worker_two,
            runtime,
            runtime_two,
            c1_human,
            broker_audience,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn write_broker_action(
        &self,
        project: &ReferenceProject,
        guidance: &Value,
        label: &str,
        credential: &str,
        packet_digest: String,
        semantic_input: WorkflowBrokerSemanticInput,
    ) -> PathBuf {
        let (key, profile, principal, separation_domain) = match credential {
            HUMAN_CREDENTIAL => (
                &self.human,
                WorkflowBrokerIssuerProfile::Human,
                "principal.workflow.p6d-human",
                "human-session.p6d-primary",
            ),
            HUMAN_TWO_CREDENTIAL => (
                &self.human_two,
                WorkflowBrokerIssuerProfile::Human,
                "principal.workflow.p6d-human-two",
                "human-session.p6d-secondary",
            ),
            WORKER_CREDENTIAL => (
                &self.worker,
                WorkflowBrokerIssuerProfile::Reviewer,
                "principal.workflow.p6d-worker",
                "reviewer-installation.p6d-primary",
            ),
            WORKER_TWO_CREDENTIAL => (
                &self.worker_two,
                WorkflowBrokerIssuerProfile::Reviewer,
                "principal.workflow.p6d-worker-two",
                "reviewer-installation.p6d-secondary",
            ),
            RUNTIME_CREDENTIAL => (
                &self.runtime,
                WorkflowBrokerIssuerProfile::Runtime,
                "principal.workflow.p6d-runtime",
                "runtime-installation.p6d-primary",
            ),
            RUNTIME_TWO_CREDENTIAL => (
                &self.runtime_two,
                WorkflowBrokerIssuerProfile::Runtime,
                "principal.workflow.p6d-runtime-two",
                "runtime-installation.p6d-secondary",
            ),
            _ => panic!("unknown P6d broker credential {credential}"),
        };
        let interaction_kind = match profile {
            WorkflowBrokerIssuerProfile::Human => {
                WorkflowBrokerHostInteractionKind::NativeHumanConfirmation
            }
            WorkflowBrokerIssuerProfile::Reviewer => {
                WorkflowBrokerHostInteractionKind::NativeReviewerConfirmation
            }
            WorkflowBrokerIssuerProfile::Runtime => {
                WorkflowBrokerHostInteractionKind::AttestedRuntimeObservation
            }
        };
        let issued_at_unix = now();
        let nonce = format!(
            "p6d-{label}-{issued_at_unix}-{}",
            learning_hash(&packet_digest)
        );
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: self.broker_audience.clone(),
            issuer_id: StableId(credential.to_owned()),
            issuer_profile: profile,
            origin_principal_id: PrincipalId(principal.to_owned()),
            separation_domain: StableId(separation_domain.to_owned()),
            event_kind: semantic_input.kind(),
            project_id: StableId(required_str(guidance, "/data/project_id").to_owned()),
            action_packet_digest: packet_digest,
            semantic_input,
            native_host_provenance: Some(WorkflowBrokerNativeHostProvenance {
                host_kind: RuntimeKind::ForgeStandalone,
                host_version: "0.12.0".to_owned(),
                adapter_id: StableId("adapter.forge-standalone.p6d".to_owned()),
                adapter_version: "0.1.0".to_owned(),
                interaction_kind,
                host_event_ref: format!("host-event-{nonce}"),
                host_session_ref: "host-session-p6d-0001".to_owned(),
                host_interaction_ref: format!("host-interaction-{nonce}"),
                host_event_descriptor_digest: format!("sha256:{}", "0".repeat(64)),
                host_observed_at_unix: issued_at_unix,
            }),
            issued_at_unix,
            expires_at_unix: issued_at_unix + 300,
            nonce,
            signature: String::new(),
        };
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("native host provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("host descriptor digest");
        envelope.signature = hex(&key
            .sign(
                &workflow_broker_event_signing_bytes(&envelope)
                    .expect("workflow broker event signing bytes"),
            )
            .to_bytes());
        write_typed_json(
            &project.inputs,
            &format!("{label}-broker-envelope.json"),
            &envelope,
        )
    }

    fn apply_broker_action(
        &self,
        project: &ReferenceProject,
        guidance: &Value,
        label: &str,
        credential: &str,
        packet_digest: String,
        semantic_input: WorkflowBrokerSemanticInput,
    ) -> Value {
        let envelope_path = self.write_broker_action(
            project,
            guidance,
            label,
            credential,
            packet_digest,
            semantic_input,
        );
        eprintln!("P6d broker apply: start label={label}");
        let result = ok(
            &project.workflow_nested(
                "action",
                "apply",
                &command_args(&["--origin-envelope-file", &path_arg(&envelope_path)]),
            ),
            "workflow.action.apply",
        );
        eprintln!("P6d broker apply: complete label={label}");
        result
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

fn signed_c1_broker_envelope(
    key: &SigningKey,
    audience: &str,
    guidance: &Value,
    packet_digest: String,
    semantic_input: WorkflowBrokerSemanticInput,
    nonce: &str,
) -> WorkflowBrokerEventEnvelope {
    let issued_at_unix = now();
    let mut envelope = WorkflowBrokerEventEnvelope {
        schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
        audience: audience.to_owned(),
        issuer_id: StableId(C1_BROKER_ISSUER_ID.to_owned()),
        issuer_profile: WorkflowBrokerIssuerProfile::Human,
        origin_principal_id: PrincipalId(C1_BROKER_PRINCIPAL_ID.to_owned()),
        separation_domain: StableId(C1_BROKER_SEPARATION_DOMAIN.to_owned()),
        event_kind: semantic_input.kind(),
        project_id: StableId(required_str(guidance, "/data/project_id").to_owned()),
        action_packet_digest: packet_digest,
        semantic_input,
        native_host_provenance: Some(WorkflowBrokerNativeHostProvenance {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "0.12.0".to_owned(),
            adapter_id: StableId("adapter.forge-standalone.p6d".to_owned()),
            adapter_version: "0.1.0".to_owned(),
            interaction_kind: WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
            host_event_ref: format!("host-event-{nonce}"),
            host_session_ref: "host-session-p6d-c1-0001".to_owned(),
            host_interaction_ref: format!("host-interaction-{nonce}"),
            host_event_descriptor_digest: format!("sha256:{}", "0".repeat(64)),
            host_observed_at_unix: issued_at_unix,
        }),
        issued_at_unix,
        expires_at_unix: issued_at_unix + 300,
        nonce: nonce.to_owned(),
        signature: String::new(),
    };
    let provenance = envelope
        .native_host_provenance
        .as_mut()
        .expect("native host provenance");
    provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
        provenance,
        &envelope.project_id,
        &envelope.action_packet_digest,
        &envelope.semantic_input,
    )
    .expect("host descriptor digest");
    let signing_bytes =
        workflow_broker_event_signing_bytes(&envelope).expect("broker signing bytes");
    envelope.signature = hex(&key.sign(&signing_bytes).to_bytes());
    envelope
}

fn current_action_packet<'a>(guidance: &'a Value, kind: &str, subject_ref: &str) -> &'a Value {
    let data = &guidance["data"];
    data["authorization"]["action_packets"]
        .as_array()
        .and_then(|packets| {
            packets.iter().find(|packet| {
                packet["authorization_kind"] == kind
                    && packet["binding"]["subject_ref"] == subject_ref
                    && packet["binding"]["state_version"] == data["state_version"]
                    && packet["binding"]["ledger_head_digest"] == data["ledger_head_digest"]
            })
        })
        .unwrap_or_else(|| panic!("missing current {kind} packet for {subject_ref}: {guidance:#}"))
}

fn current_action_packet_digest(guidance: &Value, kind: &str, subject_ref: &str) -> String {
    current_action_packet(guidance, kind, subject_ref)["packet_digest"]
        .as_str()
        .expect("current action packet digest")
        .to_owned()
}

fn broker_subject(
    guidance: &Value,
    packet: &Value,
    subject_ref: &str,
) -> (WorkflowEvidenceSubjectKind, String) {
    let subject_kinds = packet["input_contract"]["subject_kinds"]
        .as_array()
        .unwrap_or_else(|| panic!("packet has no closed subject kinds: {packet:#}"));
    let allows = |kind: &str| subject_kinds.iter().any(|candidate| candidate == kind);
    if allows("project_snapshot") {
        return (
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            required_str(guidance, "/data/project_id").to_owned(),
        );
    }
    if allows("artifact") {
        return (
            WorkflowEvidenceSubjectKind::Artifact,
            "README.md".to_owned(),
        );
    }
    if allows("repository_state") {
        return (
            WorkflowEvidenceSubjectKind::RepositoryState,
            required_str(guidance, "/data/project_id").to_owned(),
        );
    }
    if allows("runtime") {
        return (
            WorkflowEvidenceSubjectKind::Runtime,
            format!("runtime:p6d:{subject_ref}"),
        );
    }
    if allows("external_system") {
        return (
            WorkflowEvidenceSubjectKind::ExternalSystem,
            format!("external-system:p6d:{subject_ref}"),
        );
    }
    if allows("human_decision") {
        return (
            WorkflowEvidenceSubjectKind::HumanDecision,
            format!("human-decision:p6d:{subject_ref}"),
        );
    }
    panic!("packet has no supported P6d subject kind: {packet:#}");
}

fn broker_receipt_next(record: &Value) -> Value {
    let next = record["data"]["next"]
        .as_object()
        .map(|_| record["data"]["next"].clone())
        .expect("workflow broker receipt next guidance");
    let mut guidance = record.clone();
    guidance["data"] = next;
    guidance
}

fn authorize_applicability(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    guidance: &Value,
    label: &str,
) -> Value {
    let policy_ref = required_str(guidance, "/data/selected_policy_ref");
    let packet_digest = current_action_packet_digest(guidance, "applicability", policy_ref);
    let record = authority.apply_broker_action(
        project,
        guidance,
        label,
        HUMAN_CREDENTIAL,
        packet_digest,
        WorkflowBrokerSemanticInput::Applicability {
            applicable: true,
            basis_refs: vec!["README.md".to_owned()],
        },
    );
    broker_receipt_next(&record)
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
    let _ = (evaluator_ref, provider, kind, strength);
    let packet = current_action_packet(guidance, "evidence", claim_ref);
    let packet_digest = packet["packet_digest"]
        .as_str()
        .expect("evidence packet digest")
        .to_owned();
    let (subject_kind, subject_ref) = broker_subject(guidance, packet, claim_ref);
    let record = authority.apply_broker_action(
        project,
        guidance,
        label,
        credential,
        packet_digest,
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind,
            subject_ref,
            scenario_ref: "README.md".to_owned(),
        },
    );
    broker_receipt_next(&record)
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
        let is_current_gap = guidance["data"]["simulation"]["candidate_capability_gaps"]
            .as_array()
            .is_some_and(|gaps| gaps.iter().any(|gap| gap["id"] == requirement.id.0));
        if !include(&requirement.id) || !is_current_gap {
            continue;
        }
        assert_eq!(guidance["data"]["selected_policy_ref"], policy.id.0);
        let packet = current_action_packet(&guidance, "capability", &requirement.id.0);
        let packet_digest = packet["packet_digest"]
            .as_str()
            .expect("capability packet digest")
            .to_owned();
        let (subject_kind, subject_ref) = broker_subject(&guidance, packet, &requirement.id.0);
        let label = format!("reference-capability-{}", requirement.id.0);
        let record = authority.apply_broker_action(
            project,
            &guidance,
            &label,
            RUNTIME_CREDENTIAL,
            packet_digest,
            WorkflowBrokerSemanticInput::Capability {
                available: true,
                probe_ref: "README.md".to_owned(),
                subject_kind,
                subject_ref,
            },
        );
        guidance = broker_receipt_next(&record);
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
#[allow(clippy::too_many_lines)]
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
        let mut progressed = false;
        let selected_policy_ref =
            required_str(&guidance["data"], "/selected_policy_ref").to_owned();
        let selected_policy = effective_policy(core, request, &selected_policy_ref);
        let missing_selected_claims = guidance["data"]["simulation"]["candidate_claim_results"]
            .as_array()
            .expect("typed selected claim results")
            .iter()
            .filter(|claim| !matches!(claim["status"].as_str(), Some("verified" | "waived")))
            .filter_map(|claim| claim["claim_id"].as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        for claim_ref in missing_selected_claims {
            guidance = authorize_policy_claim_evidence(
                project,
                authority,
                selected_policy,
                &claim_ref,
                guidance,
            );
            progressed = true;
        }
        if guidance["data"]["simulation"]["candidate_capability_gaps"]
            .as_array()
            .is_some_and(|gaps| !gaps.is_empty())
        {
            guidance = authorize_policy_capabilities(project, authority, selected_policy, guidance);
            progressed = true;
        }
        if !guidance["data"]["simulation"]["candidate_decision_requests"]
            .as_array()
            .is_none_or(Vec::is_empty)
        {
            guidance = authorize_decision(project, authority, &guidance, selected_policy);
            progressed = true;
        }

        let boundaries = guidance["data"]["boundary_rechecks"]
            .as_array()
            .cloned()
            .expect("typed boundary rechecks");
        if boundaries.is_empty() {
            return guidance;
        }
        for boundary in boundaries {
            let policy_ref = required_str(&boundary, "/policy_ref");
            let policy = effective_policy(core, request, policy_ref);
            let mut view = guidance.clone();
            view["data"]["selected_policy_ref"] = Value::String(policy_ref.to_owned());
            view["data"]["target"] = boundary["requested_target"].clone();
            view["data"]["simulation"] = boundary["simulation"].clone();

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
    }
    panic!("boundary rechecks did not converge within the policy bound")
}

fn current_decision_packet_digest(
    guidance: &Value,
    policy: &WorkflowGovernancePolicy,
    rule: &WorkflowDecisionRule,
) -> String {
    let data = &guidance["data"];
    if let Some(packet_digest) = data["authorization"]["action_packets"]
        .as_array()
        .and_then(|packets| {
            packets.iter().find(|packet| {
                packet["authorization_kind"] == "decision"
                    && packet["binding"]["policy_ref"] == policy.id.0
                    && packet["binding"]["subject_ref"] == rule.id.0
                    && packet["binding"]["state_version"] == data["state_version"]
                    && packet["binding"]["ledger_head_digest"] == data["ledger_head_digest"]
            })
        })
        .and_then(|packet| packet["packet_digest"].as_str())
    {
        return packet_digest.to_owned();
    }
    // P7a binds a decision acknowledgement to the exact current action
    // packet, not only to the consequence strings. Reconstruct that packet
    // from the latest CAS coordinates immediately before signing. Boundary
    // recheck views are synthetic policy projections, so their decision
    // packet is not necessarily present in the selected-policy packet list.
    let packet_template = data["authorization"]["action_packets"]
        .as_array()
        .and_then(|packets| packets.first())
        .expect("current authorization packet template");
    let packet_digest_basis = serde_json::json!({
        "schema_version": "workflow_authorization_action_packets_v1",
        "packet_id": format!("packet.workflow.decision.{}", rule.id.0),
        "authorization_kind": "decision",
        "binding": {
            "project_id": data["project_id"].clone(),
            "effective_bundle_id": data["effective"]["effective_runtime_bundle"]["bundle_id"].clone(),
            "effective_bundle_digest": data["bundle_digest"].clone(),
            "policy_ref": policy.id.clone(),
            "subject_ref": rule.id.clone(),
            "state_version": data["state_version"].clone(),
            "current_phase": data["current_phase"].clone(),
            "snapshot_digest": data["snapshot_digest"].clone(),
            "ledger_head_digest": data["ledger_head_digest"].clone(),
            "trusted_principal_registry_digest": packet_template["binding"]["trusted_principal_registry_digest"].clone(),
            "trusted_broker_registry_digest": packet_template["binding"]["trusted_broker_registry_digest"].clone(),
            "readiness_target": policy.routing.readiness_target,
        },
        "required_authority": {
            "accepted_roles": ["human"],
            "required_grant": "workflow.decision.resolve",
            "approval_boundary": "human_approval_broker",
        },
        "input_contract": {
            "kind": "decision",
            "decision_ref": rule.id.clone(),
            "alternatives": rule.alternatives.clone(),
            "recommended_alternative_ref": rule.recommended_alternative_ref.clone(),
        },
    });
    canonical_digest(&packet_digest_basis)
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
    let packet_digest = current_decision_packet_digest(guidance, policy, rule);
    let record = authority.apply_broker_action(
        project,
        guidance,
        &format!("decision-{}", rule.id.0),
        HUMAN_CREDENTIAL,
        packet_digest,
        WorkflowBrokerSemanticInput::Decision {
            selected_alternative_ref: alternative.id.clone(),
        },
    );
    broker_receipt_next(&record)
}

fn resume_without_state_mutation(project: &ReferenceProject, guidance: &Value) -> Value {
    let before = snapshot(&project.state);
    let resumed = ok(&project.workflow("resume", &[]), "workflow.resume");
    assert_eq!(snapshot(&project.state), before);
    for pointer in [
        "/authority",
        "/project_id",
        "/bundle_id",
        "/bundle_digest",
        "/state_version",
        "/current_phase",
        "/snapshot_digest",
        "/ledger_head_digest",
        "/selected_policy_ref",
        "/target",
        "/effective",
        "/release",
        "/domain_pack_degraded",
        "/domain_pack_gaps",
    ] {
        assert_eq!(
            resumed["data"].pointer(pointer),
            guidance["data"].pointer(pointer),
            "workflow.resume changed durable guidance field {pointer}"
        );
    }
    resumed
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
    assert!(
        policy.claims.iter().any(|claim| claim.id.0 == claim_ref),
        "missing representative claim {claim_ref}"
    );
    let packet = current_action_packet(guidance, "evidence", claim_ref);
    assert!(packet["input_contract"]["subject_kinds"]
        .as_array()
        .is_some_and(|kinds| !kinds.iter().any(|kind| kind == "artifact")));
    let envelope_path = authority.write_broker_action(
        project,
        guidance,
        &format!("artifact-only-{claim_ref}"),
        RUNTIME_CREDENTIAL,
        packet["packet_digest"]
            .as_str()
            .expect("representative evidence packet digest")
            .to_owned(),
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Artifact,
            subject_ref: "README.md".to_owned(),
            scenario_ref: "README.md".to_owned(),
        },
    );
    let before = snapshot(&project.state);
    let rejected = project.workflow_nested(
        "action",
        "apply",
        &command_args(&["--origin-envelope-file", &path_arg(&envelope_path)]),
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

fn complete_ready_after_rechecks(
    project: &ReferenceProject,
    authority: &WorkflowAuthority,
    core: &WorkflowGovernanceBundleDocument,
    request: &DomainPackCompositionRequestDocument,
    mut guidance: Value,
) -> Value {
    const COMPLETION_ATTEMPTS: usize = 2;

    let selected_policy_ref = required_str(&guidance["data"], "/selected_policy_ref").to_owned();
    for attempt in 0..COMPLETION_ATTEMPTS {
        assert_eq!(
            guidance["data"]["status"], "ready_to_complete",
            "{guidance:#}"
        );
        let before = snapshot(&project.state);
        let output = complete_output(project, &guidance);
        if output.status.success() {
            return ok(&output, "workflow.complete");
        }

        let failure = envelope(&output);
        let message = required_str(&failure, "/error/message");
        let (expected_exit_code, expected_exit_reason) = match message {
            "selected policy is not ready for governed completion" => (2, "rejected_by_gate"),
            "governed completion drifted during late recheck; refresh and retry from new guidance" => {
                (4, "conflict")
            }
            _ => panic!(
                "workflow.complete failed unexpectedly status={:?}\n{failure:#}",
                output.status.code()
            ),
        };
        assert_eq!(
            output.status.code(),
            Some(expected_exit_code),
            "{failure:#}"
        );
        assert_eq!(failure["exit_reason"], expected_exit_reason, "{failure:#}");
        assert_eq!(snapshot(&project.state), before);
        assert!(
            attempt + 1 < COMPLETION_ATTEMPTS,
            "workflow.complete remained temporally incomplete after bounded rechecks: {failure:#}"
        );

        guidance = resume_without_state_mutation(project, &guidance);
        assert_eq!(
            guidance["data"]["selected_policy_ref"], selected_policy_ref,
            "completion retry changed selected policy: {guidance:#}"
        );
        guidance = close_boundary_rechecks(project, authority, core, request, guidance);
    }
    unreachable!("bounded completion loop always returns or panics")
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
        for claim in &policy.claims {
            guidance =
                authorize_policy_claim_evidence(project, authority, policy, &claim.id.0, guidance);
        }
        // Capability receipts are intentionally short-lived. Probe after the
        // potentially slow evidence sequence so completion observes a current
        // runtime receipt even on constrained hosts.
        guidance = authorize_policy_capabilities(project, authority, policy, guidance);
        if policy.decision_rules.is_empty() {
            // Completion performs the same late TCB recheck; avoid a redundant
            // read-only subprocess for ordinary core policies.
            complete_selected(project, &guidance);
            continue;
        }
        if !guidance["data"]["simulation"]["candidate_decision_requests"]
            .as_array()
            .is_none_or(Vec::is_empty)
        {
            guidance = authorize_decision(project, authority, &guidance, policy);
        }
        complete_ready(project, &guidance);
    }
    panic!("core policies did not route to {target_policy}")
}

fn complete_output(project: &ReferenceProject, guidance: &Value) -> Output {
    let snapshot = required_str(&guidance["data"], "/snapshot_digest").to_owned();
    project.workflow(
        "complete",
        &command_args(&[
            "--if-snapshot",
            &snapshot,
            "--principal",
            "principal.workflow.p6d-runtime",
        ]),
    )
}

fn complete_selected(project: &ReferenceProject, guidance: &Value) -> Value {
    ok(&complete_output(project, guidance), "workflow.complete")
}

#[test]
#[allow(clippy::too_many_lines)]
fn p6d_reference_pack_real_journey() {
    let started = Instant::now();
    heartbeat(started, "fixture-validation");
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

    heartbeat(started, "learning-and-trust");
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
    heartbeat(started, "acquisition-lifecycle");
    let lifecycle = write_reference_lifecycle(&project, &request, &supply, &graph.proposed);
    maybe_export_reference_catalog(&lifecycle, &supply, &reviewers, &graph.proposed);
    let prepared_acquisition = ok(
        &run(&command_args(&[
            "domain-pack",
            "acquire",
            "prepare",
            "--intent-file",
            &path_arg(&lifecycle.acquisition_intent),
            "--request-file",
            &path_arg(&lifecycle.discovery_request),
            "--projection-file",
            &path_arg(&lifecycle.discovery_projection),
            "--catalog-file",
            &path_arg(&lifecycle.acquisition_catalog),
            "--json",
        ])),
        "domain-pack acquire prepare",
    );
    let acquisition: DomainPackAcquisitionDerivationInput =
        serde_json::from_value(prepared_acquisition["data"].clone())
            .expect("prepared acquisition input");
    write_typed_yaml(&project.inputs, "acquisition-input.yaml", &acquisition);
    let result = ok(
        &run(&command_args(&[
            "domain-pack",
            "acquire",
            "apply",
            "--derivation-input-file",
            &path_arg(&lifecycle.acquisition_input),
            "--operator-approve-candidate",
            &acquisition
                .plan
                .domain_pack_acquisition_plan
                .selected
                .candidate_id
                .0,
            "--trust-policy-file",
            &path_arg(&supply.trust_policy),
            "--registry-file",
            &path_arg(&supply.registry),
            "--reviewer-registry-file",
            &path_arg(&reviewers),
            "--reviewed-registry-file",
            &path_arg(&graph.proposed),
            "--capability-registry-file",
            &path_arg(&lifecycle.capability_registry),
            "--sandbox-policy-file",
            &path_arg(&lifecycle.sandbox_policy),
            "--principal-id",
            "principal.reference-pack-installer",
            "--project-root",
            &path_arg(&project.app),
            "--artifact-root",
            &path_arg(&project.artifacts),
            "--state-root",
            &path_arg(&project.state),
            "--json",
        ])),
        "domain-pack acquire apply",
    );
    assert_eq!(
        result["data"]["domain_pack_lifecycle_receipt"]["to_state"]["generation"], 0,
        "{result:#}"
    );
    let install_receipt: DomainPackLifecycleReceiptDocument =
        serde_json::from_value(result["data"].clone()).expect("typed install lifecycle receipt");

    heartbeat(started, "workflow-discovery");
    let authority = WorkflowAuthority::install(&project);
    let initialized = ok(&project.workflow("init", &[]), "workflow.init");
    assert_eq!(
        initialized["data"]["effective"]["domain_pack_generation"]["generation"],
        0
    );
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");
    let c1_broker_key = &authority.c1_human;
    let c1_broker_audience = &authority.broker_audience;

    // The strict action surface requires a durable human-origin intent before
    // any policy mutation becomes actionable.
    let intent_guidance = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(
        intent_guidance["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let c1_intent_packet_digest = current_action_packet_digest(
        &intent_guidance,
        "intent_revision",
        "intent.workflow.project.agent-built-game",
    );
    let c1_envelope = signed_c1_broker_envelope(
        c1_broker_key,
        c1_broker_audience,
        &intent_guidance,
        c1_intent_packet_digest,
        WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome: "Deliver a trustworthy single-platform vertical slice".to_owned(),
            constraints: vec!["Keep authority and replay state fail-closed".to_owned()],
            preferences: vec!["Prefer the smallest reviewable playable slice".to_owned()],
            unacceptable_outcomes: vec!["Do not claim unsupported host assurance".to_owned()],
            uncertainties: vec!["Selected host conformance remains pending".to_owned()],
            conversation_ref: "conversation://p6d-c1/intent-0001".to_owned(),
            conversation_digest: sha256_content_hash(b"p6d-c1-intent-conversation-0001"),
        },
        "p6d-c1-intent-0001",
    );
    let c1_envelope_path = write_typed_json(
        &project.inputs,
        "c1-broker-intent-envelope.json",
        &c1_envelope,
    );
    let intent_record = ok(
        &project.workflow_nested(
            "action",
            "apply",
            &command_args(&["--origin-envelope-file", &path_arg(&c1_envelope_path)]),
        ),
        "workflow.action.apply",
    );

    // Core ordering remains sealed. Progress its three discovery policies
    // honestly before the appended reference discovery policy can run.
    let discover = broker_receipt_next(&intent_record);
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
    let reference = authorize_declared_policy_evidence(
        &project,
        &authority,
        &request,
        "reference.game-development.policy.discovery",
        reference,
    );
    let decision = authorize_policy_capabilities(&project, &authority, discovery_policy, reference);
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
    let discovery_ready = authorize_decision(&project, &authority, &decision, discovery_policy);
    let discovery_ready = resume_without_state_mutation(&project, &discovery_ready);
    complete_ready(&project, &discovery_ready);

    // The universal execute/release policies remain ahead of pack policies.
    // Advance them through the same admitted capabilities/evidence/decisions,
    // then exercise every reference policy to its declared readiness target.
    heartbeat(started, "workflow-execute-and-release");
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
    let (credentials, _, _) = evidence_authority(playable_evaluator.provider);
    let stale_packet = current_action_packet(&playable, "evidence", &playable_claim.id.0);
    let stale_packet_digest = stale_packet["packet_digest"]
        .as_str()
        .expect("stale evidence packet digest")
        .to_owned();
    let (stale_subject_kind, stale_subject_ref) =
        broker_subject(&playable, stale_packet, &playable_claim.id.0);
    let stale_envelope = authority.write_broker_action(
        &project,
        &playable,
        "stale-playable-evidence",
        credentials[0],
        stale_packet_digest,
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: stale_subject_kind,
            subject_ref: stale_subject_ref,
            scenario_ref: "README.md".to_owned(),
        },
    );
    let playable = authorize_policy_capabilities(&project, &authority, playable_policy, playable);
    let state_after_capability = snapshot(&project.state);
    let stale = project.workflow_nested(
        "action",
        "apply",
        &command_args(&["--origin-envelope-file", &path_arg(&stale_envelope)]),
    );
    assert_rejected_without_state_mutation(
        &project,
        &stale,
        &state_after_capability,
        "does not match current governance state",
    );
    let playable = authorize_policy_claim_evidence(
        &project,
        &authority,
        playable_policy,
        &playable_claim.id.0,
        playable,
    );
    let playable_ready =
        authorize_policy_capabilities(&project, &authority, playable_policy, playable);
    let playable_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, playable_ready);
    let playable_ready = resume_without_state_mutation(&project, &playable_ready);
    let playable_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, playable_ready);
    complete_ready_after_rechecks(&project, &authority, &core, &request, playable_ready);

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
    let first_use_partial = authorize_policy_claim_evidence(
        &project,
        &authority,
        first_use_policy,
        "reference.game-development.claim.first-use-playtest.independent-review",
        first_use,
    );
    assert_ne!(first_use_partial["data"]["status"], "ready_to_complete");
    reject_incomplete_completion(&project, &first_use_partial);
    let first_use_partial = resume_without_state_mutation(&project, &first_use_partial);
    let first_use = authorize_policy_claim_evidence(
        &project,
        &authority,
        first_use_policy,
        "reference.game-development.claim.first-use-playtest.representative-session",
        first_use_partial,
    );
    let first_use_ready =
        authorize_policy_capabilities(&project, &authority, first_use_policy, first_use);
    let first_use_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, first_use_ready);
    let first_use_ready = resume_without_state_mutation(&project, &first_use_ready);
    let first_use_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, first_use_ready);
    complete_ready_after_rechecks(&project, &authority, &core, &request, first_use_ready);

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
    let packaging_partial = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.clean-package-identity",
        packaging,
    );
    assert_ne!(packaging_partial["data"]["status"], "ready_to_complete");
    reject_incomplete_completion(&project, &packaging_partial);
    let packaging_partial = resume_without_state_mutation(&project, &packaging_partial);
    let packaging = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.installed-runtime-behavior",
        packaging_partial,
    );
    let packaging = authorize_policy_claim_evidence(
        &project,
        &authority,
        packaging_policy,
        "reference.game-development.claim.packaging.release-audit",
        packaging,
    );
    let packaging_ready =
        authorize_policy_capabilities(&project, &authority, packaging_policy, packaging);
    let packaging_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, packaging_ready);
    let packaging_ready = resume_without_state_mutation(&project, &packaging_ready);
    let packaging_ready =
        close_boundary_rechecks(&project, &authority, &core, &request, packaging_ready);
    complete_ready_after_rechecks(&project, &authority, &core, &request, packaging_ready);

    heartbeat(started, "core-rebase-plan");
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
    let active_plan_digest = required_str(&release_status, "/data/rebase_plan_digest");
    let planned = ok(
        &project.workflow(
            "release-rebase-plan",
            &command_args(&[
                "--target-release-id",
                target,
                "--expected-rebase-plan-digest",
                active_plan_digest,
            ]),
        ),
        "workflow.release_rebase_plan",
    );
    assert_eq!(
        planned["data"]["domain_pack_rebase_plan"]["mutation_allowed"],
        true
    );
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
    heartbeat(started, "remove-last-pack");
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
    let workflow_wal = project.state.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
    let replay_wal = project.state.join(WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH);
    let workflow_before_external_lifecycle =
        fs::read(&workflow_wal).expect("workflow WAL before external lifecycle advance");
    let replay_before_external_lifecycle =
        fs::read(&replay_wal).expect("replay WAL before external lifecycle advance");
    for (subcommand, command) in [
        ("preflight", "domain-pack preflight"),
        ("apply", "domain-pack apply"),
    ] {
        let mut args = vec!["domain-pack".to_owned(), subcommand.to_owned()];
        args.extend(removal_tail.clone());
        let result = ok(&run(&args), command);
        if subcommand == "apply" {
            // Lifecycle state commits first; the operator then publishes the
            // fresh runtime policies whose digests the new exact lock seals.
            publish_active_runtime_policy(&removal);
            assert_eq!(
                result["data"]["domain_pack_lifecycle_receipt"]["to_state"]["generation"], 1,
                "{result:#}"
            );
        }
    }
    assert_eq!(
        fs::read(&workflow_wal).expect("workflow WAL after external lifecycle advance"),
        workflow_before_external_lifecycle,
        "external Domain Pack lifecycle commit must leave the workflow ledger on its prior effective epoch"
    );
    assert_eq!(
        fs::read(&replay_wal).expect("replay WAL after external lifecycle advance"),
        replay_before_external_lifecycle,
        "external Domain Pack lifecycle commit must not touch broker replay state"
    );
    let historical_retry = project.workflow_nested(
        "action",
        "apply",
        &command_args(&["--origin-envelope-file", &path_arg(&c1_envelope_path)]),
    );
    assert!(
        !historical_retry.status.success(),
        "historical broker retry must refuse lifecycle/ledger effective-epoch drift"
    );
    let historical_retry = envelope(&historical_retry);
    assert_eq!(historical_retry["ok"], false, "{historical_retry:#}");
    assert!(
        historical_retry["error"]["message"]
            .as_str()
            .is_some_and(|message| message
                .contains("verified human authorization does not match current governance state")),
        "unexpected lifecycle-ahead refusal: {historical_retry:#}"
    );
    assert_eq!(
        fs::read(&workflow_wal).expect("workflow WAL after historical refusal"),
        workflow_before_external_lifecycle,
        "historical recovery must not reconcile the lifecycle by appending governance state"
    );
    assert_eq!(
        fs::read(&replay_wal).expect("replay WAL after historical refusal"),
        replay_before_external_lifecycle,
        "historical recovery must refuse before replay repair"
    );
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
    let degraded_next = resume_without_state_mutation(&project, &degraded_next);

    let degraded_claim_ref = "claim.workflow.discover-intent.intent-grounded";
    assert_eq!(
        degraded_next["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let packet = current_action_packet(&degraded_next, "evidence", degraded_claim_ref);
    assert_eq!(
        packet["binding"]["policy_ref"],
        "policy.workflow.discover-intent"
    );
    assert_eq!(packet["binding"]["subject_ref"], degraded_claim_ref);
    assert_eq!(
        packet["required_authority"]["required_grant"],
        "workflow.evidence.authorize_human"
    );
    assert!(packet["required_authority"]["accepted_roles"]
        .as_array()
        .is_some_and(|roles| roles.iter().any(|role| role == "human")));
    let packet_digest = packet["packet_digest"]
        .as_str()
        .expect("degraded evidence packet digest")
        .to_owned();
    let (subject_kind, subject_ref) = broker_subject(&degraded_next, packet, degraded_claim_ref);
    let blocked_envelope = authority.write_broker_action(
        &project,
        &degraded_next,
        "degraded-evidence-mutation",
        HUMAN_CREDENTIAL,
        packet_digest,
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind,
            subject_ref,
            scenario_ref: "README.md".to_owned(),
        },
    );
    let degraded_state = snapshot(&project.state);
    let blocked_evidence = project.workflow_nested(
        "action",
        "apply",
        &command_args(&["--origin-envelope-file", &path_arg(&blocked_envelope)]),
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

    // The degraded-empty generation rebases with the adjacent Core in one
    // joined workflow event. A fresh process reconstructs the same target pair
    // and preserves every explicit restoration gap.
    heartbeat(started, "crash-recovery");
    let degraded_release = ok(
        &project.workflow("release-status", &[]),
        "workflow.release_status",
    );
    let degraded_target = required_str(&degraded_release, "/data/available_successor/release_id");
    let degraded_plan_digest = required_str(&degraded_release, "/data/rebase_plan_digest");
    let interrupted = project.workflow_with_env(
        "release-rebase-apply",
        &command_args(&[
            "--target-release-id",
            degraded_target,
            "--expected-rebase-plan-digest",
            degraded_plan_digest,
        ]),
        Some(("FORGE_TEST_CRASH_AFTER_REBASE_LIFECYCLE", "1")),
    );
    assert!(!interrupted.status.success());
    let interrupted_stdout = String::from_utf8_lossy(&interrupted.stdout);
    let interrupted_stderr = String::from_utf8_lossy(&interrupted.stderr);
    assert_eq!(
        interrupted.status.code(),
        Some(86),
        "crash hook did not terminate at the lifecycle-first boundary; stdout={interrupted_stdout}; stderr={interrupted_stderr}"
    );
    assert!(
        interrupted_stdout.contains("injected crash after lifecycle commit")
            || interrupted_stderr.contains("injected crash after lifecycle commit"),
        "unexpected injected-crash response; status={:?}; stdout={interrupted_stdout}; stderr={interrupted_stderr}",
        interrupted.status.code()
    );
    let replacement_status = ok(
        &project.workflow("release-status", &[]),
        "workflow.release_status",
    );
    assert_eq!(
        replacement_status["data"]["active"]["release"]["release_id"],
        degraded_target
    );
    assert_eq!(replacement_status["data"]["domain_pack_degraded"], true);
    assert_eq!(
        replacement_status["data"]["domain_pack_rebase_required"],
        true
    );
    let replacement_next = ok(&project.workflow("next", &[]), "workflow.next");
    assert_eq!(replacement_next["data"]["domain_pack_degraded"], true);
    assert_eq!(
        replacement_next["data"]["domain_pack_gaps"],
        degraded_next["data"]["domain_pack_gaps"]
    );
    heartbeat(started, "complete");
}
