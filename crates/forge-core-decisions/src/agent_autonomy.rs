//! Pure semantic evaluator for the solo-developer agent autonomy boundary.

use forge_core_contracts::{
    AgentAutonomyAssessment, AgentAutonomyAssessmentInput, AgentAutonomyBinding,
    AgentAutonomyDecisionAlternative, AgentAutonomyDecisionRequest, AgentAutonomyEffectDescriptor,
    AgentAutonomyWork, AgentOwnedWorkClass, HumanDecisionClass, StableId,
    AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION, MAX_AGENT_AUTONOMY_SUMMARY_BYTES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAutonomyEvaluationError {
    UnsupportedSchemaVersion,
    StaleBinding,
    InvalidSummary,
    DecisionIdentityEncoding,
}

impl fmt::Display for AgentAutonomyEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion => {
                f.write_str("unsupported agent autonomy assessment schema version")
            }
            Self::StaleBinding => f.write_str("agent autonomy assessment binding is stale"),
            Self::InvalidSummary => f.write_str(
                "agent autonomy work summary must be non-empty, bounded UTF-8 text without control characters",
            ),
            Self::DecisionIdentityEncoding => {
                f.write_str("agent autonomy decision identity could not be encoded canonically")
            }
        }
    }
}

impl std::error::Error for AgentAutonomyEvaluationError {}

/// Evaluate one closed work description against current kernel-derived binding.
/// This function is pure: it has no clock, filesystem, ledger, network, or host dependency.
pub fn evaluate_agent_autonomy(
    current_binding: &AgentAutonomyBinding,
    input: &AgentAutonomyAssessmentInput,
) -> Result<AgentAutonomyAssessment, AgentAutonomyEvaluationError> {
    if input.schema_version != AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION {
        return Err(AgentAutonomyEvaluationError::UnsupportedSchemaVersion);
    }
    if &input.binding != current_binding {
        return Err(AgentAutonomyEvaluationError::StaleBinding);
    }
    let summary = input.work.summary();
    if summary.trim().is_empty()
        || summary.len() > MAX_AGENT_AUTONOMY_SUMMARY_BYTES
        || summary.chars().any(char::is_control)
    {
        return Err(AgentAutonomyEvaluationError::InvalidSummary);
    }

    match &input.work {
        AgentAutonomyWork::AgentOwned { class, .. }
            if agent_owned_effect_is_compatible(*class, input.effect) =>
        {
            Ok(AgentAutonomyAssessment::ProceedAutonomously {
                schema_version: AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION.to_owned(),
                binding: current_binding.clone(),
                class: *class,
            })
        }
        AgentAutonomyWork::AgentOwned { .. } => decision_assessment(
            current_binding,
            HumanDecisionClass::IrreversibleOrExternalEffect,
            input.effect,
            summary,
            &input.work,
            incompatible_or_external_rationale(input.effect),
        ),
        AgentAutonomyWork::HumanDecision { class, .. } => decision_assessment(
            current_binding,
            *class,
            input.effect,
            summary,
            &input.work,
            human_rationale(*class),
        ),
    }
}

fn agent_owned_effect_is_compatible(
    class: AgentOwnedWorkClass,
    effect: AgentAutonomyEffectDescriptor,
) -> bool {
    match effect {
        AgentAutonomyEffectDescriptor::LocalReadOnly => matches!(
            class,
            AgentOwnedWorkClass::ResearchAndAnalysis
                | AgentOwnedWorkClass::PlanningAndStrategy
                | AgentOwnedWorkClass::TestingAndVerification
                | AgentOwnedWorkClass::EvidenceGeneration
        ),
        AgentAutonomyEffectDescriptor::LocalReversible => {
            class != AgentOwnedWorkClass::ExternalReadOnlyResearch
        }
        AgentAutonomyEffectDescriptor::ExternalReadOnly => matches!(
            class,
            AgentOwnedWorkClass::ExternalReadOnlyResearch
                | AgentOwnedWorkClass::ResearchAndAnalysis
        ),
        AgentAutonomyEffectDescriptor::ExternalMutation
        | AgentAutonomyEffectDescriptor::ProtectedEffect { .. }
        | AgentAutonomyEffectDescriptor::UnknownOrAmbiguous => false,
    }
}

