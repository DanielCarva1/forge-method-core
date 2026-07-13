//! Guide-decision validation — the DC1 hard-gate loop closure.
//!
//! The host LLM produces a [`GuideDecision`] (a recommendation); the engine
//! VALIDATES it deterministically and rejects illegal guidance with a typed
//! [`GuideRejection`]. This is the "engine blocks illegal routing" half of DC1.
//!
//! ## Inputs (clean separation, DD8)
//!
//! - `decision`: the host's *recommendation* (recommended workflow, reason,
//!   proposed transition). Carries no factual gate evidence.
//! - `catalog`: the typed workflow catalog (loaded via [`crate::load_catalog`]).
//! - `gates`: the *factual* project gate state (typed [`ProvidedGateResult`]
//!   slice). Recommendation and evidence are separate inputs; the engine is the
//!   arbiter.
//!
//! ## Checks (in order)
//!
//! 1. `current_phase` must categorize to a known [`Phase`].
//! 2. `recommended_workflow` must exist in the catalog.
//! 3. `recommended_workflow` must be eligible in `current_phase`.
//! 4. If `proposed_next_phase` is present, it must categorize to a known [`Phase`].
//! 5. If `proposed_next_phase` differs from `current_phase`, the S1.7
//!    [`evaluate_transition`] hard gates must clear.

use crate::catalog::find_entry;
use crate::phase_transition::{evaluate_transition, ProvidedGateResult, TransitionRequest};
use forge_core_contracts::phase::Phase;
use forge_core_contracts::{Catalog, GuideDecision, StableId};

/// The engine's verdict on a host-proposed guide decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuideValidation {
    /// The guidance is legal: the workflow exists, is eligible, and any
    /// proposed transition clears the hard gates.
    Accepted,
    /// The guidance is refused, with a typed reason.
    Rejected(GuideRejection),
}

/// Typed reasons a guide decision is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuideRejection {
    /// `current_phase` did not categorize to a known phase.
    UnrecognizedCurrentPhase { raw: StableId },
    /// `recommended_workflow` is not in the catalog.
    UnknownWorkflow { id: StableId },
    /// The recommended workflow exists but is not eligible in the current phase.
    NotEligibleInPhase { workflow: StableId, phase: Phase },
    /// `proposed_next_phase` was present but did not categorize to a known phase.
    UnrecognizedProposedPhase { raw: StableId },
    /// A proposed phase transition is blocked by the S1.7 hard gates.
    IllegalTransition(crate::TransitionBlockReason),
}

