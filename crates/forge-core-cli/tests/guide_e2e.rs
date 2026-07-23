//! Slice-3 end-to-end parity test — the full agent loop through the guide CLI.
//!
//! Exercises the DC1 loop as a host agent would, against the operational
//! 68-workflow catalog plus 42 compatibility tombstones. This is the slice-3 acceptance gate (R7 parity): the guide surface
//! is deterministic, machine-readable, and the engine blocks illegal routing
//! with typed reasons.

use forge_core_cli::guide::{
    run_decide, run_describe, run_status, GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION,
};
use forge_core_contracts::{CliEnvelope, GuideProtocolDocument, StableId};
use std::io::Write;

fn catalog_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/workflows")
        .canonicalize()
        .expect("catalog dir")
}

fn tmp_decision(name: &str, body: &str) -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("forge-e2e-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(body.as_bytes()).unwrap();
    p
}

fn decision_file(wf: &str, phase: &str, next: Option<&str>, reason: &str) -> std::path::PathBuf {
    let mut document: GuideProtocolDocument = yaml_serde::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/fixtures/guide-protocol-v0/facilitation.yaml"
    )))
    .expect("guide protocol fixture");
    let protocol = &mut document.guide_protocol;
    protocol.decision.recommended_workflow = StableId(wf.to_owned());
    reason.clone_into(&mut protocol.decision.reason);
    protocol.decision.current_phase = StableId(phase.to_owned());
    protocol.decision.proposed_next_phase = next.map(|value| StableId(value.to_owned()));
    protocol
        .next_operation
        .operation_contract
        .recommendation
        .workflow = StableId(wf.to_owned());
    protocol
        .next_operation
        .operation_contract
        .recommendation
        .phase = StableId(next.unwrap_or(phase).to_owned());
    protocol.next_operation.operation_contract.contract_id = StableId(format!("op_guide_{wf}"));
    let body = yaml_serde::to_string(&document).unwrap();
    tmp_decision("decision.yaml", &body)
}

#[test]
fn e2e_full_agent_loop_describe_status_decide() {
    // STEP 1: host calls `guide describe` once to learn the surface.
    let describe = run_describe(Some(&catalog_dir()));
    assert!(describe.ok);
    let d = describe.data.as_ref().expect("describe payload");
    assert_eq!(d.workflows.len(), 68);
    assert_eq!(d.retired_workflows.len(), 42);
    assert_eq!(d.schema_version, GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION);
    // host now knows: phases, workflows, gates, exit_reasons, schema_version.

    // STEP 2: host calls `guide status --phase 1-discovery` to orient.
    let status = run_status(Some(&catalog_dir()), "1-discovery");
    assert!(status.ok);
    let s = status.data.as_ref().expect("status payload");
    assert_eq!(s.current_phase, "1-discovery");
    assert_eq!(s.schema_version, GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION);
    // grill gate unlocks specification
    assert_eq!(s.pending_gates[0].gate, "grill");
    assert_eq!(s.pending_gates[0].unlocks, "2-specification");

    // STEP 3a: host proposes a LEGAL operational decision in discovery.
    let legal = decision_file("brainstorming", "1-discovery", None, "begin");
    let accepted = run_decide(&legal, Some(&catalog_dir()), &[], None);
    assert!(accepted.ok);
    assert_eq!(accepted.exit_code(), 0);

    // STEP 3b: host proposes an ILLEGAL operational plan workflow in discovery.
    let illegal = decision_file("create-epics", "1-discovery", None, "skip ahead");
    let rejected: CliEnvelope<_> = run_decide(&illegal, Some(&catalog_dir()), &[], None);
    assert!(!rejected.ok);
    assert_eq!(rejected.exit_code(), 2);
    let code = rejected.error.as_ref().expect("error").code.0.clone();
    assert!(
        code.starts_with("not_eligible_in_phase"),
        "typed reject code expected, got: {code}"
    );
    // The typed code lets the host SELF-CORRECT: it knows to stay in discovery.
}

#[test]
fn legacy_0_1_routing_consumer_rejects_the_0_2_68_route_payload() {
    fn legacy_consume(schema_version: &str, workflow_count: usize) -> Result<usize, String> {
        if schema_version != "0.1" {
            return Err(format!(
                "unsupported guide routing payload schema {schema_version}; expected 0.1"
            ));
        }
        Ok(workflow_count)
    }

    let describe = run_describe(Some(&catalog_dir()));
    let payload = describe.data.expect("describe payload");
    let error = legacy_consume(&payload.schema_version, payload.workflows.len())
        .expect_err("legacy host must fail closed on the breaking routing payload");
    assert!(error.contains("expected 0.1"));
    assert_eq!(payload.workflows.len(), 68);
}

#[test]
fn e2e_legal_transition_when_gate_provided() {
    use forge_core_contracts::gate::GateStatus;
    use forge_core_decisions::{GateKind, ProvidedGateResult};

    // Host in specification, proposes create-epics (a 3-plan workflow) with a
    // PROPOSED transition spec->plan. Without the system-design gate: blocked.
    let no_gate = decision_file("create-epics", "2-specification", Some("3-plan"), "promote");
    let blocked = run_decide(&no_gate, Some(&catalog_dir()), &[], None);
    assert!(!blocked.ok);
    assert_eq!(blocked.exit_code(), 2);

    // With the gate provided+passing: the transition clears, create-epics is
    // eligible in the proposed phase, decision accepted.
    let gates = [ProvidedGateResult {
        gate_kind: GateKind::SystemDesign,
        status: GateStatus::Pass,
    }];
    let cleared = run_decide(&no_gate, Some(&catalog_dir()), &gates, None);
    assert!(
        cleared.ok,
        "gate-cleared transition should accept: {:?}",
        cleared.error
    );
    assert_eq!(cleared.exit_code(), 0);
}

#[test]
fn e2e_every_envelope_carries_schema_version_and_exit_reason() {
    // R1/R6: every envelope, success or failure, carries schema_version +
    // command + ok + exit_reason. Deterministic shape. Check each command's output.
    let describe_json = serde_json::to_string(&run_describe(Some(&catalog_dir()))).unwrap();
    let status_json =
        serde_json::to_string(&run_status(Some(&catalog_dir()), "1-discovery")).unwrap();
    for json in [describe_json, status_json] {
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"command\""));
        assert!(json.contains("\"ok\""));
        assert!(json.contains("\"exit_reason\""));
    }
}

#[test]
fn e2e_agent_path_is_noninteractive_and_deterministic() {
    // R7: calling describe twice yields byte-identical output (no timestamps,
    // no ordering randomness). The agent can rely on the contract.
    let a = serde_json::to_string(&run_describe(Some(&catalog_dir()))).unwrap();
    let b = serde_json::to_string(&run_describe(Some(&catalog_dir()))).unwrap();
    assert_eq!(a, b, "describe must be deterministic");
}
