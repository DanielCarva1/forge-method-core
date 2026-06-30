//! `claim` command family — the governance surface (slice 4, S4.4).
//!
//! These commands expose the claim-lifecycle engine (S4.3) to host LLMs via the
//! same [`CliEnvelope`] contract as `guide/*` (DD17). An agent DECLARES a claim
//! to reserve a scope, heartbeats to keep it alive, releases to hand back, and
//! reads `status` to see the whole coordination picture (the bus view, DD15).
//!
//! # Coordination bus = filesystem (DD15, DD22)
//!
//! Every mutating lifecycle transition is appended to the claim WAL under the
//! resolved state root. `ClaimContractDocument` YAML files in `claims-active`
//! are now only a materialized compatibility/debug cache. WAL replay is the
//! default authority for status/check-write/mutating decisions; cache-only
//! state without a WAL fails closed instead of silently becoming authoritative.
//!
//! # Time is injected (DD23)
//!
//! The engine is pure and deterministic given `now_unix: i64`. The CLI accepts
//! `--now-unix <epoch>` (for replay/tests); the default is real system time.
//! Heartbeat is agent-driven (DC9): the engine never babysits a claim.

use crate::cli_error::ExitError;
use crate::cli_util::{parse_strict_or_err, require_value_or_err, resolve_now_unix};
use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimContractDocument, ClaimScopeKind, ClaimStatus,
};
use forge_core_contracts::tool_effect::ConflictCode;
use forge_core_contracts::{
    ClaimId, CliEnvelope, ExitReason, ScopeId, StableId, ENVELOPE_SCHEMA_VERSION,
};
use forge_core_engine::{
    acquire, check_write_against_claims, heartbeat, is_expired, project_active, reconcile_claims,
    record_handoff, release, unix_to_rfc3339, AcquireRequest, ActiveClaimSummary,
    ClaimLifecycleDecision, ClaimReconcileTransition, ClaimRejection, RecordHandoffRequest,
};
use forge_core_store::claim_wal::{
    append_claim_wal_record, claim_wal_path, replay_claim_wal, ClaimWalOperation,
    ClaimWalStopReason,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// ============================================================================
// result payloads (DD17: same envelope shape as guide/*)
// ============================================================================

/// Success payload for `claim acquire|heartbeat|release` when the engine
/// accepts the operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimResult {
    pub schema_version: String,
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub agent_id: String,
    pub status: String,
    pub acquired_at: String,
    pub expires_at: String,
}

/// Success payload for `claim handoff`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimHandoffResult {
    pub schema_version: String,
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub agent_id: String,
    pub status: String,
    pub recorded_at: String,
    pub handoff_ref: String,
    pub handoff_path: String,
}

/// Durable artifact written when an expired handoff-required claim is resolved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimHandoffArtifact {
    pub schema_version: String,
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub recorded_by_agent_id: String,
    pub original_claimant_agent_id: String,
    pub previous_status: String,
    pub recorded_status: String,
    pub recorded_at: String,
    pub summary: String,
    pub evidence_refs: Vec<String>,
    pub claim_contract: ClaimContract,
}

/// Status payload for `claim status`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimStatusView {
    /// Existing compatibility field: claims that are live at `now_unix`.
    pub active: Vec<ActiveClaimSummary>,
    /// Expired/open or materialized handoff blockers that are not live, but
    /// still prevent reacquire until a handoff is recorded.
    #[serde(default)]
    pub expired_handoff_required: Vec<ExpiredHandoffRequiredClaimSummary>,
}

/// Success payload for `claim reconcile`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimReconcileResult {
    pub schema_version: String,
    pub now_unix: i64,
    pub scanned: usize,
    pub changed: usize,
    pub transitions: Vec<ClaimReconcileTransitionSummary>,
}

/// One materialized reconcile transition in the CLI envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimReconcileTransitionSummary {
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub agent_id: String,
    pub from: String,
    pub to: String,
    pub reason_code: String,
    pub evaluated_at: String,
    pub paths: Vec<String>,
}

/// A compact summary of a non-live claim that still blocks reacquire because
/// its expiry policy requires handoff evidence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExpiredHandoffRequiredClaimSummary {
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub agent_id: String,
    pub role: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub status: String,
    pub blocker_reason: String,
    pub paths: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub handoff_request_ref: Option<String>,
    pub handoff_hint: String,
}

/// Failure payload for `claim acquire|heartbeat|release` when the engine
/// refuses. Carries a machine-readable `reject_code` so the host self-corrects
/// (R2 parity with `guide decide`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimRejected {
    /// One of: `already_claimed_by_other` | `path_already_claimed` |
    /// `not_claimant` | `expired_requires_handoff` | `illegal_transition` |
    /// `claim_not_found`.
    pub reject_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PersistClaimMutationError {
    WalAppend { source: String },
    SaveClaim { source: String },
}

impl std::fmt::Display for PersistClaimMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WalAppend { source } => write!(formatter, "cannot append claim WAL: {source}"),
            Self::SaveClaim { source } => write!(formatter, "cannot persist claim: {source}"),
        }
    }
}

impl ClaimResult {
    fn from_contract(c: &ClaimContract) -> Self {
        Self {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            claim_id: c.id.0.clone(),
            scope_kind: scope_kind_slug(c.scope.kind),
            scope_id: c.scope.id.0.clone(),
            agent_id: c.claim.claimant_agent_id.0.clone(),
            status: status_slug(c.status.value),
            acquired_at: c.lease.acquired_at.clone(),
            expires_at: c.lease.expires_at.clone(),
        }
    }
}

// ============================================================================
// run_* operations
// ============================================================================

/// A claim reference as typed by an operator on the CLI argv — parsed ONCE at
/// the boundary so downstream lookups are type-safe (DD49; parse-don't-validate).
///
/// R8 (slice-5): `claim.id` (canonical `claim.lane.s1.s1`) and `scope.id`
/// (operator-typed `s1`) shared one `StableId` type, so a `==` lookup silently
/// never matched the operator's token. `ClaimRef` splits them: the operator's
/// token parses into one of two variants and [`resolve_claim`] matches on the
/// variant — it can never compare a [`ClaimId`] to a [`ScopeId`] (that would be
/// a compile error, which is the whole point).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimRef {
    /// The full derived canonical id (`claim.lane.s1.s1`).
    Full(ClaimId),
    /// The operator-typed scope id (`s1`).
    Scope(ScopeId),
}

/// Parse an operator-typed claim token into a [`ClaimRef`].
///
/// Heuristic: a token beginning with the canonical prefix [`CANONICAL_CLAIM_PREFIX`]
/// is the full id; any other token is a scope id.
///
/// parse-don't-validate completion: acquire REJECTS scope ids that start with the
/// reserved canonical prefix (see [`is_reserved_claim_prefix`]), so no live
/// claim's scope id can ever be misclassified here as the `Full` variant.
#[must_use]
pub fn parse_claim_ref(token: &str) -> ClaimRef {
    if is_reserved_claim_prefix(token) {
        ClaimRef::Full(ClaimId(token.to_string()))
    } else {
        ClaimRef::Scope(ScopeId(token.to_string()))
    }
}

/// The reserved prefix the engine uses to derive canonical claim ids
/// (`claim.<kind>.<id>.<id>`). Operator-typed scope ids MUST NOT start with this
/// or they would be misclassified by [`parse_claim_ref`] (an R8-shaped hole
/// surfaced by adversarial review of slice 6).
pub const CANONICAL_CLAIM_PREFIX: &str = "claim.";

/// True if `token` begins with the reserved canonical-claim-id prefix.
#[must_use]
pub fn is_reserved_claim_prefix(token: &str) -> bool {
    token.starts_with(CANONICAL_CLAIM_PREFIX)
}

/// Resolve a parsed [`ClaimRef`] against the loaded claims by dispatching on the
/// variant (no cross-type comparison is possible). The `Full` variant matches
/// the canonical id exactly; the `Scope` variant matches the operator-typed
/// scope id — they are decided once at parse time and never "compete".
fn resolve_claim<'a>(claims: &'a [ClaimContract], r: &ClaimRef) -> Option<&'a ClaimContract> {
    match r {
        ClaimRef::Full(id) => claims.iter().find(|c| c.id == *id),
        ClaimRef::Scope(id) => claims.iter().find(|c| c.scope.id == *id),
    }
}

