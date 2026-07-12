use forge_core_contracts::{
    RepoPath, StableId, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowReceiptCarryover, WorkflowReleasePredecessorReference, WorkflowReleaseRegistrySource,
};
use forge_core_decisions::{
    evaluate_workflow_release_registry_evolution, workflow_policy_set_digest,
    workflow_release_manifest_digest, workflow_runtime_bundle_digest,
    WorkflowReleaseRegistryEvaluationAuthority, WorkflowReleaseRegistryEvaluationStatus,
    WorkflowReleaseRegistryEvolutionArtifact, WorkflowReleaseRegistryIssueCode,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct Fixture {
    previous: WorkflowGovernanceReleaseRegistryDocument,
    current: WorkflowGovernanceReleaseRegistryDocument,
    bundles: Vec<WorkflowGovernanceBundleDocument>,
    artifacts: Vec<WorkflowReleaseRegistryEvolutionArtifact>,
}

type FixtureMutation = Box<dyn Fn(&mut Fixture)>;

fn fixture() -> Fixture {
    let previous: WorkflowGovernanceReleaseRegistryDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/migration/workflow-governance-release-registry-v0.yaml"
    ))
    .expect("canonical registry");
    let genesis_bundle: WorkflowGovernanceBundleDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/workflow-governance/golden-path-v0.yaml"
    ))
    .expect("P5c bundle");
    let successor_bundle: WorkflowGovernanceBundleDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/workflow-governance/runtime-release-foundation-v0.yaml"
    ))
    .expect("foundation runtime bundle");
    let mut appended_bundle = successor_bundle.clone();
    appended_bundle.workflow_governance_bundle.id =
        StableId("bundle.workflow-governance.review-candidate-v0".to_owned());
    let appended_bundle_bytes = yaml_serde::to_string(&appended_bundle)
        .expect("candidate bundle YAML")
        .into_bytes();

    let mut appended_manifest: WorkflowGovernanceReleaseManifestDocument = yaml_serde::from_str(
        include_str!("../../../contracts/migration/workflow-governance-release-foundation-v0.yaml"),
    )
    .expect("foundation release manifest");

    let mut current = previous.clone();
    let registry = &mut current.workflow_governance_release_registry;
    "0.2.0".clone_into(&mut registry.registry_version);
    let predecessor = registry.releases[1].clone();
    let mut appended = predecessor.clone();
    appended.release.release_id =
        StableId("workflow-governance.release.review-candidate-v0".to_owned());
    "0.2.0".clone_into(&mut appended.release.release_version);
    appended_manifest
        .workflow_governance_release_manifest
        .release_id = appended.release.release_id.clone();
    appended_manifest
        .workflow_governance_release_manifest
        .release_version
        .clone_from(&appended.release.release_version);
    appended_manifest
        .workflow_governance_release_manifest
        .previous_release_digest = Some(predecessor.release.release_digest.clone());
    let appended_manifest_bytes = yaml_serde::to_string(&appended_manifest)
        .expect("candidate manifest YAML")
        .into_bytes();
    appended.release.release_digest =
        workflow_release_manifest_digest(&appended_manifest).expect("candidate release digest");
    appended.predecessor = Some(WorkflowReleasePredecessorReference {
        release_id: predecessor.release.release_id,
        release_digest: predecessor.release.release_digest,
    });
    appended.source = WorkflowReleaseRegistrySource::EmbeddedManifest {
        embedded_ref: RepoPath(
            "contracts/migration/workflow-governance-release-review-candidate-v0.yaml".to_owned(),
        ),
        expected_digest: bytes_digest(&appended_manifest_bytes),
    };
    appended.runtime_bundle.identity.bundle_id =
        appended_bundle.workflow_governance_bundle.id.clone();
    appended.runtime_bundle.identity.bundle_digest =
        workflow_runtime_bundle_digest(&appended_bundle).expect("candidate bundle digest");
    appended.runtime_bundle.identity.policy_set_digest =
        workflow_policy_set_digest(&appended_bundle.workflow_governance_bundle.policies)
            .expect("candidate policy-set digest");
    appended.runtime_bundle.embedded_ref = RepoPath(
        "contracts/workflow-governance/runtime-release-review-candidate-v0.yaml".to_owned(),
    );
    appended.runtime_bundle.expected_digest = bytes_digest(&appended_bundle_bytes);
    appended.receipt_carryover = WorkflowReceiptCarryover::PreservePolicyEquivalent;
    registry.default_successor_release_id = appended.release.release_id.clone();
    registry.releases.push(appended);

    Fixture {
        previous,
        current,
        bundles: vec![genesis_bundle, successor_bundle, appended_bundle],
        artifacts: vec![
            WorkflowReleaseRegistryEvolutionArtifact {
                embedded_ref: RepoPath(
                    "contracts/migration/workflow-governance-release-review-candidate-v0.yaml"
                        .to_owned(),
                ),
                bytes: appended_manifest_bytes,
            },
            WorkflowReleaseRegistryEvolutionArtifact {
                embedded_ref: RepoPath(
                    "contracts/workflow-governance/runtime-release-review-candidate-v0.yaml"
                        .to_owned(),
                ),
                bytes: appended_bundle_bytes,
            },
        ],
    }
}

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn bytes_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn issues(case: &Fixture) -> Vec<WorkflowReleaseRegistryIssueCode> {
    evaluate_workflow_release_registry_evolution(
        &case.previous,
        &case.current,
        &case.bundles,
        &case.artifacts,
    )
    .issues
    .into_iter()
    .map(|issue| issue.code)
    .collect()
}

