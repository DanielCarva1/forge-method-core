//! Claim lifecycle engine — the semantic core that makes claims MEAN something.
//!
//! The contract model ([`ClaimContract`]) is rich but inert without this module.
//! Here we define the typed state machine (DD16):
//!
//! ```text
//!   acquire ──▶ Active ──(heartbeat)──▶ Active
//!                  │
//!                  ├──(now > expires_at)──▶ Expired ──▶ HandoffRequired*
//!                  │
//!                  └──(release by claimant)──▶ Released
//! ```
//!
//! Like [`crate::phase_transition`], the engine is **pure**: it takes data plus
//! an injected `now` (epoch seconds) and returns a typed verdict. IO (reading /
//! writing claim files, appending the WAL) is the CLI's job (S4.4). This keeps
//! the engine deterministic and unit-testable.
//!
//! `now` is injected as `i64` epoch seconds so tests are reproducible and the
//! engine never touches a clock (DC9: the engine does not babysit time).

use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};

// ---------------------------------------------------------------------------
// timestamp helpers — minimal RFC3339 (UTC "Z") <-> unix epoch, no deps.
// Correct for civil dates 1970-03..2100. Format: "YYYY-MM-DDTHH:MM:SSZ".
// ---------------------------------------------------------------------------

/// Parse a UTC RFC3339 timestamp of the form `YYYY-MM-DDTHH:MM:SSZ` to unix
/// seconds. Returns `None` on any deviation from the fixed shape.
///
/// Uses the well-known civil-from-days algorithm (Howard Hinnant) so it is
/// correct across leap years without pulling in a date crate.
#[must_use]
pub fn rfc3339_to_unix(ts: &str) -> Option<i64> {
    let bytes = ts.as_bytes();
    if bytes.len() != 20 || bytes[19] != b'Z' {
        return None;
    }
    let y: i64 = ts.get(0..4)?.parse().ok()?;
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let mo: i64 = ts.get(5..7)?.parse().ok()?;
    let d: i64 = ts.get(8..10)?.parse().ok()?;
    let h: i64 = ts.get(11..13)?.parse().ok()?;
    let mi: i64 = ts.get(14..16)?.parse().ok()?;
    let s: i64 = ts.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mo) {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// Format unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn unix_to_rfc3339(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let h = secs / 3600;
    let mi = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Days since unix epoch (1970-01-01) for a civil date. Hinnant's algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of `days_from_civil`. Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// request / decision types
// ---------------------------------------------------------------------------

/// A request to acquire a new claim. The engine turns this into a [`ClaimContract`]
/// if the scope is free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireRequest {
    pub scope_kind: ClaimScopeKind,
    pub scope_id: ScopeId,
    pub agent_id: StableId,
    pub role: ActorRole,
    /// Lease time-to-live in seconds. After this elapses without heartbeat the
    /// claim is expirable.
    pub ttl_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub paths: Vec<RepoPath>,
    pub product_area: Option<StableId>,
    /// State version the claimant expects to build on (optimistic concurrency).
    pub expected_state_version: Option<u64>,
}

impl AcquireRequest {
    fn default_kind(&self) -> ClaimKind {
        match self.scope_kind {
            ClaimScopeKind::Story => ClaimKind::Story,
            ClaimScopeKind::Lane => ClaimKind::Lane,
            ClaimScopeKind::ProductArea => ClaimKind::ProductArea,
            ClaimScopeKind::Integration | ClaimScopeKind::IntegrationState => {
                ClaimKind::Integration
            }
            ClaimScopeKind::Project => ClaimKind::Driver,
        }
    }
}

/// The engine's verdict on a claim-lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // accepted carries the full contract; rejection is light — rare, not hot
pub enum ClaimLifecycleDecision {
    /// The operation is allowed; carries the resulting (possibly new) claim.
    Accepted(ClaimContract),
    /// The operation is refused, with a typed reason the host can self-correct from.
    Rejected(ClaimRejection),
}

