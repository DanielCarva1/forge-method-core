use crate::{FunnelContactDensity, Phase, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PRODUCT_JOURNEY_SCHEMA_VERSION: &str = "0.1";
pub const PRODUCT_JOURNEY_CONTRACT_REF: &str = "contracts/guidance/product-journey-v0.yaml";

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
