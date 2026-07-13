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
        "workflow_governance_policy_overlay",
        "workflow_behavioral_review_subject",
        "workflow_behavioral_coverage_policy",
        "workflow_behavioral_scenario_corpus",
        "workflow_behavioral_corpus_set",
        "workflow_behavioral_shadow_report",
        "workflow_release_review_index",
        "workflow_release_reviewer_registry",
        "workflow_release_admission_authorization",
        "workflow_governance_release_manifest",
        "workflow_migration_batch",
        "workflow_retirement_authorization",
        "workflow_retirement_evidence_index",
        "workflow_deletion_proof",
        "workflow_consumer_compatibility_report",
        "workflow_consumer_compatibility_matrix",
        "workflow_retirement_tombstone_catalog",
        "workflow_final_scorecard",
        "workflow_retirement_authorization_v2",
        "workflow_governance_bundle",
        "workflow_governance_evaluation",
        "workflow_governance_ledger",
        "workflow_governance_receipt",
    ] {
        assert!(families.contains(expected), "missing schema for {expected}");
    }
}

#[test]
fn p6b_schema_registry_covers_every_closed_lifecycle_family() {
    let schemas = generated_contract_schemas();
    let views = compact_agent_views();
    for (family, root) in [
        ("domain_pack_trust_policy", "domain_pack_trust_policy"),
        (
            "domain_pack_supply_chain_registry",
            "domain_pack_supply_chain_registry",
        ),
        (
            "domain_pack_runtime_capability_registry",
            "domain_pack_runtime_capability_registry",
        ),
        (
            "domain_pack_capability_sandbox_policy",
            "domain_pack_capability_sandbox_policy",
        ),
        (
            "domain_pack_resolution_request",
            "domain_pack_resolution_request",
        ),
        (
            "domain_pack_resolution_projection",
            "domain_pack_resolution_projection",
        ),
        ("domain_pack_exact_lock", "domain_pack_exact_lock"),
        (
            "domain_pack_compatibility_report",
            "domain_pack_compatibility_report",
        ),
        (
            "domain_pack_lifecycle_request",
            "domain_pack_lifecycle_request",
        ),
        (
            "domain_pack_lifecycle_preflight",
            "domain_pack_lifecycle_preflight",
        ),
        ("domain_pack_active_pointer", "domain_pack_active_pointer"),
        (
            "domain_pack_lifecycle_ledger",
            "domain_pack_lifecycle_ledger",
        ),
        (
            "domain_pack_lifecycle_receipt",
            "domain_pack_lifecycle_receipt",
        ),
        ("domain_pack_recovery_report", "domain_pack_recovery_report"),
    ] {
        let schema = schemas
            .iter()
            .find(|schema| schema.family_id == family)
            .unwrap_or_else(|| panic!("missing P6b schema {family}"));
        assert_eq!(schema.root_key, Some(root));
        assert_eq!(schema.schema["x-forge-family-id"], family);
        let view = views
            .iter()
            .find(|view| view.family_id == family)
            .unwrap_or_else(|| panic!("missing P6b compact view {family}"));
        assert_eq!(view.root_key, Some(root));
        assert!(
            view.authority_note.contains("candidate")
                || view.authority_note.contains("trusted")
                || view.authority_note.contains("cannot")
                || view.authority_note.contains("required")
        );
    }
}

