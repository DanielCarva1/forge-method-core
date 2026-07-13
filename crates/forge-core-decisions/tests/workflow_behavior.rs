use std::collections::{BTreeMap, HashMap};

use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference,
    WorkflowBehavioralCandidateWorkflowSubject, WorkflowBehavioralContinuationIdentity,
    WorkflowBehavioralCorpusClass, WorkflowBehavioralCorpusSet,
    WorkflowBehavioralCorpusSetDocument, WorkflowBehavioralCoveragePolicy,
    WorkflowBehavioralCoveragePolicyDocument, WorkflowBehavioralDisposition,
    WorkflowBehavioralEvaluatorIdentity, WorkflowBehavioralEvidenceAuthority,
    WorkflowBehavioralEvidenceBindings, WorkflowBehavioralGovernanceInput,
    WorkflowBehavioralProposedBatchSubject, WorkflowBehavioralProposedReleaseSubject,
    WorkflowBehavioralRawSourceKind, WorkflowBehavioralRawSourceReference,
    WorkflowBehavioralReviewSubject, WorkflowBehavioralReviewSubjectAuthority,
    WorkflowBehavioralReviewSubjectDocument, WorkflowBehavioralRuntimeBundleSubject,
    WorkflowBehavioralScenario, WorkflowBehavioralScenarioCorpus,
    WorkflowBehavioralScenarioCorpusDocument, WorkflowBehavioralScenarioExecution,
    WorkflowBehavioralScenarioKind, WorkflowBehavioralVerdict, WorkflowBehavioralWorkflowCorpus,
    WorkflowCompletionAssertion, WorkflowEvidenceFreshness, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent, WorkflowGovernancePolicyOverlay,
    WorkflowGovernancePolicyOverlayDocument, WorkflowGovernanceReceiptDocument,
    WorkflowGovernedOutcome, WorkflowGovernedOutcomeDimension, WorkflowGovernedStatus,
    WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION,
};
use forge_core_decisions::{
    derive_workflow_governed_outcome, evaluate_workflow_behavior,
    workflow_behavior_execution_input_digest, workflow_policy_set_digest,
    workflow_release_legacy_digest, workflow_release_policy_digest, workflow_runtime_bundle_digest,
    LoadedWorkflowDocument, WorkflowBehavioralBundleInput, WorkflowBehavioralCorpusInput,
    WorkflowBehavioralReportIdentity,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn yaml_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    yaml_serde::to_string(value).expect("yaml").into_bytes()
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn path(value: &str) -> RepoPath {
    RepoPath(value.to_owned())
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn load_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    yaml_serde::from_str(
        &std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(relative),
        )
        .expect("fixture"),
    )
    .expect("valid fixture")
}

#[derive(Clone)]
struct Fixture {
    coverage: WorkflowBehavioralCoveragePolicyDocument,
    corpus_set: WorkflowBehavioralCorpusSetDocument,
    review: WorkflowBehavioralReviewSubjectDocument,
    corpora: Vec<WorkflowBehavioralCorpusInput>,
    identity: WorkflowBehavioralReportIdentity,
    bundles: BTreeMap<String, WorkflowBehavioralBundleInput>,
    sources: HashMap<RepoPath, Vec<u8>>,
}

impl Fixture {
    fn run(&self) -> forge_core_contracts::WorkflowBehavioralShadowReportDocument {
        evaluate_workflow_behavior(
            &self.identity,
            &self.coverage,
            &self.corpus_set,
            &self.review,
            &self.corpora,
            &self.bundles,
            &self.sources,
        )
    }

    fn refresh_scenarios(&mut self) {
        for corpus in &mut self.corpora {
            let workflow = &mut corpus
                .document
                .workflow_behavioral_scenario_corpus
                .workflow_evidence[0];
            for scenario in &mut workflow.scenarios {
                scenario.execution_input_digest =
                    workflow_behavior_execution_input_digest(&scenario.execution)
                        .expect("scenario digest");
            }
            let bytes = yaml_bytes(&corpus.document);
            corpus.artifact.expected_digest = sha256(&bytes);
            self.sources
                .insert(corpus.artifact.embedded_ref.clone(), bytes);
        }
        self.corpus_set.workflow_behavioral_corpus_set.corpora = self
            .corpora
            .iter()
            .map(|corpus| corpus.artifact.clone())
            .collect();
        let corpus_set_bytes = yaml_bytes(&self.corpus_set);
        self.identity.corpus_set.expected_digest = sha256(&corpus_set_bytes);
        self.sources.insert(
            self.identity.corpus_set.embedded_ref.clone(),
            corpus_set_bytes,
        );
    }

    fn scenario_mut(&mut self, scenario_id: &str) -> &mut WorkflowBehavioralScenario {
        self.corpora
            .iter_mut()
            .flat_map(|corpus| {
                corpus
                    .document
                    .workflow_behavioral_scenario_corpus
                    .workflow_evidence
                    .iter_mut()
                    .flat_map(|workflow| workflow.scenarios.iter_mut())
            })
            .find(|scenario| scenario.scenario_id.0 == scenario_id)
            .expect("scenario")
    }