/// Typed reasons a claim operation is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // rejection is a rare error path; not hot data
pub enum ClaimRejection {
    /// The scope is already held live by a different agent.
    AlreadyClaimedByOther {
        scope_id: ScopeId,
        holder: StableId,
        expires_at: String,
    },
    /// The caller is not the claimant of this claim.
    NotClaimant {
        claim_id: ClaimId,
        claimant: StableId,
        requested_by: StableId,
    },
    /// The claim has expired; expiry policy requires a handoff, not a silent op.
    ExpiredRequiresHandoff { claim_id: ClaimId },
    /// The requested status transition is not in the allowed set.
    IllegalTransition {
        claim_id: ClaimId,
        from: ClaimStatus,
        to: ClaimStatus,
    },
}

/// A compact, serializable summary of one active claim — the "bus view".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveClaimSummary {
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub agent_id: String,
    pub role: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub status: String,
    pub paths: Vec<String>,
}

/// The coordination-bus view: every claim that is live right now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveClaimsView {
    pub active: Vec<ActiveClaimSummary>,
}

/// Outcome of an expiry sweep on one claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimExpiry {
    pub claim_id: ClaimId,
    pub transitioned_to: ClaimStatus,
}

// ---------------------------------------------------------------------------
// predicates
// ---------------------------------------------------------------------------

/// True if a claim is currently live (held, not released, not yet expired).
#[must_use]
pub fn is_live(claim: &ClaimContract, now_unix: i64) -> bool {
    matches!(claim.status.value, ClaimStatus::Active | ClaimStatus::Stale)
        && !is_expired(claim, now_unix)
}

/// True if the lease's `expires_at` has passed at `now_unix`.
///
/// **Fail-closed:** a claim whose `expires_at` is unparseable is treated as
/// expired. A corrupt lease is never treated as live — it must route to
/// handoff/recovery, not be silently reused (design rule 2).
#[must_use]
pub fn is_expired(claim: &ClaimContract, now_unix: i64) -> bool {
    match rfc3339_to_unix(&claim.lease.expires_at) {
        Some(exp) => now_unix >= exp,
        None => true,
    }
}

/// True if this claim holds the given scope.
#[must_use]
pub fn claim_holds_scope(
    claim: &ClaimContract,
    scope_kind: ClaimScopeKind,
    scope_id: &ScopeId,
) -> bool {
    claim.scope.kind == scope_kind && claim.scope.id == *scope_id
}

// ---------------------------------------------------------------------------
// operations
// ---------------------------------------------------------------------------