/// Run `claim acquire` — declare authority over a scope.
///
/// # Errors
/// `InvalidDecisionShape` (3) if the operator's scope id uses the reserved
/// canonical-prefix form (starts with `claim.`); `EnvConfig` (5) if the claims
/// dir is corrupt; `RejectedByGate` (2) if the engine refuses with a typed
/// [`ClaimRejection`].
#[must_use]
pub fn run_acquire(
    claims_dir: &Path,
    req: &AcquireRequest,
    now_unix: i64,
) -> CliEnvelope<ClaimResult> {
    // parse-don't-validate: a scope id that starts with the reserved canonical
    // prefix would be misclassified as the `Full` variant by `parse_claim_ref`
    // on every subsequent release/heartbeat — an R8-shaped hole surfaced by the
    // adversarial review of slice 6. Reject it at the acquire boundary.
    if is_reserved_claim_prefix(&req.scope_id.0) {
        return CliEnvelope::err(
            "claim.acquire",
            ExitReason::InvalidDecisionShape,
            format!(
                "scope id '{}' starts with the reserved canonical prefix '{}'; \
                 pick a scope id that does not begin with it",
                req.scope_id.0, CANONICAL_CLAIM_PREFIX
            ),
        );
    }
    // Serialize lifecycle transitions: the pure engine can't see a concurrent
    // writer, so hold the directory lock for the whole load->decide->write.
    let _lock = match crate::io_util::DirLock::acquire(claims_dir, ".forge-claim.lock") {
        Ok(l) => l,
        Err(e) => {
            return CliEnvelope::err(
                "claim.acquire",
                ExitReason::EnvConfig,
                format!("cannot acquire claims lock: {e}"),
            );
        }
    };
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    match acquire(&existing, req, now_unix) {
        ClaimLifecycleDecision::Accepted(claim) => {
            if let Err(e) = persist_claim_mutation(
                claims_dir,
                ClaimWalOperation::Acquire,
                &claim,
                &claim.lease.acquired_at,
            ) {
                return CliEnvelope::err("claim.acquire", ExitReason::EnvConfig, e.to_string());
            }
            let result = ClaimResult::from_contract(&claim);
            CliEnvelope::ok("claim.acquire", result)
        }
        ClaimLifecycleDecision::Rejected(reason) => rejected_envelope("claim.acquire", &reason),
    }
}

/// Run `claim heartbeat`. Refreshes the lease of the claimant's claim.
///
/// # Errors
/// `InvalidDecisionShape` (3) if the claim id does not exist;
/// `RejectedByGate` (2) on a typed [`ClaimRejection`].
#[must_use]
pub fn run_heartbeat(
    claims_dir: &Path,
    claim_id: &StableId,
    agent_id: &StableId,
    now_unix: i64,
) -> CliEnvelope<ClaimResult> {
    let _lock = match crate::io_util::DirLock::acquire(claims_dir, ".forge-claim.lock") {
        Ok(l) => l,
        Err(e) => {
            return CliEnvelope::err(
                "claim.heartbeat",
                ExitReason::EnvConfig,
                format!("cannot acquire claims lock: {e}"),
            );
        }
    };
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let claim_ref = parse_claim_ref(&claim_id.0);
    let Some(target) = resolve_claim(&existing, &claim_ref) else {
        return CliEnvelope::err(
            "claim.heartbeat",
            ExitReason::InvalidDecisionShape,
            format!("claim '{}' not found", claim_id.0),
        );
    };
    match heartbeat(target, agent_id, now_unix) {
        ClaimLifecycleDecision::Accepted(updated) => {
            if let Err(e) = persist_claim_mutation(
                claims_dir,
                ClaimWalOperation::Heartbeat,
                &updated,
                &updated.lease.last_heartbeat_at,
            ) {
                return CliEnvelope::err("claim.heartbeat", ExitReason::EnvConfig, e.to_string());
            }
            CliEnvelope::ok("claim.heartbeat", ClaimResult::from_contract(&updated))
        }
        ClaimLifecycleDecision::Rejected(reason) => rejected_envelope("claim.heartbeat", &reason),
    }
}

/// Run `claim release`. Returns the claim to the pool (Released), unless the
/// expiry policy mandates a handoff — then the engine refuses.
///
/// # Errors
/// `InvalidDecisionShape` (3) if the claim id does not exist;
/// `RejectedByGate` (2) on a typed [`ClaimRejection`].
#[must_use]
pub fn run_release(
    claims_dir: &Path,
    claim_id: &StableId,
    agent_id: &StableId,
    now_unix: i64,
) -> CliEnvelope<ClaimResult> {
    let _lock = match crate::io_util::DirLock::acquire(claims_dir, ".forge-claim.lock") {
        Ok(l) => l,
        Err(e) => {
            return CliEnvelope::err(
                "claim.release",
                ExitReason::EnvConfig,
                format!("cannot acquire claims lock: {e}"),
            );
        }
    };
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let claim_ref = parse_claim_ref(&claim_id.0);
    let Some(target) = resolve_claim(&existing, &claim_ref) else {
        return CliEnvelope::err(
            "claim.release",
            ExitReason::InvalidDecisionShape,
            format!("claim '{}' not found", claim_id.0),
        );
    };
    match release(target, agent_id, now_unix) {
        ClaimLifecycleDecision::Accepted(updated) => {
            if let Err(e) = persist_claim_mutation(
                claims_dir,
                ClaimWalOperation::Release,
                &updated,
                updated.status.evaluated_at.as_str(),
            ) {
                return CliEnvelope::err("claim.release", ExitReason::EnvConfig, e.to_string());
            }
            CliEnvelope::ok("claim.release", ClaimResult::from_contract(&updated))
        }
        ClaimLifecycleDecision::Rejected(reason) => rejected_envelope("claim.release", &reason),
    }
}

/// Run `claim handoff`. This is the official recovery command for an expired
/// claim whose policy requires handoff before the scope can be reused.
///
/// # Errors
/// `InvalidDecisionShape` (3) if the claim id does not exist;
/// `RejectedByGate` (2) on a typed [`ClaimRejection`]; `EnvConfig` (5) if the
/// handoff artifact or updated claim cannot be persisted.
#[must_use]
pub fn run_handoff(
    claims_dir: &Path,
    claim_id: &StableId,
    agent_id: &StableId,
    summary: &str,
    evidence_refs: &[String],
    now_unix: i64,
) -> CliEnvelope<ClaimHandoffResult> {
    let _lock = match crate::io_util::DirLock::acquire(claims_dir, ".forge-claim.lock") {
        Ok(l) => l,
        Err(e) => {
            return CliEnvelope::err(
                "claim.handoff",
                ExitReason::EnvConfig,
                format!("cannot acquire claims lock: {e}"),
            );
        }
    };
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let claim_ref = parse_claim_ref(&claim_id.0);
    let Some(target) = resolve_claim(&existing, &claim_ref) else {
        return CliEnvelope::err(
            "claim.handoff",
            ExitReason::InvalidDecisionShape,
            format!("claim '{}' not found", claim_id.0),
        );
    };

    let artifact_ref = handoff_artifact_ref(target, now_unix);
    let mut claim_evidence_refs = Vec::with_capacity(evidence_refs.len() + 1);
    claim_evidence_refs.push(artifact_ref.clone());
    claim_evidence_refs.extend(evidence_refs.iter().cloned());
    let request = RecordHandoffRequest {
        recorder_agent_id: agent_id.clone(),
        summary: summary.to_string(),
        evidence_refs: claim_evidence_refs,
    };

    match record_handoff(target, &request, now_unix) {
        ClaimLifecycleDecision::Accepted(updated) => {
            let artifact_path = state_root_from_claims_dir(claims_dir).join(&artifact_ref);
            let artifact = ClaimHandoffArtifact {
                schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
                claim_id: updated.id.0.clone(),
                scope_kind: scope_kind_slug(updated.scope.kind),
                scope_id: updated.scope.id.0.clone(),
                recorded_by_agent_id: agent_id.0.clone(),
                original_claimant_agent_id: target.claim.claimant_agent_id.0.clone(),
                previous_status: status_slug(target.status.value),
                recorded_status: status_slug(updated.status.value),
                recorded_at: unix_to_rfc3339(now_unix),
                summary: summary.trim().to_string(),
                evidence_refs: evidence_refs.to_vec(),
                claim_contract: target.clone(),
            };
            if let Err(e) = save_handoff_artifact(&artifact_path, &artifact) {
                return CliEnvelope::err(
                    "claim.handoff",
                    ExitReason::EnvConfig,
                    format!("cannot persist handoff artifact: {e}"),
                );
            }
            if let Err(e) = persist_claim_mutation(
                claims_dir,
                ClaimWalOperation::HandoffRecorded,
                &updated,
                updated.status.evaluated_at.as_str(),
            ) {
                return CliEnvelope::err("claim.handoff", ExitReason::EnvConfig, e.to_string());
            }
            CliEnvelope::ok(
                "claim.handoff",
                ClaimHandoffResult {
                    schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
                    claim_id: updated.id.0,
                    scope_kind: scope_kind_slug(updated.scope.kind),
                    scope_id: updated.scope.id.0,
                    agent_id: agent_id.0.clone(),
                    status: status_slug(updated.status.value),
                    recorded_at: unix_to_rfc3339(now_unix),
                    handoff_ref: artifact_ref,
                    handoff_path: artifact_path.display().to_string(),
                },
            )
        }
        ClaimLifecycleDecision::Rejected(reason) => rejected_handoff_envelope(&reason),
    }
}