    fn bindings_mut(&mut self) -> impl Iterator<Item = &mut WorkflowBehavioralEvidenceBindings> {
        self.corpora.iter_mut().flat_map(|corpus| {
            corpus
                .document
                .workflow_behavioral_scenario_corpus
                .workflow_evidence
                .iter_mut()
                .map(|workflow| &mut workflow.bindings)
        })
    }

    fn refresh_review(&mut self) {
        let review_bytes = yaml_bytes(&self.review);
        let source_digest = sha256(&review_bytes);
        let canonical = canonical_digest(&self.review);
        let path = self
            .corpora
            .first()
            .expect("corpus")
            .document
            .workflow_behavioral_scenario_corpus
            .workflow_evidence[0]
            .bindings
            .review_subject
            .embedded_ref
            .clone();
        for bindings in self.bindings_mut() {
            bindings
                .review_subject
                .expected_digest
                .clone_from(&source_digest);
            bindings.review_subject_digest.clone_from(&canonical);
        }
        self.sources.insert(path, review_bytes);
        self.refresh_scenarios();
    }
}

// The full fixture deliberately keeps every binding, scenario class, resume,
// and ablation in one reviewable construction used by all adversarial tests.
#[allow(clippy::too_many_lines)]
fn fixture() -> Fixture {
    let bundle: WorkflowGovernanceBundleDocument =
        load_yaml("contracts/workflow-governance/kernel-v0.yaml");
    let complete: WorkflowGovernanceEvaluationDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
    let missing: WorkflowGovernanceEvaluationDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/missing-evidence.yaml");
    let invented: WorkflowGovernanceEvaluationDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/invented-completion.yaml");
    let ambiguity: WorkflowGovernanceEvaluationDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/human-decision.yaml");
    let policy_id = complete.workflow_governance_evaluation.policy_id.clone();
    let policy = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == policy_id)
        .expect("policy");
    let workflow_id = policy.compatibility_workflow_id.clone();
    let legacy_ref = path("contracts/workflows/build-story.yaml");
    let legacy_bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("contracts/evidence/workflow-retirement/legacy-catalog/build-story.yaml"),
    )
    .expect("legacy bytes");
    let legacy_document =
        yaml_serde::from_str(std::str::from_utf8(&legacy_bytes).expect("legacy UTF-8"))
            .expect("legacy document");
    let legacy_digest = workflow_release_legacy_digest(&LoadedWorkflowDocument {
        workflow_ref: legacy_ref.clone(),
        document: legacy_document,
    })
    .expect("legacy digest");
    let bundle_digest = workflow_runtime_bundle_digest(&bundle).expect("bundle digest");
    let policy_digest = workflow_release_policy_digest(policy).expect("policy digest");
    let policy_set_digest =
        workflow_policy_set_digest(&bundle.workflow_governance_bundle.policies).expect("set");

    let coverage = WorkflowBehavioralCoveragePolicyDocument {
        schema_version: WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_coverage_policy: WorkflowBehavioralCoveragePolicy {
            id: id("policy.workflow-behavioral-coverage-v0"),
            policy_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            required_scenario_kinds: WorkflowBehavioralScenarioKind::all().to_vec(),
            minimum_scenarios_per_kind: 1,
            minimum_scenarios_per_workflow: 7,
            required_coverage_basis_points: 10_000,
            require_zero_mismatches: true,
            require_zero_evaluation_errors: true,
            required_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
            require_resume_equivalence: true,
            require_ablation_semantic_delta: true,
            require_representative_scenarios: true,
            require_adversarial_scenarios: true,
        },
    };
    let coverage_digest = canonical_digest(&coverage);
    let coverage_bytes = yaml_bytes(&coverage);
    let coverage_ref = WorkflowBehavioralArtifactReference {
        id: coverage.workflow_behavioral_coverage_policy.id.clone(),
        embedded_ref: path("contracts/evidence/coverage.yaml"),
        expected_digest: sha256(&coverage_bytes),
    };
    let evaluator_bytes = b"trusted evaluator source".to_vec();
    let evaluator = WorkflowBehavioralEvaluatorIdentity {
        evaluator_id: id("evaluator.workflow-behavioral-shadow"),
        evaluator_version: "0.1.0".to_owned(),
        governed_projection_version: "0.1.0".to_owned(),
        evaluator_source_digest: sha256(&evaluator_bytes),
    };
    let overlay_document = WorkflowGovernancePolicyOverlayDocument {
        schema_version: "0.1".to_owned(),
        workflow_governance_policy_overlay: WorkflowGovernancePolicyOverlay {
            id: id("overlay.test"),
            base_bundle_id: bundle.workflow_governance_bundle.id.clone(),
            policies: vec![policy.clone()],
        },
    };
    let overlay_bytes = yaml_bytes(&overlay_document);
    let overlay_ref = WorkflowBehavioralArtifactReference {
        id: id("overlay.test"),
        embedded_ref: path("contracts/policies/test-overlay.yaml"),
        expected_digest: sha256(&overlay_bytes),
    };
    let baseline_history_path =
        path("crates/forge-core-kernel/tests/fixtures/p5d2-foundation-history.ndjson");
    let baseline_history_bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(&baseline_history_path.0),
    )
    .expect("baseline history");
    let history_documents = baseline_history_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            yaml_serde::from_str::<WorkflowGovernanceReceiptDocument>(
                std::str::from_utf8(line).expect("history UTF-8"),
            )
            .expect("history record")
        })
        .collect::<Vec<_>>();
    let last_record = &history_documents
        .last()
        .expect("history head")
        .workflow_governance_receipt;
    let WorkflowGovernanceEvent::ReleaseUpgraded(baseline_upgrade) = &last_record.event else {
        panic!("baseline upgrade")
    };
    let WorkflowGovernanceEvent::ProjectImported(baseline_import) =
        &history_documents[0].workflow_governance_receipt.event
    else {
        panic!("baseline import")
    };
    let baseline_history_ref = WorkflowBehavioralArtifactReference {
        id: id("history.p5d2.foundation"),
        embedded_ref: baseline_history_path.clone(),
        expected_digest: sha256(&baseline_history_bytes),
    };
    let review = WorkflowBehavioralReviewSubjectDocument {
        schema_version: WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_review_subject: WorkflowBehavioralReviewSubject {
            id: id("review-subject.test"),
            authority: WorkflowBehavioralReviewSubjectAuthority::CandidateOnly,
            overlay: overlay_ref.clone(),
            baseline_history: baseline_history_ref.clone(),
            baseline_release: baseline_upgrade.to_release.clone(),
            baseline_runtime_bundle: baseline_upgrade.to_runtime_bundle.clone(),
            runtime_bundle: WorkflowBehavioralRuntimeBundleSubject {
                bundle_id: bundle.workflow_governance_bundle.id.clone(),
                bundle_digest: bundle_digest.clone(),
                policy_set_digest: policy_set_digest.clone(),
            },
            proposed_batch: WorkflowBehavioralProposedBatchSubject {
                batch_id: id("batch.test"),
                batch_version: "0.2.0".to_owned(),
                previous_batch_digest: digest('b'),
            },
            proposed_release: WorkflowBehavioralProposedReleaseSubject {
                lineage_id: id("lineage.test"),
                release_id: id("release.test"),
                release_version: "0.2.0".to_owned(),
                previous_release_digest: digest('c'),
            },
            evaluator: evaluator.clone(),
            candidate_workflows: vec![WorkflowBehavioralCandidateWorkflowSubject {
                workflow_id: workflow_id.clone(),
                legacy_workflow_digest: legacy_digest.clone(),
                policy_ref: policy_id.clone(),
                policy_digest: policy_digest.clone(),
            }],
            quarantines: Vec::new(),
        },
    };
    let review_bytes = yaml_bytes(&review);
    let review_ref = WorkflowBehavioralArtifactReference {
        id: review.workflow_behavioral_review_subject.id.clone(),
        embedded_ref: path("contracts/evidence/review-subject.yaml"),
        expected_digest: sha256(&review_bytes),
    };
    let review_digest = canonical_digest(&review);
    let bundle_bytes = yaml_bytes(&bundle);
    let bundle_ref = WorkflowBehavioralArtifactReference {
        id: bundle.workflow_governance_bundle.id.clone(),
        embedded_ref: path("contracts/workflow-governance/test-bundle.yaml"),
        expected_digest: sha256(&bundle_bytes),
    };
    let bindings = WorkflowBehavioralEvidenceBindings {
        review_subject: review_ref.clone(),
        review_subject_digest: review_digest,
        workflow_id,
        legacy_workflow_digest: legacy_digest,
        policy_ref: policy_id,
        policy_digest: policy_digest.clone(),
        candidate_bundle_id: bundle.workflow_governance_bundle.id.clone(),
        candidate_bundle_digest: bundle_digest.clone(),
        candidate_bundle_source_digest: bundle_ref.expected_digest.clone(),
        candidate_policy_set_digest: policy_set_digest,
        migration_batch_id: id("batch.test"),
        migration_batch_version: "0.2.0".to_owned(),
        governance_release_id: id("release.test"),
        governance_release_version: "0.2.0".to_owned(),
        predecessor_release_digest: digest('c'),
        coverage_policy_id: coverage_ref.id.clone(),
        coverage_policy_digest: coverage_digest,
        coverage_policy_source_digest: coverage_ref.expected_digest.clone(),
        evaluator,
        raw_sources: vec![
            WorkflowBehavioralRawSourceReference {
                kind: WorkflowBehavioralRawSourceKind::LegacyWorkflow,
                embedded_ref: legacy_ref.clone(),
                expected_digest: sha256(&legacy_bytes),
            },
            WorkflowBehavioralRawSourceReference {
                kind: WorkflowBehavioralRawSourceKind::GovernancePolicy,
                embedded_ref: overlay_ref.embedded_ref.clone(),
                expected_digest: overlay_ref.expected_digest.clone(),
            },
            WorkflowBehavioralRawSourceReference {
                kind: WorkflowBehavioralRawSourceKind::CandidateBundle,
                embedded_ref: bundle_ref.embedded_ref.clone(),
                expected_digest: bundle_ref.expected_digest.clone(),
            },
            WorkflowBehavioralRawSourceReference {
                kind: WorkflowBehavioralRawSourceKind::CoveragePolicy,
                embedded_ref: coverage_ref.embedded_ref.clone(),
                expected_digest: coverage_ref.expected_digest.clone(),
            },
            WorkflowBehavioralRawSourceReference {
                kind: WorkflowBehavioralRawSourceKind::Evaluator,
                embedded_ref: path("evaluator.rs"),
                expected_digest: sha256(&evaluator_bytes),
            },
        ],
    };

    let complete_outcome = derive_workflow_governed_outcome(&bundle, &complete).expect("complete");
    let missing_outcome = derive_workflow_governed_outcome(&bundle, &missing).expect("missing");
    let invented_outcome = derive_workflow_governed_outcome(&bundle, &invented).expect("invented");
    let ambiguity_outcome =
        derive_workflow_governed_outcome(&bundle, &ambiguity).expect("ambiguity");
    let mut stale = complete.clone();
    for evidence in &mut stale.workflow_governance_evaluation.evidence {
        evidence.freshness = WorkflowEvidenceFreshness::Stale;
    }
    stale.workflow_governance_evaluation.completion_assertion =
        WorkflowCompletionAssertion::Asserted;
    let stale_outcome = derive_workflow_governed_outcome(&bundle, &stale).expect("stale");

    let mut ablated_bundle = bundle.clone();
    let ablated_policy = ablated_bundle
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|candidate| candidate.id == bindings.policy_ref)
        .expect("ablated policy");
    let removed_obligation = ablated_policy.obligations[0].clone();
    let removed_ids = vec![removed_obligation.id.clone()];
    ablated_policy.obligations.remove(0);
    let ablated_digest = workflow_runtime_bundle_digest(&ablated_bundle).expect("ablated digest");
    let ablated_bytes = yaml_bytes(&ablated_bundle);
    let ablated_ref = WorkflowBehavioralArtifactReference {
        id: ablated_bundle.workflow_governance_bundle.id.clone(),
        embedded_ref: path("contracts/workflow-governance/test-bundle-ablated.yaml"),
        expected_digest: sha256(&ablated_bytes),
    };
    let mut ablation_evaluation = complete.clone();
    ablation_evaluation
        .workflow_governance_evaluation
        .evidence
        .retain(|evidence| !removed_obligation.claim_refs.contains(&evidence.claim_ref));
    let control_ablation_outcome =
        derive_workflow_governed_outcome(&bundle, &ablation_evaluation).expect("control ablation");
    let ablated_outcome =
        derive_workflow_governed_outcome(&ablated_bundle, &ablation_evaluation).expect("ablated");
    let ablation_dimensions = outcome_differences(&control_ablation_outcome, &ablated_outcome);

    let input =
        |evaluation: WorkflowGovernanceEvaluationDocument| WorkflowBehavioralGovernanceInput {
            bundle: bundle_ref.clone(),
            evaluation,
        };
    let mut scenarios = vec![
        single(
            "positive",
            WorkflowBehavioralScenarioKind::Positive,
            WorkflowBehavioralCorpusClass::Representative,
            input(complete.clone()),
            complete_outcome.clone(),
        ),
        single(
            "negative",
            WorkflowBehavioralScenarioKind::Negative,
            WorkflowBehavioralCorpusClass::Representative,
            input(missing.clone()),
            missing_outcome.clone(),
        ),
        single(
            "ambiguity",
            WorkflowBehavioralScenarioKind::Ambiguity,
            WorkflowBehavioralCorpusClass::Representative,
            input(ambiguity),
            ambiguity_outcome,
        ),
        single(
            "false-completion",
            WorkflowBehavioralScenarioKind::FalseCompletion,
            WorkflowBehavioralCorpusClass::Adversarial,
            input(invented),
            invented_outcome,
        ),
        single(
            "stale",
            WorkflowBehavioralScenarioKind::StaleEvidence,
            WorkflowBehavioralCorpusClass::Adversarial,
            input(stale),
            stale_outcome,
        ),
    ];
    let resume_input = input(complete.clone());
    let checkpoint_bytes = yaml_bytes(&resume_input);
    let checkpoint_ref = WorkflowBehavioralArtifactReference {
        id: id("checkpoint.test"),
        embedded_ref: path("contracts/evidence/checkpoint-test.yaml"),
        expected_digest: sha256(&checkpoint_bytes),
    };
    scenarios.push(WorkflowBehavioralScenario {
        scenario_id: id("resume"),
        scenario_kind: WorkflowBehavioralScenarioKind::Resume,
        corpus_class: WorkflowBehavioralCorpusClass::Adversarial,
        execution_input_digest: String::new(),
        execution: WorkflowBehavioralScenarioExecution::Resume {
            continuation: Box::new(WorkflowBehavioralContinuationIdentity {
                ledger_digest: baseline_history_ref.expected_digest.clone(),
                ledger_head_digest: last_record.record_digest.clone(),
                snapshot_digest: baseline_import.snapshot_digest.clone(),
                active_release_id: baseline_upgrade.to_release.release_id.clone(),
                active_release_digest: baseline_upgrade.to_release.release_digest.clone(),
                runtime_bundle_id: baseline_upgrade.to_runtime_bundle.bundle_id.clone(),
                runtime_bundle_digest: baseline_upgrade.to_runtime_bundle.bundle_digest.clone(),
                state_version: last_record.state_version,
                current_phase: baseline_import.initial_phase.clone(),
                observed_at_unix: last_record.recorded_at_unix,
            }),
            checkpoint_source: checkpoint_ref.clone(),
            checkpoint_digest: canonical_digest(&resume_input),
            checkpoint_input: Box::new(resume_input.clone()),
            checkpoint_expected: Box::new(complete_outcome.clone()),
            resumed_input: Box::new(resume_input),
            resumed_expected: Box::new(complete_outcome.clone()),
            equivalence_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
        },
    });
    scenarios.push(WorkflowBehavioralScenario {
        scenario_id: id("ablation"),
        scenario_kind: WorkflowBehavioralScenarioKind::Ablation,
        corpus_class: WorkflowBehavioralCorpusClass::Adversarial,
        execution_input_digest: String::new(),
        execution: WorkflowBehavioralScenarioExecution::Ablation {
            control_input: Box::new(input(ablation_evaluation.clone())),
            control_expected: Box::new(control_ablation_outcome),
            ablated_input: Box::new(WorkflowBehavioralGovernanceInput {
                bundle: ablated_ref.clone(),
                evaluation: ablation_evaluation,
            }),
            ablated_expected: Box::new(ablated_outcome),
            removed_semantic_refs: removed_ids,
            required_difference_dimensions: ablation_dimensions,
        },
    });
    for scenario in &mut scenarios {
        scenario.execution_input_digest =
            workflow_behavior_execution_input_digest(&scenario.execution).expect("digest");
    }
    let mut corpora = Vec::new();
    for (class, name) in [
        (
            WorkflowBehavioralCorpusClass::Representative,
            "representative",
        ),
        (WorkflowBehavioralCorpusClass::Adversarial, "adversarial"),
    ] {
        let corpus_document = WorkflowBehavioralScenarioCorpusDocument {
            schema_version: WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION.to_owned(),
            workflow_behavioral_scenario_corpus: WorkflowBehavioralScenarioCorpus {
                id: id(&format!("corpus.test.{name}")),
                corpus_version: "0.1.0".to_owned(),
                authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
                partition_class: class,
                coverage_policy: coverage_ref.clone(),
                workflow_evidence: vec![WorkflowBehavioralWorkflowCorpus {
                    bindings: bindings.clone(),
                    scenarios: scenarios
                        .iter()
                        .filter(|scenario| scenario.corpus_class == class)
                        .cloned()
                        .collect(),
                }],
            },
        };
        corpora.push(WorkflowBehavioralCorpusInput {
            artifact: WorkflowBehavioralArtifactReference {
                id: corpus_document
                    .workflow_behavioral_scenario_corpus
                    .id
                    .clone(),
                embedded_ref: path(&format!("contracts/evidence/corpus-{name}.yaml")),
                expected_digest: sha256(&yaml_bytes(&corpus_document)),
            },
            document: corpus_document,
        });
    }
    let corpus_set = WorkflowBehavioralCorpusSetDocument {
        schema_version: WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_corpus_set: WorkflowBehavioralCorpusSet {
            id: id("corpus-set.test"),
            corpus_set_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            corpora: corpora
                .iter()
                .map(|corpus| corpus.artifact.clone())
                .collect(),
        },
    };
    let corpus_set_bytes = yaml_bytes(&corpus_set);
    let identity = WorkflowBehavioralReportIdentity {
        report_id: id("report.test"),
        report_version: "0.1.0".to_owned(),
        corpus_set: WorkflowBehavioralArtifactReference {
            id: corpus_set.workflow_behavioral_corpus_set.id.clone(),
            embedded_ref: path("contracts/evidence/corpus-set.yaml"),
            expected_digest: sha256(&corpus_set_bytes),
        },
        coverage_policy: coverage_ref.clone(),
    };
    let mut bundles = BTreeMap::new();
    bundles.insert(
        bundle_digest,
        WorkflowBehavioralBundleInput {
            artifact: bundle_ref.clone(),
            document: bundle,
        },
    );
    bundles.insert(
        ablated_digest,
        WorkflowBehavioralBundleInput {
            artifact: ablated_ref.clone(),
            document: ablated_bundle,
        },
    );
    let mut sources = HashMap::new();
    sources.insert(coverage_ref.embedded_ref.clone(), coverage_bytes);
    sources.insert(review_ref.embedded_ref.clone(), review_bytes);
    sources.insert(overlay_ref.embedded_ref.clone(), overlay_bytes);
    sources.insert(baseline_history_path, baseline_history_bytes);
    sources.insert(checkpoint_ref.embedded_ref.clone(), checkpoint_bytes);
    sources.insert(bundle_ref.embedded_ref.clone(), bundle_bytes);
    sources.insert(ablated_ref.embedded_ref.clone(), ablated_bytes);
    sources.insert(identity.corpus_set.embedded_ref.clone(), corpus_set_bytes);
    for corpus in &corpora {
        sources.insert(
            corpus.artifact.embedded_ref.clone(),
            yaml_bytes(&corpus.document),
        );
    }
    sources.insert(legacy_ref, legacy_bytes);
    sources.insert(path("evaluator.rs"), evaluator_bytes);
    Fixture {
        coverage,
        corpus_set,
        review,
        corpora,
        identity,
        bundles,
        sources,
    }
}

