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
use forge_core_contracts::{
    ClaimId, ConflictContract, ConflictDetectionReason, ConflictResolutionState, IntentScope,
    IntentScopeKind, PrincipalId, RepoPath, ScopeId, StableId,
};

use crate::conflict_detection::repo_paths_overlap;

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
    let max_day = days_in_month(y, mo)?;
    if !(1..=max_day).contains(&d)
        || !(0..=23).contains(&h)
        || !(0..=59).contains(&mi)
        || !(0..=59).contains(&s)
    {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

fn days_in_month(year: i64, month: i64) -> Option<i64> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
    pub principal_id: Option<PrincipalId>,
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

/// A request to record the required handoff for an expired claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHandoffRequest {
    pub recorder_agent_id: StableId,
    pub summary: String,
    pub evidence_refs: Vec<String>,
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
// Both variants can be large: Accepted carries the full ClaimContract, and
// Rejected now carries an optional ConflictContract (F07.4). Rejection is a
// rare error path, not hot data, so the size is acceptable.
#[allow(clippy::large_enum_variant, clippy::result_large_err)]
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
        /// F07.4 — a structured conflict object, populated when the holder is a
        /// *distinct* principal from the requester (a real A↔B conflict). `None`
        /// when the same agent re-acquires (continuation, not conflict — the
        /// validator's `GovernanceConflictPartiesNotDistinct` rule). The flat
        /// rejection fields above remain for backward compatibility; this is the
        /// first-class [`ConflictContract`] ADR-0007 mandates.
        conflict: Option<ConflictContract>,
    },
    /// A requested path is already covered by a live claim.
    PathAlreadyClaimed {
        path: RepoPath,
        blocking_claim_id: ClaimId,
        holder: StableId,
        expires_at: String,
        /// F07.4 — structured conflict object (see [`ClaimRejection::AlreadyClaimedByOther`]).
        conflict: Option<ConflictContract>,
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
    /// The request payload is structurally valid Rust data but violates an
    /// engine invariant (for example, a lease TTL that cannot be represented).
    InvalidRequest {
        field: &'static str,
        message: String,
    },
}

/// A compact, serializable summary of one active claim — the "bus view".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveClaimSummary {
    pub claim_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
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

/// Deterministic report for one reconcile pass over the claim bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimReconcileReport {
    pub scanned: usize,
    pub transitions: Vec<ClaimReconcileTransition>,
}