/// Run `claim status` — the coordination-bus view: every claim that is live
/// right now, plus non-live handoff-required blockers that still prevent
/// reacquire until their recovery evidence is recorded.
#[must_use]
pub fn run_status(claims_dir: &Path, now_unix: i64) -> CliEnvelope<ClaimStatusView> {
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let active_view = project_active(&existing, now_unix);
    let expired_handoff_required = existing
        .iter()
        .filter_map(|claim| expired_handoff_required_summary(claim, now_unix))
        .collect();
    let view = ClaimStatusView {
        active: active_view.active,
        expired_handoff_required,
    };
    CliEnvelope::ok("claim.status", view)
}

/// Run one deterministic claim reconciliation pass.
///
/// This materializes stale/expired claim lifecycle transitions in the WAL and
/// refreshes the YAML compatibility cache. It does not record handoff evidence;
/// `claim handoff` remains the official recovery edge.
#[must_use]
pub fn run_reconcile_once(claims_dir: &Path, now_unix: i64) -> CliEnvelope<ClaimReconcileResult> {
    let _lock = match crate::io_util::DirLock::acquire(claims_dir, ".forge-claim.lock") {
        Ok(l) => l,
        Err(e) => {
            return CliEnvelope::err(
                "claim.reconcile",
                ExitReason::EnvConfig,
                format!("cannot acquire claims lock: {e}"),
            );
        }
    };
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let report = reconcile_claims(&existing, now_unix);
    let mut transitions = Vec::with_capacity(report.transitions.len());
    for transition in report.transitions {
        if let Err(e) = persist_claim_mutation(
            claims_dir,
            ClaimWalOperation::ReconcileStatus,
            &transition.updated,
            transition.updated.status.evaluated_at.as_str(),
        ) {
            return CliEnvelope::err("claim.reconcile", ExitReason::EnvConfig, e.to_string());
        }
        transitions.push(reconcile_transition_summary(&transition));
    }
    CliEnvelope::ok(
        "claim.reconcile",
        ClaimReconcileResult {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            now_unix,
            scanned: report.scanned,
            changed: transitions.len(),
            transitions,
        },
    )
}

fn expired_handoff_required_summary(
    claim: &ClaimContract,
    now_unix: i64,
) -> Option<ExpiredHandoffRequiredClaimSummary> {
    let blocker_reason = handoff_blocker_reason(claim, now_unix)?;
    Some(ExpiredHandoffRequiredClaimSummary {
        claim_id: claim.id.0.clone(),
        scope_kind: scope_kind_slug(claim.scope.kind),
        scope_id: claim.scope.id.0.clone(),
        agent_id: claim.claim.claimant_agent_id.0.clone(),
        role: actor_role_slug(claim.claim.claimant_role),
        acquired_at: claim.lease.acquired_at.clone(),
        expires_at: claim.lease.expires_at.clone(),
        status: status_slug(claim.status.value),
        blocker_reason: blocker_reason.to_string(),
        paths: claim.scope.paths.iter().map(|p| p.0.clone()).collect(),
        evidence_refs: claim.evidence_refs.clone(),
        handoff_request_ref: claim
            .expiry_policy
            .handoff_request_ref
            .as_ref()
            .map(|p| p.0.clone()),
        handoff_hint: handoff_hint(claim),
    })
}

fn handoff_blocker_reason(claim: &ClaimContract, now_unix: i64) -> Option<&'static str> {
    match claim.status.value {
        ClaimStatus::HandoffRequired => Some("handoff_required"),
        ClaimStatus::Active | ClaimStatus::Stale
            if claim.expiry_policy.handoff_required && is_expired(claim, now_unix) =>
        {
            Some("expired_requires_handoff")
        }
        _ => None,
    }
}

fn handoff_hint(claim: &ClaimContract) -> String {
    format!(
        "Record recovery evidence with `forge-core claim handoff --id {} --agent <recorder-agent> --summary <summary> [--evidence <path>]`; do not delete the claim file.",
        claim.id.0
    )
}

fn reconcile_transition_summary(
    transition: &ClaimReconcileTransition,
) -> ClaimReconcileTransitionSummary {
    ClaimReconcileTransitionSummary {
        claim_id: transition.claim_id.0.clone(),
        scope_kind: scope_kind_slug(transition.updated.scope.kind),
        scope_id: transition.updated.scope.id.0.clone(),
        agent_id: transition.updated.claim.claimant_agent_id.0.clone(),
        from: status_slug(transition.from),
        to: status_slug(transition.to),
        reason_code: transition.reason_code.0.clone(),
        evaluated_at: transition.updated.status.evaluated_at.clone(),
        paths: transition
            .updated
            .scope
            .paths
            .iter()
            .map(|path| path.0.clone())
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// check-write: layer-2 prevention exposed to agents (S4.5)
// ---------------------------------------------------------------------------

/// One blocked target in the serialized payload.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WriteBlock {
    pub blocked_path: String,
    pub blocking_claim_id: String,
    pub claimant: String,
    pub conflict_code: String,
}

/// Payload for `forge-core check-write`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WriteCheckPayload {
    pub writer: String,
    /// `blocked` if false, `allowed` if true.
    pub allowed: bool,
    pub governed_by_self: Vec<String>,
    pub ungoverned: Vec<String>,
    pub blocks: Vec<WriteBlock>,
}