/// Acquire a new claim for `req.scope`.
///
/// Rejection rules (closing the lifecycle soundness gaps):
/// - A **live** claim by *any* agent (including the same agent) blocks acquire —
///   the claimant must heartbeat/renew, not open a second authority.
/// - An **expired but still-open** claim (status Active/Stale past its lease) whose
///   policy requires handoff is NOT silently re-acquirable — it returns
///   [`ClaimRejection::ExpiredRequiresHandoff`] (design rule 2: expired claims
///   route to recovery, never silent release).
/// - A scope whose only prior claims are Released / Expired (no handoff) /
///   `HandoffRecorded` is free to acquire.
///
/// # Errors
/// Returns a typed [`ClaimRejection`] when the scope cannot be cleanly acquired.
#[must_use]
pub fn acquire(
    active: &[ClaimContract],
    req: &AcquireRequest,
    now_unix: i64,
) -> ClaimLifecycleDecision {
    for c in active {
        if !claim_holds_scope(c, req.scope_kind, &req.scope_id) {
            continue;
        }
        if is_live(c, now_unix) {
            // Live by anyone (including the same agent) blocks a fresh acquire;
            // same-agent continuation is heartbeat, not a second authority.
            return ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
                scope_id: req.scope_id.clone(),
                holder: c.claim.claimant_agent_id.clone(),
                expires_at: c.lease.expires_at.clone(),
            });
        }
        // Not live. If it is still *open* (Active/Stale) yet past its lease, and
        // the policy mandates handoff, the scope is NOT free — it needs recovery.
        if matches!(c.status.value, ClaimStatus::Active | ClaimStatus::Stale)
            && is_expired(c, now_unix)
            && c.expiry_policy.handoff_required
        {
            return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
                claim_id: c.id.clone(),
            });
        }
    }

    let now = unix_to_rfc3339(now_unix);
    let expires_unix = now_unix + i64::try_from(req.ttl_seconds).unwrap_or(i64::MAX);
    let claim = ClaimContract {
        id: ClaimId(format!(
            "claim.{}.{}.{}",
            scope_kind_slug(req.scope_kind),
            req.scope_id.0,
            req.scope_id.0 // disambiguator: scope id is unique enough for v0
        )),
        contract_ref: RepoPath("contracts/claims/claim-contract-v0.yaml".into()),
        claim: ClaimIdentity {
            kind: req.default_kind(),
            claimant_agent_id: req.agent_id.clone(),
            claimant_role: req.role,
            registry_ref: None,
        },
        scope: ClaimScope {
            kind: req.scope_kind,
            id: req.scope_id.clone(),
            product_area: req.product_area.clone(),
            paths: req.paths.clone(),
        },
        lease: ClaimLease {
            acquired_at: now.clone(),
            last_heartbeat_at: now.clone(),
            expires_at: unix_to_rfc3339(expires_unix),
            ttl_seconds: req.ttl_seconds,
            heartbeat_interval_seconds: req.heartbeat_interval_seconds,
            expected_state_version: req.expected_state_version.unwrap_or(0),
        },
        status: ClaimStatusRecord {
            value: ClaimStatus::Active,
            evaluated_at: now,
            reason_code: None,
        },
        expiry_policy: default_expiry_policy(),
        evidence_refs: Vec::new(),
    };
    ClaimLifecycleDecision::Accepted(claim)
}

/// Refresh a claim's heartbeat and extend its lease. Only the claimant may
/// heartbeat, and only while the claim is still live.
#[must_use]
pub fn heartbeat(
    claim: &ClaimContract,
    agent_id: &StableId,
    now_unix: i64,
) -> ClaimLifecycleDecision {
    if claim.claim.claimant_agent_id != *agent_id {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::NotClaimant {
            claim_id: claim.id.clone(),
            claimant: claim.claim.claimant_agent_id.clone(),
            requested_by: agent_id.clone(),
        });
    }
    // Only Active/Stale claims may be heartbeated. Released/Expired/
    // Handoff* claims cannot be resurrected (lifecycle authority is append-only).
    if !matches!(claim.status.value, ClaimStatus::Active | ClaimStatus::Stale) {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition {
            claim_id: claim.id.clone(),
            from: claim.status.value,
            to: ClaimStatus::Active,
        });
    }
    if is_expired(claim, now_unix) {
        // Expired (or corrupt-lease) claims need handoff, not a silent re-heartbeat.
        return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
            claim_id: claim.id.clone(),
        });
    }
    let mut next = claim.clone();
    let expires_unix = now_unix + i64::try_from(claim.lease.ttl_seconds).unwrap_or(i64::MAX);
    let now = unix_to_rfc3339(now_unix);
    next.lease.last_heartbeat_at = now;
    next.lease.expires_at = unix_to_rfc3339(expires_unix);
    next.status = ClaimStatusRecord {
        value: ClaimStatus::Active,
        evaluated_at: unix_to_rfc3339(now_unix),
        reason_code: None,
    };
    ClaimLifecycleDecision::Accepted(next)
}

