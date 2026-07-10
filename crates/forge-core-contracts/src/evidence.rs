use crate::common::{SourceId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldEvidenceRegistry {
    pub schema_version: String,
    pub research: String,
    pub created_at: String,
    pub status: String,
    pub policy: EvidencePolicy,
    pub sources: Vec<EvidenceSource>,
    #[serde(default)]
    pub plan_level_implications: Vec<PlanLevelImplication>,
    #[serde(default)]
    pub open_research_gaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidencePolicy {
    pub purpose: String,
    pub evidence_tiers: Vec<EvidenceTier>,
    pub rule: String,
    pub geographic_coverage: GeographicCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceTier {
    pub id: StableId,
    pub description: String,
    pub decision_weight: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeographicCoverage {
    pub rule: String,
    pub rationale: String,
    pub minimum_behavior: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSource {
    pub id: SourceId,
    pub tier: StableId,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub confirmed_origin: Option<String>,
    #[serde(default)]
    pub observed_claims: Vec<String>,
    #[serde(default)]
    pub forge_implications: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanLevelImplication {
    pub id: StableId,
    pub rule: String,
}
