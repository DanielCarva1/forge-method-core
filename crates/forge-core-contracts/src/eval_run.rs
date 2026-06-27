//! Eval run / outcome observability contract — stub, fleshed out by Wave 4 worker.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalRunContractDocument {
    pub schema_version: String,
    pub eval_run_contract: EvalRunContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvalRunContract {
    pub _placeholder: String,
}
