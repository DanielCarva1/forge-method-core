//! Telemetry export contract — stub, fleshed out by Wave 4 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryContractDocument {
    pub schema_version: String,
    pub telemetry_contract: TelemetryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryContract {
    pub _placeholder: String,
}