/// Release a claim. Only the claimant may release, and only while the claim
/// is still open (Active/Stale). A claim whose lease has expired must NOT be
/// released when its policy mandates a handoff — doing so would silently drop
/// coordination evidence (design rule 2).
#[must_use]
pub fn release(
    claim: &ClaimContract,
    agent_id: &StableId,
    now_unix: i64,
) -> ClaimLifecycleDecision {
    if claim.claim.claimant_agent_id != *agent_id {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::NotClaimant {
            claim_id: claim.id.clone(),
            claimant: claim.claim.claimant_agent_id.clone(),
            requested_by: agent_id.clone(),
        });
    }
    if !matches!(claim.status.value, ClaimStatus::Active | ClaimStatus::Stale) {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition {
            claim_id: claim.id.clone(),
            from: claim.status.value,
            to: ClaimStatus::Released,
        });
    }
    // Expired claims that require handoff cannot be silently released.
    if is_expired(claim, now_unix)
        && (claim.expiry_policy.handoff_required
            || !claim.expiry_policy.release_without_handoff_allowed)
    {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
            claim_id: claim.id.clone(),
        });
    }
    let now = unix_to_rfc3339(now_unix);
    let mut next = claim.clone();
    next.status = ClaimStatusRecord {
        value: ClaimStatus::Released,
        evaluated_at: now,
        reason_code: Some(StableId("released_by_claimant".into())),
    };
    ClaimLifecycleDecision::Accepted(next)
}

/// Sweep claims for expiry. Any live claim whose lease has passed becomes
/// `Expired` (or `HandoffRequired` when the policy mandates handoff).
#[must_use]
pub fn expire_stale(claims: &[ClaimContract], now_unix: i64) -> Vec<ClaimExpiry> {
    claims
        .iter()
        .filter(|c| {
            matches!(c.status.value, ClaimStatus::Active | ClaimStatus::Stale)
                && is_expired(c, now_unix)
        })
        .map(|c| ClaimExpiry {
            claim_id: c.id.clone(),
            transitioned_to: if c.expiry_policy.handoff_required {
                ClaimStatus::HandoffRequired
            } else {
                ClaimStatus::Expired
            },
        })
        .collect()
}