fn decision_assessment(
    binding: &AgentAutonomyBinding,
    class: HumanDecisionClass,
    effect: AgentAutonomyEffectDescriptor,
    summary: &str,
    work: &AgentAutonomyWork,
    rationale: &str,
) -> Result<AgentAutonomyAssessment, AgentAutonomyEvaluationError> {
    let request_id = decision_request_id(binding, class, effect, summary, work)?;
    let (question, recommended, alternative_one, alternative_two) = decision_templates(class);
    let recommendation = alternative(
        &request_id,
        "recommendation",
        recommended,
        summary,
        "preserves the active objective and the reversible local boundary until the owner chooses otherwise",
    );
    let alternatives = vec![
        alternative(
            &request_id,
            "alternative.1",
            alternative_one,
            summary,
            "records an explicit owner choice and its material consequences before work crosses the boundary",
        ),
        alternative(
            &request_id,
            "alternative.2",
            alternative_two,
            summary,
            "changes the proposed approach so the agent can continue without silently assuming owner authority",
        ),
    ];
    Ok(AgentAutonomyAssessment::DecisionRequired {
        schema_version: AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION.to_owned(),
        binding: binding.clone(),
        request: AgentAutonomyDecisionRequest {
            id: request_id,
            class,
            effect,
            question: format!("{question} Proposed work: \"{summary}\"."),
            rationale: format!("{rationale} Proposed work: \"{summary}\"."),
            recommendation,
            alternatives,
        },
    })
}

fn alternative(
    request_id: &StableId,
    suffix: &str,
    description: &str,
    summary: &str,
    consequence: &str,
) -> AgentAutonomyDecisionAlternative {
    AgentAutonomyDecisionAlternative {
        id: StableId(format!("{}.{}", request_id.0, suffix)),
        description: description.to_owned(),
        consequences: vec![format!("For \"{summary}\", this choice {consequence}.")],
    }
}

#[derive(Serialize)]
struct DecisionIdentity<'a> {
    binding: &'a AgentAutonomyBinding,
    decision_class: HumanDecisionClass,
    effect: AgentAutonomyEffectDescriptor,
    summary: &'a str,
    declared_work: &'a AgentAutonomyWork,
}

fn decision_request_id(
    binding: &AgentAutonomyBinding,
    class: HumanDecisionClass,
    effect: AgentAutonomyEffectDescriptor,
    summary: &str,
    work: &AgentAutonomyWork,
) -> Result<StableId, AgentAutonomyEvaluationError> {
    let canonical = serde_json_canonicalizer::to_vec(&DecisionIdentity {
        binding,
        decision_class: class,
        effect,
        summary,
        declared_work: work,
    })
    .map_err(|_| AgentAutonomyEvaluationError::DecisionIdentityEncoding)?;
    Ok(StableId(format!(
        "decision.agent-autonomy.{:x}",
        Sha256::digest(canonical)
    )))
}

fn human_rationale(class: HumanDecisionClass) -> &'static str {
    match class {
        HumanDecisionClass::ProductObjectiveChange => {
            "The proposed work changes the accepted product direction rather than implementation tactics"
        }
        HumanDecisionClass::MaterialTradeoff => {
            "Available evidence cannot choose a material product, cost, quality, or schedule trade-off for the owner"
        }
        HumanDecisionClass::MaterialRiskAcceptance => {
            "Proceeding requires the owner to accept a material residual risk that agent verification cannot eliminate"
        }
        HumanDecisionClass::IrreversibleOrExternalEffect => {
            "The action crosses the reversible local boundary or causes an external effect and therefore requires explicit owner choice"
        }
    }
}

