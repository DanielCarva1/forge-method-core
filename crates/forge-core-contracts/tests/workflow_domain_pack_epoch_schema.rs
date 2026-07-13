use forge_core_contracts::{
    DomainPackGenerationTransitionedEvent, StableId, WorkflowDecisionActivation,
    WorkflowDomainPackGenerationIdentity, WorkflowEffectiveBundleIdentity, WorkflowGovernanceEvent,
    WorkflowReceiptCarryover, WorkflowRuntimeBundleIdentity,
    WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
};

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn runtime(id: &str, byte: char) -> WorkflowRuntimeBundleIdentity {
    WorkflowRuntimeBundleIdentity {
        bundle_id: StableId(id.to_owned()),
        bundle_digest: digest(byte),
        policy_set_digest: digest(char::from_u32(byte as u32 + 1).expect("next char")),
    }
}

#[test]
fn effective_epoch_wire_is_closed_and_round_trips() {
    let core = runtime("bundle.core", '1');
    let from = WorkflowEffectiveBundleIdentity {
        core_runtime_bundle: core.clone(),
        effective_runtime_bundle: core.clone(),
        domain_pack_generation: None,
        receipt_context_digest: digest('3'),
    };
    let to = WorkflowEffectiveBundleIdentity {
        core_runtime_bundle: core,
        effective_runtime_bundle: runtime("bundle.effective", '4'),
        domain_pack_generation: Some(WorkflowDomainPackGenerationIdentity {
            generation: 7,
            active_lock_digest: digest('6'),
            composition_digest: digest('7'),
            base_core_bundle_digest: digest('8'),
            supply_chain_registry_digest: digest('9'),
            reviewer_registry_digest: digest('a'),
            reviewed_registry_digest: digest('b'),
        }),
        receipt_context_digest: digest('c'),
    };
    let event = WorkflowGovernanceEvent::DomainPackGenerationTransitioned(
        DomainPackGenerationTransitionedEvent {
            from_effective_bundle: from,
            to_effective_bundle: to,
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            prior_ledger_head_digest: digest('d'),
        },
    );
    let value = serde_json::to_value(&event).expect("serialize closed event");
    assert_eq!(value["type"], "domain_pack_generation_transitioned");
    let round_trip: WorkflowGovernanceEvent =
        serde_json::from_value(value.clone()).expect("round trip");
    assert_eq!(round_trip, event);
    let mut unknown = value;
    unknown["payload"]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowGovernanceEvent>(unknown).is_err());
    assert_eq!(WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION, "0.2");
}

#[test]
fn claim_verified_activation_has_an_additive_stable_wire_name() {
    let value = serde_json::to_value(WorkflowDecisionActivation::ClaimVerified)
        .expect("serialize activation");
    assert_eq!(value, serde_json::json!("claim_verified"));
    assert_eq!(
        serde_json::from_value::<WorkflowDecisionActivation>(value).expect("round trip"),
        WorkflowDecisionActivation::ClaimVerified
    );
}