/// One materializable lifecycle transition selected by reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimReconcileTransition {
    pub claim_id: ClaimId,
    pub from: ClaimStatus,
    pub to: ClaimStatus,
    pub reason_code: StableId,
    pub updated: ClaimContract,
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
/// - A **live** claim whose paths overlap the requested paths also blocks
///   acquire, even when the scope id differs. A repo path has one live owner.
/// - An **expired but still-open** claim (status Active/Stale past its lease) whose
///   policy requires handoff is NOT silently re-acquirable — it returns
///   [`ClaimRejection::ExpiredRequiresHandoff`] (design rule 2: expired claims
///   route to recovery, never silent release).
/// - A materialized `HandoffRequired` claim blocks acquire until a handoff is
///   recorded.
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
            let holder = c.claim.claimant_agent_id.clone();
            return ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
                scope_id: req.scope_id.clone(),
                holder: holder.clone(),
                expires_at: c.lease.expires_at.clone(),
                // F07.4: emit a structured ConflictContract only when the holder
                // is a distinct principal from the requester (a real A↔B conflict).
                conflict: build_conflict(
                    ConflictActor::new(req.principal_id.as_ref(), &req.agent_id),
                    ConflictActor::new(c.claim.claimant_principal_id.as_ref(), &holder),
                    IntentScopeKind::Project,
                    req.scope_id.0.as_str(),
                    ConflictDetectionReason::AuthorityScopeOverlap,
                    now_unix,
                ),
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
        if c.status.value == ClaimStatus::HandoffRequired {
            return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
                claim_id: c.id.clone(),
            });
        }
    }

    for c in active {
        let Some(path) = first_overlapping_path(&req.paths, &c.scope.paths) else {
            continue;
        };
        if is_live(c, now_unix) {
            let holder = c.claim.claimant_agent_id.clone();
            // F07.4: emit a structured ConflictContract only when the holder is a
            // distinct principal from the requester. The contested scope is the
            // overlapping path prefix (PathOverlap detection reason).
            let conflict = build_conflict(
                ConflictActor::new(req.principal_id.as_ref(), &req.agent_id),
                ConflictActor::new(c.claim.claimant_principal_id.as_ref(), &holder),
                IntentScopeKind::PathPrefix,
                path.0.as_str(),
                ConflictDetectionReason::PathOverlap,
                now_unix,
            );
            return ClaimLifecycleDecision::Rejected(ClaimRejection::PathAlreadyClaimed {
                path,
                blocking_claim_id: c.id.clone(),
                holder,
                expires_at: c.lease.expires_at.clone(),
                conflict,
            });
        }
        if matches!(c.status.value, ClaimStatus::Active | ClaimStatus::Stale)
            && is_expired(c, now_unix)
            && c.expiry_policy.handoff_required
        {
            return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
                claim_id: c.id.clone(),
            });
        }
        if c.status.value == ClaimStatus::HandoffRequired {
            return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
                claim_id: c.id.clone(),
            });
        }
    }

    let expires_unix = match checked_lease_expiry(now_unix, req.ttl_seconds, "ttl_seconds") {
        Ok(expires_unix) => expires_unix,
        Err(rejection) => return ClaimLifecycleDecision::Rejected(rejection),
    };
    let now = unix_to_rfc3339(now_unix);
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
            claimant_principal_id: req.principal_id.clone(),
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
    if claim.status.value == ClaimStatus::HandoffRequired {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
            claim_id: claim.id.clone(),
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
    let expires_unix =
        match checked_lease_expiry(now_unix, claim.lease.ttl_seconds, "lease.ttl_seconds") {
            Ok(expires_unix) => expires_unix,
            Err(rejection) => return ClaimLifecycleDecision::Rejected(rejection),
        };
    let mut next = claim.clone();
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
    if claim.status.value == ClaimStatus::HandoffRequired {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff {
            claim_id: claim.id.clone(),
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

/// Record the required handoff for an expired claim.
///
/// This is the official recovery edge for claims that intentionally block
/// `heartbeat`, `release`, and overlapping `acquire` after lease expiry. The
/// operation does not resurrect or silently release the old authority; it marks
/// the old claim `HandoffRecorded` and lets a later acquire create fresh
/// authority for the scope.
#[must_use]
pub fn record_handoff(
    claim: &ClaimContract,
    req: &RecordHandoffRequest,
    now_unix: i64,
) -> ClaimLifecycleDecision {
    if req.summary.trim().is_empty() {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest {
            field: "summary",
            message: "handoff summary is required".into(),
        });
    }
    if !claim.expiry_policy.handoff_required {
        return ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition {
            claim_id: claim.id.clone(),
            from: claim.status.value,
            to: ClaimStatus::HandoffRecorded,
        });
    }

    match claim.status.value {
        ClaimStatus::Active | ClaimStatus::Stale => {
            if !is_expired(claim, now_unix) {
                return ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition {
                    claim_id: claim.id.clone(),
                    from: claim.status.value,
                    to: ClaimStatus::HandoffRecorded,
                });
            }
        }
        ClaimStatus::HandoffRequired => {}
        _ => {
            return ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition {
                claim_id: claim.id.clone(),
                from: claim.status.value,
                to: ClaimStatus::HandoffRecorded,
            });
        }
    }

    let mut next = claim.clone();
    next.status = ClaimStatusRecord {
        value: ClaimStatus::HandoffRecorded,
        evaluated_at: unix_to_rfc3339(now_unix),
        reason_code: Some(StableId(format!(
            "handoff_recorded_by_{}",
            req.recorder_agent_id.0
        ))),
    };
    for evidence_ref in &req.evidence_refs {
        if !next.evidence_refs.contains(evidence_ref) {
            next.evidence_refs.push(evidence_ref.clone());
        }
    }
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

