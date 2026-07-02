use forge_core_contracts::gate::GateStatus;
use forge_core_contracts::StableId;
use forge_core_decisions::{
    evaluate_transition, GateKind, Phase, ProvidedGateResult, TransitionBlockReason,
    TransitionDecision, TransitionRequest, Waiver,
};

fn passing(kind: GateKind) -> ProvidedGateResult {
    ProvidedGateResult {
        gate_kind: kind,
        status: GateStatus::Pass,
    }
}

fn failing(kind: GateKind) -> ProvidedGateResult {
    ProvidedGateResult {
        gate_kind: kind,
        status: GateStatus::Fail,
    }
}

fn valid_waiver() -> Waiver {
    Waiver {
        owner: StableId("tech-lead".into()),
        rationale: "Owner-approved phase skip for a constrained autonomous track.".into(),
    }
}

#[test]
fn legal_forward_transition_with_required_gate_satisfied_is_allowed() {
    let gate = passing(GateKind::SystemDesign);
    let req = TransitionRequest {
        from: Phase::Specification,
        to: Phase::Plan,
        gates: &[gate],
        waiver: None,
    };

    assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
}

#[test]
fn illegal_forward_skip_without_waiver_is_blocked() {
    let req = TransitionRequest {
        from: Phase::Discovery,
        to: Phase::BuildVerify,
        gates: &[],
        waiver: None,
    };

    assert_eq!(
        evaluate_transition(&req),
        TransitionDecision::Blocked(TransitionBlockReason::ForwardSkipWithoutWaiver {
            from: Phase::Discovery,
            to: Phase::BuildVerify,
        })
    );
}

#[test]
fn legal_forward_transition_missing_required_gate_is_blocked() {
    let req = TransitionRequest {
        from: Phase::Specification,
        to: Phase::Plan,
        gates: &[],
        waiver: None,
    };

    assert_eq!(
        evaluate_transition(&req),
        TransitionDecision::Blocked(TransitionBlockReason::RequiredGateMissing {
            required: GateKind::SystemDesign,
        })
    );
}

#[test]
fn legal_forward_transition_with_failing_gate_is_blocked() {
    let req = TransitionRequest {
        from: Phase::Specification,
        to: Phase::Plan,
        gates: &[failing(GateKind::SystemDesign)],
        waiver: None,
    };

    assert_eq!(
        evaluate_transition(&req),
        TransitionDecision::Blocked(TransitionBlockReason::RequiredGateNotPassing {
            required: GateKind::SystemDesign,
            status: GateStatus::Fail,
        })
    );
}

#[test]
fn valid_phase_skip_waiver_still_requires_path_gates() {
    let waiver = valid_waiver();
    let gates = [
        passing(GateKind::Grill),
        failing(GateKind::SystemDesign),
        passing(GateKind::StoryReady),
    ];
    let req = TransitionRequest {
        from: Phase::Discovery,
        to: Phase::BuildVerify,
        gates: &gates,
        waiver: Some(&waiver),
    };

    assert_eq!(
        evaluate_transition(&req),
        TransitionDecision::Blocked(TransitionBlockReason::RequiredGateNotPassing {
            required: GateKind::SystemDesign,
            status: GateStatus::Fail,
        })
    );
}
