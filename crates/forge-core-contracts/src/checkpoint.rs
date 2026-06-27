//! Checkpoint / resume contract — stub, fleshed out by Wave 3 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointContractDocument {
    pub schema_version: String,
    pub checkpoint_contract: CheckpointContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointContract {
    pub _placeholder: String,
}
