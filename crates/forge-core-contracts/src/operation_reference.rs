use crate::common::{EvidenceBasis, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Policy governing which **cross-contract references** an operation may carry
/// (e.g. pointing at a `Request`, `Decision`, `Gate`, or `Effect` contract).
///
/// Despite the word "reference", this is NOT about citation/evidence references
/// in the F14 sense (those are `ResearchSource` / `EvidenceSource` in
/// `research.rs` / `evidence.rs`). This document constrains the *side-contract*
/// wiring of an operation: which other contracts it is allowed to point at and
/// under which design rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationCrossReferencePolicyDocument {
    pub schema_version: String,
    pub contract: StableId,
    pub status: StableId,
    pub purpose: String,
    pub design_rules: Vec<ReferenceDesignRule>,
    pub allowed_reference_fields: Vec<ReferenceField>,
    #[serde(default)]
    pub future_reference_fields: Vec<FutureReferenceField>,
    pub evidence_basis: EvidenceBasis,
    pub failure_modes_prevented: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceDesignRule {
    pub id: StableId,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceField {
    pub kind: ReferenceKind,
    pub field_path: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FutureReferenceField {
    pub kind: ReferenceKind,
    pub field_path: String,
    pub status: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Claim,
    Completion,
    Gate,
    Effect,
    Request,
    Decision,
    RuntimeHandoff,
    Eval,
    HealthRecovery,
}