#[test]
fn exact_append_with_linear_chain_and_equivalent_policy_set_is_structurally_valid_only() {
    let case = fixture();
    let result = evaluate_workflow_release_registry_evolution(
        &case.previous,
        &case.current,
        &case.bundles,
        &case.artifacts,
    );
    assert_eq!(
        result.status,
        WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
    );
    assert_eq!(
        result.authority,
        WorkflowReleaseRegistryEvaluationAuthority::NonAuthoritative
    );
    assert_eq!(result.previous_release_count, 2);
    assert_eq!(result.current_release_count, 3);
    assert_eq!(result.appended_release_count, 1);
    assert!(result.issues.is_empty());
    assert_ne!(
        result.previous_registry_digest,
        result.current_registry_digest
    );
}

#[test]
fn registry_identity_and_version_are_immutable_and_monotonic() {
    let mutations: Vec<FixtureMutation> = vec![
        Box::new(|case| {
            case.current
                .workflow_governance_release_registry
                .registry_id = StableId("replacement.registry".to_owned());
        }),
        Box::new(|case| {
            case.current.workflow_governance_release_registry.lineage_id =
                StableId("replacement.lineage".to_owned());
        }),
        Box::new(|case| {
            case.current
                .workflow_governance_release_registry
                .registry_version = "0.1.0".to_owned();
        }),
        Box::new(|case| {
            case.current
                .workflow_governance_release_registry
                .registry_version = "0.2.0-alpha.1".to_owned();
            case.previous
                .workflow_governance_release_registry
                .registry_version = "0.2.0".to_owned();
        }),
    ];
    for mutate in mutations {
        let mut case = fixture();
        mutate(&mut case);
        let found = issues(&case);
        assert!(found.iter().any(|code| {
            matches!(
                code,
                WorkflowReleaseRegistryIssueCode::RegistryIdentityChanged
                    | WorkflowReleaseRegistryIssueCode::RegistryVersionNotIncreasing
            )
        }));
    }
}