fn incompatible_or_external_rationale(effect: AgentAutonomyEffectDescriptor) -> &'static str {
    match effect {
        AgentAutonomyEffectDescriptor::ExternalMutation => {
            "The host-derived operation is an external mutation, which cannot be masked by an agent-owned class"
        }
        AgentAutonomyEffectDescriptor::ProtectedEffect { .. } => {
            "The host-derived operation is protected, which cannot be masked by an agent-owned class"
        }
        AgentAutonomyEffectDescriptor::UnknownOrAmbiguous => {
            "The host/tool boundary could not classify the effect unambiguously, so Forge fails closed"
        }
        AgentAutonomyEffectDescriptor::LocalReadOnly
        | AgentAutonomyEffectDescriptor::LocalReversible
        | AgentAutonomyEffectDescriptor::ExternalReadOnly => {
            "The declared work class contradicts the independently host-derived effect scope, so Forge fails closed"
        }
    }
}

fn decision_templates(
    class: HumanDecisionClass,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match class {
        HumanDecisionClass::ProductObjectiveChange => (
            "Should the active product objective be superseded?",
            "Keep the active objective and continue within it.",
            "Supersede the objective through the governed objective-revision flow.",
            "Narrow or restate the work so it remains within the active objective.",
        ),
        HumanDecisionClass::MaterialTradeoff => (
            "Which material trade-off should govern the work?",
            "Keep the current balance until the owner chooses otherwise.",
            "Choose the proposed material trade-off explicitly.",
            "Redesign the approach to avoid the material trade-off.",
        ),
        HumanDecisionClass::MaterialRiskAcceptance => (
            "Should the identified material residual risk be accepted?",
            "Do not accept the risk; mitigate or stop first.",
            "Accept the identified risk explicitly and proceed.",
            "Choose a lower-risk implementation or verification path.",
        ),
        HumanDecisionClass::IrreversibleOrExternalEffect => (
            "Should the external, protected, ambiguous, or incompatible effect be performed?",
            "Keep the operation local and reversible or do not perform it.",
            "Explicitly authorize the described effect.",
            "Choose a compatible agent-owned operation with a host-derived safe effect scope.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::ProtectedEffect;
    use std::collections::BTreeSet;

    fn binding() -> AgentAutonomyBinding {
        AgentAutonomyBinding {
            objective_id: StableId("objective.test".to_owned()),
            objective_revision: 1,
            objective_digest: format!("sha256:{}", "a".repeat(64)),
            assurance_epoch: 7,
            snapshot_digest: format!("sha256:{}", "b".repeat(64)),
            ledger_head_digest: format!("sha256:{}", "c".repeat(64)),
            state_version: 1,
        }
    }

    fn input(
        work: AgentAutonomyWork,
        effect: AgentAutonomyEffectDescriptor,
    ) -> AgentAutonomyAssessmentInput {
        AgentAutonomyAssessmentInput {
            schema_version: AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION.to_owned(),
            binding: binding(),
            work,
            effect,
        }
    }

    fn request(result: AgentAutonomyAssessment) -> AgentAutonomyDecisionRequest {
        match result {
            AgentAutonomyAssessment::DecisionRequired { request, .. } => request,
            AgentAutonomyAssessment::ProceedAutonomously { .. } => panic!("decision required"),
        }
    }

    #[test]
    fn delegated_classes_proceed_only_with_compatible_independent_effects() {
        for class in AgentOwnedWorkClass::ALL {
            let effect = if class == AgentOwnedWorkClass::ExternalReadOnlyResearch {
                AgentAutonomyEffectDescriptor::ExternalReadOnly
            } else {
                AgentAutonomyEffectDescriptor::LocalReversible
            };
            assert!(matches!(
                evaluate_agent_autonomy(
                    &binding(),
                    &input(
                        AgentAutonomyWork::AgentOwned {
                            class,
                            summary: "bounded work inside the objective".to_owned(),
                        },
                        effect,
                    ),
                ),
                Ok(AgentAutonomyAssessment::ProceedAutonomously { class: found, .. }) if found == class
            ));
        }
    }

    #[test]
    fn all_four_human_classes_produce_distinct_concrete_requests() {
        let mut ids = BTreeSet::new();
        for class in HumanDecisionClass::ALL {
            let summary = format!("owner decision for {class:?}");
            let request = request(
                evaluate_agent_autonomy(
                    &binding(),
                    &input(
                        AgentAutonomyWork::HumanDecision {
                            class,
                            summary: summary.clone(),
                        },
                        AgentAutonomyEffectDescriptor::LocalReadOnly,
                    ),
                )
                .expect("human decision"),
            );
            assert!(ids.insert(request.id.clone()));
            assert!(request.question.contains(&summary));
            assert!(request.rationale.contains(&summary));
            assert!(request.recommendation.consequences[0].contains(&summary));
            assert!(request.alternatives.len() >= 2);
            let choice_ids = std::iter::once(&request.recommendation)
                .chain(request.alternatives.iter())
                .map(|choice| choice.id.clone())
                .collect::<BTreeSet<_>>();
            assert_eq!(choice_ids.len(), request.alternatives.len() + 1);
            assert!(request.alternatives.iter().all(|choice| {
                !choice.consequences.is_empty()
                    && choice.consequences.iter().all(|item| !item.is_empty())
            }));
        }
    }

    #[test]
    fn external_mutation_protected_unknown_and_contradiction_fail_closed() {
        for effect in [
            AgentAutonomyEffectDescriptor::ExternalMutation,
            AgentAutonomyEffectDescriptor::ProtectedEffect {
                effect: ProtectedEffect::Publication,
            },
            AgentAutonomyEffectDescriptor::UnknownOrAmbiguous,
            AgentAutonomyEffectDescriptor::ExternalReadOnly,
        ] {
            assert!(matches!(
                evaluate_agent_autonomy(
                    &binding(),
                    &input(
                        AgentAutonomyWork::AgentOwned {
                            class: AgentOwnedWorkClass::ReversibleLocalEditing,
                            summary: "claimed local edit with unsafe or contradictory effect"
                                .to_owned(),
                        },
                        effect,
                    ),
                ),
                Ok(AgentAutonomyAssessment::DecisionRequired { .. })
            ));
        }
    }

    #[test]
    fn missing_effect_descriptor_is_rejected_and_summary_changes_request_identity() {
        let missing = serde_json::json!({
            "schema_version": AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION,
            "binding": binding(),
            "work": {
                "kind": "agent_owned",
                "class": "reversible_local_editing",
                "summary": "edit locally"
            }
        });
        assert!(serde_json::from_value::<AgentAutonomyAssessmentInput>(missing).is_err());

        let make = |summary: &str| {
            request(
                evaluate_agent_autonomy(
                    &binding(),
                    &input(
                        AgentAutonomyWork::HumanDecision {
                            class: HumanDecisionClass::MaterialTradeoff,
                            summary: summary.to_owned(),
                        },
                        AgentAutonomyEffectDescriptor::LocalReadOnly,
                    ),
                )
                .expect("request"),
            )
            .id
        };
        assert_ne!(
            make("trade quality for speed"),
            make("trade cost for scope")
        );
    }

    #[test]
    fn assurance_epoch_is_part_of_the_stale_binding_cas() {
        let input = input(
            AgentAutonomyWork::AgentOwned {
                class: AgentOwnedWorkClass::TestingAndVerification,
                summary: "run a focused test".to_owned(),
            },
            AgentAutonomyEffectDescriptor::LocalReadOnly,
        );
        let mut changed = binding();
        changed.assurance_epoch += 1;
        assert_eq!(
            evaluate_agent_autonomy(&changed, &input),
            Err(AgentAutonomyEvaluationError::StaleBinding)
        );
    }
}