/// Reconcile open claim lifecycle state at `now_unix`.
///
/// This is pure and deterministic: it never reads a clock and never writes the
/// filesystem. Hosts persist the returned transitions (normally to the claim
/// WAL, then the YAML compatibility cache).
///
/// Rules:
/// - `Active|Stale` claims whose lease has expired become
///   `HandoffRequired` when policy mandates handoff, otherwise `Expired`.
/// - `Active` claims whose heartbeat is overdue but whose lease has not expired
///   become `Stale`.
/// - Terminal/recovery states are left unchanged.
/// - An unparseable `last_heartbeat_at` is fail-closed to `Stale` while the
///   lease is still representably unexpired; an unparseable `expires_at`
///   remains fail-closed through [`is_expired`].
#[must_use]
pub fn reconcile_claims(claims: &[ClaimContract], now_unix: i64) -> ClaimReconcileReport {
    let transitions = claims
        .iter()
        .filter_map(|claim| reconcile_claim(claim, now_unix))
        .collect();
    ClaimReconcileReport {
        scanned: claims.len(),
        transitions,
    }
}

fn reconcile_claim(claim: &ClaimContract, now_unix: i64) -> Option<ClaimReconcileTransition> {
    match claim.status.value {
        ClaimStatus::Active | ClaimStatus::Stale if is_expired(claim, now_unix) => {
            let to = if claim.expiry_policy.handoff_required {
                ClaimStatus::HandoffRequired
            } else {
                ClaimStatus::Expired
            };
            Some(reconciled_transition(
                claim,
                now_unix,
                to,
                StableId("lease_expired".into()),
            ))
        }
        ClaimStatus::Active => heartbeat_stale_reason(claim, now_unix)
            .map(|reason| reconciled_transition(claim, now_unix, ClaimStatus::Stale, reason)),
        ClaimStatus::Stale
        | ClaimStatus::Expired
        | ClaimStatus::HandoffRequired
        | ClaimStatus::HandoffRecorded
        | ClaimStatus::Released => None,
    }
}

fn heartbeat_stale_reason(claim: &ClaimContract, now_unix: i64) -> Option<StableId> {
    let Some(last_heartbeat_unix) = rfc3339_to_unix(&claim.lease.last_heartbeat_at) else {
        return Some(StableId("last_heartbeat_unparseable".into()));
    };
    let Ok(interval) = i64::try_from(claim.lease.heartbeat_interval_seconds) else {
        return Some(StableId("heartbeat_interval_unrepresentable".into()));
    };
    let Some(stale_at) = last_heartbeat_unix.checked_add(interval) else {
        return Some(StableId("heartbeat_stale_at_overflow".into()));
    };
    (now_unix >= stale_at).then(|| StableId("heartbeat_overdue".into()))
}

fn reconciled_transition(
    claim: &ClaimContract,
    now_unix: i64,
    to: ClaimStatus,
    reason_code: StableId,
) -> ClaimReconcileTransition {
    let mut updated = claim.clone();
    updated.status = ClaimStatusRecord {
        value: to,
        evaluated_at: unix_to_rfc3339(now_unix),
        reason_code: Some(reason_code.clone()),
    };
    ClaimReconcileTransition {
        claim_id: claim.id.clone(),
        from: claim.status.value,
        to,
        reason_code,
        updated,
    }
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
            principal_id: c
                .claim
                .claimant_principal_id
                .as_ref()
                .map(|principal| principal.0.clone()),
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
        handoff_request_ref: Some(RepoPath(
            "contracts/requests/claim-expiry-handoff-request.yaml".into(),
        )),
    }
}

fn first_overlapping_path(requested: &[RepoPath], existing: &[RepoPath]) -> Option<RepoPath> {
    requested
        .iter()
        .find(|requested_path| {
            existing
                .iter()
                .any(|existing_path| repo_paths_overlap(requested_path, existing_path))
        })
        .cloned()
}

/// F07.4 — Build a structured [`ConflictContract`] for two principals
/// contesting a scope, or return `None` when they are the same principal (a
/// same-agent re-acquire is continuation, not conflict — ADR-0007 /
/// `GovernanceConflictPartiesNotDistinct`).
///
/// Pure and deterministic: the `conflict_id` is derived from the two
/// principals + the contested target so two acquires of the same conflict
/// produce the same id (idempotent identification; the arbitration ledger in
/// F07.5 deduplicates on it). `now_unix` drives `detected_at`; the resolution
/// starts `Pending` (F07.5 moves it to `Resolved`/`Escalated`).
#[must_use]
#[derive(Clone, Copy)]
struct ConflictActor<'a> {
    principal: Option<&'a PrincipalId>,
    agent: &'a StableId,
}

