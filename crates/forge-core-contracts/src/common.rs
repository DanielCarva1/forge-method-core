use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A stable, opaque identifier carried as a plain string on the wire.
///
/// Use this for ids that are NEVER compared against a claim id in the R8 sense
/// (agent ids, product-area ids, reason codes). For the two ids that WERE confused
/// in R8, use the dedicated newtypes below.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct StableId(pub String);

/// The id of a CLAIM SCOPE — what the operator types at acquire time (e.g. `s1`).
///
/// R8 (slice-5 live demo): `scope.id` and `claim.id` shared the same `StableId`
/// type, so `claims.iter().find(|c| &c.id == id)` compiled but silently never
/// matched the operator's scope id (the on-disk id is the derived canonical form
/// `claim.lane.s1.s1`). Splitting into a distinct type makes that comparison a
/// **compile error** — the R8 bug class becomes unrepresentable.
///
/// `#[serde(transparent)]` keeps the wire format identical to the legacy string,
/// so existing YAML fixtures deserialize unchanged (zero migration cost).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ScopeId(pub String);

/// The derived canonical id of a CLAIM (`claim.lane.s1.s1`).
///
/// Distinct from [`ScopeId`] so the two can never be silently compared. See the
/// [`ScopeId`] docs for the R8 rationale.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ClaimId(pub String);

/// The id of a PRINCIPAL — a human or an agent (F07 multi-principal governance).
///
/// R8 type-discipline: F07's authorization structures (`IntentContract { principal,
/// authority_scope }`, `ConflictContract { principal_a, principal_b }`, governance
/// `is_authorized(principal, resource)`) put a principal id and a resource id in the
/// same comparison, where a field/argument swap is a silent, security-relevant bug
/// — exactly the class the [`ScopeId`]/[`ClaimId`] split made unrepresentable. A
/// distinct `PrincipalId` makes that swap a compile error.
///
/// This **formally supersedes ADR 0023's F07-prediction** ("F07 does not introduce a
/// rival `PrincipalId` type"), recorded in the expanded ADR-0007. The industrial
/// precedent ADR 0023 itself cites (AWS Cedar, Google Zanzibar) enforces typed
/// `Principal`/`Resource` separation for the same reason. `#[serde(transparent)]`
/// keeps the wire format identical to the legacy string (`"principal.daniel"`), so
/// existing YAML (including F06's `reviewed_by`) deserializes unchanged — zero
/// migration cost, the proven `ScopeId` pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PrincipalId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct SourceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct RepoPath(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IdRule {
    pub id: StableId,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourcePattern {
    pub source_id: SourceId,
    pub supports: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBasis {
    #[serde(default)]
    pub direct_patterns: Vec<SourcePattern>,
    #[serde(default)]
    pub non_western_coverage_note: Option<String>,
    pub inference_boundary: String,
}