#[test]
fn historical_prefix_cannot_be_removed_changed_or_reordered() {
    let mut removed = fixture();
    removed
        .current
        .workflow_governance_release_registry
        .releases
        .remove(1);
    assert!(issues(&removed).contains(&WorkflowReleaseRegistryIssueCode::HistoricalEntryChanged));

    let mut changed = fixture();
    changed
        .current
        .workflow_governance_release_registry
        .releases[1]
        .release
        .release_digest = digest('d');
    assert!(issues(&changed).contains(&WorkflowReleaseRegistryIssueCode::HistoricalEntryChanged));

    let mut reordered = fixture();
    reordered
        .current
        .workflow_governance_release_registry
        .releases
        .swap(0, 1);
    let found = issues(&reordered);
    assert!(found.contains(&WorkflowReleaseRegistryIssueCode::HistoricalEntryChanged));
    assert!(found.contains(&WorkflowReleaseRegistryIssueCode::NonLinearReleaseChain));

    let mut truncated = fixture();
    truncated
        .current
        .workflow_governance_release_registry
        .releases
        .truncate(1);
    assert!(
        issues(&truncated).contains(&WorkflowReleaseRegistryIssueCode::HistoricalReleaseRemoved)
    );
}

#[test]
fn version_bump_without_an_appended_release_is_rejected() {
    let mut case = fixture();
    case.current
        .workflow_governance_release_registry
        .releases
        .pop();
    case.current
        .workflow_governance_release_registry
        .default_successor_release_id = case.current.workflow_governance_release_registry.releases
        [1]
    .release
    .release_id
    .clone();
    assert!(issues(&case).contains(&WorkflowReleaseRegistryIssueCode::NoReleaseAppended));
}

#[test]
fn missing_skipped_cyclic_and_forked_predecessors_fail_closed() {
    for predecessor in [
        None,
        Some(WorkflowReleasePredecessorReference {
            release_id: StableId("workflow-governance.release.p5c-implicit-v0".to_owned()),
            release_digest: digest('1'),
        }),
        Some(WorkflowReleasePredecessorReference {
            release_id: StableId("workflow-governance.release.review-candidate-v0".to_owned()),
            release_digest: digest('a'),
        }),
    ] {
        let mut case = fixture();
        case.current.workflow_governance_release_registry.releases[2].predecessor = predecessor;
        assert!(issues(&case).contains(&WorkflowReleaseRegistryIssueCode::NonLinearReleaseChain));
    }

    let mut fork = fixture();
    let mut fourth = fork.current.workflow_governance_release_registry.releases[2].clone();
    fourth.release.release_id = StableId("workflow-governance.release.fork-v0".to_owned());
    fourth.release.release_digest = digest('e');
    fourth.runtime_bundle.identity.bundle_id =
        StableId("bundle.workflow-governance.fork-v0".to_owned());
    let mut fourth_bundle = fork.bundles[2].clone();
    fourth_bundle.workflow_governance_bundle.id = fourth.runtime_bundle.identity.bundle_id.clone();
    fourth.runtime_bundle.identity.bundle_digest =
        workflow_runtime_bundle_digest(&fourth_bundle).expect("fourth bundle digest");
    fourth.predecessor = fork.current.workflow_governance_release_registry.releases[2]
        .predecessor
        .clone();
    fork.current
        .workflow_governance_release_registry
        .default_successor_release_id = fourth.release.release_id.clone();
    fork.current
        .workflow_governance_release_registry
        .releases
        .push(fourth);
    fork.bundles.push(fourth_bundle);
    assert!(issues(&fork).contains(&WorkflowReleaseRegistryIssueCode::NonLinearReleaseChain));
}

