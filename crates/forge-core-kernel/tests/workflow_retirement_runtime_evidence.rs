//! Re-runnable P5d.5 proof that deleting legacy routing did not delete the
//! behavior admitted in P5d.4.
//!
//! This deliberately does more than compare authored hashes. It loads the
//! frozen P5d.4 corpora, re-executes the behavioral evaluator against the
//! content-addressed candidate bundles, compares every derived outcome with
//! the frozen reports, and then binds every reviewed policy to the currently
//! admitted 42-policy runtime. The 15 foundation policies are exercised by
//! `workflow_governance_golden_path`; this test executes the remaining 27.

use forge_core_contracts::{
    RepoPath, WorkflowBehavioralArtifactReference, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicyDocument, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralScenarioCorpusDocument, WorkflowBehavioralShadowReportDocument,
    WorkflowGovernanceBundleDocument,
};
use forge_core_decisions::{
    evaluate_workflow_behavior, workflow_release_policy_digest, workflow_runtime_bundle_digest,
    WorkflowBehavioralBundleInput, WorkflowBehavioralCorpusInput, WorkflowBehavioralReportIdentity,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

const BATCHES: [&str; 3] = [
    "core-assurance",
    "assurance-operations",
    "agent-native-continuity",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    yaml_serde::from_str(
        &fs::read_to_string(root().join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {relative}: {error}"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn collect_embedded_refs(value: &serde_json::Value, refs: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(reference) = map.get("embedded_ref").and_then(serde_json::Value::as_str) {
                refs.insert(reference.to_owned());
            }
            for child in map.values() {
                collect_embedded_refs(child, refs);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_embedded_refs(child, refs);
            }
        }
        _ => {}
    }
}

fn source_bytes(reference: &str) -> Vec<u8> {
    let direct = root().join(reference);
    if direct.is_file() {
        return fs::read(&direct).unwrap_or_else(|error| panic!("read {reference}: {error}"));
    }
    let legacy_name = Path::new(reference)
        .file_name()
        .unwrap_or_else(|| panic!("legacy reference has no file name: {reference}"));
    let frozen = root()
        .join("contracts/evidence/workflow-retirement/legacy-catalog")
        .join(legacy_name);
    fs::read(&frozen).unwrap_or_else(|error| {
        panic!(
            "source {reference} is absent from both the operational tree and frozen legacy evidence {}: {error}",
            frozen.display()
        )
    })
}

#[allow(clippy::too_many_lines)]
fn replay_batch(batch: &str) -> (BTreeSet<String>, usize) {
    let report_path = format!("contracts/evidence/workflow-{batch}-shadow-report-v0.yaml");
    let expected: WorkflowBehavioralShadowReportDocument = load(&report_path);
    let frozen = &expected.workflow_behavioral_shadow_report;
    let corpus_set: WorkflowBehavioralCorpusSetDocument = load(&frozen.corpus.embedded_ref.0);
    let coverage: WorkflowBehavioralCoveragePolicyDocument =
        load(&frozen.coverage_policy.embedded_ref.0);

    let review_ref = frozen
        .workflow_reports
        .first()
        .expect("non-empty frozen report")
        .bindings
        .review_subject
        .embedded_ref
        .0
        .clone();
    let review: WorkflowBehavioralReviewSubjectDocument = load(&review_ref);

    let corpora = corpus_set
        .workflow_behavioral_corpus_set
        .corpora
        .iter()
        .map(|artifact| WorkflowBehavioralCorpusInput {
            artifact: artifact.clone(),
            document: load::<WorkflowBehavioralScenarioCorpusDocument>(&artifact.embedded_ref.0),
        })
        .collect::<Vec<_>>();

    let mut refs = BTreeSet::new();
    for value in [
        serde_json::to_value(&expected).expect("report JSON"),
        serde_json::to_value(&coverage).expect("coverage JSON"),
        serde_json::to_value(&corpus_set).expect("corpus-set JSON"),
        serde_json::to_value(&review).expect("review JSON"),
    ] {
        collect_embedded_refs(&value, &mut refs);
    }
    for corpus in &corpora {
        collect_embedded_refs(
            &serde_json::to_value(&corpus.document).expect("corpus JSON"),
            &mut refs,
        );
    }

    let mut sources = HashMap::new();
    let mut bundles = BTreeMap::new();
    for reference in refs {
        let bytes = source_bytes(&reference);
        sources.insert(RepoPath(reference.clone()), bytes.clone());
        if let Ok(text) = std::str::from_utf8(&bytes) {
            if let Ok(document) = yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(text) {
                let digest = workflow_runtime_bundle_digest(&document)
                    .unwrap_or_else(|error| panic!("bundle digest {reference}: {error}"));
                bundles.insert(
                    digest,
                    WorkflowBehavioralBundleInput {
                        artifact: WorkflowBehavioralArtifactReference {
                            id: document.workflow_governance_bundle.id.clone(),
                            embedded_ref: RepoPath(reference),
                            expected_digest: sha256(&bytes),
                        },
                        document,
                    },
                );
            }
        }
    }

    let fresh = evaluate_workflow_behavior(
        &WorkflowBehavioralReportIdentity {
            report_id: frozen.id.clone(),
            report_version: frozen.report_version.clone(),
            corpus_set: frozen.corpus.clone(),
            coverage_policy: frozen.coverage_policy.clone(),
        },
        &coverage,
        &corpus_set,
        &review,
        &corpora,
        &bundles,
        &sources,
    );
    assert_eq!(
        fresh, expected,
        "fresh {batch} execution must preserve every frozen verdict, outcome, receipt projection, and continuation result"
    );

    let ids = fresh
        .workflow_behavioral_shadow_report
        .workflow_reports
        .iter()
        .map(|report| report.bindings.policy_ref.0.clone())
        .collect();
    let scenarios = fresh
        .workflow_behavioral_shadow_report
        .workflow_reports
        .iter()
        .map(|report| usize::from(report.total_scenarios))
        .sum();
    (ids, scenarios)
}

