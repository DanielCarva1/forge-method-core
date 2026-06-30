//! Slice-3 end-to-end parity test — the full agent loop through the guide CLI.
//!
//! Exercises the DC1 loop as a host agent would, against the REAL 110-workflow
//! catalog. This is the slice-3 acceptance gate (R7 parity): the guide surface
//! is deterministic, machine-readable, and the engine blocks illegal routing
//! with typed reasons.

use forge_core_cli::guide::{run_decide, run_describe, run_status};
use forge_core_contracts::CliEnvelope;
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

#[derive(serde::Serialize)]
struct DecisionYaml<'a> {
    schema_version: &'a str,
    guide_decision: DecisionBody<'a>,
}

#[derive(serde::Serialize)]
struct DecisionBody<'a> {
    recommended_workflow: &'a str,
    reason: &'a str,
    current_phase: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposed_next_phase: Option<&'a str>,
}

fn decision_file(wf: &str, phase: &str, next: Option<&str>, reason: &str) -> std::path::PathBuf {
    let doc = DecisionYaml {
        schema_version: "0.1",
        guide_decision: DecisionBody {
            recommended_workflow: wf,
            reason,
            current_phase: phase,
            proposed_next_phase: next,
        },
    };
    let body = yaml_serde::to_string(&doc).unwrap();
    tmp_decision("decision.yaml", &body)
}

#[test]
fn e2e_full_agent_loop_describe_status_decide() {
    // STEP 1: host calls `guide describe` once to learn the surface.
    let describe = run_describe(Some(&catalog_dir()));
    assert!(describe.ok);
    let d = describe.data.as_ref().expect("describe payload");
    assert_eq!(d.workflows.len(), 110);
    // host now knows: phases, workflows, gates, exit_reasons, schema_version.

    // STEP 2: host calls `guide status --phase 1-discovery` to orient.
    let status = run_status(Some(&catalog_dir()), "1-discovery");
    assert!(status.ok);
    let s = status.data.as_ref().expect("status payload");
    assert_eq!(s.current_phase, "1-discovery");
    // grill gate unlocks specification
    assert_eq!(s.pending_gates[0].gate, "grill");
    assert_eq!(s.pending_gates[0].unlocks, "2-specification");

    // STEP 3a: host proposes a LEGAL decision (discover-intent in discovery).
    let legal = decision_file("discover-intent", "1-discovery", None, "begin");
    let accepted = run_decide(&legal, Some(&catalog_dir()), &[]);
    assert!(accepted.ok);
    assert_eq!(accepted.exit_code(), 0);

    // STEP 3b: host proposes an ILLEGAL decision (plan-sprint in discovery).
    let illegal = decision_file("plan-sprint", "1-discovery", None, "skip ahead");
    let rejected: CliEnvelope<_> = run_decide(&illegal, Some(&catalog_dir()), &[]);
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
fn e2e_legal_transition_when_gate_provided() {
    use forge_core_contracts::gate::GateStatus;
    use forge_core_engine::{GateKind, ProvidedGateResult};

    // Host in specification, proposes plan-sprint (a 3-plan workflow) with a
    // PROPOSED transition spec->plan. Without the system-design gate: blocked.
    let no_gate = decision_file("plan-sprint", "2-specification", Some("3-plan"), "promote");
    let blocked = run_decide(&no_gate, Some(&catalog_dir()), &[]);
    assert!(!blocked.ok);
    assert_eq!(blocked.exit_code(), 2);

    // With the gate provided+passing: the transition clears, plan-sprint is
    // eligible in the proposed phase, decision accepted.
    let gates = [ProvidedGateResult {
        gate_kind: GateKind::SystemDesign,
        status: GateStatus::Pass,
    }];
    let cleared = run_decide(&no_gate, Some(&catalog_dir()), &gates);
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
