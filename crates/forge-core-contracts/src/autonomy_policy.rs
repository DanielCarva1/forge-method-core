use crate::common::{EvidenceBasis, StableId};
use schemars::JsonSchema;
use serde::de::{Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};

const MAX_RISK_SCORE: u64 = 100;

fn deserialize_optional_risk_score<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer)?
        .map(|value| bounded_percent(value, "risk_score"))
        .transpose()
}

fn deserialize_optional_approval_threshold<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer)?
        .map(|value| bounded_percent(value, "requires_approval_above"))
        .transpose()
}

fn bounded_percent<E>(value: u64, field: &str) -> Result<u8, E>
where
    E: DeError,
{
    match u8::try_from(value) {
        Ok(percent) if value <= MAX_RISK_SCORE => Ok(percent),
        _ => Err(E::custom(format!(
            "{field} must be in the inclusive range 0..=100; got {value}"
        ))),
    }
}

/// A document declaring the autonomy policy applied to a run, agent, lane, phase, or repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPolicyContractDocument {
    pub schema_version: String,
    pub autonomy_policy_contract: AutonomyPolicyContract,
}

/// Declares the default autonomy mode plus per-tool overrides and escalation rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPolicyContract {
    pub id: StableId,
    pub applies_to: PolicyScope,
    pub default_mode: AutonomyMode,
    pub tool_classes: Vec<ToolClassPolicy>,
    pub escalation: EscalationPolicy,
    pub evidence_basis: Option<EvidenceBasis>,
}

impl AutonomyPolicyContract {
    /// Return the tool-class-specific autonomy mode, falling back to the policy default.
    #[must_use]
    pub fn effective_mode_for(&self, class: ToolClass) -> AutonomyMode {
        self.tool_classes
            .iter()
            .find(|policy| policy.class == class)
            .map_or(self.default_mode, |policy| policy.mode)
    }
}

/// The scope this autonomy policy binds to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyScope {
    pub kind: PolicyScopeKind,
    pub ids: Vec<StableId>,
}

/// Per-tool autonomy override and risk threshold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolClassPolicy {
    pub class: ToolClass,
    pub mode: AutonomyMode,
    #[schemars(range(min = 0, max = 100))]
    #[serde(default, deserialize_with = "deserialize_optional_risk_score")]
    pub risk_score: Option<u8>,
    #[schemars(range(min = 0, max = 100))]
    #[serde(default, deserialize_with = "deserialize_optional_approval_threshold")]
    pub requires_approval_above: Option<u8>,
}

/// Escalation conditions that force a more rigorous lane or human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EscalationPolicy {
    pub on_repeated_failure: u8,
    pub on_high_risk_path: bool,
    pub on_semantic_uncertainty: bool,
    pub max_retries_before_human: u8,
    pub cooldown_seconds: u64,
}

/// The autonomy level permitted by the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    Manual,
    Allowlist,
    SandboxAuto,
    ConfidenceThreshold,
    Yolo,
}

/// A class of tool or operation that may need a dedicated autonomy mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolClass {
    FileEdit,
    TerminalCommand,
    NetworkEgress,
    PackageInstall,
    SecretAccess,
    McpToolCall,
    CodeExec,
    GitMutation,
}