/// Run `forge-core check-write`. An agent calls this BEFORE editing a file to
/// learn whether its write set collides with another agent's live claim.
///
/// - allowed + no blocks  -> every target is inside the writer's own live
///   claim.
/// - blocked              -> exit 2 (`RejectedByGate`); the writer must acquire
///   its own scope or wait for a handoff. The `blocks` / `ungoverned` arrays
///   tell it exactly which paths need correction.
///
/// # Time-of-check vs time-of-use
///
/// The verdict is a POINT-IN-TIME advisory snapshot. Between this call and
/// the actual write, another agent may acquire a conflicting claim. Layer-2
/// prevention therefore narrows — but does not eliminate — the collision
/// window. The integrity spine (layer 3) remains the authoritative gate at
/// write time: a write that slips past this stale verdict still fails safe at
/// the spine. Callers MUST NOT treat `allowed` as a durable authorization.
///
/// # Errors
/// `EnvConfig` (5) if the claims directory cannot be read.
#[must_use]
pub fn run_check_write(
    claims_dir: &Path,
    writer_agent_id: &StableId,
    targets: &[String],
    now_unix: i64,
) -> CliEnvelope<WriteCheckPayload> {
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let repo_paths: Vec<_> = targets
        .iter()
        .map(|t| forge_core_contracts::RepoPath(t.clone()))
        .collect();
    let verdict = check_write_against_claims(&repo_paths, writer_agent_id, &existing, now_unix);
    match verdict {
        forge_core_engine::WriteCheck::Ok {
            governed_by_self,
            ungoverned,
        } => {
            let governed_by_self: Vec<String> = governed_by_self.into_iter().map(|p| p.0).collect();
            let ungoverned: Vec<String> = ungoverned.into_iter().map(|p| p.0).collect();
            if ungoverned.is_empty() {
                CliEnvelope::ok(
                    "check-write",
                    WriteCheckPayload {
                        writer: writer_agent_id.0.clone(),
                        allowed: true,
                        governed_by_self,
                        ungoverned,
                        blocks: Vec::new(),
                    },
                )
            } else {
                let payload = WriteCheckPayload {
                    writer: writer_agent_id.0.clone(),
                    allowed: false,
                    governed_by_self,
                    ungoverned: ungoverned.clone(),
                    blocks: Vec::new(),
                };
                let mut msg = format!(
                    "write blocked: {} target(s) are not covered by the writer's live claim:\n",
                    ungoverned.len()
                );
                for path in &ungoverned {
                    let _ = writeln!(msg, "  - {path}");
                }
                msg.push_str("acquire a claim covering every target before writing.");
                CliEnvelope::reject("check-write", ExitReason::RejectedByGate, msg, payload)
            }
        }
        forge_core_engine::WriteCheck::Blocked { blocks } => {
            // M1 fix: emit the STRUCTURED payload alongside the rejection so
            // the writer can self-correct programmatically (DD17), not just by
            // parsing prose. The legible message is still attached for stderr.
            let payload_blocks: Vec<WriteBlock> = blocks
                .iter()
                .map(|b| WriteBlock {
                    blocked_path: b.blocked_path.0.clone(),
                    blocking_claim_id: b.blocking_claim_id.0.clone(),
                    claimant: b.claimant.0.clone(),
                    conflict_code: conflict_code_str(b.conflict_code).to_string(),
                })
                .collect();
            let payload = WriteCheckPayload {
                writer: writer_agent_id.0.clone(),
                allowed: false,
                governed_by_self: Vec::new(),
                ungoverned: Vec::new(),
                blocks: payload_blocks,
            };
            let mut msg = format!(
                "write blocked: {} target(s) collide with another agent's live claim:\n",
                blocks.len()
            );
            for b in &blocks {
                let _ = writeln!(
                    msg,
                    "  - {} (claimed by '{}' via claim '{}'; conflict: {})",
                    b.blocked_path.0,
                    b.claimant.0,
                    b.blocking_claim_id.0,
                    conflict_code_str(b.conflict_code)
                );
            }
            msg.push_str("acquire your own scope or wait for a handoff.");
            CliEnvelope::reject("check-write", ExitReason::RejectedByGate, msg, payload)
        }
    }
}

/// Map a [`ConflictCode`] to its stable `snake_case` string for the
/// machine-readable payload. Hard-coded (not Debug-derived) so the wire
/// format is independent of future enum refactors (review S4.5 M1).
#[must_use]
pub fn conflict_code_str(c: ConflictCode) -> &'static str {
    match c {
        ConflictCode::WriteTargetClaimed => "write_target_claimed",
        ConflictCode::OverlappingWriteSet => "overlapping_write_set",
        ConflictCode::PathOutsideScope => "path_outside_scope",
        ConflictCode::ReadTargetChanged => "read_target_changed",
        ConflictCode::WriteTargetChanged => "write_target_changed",
        ConflictCode::ExpectedStateVersionMismatch => "expected_state_version_mismatch",
        ConflictCode::CompletionNowDone => "completion_now_done",
        ConflictCode::MissingInverseForDestructiveWrite => "missing_inverse_for_destructive_write",
    }
}
// ============================================================================
// persistence (DD22: filesystem is the bus; spine audits writes)
// ============================================================================

/// Load the authoritative claim state for `dir`.
///
/// WAL replay is authoritative when `wal/claims.fmw1` exists under the inferred
/// state root. The YAML files in `dir` are only a materialized cache. If that
/// cache contains claim documents but no WAL exists, this fails closed with a
/// migration/debug error instead of silently trusting cache-only state.
#[must_use]
pub fn load_claims(dir: &Path) -> (Vec<ClaimContract>, Vec<String>) {
    let state_root = state_root_from_claims_dir(dir);
    let wal_path = claim_wal_path(&state_root);
    if wal_path.exists() {
        return load_claims_from_wal(&state_root);
    }
    let (cache_claims, cache_errors) = load_claims_from_cache(dir);
    if !cache_errors.is_empty() {
        return (Vec::new(), cache_errors);
    }
    if cache_claims.is_empty() {
        return (cache_claims, Vec::new());
    }
    (
        Vec::new(),
        vec![format!(
            "{}: claim cache contains {} YAML document(s), but authoritative WAL {} is missing; run an explicit migration/import or inspect with a cache/debug reader",
            dir.display(),
            cache_claims.len(),
            wal_path.display()
        )],
    )
}

fn load_claims_from_wal(state_root: &Path) -> (Vec<ClaimContract>, Vec<String>) {
    match replay_claim_wal(state_root, false) {
        Ok(projection) if projection.recovery.stop_reason == ClaimWalStopReason::CleanEof => {
            (projection.claims, Vec::new())
        }
        Ok(projection)
            if matches!(
                projection.recovery.stop_reason,
                ClaimWalStopReason::TruncatedHeader | ClaimWalStopReason::TruncatedPayload
            ) =>
        {
            match replay_claim_wal(state_root, true) {
                Ok(_) => match replay_claim_wal(state_root, false) {
                    Ok(repaired)
                        if repaired.recovery.stop_reason == ClaimWalStopReason::CleanEof =>
                    {
                        (repaired.claims, Vec::new())
                    }
                    Ok(repaired) => (
                        Vec::new(),
                        vec![format!(
                            "{}: claim WAL recovery still stopped with {:?} after repair",
                            repaired.recovery.wal_path.display(),
                            repaired.recovery.stop_reason
                        )],
                    ),
                    Err(error) => (
                        Vec::new(),
                        vec![format!(
                            "{}: claim WAL reread after repair failed: {error}",
                            state_root.display()
                        )],
                    ),
                },
                Err(error) => (
                    Vec::new(),
                    vec![format!(
                        "{}: claim WAL truncation repair failed: {error}",
                        state_root.display()
                    )],
                ),
            }
        }
        Ok(projection) => (
            Vec::new(),
            vec![format!(
                "{}: claim WAL recovery stopped with {:?} at {}/{}; refusing YAML fallback",
                projection.recovery.wal_path.display(),
                projection.recovery.stop_reason,
                projection.recovery.last_good_offset,
                projection.recovery.original_len
            )],
        ),
        Err(error) => (
            Vec::new(),
            vec![format!(
                "{}: claim WAL replay failed: {error}",
                state_root.display()
            )],
        ),
    }
}

/// Load every `*.yaml` claim document in `dir` as a compatibility/debug cache.
#[must_use]
pub fn load_claims_from_cache(dir: &Path) -> (Vec<ClaimContract>, Vec<String>) {
    let mut claims = Vec::new();
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        // A missing dir is a fresh/empty claims bus, not an error.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (claims, errors),
        // Permission/IO errors MUST surface: an unreadable cache is a broken local state.
        Err(e) => {
            errors.push(format!("{}: cannot read claims dir: {e}", dir.display()));
            return (claims, errors);
        }
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        match entry {
            Ok(e) => paths.push(e.path()),
            Err(e) => errors.push(format!("{}: dir entry error: {e}", dir.display())),
        }
    }
    paths.retain(|p| p.extension().is_some_and(|x| x == "yaml"));
    paths.sort();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            errors.push(format!("{}: unreadable", path.display()));
            continue;
        };
        match yaml_serde::from_str::<ClaimContractDocument>(&text) {
            Ok(doc) => claims.push(doc.claim_contract),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (claims, errors)
}

