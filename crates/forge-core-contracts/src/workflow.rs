use crate::common::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Serde document wrapper for a workflow contract on disk.
///
/// Mirrors the established `*ContractDocument` pattern (`OperationContractDocument`,
/// `GateContractDocument`): a `schema_version` plus a single inner contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDocument {
    pub schema_version: String,
    pub workflow: Workflow,
}

/// A single Forge Method workflow: a compact state machine.
///
/// Agent-facing workflow docs are compact state machines with exactly these
/// directional slots: `trigger`, `inputs`, `steps`, `outputs`, `done_when`,
/// `blocked_when`, `handoff`.
///
/// `steps` carry DIRECTIONAL text (what to achieve), never scripted persona
/// behavior (DC9). The orchestrator-guide (slice 2) classifies human intent
/// and dispatches onto these typed workflows (DC1).
///
/// `phase` is a free-form `StableId` (e.g. `"3-plan"`) so that catalog migration
/// never fails deserialization; the engine categorizes each tag via
// [`crate::phase::Phase::parse`] / [`crate::phase::Phase::tag_eligible`] at
// routing time. A workflow that has not yet been assigned phases deserializes
// to an empty set. The string `"anytime"` is an eligibility wildcard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    /// Canonical id, e.g. `"plan-sprint"`. Stable and unique across the catalog.
    pub id: StableId,
    /// Funnel phase tags this workflow is eligible in, e.g.
    /// `["3-plan"]` or `["1-discovery", "2-specification", "3-plan"]`.
    /// Free-form strings; parsed by the engine. May include `"anytime"`.
    #[serde(default)]
    pub phases: Vec<StableId>,
    /// Predicate expressions evaluated against project state, e.g.
    /// `state.phase == 3-plan`. The orchestrator matches these to route intent.
    #[serde(default)]
    pub trigger: Vec<String>,
    /// Artifacts/state the workflow consumes.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Directional steps (what to achieve). NOT scripted persona behavior (DC9).
    #[serde(default)]
    pub steps: Vec<String>,
    /// Artifacts/state the workflow produces.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Predicate expressions: when the workflow is complete.
    #[serde(default)]
    pub done_when: Vec<String>,
    /// Predicate expressions: when the workflow cannot proceed.
    #[serde(default)]
    pub blocked_when: Vec<String>,
    /// State to preserve when handing off to the next actor/workflow.
    #[serde(default)]
    pub handoff: Vec<String>,
    /// Owning module id, when this workflow belongs to a module group.
    #[serde(default)]
    pub module: Option<StableId>,
}
