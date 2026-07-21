use forge_core_contracts::operation::{ForgeOperation, NextActor};
use forge_core_contracts::{
    GuideDecision, GuideProtocol, GuideProtocolDocument, OperationContractDocument, StableId,
    GUIDE_PROTOCOL_SCHEMA_VERSION,
};
use forge_core_decisions::load_catalog;
use forge_core_kernel::{validate_guide_protocol, GuideProtocolRejectionCode, GuideRoute};
use std::path::{Path, PathBuf};

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn catalog() -> forge_core_contracts::Catalog {
    let report = load_catalog(&workspace_path("contracts/workflows"));
    assert!(report.is_clean(), "catalog errors: {:?}", report.errors);
    report.catalog
}

fn operation_fixture(name: &str) -> OperationContractDocument {
    let text = std::fs::read_to_string(workspace_path(&format!(
        "docs/fixtures/operation-contract-v0/{name}.yaml"
    )))
    .expect("read operation fixture");
    yaml_serde::from_str(&text).expect("deserialize operation fixture")
}

fn protocol_from_operation(operation: OperationContractDocument) -> GuideProtocolDocument {
    let route = &operation.operation_contract;
    GuideProtocolDocument {
        schema_version: GUIDE_PROTOCOL_SCHEMA_VERSION.to_owned(),
        guide_protocol: GuideProtocol {
            decision: GuideDecision {
                recommended_workflow: route.recommendation.workflow.clone(),
                reason: "fixture route".to_owned(),
                allowed_actions: route.allowed_actions.clone(),
                blocked_by_gates: Vec::new(),
                current_phase: route.recommendation.phase.clone(),
                proposed_next_phase: None,
            },
            next_operation: operation,
        },
    }
}

#[test]
fn closed_guide_routes_return_the_exact_validated_operation_contract() {
    let catalog = catalog();
    let cases = [
        ("facilitate-first-product-idea", GuideRoute::Facilitation),
        ("research-market-scan", GuideRoute::Research),
        ("correct-course-frustrated-user", GuideRoute::CorrectCourse),
        ("already-done-story", GuideRoute::AlreadyDone),
        ("mechanical-story-execute", GuideRoute::MechanicalExecution),
    ];

    for (fixture, expected) in cases {
        let protocol = protocol_from_operation(operation_fixture(fixture));
        let exact_contract_id = protocol
            .guide_protocol
            .next_operation
            .operation_contract
            .contract_id
            .clone();
        let route = validate_guide_protocol(&protocol, &catalog, &[])
            .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
        assert_eq!(route, expected, "{fixture}");
        assert_eq!(
            protocol
                .guide_protocol
                .next_operation
                .operation_contract
                .contract_id,
            exact_contract_id,
            "validation must not rewrite the exact operation contract"
        );
    }
}

#[test]
fn visual_alignment_is_a_closed_facilitation_route() {
    let catalog = catalog();
    let mut operation = operation_fixture("facilitate-first-product-idea");
    operation.operation_contract.contract_id = StableId("op_visual_alignment".to_owned());
    operation.operation_contract.recommendation.workflow =
        StableId("visual-alignment-prototype".to_owned());
    operation.operation_contract.recommendation.action =
        StableId("align_visual_direction".to_owned());
    let protocol = protocol_from_operation(operation);

    assert_eq!(
        validate_guide_protocol(&protocol, &catalog, &[]),
        Ok(GuideRoute::VisualAlignment)
    );
}

#[test]
fn host_cannot_invent_phase_or_workflow_control_flow() {
    let catalog = catalog();
    let mut phase = protocol_from_operation(operation_fixture("facilitate-first-product-idea"));
    phase
        .guide_protocol
        .next_operation
        .operation_contract
        .recommendation
        .phase = StableId("3-plan".to_owned());
    let error = validate_guide_protocol(&phase, &catalog, &[]).expect_err("phase mismatch");
    assert_eq!(error.code, GuideProtocolRejectionCode::PhaseMismatch);

    let mut workflow = protocol_from_operation(operation_fixture("facilitate-first-product-idea"));
    workflow
        .guide_protocol
        .next_operation
        .operation_contract
        .recommendation
        .workflow = StableId("market-scan".to_owned());
    let error = validate_guide_protocol(&workflow, &catalog, &[]).expect_err("workflow mismatch");
    assert_eq!(error.code, GuideProtocolRejectionCode::WorkflowMismatch);

    let mut source = protocol_from_operation(operation_fixture("facilitate-first-product-idea"));
    source
        .guide_protocol
        .next_operation
        .operation_contract
        .source
        .operation = ForgeOperation::Gate;
    let error = validate_guide_protocol(&source, &catalog, &[]).expect_err("source mismatch");
    assert_eq!(
        error.code,
        GuideProtocolRejectionCode::OperationSourceNotGuide
    );
}

#[test]
fn route_policy_and_allowed_actions_fail_closed() {
    let catalog = catalog();
    let mut actor = protocol_from_operation(operation_fixture("facilitate-first-product-idea"));
    actor
        .guide_protocol
        .next_operation
        .operation_contract
        .recommendation
        .next_actor = NextActor::HostAgent;
    let error = validate_guide_protocol(&actor, &catalog, &[]).expect_err("route policy");
    assert_eq!(error.code, GuideProtocolRejectionCode::RoutePolicyMismatch);

    let mut actions = protocol_from_operation(operation_fixture("facilitate-first-product-idea"));
    actions
        .guide_protocol
        .decision
        .allowed_actions
        .push(StableId("invent_control_flow".to_owned()));
    let error = validate_guide_protocol(&actions, &catalog, &[]).expect_err("action mismatch");
    assert_eq!(
        error.code,
        GuideProtocolRejectionCode::AllowedActionsMismatch
    );
}