/// Validate a host-proposed [`GuideDecision`] against the catalog and gates.
///
/// Deterministic and authoritative: the same inputs always yield the same
/// verdict. The host may *propose* anything; the engine decides legality.
#[must_use]
pub fn validate_guide_decision(
    decision: &GuideDecision,
    catalog: &Catalog,
    gates: &[ProvidedGateResult],
) -> GuideValidation {
    // 1. current phase must be recognizable.
    let Some(current) = decision.current_phase_category() else {
        return GuideValidation::Rejected(GuideRejection::UnrecognizedCurrentPhase {
            raw: decision.current_phase.clone(),
        });
    };

    // 2. recommended workflow must exist.
    let Some(entry) = find_entry(catalog, &decision.recommended_workflow) else {
        return GuideValidation::Rejected(GuideRejection::UnknownWorkflow {
            id: decision.recommended_workflow.clone(),
        });
    };

    // 3. If a phase transition is proposed, check the S1.7 hard gates FIRST.
    //    An illegal transition rejects before eligibility (you cannot use a
    //    proposed transition to justify a workflow if the transition itself
    //    is blocked). A CLEARED transition widens eligibility to include the
    //    proposed phase (recommending a next-phase workflow while proposing
    //    the move into it is coherent guidance).
    let mut proposed_seen = None;
    if let Some(raw_proposed) = &decision.proposed_next_phase {
        let Some(proposed) = Phase::parse(&raw_proposed.0) else {
            return GuideValidation::Rejected(GuideRejection::UnrecognizedProposedPhase {
                raw: raw_proposed.clone(),
            });
        };
        if proposed != current {
            let req = TransitionRequest {
                from: current,
                to: proposed,
                gates,
                waiver: None,
            };
            if let crate::TransitionDecision::Blocked(reason) = evaluate_transition(&req) {
                return GuideValidation::Rejected(GuideRejection::IllegalTransition(reason));
            }
            proposed_seen = Some(proposed);
        }
    }

    // 4. eligibility against current phase (and proposed, if the transition cleared).
    let eligible = entry.phases.iter().any(|tag| {
        Phase::tag_eligible(&tag.0, current)
            || proposed_seen.is_some_and(|p| Phase::tag_eligible(&tag.0, p))
    });
    if !eligible {
        return GuideValidation::Rejected(GuideRejection::NotEligibleInPhase {
            workflow: decision.recommended_workflow.clone(),
            phase: current,
        });
    }

    GuideValidation::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog;
    use forge_core_contracts::{GuideDecision, StableId};

    /// Use the real 110-workflow catalog so the validation is tested against
    /// actual routing data, not a stub.
    fn real_catalog() -> Catalog {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/evidence/workflow-retirement/legacy-catalog")
            .canonicalize()
            .expect("catalog dir");
        let report = load_catalog(&dir);
        assert!(
            report.is_clean(),
            "catalog must load cleanly: {:?}",
            report.errors
        );
        report.catalog
    }

    fn decision(workflow: &str, phase: &str, next: Option<&str>) -> GuideDecision {
        GuideDecision {
            recommended_workflow: StableId(workflow.into()),
            reason: "test".into(),
            allowed_actions: vec![],
            blocked_by_gates: vec![],
            current_phase: StableId(phase.into()),
            proposed_next_phase: next.map(|p| StableId(p.into())),
        }
    }

    #[test]
    fn accepts_valid_in_phase_decision_with_no_transition() {
        // discover-intent is a 1-discovery workflow; current phase discovery.
        let cat = real_catalog();
        let d = decision("discover-intent", "1-discovery", None);
        assert_eq!(
            validate_guide_decision(&d, &cat, &[]),
            GuideValidation::Accepted
        );
    }

    #[test]
    fn rejects_unknown_workflow() {
        let cat = real_catalog();
        let d = decision("does-not-exist", "1-discovery", None);
        assert_eq!(
            validate_guide_decision(&d, &cat, &[]),
            GuideValidation::Rejected(GuideRejection::UnknownWorkflow {
                id: StableId("does-not-exist".into())
            })
        );
    }

    #[test]
    fn rejects_workflow_ineligible_in_current_phase() {
        // plan-sprint is a 3-plan workflow; offering it in discovery is illegal.
        let cat = real_catalog();
        let d = decision("plan-sprint", "1-discovery", None);
        match validate_guide_decision(&d, &cat, &[]) {
            GuideValidation::Rejected(GuideRejection::NotEligibleInPhase { workflow, phase }) => {
                assert_eq!(workflow.0, "plan-sprint");
                assert_eq!(phase, Phase::Discovery);
            }
            other => panic!("expected NotEligibleInPhase, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unrecognized_current_phase() {
        let cat = real_catalog();
        let d = decision("discover-intent", "nonsense", None);
        assert_eq!(
            validate_guide_decision(&d, &cat, &[]),
            GuideValidation::Rejected(GuideRejection::UnrecognizedCurrentPhase {
                raw: StableId("nonsense".into())
            })
        );
    }

    #[test]
    fn rejects_unrecognized_proposed_phase() {
        let cat = real_catalog();
        let d = decision("discover-intent", "1-discovery", Some("not-a-phase"));
        assert_eq!(
            validate_guide_decision(&d, &cat, &[]),
            GuideValidation::Rejected(GuideRejection::UnrecognizedProposedPhase {
                raw: StableId("not-a-phase".into())
            })
        );
    }

    #[test]
    fn rejects_illegal_transition_missing_gate() {
        // A spec->plan transition requires the system-design gate. Provide none.
        // plan-sprint is eligible in 3-plan; but the TRANSITION spec->plan is gated.
        let cat = real_catalog();
        let d = decision("plan-sprint", "2-specification", Some("3-plan"));
        match validate_guide_decision(&d, &cat, &[]) {
            GuideValidation::Rejected(GuideRejection::IllegalTransition(
                crate::TransitionBlockReason::RequiredGateMissing { required },
            )) => {
                assert_eq!(required, crate::GateKind::SystemDesign);
            }
            other => panic!("expected IllegalTransition/RequiredGateMissing, got {other:?}"),
        }
    }

    #[test]
    fn accepts_transition_when_gate_is_provided_and_passing() {
        use crate::phase_transition::ProvidedGateResult;
        use forge_core_contracts::gate::GateStatus;
        let cat = real_catalog();
        let d = decision("plan-sprint", "2-specification", Some("3-plan"));
        let gates = [ProvidedGateResult {
            gate_kind: crate::GateKind::SystemDesign,
            status: GateStatus::Pass,
        }];
        assert_eq!(
            validate_guide_decision(&d, &cat, &gates),
            GuideValidation::Accepted
        );
    }

    #[test]
    fn accepts_anytime_workflow_in_any_phase() {
        // adversarial-review is tagged 'anytime'; it should be accepted in any phase.
        let cat = real_catalog();
        for phase in Phase::ALL {
            // skip Route (no real workflow lives there besides start-runtime)
            let d = decision("adversarial-review", &phase.to_string(), None);
            let v = validate_guide_decision(&d, &cat, &[]);
            // anytime should never be NotEligibleInPhase
            assert!(
                !matches!(
                    v,
                    GuideValidation::Rejected(GuideRejection::NotEligibleInPhase { .. })
                ),
                "adversarial-review (anytime) rejected as ineligible in {phase}"
            );
        }
    }
}
