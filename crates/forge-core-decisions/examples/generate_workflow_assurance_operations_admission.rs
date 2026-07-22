use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    PrincipalId, RepoPath, StableId, WorkflowBehavioralArtifactReference,
    WorkflowBehavioralCorpusSetDocument, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralReviewSubjectDocument, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralScenarioExecution, WorkflowBehavioralShadowReportDocument,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowGovernanceReleaseRegistryEntry, WorkflowMigrationBatchDocument,
    WorkflowMigrationPlanDocument, WorkflowReceiptCarryover,
    WorkflowReleaseAdmissionAuthorizationPayloadV2, WorkflowReleaseAdmissionAuthorizationV2,
    WorkflowReleaseAdmissionAuthorizationV2Authority,
    WorkflowReleaseAdmissionAuthorizationV2Document, WorkflowReleaseAdmissionSignatureAlgorithm,
    WorkflowReleaseAdmissionSignatureV2, WorkflowReleasePredecessorReference,
    WorkflowReleaseRegistryAuthority, WorkflowReleaseRegistrySource,
    WorkflowReleaseReviewArtifactBindingV2, WorkflowReleaseReviewDecision,
    WorkflowReleaseReviewDimensionDecisionV2, WorkflowReleaseReviewIndexV2,
    WorkflowReleaseReviewIndexV2Authority, WorkflowReleaseReviewIndexV2Document,
    WorkflowReleaseReviewQuarantineDecisionV2, WorkflowReleaseReviewWorkflowDecisionV2,
    WorkflowReleaseReviewerCredential, WorkflowReleaseReviewerCredentialStatus,
    WorkflowReleaseReviewerRegistry, WorkflowReleaseReviewerRegistryAuthority,
    WorkflowReleaseReviewerRegistryDocument, WorkflowReleaseReviewerRole,
    WorkflowRuntimeBundleIdentity, WorkflowRuntimeBundleReference,
    WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_workflow_migration, evaluate_workflow_release_admission_candidate_v2, load_catalog,
    load_workflow_documents, workflow_policy_set_digest, workflow_release_manifest_digest,
    workflow_runtime_bundle_digest, WorkflowBehavioralBundleInput,
    WorkflowBehavioralReportIdentity, WorkflowMigrationAuditStatus,
    WorkflowReleaseAdmissionCandidateV2Input, WorkflowReleaseAdmissionV2Evaluation,
    WorkflowReleaseAdmissionV2EvaluationStatus,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

const PROMOTED_BUNDLE: &str = "contracts/workflow-governance/runtime-assurance-operations-v0.yaml";
const PROPOSED_REGISTRY: &str =
    "contracts/migration/workflow-governance-release-registry-assurance-operations-v0.yaml";
const REVIEW_INDEX: &str = "contracts/migration/workflow-assurance-operations-review-index-v0.yaml";
const REVIEWER_REGISTRY: &str =
    "contracts/policies/workflow-release-reviewer-registry-assurance-operations-v0.yaml";
const AUTHORIZATION: &str =
    "contracts/migration/workflow-assurance-operations-admission-authorization-v0.yaml";
const INDEPENDENT_REVIEW: &str =
    "contracts/evidence/workflow-assurance-operations-independent-review-v0.yaml";
const REVIEW_SIGNED_AT_UNIX: u64 = 1_783_951_200;
const SEMANTIC_SIGNATURE: &str =
    "5ceaedb8da041609cadb3a3a44e070fa6a55087bba16f5d8f1b9677a57855a6d033cd35a35a9b9fbe904e0bb80950f7b715435745511308cfef2241e0538b508";
const AUTHORIZER_SIGNATURE: &str =
    "370a436638bdda7cbd9b92c71283bf2c22b1849ce010899ba65ad469c31250abd6b1a779a8a169f2ec9a76f20b9b7765106a0794d9cddfc2507d8a7a8aae750f";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

struct GeneratedArtifact {
    relative_path: &'static str,
    bytes: Vec<u8>,
}