#[test]
fn p5d5_retirement_views_preserve_candidate_only_two_axis_boundary() {
    let views = compact_agent_views();
    for (family, root) in [
        (
            "workflow_retirement_evidence_index",
            "workflow_retirement_evidence_index",
        ),
        ("workflow_deletion_proof", "workflow_deletion_proof"),
        (
            "workflow_consumer_compatibility_report",
            "workflow_consumer_compatibility_report",
        ),
        (
            "workflow_consumer_compatibility_matrix",
            "workflow_consumer_compatibility_matrix",
        ),
        (
            "workflow_retirement_tombstone_catalog",
            "workflow_retirement_tombstone_catalog",
        ),
        ("workflow_final_scorecard", "workflow_final_scorecard"),
        (
            "workflow_retirement_authorization_v2",
            "workflow_retirement_authorization_v2",
        ),
    ] {
        let view = views
            .iter()
            .find(|view| view.family_id == family)
            .unwrap_or_else(|| panic!("missing P5d.5 view {family}"));
        assert_eq!(view.root_key, Some(root));
        assert!(
            view.authority_note.contains("candidate")
                || view.authority_note.contains("non-authoritative")
        );
        assert!(
            view.authority_note.contains("cannot")
                || view.authority_note.contains("required")
                || view.authority_note.contains("not ")
        );
    }
    let scorecard = views
        .iter()
        .find(|view| view.family_id == "workflow_final_scorecard")
        .expect("final scorecard view");
    assert!(scorecard
        .enum_definitions
        .contains(&"WorkflowFinalRuntimeDisposition".to_owned()));
    assert!(scorecard
        .enum_definitions
        .contains(&"WorkflowFinalLegacyAuthorityState".to_owned()));
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

#[test]
fn p5d3_schema_selectors_use_exact_document_roots_and_non_authority_notes() {
    let schemas = generated_contract_schemas();
    let views = compact_agent_views();
    let expected = [
        (
            "workflow_governance_policy_overlay",
            "WorkflowGovernancePolicyOverlayDocument",
            "workflow_governance_policy_overlay",
            "id",
        ),
        (
            "workflow_behavioral_review_subject",
            "WorkflowBehavioralReviewSubjectDocument",
            "workflow_behavioral_review_subject",
            "proposed_release",
        ),
        (
            "workflow_behavioral_coverage_policy",
            "WorkflowBehavioralCoveragePolicyDocument",
            "workflow_behavioral_coverage_policy",
            "required_scenario_kinds",
        ),
        (
            "workflow_behavioral_scenario_corpus",
            "WorkflowBehavioralScenarioCorpusDocument",
            "workflow_behavioral_scenario_corpus",
            "workflow_evidence",
        ),
        (
            "workflow_behavioral_corpus_set",
            "WorkflowBehavioralCorpusSetDocument",
            "workflow_behavioral_corpus_set",
            "corpora",
        ),
        (
            "workflow_behavioral_shadow_report",
            "WorkflowBehavioralShadowReportDocument",
            "workflow_behavioral_shadow_report",
            "verdict",
        ),
    ];

    for (family, document_type, root, contract_field) in expected {
        let schema = schemas
            .iter()
            .find(|artifact| artifact.family_id == family)
            .unwrap_or_else(|| panic!("missing schema selector {family}"));
        assert_eq!(schema.document_type, document_type);
        assert_eq!(schema.root_key, Some(root));
        assert_eq!(schema.schema["x-forge-family-id"], family);
        assert!(schema.schema["x-forge-authority-note"]
            .as_str()
            .is_some_and(|note| {
                note.contains("non-authoritative") || note.contains("candidate")
            }));

        let view = views
            .iter()
            .find(|view| view.family_id == family)
            .expect("compact P5d.3 view");
        assert_eq!(view.document_type, document_type);
        assert_eq!(view.root_key, Some(root));
        assert!(view
            .top_level_required_fields
            .contains(&"schema_version".to_owned()));
        assert!(view.top_level_required_fields.contains(&root.to_owned()));
        assert!(view
            .contract_required_fields
            .contains(&contract_field.to_owned()));
        assert!(view.authority_note.contains("non-authoritative"));
        assert!(view.authority_note.contains("cannot") || view.authority_note.contains("not "));
    }
}

#[test]
fn p5d3_behavioral_schemas_expose_closed_authority_and_verdict_enums() {
    let views = compact_agent_views();
    let report = views
        .iter()
        .find(|view| view.family_id == "workflow_behavioral_shadow_report")
        .expect("shadow report view");
    assert!(report
        .enum_definitions
        .contains(&"WorkflowBehavioralEvidenceAuthority".to_owned()));
    assert!(report
        .enum_definitions
        .contains(&"WorkflowBehavioralVerdict".to_owned()));
    assert!(report
        .enum_definitions
        .contains(&"WorkflowBehavioralDisposition".to_owned()));

    let review = views
        .iter()
        .find(|view| view.family_id == "workflow_behavioral_review_subject")
        .expect("review subject view");
    assert!(review
        .enum_definitions
        .contains(&"WorkflowBehavioralReviewSubjectAuthority".to_owned()));
}

#[test]
fn p5d4_review_schemas_expose_candidate_only_authority_boundaries() {
    let views = compact_agent_views();
    let expected = [
        (
            "workflow_release_review_index",
            "workflow_release_review_index",
            "workflow_decisions",
        ),
        (
            "workflow_release_reviewer_registry",
            "workflow_release_reviewer_registry",
            "credentials",
        ),
        (
            "workflow_release_admission_authorization",
            "workflow_release_admission_authorization",
            "signatures",
        ),
    ];

    for (family, root, required) in expected {
        let view = views
            .iter()
            .find(|view| view.family_id == family)
            .unwrap_or_else(|| panic!("missing P5d.4 schema view {family}"));
        assert_eq!(view.root_key, Some(root));
        assert!(view.contract_required_fields.contains(&required.to_owned()));
        assert!(view.authority_note.contains("candidate"));
        assert!(view.authority_note.contains("cannot") || view.authority_note.contains("required"));
    }

    let authorization = views
        .iter()
        .find(|view| view.family_id == "workflow_release_admission_authorization")
        .expect("authorization view");
    assert!(authorization
        .enum_definitions
        .contains(&"WorkflowReleaseAdmissionAuthorizationAuthority".to_owned()));
    assert!(authorization
        .enum_definitions
        .contains(&"WorkflowReleaseReviewerRole".to_owned()));
}
