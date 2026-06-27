//! Memory / playbook contract — stub, fleshed out by Wave 3 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryContractDocument {
    pub schema_version: String,
    pub memory_contract: MemoryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryContract {
    pub _placeholder: String,
}
