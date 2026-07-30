//! Host-neutral semantic boundary between delegated agent work and irreducible human decisions.

use crate::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const AGENT_AUTONOMY_ASSESSMENT_SCHEMA_VERSION: &str = "agent_autonomy_assessment_v1";
pub const MAX_AGENT_AUTONOMY_INPUT_BYTES: u64 = 64 * 1024;
pub const MAX_AGENT_AUTONOMY_SUMMARY_BYTES: usize = 4 * 1024;

/// Work the owner delegates to an agent once an objective is active.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentOwnedWorkClass {
    ResearchAndAnalysis,
    PlanningAndStrategy,
    ReversibleLocalEditing,
    TestingAndVerification,
    Documentation,
    TacticFileOrderOrRetryChange,
    ExternalReadOnlyResearch,
    ReversibleLocalCommit,
    /// Producing local evidence is delegated. Admission, promotion, and external
    /// publication of that evidence remain governed separately.
    EvidenceGeneration,
}

impl AgentOwnedWorkClass {
    pub const ALL: [Self; 9] = [
        Self::ResearchAndAnalysis,
        Self::PlanningAndStrategy,
        Self::ReversibleLocalEditing,
        Self::TestingAndVerification,
        Self::Documentation,
        Self::TacticFileOrderOrRetryChange,
        Self::ExternalReadOnlyResearch,
        Self::ReversibleLocalCommit,
        Self::EvidenceGeneration,
    ];
}

/// The complete and intentionally small set of decisions Forge reserves for a human.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HumanDecisionClass {
    ProductObjectiveChange,
    MaterialTradeoff,
    MaterialRiskAcceptance,
    IrreversibleOrExternalEffect,
}

impl HumanDecisionClass {
    pub const ALL: [Self; 4] = [
        Self::ProductObjectiveChange,
        Self::MaterialTradeoff,
        Self::MaterialRiskAcceptance,
        Self::IrreversibleOrExternalEffect,
    ];
}

/// Effects that may never be represented as autonomous agent-owned work.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedEffect {
    Publication,
    RemotePushOrMerge,
    Deployment,
    ProductionMutation,
    SecretUse,
    DestructiveExternalEffect,
}

impl ProtectedEffect {
    pub const ALL: [Self; 6] = [
        Self::Publication,
        Self::RemotePushOrMerge,
        Self::Deployment,
        Self::ProductionMutation,
        Self::SecretUse,
        Self::DestructiveExternalEffect,
    ];
}

/// A closed effect/scope descriptor derived by a host from the selected tool
/// and operation boundary, independently of the model-declared work class.
/// Hosts must not derive this value from free-form task text alone.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentAutonomyEffectDescriptor {
    LocalReadOnly,
    LocalReversible,
    ExternalReadOnly,
    /// A non-destructive external write such as sending a message/email,
    /// mutating Jira, making an HTTP write, or changing a staging system.
    ExternalMutation,
    ProtectedEffect {
        effect: ProtectedEffect,
    },
    UnknownOrAmbiguous,
}

impl AgentAutonomyEffectDescriptor {
    pub const ALL: [Self; 11] = [
        Self::LocalReadOnly,
        Self::LocalReversible,
        Self::ExternalReadOnly,
        Self::ExternalMutation,
        Self::ProtectedEffect {
            effect: ProtectedEffect::Publication,
        },
        Self::ProtectedEffect {
            effect: ProtectedEffect::RemotePushOrMerge,
        },
        Self::ProtectedEffect {
            effect: ProtectedEffect::Deployment,
        },
        Self::ProtectedEffect {
            effect: ProtectedEffect::ProductionMutation,
        },
        Self::ProtectedEffect {
            effect: ProtectedEffect::SecretUse,
        },
        Self::ProtectedEffect {
            effect: ProtectedEffect::DestructiveExternalEffect,
        },
        Self::UnknownOrAmbiguous,
    ];
}

/// Read-only compare-and-swap coordinates projected by `workflow next`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentAutonomyBinding {
    pub objective_id: StableId,
    pub objective_revision: u64,
    pub objective_digest: String,
    pub assurance_epoch: u64,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
}

/// One closed semantic work description. Effect scope is deliberately carried
/// separately in `AgentAutonomyAssessmentInput` so a declared class cannot mask
/// an external or protected operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentAutonomyWork {
    AgentOwned {
        class: AgentOwnedWorkClass,
        summary: String,
    },
    HumanDecision {
        class: HumanDecisionClass,
        summary: String,
    },
}

impl AgentAutonomyWork {
    #[must_use]
    pub fn summary(&self) -> &str {
        match self {
            Self::AgentOwned { summary, .. } | Self::HumanDecision { summary, .. } => summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentAutonomyAssessmentInput {
    pub schema_version: String,
    pub binding: AgentAutonomyBinding,
    pub work: AgentAutonomyWork,
    /// Required host/tool-derived descriptor. It has no serde default by design.
    pub effect: AgentAutonomyEffectDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentAutonomyDecisionAlternative {
    pub id: StableId,
    pub description: String,
    pub consequences: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentAutonomyDecisionRequest {
    pub id: StableId,
    pub class: HumanDecisionClass,
    pub effect: AgentAutonomyEffectDescriptor,
    pub question: String,
    pub rationale: String,
    pub recommendation: AgentAutonomyDecisionAlternative,
    /// At least two choices in addition to `recommendation`.
    pub alternatives: Vec<AgentAutonomyDecisionAlternative>,
}

/// A closed response shape prevents consumers from observing contradictory
/// proceed/decision fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentAutonomyAssessment {
    ProceedAutonomously {
        schema_version: String,
        binding: AgentAutonomyBinding,
        class: AgentOwnedWorkClass,
    },
    DecisionRequired {
        schema_version: String,
        binding: AgentAutonomyBinding,
        request: AgentAutonomyDecisionRequest,
    },
}