/// Persist a claim as `ClaimContractDocument` YAML, overwriting its prior state.
/// The filename is derived from the claim id (sanitized; no path traversal).
///
/// # Errors
/// Returns `io::Error` if the directory cannot be created, the claim cannot
/// be serialized, or the file cannot be written.
pub fn save_claim(dir: &Path, claim: &ClaimContract) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let doc = ClaimContractDocument {
        schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
        claim_contract: claim.clone(),
    };
    let yaml = yaml_serde::to_string(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = dir.join(format!("{}.yaml", slug_for_file(&claim.id.0)));
    // Atomic write: temp file in the same dir, then rename over the target.
    // Prevents a crash mid-write from leaving a truncated YAML that would DoS
    // the whole claims dir on the next load (review S4.4 bug #3).
    crate::io_util::atomic_write(&path, &yaml)?;
    Ok(path)
}

fn persist_claim_mutation(
    claims_dir: &Path,
    operation: ClaimWalOperation,
    claim: &ClaimContract,
    recorded_at: &str,
) -> Result<PathBuf, PersistClaimMutationError> {
    let state_root = state_root_from_claims_dir(claims_dir);
    append_claim_wal_record(&state_root, operation, claim, recorded_at).map_err(|source| {
        PersistClaimMutationError::WalAppend {
            source: source.to_string(),
        }
    })?;
    save_claim(claims_dir, claim).map_err(|source| PersistClaimMutationError::SaveClaim {
        source: source.to_string(),
    })
}

fn state_root_from_claims_dir(claims_dir: &Path) -> PathBuf {
    match claims_dir.file_name().and_then(|name| name.to_str()) {
        Some("claims-active") => claims_dir
            .parent()
            .map_or_else(|| claims_dir.to_path_buf(), Path::to_path_buf),
        _ => claims_dir.to_path_buf(),
    }
}

fn handoff_artifact_ref(claim: &ClaimContract, now_unix: i64) -> String {
    format!(
        "handoffs/expired-claims/expired-claim-{}-{}.yaml",
        slug_for_file(&claim.scope.id.0),
        slug_for_file(&unix_to_rfc3339(now_unix))
    )
}

fn save_handoff_artifact(path: &Path, artifact: &ClaimHandoffArtifact) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = yaml_serde::to_string(artifact)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    crate::io_util::atomic_write(path, &yaml)
}

fn rejected_handoff_envelope(reason: &ClaimRejection) -> CliEnvelope<ClaimHandoffResult> {
    let rejected = ClaimRejected {
        reject_code: reject_code(reason),
        detail: rejection_detail(reason),
    };
    let mut env: CliEnvelope<ClaimHandoffResult> = CliEnvelope::err(
        "claim.handoff",
        ExitReason::RejectedByGate,
        &rejected.detail,
    );
    if let Some(err) = env.error.as_mut() {
        err.code.0 = format!("{}:{}", rejected.reject_code, rejected.detail);
    }
    env
}

// ============================================================================
// parse helpers (CLI args -> typed enums)
// ============================================================================

/// Parse a `--scope` argument into a [`ClaimScopeKind`]. Accepts the
/// `snake_case` serde names plus a few friendly aliases.
#[must_use]
pub fn parse_scope_kind(s: &str) -> Option<ClaimScopeKind> {
    match s {
        "story" => Some(ClaimScopeKind::Story),
        "lane" => Some(ClaimScopeKind::Lane),
        "product_area" | "product-area" | "area" => Some(ClaimScopeKind::ProductArea),
        "project" => Some(ClaimScopeKind::Project),
        "integration" => Some(ClaimScopeKind::Integration),
        "integration_state" | "integration-state" => Some(ClaimScopeKind::IntegrationState),
        _ => None,
    }
}

/// Parse a `--role` argument into an [`ActorRole`].
#[must_use]
pub fn parse_role(s: &str) -> Option<ActorRole> {
    match s {
        "driver" => Some(ActorRole::Driver),
        "worker" => Some(ActorRole::Worker),
        "human" => Some(ActorRole::Human),
        "runtime" => Some(ActorRole::Runtime),
        "unknown" => Some(ActorRole::Unknown),
        _ => None,
    }
}

// ============================================================================
// internals
// ============================================================================

fn rejected_envelope(op: &str, reason: &ClaimRejection) -> CliEnvelope<ClaimResult> {
    let rejected = ClaimRejected {
        reject_code: reject_code(reason),
        detail: rejection_detail(reason),
    };
    let mut env: CliEnvelope<ClaimResult> =
        CliEnvelope::err(op, ExitReason::RejectedByGate, &rejected.detail);
    if let Some(err) = env.error.as_mut() {
        err.code.0 = format!("{}:{}", rejected.reject_code, rejected.detail);
    }
    env
}

/// Map a [`ClaimRejection`] to a stable machine-readable code string.
fn reject_code(r: &ClaimRejection) -> String {
    match r {
        ClaimRejection::AlreadyClaimedByOther { .. } => "already_claimed_by_other",
        ClaimRejection::PathAlreadyClaimed { .. } => "path_already_claimed",
        ClaimRejection::NotClaimant { .. } => "not_claimant",
        ClaimRejection::ExpiredRequiresHandoff { .. } => "expired_requires_handoff",
        ClaimRejection::IllegalTransition { .. } => "illegal_transition",
        ClaimRejection::InvalidRequest { .. } => "invalid_request",
    }
    .into()
}

fn rejection_detail(reason: &ClaimRejection) -> String {
    match reason {
        ClaimRejection::ExpiredRequiresHandoff { claim_id } => format!(
            "{reason:?}. Recovery: record the required handoff with `forge-core claim handoff --id {} --agent <recorder-agent> --summary <what-happened> [--evidence <path>...]`; do not delete the claim file.",
            claim_id.0
        ),
        _ => format!("{reason:?}"),
    }
}

/// If the claims dir produced load errors, build the `EnvConfig` envelope once.
fn env_config_if_errors<T: serde::Serialize>(
    _dir: &Path,
    errors: &[String],
) -> Option<CliEnvelope<T>> {
    if errors.is_empty() {
        None
    } else {
        Some(CliEnvelope::err(
            "claim",
            ExitReason::EnvConfig,
            format!(
                "claims dir has {} malformed file(s): {}",
                errors.len(),
                errors.join("; ")
            ),
        ))
    }
}