/// What kind of project entity a policy scope targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyScopeKind {
    Run,
    AgentRole,
    Lane,
    Phase,
    Repo,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{SourceId, SourcePattern};

    fn sid(value: &str) -> StableId {
        StableId(value.to_owned())
    }

    fn example_contract() -> AutonomyPolicyContractDocument {
        AutonomyPolicyContractDocument {
            schema_version: "0.1".to_owned(),
            autonomy_policy_contract: AutonomyPolicyContract {
                id: sid("autonomy.fast_lane.default"),
                applies_to: PolicyScope {
                    kind: PolicyScopeKind::Lane,
                    ids: vec![sid("fast_lane"), sid("build_verify")],
                },
                default_mode: AutonomyMode::SandboxAuto,
                tool_classes: vec![
                    ToolClassPolicy {
                        class: ToolClass::FileEdit,
                        mode: AutonomyMode::Yolo,
                        risk_score: Some(20),
                        requires_approval_above: Some(80),
                    },
                    ToolClassPolicy {
                        class: ToolClass::TerminalCommand,
                        mode: AutonomyMode::ConfidenceThreshold,
                        risk_score: Some(55),
                        requires_approval_above: Some(70),
                    },
                    ToolClassPolicy {
                        class: ToolClass::SecretAccess,
                        mode: AutonomyMode::Manual,
                        risk_score: Some(100),
                        requires_approval_above: Some(1),
                    },
                ],
                escalation: EscalationPolicy {
                    on_repeated_failure: 2,
                    on_high_risk_path: true,
                    on_semantic_uncertainty: true,
                    max_retries_before_human: 3,
                    cooldown_seconds: 300,
                },
                evidence_basis: Some(EvidenceBasis {
                    direct_patterns: vec![SourcePattern {
                        source_id: SourceId("radar-case-study".to_owned()),
                        supports: "Risk-calibrated auto-accept policies reduce incidents."
                            .to_owned(),
                    }],
                    non_western_coverage_note: Some(
                        "Includes eastern agentic-coding product research.".to_owned(),
                    ),
                    inference_boundary:
                        "Policy declares allowed autonomy; enforcement lives in the engine."
                            .to_owned(),
                }),
            },
        }
    }

    #[test]
    fn serde_round_trip_preserves_populated_contract() {
        let document = example_contract();

        let serialized = serde_yaml::to_string(&document).expect("serialize autonomy policy");
        let deserialized: AutonomyPolicyContractDocument =
            serde_yaml::from_str(&serialized).expect("deserialize autonomy policy");

        assert_eq!(document, deserialized);
    }

    #[test]
    fn example_autonomy_policy_yaml_round_trips() {
        let yaml = include_str!("../../../contracts/examples/autonomy-policy.yaml");
        let document: AutonomyPolicyContractDocument =
            serde_yaml::from_str(yaml).expect("deserialize autonomy policy example");
        let serialized =
            serde_yaml::to_string(&document).expect("serialize autonomy policy example");
        let reparsed: AutonomyPolicyContractDocument =
            serde_yaml::from_str(&serialized).expect("deserialize serialized autonomy example");

        assert_eq!(document, reparsed);
    }

    #[test]
    fn deny_unknown_fields_rejects_extra_contract_key() {
        let yaml = r#"
schema_version: "0.1"
autonomy_policy_contract:
  id: "autonomy.fast_lane.default"
  applies_to:
    kind: "lane"
    ids: ["fast_lane"]
  default_mode: "sandbox_auto"
  tool_classes: []
  escalation:
    on_repeated_failure: 2
    on_high_risk_path: true
    on_semantic_uncertainty: true
    max_retries_before_human: 3
    cooldown_seconds: 300
  evidence_basis: null
  unexpected_key: "must fail"
"#;

        let result = serde_yaml::from_str::<AutonomyPolicyContractDocument>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_risk_score_above_100() {
        let yaml = include_str!("../../../contracts/examples/autonomy-policy.yaml").replacen(
            "risk_score: 25",
            "risk_score: 101",
            1,
        );

        let err = serde_yaml::from_str::<AutonomyPolicyContractDocument>(&yaml).unwrap_err();

        assert!(err.to_string().contains("risk_score"));
    }

    #[test]
    fn rejects_requires_approval_above_above_100() {
        let yaml = include_str!("../../../contracts/examples/autonomy-policy.yaml").replacen(
            "requires_approval_above: 80",
            "requires_approval_above: 101",
            1,
        );

        let err = serde_yaml::from_str::<AutonomyPolicyContractDocument>(&yaml).unwrap_err();

        assert!(err.to_string().contains("requires_approval_above"));
    }

    #[test]
    fn effective_mode_for_uses_override_or_default() {
        let contract = example_contract().autonomy_policy_contract;

        assert_eq!(
            AutonomyMode::Yolo,
            contract.effective_mode_for(ToolClass::FileEdit)
        );
        assert_eq!(
            AutonomyMode::Manual,
            contract.effective_mode_for(ToolClass::SecretAccess)
        );
        assert_eq!(
            AutonomyMode::SandboxAuto,
            contract.effective_mode_for(ToolClass::PackageInstall)
        );
    }
}
