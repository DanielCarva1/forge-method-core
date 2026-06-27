//! Agent run / run-graph contract — stub, fleshed out by Wave 3 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRunContractDocument {
    pub schema_version: String,
    pub agent_run_contract: AgentRunContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRunContract {
    pub _placeholder: String,
}