/// Sanitize an id into a filesystem-safe filename stem (no traversal).
#[must_use]
pub fn slug_for_file(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn scope_kind_slug(k: ClaimScopeKind) -> String {
    match k {
        ClaimScopeKind::Story => "story",
        ClaimScopeKind::Lane => "lane",
        ClaimScopeKind::ProductArea => "product_area",
        ClaimScopeKind::Project => "project",
        ClaimScopeKind::Integration => "integration",
        ClaimScopeKind::IntegrationState => "integration_state",
    }
    .into()
}

fn actor_role_slug(r: ActorRole) -> String {
    match r {
        ActorRole::Driver => "driver",
        ActorRole::Worker => "worker",
        ActorRole::Human => "human",
        ActorRole::Runtime => "runtime",
        ActorRole::Unknown => "unknown",
    }
    .into()
}

fn status_slug(s: forge_core_contracts::claim::ClaimStatus) -> String {
    use forge_core_contracts::claim::ClaimStatus;
    match s {
        ClaimStatus::Active => "active",
        ClaimStatus::Stale => "stale",
        ClaimStatus::Expired => "expired",
        ClaimStatus::HandoffRequired => "handoff_required",
        ClaimStatus::HandoffRecorded => "handoff_recorded",
        ClaimStatus::Released => "released",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{parse_role, parse_scope_kind};
    use forge_core_contracts::claim::{ActorRole, ClaimScopeKind};
    use forge_core_contracts::{RepoPath, StableId};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn tempfile_dir() -> std::path::PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("forge-claim-test-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn req(scope_id: &str, agent: &str) -> AcquireRequest {
        AcquireRequest {
            scope_kind: ClaimScopeKind::Story,
            scope_id: ScopeId(scope_id.into()),
            agent_id: StableId(agent.into()),
            role: ActorRole::Worker,
            ttl_seconds: 600,
            heartbeat_interval_seconds: 120,
            paths: vec![RepoPath(format!("contracts/stories/{scope_id}.yaml"))],
            product_area: None,
            expected_state_version: None,
        }
    }

    const T0: i64 = 1_800_000_000;

    // --- acquire ---

    #[test]
    fn acquire_writes_claim_and_returns_ok() {
        let dir = tempfile_dir();
        let env = run_acquire(&dir, &req("s1", "agentA"), T0);
        assert!(env.ok, "{:?}", env.error);
        assert_eq!(env.exit_code(), 0);
        let p = env.data.as_ref().unwrap();
        assert_eq!(p.scope_id, "s1");
        assert_eq!(p.agent_id, "agentA");
        assert_eq!(p.status, "active");
        let yaml_files = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .ok()
                    .and_then(|entry| entry.path().extension().map(|ext| ext == "yaml"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(yaml_files, 1);
        let wal =
            forge_core_store::claim_wal::recover_claim_wal(&dir, false).expect("recover claim WAL");
        assert_eq!(wal.records.len(), 1);
        assert_eq!(wal.records[0].operation, ClaimWalOperation::Acquire);
    }

    #[test]
    fn acquire_rejects_second_agent_on_same_scope() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let env = run_acquire(&dir, &req("s1", "agentB"), T0);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
        let code = env.error.as_ref().unwrap().code.0.clone();
        assert!(code.starts_with("already_claimed_by_other"), "got: {code}");
    }

    #[test]
    fn acquire_rejects_same_agent_duplicate_while_live() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let env = run_acquire(&dir, &req("s1", "agentA"), T0);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
    }

    #[test]
    fn acquire_allows_disjoint_scopes() {
        let dir = tempfile_dir();
        let a = run_acquire(&dir, &req("s1", "agentA"), T0);
        let b = run_acquire(&dir, &req("s2", "agentB"), T0);
        assert!(a.ok && b.ok);
    }

    #[test]
    fn acquire_rejects_different_scope_with_overlapping_path() {
        let dir = tempfile_dir();
        let first = run_acquire(&dir, &req("s1", "agentA"), T0);
        assert!(first.ok, "{:?}", first.error);

        let mut overlapping = req("s2", "agentB");
        overlapping.paths = vec![RepoPath("contracts/stories/s1.yaml".into())];
        let env = run_acquire(&dir, &overlapping, T0 + 1);

        assert!(!env.ok, "overlapping path must be rejected");
        assert_eq!(env.exit_code(), 2);
        let code = env.error.as_ref().unwrap().code.0.clone();
        assert!(code.starts_with("path_already_claimed"), "got: {code}");
    }

    // --- heartbeat ---

    #[test]
    fn heartbeat_extends_lease_and_persists() {
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        let claim_id = StableId(acquired.data.as_ref().unwrap().claim_id.clone());
        let env = run_heartbeat(&dir, &claim_id, &StableId("agentA".into()), T0 + 300);
        assert!(env.ok, "{:?}", env.error);
        assert_eq!(env.data.as_ref().unwrap().status, "active");
    }

    #[test]
    fn heartbeat_rejects_non_claimant() {
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        let claim_id = StableId(acquired.data.as_ref().unwrap().claim_id.clone());
        let env = run_heartbeat(&dir, &claim_id, &StableId("agentB".into()), T0);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
        assert!(env
            .error
            .as_ref()
            .unwrap()
            .code
            .0
            .starts_with("not_claimant"));
    }

    #[test]
    fn heartbeat_unknown_claim_returns_exit_3() {
        let dir = tempfile_dir();
        let env = run_heartbeat(
            &dir,
            &StableId("claim.nope".into()),
            &StableId("agentA".into()),
            T0,
        );
        assert_eq!(env.exit_code(), 3);
    }

    // --- release ---

    #[test]
    fn release_round_trip_frees_scope() {
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        let claim_id = StableId(acquired.data.as_ref().unwrap().claim_id.clone());
        let rel = run_release(&dir, &claim_id, &StableId("agentA".into()), T0);
        assert!(rel.ok, "{:?}", rel.error);
        assert_eq!(rel.data.as_ref().unwrap().status, "released");
        // scope is now free -> another agent can acquire
        let again = run_acquire(&dir, &req("s1", "agentB"), T0);
        assert!(again.ok, "released scope must be re-acquirable");
    }

    #[test]
    fn release_rejects_non_claimant() {
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        let claim_id = StableId(acquired.data.as_ref().unwrap().claim_id.clone());
        let env = run_release(&dir, &claim_id, &StableId("agentB".into()), T0);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
    }

    #[test]
    fn expired_handoff_rejection_points_to_official_recovery_command() {
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        let claim_id = acquired.data.as_ref().unwrap().claim_id.clone();
        let env = run_release(
            &dir,
            &StableId(claim_id.clone()),
            &StableId("agentA".into()),
            T0 + 601,
        );

        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
        let err = env.error.as_ref().expect("release should return error");
        assert!(
            err.code.0.starts_with("expired_requires_handoff"),
            "unexpected code: {}",
            err.code.0
        );
        assert!(
            err.message.contains("forge-core claim handoff --id"),
            "message should be actionable: {}",
            err.message
        );
        assert!(
            err.message.contains(&claim_id),
            "message should name the blocking claim: {}",
            err.message
        );
    }

    #[test]
    fn release_resolves_by_scope_id_r8() {
        // R8 (slice-5 live demo): the CLI `--id` flag carries the SCOPE id the
        // operator typed at acquire, not the full derived claim id. resolve_claim
        // must accept both forms. Releasing by scope id must succeed and free the
        // scope for re-acquisition.
        let dir = tempfile_dir();
        let acquired = run_acquire(&dir, &req("s1", "agentA"), T0);
        // The full claim id is `claim.lane.s1.s1`; the operator only knows "s1".
        let scope_id = StableId("s1".into());
        assert_ne!(
            acquired.data.as_ref().unwrap().claim_id,
            scope_id.0,
            "test setup: scope id must differ from full claim id to exercise R8"
        );
        let rel = run_release(&dir, &scope_id, &StableId("agentA".into()), T0);
        assert!(rel.ok, "release by scope id must work: {:?}", rel.error);
        assert_eq!(rel.data.as_ref().unwrap().status, "released");
        // Full-id form STILL works (backwards compatible with e2e tests).
        let dir2 = tempfile_dir();
        let acquired2 = run_acquire(&dir2, &req("s1", "agentA"), T0);
        let full_id = StableId(acquired2.data.as_ref().unwrap().claim_id.clone());
        let rel2 = run_release(&dir2, &full_id, &StableId("agentA".into()), T0);
        assert!(
            rel2.ok,
            "release by full id must still work: {:?}",
            rel2.error
        );
    }

    #[test]
    fn heartbeat_resolves_by_scope_id_r8() {
        // R8 echo for heartbeat (same resolve_claim path).
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let scope_id = StableId("s1".into());
        let hb = run_heartbeat(&dir, &scope_id, &StableId("agentA".into()), T0);
        assert!(hb.ok, "heartbeat by scope id must work: {:?}", hb.error);
    }

    // --- status (the coordination-bus view) ---

    #[test]
    fn status_shows_active_claims_only() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let _ = run_acquire(&dir, &req("s2", "agentB"), T0);
        let env = run_status(&dir, T0);
        assert!(env.ok, "{:?}", env.error);
        let view = env.data.as_ref().unwrap();
        assert_eq!(view.active.len(), 2);
        assert!(view.expired_handoff_required.is_empty());
        let ids: Vec<&str> = view.active.iter().map(|c| c.scope_id.as_str()).collect();
        assert!(ids.contains(&"s1"));
        assert!(ids.contains(&"s2"));
    }

    #[test]
    fn status_excludes_expired_from_active_and_reports_handoff_blockers() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0); // will be expired at T0 + 9999
        let _ = run_acquire(&dir, &req("s2", "agentB"), T0);
        let env = run_status(&dir, T0 + 9_999); // s1 lease (ttl 600) is long past
        let view = env.data.as_ref().unwrap();
        assert_eq!(view.active.len(), 0, "both past ttl at T0+9999");
        assert_eq!(view.expired_handoff_required.len(), 2);
        assert!(view
            .expired_handoff_required
            .iter()
            .all(|claim| claim.blocker_reason == "expired_requires_handoff"));
    }

    #[test]
    fn status_serializes_to_json_cleanly() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let env = run_status(&dir, T0);
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"active\""));
        assert!(json.contains("\"expired_handoff_required\""));
        assert!(json.contains("\"scope_id\":\"s1\""));
    }

    // --- env / persistence ---

    #[test]
    fn status_on_missing_dir_is_ok_empty() {
        let env = run_status(std::path::Path::new("/nonexistent/forge/nope"), T0);
        assert!(env.ok, "{:?}", env.error);
        assert!(env.data.as_ref().unwrap().active.is_empty());
        assert!(env
            .data
            .as_ref()
            .unwrap()
            .expired_handoff_required
            .is_empty());
    }

    #[test]
    fn malformed_claim_file_surfaces_as_env_config() {
        let dir = tempfile_dir();
        std::fs::write(dir.join("broken.yaml"), "claim_contract: { not: valid").unwrap();
        let env = run_status(&dir, T0);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 5);
    }

    #[test]
    fn slug_for_file_never_allows_traversal() {
        assert_eq!(slug_for_file("claim.story.s4.5"), "claim-story-s4-5");
        assert!(!slug_for_file("../../etc/passwd").contains('/'));
        assert!(!slug_for_file("..\\evil").contains('\\'));
    }

    #[test]
    fn parse_helpers_accept_aliases() {
        assert_eq!(parse_scope_kind("story"), Some(ClaimScopeKind::Story));
        assert_eq!(
            parse_scope_kind("product-area"),
            Some(ClaimScopeKind::ProductArea)
        );
        assert_eq!(parse_role("worker"), Some(ActorRole::Worker));
        assert_eq!(parse_role("garbage"), None);
    }

    // --- R8 structural-fix defenses (slice 6 Frente A) ---

    #[test]
    fn parse_claim_ref_classifies_scope_vs_full() {
        // operator scope id -> Scope variant
        assert!(matches!(parse_claim_ref("s1"), ClaimRef::Scope(_)));
        // canonical derived id -> Full variant
        assert!(matches!(
            parse_claim_ref("claim.lane.s1.s1"),
            ClaimRef::Full(_)
        ));
    }

    #[test]
    fn acquire_rejects_reserved_prefix_scope_id() {
        // Adversarial-review hole: a scope id starting with the canonical
        // prefix would be misclassified as Full on every later release/
        // heartbeat. Acquire must reject it (parse-don't-validate).
        let dir = tempfile_dir();
        let env = run_acquire(&dir, &req("claim.evil", "agentA"), T0);
        assert!(!env.ok, "reserved-prefix scope id must be rejected");
        assert_eq!(env.exit_code(), 3, "must be InvalidDecisionShape (3)");
        // and the bus must stay empty (no claim written)
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn check_write_rejects_unclaimed_targets_by_default() {
        let dir = tempfile_dir();
        let env = run_check_write(
            &dir,
            &StableId("agentA".into()),
            &["README.md".to_string()],
            T0,
        );

        assert!(!env.ok, "unclaimed write must be rejected");
        assert_eq!(env.exit_code(), 2);
        let payload = env.data.expect("rejection should carry payload");
        assert!(!payload.allowed);
        assert_eq!(payload.ungoverned, vec!["README.md"]);
        assert!(payload.blocks.is_empty());
    }
}
pub fn run_claim_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "acquire" => run_claim_acquire(&args[2..]),
        "heartbeat" => run_claim_heartbeat(&args[2..]),
        "release" => run_claim_release(&args[2..]),
        "handoff" => run_claim_handoff(&args[2..]),
        "status" => run_claim_status(&args[2..]),
        "reconcile" => run_claim_reconcile(&args[2..]),
        "check-write" => run_claim_check_write(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core claim <subcommand> [options]");
            println!("  acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--role worker] [--ttl 600] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  heartbeat [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  release [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--loop] [--interval-ms 30000] [--max-ticks <n>] [--no-json]");
            println!("  check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--target <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  Defaults: without --claims-dir, resolves --root as a Forge project and uses <state_root>/claims-active; --claims-dir is an explicit override.");
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core claim: unknown subcommand '{other}'. Try: acquire | heartbeat | release | handoff | status | reconcile | check-write"
        ))),
    }
}

