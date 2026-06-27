//! Autonomy policy contract — stub, fleshed out by Wave 3 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Document stub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPolicyContractDocument {
    pub schema_version: String,
    pub autonomy_policy_contract: AutonomyPolicyContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPolicyContract {
    pub _placeholder: String,
}