fn single(
    name: &str,
    scenario_kind: WorkflowBehavioralScenarioKind,
    corpus_class: WorkflowBehavioralCorpusClass,
    input: WorkflowBehavioralGovernanceInput,
    expected: WorkflowGovernedOutcome,
) -> WorkflowBehavioralScenario {
    WorkflowBehavioralScenario {
        scenario_id: id(name),
        scenario_kind,
        corpus_class,
        execution_input_digest: String::new(),
        execution: WorkflowBehavioralScenarioExecution::Single {
            input: Box::new(input),
            expected: Box::new(expected),
        },
    }
}

fn outcome_differences(
    left: &WorkflowGovernedOutcome,
    right: &WorkflowGovernedOutcome,
) -> Vec<WorkflowGovernedOutcomeDimension> {
    WorkflowGovernedOutcomeDimension::all()
        .into_iter()
        .filter(|dimension| match dimension {
            WorkflowGovernedOutcomeDimension::Status => left.status != right.status,
            WorkflowGovernedOutcomeDimension::Eligibility => left.eligibility != right.eligibility,
            WorkflowGovernedOutcomeDimension::Progression => left.progression != right.progression,
            WorkflowGovernedOutcomeDimension::Completion => left.completion != right.completion,
            WorkflowGovernedOutcomeDimension::Obligations => left.obligations != right.obligations,
            WorkflowGovernedOutcomeDimension::Claims => left.claims != right.claims,
            WorkflowGovernedOutcomeDimension::Decisions => {
                left.decision_refs != right.decision_refs
            }
            WorkflowGovernedOutcomeDimension::Capabilities => {
                left.capability_refs != right.capability_refs
            }
            WorkflowGovernedOutcomeDimension::Issues => left.issues != right.issues,
            WorkflowGovernedOutcomeDimension::NextActions => {
                left.next_actions != right.next_actions
            }
        })
        .collect()
}

