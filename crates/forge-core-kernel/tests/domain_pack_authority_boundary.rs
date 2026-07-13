use std::collections::BTreeSet;

use forge_core_contracts::{
    WorkflowFinalLegacyAuthorityState, WorkflowFinalRuntimeDisposition,
    WorkflowFinalScorecardAuthority,
};
use forge_core_kernel::{
    load_admitted_workflow_governance_reviewed_release_registry,
    load_admitted_workflow_retirement_checkpoint, AdmittedWorkflowGovernanceReleaseError,
    AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowRetirementCheckpoint,
    AdmittedWorkflowRetirementError,
};

const DOMAIN_WORKFLOW_IDS: [&str; 18] = [
    "game-brief",
    "game-context",
    "game-e2e-scaffold",
    "game-prd",
    "game-project",
    "game-qa-review",
    "game-retrospective",
    "game-sprint-planning",
    "game-sprint-status",
    "game-story-creation",
    "game-test-automation",
    "game-test-framework",
    "game-ux-design",
    "gdd",
    "mechanics-design",
    "narrative-design",
    "playtest-plan",
    "visual-alignment-prototype",
];

fn assert_core_only_registry(registry: &AdmittedWorkflowGovernanceReleaseRegistry) {
    assert_eq!(registry.release_count(), 5);
    assert_eq!(registry.latest_release().policy_count(), 42);
    for workflow_id in DOMAIN_WORKFLOW_IDS {
        assert!(
            !registry
                .latest_release()
                .contains_workflow_policy(workflow_id),
            "domain workflow {workflow_id} crossed the fixed core admission boundary"
        );
    }
}

#[test]
fn retirement_checkpoint_cannot_expand_the_admitted_core_registry() {
    let before = load_admitted_workflow_governance_reviewed_release_registry()
        .expect("fixed reviewed registry");
    assert_core_only_registry(&before);

    let registry_digest = before.registry_digest().to_owned();
    let release_id = before.latest_release().release().release_id.clone();
    let runtime_digest = before.latest_release().digest().to_owned();

    let checkpoint =
        load_admitted_workflow_retirement_checkpoint().expect("fixed retirement checkpoint");
    let scorecard = &checkpoint.scorecard().workflow_final_scorecard;
    assert_eq!(
        scorecard.authority,
        WorkflowFinalScorecardAuthority::DerivedCandidateOnly
    );
    assert_eq!(scorecard.runtime_disposition_counts.executable, 42);
    assert_eq!(
        scorecard.runtime_disposition_counts.domain_pack_candidate,
        18
    );

    let scored_domain_ids = scorecard
        .assessments
        .iter()
        .filter(|assessment| {
            assessment.runtime_disposition == WorkflowFinalRuntimeDisposition::DomainPackCandidate
        })
        .map(|assessment| {
            assert_eq!(
                assessment.legacy_authority,
                WorkflowFinalLegacyAuthorityState::Retained
            );
            assessment.workflow_id.0.as_str()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        scored_domain_ids,
        DOMAIN_WORKFLOW_IDS.into_iter().collect::<BTreeSet<_>>()
    );
    assert!(checkpoint
        .tombstones()
        .workflow_retirement_tombstone_catalog
        .tombstones
        .iter()
        .all(|tombstone| !DOMAIN_WORKFLOW_IDS.contains(&tombstone.workflow_id.0.as_str())));

    let after = load_admitted_workflow_governance_reviewed_release_registry()
        .expect("fixed reviewed registry after retirement projection");
    assert_core_only_registry(&after);
    assert_eq!(after.registry_digest(), registry_digest);
    assert_eq!(after.latest_release().release().release_id, release_id);
    assert_eq!(after.latest_release().digest(), runtime_digest);
}

#[test]
fn caller_yaml_has_no_argument_to_either_opaque_admission_loader() {
    // These function-pointer checks are compile-time API assertions: the only
    // public kernel loaders accept no caller document, path, manifest, pack, or
    // projection. If either boundary starts accepting raw input, this test no
    // longer compiles.
    let reviewed_loader: fn() -> Result<
        AdmittedWorkflowGovernanceReleaseRegistry,
        AdmittedWorkflowGovernanceReleaseError,
    > = load_admitted_workflow_governance_reviewed_release_registry;
    let retirement_loader: fn() -> Result<
        &'static AdmittedWorkflowRetirementCheckpoint,
        AdmittedWorkflowRetirementError,
    > = load_admitted_workflow_retirement_checkpoint;

    let caller_authored_pack: serde_json::Value = yaml_serde::from_str(
        r#"
schema_version: "0.1"
domain_pack:
  pack_id: "forge.core"
  pack_version: "999.0.0"
  requested_authority: "admit_as_core"
  contributions:
    - workflow_id: "game-brief"
      shadows: "discover-intent"
"#,
    )
    .expect("untrusted YAML remains parseable as inert data");
    assert_eq!(
        caller_authored_pack["domain_pack"]["requested_authority"],
        "admit_as_core"
    );

    let before = reviewed_loader().expect("fixed reviewed registry");
    let before_digest = before.registry_digest().to_owned();
    assert_core_only_registry(&before);

    let checkpoint = retirement_loader().expect("fixed retirement checkpoint");
    assert_eq!(
        checkpoint
            .scorecard()
            .workflow_final_scorecard
            .runtime_disposition_counts
            .domain_pack_candidate,
        18
    );

    let after = reviewed_loader().expect("caller YAML cannot select admission bytes");
    assert_core_only_registry(&after);
    assert_eq!(after.registry_digest(), before_digest);
}
