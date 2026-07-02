//! Hard-gate phase-transition enforcement.
//!
//! The engine decides whether a requested phase transition is allowed. This is
//! deterministic and independent of the host LLM: the orchestrator may *suggest*
//! a transition, but the engine BLOCKS illegal ones (DC1 hard-gate half).
//!
//! ## Rules
//!
//! 1. **Forward skip**: a transition that jumps forward over one or more phases
//!    (`to.rank - from.rank > 1`) is blocked unless a valid [`Waiver`] is
//!    supplied. A waiver must carry an owner and a non-empty rationale.
//! 2. **Backward / same-phase**: allowed without a waiver (the funnel is cyclic;
//!    a project may regress to redo work or re-enter a phase).
//! 3. **Required gates**: certain forward transitions mandate a passing gate.
//!    `Specification -> Plan` mandates the [`GateKind::SystemDesign`] gate (DC6).
//!    A required gate must be both PRESENT and `Pass`/`NotApplicable`. A phase-skip
//!    waiver permits skipping phases, not mandatory gates along the skipped path.

use forge_core_contracts::gate::GateStatus;
use forge_core_contracts::phase::Phase;
use forge_core_contracts::StableId;

/// The mandatory gate kinds the engine recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateKind {
    /// Decision-closure gate at the end of discovery (grill).
    Grill,
    /// System-design gate: SPECIFICATION -> PLAN (DC6). Mandatory.
    SystemDesign,
    /// Story-readiness gate: PLAN -> BUILD.
    StoryReady,
    /// Readiness gate: BUILD -> READY.
    Readiness,
    /// Release gate: READY -> EVOLVE.
    Release,
}

impl GateKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            GateKind::Grill => "grill",
            GateKind::SystemDesign => "system-design",
            GateKind::StoryReady => "story-ready",
            GateKind::Readiness => "readiness",
            GateKind::Release => "release",
        }
    }
}

/// A gate result supplied for a transition, tagged with which gate it satisfies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvidedGateResult {
    pub gate_kind: GateKind,
    pub status: GateStatus,
}

/// An owner-approved waiver permitting an otherwise-illegal transition (e.g. a
/// forward phase skip). Must carry a non-empty rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiver {
    pub owner: StableId,
    pub rationale: String,
}

impl Waiver {
    /// A waiver is valid only if it has a non-empty owner AND rationale.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.owner.0.trim().is_empty() && !self.rationale.trim().is_empty()
    }
}

/// A request to transition the project's phase.
#[derive(Debug, Clone)]
pub struct TransitionRequest<'a> {
    pub from: Phase,
    pub to: Phase,
    pub gates: &'a [ProvidedGateResult],
    pub waiver: Option<&'a Waiver>,
}

/// The engine's verdict on a transition request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionDecision {
    /// The transition may proceed.
    Allowed,
    /// The transition is refused, with a typed reason.
    Blocked(TransitionBlockReason),
}

/// Typed reasons a transition is blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionBlockReason {
    /// A forward jump over one or more phases without a valid waiver.
    ForwardSkipWithoutWaiver { from: Phase, to: Phase },
    /// A waiver was supplied but is incomplete (empty owner or rationale).
    InvalidWaiver,
    /// The transition mandates a gate that was not provided.
    RequiredGateMissing { required: GateKind },
    /// The transition mandates a gate that was provided but did not pass.
    RequiredGateNotPassing {
        required: GateKind,
        status: GateStatus,
    },
}

/// Returns the gate a forward transition mandates, if any.
#[must_use]
fn required_gate_for(from: Phase, to: Phase) -> Option<GateKind> {
    match (from, to) {
        // DC6: the system-design gate is mandatory before planning.
        (Phase::Specification, Phase::Plan) => Some(GateKind::SystemDesign),
        // Discovery -> Specification closes decisions (grill).
        (Phase::Discovery, Phase::Specification) => Some(GateKind::Grill),
        (Phase::Plan, Phase::BuildVerify) => Some(GateKind::StoryReady),
        (Phase::BuildVerify, Phase::ReadyOperate) => Some(GateKind::Readiness),
        (Phase::ReadyOperate, Phase::Evolve) => Some(GateKind::Release),
        _ => None,
    }
}