#[test]
fn complete_corpus_derives_review_candidate_without_trusting_a_report() {
    let fixture = fixture();
    let report = fixture.run();
    assert_eq!(
        report.workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
    );
    assert_eq!(
        report.workflow_behavioral_shadow_report.disposition,
        WorkflowBehavioralDisposition::ReviewCandidate
    );
    assert!(report.validate().is_empty(), "{:?}", report.validate());

    let absent = evaluate_workflow_behavior(
        &fixture.identity,
        &fixture.coverage,
        &fixture.corpus_set,
        &fixture.review,
        &[],
        &fixture.bundles,
        &fixture.sources,
    );
    assert_eq!(
        absent.workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
    assert!(absent.validate().is_empty(), "{:?}", absent.validate());
}

#[test]
fn authored_expected_pass_cannot_override_fresh_simulation() {
    let mut fixture = fixture();
    let scenario = fixture.scenario_mut("positive");
    let WorkflowBehavioralScenarioExecution::Single { expected, .. } = &mut scenario.execution
    else {
        panic!("single")
    };
    expected.status = WorkflowGovernedStatus::Blocked;
    fixture.refresh_scenarios();
    let report = fixture.run();
    assert_eq!(
        report.workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::MismatchDetected
    );
    assert_eq!(
        report.workflow_behavioral_shadow_report.workflow_reports[0].mismatch_count,
        1
    );
}

#[test]
fn weakened_policy_missing_kind_and_source_drift_fail_closed() {
    let mut weakened = fixture();
    weakened
        .coverage
        .workflow_behavioral_coverage_policy
        .require_zero_mismatches = false;
    assert_eq!(
        weakened.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut missing = fixture();
    for corpus in &mut missing.corpora {
        corpus
            .document
            .workflow_behavioral_scenario_corpus
            .workflow_evidence[0]
            .scenarios
            .retain(|scenario| scenario.scenario_id.0 != "ablation");
    }
    missing.refresh_scenarios();
    assert_eq!(
        missing.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InsufficientEvidence
    );

    let mut drift = fixture();
    drift
        .sources
        .insert(path("evaluator.rs"), b"drifted evaluator".to_vec());
    assert_eq!(
        drift.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
}

#[test]
fn wrong_policy_bundle_review_subject_and_resume_bindings_reject() {
    for mutation in 0..3 {
        let mut fixture = fixture();
        for bindings in fixture.bindings_mut() {
            match mutation {
                0 => bindings.policy_digest = digest('f'),
                1 => bindings.candidate_bundle_digest = digest('f'),
                _ => bindings.review_subject_digest = digest('f'),
            }
        }
        fixture.refresh_scenarios();
        assert_eq!(
            fixture.run().workflow_behavioral_shadow_report.verdict,
            WorkflowBehavioralVerdict::InvalidBindings
        );
    }

    let mut resume = fixture();
    let scenario = resume.scenario_mut("resume");
    let WorkflowBehavioralScenarioExecution::Resume { continuation, .. } = &mut scenario.execution
    else {
        panic!("resume")
    };
    continuation.state_version += 1;
    resume.refresh_scenarios();
    assert_eq!(
        resume.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
}

#[test]
fn no_op_or_unrelated_ablation_cannot_become_evidence() {
    let mut no_op = fixture();
    let scenario = no_op.scenario_mut("ablation");
    let WorkflowBehavioralScenarioExecution::Ablation {
        ablated_input,
        control_input,
        ..
    } = &mut scenario.execution
    else {
        panic!("ablation")
    };
    *ablated_input = control_input.clone();
    no_op.refresh_scenarios();
    assert_eq!(
        no_op.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut unrelated = fixture();
    let scenario = unrelated.scenario_mut("ablation");
    let WorkflowBehavioralScenarioExecution::Ablation {
        removed_semantic_refs,
        ..
    } = &mut scenario.execution
    else {
        panic!("ablation")
    };
    removed_semantic_refs[0] = id("unrelated.semantic");
    unrelated.refresh_scenarios();
    assert_eq!(
        unrelated.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
}

#[test]
fn prose_is_excluded_and_authored_order_is_normalized() {
    let base = fixture();
    let original_bundle = &base.bundles.values().next().expect("bundle").document;
    let evaluation = load_yaml("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
    let original =
        derive_workflow_governed_outcome(original_bundle, &evaluation).expect("original");
    let mut prose = original_bundle.clone();
    let policy = prose
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|policy| policy.id == evaluation.workflow_governance_evaluation.policy_id)
        .expect("policy");
    policy
        .advisory_playbook
        .steps
        .push("different display prose".to_owned());
    for obligation in &mut policy.obligations {
        obligation.description.push_str(" changed");
    }
    let changed = derive_workflow_governed_outcome(&prose, &evaluation).expect("changed");
    assert_eq!(original, changed);

    let mut ordering = fixture();
    let scenario = ordering.scenario_mut("positive");
    let WorkflowBehavioralScenarioExecution::Single { expected, .. } = &mut scenario.execution
    else {
        panic!("single")
    };
    expected.claims.reverse();
    expected.obligations.reverse();
    expected.next_actions.reverse();
    ordering.refresh_scenarios();
    assert_eq!(
        ordering.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
    );
}

#[test]
fn typed_document_substitution_and_partition_class_mismatch_reject() {
    let mut substituted = fixture();
    substituted.corpora[0]
        .document
        .workflow_behavioral_scenario_corpus
        .corpus_version = "substituted-without-source-bytes".to_owned();
    assert_eq!(
        substituted.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut wrong_class = fixture();
    wrong_class.scenario_mut("positive").corpus_class = WorkflowBehavioralCorpusClass::Adversarial;
    wrong_class.refresh_scenarios();
    assert_eq!(
        wrong_class.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
}

#[test]
fn relabeling_inputs_cannot_manufacture_scenario_coverage() {
    let mut relabeled = fixture();
    relabeled.scenario_mut("negative").scenario_kind = WorkflowBehavioralScenarioKind::Ambiguity;
    relabeled.scenario_mut("ambiguity").scenario_kind = WorkflowBehavioralScenarioKind::Negative;
    relabeled.refresh_scenarios();
    assert_eq!(
        relabeled.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InsufficientEvidence
    );

    for target in ["false-completion", "stale"] {
        let mut theater = fixture();
        let positive_execution = theater.scenario_mut("positive").execution.clone();
        theater.scenario_mut(target).execution = positive_execution;
        theater.refresh_scenarios();
        assert_eq!(
            theater.run().workflow_behavioral_shadow_report.verdict,
            WorkflowBehavioralVerdict::InsufficientEvidence
        );
    }
}

#[test]
fn candidate_set_checkpoint_and_baseline_history_are_exact() {
    let mut missing_candidate = fixture();
    let mut extra = missing_candidate
        .review
        .workflow_behavioral_review_subject
        .candidate_workflows[0]
        .clone();
    extra.workflow_id = id("candidate.not-in-corpus");
    missing_candidate
        .review
        .workflow_behavioral_review_subject
        .candidate_workflows
        .push(extra);
    missing_candidate.refresh_review();
    assert_eq!(
        missing_candidate
            .run()
            .workflow_behavioral_shadow_report
            .verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut checkpoint = fixture();
    let checkpoint_path = match &checkpoint.scenario_mut("resume").execution {
        WorkflowBehavioralScenarioExecution::Resume {
            checkpoint_source, ..
        } => checkpoint_source.embedded_ref.clone(),
        _ => panic!("resume"),
    };
    checkpoint
        .sources
        .insert(checkpoint_path, b"substituted checkpoint".to_vec());
    assert_eq!(
        checkpoint.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut history = fixture();
    let history_path = history
        .review
        .workflow_behavioral_review_subject
        .baseline_history
        .embedded_ref
        .clone();
    history
        .sources
        .get_mut(&history_path)
        .expect("history")
        .push(b' ');
    assert_eq!(
        history.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );
}

#[test]
fn one_good_case_cannot_mask_a_bad_resume_or_ablation() {
    let mut bad_resume = fixture();
    let mut duplicate = bad_resume.scenario_mut("resume").clone();
    duplicate.scenario_id = id("resume-bad");
    let WorkflowBehavioralScenarioExecution::Resume { continuation, .. } = &mut duplicate.execution
    else {
        panic!("resume")
    };
    continuation.ledger_head_digest = digest('f');
    duplicate.execution_input_digest =
        workflow_behavior_execution_input_digest(&duplicate.execution).expect("digest");
    bad_resume
        .corpora
        .iter_mut()
        .find(|corpus| {
            corpus
                .document
                .workflow_behavioral_scenario_corpus
                .partition_class
                == WorkflowBehavioralCorpusClass::Adversarial
        })
        .expect("adversarial")
        .document
        .workflow_behavioral_scenario_corpus
        .workflow_evidence[0]
        .scenarios
        .push(duplicate);
    bad_resume.refresh_scenarios();
    assert_eq!(
        bad_resume.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InvalidBindings
    );

    let mut bad_ablation = fixture();
    let mut duplicate = bad_ablation.scenario_mut("ablation").clone();
    duplicate.scenario_id = id("ablation-bad-direction");
    let missing: WorkflowGovernanceEvaluationDocument =
        load_yaml("docs/fixtures/workflow-governance-kernel-v0/missing-evidence.yaml");
    let WorkflowBehavioralScenarioExecution::Ablation {
        control_input,
        control_expected,
        ablated_input,
        ablated_expected,
        required_difference_dimensions,
        ..
    } = &mut duplicate.execution
    else {
        panic!("ablation")
    };
    control_input.evaluation = missing.clone();
    ablated_input.evaluation = missing;
    let control_bundle = bad_ablation
        .bundles
        .values()
        .find(|bundle| bundle.artifact == control_input.bundle)
        .expect("control bundle");
    let ablation_bundle = bad_ablation
        .bundles
        .values()
        .find(|bundle| bundle.artifact == ablated_input.bundle)
        .expect("ablated bundle");
    **control_expected =
        derive_workflow_governed_outcome(&control_bundle.document, &control_input.evaluation)
            .expect("control");
    **ablated_expected =
        derive_workflow_governed_outcome(&ablation_bundle.document, &ablated_input.evaluation)
            .expect("ablated");
    *required_difference_dimensions =
        outcome_differences(control_expected.as_ref(), ablated_expected.as_ref());
    duplicate.execution_input_digest =
        workflow_behavior_execution_input_digest(&duplicate.execution).expect("digest");
    bad_ablation
        .corpora
        .iter_mut()
        .find(|corpus| {
            corpus
                .document
                .workflow_behavioral_scenario_corpus
                .partition_class
                == WorkflowBehavioralCorpusClass::Adversarial
        })
        .expect("adversarial")
        .document
        .workflow_behavioral_scenario_corpus
        .workflow_evidence[0]
        .scenarios
        .push(duplicate);
    bad_ablation.refresh_scenarios();
    assert_eq!(
        bad_ablation.run().workflow_behavioral_shadow_report.verdict,
        WorkflowBehavioralVerdict::InsufficientEvidence
    );
}
