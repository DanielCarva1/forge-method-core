use forge_core_contracts::{
    LegacyWorkflowField, LegacyWorkflowFieldRole, WorkflowMigrationPlanDocument,
    WorkflowShadowMode, WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
};
use std::path::PathBuf;

fn plan_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/policies/workflow-migration-foundation-v0.yaml")
}

#[test]
fn published_workflow_migration_plan_round_trips_closed_schema() {
    let yaml = std::fs::read_to_string(plan_path()).expect("migration plan");
    let document: WorkflowMigrationPlanDocument =
        yaml_serde::from_str(&yaml).expect("typed migration plan");
    assert_eq!(
        document.schema_version,
        WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION
    );
    assert_eq!(document.workflow_migration_plan.expected_catalog_count, 110);
    assert_eq!(
        document
            .workflow_migration_plan
            .compatibility_projection
            .mode,
        WorkflowShadowMode::ReadOnlyExactProjection
    );
    assert!(document
        .workflow_migration_plan
        .field_mappings
        .iter()
        .any(|mapping| mapping.field == LegacyWorkflowField::Steps
            && mapping.role == LegacyWorkflowFieldRole::AdvisoryPlaybook));
    let serialized = yaml_serde::to_string(&document).expect("serialize migration plan");
    let round_trip: WorkflowMigrationPlanDocument =
        yaml_serde::from_str(&serialized).expect("round-trip migration plan");
    assert_eq!(round_trip, document);
}

#[test]
fn workflow_migration_plan_rejects_unknown_fields_and_enum_values() {
    let yaml = std::fs::read_to_string(plan_path()).expect("migration plan");
    let unknown = yaml.replace(
        "  expected_catalog_count: 110",
        "  expected_catalog_count: 110\n  caller_authority: true",
    );
    assert!(yaml_serde::from_str::<WorkflowMigrationPlanDocument>(&unknown).is_err());

    let unsafe_role = yaml.replace("role: \"advisory_playbook\"", "role: \"authority_script\"");
    assert!(yaml_serde::from_str::<WorkflowMigrationPlanDocument>(&unsafe_role).is_err());
}