/// Returns every mandatory gate on a forward transition path.
fn required_gates_for_forward_path(from: Phase, to: Phase) -> impl Iterator<Item = GateKind> {
    let from_rank = from.rank();
    let to_rank = to.rank();
    Phase::ALL
        .into_iter()
        .zip(Phase::ALL.into_iter().skip(1))
        .filter_map(move |(edge_from, edge_to)| {
            (edge_from.rank() >= from_rank && edge_to.rank() <= to_rank)
                .then(|| required_gate_for(edge_from, edge_to))
                .flatten()
        })
}

/// Whether a gate status counts as "passing" for transition purposes.
/// `Pass` and `NotApplicable` clear a gate; `Fail`, `Concerns`, `Missing` do not.
#[must_use]
fn gate_clears(status: GateStatus) -> bool {
    matches!(status, GateStatus::Pass | GateStatus::NotApplicable)
}

/// Evaluate a transition request against the hard gates.
///
/// Deterministic: the same request always yields the same decision. The host
/// LLM never calls this to "ask permission" at runtime in a way that bypasses
/// it — the engine's verdict is authoritative.
#[must_use]
pub fn evaluate_transition(req: &TransitionRequest<'_>) -> TransitionDecision {
    let rank_delta = req.to.rank().saturating_sub(req.from.rank());

    // 1. Forward skip: jumping forward over a phase needs a valid waiver.
    //    The waiver covers skipping phase states only; mandatory gates on the
    //    skipped transition path still have to be present and passing below.
    if rank_delta > 1 {
        match req.waiver {
            None => {
                return TransitionDecision::Blocked(
                    TransitionBlockReason::ForwardSkipWithoutWaiver {
                        from: req.from,
                        to: req.to,
                    },
                );
            }
            Some(w) if !w.is_valid() => {
                return TransitionDecision::Blocked(TransitionBlockReason::InvalidWaiver);
            }
            Some(_) => {}
        }
    }

    // 2. Required gates for every forward edge in the transition path.
    if rank_delta >= 1 {
        for required in required_gates_for_forward_path(req.from, req.to) {
            let provided = req.gates.iter().find(|g| g.gate_kind == required);
            match provided {
                None => {
                    return TransitionDecision::Blocked(
                        TransitionBlockReason::RequiredGateMissing { required },
                    );
                }
                Some(g) if !gate_clears(g.status) => {
                    return TransitionDecision::Blocked(
                        TransitionBlockReason::RequiredGateNotPassing {
                            required,
                            status: g.status,
                        },
                    );
                }
                Some(_) => {}
            }
        }
    }

    // 3. Backward (delta 0 via saturating, or to.rank < from.rank) and
    //    forward-with-cleared-gates: allowed.
    TransitionDecision::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::gate::GateStatus;

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
            rationale: "Prototype-only track; specification deferred by product owner.".into(),
        }
    }

    // --- acceptance: legal transition with passing gate is allowed ---

    #[test]
    fn adjacent_forward_with_passing_gate_is_allowed() {
        let gate = passing(GateKind::SystemDesign);
        let req = TransitionRequest {
            from: Phase::Specification,
            to: Phase::Plan,
            gates: &[gate],
            waiver: None,
        };
        assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
    }

    // --- acceptance: transition missing its required gate is rejected ---

    #[test]
    fn spec_to_plan_without_system_design_gate_is_blocked() {
        let req = TransitionRequest {
            from: Phase::Specification,
            to: Phase::Plan,
            gates: &[],
            waiver: None,
        };
        assert_eq!(
            evaluate_transition(&req),
            TransitionDecision::Blocked(TransitionBlockReason::RequiredGateMissing {
                required: GateKind::SystemDesign
            })
        );
    }

    #[test]
    fn spec_to_plan_with_failing_gate_is_blocked() {
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
                status: GateStatus::Fail
            })
        );
    }

    // --- acceptance: phase-skip rejected unless waived ---

    #[test]
    fn forward_skip_without_waiver_is_blocked() {
        // discovery -> build-verify jumps over specification + plan.
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
    fn forward_skip_with_valid_waiver_and_passing_path_gates_is_allowed() {
        let waiver = valid_waiver();
        let gates = [
            passing(GateKind::Grill),
            passing(GateKind::SystemDesign),
            passing(GateKind::StoryReady),
        ];
        let req = TransitionRequest {
            from: Phase::Discovery,
            to: Phase::BuildVerify,
            gates: &gates,
            waiver: Some(&waiver),
        };
        assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
    }

    #[test]
    fn forward_skip_with_valid_waiver_still_requires_path_gates() {
        let waiver = valid_waiver();
        let req = TransitionRequest {
            from: Phase::Discovery,
            to: Phase::BuildVerify,
            gates: &[],
            waiver: Some(&waiver),
        };
        assert_eq!(
            evaluate_transition(&req),
            TransitionDecision::Blocked(TransitionBlockReason::RequiredGateMissing {
                required: GateKind::Grill
            })
        );
    }

    #[test]
    fn forward_skip_with_empty_rationale_waiver_is_blocked() {
        let waiver = Waiver {
            owner: StableId("lead".into()),
            rationale: "   ".into(),
        };
        let req = TransitionRequest {
            from: Phase::Discovery,
            to: Phase::BuildVerify,
            gates: &[],
            waiver: Some(&waiver),
        };
        assert_eq!(
            evaluate_transition(&req),
            TransitionDecision::Blocked(TransitionBlockReason::InvalidWaiver)
        );
    }

    // --- funnel is cyclic: backward transitions are allowed (rework/new cycle) ---

    #[test]
    fn backward_transition_is_allowed_without_waiver() {
        let req = TransitionRequest {
            from: Phase::Evolve,
            to: Phase::Discovery,
            gates: &[],
            waiver: None,
        };
        assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
    }

    #[test]
    fn same_phase_reentry_is_allowed() {
        let req = TransitionRequest {
            from: Phase::BuildVerify,
            to: Phase::BuildVerify,
            gates: &[],
            waiver: None,
        };
        assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
    }

    // --- the other adjacent gates ---

    #[test]
    fn discovery_to_specification_requires_grill() {
        let req = TransitionRequest {
            from: Phase::Discovery,
            to: Phase::Specification,
            gates: &[],
            waiver: None,
        };
        assert_eq!(
            evaluate_transition(&req),
            TransitionDecision::Blocked(TransitionBlockReason::RequiredGateMissing {
                required: GateKind::Grill
            })
        );
    }

    #[test]
    fn route_to_discovery_has_no_gate() {
        // Entry into the funnel: Route -> Discovery needs no gate.
        let req = TransitionRequest {
            from: Phase::Route,
            to: Phase::Discovery,
            gates: &[],
            waiver: None,
        };
        assert_eq!(evaluate_transition(&req), TransitionDecision::Allowed);
    }

    #[test]
    fn concerns_status_does_not_clear_gate() {
        // Concerns is not Pass/NotApplicable -> gate does not clear.
        let gate = ProvidedGateResult {
            gate_kind: GateKind::SystemDesign,
            status: GateStatus::Concerns,
        };
        let req = TransitionRequest {
            from: Phase::Specification,
            to: Phase::Plan,
            gates: &[gate],
            waiver: None,
        };
        assert_eq!(
            evaluate_transition(&req),
            TransitionDecision::Blocked(TransitionBlockReason::RequiredGateNotPassing {
                required: GateKind::SystemDesign,
                status: GateStatus::Concerns
            })
        );
    }
}
