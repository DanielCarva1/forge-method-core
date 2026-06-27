//! `claim` command family — the governance surface (slice 4, S4.4).
//!
//! These commands expose the claim-lifecycle engine (S4.3) to host LLMs via the
//! same [`CliEnvelope`] contract as `guide/*` (DD17). An agent DECLARES a claim
//! to reserve a scope, heartbeats to keep it alive, releases to hand back, and
//! reads `status` to see the whole coordination picture (the bus view, DD15).
//!
//! # Coordination bus = filesystem (DD15, DD22)
//!
//! Claims live as `ClaimContractDocument` YAML files in a claims directory
//! (default `contracts/claims`). Each lifecycle transition rewrites the claim's
//! file with its new state; the append-only, tamper-evident integrity spine
//! (layer 3, proven in S0.4) audits every write — we do NOT duplicate layer 3
//! with a parallel claim event-log. The claim file IS the materialized state.
//!
//! # Time is injected (DD23)
//!
//! The engine is pure and deterministic given `now_unix: i64`. The CLI accepts
//! `--now-unix <epoch>` (for replay/tests); the default is real system time.
//! Heartbeat is agent-driven (DC9): the engine never babysits a claim.

use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimContractDocument, ClaimScopeKind,
};
use forge_core_contracts::tool_effect::ConflictCode;
use forge_core_contracts::{
    ClaimId, CliEnvelope, ExitReason, ScopeId, StableId, ENVELOPE_SCHEMA_VERSION,
};
use forge_core_engine::{
    acquire, check_write_against_claims, heartbeat, project_active, release, AcquireRequest,
    ClaimLifecycleDecision, ClaimRejection,
};
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

/// Failure payload for `claim acquire|heartbeat|release` when the engine
/// refuses. Carries a machine-readable `reject_code` so the host self-corrects
/// (R2 parity with `guide decide`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClaimRejected {
    /// One of: `already_claimed_by_other` | `not_claimant` |
    /// `expired_requires_handoff` | `illegal_transition` | `claim_not_found`.
    pub reject_code: String,
    pub detail: String,
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
            if let Err(e) = save_claim(claims_dir, &claim) {
                return CliEnvelope::err(
                    "claim.acquire",
                    ExitReason::EnvConfig,
                    format!("cannot persist claim: {e}"),
                );
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
            if let Err(e) = save_claim(claims_dir, &updated) {
                return CliEnvelope::err(
                    "claim.heartbeat",
                    ExitReason::EnvConfig,
                    format!("cannot persist claim: {e}"),
                );
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
            if let Err(e) = save_claim(claims_dir, &updated) {
                return CliEnvelope::err(
                    "claim.release",
                    ExitReason::EnvConfig,
                    format!("cannot persist claim: {e}"),
                );
            }
            CliEnvelope::ok("claim.release", ClaimResult::from_contract(&updated))
        }
        ClaimLifecycleDecision::Rejected(reason) => rejected_envelope("claim.release", &reason),
    }
}

/// Run `claim status` — the coordination-bus view: every claim that is live
/// right now (who holds what, since when, expires when).
#[must_use]
pub fn run_status(
    claims_dir: &Path,
    now_unix: i64,
) -> CliEnvelope<forge_core_engine::ActiveClaimsView> {
    let (existing, errs) = load_claims(claims_dir);
    if let Some(env) = env_config_if_errors(claims_dir, &errs) {
        return env;
    }
    let view = project_active(&existing, now_unix);
    CliEnvelope::ok("claim.status", view)
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
/// - allowed + no blocks  -> write is clear (either inside the writer's own
///   claim, or ungoverned).
/// - blocked              -> exit 2 (RejectedByGate); the writer must acquire
///   its own scope or wait for a handoff. The `blocks` array tells it exactly
///   which paths collide and who owns them.
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
        } => CliEnvelope::ok(
            "check-write",
            WriteCheckPayload {
                writer: writer_agent_id.0.clone(),
                allowed: true,
                governed_by_self: governed_by_self.into_iter().map(|p| p.0).collect(),
                ungoverned: ungoverned.into_iter().map(|p| p.0).collect(),
                blocks: Vec::new(),
            },
        ),
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
                msg.push_str(&format!(
                    "  - {} (claimed by '{}' via claim '{}'; conflict: {})\n",
                    b.blocked_path.0,
                    b.claimant.0,
                    b.blocking_claim_id.0,
                    conflict_code_str(b.conflict_code)
                ));
            }
            msg.push_str("acquire your own scope or wait for a handoff.");
            CliEnvelope::reject("check-write", ExitReason::RejectedByGate, msg, payload)
        }
    }
}

/// Map a [`ConflictCode`] to its stable snake_case string for the
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

/// Load every `*.yaml` claim document in `dir`. Returns the contracts and a
/// list of per-file errors (a malformed claim file is never silently dropped
/// — it would corrupt authority, so it surfaces as `EnvConfig`).
#[must_use]
pub fn load_claims(dir: &Path) -> (Vec<ClaimContract>, Vec<String>) {
    let mut claims = Vec::new();
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        // A missing dir is a fresh/empty claims bus — not an error.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (claims, errors),
        // Permission/IO errors MUST surface: an unreadable dir would blind
        // the engine to existing claims and break authority.
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
    paths.sort(); // deterministic load order
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            errors.push(format!("{}: unreadable", path.display()));
            continue;
        };
        match serde_yaml::from_str::<ClaimContractDocument>(&text) {
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
    let yaml = serde_yaml::to_string(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = dir.join(format!("{}.yaml", slug_for_file(&claim.id.0)));
    // Atomic write: temp file in the same dir, then rename over the target.
    // Prevents a crash mid-write from leaving a truncated YAML that would DoS
    // the whole claims dir on the next load (review S4.4 bug #3).
    crate::io_util::atomic_write(&path, &yaml)?;
    Ok(path)
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
        detail: format!("{reason:?}"),
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
        ClaimRejection::NotClaimant { .. } => "not_claimant",
        ClaimRejection::ExpiredRequiresHandoff { .. } => "expired_requires_handoff",
        ClaimRejection::IllegalTransition { .. } => "illegal_transition",
        ClaimRejection::InvalidRequest { .. } => "invalid_request",
    }
    .into()
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
        // file was written
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
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
        let ids: Vec<&str> = view.active.iter().map(|c| c.scope_id.as_str()).collect();
        assert!(ids.contains(&"s1"));
        assert!(ids.contains(&"s2"));
    }

    #[test]
    fn status_excludes_expired_and_released() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0); // will be expired at T0 + 9999
        let _ = run_acquire(&dir, &req("s2", "agentB"), T0);
        let env = run_status(&dir, T0 + 9_999); // s1 lease (ttl 600) is long past
        let view = env.data.as_ref().unwrap();
        assert_eq!(view.active.len(), 0, "both past ttl at T0+9999");
    }

    #[test]
    fn status_serializes_to_json_cleanly() {
        let dir = tempfile_dir();
        let _ = run_acquire(&dir, &req("s1", "agentA"), T0);
        let env = run_status(&dir, T0);
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"active\""));
        assert!(json.contains("\"scope_id\":\"s1\""));
    }

    // --- env / persistence ---

    #[test]
    fn status_on_missing_dir_is_ok_empty() {
        let env = run_status(std::path::Path::new("/nonexistent/forge/nope"), T0);
        assert!(env.ok, "{:?}", env.error);
        assert!(env.data.as_ref().unwrap().active.is_empty());
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
}
