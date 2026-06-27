//! Verification goal contract — stub, fleshed out by Wave 3 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationGoalContractDocument {
    pub schema_version: String,
    pub verification_goal_contract: VerificationGoalContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationGoalContract {
    pub _placeholder: String,
}