/// Resolve --now-unix to epoch seconds, defaulting to real system time (DD23).

#[must_use]
pub fn resolve_claims_dir_or_err(
    command: &str,
    claims_dir: Option<PathBuf>,
    root: &std::path::Path,
    allow_bootstrap_core: bool,
    want_json: bool,
) -> Result<PathBuf, ExitError> {
    if let Some(claims_dir) = claims_dir {
        return Ok(claims_dir);
    }

    match crate::project_cmd::resolve_project(root, allow_bootstrap_core) {
        Ok(project) if project.state_exists => {
            Ok(PathBuf::from(project.state_root).join("claims-active"))
        }
        Ok(project) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                forge_core_contracts::ExitReason::EnvConfig,
                format!(
                    "resolved Forge state_root does not exist for claim command: {}; create the sidecar .forge-method directory or fix {}",
                    project.state_root,
                    forge_core_contracts::PROJECT_LINK_FILE_NAME
                ),
            );
            crate::cli_util::emit_envelope_or_err("claim", env, want_json).map(|()| {
                unreachable!(
                    "emit_envelope_or_err Ok path is unreachable: envelope always non-zero here"
                )
            })
        }
        Err(err) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                err.exit_reason(),
                format!("project resolve failed for claim command: {err}"),
            );
            crate::cli_util::emit_envelope_or_err("claim", env, want_json).map(|()| {
                unreachable!(
                    "emit_envelope_or_err Ok path is unreachable: envelope always non-zero here"
                )
            })
        }
    }
}

pub fn run_claim_acquire(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::{parse_role, parse_scope_kind, run_acquire};
    use forge_core_contracts::{RepoPath, ScopeId, StableId};
    use forge_core_engine::AcquireRequest;

    let mut scope_kind: Option<String> = None;
    let mut scope_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut role = "worker".to_string();
    let mut ttl: u64 = 600;
    let mut heartbeat_interval: u64 = 120;
    let mut paths: Vec<String> = Vec::new();
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--scope" => {
                idx += 1;
                scope_kind = Some(require_value_or_err(args, idx, "scope")?);
            }
            "--id" => {
                idx += 1;
                scope_id = Some(require_value_or_err(args, idx, "id")?);
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value_or_err(args, idx, "agent")?);
            }
            "--role" => {
                idx += 1;
                role = require_value_or_err(args, idx, "role")?;
            }
            "--ttl" => {
                idx += 1;
                ttl = parse_strict_or_err(&require_value_or_err(args, idx, "ttl")?, "ttl")?;
            }
            "--heartbeat-interval" => {
                idx += 1;
                heartbeat_interval = parse_strict_or_err(
                    &require_value_or_err(args, idx, "heartbeat-interval")?,
                    "heartbeat-interval",
                )?;
            }
            "--path" => {
                idx += 1;
                paths.push(require_value_or_err(args, idx, "path")?);
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--role worker] [--ttl 600] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }

    let (Some(scope_kind_str), Some(scope_id), Some(agent_id)) = (scope_kind, scope_id, agent_id)
    else {
        eprintln!("claim acquire: --scope, --id, --agent are all required");
        return Err(ExitError::invalid_value(
            "claim acquire: --scope, --id, --agent are all required",
        ));
    };
    let Some(sk) = parse_scope_kind(&scope_kind_str) else {
        eprintln!("claim acquire: unknown --scope '{scope_kind_str}'");
        return Err(ExitError::invalid_value(format!(
            "claim acquire: unknown --scope '{scope_kind_str}'"
        )));
    };
    let Some(role_kind) = parse_role(&role) else {
        eprintln!("claim acquire: unknown --role '{role}'");
        return Err(ExitError::invalid_value(format!(
            "claim acquire: unknown --role '{role}'"
        )));
    };

    let req = AcquireRequest {
        scope_kind: sk,
        scope_id: ScopeId(scope_id),
        agent_id: StableId(agent_id),
        role: role_kind,
        ttl_seconds: ttl,
        heartbeat_interval_seconds: heartbeat_interval,
        paths: paths.iter().map(|p| RepoPath(p.clone())).collect(),
        product_area: None,
        expected_state_version: None,
    };
    let claims_dir = resolve_claims_dir_or_err(
        "claim.acquire",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_acquire(&claims_dir, &req, resolve_now_unix(now_unix));
    crate::cli_util::emit_envelope_or_err("claim", env, want_json)
}

