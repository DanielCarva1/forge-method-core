use forge_core_schema::{compact_agent_views, generated_contract_schemas};
use std::collections::HashSet;

#[test]
fn generated_schemas_cover_v0_contract_surface() {
    let schemas = generated_contract_schemas();
    let families = schemas
        .iter()
        .map(|artifact| artifact.family_id)
        .collect::<HashSet<_>>();

    for expected in [
        "operation_contract",
        "operation_reference_policy",
        "command_contract",
        "claim_contract",
        "completion_contract",
        "gate_contract",
        "request_contract",
        "tool_effect_contract",
        "decision_close_contract",
        "runtime_handoff_contract",
        "runtime_registry_entry",
        "runtime_capability",
        "health_recovery_contract",
        "coordination_eval_contract",
        "assurance_case",
        "contract_family_inventory",
        "field_evidence_registry",
        "workflow_migration_plan",
        "workflow_governance_release_manifest",
        "workflow_migration_batch",
        "workflow_retirement_authorization",
        "workflow_governance_bundle",
        "workflow_governance_evaluation",
        "workflow_governance_ledger",
        "workflow_governance_receipt",
    ] {
        assert!(families.contains(expected), "missing schema for {expected}");
    }
}

#[test]
fn compact_agent_views_are_derived_and_nonempty() {
    let views = compact_agent_views();
    assert_eq!(views.len(), generated_contract_schemas().len());

    for view in views {
        assert!(
            !view.top_level_required_fields.is_empty(),
            "top-level required fields missing for {}",
            view.family_id
        );
        assert!(
            !view.authority_note.trim().is_empty(),
            "authority note missing for {}",
            view.family_id
        );
        if view.root_key.is_some() {
            assert!(
                !view.contract_required_fields.is_empty(),
                "contract required fields missing for {}",
                view.family_id
            );
        }
    }
}

#[test]
fn workflow_governance_views_make_non_mutation_boundary_explicit() {
    let views = compact_agent_views();
    let bundle = views
        .iter()
        .find(|view| view.family_id == "workflow_governance_bundle")
        .expect("bundle schema view");
    assert_eq!(bundle.root_key, Some("workflow_governance_bundle"));
    assert!(bundle.authority_note.contains("simulation-only"));
    assert!(bundle
        .authority_note
        .contains("opaque trusted kernel snapshot"));

    let evaluation = views
        .iter()
        .find(|view| view.family_id == "workflow_governance_evaluation")
        .expect("evaluation schema view");
    assert_eq!(evaluation.root_key, Some("workflow_governance_evaluation"));
    assert!(evaluation
        .authority_note
        .contains("candidate completion is not authority"));
}

#[test]
fn workflow_release_views_keep_candidate_and_trusted_authority_boundaries_explicit() {
    let views = compact_agent_views();

    let manifest = views
        .iter()
        .find(|view| view.family_id == "workflow_governance_release_manifest")
        .expect("release manifest schema view");
    assert_eq!(
        manifest.root_key,
        Some("workflow_governance_release_manifest")
    );
    assert!(manifest.authority_note.contains("intent only"));
    assert!(manifest
        .authority_note
        .contains("trusted derived admission"));

    let batch = views
        .iter()
        .find(|view| view.family_id == "workflow_migration_batch")
        .expect("migration batch schema view");
    assert_eq!(batch.root_key, Some("workflow_migration_batch"));
    assert!(batch.authority_note.contains("candidate-only"));
    assert!(batch
        .authority_note
        .contains("never grant executable authority"));

    let retirement = views
        .iter()
        .find(|view| view.family_id == "workflow_retirement_authorization")
        .expect("retirement authorization schema view");
    assert_eq!(
        retirement.root_key,
        Some("workflow_retirement_authorization")
    );
    assert!(retirement.authority_note.contains("signature verification"));
    assert!(retirement
        .authority_note
        .contains("deserialization is not authority"));
}