#[derive(Serialize)]
struct IndependentReviewDocument<'a> {
    schema_version: &'static str,
    workflow_release_admission_evaluation: &'a WorkflowReleaseAdmissionV2Evaluation,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = parse_mode()?;
    let input = fixture();
    let reviewer_registry = reviewer_registry();
    validate_reviewer_registry(&reviewer_registry)?;
    let evaluation = evaluate_workflow_release_admission_candidate_v2(&input);
    if evaluation.status
        != WorkflowReleaseAdmissionV2EvaluationStatus::ReadyForIndependentAuthorization
        || !evaluation.issues.is_empty()
    {
        return Err(format!(
            "candidate admission review is blocked: {:#?}",
            evaluation.issues
        )
        .into());
    }
    let review = IndependentReviewDocument {
        schema_version: "0.2",
        workflow_release_admission_evaluation: &evaluation,
    };
    let review_index_bytes = yaml_bytes(&input.review_index);
    let reviewer_registry_bytes = yaml_bytes(&reviewer_registry);
    let authorization = authorization(
        &input,
        &evaluation,
        &reviewer_registry,
        &review_index_bytes,
        &reviewer_registry_bytes,
    );
    let authorization_issues = authorization.validate();
    if !authorization_issues.is_empty() {
        return Err(format!("authorization contract is invalid: {authorization_issues:#?}").into());
    }
    let artifacts = vec![
        GeneratedArtifact {
            relative_path: PROMOTED_BUNDLE,
            bytes: yaml_bytes(&input.promoted_runtime_bundle),
        },
        GeneratedArtifact {
            relative_path: PROPOSED_REGISTRY,
            bytes: yaml_bytes(&input.proposed_registry),
        },
        GeneratedArtifact {
            relative_path: REVIEWER_REGISTRY,
            bytes: reviewer_registry_bytes,
        },
        GeneratedArtifact {
            relative_path: REVIEW_INDEX,
            bytes: review_index_bytes,
        },
        GeneratedArtifact {
            relative_path: INDEPENDENT_REVIEW,
            bytes: yaml_bytes(&review),
        },
        GeneratedArtifact {
            relative_path: AUTHORIZATION,
            bytes: yaml_bytes(&authorization),
        },
    ];
    match mode {
        Mode::Write => write_artifacts(&artifacts)?,
        Mode::Check => check_artifacts(&artifacts)?,
    }
    Ok(())
}

fn validate_reviewer_registry(
    document: &WorkflowReleaseReviewerRegistryDocument,
) -> Result<(), Box<dyn std::error::Error>> {
    let issues = document.validate();
    if !issues.is_empty() {
        return Err(format!("reviewer registry is invalid: {issues:#?}").into());
    }
    let credentials = &document.workflow_release_reviewer_registry.credentials;
    if credentials.len() != 2
        || credentials[0].principal_id == credentials[1].principal_id
        || credentials[0].credential_id == credentials[1].credential_id
        || credentials[0].public_key_fingerprint == credentials[1].public_key_fingerprint
        || credentials[0].independence_domain == credentials[1].independence_domain
    {
        return Err("reviewer quorum must use two distinct custodians".into());
    }
    for credential in credentials {
        let key = decode_hex(&credential.public_key_hex)?;
        if sha256(&key) != credential.public_key_fingerprint {
            return Err(format!(
                "reviewer fingerprint mismatch for {}",
                credential.credential_id.0
            )
            .into());
        }
        if !(credential.valid_from_unix <= REVIEW_SIGNED_AT_UNIX
            && REVIEW_SIGNED_AT_UNIX <= credential.valid_until_unix)
        {
            return Err(format!(
                "review time is outside credential window for {}",
                credential.credential_id.0
            )
            .into());
        }
    }
    Ok(())
}

#[allow(clippy::manual_is_multiple_of)] // `is_multiple_of` is unstable on the Rust 1.85 MSRV.
fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if value.len() % 2 != 0 {
        return Err("hex value must have even length".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}

fn parse_mode() -> Result<Mode, Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--write" => Ok(Mode::Write),
        [flag] if flag == "--check" => Ok(Mode::Check),
        _ => Err("usage: cargo run -p forge-core-decisions --example generate_workflow_assurance_operations_admission -- (--write|--check)".into()),
    }
}