#[test]
fn genesis_and_receipt_carryover_rules_cannot_be_bypassed() {
    let mut non_genesis_not_applicable = fixture();
    non_genesis_not_applicable
        .current
        .workflow_governance_release_registry
        .releases[2]
        .receipt_carryover = WorkflowReceiptCarryover::NotApplicable;
    assert!(issues(&non_genesis_not_applicable)
        .contains(&WorkflowReleaseRegistryIssueCode::ReceiptCarryoverInvalid));

    let mut second_genesis = fixture();
    second_genesis
        .current
        .workflow_governance_release_registry
        .releases[2]
        .source = WorkflowReleaseRegistrySource::ImplicitP5cGenesis;
    assert!(
        issues(&second_genesis).contains(&WorkflowReleaseRegistryIssueCode::NonLinearReleaseChain)
    );

    let mut changed_policy = fixture();
    changed_policy.bundles[2]
        .workflow_governance_bundle
        .policies
        .pop();
    let appended = &mut changed_policy
        .current
        .workflow_governance_release_registry
        .releases[2];
    appended.runtime_bundle.identity.bundle_digest =
        workflow_runtime_bundle_digest(&changed_policy.bundles[2]).expect("changed bundle digest");
    appended.runtime_bundle.identity.policy_set_digest = workflow_policy_set_digest(
        &changed_policy.bundles[2]
            .workflow_governance_bundle
            .policies,
    )
    .expect("changed policy digest");
    assert!(issues(&changed_policy)
        .contains(&WorkflowReleaseRegistryIssueCode::ReceiptCarryoverInvalid));

    changed_policy
        .current
        .workflow_governance_release_registry
        .releases[2]
        .receipt_carryover = WorkflowReceiptCarryover::InvalidateAll;
    assert!(!issues(&changed_policy)
        .contains(&WorkflowReleaseRegistryIssueCode::ReceiptCarryoverInvalid));
}

#[test]
fn authored_bundle_digests_cannot_replace_supplied_canonical_bytes() {
    let mut missing = fixture();
    missing.bundles.pop();
    assert!(issues(&missing).contains(&WorkflowReleaseRegistryIssueCode::SuppliedBundleMissing));

    let mut drift = fixture();
    drift.bundles[2].workflow_governance_bundle.policies.pop();
    let found = issues(&drift);
    assert!(found.contains(&WorkflowReleaseRegistryIssueCode::RuntimeBundleIdentityMismatch));
    assert!(found.contains(&WorkflowReleaseRegistryIssueCode::PolicySetDrift));
}

#[test]
fn arbitrary_length_semver_precedence_is_panic_free_and_numeric() {
    let mut huge_core = fixture();
    huge_core
        .current
        .workflow_governance_release_registry
        .registry_version = "18446744073709551616.0.0".to_owned();
    let found = issues(&huge_core);
    assert!(found.is_empty(), "{found:?}");

    let mut huge_prerelease = fixture();
    huge_prerelease
        .previous
        .workflow_governance_release_registry
        .registry_version = "1.0.0-99999999999999999999".to_owned();
    huge_prerelease
        .current
        .workflow_governance_release_registry
        .registry_version = "1.0.0-100000000000000000000".to_owned();
    let found = issues(&huge_prerelease);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn appended_manifest_and_bundle_require_exact_raw_bytes() {
    let mut missing_manifest = fixture();
    missing_manifest
        .artifacts
        .retain(|artifact| !artifact.embedded_ref.0.starts_with("contracts/migration/"));
    assert!(issues(&missing_manifest)
        .contains(&WorkflowReleaseRegistryIssueCode::EmbeddedReferenceMissing));

    let mut drifted_bundle = fixture();
    let bundle = drifted_bundle
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.embedded_ref.0.contains("runtime-release-review"))
        .expect("candidate bundle artifact");
    bundle.bytes.extend_from_slice(b"\n# byte drift\n");
    assert!(
        issues(&drifted_bundle).contains(&WorkflowReleaseRegistryIssueCode::EmbeddedDigestMismatch)
    );

    let mut invalid_manifest = fixture();
    let manifest = invalid_manifest
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.embedded_ref.0.starts_with("contracts/migration/"))
        .expect("candidate manifest artifact");
    manifest.bytes = b"not: [valid".to_vec();
    assert!(issues(&invalid_manifest)
        .contains(&WorkflowReleaseRegistryIssueCode::EmbeddedDocumentInvalid));
}

#[test]
fn appended_release_identity_is_recomputed_from_exact_manifest_bytes() {
    let mut case = fixture();
    case.current.workflow_governance_release_registry.releases[2]
        .release
        .release_digest = digest('d');
    assert!(
        issues(&case).contains(&WorkflowReleaseRegistryIssueCode::ReleaseManifestIdentityMismatch)
    );
}