impl<'a> ConflictActor<'a> {
    const fn new(principal: Option<&'a PrincipalId>, agent: &'a StableId) -> Self {
        Self { principal, agent }
    }
}

fn build_conflict(
    requester: ConflictActor<'_>,
    holder: ConflictActor<'_>,
    scope_kind: IntentScopeKind,
    contested_target: &str,
    reason: ConflictDetectionReason,
    now_unix: i64,
) -> Option<ConflictContract> {
    let principal_a = requester
        .principal
        .cloned()
        .unwrap_or_else(|| PrincipalId(requester.agent.0.clone()));
    let principal_b = holder
        .principal
        .cloned()
        .unwrap_or_else(|| PrincipalId(holder.agent.0.clone()));
    // Same principal re-acquiring is continuation (heartbeat), not conflict.
    if principal_a == principal_b {
        return None;
    }
    // Deterministic, ordering-independent id: the two principals are sorted so
    // alice-vs-bob and bob-vs-alice produce the same conflict_id.
    let (lo, hi) = if principal_a.0 <= principal_b.0 {
        (&principal_a.0, &principal_b.0)
    } else {
        (&principal_b.0, &principal_a.0)
    };
    let conflict_id = StableId(format!("conflict.{lo}.{hi}.{contested_target}"));
    Some(ConflictContract {
        conflict_id,
        // Intent refs are not yet wired (F07.5 ledger links them); use the
        // principals + scope as the identifying tuple for now.
        intent_a: StableId(principal_a.0.clone()),
        intent_b: StableId(principal_b.0.clone()),
        principal_a,
        principal_b,
        contested_scope: IntentScope {
            kind: scope_kind,
            target: StableId(contested_target.to_string()),
        },
        detection_reason: reason,
        detected_at: u64::try_from(now_unix.max(0)).unwrap_or(0),
        resolution: ConflictResolutionState::Pending,
    })
}