fn write_artifacts(artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn std::error::Error>> {
    for artifact in artifacts {
        let destination = root().join(artifact.relative_path);
        std::fs::create_dir_all(destination.parent().expect("artifact parent"))?;
        std::fs::write(&destination, &artifact.bytes)?;
        println!("wrote {}", artifact.relative_path);
    }
    Ok(())
}

fn check_artifacts(artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn std::error::Error>> {
    for artifact in artifacts {
        let actual = std::fs::read(root().join(artifact.relative_path)).map_err(|error| {
            format!(
                "missing generated artifact {}: {error}",
                artifact.relative_path
            )
        })?;
        if actual != artifact.bytes {
            return Err(format!("generated artifact is stale: {}", artifact.relative_path).into());
        }
        println!("checked {}", artifact.relative_path);
    }
    Ok(())
}

fn reviewer_registry() -> WorkflowReleaseReviewerRegistryDocument {
    WorkflowReleaseReviewerRegistryDocument {
        schema_version: WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION.to_owned(),
        workflow_release_reviewer_registry: WorkflowReleaseReviewerRegistry {
            registry_id: id("workflow-release-reviewers.assurance-operations-v0"),
            registry_version: "0.2.0".to_owned(),
            authority: WorkflowReleaseReviewerRegistryAuthority::CandidateOnly,
            credentials: vec![
                WorkflowReleaseReviewerCredential {
                    credential_id: id("reviewer.workflow-release.semantic-assurance-operations-v0"),
                    principal_id: PrincipalId(
                        "principal.release-review.semantic-assurance-operations-v0".to_owned(),
                    ),
                    public_key_fingerprint:
                        "sha256:4bb6db66f1dcf3c3489e0d4a20240565b1a5698c38a0b086a363eb560a2ea98c"
                            .to_owned(),
                    public_key_hex:
                        "a67886dd3e90364874750e995508f6d73b6f0e672ada37eeebca979f007737e1"
                            .to_owned(),
                    algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
                    roles: vec![WorkflowReleaseReviewerRole::SemanticReviewer],
                    status: WorkflowReleaseReviewerCredentialStatus::Active,
                    valid_from_unix: 1_783_778_400,
                    valid_until_unix: 1_815_314_400,
                    independence_domain: "semantic-review".to_owned(),
                },
                WorkflowReleaseReviewerCredential {
                    credential_id: id(
                        "reviewer.workflow-release.authorizer-assurance-operations-v0",
                    ),
                    principal_id: PrincipalId(
                        "principal.release-review.authorizer-assurance-operations-v0".to_owned(),
                    ),
                    public_key_fingerprint:
                        "sha256:229f1f3d5b72320eb55c05e01177da22ddc7c7943cee4d483a661a09f9ff5250"
                            .to_owned(),
                    public_key_hex:
                        "a1dd6a040b530179a0b15d59a1c4971fced185ef04ca4d34cba707215b6aaecf"
                            .to_owned(),
                    algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
                    roles: vec![WorkflowReleaseReviewerRole::ReleaseAuthorizer],
                    status: WorkflowReleaseReviewerCredentialStatus::Active,
                    valid_from_unix: 1_783_778_400,
                    valid_until_unix: 1_815_314_400,
                    independence_domain: "release-authorization".to_owned(),
                },
            ],
        },
    }
}

fn authorization(
    input: &WorkflowReleaseAdmissionCandidateV2Input,
    evaluation: &WorkflowReleaseAdmissionV2Evaluation,
    reviewer_registry: &WorkflowReleaseReviewerRegistryDocument,
    review_index_bytes: &[u8],
    reviewer_registry_bytes: &[u8],
) -> WorkflowReleaseAdmissionAuthorizationV2Document {
    let index = &input.review_index.workflow_release_review_index;
    let registry = &reviewer_registry.workflow_release_reviewer_registry;
    let payload = WorkflowReleaseAdmissionAuthorizationPayloadV2 {
        authorization_id: id("workflow-release-admission.assurance-operations-v0"),
        review_index_id: index.id.clone(),
        review_index_version: index.index_version.clone(),
        review_index_raw_digest: sha256(review_index_bytes),
        review_index_canonical_digest: canonical(&input.review_index),
        evaluation_digest: evaluation.evaluation_digest.clone(),
        reviewer_registry_id: registry.registry_id.clone(),
        reviewer_registry_version: registry.registry_version.clone(),
        reviewer_registry_raw_digest: sha256(reviewer_registry_bytes),
        reviewer_registry_canonical_digest: canonical(reviewer_registry),
        promotion: index.promotion.clone(),
        release_manifest: index.release_manifest.clone(),
        review_subject: index.review_subject.clone(),
        full_catalog: index.full_catalog.clone(),
        predecessor_registry: index.predecessor_registry.clone(),
        proposed_registry: index.proposed_registry.clone(),
        invalidate_all_receipts: true,
        workflow_decisions: index.workflow_decisions.clone(),
        quarantine_decisions: index.quarantine_decisions.clone(),
        dimension_decisions: index.dimension_decisions.clone(),
        audience: "forge-core:workflow-release-admission:embedded".to_owned(),
        domain: "forge-method:workflow-release-admission:v2".to_owned(),
        nonce: "release-admission.assurance-operations-v0".to_owned(),
        issued_at_unix: REVIEW_SIGNED_AT_UNIX,
        expires_at_unix: 1_815_314_400,
    };
    let payload_digest = canonical(&payload);
    let credentials = &registry.credentials;
    let signature = |credential_index: usize, role: WorkflowReleaseReviewerRole, bytes: &str| {
        WorkflowReleaseAdmissionSignatureV2 {
            principal_id: credentials[credential_index].principal_id.clone(),
            credential_id: credentials[credential_index].credential_id.clone(),
            role,
            algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
            payload_digest: payload_digest.clone(),
            signature: bytes.to_owned(),
            signed_at_unix: REVIEW_SIGNED_AT_UNIX,
        }
    };
    WorkflowReleaseAdmissionAuthorizationV2Document {
        schema_version: WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_V2_SCHEMA_VERSION.to_owned(),
        workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorizationV2 {
            authority: WorkflowReleaseAdmissionAuthorizationV2Authority::CandidateAuthorization,
            payload,
            signatures: vec![
                signature(
                    0,
                    WorkflowReleaseReviewerRole::SemanticReviewer,
                    SEMANTIC_SIGNATURE,
                ),
                signature(
                    1,
                    WorkflowReleaseReviewerRole::ReleaseAuthorizer,
                    AUTHORIZER_SIGNATURE,
                ),
            ],
        },
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn path(value: &str) -> RepoPath {
    RepoPath(value.to_owned())
}

fn load<T: DeserializeOwned>(relative: &str) -> T {
    yaml_serde::from_str(&std::fs::read_to_string(root().join(relative)).expect("fixture source"))
        .expect("fixture YAML")
}

fn yaml_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    yaml_serde::to_string(value)
        .expect("serialize YAML")
        .into_bytes()
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    sha256(&bytes)
}

fn binding<T: Serialize>(
    artifact_id: &str,
    relative: &str,
    document: &T,
) -> WorkflowReleaseReviewArtifactBindingV2 {
    WorkflowReleaseReviewArtifactBindingV2 {
        artifact_id: id(artifact_id),
        embedded_ref: path(relative),
        raw_digest: sha256(&std::fs::read(root().join(relative)).expect("raw artifact")),
        canonical_digest: canonical(document),
    }
}

fn source_binding(
    artifact_id: &str,
    relative: &str,
    bytes: &[u8],
    canonical_digest: String,
) -> WorkflowReleaseReviewArtifactBindingV2 {
    WorkflowReleaseReviewArtifactBindingV2 {
        artifact_id: id(artifact_id),
        embedded_ref: path(relative),
        raw_digest: sha256(bytes),
        canonical_digest,
    }
}

fn add_files(directory: &Path, sources: &mut HashMap<RepoPath, Vec<u8>>) {
    for entry in std::fs::read_dir(directory).expect("read fixture directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            add_files(&path, sources);
        } else if let Ok(relative) = path.strip_prefix(root()) {
            sources.insert(
                RepoPath(relative.to_string_lossy().replace('\\', "/")),
                std::fs::read(&path).expect("fixture bytes"),
            );
        }
    }
}

fn collect_bundle_refs(corpus: &WorkflowBehavioralScenarioCorpusDocument) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for scenario in corpus
        .workflow_behavioral_scenario_corpus
        .workflow_evidence
        .iter()
        .flat_map(|workflow| &workflow.scenarios)
    {
        match &scenario.execution {
            WorkflowBehavioralScenarioExecution::Single { input, .. } => {
                refs.insert(input.bundle.embedded_ref.0.clone());
            }
            WorkflowBehavioralScenarioExecution::Resume {
                checkpoint_input,
                resumed_input,
                ..
            } => {
                refs.insert(checkpoint_input.bundle.embedded_ref.0.clone());
                refs.insert(resumed_input.bundle.embedded_ref.0.clone());
            }
            WorkflowBehavioralScenarioExecution::Ablation {
                control_input,
                ablated_input,
                ..
            } => {
                refs.insert(control_input.bundle.embedded_ref.0.clone());
                refs.insert(ablated_input.bundle.embedded_ref.0.clone());
            }
        }
    }
    refs
}

#[allow(clippy::too_many_lines)]
fn fixture() -> WorkflowReleaseAdmissionCandidateV2Input {
    const COVERAGE: &str =
        "contracts/policies/workflow-behavioral-coverage-assurance-operations-v0.yaml";
    const CORPUS_SET: &str = "contracts/evidence/workflow-assurance-operations-corpus-set-v0.yaml";
    const REPRESENTATIVE: &str =
        "contracts/evidence/workflow-assurance-operations-representative-v0.yaml";
    const ADVERSARIAL: &str =
        "contracts/evidence/workflow-assurance-operations-adversarial-v0.yaml";
    const SUBJECT: &str =
        "contracts/migration/workflow-assurance-operations-review-subject-v0.yaml";
    const REPORT: &str = "contracts/evidence/workflow-assurance-operations-shadow-report-v0.yaml";
    const MIGRATION_PLAN: &str = "contracts/policies/workflow-migration-foundation-v0.yaml";
    const GOLDEN_BATCH: &str = "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";
    const CORE_BATCH: &str = "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml";
    const ASSURANCE_BATCH: &str =
        "contracts/migration/workflow-governance-batch-assurance-operations-v0.yaml";
    const MANIFEST: &str =
        "contracts/migration/workflow-governance-release-assurance-operations-candidate-v0.yaml";
    const CANDIDATE_BUNDLE: &str =
        "contracts/workflow-governance/runtime-assurance-operations-candidate-v0.yaml";
    const PREDECESSOR_REGISTRY: &str =
        "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml";
    const PROMOTED_BUNDLE: &str =
        "contracts/workflow-governance/runtime-assurance-operations-v0.yaml";
    const PROPOSED_REGISTRY: &str =
        "contracts/migration/workflow-governance-release-registry-assurance-operations-v0.yaml";
    const EVALUATOR: &str = "crates/forge-core-decisions/src/workflow_behavior.rs";
    const HISTORY: &str = "contracts/evidence/workflow-core-assurance-frozen-history-v0.ndjson";

    let coverage: WorkflowBehavioralCoveragePolicyDocument = load(COVERAGE);
    let corpus_set: WorkflowBehavioralCorpusSetDocument = load(CORPUS_SET);
    let representative: WorkflowBehavioralScenarioCorpusDocument = load(REPRESENTATIVE);
    let adversarial: WorkflowBehavioralScenarioCorpusDocument = load(ADVERSARIAL);
    let subject: WorkflowBehavioralReviewSubjectDocument = load(SUBJECT);
    let report: WorkflowBehavioralShadowReportDocument = load(REPORT);
    let migration_plan: WorkflowMigrationPlanDocument = load(MIGRATION_PLAN);
    let golden_batch: WorkflowMigrationBatchDocument = load(GOLDEN_BATCH);
    let core_batch: WorkflowMigrationBatchDocument = load(CORE_BATCH);
    let assurance_batch: WorkflowMigrationBatchDocument = load(ASSURANCE_BATCH);
    let manifest: WorkflowGovernanceReleaseManifestDocument = load(MANIFEST);
    let candidate_bundle: WorkflowGovernanceBundleDocument = load(CANDIDATE_BUNDLE);
    let predecessor_registry: WorkflowGovernanceReleaseRegistryDocument =
        load(PREDECESSOR_REGISTRY);
    let loaded = load_workflow_documents(
        &root().join("contracts/evidence/workflow-retirement/legacy-catalog"),
    );
    assert!(
        loaded.is_clean(),
        "legacy workflow inventory must parse cleanly"
    );
    let legacy_workflows = loaded.workflows;
    let catalog =
        load_catalog(&root().join("contracts/evidence/workflow-retirement/legacy-catalog"));
    assert!(
        catalog.is_clean(),
        "legacy routing catalog must parse cleanly"
    );
    let migration_audit =
        evaluate_workflow_migration(&migration_plan, &legacy_workflows, &catalog.catalog);
    assert_eq!(
        migration_audit.status,
        WorkflowMigrationAuditStatus::ReadyForShadow
    );
    assert!(migration_audit.issues.is_empty());

    let mut promoted_bundle = candidate_bundle.clone();
    promoted_bundle.workflow_governance_bundle.id =
        id("bundle.workflow-governance.assurance-operations-v0");
    let promoted_bundle_bytes = yaml_bytes(&promoted_bundle);
    let promoted_identity = WorkflowRuntimeBundleIdentity {
        bundle_id: promoted_bundle.workflow_governance_bundle.id.clone(),
        bundle_digest: workflow_runtime_bundle_digest(&promoted_bundle).expect("bundle digest"),
        policy_set_digest: workflow_policy_set_digest(
            &promoted_bundle.workflow_governance_bundle.policies,
        )
        .expect("policy set digest"),
    };
    let manifest_digest = workflow_release_manifest_digest(&manifest).expect("manifest digest");
    let manifest_subject = &manifest.workflow_governance_release_manifest;
    let candidate_release = WorkflowGovernanceReleaseIdentity {
        lineage_id: manifest_subject.lineage_id.clone(),
        release_id: manifest_subject.release_id.clone(),
        release_version: manifest_subject.release_version.clone(),
        release_digest: manifest_digest,
    };
    let predecessor_entry = predecessor_registry
        .workflow_governance_release_registry
        .releases
        .last()
        .expect("core-assurance release");
    let predecessor = WorkflowReleasePredecessorReference {
        release_id: predecessor_entry.release.release_id.clone(),
        release_digest: predecessor_entry.release.release_digest.clone(),
    };
    let manifest_bytes = std::fs::read(root().join(MANIFEST)).expect("manifest bytes");
    let mut proposed_registry = predecessor_registry.clone();
    "0.3.0".clone_into(
        &mut proposed_registry
            .workflow_governance_release_registry
            .registry_version,
    );
    proposed_registry
        .workflow_governance_release_registry
        .default_successor_release_id = candidate_release.release_id.clone();
    proposed_registry
        .workflow_governance_release_registry
        .releases
        .push(WorkflowGovernanceReleaseRegistryEntry {
            release: candidate_release.clone(),
            runtime_bundle: WorkflowRuntimeBundleReference {
                identity: promoted_identity.clone(),
                embedded_ref: path(PROMOTED_BUNDLE),
                expected_digest: sha256(&promoted_bundle_bytes),
            },
            predecessor: Some(predecessor.clone()),
            source: WorkflowReleaseRegistrySource::EmbeddedManifest {
                embedded_ref: path(MANIFEST),
                expected_digest: sha256(&manifest_bytes),
            },
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            authority: WorkflowReleaseRegistryAuthority::CandidateOnly,
        });
    let proposed_registry_bytes = yaml_bytes(&proposed_registry);

    let mut sources = HashMap::new();
    add_files(&root().join("contracts"), &mut sources);
    sources.extend(
        forge_core_decisions::catalog::embedded_frozen_legacy_workflow_source_bytes()
            .into_iter()
            .map(|(path, bytes)| (path, bytes.to_vec())),
    );
    for relative in [EVALUATOR, HISTORY] {
        sources.insert(
            path(relative),
            std::fs::read(root().join(relative)).expect("source bytes"),
        );
    }
    sources.insert(path(PROMOTED_BUNDLE), promoted_bundle_bytes.clone());
    sources.insert(path(PROPOSED_REGISTRY), proposed_registry_bytes.clone());

    let mut behavioral_bundles = BTreeMap::new();
    for relative in collect_bundle_refs(&representative)
        .into_iter()
        .chain(collect_bundle_refs(&adversarial))
    {
        let document: WorkflowGovernanceBundleDocument = load(&relative);
        let artifact = WorkflowBehavioralArtifactReference {
            id: document.workflow_governance_bundle.id.clone(),
            embedded_ref: path(&relative),
            expected_digest: sha256(sources.get(&path(&relative)).expect("bundle source")),
        };
        behavioral_bundles.insert(
            workflow_runtime_bundle_digest(&document).expect("canonical bundle"),
            WorkflowBehavioralBundleInput { artifact, document },
        );
    }

    let evaluator_bytes = sources.get(&path(EVALUATOR)).expect("evaluator bytes");
    let history_bytes = sources.get(&path(HISTORY)).expect("history bytes");
    let history_values = std::str::from_utf8(history_bytes)
        .expect("history UTF-8")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| yaml_serde::from_str::<yaml_serde::Value>(line).expect("history JSON"))
        .collect::<Vec<_>>();
    let proposed_registry_binding = WorkflowReleaseReviewArtifactBindingV2 {
        artifact_id: id("workflow-governance.registry.assurance-operations-v0"),
        embedded_ref: path(PROPOSED_REGISTRY),
        raw_digest: sha256(&proposed_registry_bytes),
        canonical_digest: canonical(&proposed_registry),
    };
    let promoted_bundle_binding = WorkflowReleaseReviewArtifactBindingV2 {
        artifact_id: promoted_bundle.workflow_governance_bundle.id.clone(),
        embedded_ref: path(PROMOTED_BUNDLE),
        raw_digest: sha256(&promoted_bundle_bytes),
        canonical_digest: canonical(&promoted_bundle),
    };
    let index = WorkflowReleaseReviewIndexV2Document {
        schema_version: WORKFLOW_RELEASE_REVIEW_INDEX_V2_SCHEMA_VERSION.to_owned(),
        workflow_release_review_index: WorkflowReleaseReviewIndexV2 {
            id: id("workflow-release-review.assurance-operations-v0"),
            index_version: "0.2.0".to_owned(),
            authority: WorkflowReleaseReviewIndexV2Authority::CandidateOnly,
            promotion: forge_core_contracts::WorkflowReleasePromotionBindingV2 {
                predecessor,
                candidate_release,
                candidate_runtime_bundle: WorkflowRuntimeBundleIdentity {
                    bundle_id: subject
                        .workflow_behavioral_review_subject
                        .runtime_bundle
                        .bundle_id
                        .clone(),
                    bundle_digest: subject
                        .workflow_behavioral_review_subject
                        .runtime_bundle
                        .bundle_digest
                        .clone(),
                    policy_set_digest: subject
                        .workflow_behavioral_review_subject
                        .runtime_bundle
                        .policy_set_digest
                        .clone(),
                },
                promoted_runtime_bundle: promoted_identity,
            },
            release_manifest: binding(
                "release-manifest.assurance-operations-v0",
                MANIFEST,
                &manifest,
            ),
            migration_batches: vec![
                binding(
                    "migration-batch.golden-path-v0",
                    GOLDEN_BATCH,
                    &golden_batch,
                ),
                binding("migration-batch.core-assurance-v0", CORE_BATCH, &core_batch),
                binding(
                    "migration-batch.assurance-operations-v0",
                    ASSURANCE_BATCH,
                    &assurance_batch,
                ),
            ],
            review_subject: binding("review-subject.assurance-operations-v0", SUBJECT, &subject),
            coverage_policy: binding(
                "coverage-policy.assurance-operations-v0",
                COVERAGE,
                &coverage,
            ),
            full_catalog: binding(
                "full-catalog.foundation-v0",
                MIGRATION_PLAN,
                &migration_plan,
            ),
            corpus_set: binding(
                "corpus-set.assurance-operations-v0",
                CORPUS_SET,
                &corpus_set,
            ),
            representative_corpus: binding(
                &representative.workflow_behavioral_scenario_corpus.id.0,
                REPRESENTATIVE,
                &representative,
            ),
            adversarial_corpus: binding(
                &adversarial.workflow_behavioral_scenario_corpus.id.0,
                ADVERSARIAL,
                &adversarial,
            ),
            shadow_report: binding("shadow-report.assurance-operations-v0", REPORT, &report),
            candidate_runtime_bundle: binding(
                "runtime-bundle.assurance-operations-candidate-v0",
                CANDIDATE_BUNDLE,
                &candidate_bundle,
            ),
            promoted_runtime_bundle: promoted_bundle_binding,
            predecessor_registry: binding(
                "registry.core-assurance-v0",
                PREDECESSOR_REGISTRY,
                &predecessor_registry,
            ),
            proposed_registry: proposed_registry_binding,
            evaluator_source: source_binding(
                "evaluator.workflow-behavior-v0",
                EVALUATOR,
                evaluator_bytes,
                canonical(
                    &std::str::from_utf8(evaluator_bytes)
                        .expect("evaluator UTF-8")
                        .to_owned(),
                ),
            ),
            frozen_history: source_binding(
                "history.workflow-governance.p5d2-foundation-compatibility-v0",
                HISTORY,
                history_bytes,
                canonical(&history_values),
            ),
            workflow_decisions: subject
                .workflow_behavioral_review_subject
                .candidate_workflows
                .iter()
                .map(|workflow| WorkflowReleaseReviewWorkflowDecisionV2 {
                    workflow_id: workflow.workflow_id.clone(),
                    decision: WorkflowReleaseReviewDecision::Approved,
                    rationale: "independent semantic review passed".to_owned(),
                    finding_refs: Vec::new(),
                })
                .collect(),
            quarantine_decisions: subject
                .workflow_behavioral_review_subject
                .quarantines
                .iter()
                .map(|quarantine| WorkflowReleaseReviewQuarantineDecisionV2 {
                    workflow_id: quarantine.workflow_id.clone(),
                    decision: WorkflowReleaseReviewDecision::Approved,
                    rationale: "quarantine remains required".to_owned(),
                    finding_refs: Vec::new(),
                })
                .collect(),
            dimension_decisions: forge_core_contracts::WorkflowGovernedOutcomeDimension::all()
                .into_iter()
                .map(|dimension| WorkflowReleaseReviewDimensionDecisionV2 {
                    dimension,
                    decision: WorkflowReleaseReviewDecision::Approved,
                    rationale: "governed projection reviewed".to_owned(),
                    finding_refs: Vec::new(),
                })
                .collect(),
        },
    };
    let report_data = &report.workflow_behavioral_shadow_report;
    let report_identity = WorkflowBehavioralReportIdentity {
        report_id: report_data.id.clone(),
        report_version: report_data.report_version.clone(),
        corpus_set: WorkflowBehavioralArtifactReference {
            id: corpus_set.workflow_behavioral_corpus_set.id.clone(),
            embedded_ref: path(CORPUS_SET),
            expected_digest: sha256(sources.get(&path(CORPUS_SET)).expect("corpus set bytes")),
        },
        coverage_policy: WorkflowBehavioralArtifactReference {
            id: coverage.workflow_behavioral_coverage_policy.id.clone(),
            embedded_ref: path(COVERAGE),
            expected_digest: sha256(sources.get(&path(COVERAGE)).expect("coverage bytes")),
        },
    };
    let registry_promoted_bundle = promoted_bundle.clone();
    WorkflowReleaseAdmissionCandidateV2Input {
        review_index: index,
        report_identity,
        coverage_policy: coverage,
        corpus_set,
        representative_corpus: representative,
        adversarial_corpus: adversarial,
        review_subject: subject,
        behavioral_bundles,
        authored_shadow_report: report,
        migration_batches: vec![golden_batch, core_batch, assurance_batch],
        migration_plan,
        migration_audit,
        legacy_workflows,
        candidate_manifest: manifest,
        candidate_runtime_bundle: candidate_bundle,
        promoted_runtime_bundle: promoted_bundle,
        predecessor_registry,
        proposed_registry,
        registry_bundles: vec![
            load("contracts/workflow-governance/golden-path-v0.yaml"),
            load("contracts/workflow-governance/runtime-release-foundation-v0.yaml"),
            load("contracts/workflow-governance/runtime-core-assurance-v0.yaml"),
            registry_promoted_bundle,
        ],
        source_bytes: sources,
    }
}