#[test]
fn frozen_p5d4_corpora_reexecute_against_the_admitted_42_policy_runtime() {
    let registry = forge_core_kernel::load_admitted_workflow_governance_reviewed_release_registry()
        .expect("independently admitted release registry");
    let latest = registry.latest_release();
    assert_eq!(latest.policy_count(), 42);
    let promoted: WorkflowGovernanceBundleDocument =
        load("contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml");
    assert_eq!(
        promoted.workflow_governance_bundle.id,
        latest.runtime_bundle().bundle_id
    );
    assert_eq!(
        workflow_runtime_bundle_digest(&promoted).expect("promoted bundle digest"),
        latest.runtime_bundle().bundle_digest
    );
    assert_eq!(promoted.workflow_governance_bundle.policies.len(), 42);

    let operational = forge_core_decisions::load_embedded_catalog();
    assert!(operational.is_clean());
    assert_eq!(operational.catalog.len(), 68);

    let mut replayed = BTreeSet::new();
    let mut scenario_count = 0;
    for batch in BATCHES {
        let (ids, scenarios) = replay_batch(batch);
        assert!(
            replayed.is_disjoint(&ids),
            "duplicate policy across P5d.4 batches"
        );
        replayed.extend(ids);
        scenario_count += scenarios;
    }
    assert_eq!(replayed.len(), 27);
    assert_eq!(scenario_count, 189, "27 policies x 7 scenarios");

    let promoted_by_id = promoted
        .workflow_governance_bundle
        .policies
        .iter()
        .map(|policy| (policy.id.0.as_str(), policy))
        .collect::<BTreeMap<_, _>>();
    for batch in BATCHES {
        let expected: WorkflowBehavioralShadowReportDocument = load(&format!(
            "contracts/evidence/workflow-{batch}-shadow-report-v0.yaml"
        ));
        for report in expected.workflow_behavioral_shadow_report.workflow_reports {
            let policy = promoted_by_id
                .get(report.bindings.policy_ref.0.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "reviewed policy is absent: {}",
                        report.bindings.policy_ref.0
                    )
                });
            assert_eq!(
                workflow_release_policy_digest(policy).expect("promoted policy digest"),
                report.bindings.policy_digest,
                "promoted policy drifted after frozen behavioral review"
            );
        }
    }

    let foundation_count = promoted_by_id
        .keys()
        .filter(|id| !replayed.contains(**id))
        .count();
    assert_eq!(
        foundation_count, 15,
        "the runtime golden path owns the base 15"
    );
}
