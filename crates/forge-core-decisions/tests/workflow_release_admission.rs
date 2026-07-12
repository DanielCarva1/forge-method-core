use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicyDocument, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralScenarioCorpusDocument, WorkflowBehavioralScenarioExecution,
    WorkflowBehavioralShadowReportDocument, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceReleaseIdentity, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowGovernanceReleaseRegistryEntry,
    WorkflowMigrationBatchDocument, WorkflowReceiptCarryover, WorkflowReleasePredecessorReference,
    WorkflowReleaseRegistryAuthority, WorkflowReleaseRegistrySource,
    WorkflowReleaseReviewArtifactBinding, WorkflowReleaseReviewDecision,
    WorkflowReleaseReviewDimensionDecision, WorkflowReleaseReviewIndex,
    WorkflowReleaseReviewIndexAuthority, WorkflowReleaseReviewIndexDocument,
    WorkflowReleaseReviewQuarantineDecision, WorkflowReleaseReviewWorkflowDecision,
    WorkflowRuntimeBundleIdentity, WorkflowRuntimeBundleReference,
    WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_workflow_release_admission_candidate, workflow_policy_set_digest,
    workflow_release_manifest_digest, workflow_runtime_bundle_digest,
    WorkflowBehavioralBundleInput, WorkflowBehavioralReportIdentity,
    WorkflowReleaseAdmissionCandidateInput, WorkflowReleaseAdmissionEvaluationAuthority,
    WorkflowReleaseAdmissionEvaluationStatus,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

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
) -> WorkflowReleaseReviewArtifactBinding {
    WorkflowReleaseReviewArtifactBinding {
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
) -> WorkflowReleaseReviewArtifactBinding {
    WorkflowReleaseReviewArtifactBinding {
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
fn fixture() -> WorkflowReleaseAdmissionCandidateInput {
    const COVERAGE: &str = "contracts/policies/workflow-behavioral-coverage-v0.yaml";
    const CORPUS_SET: &str = "contracts/evidence/workflow-core-assurance-corpus-set-v0.yaml";
    const REPRESENTATIVE: &str =
        "contracts/evidence/workflow-core-assurance-representative-v0.yaml";
    const ADVERSARIAL: &str = "contracts/evidence/workflow-core-assurance-adversarial-v0.yaml";
    const SUBJECT: &str = "contracts/migration/workflow-core-assurance-review-subject-v0.yaml";
    const REPORT: &str = "contracts/evidence/workflow-core-assurance-shadow-report-v0.yaml";
    const GOLDEN_BATCH: &str = "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";
    const CORE_BATCH: &str = "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml";
    const MANIFEST: &str =
        "contracts/migration/workflow-governance-release-core-assurance-candidate-v0.yaml";
    const CANDIDATE_BUNDLE: &str =
        "contracts/workflow-governance/runtime-core-assurance-candidate-v0.yaml";
    const PREDECESSOR_REGISTRY: &str =
        "contracts/migration/workflow-governance-release-registry-v0.yaml";
    const PROMOTED_BUNDLE: &str = "contracts/workflow-governance/runtime-core-assurance-v0.yaml";
    const PROPOSED_REGISTRY: &str =
        "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml";
    const EVALUATOR: &str = "crates/forge-core-decisions/src/workflow_behavior.rs";
    const HISTORY: &str = "crates/forge-core-kernel/tests/fixtures/p5d2-foundation-history.ndjson";

    let coverage: WorkflowBehavioralCoveragePolicyDocument = load(COVERAGE);
    let corpus_set: WorkflowBehavioralCorpusSetDocument = load(CORPUS_SET);
    let representative: WorkflowBehavioralScenarioCorpusDocument = load(REPRESENTATIVE);
    let adversarial: WorkflowBehavioralScenarioCorpusDocument = load(ADVERSARIAL);
    let subject: WorkflowBehavioralReviewSubjectDocument = load(SUBJECT);
    let report: WorkflowBehavioralShadowReportDocument = load(REPORT);
    let golden_batch: WorkflowMigrationBatchDocument = load(GOLDEN_BATCH);
    let core_batch: WorkflowMigrationBatchDocument = load(CORE_BATCH);
    let manifest: WorkflowGovernanceReleaseManifestDocument = load(MANIFEST);
    let candidate_bundle: WorkflowGovernanceBundleDocument = load(CANDIDATE_BUNDLE);
    let predecessor_registry: WorkflowGovernanceReleaseRegistryDocument =
        load(PREDECESSOR_REGISTRY);

    let mut promoted_bundle = candidate_bundle.clone();
    promoted_bundle.workflow_governance_bundle.id =
        id("bundle.workflow-governance.core-assurance-v0");
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
        .expect("foundation release");
    let predecessor = WorkflowReleasePredecessorReference {
        release_id: predecessor_entry.release.release_id.clone(),
        release_digest: predecessor_entry.release.release_digest.clone(),
    };
    let manifest_bytes = std::fs::read(root().join(MANIFEST)).expect("manifest bytes");
    let mut proposed_registry = predecessor_registry.clone();
    "0.2.0".clone_into(
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
    let proposed_registry_binding = WorkflowReleaseReviewArtifactBinding {
        artifact_id: id("workflow-governance.registry.core-assurance-v0"),
        embedded_ref: path(PROPOSED_REGISTRY),
        raw_digest: sha256(&proposed_registry_bytes),
        canonical_digest: canonical(&proposed_registry),
    };
    let promoted_bundle_binding = WorkflowReleaseReviewArtifactBinding {
        artifact_id: promoted_bundle.workflow_governance_bundle.id.clone(),
        embedded_ref: path(PROMOTED_BUNDLE),
        raw_digest: sha256(&promoted_bundle_bytes),
        canonical_digest: canonical(&promoted_bundle),
    };
    let index = WorkflowReleaseReviewIndexDocument {
        schema_version: WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION.to_owned(),
        workflow_release_review_index: WorkflowReleaseReviewIndex {
            id: id("workflow-release-review.core-assurance-v0"),
            index_version: "0.1.0".to_owned(),
            authority: WorkflowReleaseReviewIndexAuthority::CandidateOnly,
            promotion: forge_core_contracts::WorkflowReleasePromotionBinding {
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
            release_manifest: binding("release-manifest.core-assurance-v0", MANIFEST, &manifest),
            migration_batches: vec![
                binding(
                    "migration-batch.golden-path-v0",
                    GOLDEN_BATCH,
                    &golden_batch,
                ),
                binding("migration-batch.core-assurance-v0", CORE_BATCH, &core_batch),
            ],
            review_subjects: vec![binding(
                "review-subject.core-assurance-v0",
                SUBJECT,
                &subject,
            )],
            coverage_policy: binding("coverage-policy.core-assurance-v0", COVERAGE, &coverage),
            corpus_set: binding("corpus-set.core-assurance-v0", CORPUS_SET, &corpus_set),
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
            shadow_report: binding("shadow-report.core-assurance-v0", REPORT, &report),
            candidate_runtime_bundle: binding(
                "runtime-bundle.core-assurance-candidate-v0",
                CANDIDATE_BUNDLE,
                &candidate_bundle,
            ),
            promoted_runtime_bundle: promoted_bundle_binding,
            predecessor_registry: binding(
                "registry.foundation-v0",
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
                "history.p5d2-foundation-v0",
                HISTORY,
                history_bytes,
                canonical(&history_values),
            ),
            workflow_decisions: subject
                .workflow_behavioral_review_subject
                .candidate_workflows
                .iter()
                .map(|workflow| WorkflowReleaseReviewWorkflowDecision {
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
                .map(|quarantine| WorkflowReleaseReviewQuarantineDecision {
                    workflow_id: quarantine.workflow_id.clone(),
                    decision: WorkflowReleaseReviewDecision::Approved,
                    rationale: "quarantine remains required".to_owned(),
                    finding_refs: Vec::new(),
                })
                .collect(),
            dimension_decisions: forge_core_contracts::WorkflowGovernedOutcomeDimension::all()
                .into_iter()
                .map(|dimension| WorkflowReleaseReviewDimensionDecision {
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
    WorkflowReleaseAdmissionCandidateInput {
        review_index: index,
        report_identity,
        coverage_policy: coverage,
        corpus_set,
        representative_corpus: representative,
        adversarial_corpus: adversarial,
        review_subject: subject,
        behavioral_bundles,
        authored_shadow_report: report,
        migration_batches: vec![golden_batch, core_batch],
        candidate_manifest: manifest,
        candidate_runtime_bundle: candidate_bundle,
        promoted_runtime_bundle: promoted_bundle,
        predecessor_registry,
        proposed_registry,
        registry_bundles: vec![
            load("contracts/workflow-governance/golden-path-v0.yaml"),
            load("contracts/workflow-governance/runtime-release-foundation-v0.yaml"),
            registry_promoted_bundle,
        ],
        source_bytes: sources,
    }
}

#[test]
fn derives_only_non_authoritative_readiness_from_closed_inputs() {
    let evaluation = evaluate_workflow_release_admission_candidate(&fixture());
    assert_eq!(
        evaluation.status,
        WorkflowReleaseAdmissionEvaluationStatus::ReadyForIndependentAuthorization,
        "{:#?}",
        evaluation.issues
    );
    assert_eq!(
        evaluation.authority,
        WorkflowReleaseAdmissionEvaluationAuthority::NonAuthoritative
    );
    assert_eq!(evaluation.predecessor_policy_count, 15);
    assert_eq!(evaluation.candidate_policy_count, 20);
    assert_eq!(evaluation.reviewed_workflow_count, 5);
    assert_eq!(evaluation.quarantine_count, 3);
    assert_eq!(evaluation.behavioral_mismatch_count, 0);
    assert_eq!(evaluation.behavioral_evaluation_error_count, 0);
}

#[test]
fn blocks_policy_drift_rejection_and_receipt_carryover() {
    let mut input = fixture();
    input
        .promoted_runtime_bundle
        .workflow_governance_bundle
        .policies
        .pop();
    input
        .review_index
        .workflow_release_review_index
        .workflow_decisions[0]
        .decision = WorkflowReleaseReviewDecision::ChangesRequired;
    input
        .proposed_registry
        .workflow_governance_release_registry
        .releases[2]
        .receipt_carryover = WorkflowReceiptCarryover::PreservePolicyEquivalent;
    let evaluation = evaluate_workflow_release_admission_candidate(&input);
    assert_eq!(
        evaluation.status,
        WorkflowReleaseAdmissionEvaluationStatus::Blocked
    );
    let codes = evaluation
        .issues
        .iter()
        .map(|issue| issue.code)
        .collect::<BTreeSet<_>>();
    assert!(
        codes.contains(&forge_core_decisions::WorkflowReleaseAdmissionIssueCode::PolicySetDrift)
    );
    assert!(codes
        .contains(&forge_core_decisions::WorkflowReleaseAdmissionIssueCode::ReviewDecisionBlocked));
    assert!(codes.contains(
        &forge_core_decisions::WorkflowReleaseAdmissionIssueCode::ReceiptCarryoverInvalid
    ));
}

#[test]
fn authored_counts_and_labels_cannot_hide_binding_or_history_drift() {
    let mut input = fixture();
    input
        .authored_shadow_report
        .workflow_behavioral_shadow_report
        .workflow_reports[0]
        .mismatch_count = 0;
    input
        .source_bytes
        .get_mut(
            &input
                .review_index
                .workflow_release_review_index
                .frozen_history
                .embedded_ref,
        )
        .expect("history bytes")
        .push(b' ');
    input
        .review_index
        .workflow_release_review_index
        .workflow_decisions
        .pop();
    let evaluation = evaluate_workflow_release_admission_candidate(&input);
    assert_eq!(
        evaluation.status,
        WorkflowReleaseAdmissionEvaluationStatus::Blocked
    );
    assert!(evaluation.issues.iter().any(|issue| {
        matches!(
            issue.code,
            forge_core_decisions::WorkflowReleaseAdmissionIssueCode::RawDigestMismatch
                | forge_core_decisions::WorkflowReleaseAdmissionIssueCode::InvalidReviewIndex
                | forge_core_decisions::WorkflowReleaseAdmissionIssueCode::ReviewSetMismatch
        )
    }));
}