#[allow(clippy::result_large_err)] // ClaimRejection is a rare error path, not hot data (F07.4 added ConflictContract)
fn checked_lease_expiry(
    now_unix: i64,
    ttl_seconds: u64,
    field: &'static str,
) -> Result<i64, ClaimRejection> {
    let ttl = i64::try_from(ttl_seconds).map_err(|_| ClaimRejection::InvalidRequest {
        field,
        message: "lease ttl_seconds exceeds the supported signed epoch range".into(),
    })?;
    now_unix
        .checked_add(ttl)
        .ok_or_else(|| ClaimRejection::InvalidRequest {
            field,
            message: "lease expires_at would overflow the supported signed epoch range".into(),
        })
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
            principal_id: Some(PrincipalId(agent.into())),
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
    fn timestamp_rejects_invalid_calendar_values() {
        assert!(rfc3339_to_unix("2026-02-31T00:00:00Z").is_none());
        assert!(rfc3339_to_unix("2026-04-31T00:00:00Z").is_none());
        assert!(rfc3339_to_unix("2026-01-01T99:00:00Z").is_none());
        assert!(rfc3339_to_unix("2026-01-01T00:99:00Z").is_none());
        assert!(rfc3339_to_unix("2026-01-01T00:00:99Z").is_none());
        assert!(rfc3339_to_unix("2024-02-29T00:00:00Z").is_some());
        assert!(rfc3339_to_unix("2026-02-29T00:00:00Z").is_none());
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
    fn acquire_rejects_ttl_overflow() {
        let mut request = req("s1", "agentA");
        request.ttl_seconds = u64::MAX;
        let d = acquire(&[], &request, T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest { field, .. }) if field == "ttl_seconds"
        ));
    }

    #[test]
    fn acquire_rejects_expires_at_overflow() {
        let mut request = req("s1", "agentA");
        request.ttl_seconds = 1;
        let d = acquire(&[], &request, i64::MAX);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest { field, .. }) if field == "ttl_seconds"
        ));
    }

    #[test]
    fn acquire_default_expiry_policy_is_validator_consistent() {
        let d = acquire(&[], &req("s1", "agentA"), T0);
        let ClaimLifecycleDecision::Accepted(c) = d else {
            panic!("should accept: {d:?}");
        };
        assert!(c.expiry_policy.handoff_required);
        assert!(!c.expiry_policy.release_without_handoff_allowed);
        assert!(
            c.expiry_policy.handoff_request_ref.is_some(),
            "validate_claim rejects handoff_required policies without handoff_request_ref"
        );
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
    fn acquire_rejects_live_path_overlap_across_scopes() {
        let holder = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let mut request = req("s2", "agentB");
        request.paths = vec![RepoPath("crates/x-s1/src/lib.rs".into())];
        let d = acquire(&[holder], &request, T0);
        match d {
            ClaimLifecycleDecision::Rejected(ClaimRejection::PathAlreadyClaimed {
                path,
                holder,
                ..
            }) => {
                assert_eq!(path.0, "crates/x-s1/src/lib.rs");
                assert_eq!(holder.0, "agentA");
            }
            other => panic!("expected PathAlreadyClaimed, got {other:?}"),
        }
    }

    #[test]
    fn acquire_rejects_expired_path_overlap_requiring_handoff() {
        let expired = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        let mut request = req("s2", "agentB");
        request.paths = vec![RepoPath("crates/x-s1/src/lib.rs".into())];
        let d = acquire(&[expired], &request, T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));
    }

    // --- F07.4: structured ConflictContract emission on intent overlap ---

    #[test]
    fn acquire_emits_conflict_when_distinct_principals_overlap_path() {
        // agentA holds a live claim covering crates/x-s1; agentB requests an
        // overlapping path → PathAlreadyClaimed WITH a structured ConflictContract
        // (distinct principals ⇒ real conflict).
        let holder = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let mut request = req("s2", "agentB");
        request.paths = vec![RepoPath("crates/x-s1/src/lib.rs".into())];
        let d = acquire(&[holder], &request, T0);
        match d {
            ClaimLifecycleDecision::Rejected(ClaimRejection::PathAlreadyClaimed {
                conflict: Some(conflict),
                ..
            }) => {
                assert_eq!(conflict.principal_a, PrincipalId("agentB".into()));
                assert_eq!(conflict.principal_b, PrincipalId("agentA".into()));
                assert_eq!(
                    conflict.detection_reason,
                    ConflictDetectionReason::PathOverlap
                );
                assert_eq!(conflict.contested_scope.kind, IntentScopeKind::PathPrefix);
                assert_eq!(conflict.contested_scope.target.0, "crates/x-s1/src/lib.rs");
                assert_eq!(conflict.resolution, ConflictResolutionState::Pending);
            }
            other => panic!("expected PathAlreadyClaimed with conflict, got {other:?}"),
        }
    }

    #[test]
    fn acquire_no_conflict_when_same_agent_reacquires() {
        // Same agent re-acquiring a live scope is continuation (heartbeat), NOT
        // conflict. The rejection fires (a fresh acquire is still blocked), but
        // conflict must be None.
        let holder = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let d = acquire(&[holder], &req("s1", "agentA"), T0);
        match d {
            ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
                conflict: None,
                ..
            }) => {}
            other => panic!("expected AlreadyClaimedByOther with conflict=None, got {other:?}"),
        }
    }

    #[test]
    fn distinct_principals_sharing_agent_label_still_emit_conflict() {
        let mut holder = manual_claim("shared", "agent-shared", T0 + 600, ClaimStatus::Active);
        holder.claim.claimant_principal_id = Some(PrincipalId("principal.alpha".into()));
        let mut request = req("shared", "agent-shared");
        request.principal_id = Some(PrincipalId("principal.beta".into()));

        let ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
            conflict: Some(conflict),
            ..
        }) = acquire(&[holder], &request, T0)
        else {
            panic!("distinct execution principals must produce a conflict");
        };
        let principals = [conflict.principal_a.0, conflict.principal_b.0];
        assert!(principals.iter().any(|value| value == "principal.alpha"));
        assert!(principals.iter().any(|value| value == "principal.beta"));
    }

    #[test]
    fn acquire_emits_conflict_with_correct_attribution() {
        // Two distinct principals contesting a scope → full attribution check.
        let holder = manual_claim("s1", "alice", T0 + 600, ClaimStatus::Active);
        let d = acquire(&[holder], &req("s1", "bob"), T0);
        match d {
            ClaimLifecycleDecision::Rejected(ClaimRejection::AlreadyClaimedByOther {
                conflict: Some(conflict),
                ..
            }) => {
                // detected_at = now (deterministic; the engine never touches a clock).
                assert_eq!(conflict.detected_at, u64::try_from(T0).unwrap());
                // conflict_id is deterministic + ordering-independent.
                assert!(
                    conflict.conflict_id.0.contains("alice")
                        && conflict.conflict_id.0.contains("bob")
                        && conflict.conflict_id.0.contains("s1"),
                    "conflict_id must name both principals + the scope: {}",
                    conflict.conflict_id.0
                );
                assert_eq!(
                    conflict.detection_reason,
                    ConflictDetectionReason::AuthorityScopeOverlap
                );
            }
            other => panic!("expected AlreadyClaimedByOther with conflict, got {other:?}"),
        }
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
    fn heartbeat_rejects_ttl_overflow() {
        let mut c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        c.lease.ttl_seconds = u64::MAX;
        let d = heartbeat(&c, &StableId("agentA".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest { field, .. }) if field == "lease.ttl_seconds"
        ));
    }

    #[test]
    fn heartbeat_rejects_expires_at_overflow() {
        let mut c = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        c.lease.ttl_seconds = i64::MAX as u64;
        let d = heartbeat(&c, &StableId("agentA".into()), T0);
        assert!(matches!(
            d,
            ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest { field, .. }) if field == "lease.ttl_seconds"
        ));
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
    fn heartbeat_handoff_required_keeps_recovery_hint_rejection() {
        let handoff_required = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::HandoffRequired);
        let d = heartbeat(&handoff_required, &StableId("agentA".into()), T0);
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
    fn reconcile_marks_active_claim_stale_when_heartbeat_is_overdue() {
        let mut claim = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        claim.lease.last_heartbeat_at = unix_to_rfc3339(T0);
        claim.lease.heartbeat_interval_seconds = 120;

        let report = reconcile_claims(&[claim], T0 + 120);

        assert_eq!(report.scanned, 1);
        assert_eq!(report.transitions.len(), 1);
        let transition = &report.transitions[0];
        assert_eq!(transition.from, ClaimStatus::Active);
        assert_eq!(transition.to, ClaimStatus::Stale);
        assert_eq!(transition.reason_code.0, "heartbeat_overdue");
        assert_eq!(transition.updated.status.value, ClaimStatus::Stale);
        assert_eq!(
            transition.updated.status.evaluated_at,
            unix_to_rfc3339(T0 + 120)
        );
    }

    #[test]
    fn reconcile_materializes_expired_handoff_required_claim() {
        let claim = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);

        let report = reconcile_claims(&[claim], T0);

        assert_eq!(report.transitions.len(), 1);
        let transition = &report.transitions[0];
        assert_eq!(transition.from, ClaimStatus::Active);
        assert_eq!(transition.to, ClaimStatus::HandoffRequired);
        assert_eq!(transition.reason_code.0, "lease_expired");
        assert_eq!(
            transition.updated.status.value,
            ClaimStatus::HandoffRequired
        );
    }

    #[test]
    fn reconcile_materializes_expired_when_no_handoff_required() {
        let mut claim = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Stale);
        claim.expiry_policy.handoff_required = false;
        claim.expiry_policy.release_without_handoff_allowed = true;

        let report = reconcile_claims(&[claim], T0);

        assert_eq!(report.transitions.len(), 1);
        assert_eq!(report.transitions[0].from, ClaimStatus::Stale);
        assert_eq!(report.transitions[0].to, ClaimStatus::Expired);
    }

    #[test]
    fn reconcile_is_idempotent_for_terminal_and_already_stale_claims() {
        let stale = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Stale);
        let handoff_required = manual_claim("s2", "agentB", T0 - 1, ClaimStatus::HandoffRequired);
        let released = manual_claim("s3", "agentC", T0 + 600, ClaimStatus::Released);

        let report = reconcile_claims(&[stale, handoff_required, released], T0);

        assert_eq!(report.scanned, 3);
        assert!(report.transitions.is_empty());
    }

    #[test]
    fn reconcile_treats_unparseable_heartbeat_as_stale_not_live_ok() {
        let mut claim = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        claim.lease.last_heartbeat_at = "not-a-timestamp".into();

        let report = reconcile_claims(&[claim], T0);

        assert_eq!(report.transitions.len(), 1);
        assert_eq!(report.transitions[0].to, ClaimStatus::Stale);
        assert_eq!(
            report.transitions[0].reason_code.0,
            "last_heartbeat_unparseable"
        );
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
        assert_eq!(view.active[0].principal_id.as_deref(), Some("agentA"));
        assert_eq!(view.active[0].agent_id, "agentA");
    }

    #[test]
    fn project_active_is_serializable_bus_view() {
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let view = project_active(&[live], T0);
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"scope_id\":\"s1\""));
        assert!(json.contains("\"principal_id\":\"agentA\""));
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
    fn release_handoff_required_keeps_recovery_hint_rejection() {
        let handoff_required = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::HandoffRequired);
        let d = release(&handoff_required, &StableId("agentA".into()), T0);
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
    fn record_handoff_marks_expired_claim_recorded_and_frees_scope() {
        let expired = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::Active);
        let request = RecordHandoffRequest {
            recorder_agent_id: StableId("agentB".into()),
            summary: "agentA lease expired; compact handoff recorded".into(),
            evidence_refs: vec!["handoffs/expired-claims/s1.yaml".into()],
        };
        let d = record_handoff(&expired, &request, T0);
        let ClaimLifecycleDecision::Accepted(recorded) = d else {
            panic!("handoff should record: {d:?}");
        };
        assert_eq!(recorded.status.value, ClaimStatus::HandoffRecorded);
        assert_eq!(
            recorded.status.reason_code.as_ref().unwrap().0,
            "handoff_recorded_by_agentB"
        );
        assert!(recorded
            .evidence_refs
            .contains(&"handoffs/expired-claims/s1.yaml".to_string()));

        let reacquire = acquire(&[recorded], &req("s1", "agentB"), T0);
        assert!(matches!(reacquire, ClaimLifecycleDecision::Accepted(_)));
    }

    #[test]
    fn materialized_handoff_required_blocks_until_handoff_recorded() {
        let mut pending = manual_claim("s1", "agentA", T0 - 1, ClaimStatus::HandoffRequired);
        pending.status.reason_code = Some(StableId("lease_expired".into()));

        let blocked = acquire(&[pending.clone()], &req("s1", "agentB"), T0);
        assert!(matches!(
            blocked,
            ClaimLifecycleDecision::Rejected(ClaimRejection::ExpiredRequiresHandoff { .. })
        ));

        let request = RecordHandoffRequest {
            recorder_agent_id: StableId("agentB".into()),
            summary: "materialized handoff_required claim resolved".into(),
            evidence_refs: Vec::new(),
        };
        let ClaimLifecycleDecision::Accepted(recorded) = record_handoff(&pending, &request, T0)
        else {
            panic!("handoff_required claim should be recordable");
        };
        assert_eq!(recorded.status.value, ClaimStatus::HandoffRecorded);
    }

    #[test]
    fn record_handoff_rejects_live_or_empty_summary() {
        let live = manual_claim("s1", "agentA", T0 + 600, ClaimStatus::Active);
        let empty = RecordHandoffRequest {
            recorder_agent_id: StableId("agentB".into()),
            summary: "  ".into(),
            evidence_refs: Vec::new(),
        };
        assert!(matches!(
            record_handoff(&live, &empty, T0),
            ClaimLifecycleDecision::Rejected(ClaimRejection::InvalidRequest { field, .. }) if field == "summary"
        ));

        let request = RecordHandoffRequest {
            recorder_agent_id: StableId("agentB".into()),
            summary: "not expired".into(),
            evidence_refs: Vec::new(),
        };
        assert!(matches!(
            record_handoff(&live, &request, T0),
            ClaimLifecycleDecision::Rejected(ClaimRejection::IllegalTransition { to, .. }) if to == ClaimStatus::HandoffRecorded
        ));
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