pub fn run_claim_heartbeat(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_heartbeat;
    run_claim_single_target(args, "heartbeat", |claims_dir, claim_id, agent_id, now| {
        run_heartbeat(claims_dir, claim_id, agent_id, now)
    })
}

pub fn run_claim_release(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_release;
    run_claim_single_target(args, "release", |claims_dir, claim_id, agent_id, now| {
        run_release(claims_dir, claim_id, agent_id, now)
    })
}

/// Shared arg parsing for heartbeat/release (both take --id + --agent + optional dirs/time).
pub fn run_claim_single_target(
    args: &[String],
    sub: &str,
    op: impl Fn(
        &std::path::Path,
        &forge_core_contracts::StableId,
        &forge_core_contracts::StableId,
        i64,
    ) -> forge_core_contracts::CliEnvelope<ClaimResult>,
) -> Result<(), ExitError> {
    use forge_core_contracts::StableId;
    let mut claim_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--id" => {
                idx += 1;
                claim_id = Some(require_value_or_err(args, idx, "id")?);
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value_or_err(args, idx, "agent")?);
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim {sub} [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    let (Some(claim_id), Some(agent_id)) = (claim_id, agent_id) else {
        eprintln!("claim {sub}: --id and --agent are required");
        return Err(ExitError::invalid_value(format!(
            "claim {sub}: --id and --agent are required"
        )));
    };
    let claims_dir = resolve_claims_dir_or_err(
        &format!("claim.{sub}"),
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = op(
        &claims_dir,
        &StableId(claim_id),
        &StableId(agent_id),
        resolve_now_unix(now_unix),
    );
    crate::cli_util::emit_envelope_or_err("claim", env, want_json)
}

pub fn run_claim_handoff(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_handoff;
    use forge_core_contracts::StableId;

    let mut claim_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--id" => {
                idx += 1;
                claim_id = Some(require_value_or_err(args, idx, "id")?);
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value_or_err(args, idx, "agent")?);
            }
            "--summary" => {
                idx += 1;
                summary = Some(require_value_or_err(args, idx, "summary")?);
            }
            "--evidence" => {
                idx += 1;
                evidence_refs.push(require_value_or_err(args, idx, "evidence")?);
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Records official context for an expired handoff-required claim, writes <state_root>/handoffs/expired-claims, marks the old claim handoff_recorded, and reopens the scope.");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    let (Some(claim_id), Some(agent_id), Some(summary)) = (claim_id, agent_id, summary) else {
        eprintln!("claim handoff: --id, --agent, and --summary are required");
        return Err(ExitError::invalid_value(
            "claim handoff: --id, --agent, and --summary are required",
        ));
    };
    let claims_dir = resolve_claims_dir_or_err(
        "claim.handoff",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_handoff(
        &claims_dir,
        &StableId(claim_id),
        &StableId(agent_id),
        &summary,
        &evidence_refs,
        resolve_now_unix(now_unix),
    );
    crate::cli_util::emit_envelope_or_err("claim", env, want_json)
}

pub fn run_claim_status(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_status;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    let claims_dir = resolve_claims_dir_or_err(
        "claim.status",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_status(&claims_dir, resolve_now_unix(now_unix));
    crate::cli_util::emit_envelope_or_err("claim", env, want_json)
}

pub fn run_claim_reconcile(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_reconcile_once;

    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut run_loop = false;
    let mut interval_ms: u64 = 30_000;
    let mut max_ticks: Option<u64> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--loop" => run_loop = true,
            "--interval-ms" => {
                idx += 1;
                interval_ms = parse_strict_or_err(
                    &require_value_or_err(args, idx, "interval-ms")?,
                    "interval-ms",
                )?;
            }
            "--max-ticks" => {
                idx += 1;
                max_ticks = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "max-ticks")?,
                    "max-ticks",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--loop] [--interval-ms 30000] [--max-ticks <n>] [--no-json]");
                println!("  One-shot mode is deterministic and materializes stale/expired claim statuses once.");
                println!("  --loop runs a foreground Tokio interval reconciler; missed ticks use Skip and no filesystem watcher/notify is used.");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    if interval_ms == 0 {
        eprintln!("claim reconcile: --interval-ms must be greater than zero");
        return Err(ExitError::invalid_value(
            "claim reconcile: --interval-ms must be greater than zero",
        ));
    }

    let claims_dir = resolve_claims_dir_or_err(
        "claim.reconcile",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    if !run_loop {
        let env = run_reconcile_once(&claims_dir, resolve_now_unix(now_unix));
        crate::cli_util::emit_envelope_or_err("claim", env, want_json)?;
        return Ok(());
    }

    run_claim_reconcile_loop_or_err(ClaimReconcileLoopConfig {
        claims_dir,
        now_unix,
        interval_ms,
        max_ticks,
        want_json,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimReconcileLoopConfig {
    claims_dir: PathBuf,
    now_unix: Option<i64>,
    interval_ms: u64,
    max_ticks: Option<u64>,
    want_json: bool,
}

pub(crate) fn run_claim_reconcile_loop_or_err(
    config: ClaimReconcileLoopConfig,
) -> Result<(), ExitError> {
    use crate::claim::run_reconcile_once;
    use std::time::Duration;
    use tokio::time::{interval_at, Instant, MissedTickBehavior};

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return Err(ExitError::env_config(format!(
                "claim reconcile: cannot build Tokio runtime: {error}"
            )));
        }
    };
    let ClaimReconcileLoopConfig {
        claims_dir,
        now_unix,
        interval_ms,
        max_ticks,
        want_json,
    } = config;
    let exit_code = runtime.block_on(async move {
        let period = Duration::from_millis(interval_ms);
        let mut ticker = interval_at(Instant::now(), period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut ticks = 0_u64;
        loop {
            ticker.tick().await;
            ticks = ticks.saturating_add(1);
            let env = run_reconcile_once(&claims_dir, resolve_now_unix(now_unix));
            let code = env.exit_code();
            if want_json {
                println!("{}", serde_json::to_string(&env).unwrap());
            } else if let Some(data) = env.data.as_ref() {
                eprintln!(
                    "claim.reconcile tick={ticks} scanned={} changed={}",
                    data.scanned, data.changed
                );
            } else if let Some(error) = env.error.as_ref() {
                eprintln!("claim.reconcile tick={ticks} failed: {}", error.message);
            }
            if code != 0 {
                return code;
            }
            if max_ticks.is_some_and(|limit| ticks >= limit) {
                return 0;
            }
        }
    });
    if exit_code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(exit_code, String::new()))
    }
}

pub fn run_claim_check_write(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::run_check_write;
    use forge_core_contracts::StableId;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut agent_id = String::new();
    let mut targets: Vec<String> = Vec::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--agent" => {
                idx += 1;
                agent_id = require_value_or_err(args, idx, "agent")?;
            }
            "--target" => {
                idx += 1;
                targets.push(require_value_or_err(args, idx, "target")?);
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "claims-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--target <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    if agent_id.is_empty() {
        eprintln!("claim check-write: --agent <id> is required");
        return Err(ExitError::invalid_value(
            "claim check-write: --agent <id> is required",
        ));
    }
    if targets.is_empty() {
        eprintln!("claim check-write: at least one --target <path> is required");
        return Err(ExitError::invalid_value(
            "claim check-write: at least one --target <path> is required",
        ));
    }
    let claims_dir = resolve_claims_dir_or_err(
        "check-write",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_check_write(
        &claims_dir,
        &StableId(agent_id),
        &targets,
        resolve_now_unix(now_unix),
    );
    crate::cli_util::emit_envelope_or_err("claim", env, want_json)
}
