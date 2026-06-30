use crate::common::StableId;
use crate::phase::Phase;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Serde document wrapper for a guide decision on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideDecisionDocument {
    pub schema_version: String,
    pub guide_decision: GuideDecision,
}

/// The auditable output of the orchestrator-guide: the host LLM reasons over
/// the typed catalog + project state and emits this decision; the engine then
/// VALIDATES it (S2.3) against hard gates. Because it is typed (not free text),
/// guidance is replayable and the engine can reject illegal routing (DC1).
///
/// Field shape follows engine-architecture.yaml#deliverable_contract:
/// `{recommended_workflow, reason, allowed, blocked_by_gates, phase}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideDecision {
    /// The workflow id the guide recommends running next. Must resolve in the
    /// catalog and be eligible in `current_phase` (validated in S2.3).
    pub recommended_workflow: StableId,
    /// Short human/agent-readable rationale for the recommendation.
    pub reason: String,
    /// Actions permitted within the recommended workflow's gates.
    #[serde(default)]
    pub allowed_actions: Vec<StableId>,
    /// Gates currently blocking progress (free-form reasons; the engine adds
    /// the typed gate-state on validation).
    #[serde(default)]
    pub blocked_by_gates: Vec<String>,
    /// The project's current phase (free-form string; parsed by the engine).
    pub current_phase: StableId,
    /// The phase the guide proposes to move to, if any. If present and
    /// different from current, the engine runs the S1.7 transition gates.
    #[serde(default)]
    pub proposed_next_phase: Option<StableId>,
}

impl GuideDecision {
    /// Categorize `current_phase` into a canonical [`Phase`]. Returns `None`
    /// for an unrecognized/empty label (the engine treats that as a rejection).
    #[must_use]
    pub fn current_phase_category(&self) -> Option<Phase> {
        Phase::parse(&self.current_phase.0)
    }

    /// Categorize `proposed_next_phase` if present.
    #[must_use]
    pub fn proposed_phase_category(&self) -> Option<Phase> {
        self.proposed_next_phase
            .as_ref()
            .and_then(|p| Phase::parse(&p.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_full_decision() {
        let yaml = r#"schema_version: "0.1"
guide_decision:
  recommended_workflow: plan-sprint
  reason: "specification accepted; ready to sequence stories"
  allowed_actions:
    - create-stories
    - build-story
  blocked_by_gates:
    - "system-design gate not yet run"
  current_phase: 2-specification
  proposed_next_phase: 3-plan
"#;
        let doc: GuideDecisionDocument =
            yaml_serde::from_str(yaml).expect("deserialize guide decision");
        assert_eq!(
            doc.guide_decision.recommended_workflow,
            StableId("plan-sprint".into())
        );
        assert_eq!(
            doc.guide_decision.current_phase_category(),
            Some(Phase::Specification)
        );
        assert_eq!(
            doc.guide_decision.proposed_phase_category(),
            Some(Phase::Plan)
        );

        let again = yaml_serde::to_string(&doc).expect("serialize");
        let doc2: GuideDecisionDocument = yaml_serde::from_str(&again).expect("deserialize again");
        assert_eq!(doc, doc2, "round-trip not stable");
    }

    #[test]
    fn decision_without_next_phase_round_trips() {
        let yaml = r#"schema_version: "0.1"
guide_decision:
  recommended_workflow: discover-intent
  reason: "new project"
  current_phase: 1-discovery
"#;
        let doc: GuideDecisionDocument = yaml_serde::from_str(yaml).expect("deserialize");
        assert!(doc.guide_decision.proposed_next_phase.is_none());
        assert!(doc.guide_decision.allowed_actions.is_empty());
    }

    #[test]
    fn unrecognized_phase_label_categorizes_to_none() {
        let doc = GuideDecisionDocument {
            schema_version: "0.1".into(),
            guide_decision: GuideDecision {
                recommended_workflow: StableId("x".into()),
                reason: "r".into(),
                allowed_actions: vec![],
                blocked_by_gates: vec![],
                current_phase: StableId("nonsense".into()),
                proposed_next_phase: None,
            },
        };
        assert_eq!(doc.guide_decision.current_phase_category(), None);
    }
}