/// Project the coordination-bus view: every claim that is live right now.
#[must_use]
pub fn project_active(claims: &[ClaimContract], now_unix: i64) -> ActiveClaimsView {
    let active = claims
        .iter()
        .filter(|c| is_live(c, now_unix))
        .map(|c| ActiveClaimSummary {
            claim_id: c.id.0.clone(),
            scope_kind: scope_kind_slug(c.scope.kind),
            scope_id: c.scope.id.0.clone(),
            agent_id: c.claim.claimant_agent_id.0.clone(),
            role: actor_role_slug(c.claim.claimant_role),
            acquired_at: c.lease.acquired_at.clone(),
            expires_at: c.lease.expires_at.clone(),
            status: status_slug(c.status.value),
            paths: c.scope.paths.iter().map(|p| p.0.clone()).collect(),
        })
        .collect();
    ActiveClaimsView { active }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn default_expiry_policy() -> ExpiryPolicy {
    ExpiryPolicy {
        on_expiry: ExpiryAction::RecordHandoffRequest,
        handoff_required: true,
        release_without_handoff_allowed: false,
        reclaim_policy: ReclaimPolicy::DriverReview,
        handoff_request_ref: None,
    }
}

fn scope_kind_slug(k: ClaimScopeKind) -> String {
    match k {
        ClaimScopeKind::Project => "project",
        ClaimScopeKind::ProductArea => "product_area",
        ClaimScopeKind::Story => "story",
        ClaimScopeKind::Lane => "lane",
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

fn status_slug(s: ClaimStatus) -> String {
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

    const T0: i64 = 1_779_000_000; // 2026-06-25T00:00:00Z-ish, arbitrary fixed point

    fn req(scope_id: &str, agent: &str) -> AcquireRequest {
        AcquireRequest {
            scope_kind: ClaimScopeKind::Story,
            scope_id: ScopeId(scope_id.into()),
            agent_id: StableId(agent.into()),
            role: ActorRole::Worker,
            ttl_seconds: 600,
            heartbeat_interval_seconds: 120,
            paths: vec![RepoPath(format!("crates/x-{scope_id}"))],
            product_area: None,
            expected_state_version: Some(1),
        }
    }

    fn manual_claim(
        scope_id: &str,
        agent: &str,
        expires_unix: i64,
        status: ClaimStatus,
    ) -> ClaimContract {
        let ClaimLifecycleDecision::Accepted(mut c) = acquire(&[], &req(scope_id, agent), T0)
        else {
            unreachable!("fixture acquire must succeed on empty scope");
        };
        c.lease.expires_at = unix_to_rfc3339(expires_unix);
        c.status.value = status;
        c
    }

    #[test]
    fn timestamp_roundtrip() {
        for ts in [
            "2026-06-25T00:00:00Z",
            "2026-06-25T00:04:00Z",
            "2026-06-26T12:34:56Z",
        ] {
            let unix = rfc3339_to_unix(ts).unwrap();
            assert_eq!(unix_to_rfc3339(unix), ts, "roundtrip {ts}");
        }
    }

    #[test]
    fn timestamp_rejects_bad_shape() {
        assert!(rfc3339_to_unix("2026-6-5T0:0:0Z").is_none());
        assert!(rfc3339_to_unix("not-a-date").is_none());
        assert!(rfc3339_to_unix("2026-13-01T00:00:00Z").is_none());
    }

    #[test]
    fn acquire_succeeds_on_free_scope() {
        let d = acquire(&[], &req("s1", "agentA"), T0);
        let ClaimLifecycleDecision::Accepted(c) = d else {
            panic!("should accept: {d:?}");
        };
        assert_eq!(c.status.value, ClaimStatus::Active);
        assert_eq!(c.claim.claimant_agent_id.0, "agentA");
        assert_eq!(c.scope.id.0, "s1");
        // lease end = T0 + 600s
        assert_eq!(rfc3339_to_unix(&c.lease.expires_at), Some(T0 + 600));
    }

    #[test]
    fn acquire_rejects_when_scope_held_by_other() {
        let holder = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let d = acquire(&[holder], &req("s1", "agentB"), T0);
        match d {
            ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
                holder,
                ..
            }) => {
                assert_eq!(holder.0, "agentA");
            }
            other => panic!("expected AlreadyClaimedByOther, got {other:?}"),
        }
    }

    #[test]
    fn acquire_rejects_same_agent_duplicate_while_live() {
        // A live claim blocks acquire even for the SAME agent — the claimant
        // must heartbeat/renew, not open a second authority (review bug #2).
        let holder = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let d = acquire(&[holder], &req("s1", "agentA"), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther { .. })
        ));
    }

    #[test]
    fn acquire_rejects_expired_open_claim_requiring_handoff() {
        // An Active claim whose lease is past AND whose policy requires handoff
        // must NOT be silently re-acquired — design rule 2 (review bug #1).
        let expired = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        let d = acquire(&[expired], &req("s1", "agentB"), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));
    }

    #[test]
    fn heartbeat_rejects_non_claimant() {
        let c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let d = heartbeat(&c, &StableId("agentB".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::NotClaimant { .. })
        ));
    }

    #[test]
    fn heartbeat_extends_lease() {
        let c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let later = T0 + 300;
        let d = heartbeat(&c, &StableId("agentA".into()), later);
        let ClaimLifecycleDecision::Accepted(next) = d else {
            panic!("should accept: {d:?}");
        };
        // lease extended by ttl from heartbeat time
        assert_eq!(rfc3339_to_unix(&next.lease.expires_at), Some(later + 600));
        assert_eq!(next.status.value, ClaimStatus::Active);
    }

    #[test]
    fn heartbeat_rejects_expired_claim() {
        let expired = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        let d = heartbeat(&expired, &StableId("agentA".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));
    }

    #[test]
    fn release_only_by_claimant() {
        let c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        assert!(matches!(
            release(&c, &StableId("agentB".into()), T0),
            ClaimLifecycleDecision::Rejected(ClaimRejection::NotClaimant { .. })
        ));
        let ClaimLifecycleDecision::Accepted(released) =
            release(&c, &StableId("agentA".into()), T0)
        else {
            panic!();
        };
        assert_eq!(released.status.value, ClaimStatus::Released);
    }

    #[test]
    fn expire_sweeps_past_due_claims() {
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let due = manual_claim("s2", "agentB", T0 - 1, ClaimStatus::Active);
        let expiries = expire_stale(&[live, due], T0);
        assert_eq!(expiries.len(), 1);
        assert_eq!(expiries[0].claim_id.0, "claim.story.s2.s2");
        // default policy mandates handoff
        assert_eq!(expiries[0].transitioned_to, ClaimStatus::HandoffRequired);
    }

    #[test]
    fn project_active_lists_only_live_claims() {
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let expired = manual_claim("s2", "agentB", T0 - 1, ClaimStatus::Active);
        let released = {
            let mut c = manual_claim("s3", "agentC", T0 + 600, ClaimStatus::Active);
            c.status.value = ClaimStatus::Released;
            c
        };
        let view = project_active(&[live, expired, released], T0);
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].scope_id, "s1");
        assert_eq!(view.active[0].agent_id, "agentA");
    }

    #[test]
    fn project_active_is_serializable_bus_view() {
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let view = project_active(&[live], T0);
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"scope_id\":\"s1\""));
        assert!(json.contains("\"agent_id\":\"agentA\""));
        assert!(json.contains("\"status\":\"active\""));
    }

    // --- regression tests for the gpt-5.5 adversarial review (S4.3 v2) ---

    #[test]
    fn heartbeat_cannot_resurrect_released_claim() {
        // review bug #3: heartbeat must reject Released/Handoff* claims.
        let mut c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        c.status.value = ClaimStatus::Released;
        let d = heartbeat(&c, &StableId("agentA".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition { to, .. }) if to == ClaimStatus::Active
        ));
    }

    #[test]
    fn release_blocked_when_expired_and_handoff_required() {
        // review bug #4: cannot silently release an expired claim that mandates handoff.
        let expired = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        let d = release(&expired, &StableId("agentA".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));
    }

    #[test]
    fn release_allowed_for_unexpired_claim() {
        // happy path still works: an in-lease Active claim is releasable.
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let d = release(&live, &StableId("agentA".into()), T0);
        assert!(matches!(d, ClaimLifecycleDecision::Accepted(_)));
    }

    #[test]
    fn is_expired_fail_closed_on_corrupt_timestamp() {
        // review bug #5: an unparseable lease must NOT be treated as live.
        let mut c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        c.lease.expires_at = "not-a-timestamp".into();
        assert!(
            is_expired(&c, T0),
            "corrupt lease must be treated as expired"
        );
        assert!(!is_live(&c, T0), "corrupt lease must not be live");
        // and acquire must route it to handoff recovery, not silently reuse:
        let d = acquire(&[c], &req("s1", "agentB"), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));
    }

    #[test]
    fn is_expired_boundary_is_inclusive_at_expires_at() {
        // review v2 nit: pin the exact boundary contract. `>=` means the
        // instant a lease hits its expires_at it is already expired.
        let mut c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        c.lease.expires_at = unix_to_rfc3339(T0 + 600);
        assert!(
            is_expired(&c, T0 + 600),
            "now == expires_at must be expired"
        );
        assert!(!is_expired(&c, T0 + 599), "now < expires_at must be live");
    }

    #[test]
    fn release_allowed_when_expired_but_no_handoff_required() {
        // reclaim_policy is a DRIVER/HUMAN governance decision (DC9: the engine
        // does not babysit). The engine only enforces NON-DELEGABLE rules.
        // When handoff is not required AND release-without-handoff is allowed,
        // an expired claim may be released (no silent loss of coordination).
        let mut c = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        c.expiry_policy.handoff_required = false;
        c.expiry_policy.release_without_handoff_allowed = true;
        let d = release(&c, &StableId("agentA".into()), T0);
        assert!(matches!(d, ClaimLifecycleDecision::Accepted(_)));
    }

    #[test]
    fn acquire_frees_scope_when_prior_claim_was_released() {
        // A Released claim (clean handoff/normal close) leaves the scope free.
        let mut prior = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        prior.status.value = ClaimStatus::Released;
        let d = acquire(&[prior], &req("s1", "agentB"), T0);
        assert!(matches!(d, ClaimLifecycleDecision::Accepted(_)));
    }
}
