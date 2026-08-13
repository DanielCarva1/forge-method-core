use crate::{FunnelContactDensity, Phase, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PRODUCT_JOURNEY_SCHEMA_VERSION: &str = "0.1";
pub const PRODUCT_JOURNEY_CONTRACT_REF: &str = "contracts/guidance/product-journey-v0.yaml";
pub const PRODUCT_JOURNEY_GUIDANCE_SCHEMA_VERSION: &str = "product_journey_guidance_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyDocument {
    pub schema_version: String,
    pub artifact_kind: ProductJourneyArtifactKind,
    pub status: ProductJourneyStatus,
    pub product_journey: ProductJourney,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourney {
    pub id: StableId,
    pub stages: Vec<ProductStageDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductStageDescriptor {
    pub phase: Phase,
    pub stage_id: StableId,
    pub display_name: String,
    pub objective: String,
    pub contact_density: FunnelContactDensity,
    pub lifecycle_stage: bool,
    #[serde(default)]
    pub transition_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductJourneyArtifactKind {
    ProductJourney,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductJourneyStatus {
    Accepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyGuidance {
    pub schema_version: String,
    pub authority: ProductJourneyGuidanceAuthority,
    pub phase: Phase,
    pub stage: ProductJourneyGuidanceStage,
    pub catalog: ProductJourneyGuidanceCatalog,
    pub recommendation_owner: ProductJourneyRecommendationOwner,
    #[serde(deserialize_with = "deserialize_false")]
    pub recommendation_is_authority: bool,
}

fn deserialize_false<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = bool::deserialize(deserializer)?;
    if value {
        return Err(serde::de::Error::custom(
            "advisory journey guidance cannot be authority",
        ));
    }
    Ok(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyGuidanceStage {
    pub id: StableId,
    pub display_name: String,
    pub objective: String,
    pub contact_density: FunnelContactDensity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyGuidanceCatalog {
    pub eligible_count: usize,
    pub status_argv: Vec<String>,
    pub detail_argv: ProductJourneyDetailArgv,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyDetailArgv {
    pub argv: Vec<String>,
    pub workflow_id_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductJourneyGuidanceAuthority {
    AdvisoryReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductJourneyRecommendationOwner {
    HostAgent,
}

#[cfg(test)]
mod tests {
    #[test]
    fn advisory_guidance_rejects_authority_true() {
        let value = serde_json::json!({
            "schema_version": "product_journey_guidance_v1",
            "authority": "advisory_read_only",
            "phase": "1-discovery",
            "stage": {
                "id": "analysis-discovery",
                "display_name": "Analysis and Discovery",
                "objective": "Understand the product change",
                "contact_density": "high"
            },
            "catalog": {
                "eligible_count": 1,
                "status_argv": ["forge-core", "guide", "status", "--json"],
                "detail_argv": {
                    "argv": ["forge-core", "guide", "detail", "--workflow", "__ID__", "--json"],
                    "workflow_id_token": "__ID__"
                }
            },
            "recommendation_owner": "host_agent",
            "recommendation_is_authority": true
        });

        let error = serde_json::from_value::<super::ProductJourneyGuidance>(value)
            .expect_err("advisory guidance cannot claim authority");
        assert!(error.to_string().contains("cannot be authority"));
    }
}
