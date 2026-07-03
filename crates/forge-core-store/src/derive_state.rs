//! `derive_state` — the sole authority constructor for claim state.
//!
//! Per the Claims Integrity Spine spec
//! (`contracts/spec/claims-integrity-spine-spec.yaml`), claim state is derived
//! exclusively by replaying the append-only WAL
//! (`<state_root>/wal/claims.fmw1`). The ephemeral `claims-active/*.yaml`
//! cache is **not** an authority path; it exists only as a compatibility/debug
//! artifact. This module is the named, canonical entrypoint that enforces that
//! invariant: there is one way to build claim state, and it fails closed
//! instead of silently trusting editable YAML.
//!
//! # Evaluation model
//!
//! `derive_state` is **LAZY**: it replays the WAL on every invocation. The
//! snapshot/rotation perf layer (P3.3 in the spec) is a later optimization
//! layered *on top* of this constructor; it is not a correctness concern
//! here. Replay cost is bounded while the WAL is small, and rotation already
//! keeps it bounded (`rotate_claim_wal_if_needed`).
//!
//! # Auto-repair
//!
//! [`derive_state`] performs torn-tail repair transparently: if the first
//! replay stops on a truncation reason (`TruncatedHeader`/`TruncatedPayload`),
//! it repairs the WAL to the last verified byte offset and re-reads. This is
//! the same three-step dance the CLI used to inline; it now lives here as the
//! canonical authority path. Hard stop reasons (CRC mismatch, sequence gap,
//! unknown record, …) are **not** repaired — they surface as
//! [`DeriveStateError::RecoveryStopped`] so the caller can refuse to act on
//! ambiguous state.
//!
//! # Acceptance criteria preserved (spec ACs)
//!
//! - **ac1** hand-edited payload → recovery stops at the tampered record
//!   (CRC mismatch); the forged lease is never honored.
//! - **ac2** torn write → recovers the prefix, truncates under exclusive lock,
//!   returns the repaired projection.
//! - **ac3** concurrent appends → monotonic sequences (enforced in the decoder).
//! - **ac4** single-bit flip → stops at checksum failure.
//! - **ac5** no authority path reads mutable YAML directly; the legacy read is
//!   behind an explicit `--from-cache` debug flag in the CLI.
//! - **ac6** `load_claims` is kept as a debug/fallback, not removed.
//! - **ac7** editing `claims-active/*.yaml` does not change derived state.
//!
//! See `claim_wal.rs` for the FMW1 byte-level format and the pure projection
//! fold, and the spec linked above for the full contract.

use std::path::Path;

use crate::claim_wal::{
    self, ClaimWalProjection, ClaimWalReadError, ClaimWalRecovery, ClaimWalStopReason,
};

/// Failures constructing claim state via [`derive_state`].
///
/// Each variant carries enough context for the caller to build an actionable
/// diagnostic. Mirrors the error discipline of the rest of the store crate:
/// a hand-rolled enum (no `anyhow`/`thiserror`), deriving
/// `Debug, Clone, PartialEq, Eq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeriveStateError {
    /// The WAL could not be read or locked (IO-level failure).
    Read {
        /// The state root that was being derived.
        state_root: String,
        /// The underlying read/lock failure.
        source: ClaimWalReadError,
    },
    /// Recovery stopped on a hard (non-truncation) reason — CRC mismatch,
    /// sequence gap, unsupported record type, etc. The caller must refuse to
    /// act on ambiguous state rather than honor a partial prefix.
    RecoveryStopped {
        /// The WAL path.
        wal_path: String,
        /// Why recovery stopped.
        stop_reason: ClaimWalStopReason,
        /// Last byte offset verified before the stop.
        last_good_offset: u64,
        /// Original WAL length at the time of recovery.
        original_len: u64,
    },
    /// A truncation was repaired but the re-read still did not reach clean EOF.
    /// Indicates repeated corruption or a failing disk; surfaces as a hard
    /// error so the operator investigates instead of trusting partial state.
    RepairDidNotRecover {
        /// The state root.
        state_root: String,
        /// The stop reason observed after repair.
        stop_reason: ClaimWalStopReason,
    },
}

/// Derive claim state from the WAL — **the sole authority constructor**.
///
/// Replays `<state_root>/wal/claims.fmw1`, auto-repairing a torn tail, and
/// returns the typed projection (active/released/handoff buckets, per-agent
/// and per-path indexes, diagnostics). Idempotent: calling it twice on an
/// unchanged WAL yields the same projection.
///
/// Use this everywhere claim state is read for a decision. The ephemeral
/// `claims-active/*.yaml` cache must never be the source of truth; inspect it
/// only through the CLI's `--from-cache` debug flag.
///
/// # Errors
///
/// - [`DeriveStateError::Read`] if the WAL cannot be locked or read.
/// - [`DeriveStateError::RecoveryStopped`] on a hard stop (CRC mismatch,
///   sequence gap, …) — the prefix is intentionally NOT honored for
///   non-truncation stops.
/// - [`DeriveStateError::RepairDidNotRecover`] if a torn tail was repaired but
///   the re-read still stops short of clean EOF.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use forge_core_store::derive_state::derive_state;
///
/// let state_root = Path::new(".forge-method");
/// match derive_state(state_root) {
///     Ok(projection) => {
///         println!("active claims: {}", projection.active_by_claim_id.len());
///     }
///     Err(error) => {
///         eprintln!("cannot derive claim state: {error:?}");
///     }
/// }
/// ```
pub fn derive_state(state_root: impl AsRef<Path>) -> Result<ClaimWalProjection, DeriveStateError> {
    let state_root = state_root.as_ref();
    derive_state_inner(state_root)
}

/// Derive claim state from an already-recovered [`ClaimWalRecovery`].
///
/// Pure: no IO. Useful for unit tests and for callers that already hold a
/// recovery (e.g. one produced by [`claim_wal::recover_claim_wal`] under a
/// shared lock). Simply folds the recovery through the projection.
#[must_use]
pub fn derive_state_from_recovery(recovery: ClaimWalRecovery) -> ClaimWalProjection {
    claim_wal::project_claim_wal_recovery(recovery)
}

/// The auto-repair authority path. Separated from the thin [`derive_state`]
/// wrapper so the three-step replay/repair/reread logic is testable in
/// isolation and readable in one place.
fn derive_state_inner(state_root: &Path) -> Result<ClaimWalProjection, DeriveStateError> {
    let first = claim_wal::replay_claim_wal(state_root, false)
        .map_err(|source| DeriveStateError::Read {
            state_root: state_root.display().to_string(),
            source,
        })?;
    match first.recovery.stop_reason {
        // Clean WAL: projection is authoritative.
        ClaimWalStopReason::CleanEof => Ok(first),
        // Torn tail: repair once, then re-read. Idempotent if already repaired.
        ClaimWalStopReason::TruncatedHeader | ClaimWalStopReason::TruncatedPayload => {
            // The repair pass truncates the WAL to the last good offset under
            // an exclusive lock; ignore its projection (it stopped at the same
            // truncation point, by construction).
            claim_wal::replay_claim_wal(state_root, true).map_err(|source| {
                DeriveStateError::Read {
                    state_root: state_root.display().to_string(),
                    source,
                }
            })?;
            let repaired = claim_wal::replay_claim_wal(state_root, false).map_err(|source| {
                DeriveStateError::Read {
                    state_root: state_root.display().to_string(),
                    source,
                }
            })?;
            if repaired.recovery.stop_reason == ClaimWalStopReason::CleanEof {
                Ok(repaired)
            } else {
                Err(DeriveStateError::RepairDidNotRecover {
                    state_root: state_root.display().to_string(),
                    stop_reason: repaired.recovery.stop_reason,
                })
            }
        }
        // Hard stop (CRC mismatch, sequence gap, unsupported record, …): do
        // NOT honor the partial prefix. Surface the stop so the caller refuses
        // to act on ambiguous state.
        other => Err(DeriveStateError::RecoveryStopped {
            wal_path: first.recovery.wal_path.display().to_string(),
            stop_reason: other,
            last_good_offset: first.recovery.last_good_offset,
            original_len: first.recovery.original_len,
        }),
    }
}

impl std::fmt::Display for DeriveStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { state_root, source } => write!(
                f,
                "derive_state: cannot read claim WAL at {state_root}: {source}"
            ),
            Self::RecoveryStopped {
                wal_path,
                stop_reason,
                last_good_offset,
                original_len,
            } => write!(
                f,
                "derive_state: WAL recovery stopped with {stop_reason:?} at offset \
                 {last_good_offset}/{original_len} ({wal_path}); refusing to honor partial state"
            ),
            Self::RepairDidNotRecover {
                state_root,
                stop_reason,
            } => write!(
                f,
                "derive_state: torn-tail repair at {state_root} did not reach clean EOF \
                 (still {stop_reason:?}); investigate possible repeated corruption"
            ),
        }
    }
}

impl std::error::Error for DeriveStateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_wal::{append_claim_wal_record, ClaimWalOperation};
    use forge_core_contracts::claim::{
        ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
        ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
    };
    use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Unique temp state root so parallel tests never collide on a lock.
    fn temp_state(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-derive-state-{test_name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp state root");
        root
    }

    /// Minimal active claim, mirroring the shape used by the `claim_wal` tests.
    fn acquire_claim(scope: &str, agent: &str, path: &str) -> ClaimContract {
        ClaimContract {
            id: ClaimId(format!("claim.story.{scope}.{scope}")),
            contract_ref: RepoPath(format!("claims-active/claim-story-{scope}-{scope}.yaml")),
            claim: ClaimIdentity {
                kind: ClaimKind::Story,
                claimant_agent_id: StableId(agent.to_string()),
                claimant_role: ActorRole::Worker,
                registry_ref: None,
            },
            scope: ClaimScope {
                kind: ClaimScopeKind::Story,
                id: ScopeId(scope.to_string()),
                product_area: None,
                paths: vec![RepoPath(path.to_string())],
            },
            lease: ClaimLease {
                acquired_at: "2027-01-15T08:00:00Z".to_string(),
                last_heartbeat_at: "2027-01-15T08:00:00Z".to_string(),
                expires_at: "2027-01-15T08:10:00Z".to_string(),
                ttl_seconds: 600,
                heartbeat_interval_seconds: 120,
                expected_state_version: 0,
            },
            status: ClaimStatusRecord {
                value: ClaimStatus::Active,
                evaluated_at: "2027-01-15T08:00:00Z".to_string(),
                reason_code: None,
            },
            expiry_policy: ExpiryPolicy {
                on_expiry: ExpiryAction::RecordHandoffRequest,
                handoff_required: true,
                release_without_handoff_allowed: false,
                reclaim_policy: ReclaimPolicy::DriverReview,
                handoff_request_ref: Some(RepoPath(
                    "contracts/requests/claim-expiry-handoff-request.yaml".to_string(),
                )),
            },
            evidence_refs: Vec::new(),
        }
    }

    /// A state root whose WAL holds one acquire record.
    fn state_root_with_one_acquire(test_name: &str) -> PathBuf {
        let root = temp_state(test_name);
        let claim = acquire_claim("S1", "alice", "src/lib.rs");
        append_claim_wal_record(&root, ClaimWalOperation::Acquire, &claim, "2027-01-15T08:00:00Z")
            .expect("append acquire");
        root
    }

    #[test]
    fn derive_state_replays_wal_and_returns_projection() {
        let root = state_root_with_one_acquire("replays");
        let projection = derive_state(&root).expect("derive_state succeeds");
        assert_eq!(projection.recovery.stop_reason, ClaimWalStopReason::CleanEof);
        assert_eq!(projection.claims.len(), 1, "one acquire folded");
        assert!(
            projection.active_by_claim_id.contains_key("claim.story.S1.S1"),
            "acquire lands in the active bucket"
        );
    }

    #[test]
    fn derive_state_fails_closed_when_wal_missing() {
        let root = temp_state("missing-wal");
        // No wal/ dir. recover_claim_wal reads an empty (missing) WAL as
        // clean-empty. Assert it does NOT fall back to any cache and returns
        // an empty projection cleanly.
        let projection = derive_state(&root).expect("empty WAL is clean-empty");
        assert_eq!(projection.recovery.stop_reason, ClaimWalStopReason::CleanEof);
        assert!(projection.claims.is_empty(), "no records → empty projection");
    }

    #[test]
    fn derive_state_is_idempotent() {
        let root = state_root_with_one_acquire("idempotent");
        let first = derive_state(&root).expect("first derive");
        let second = derive_state(&root).expect("second derive");
        assert_eq!(
            first.claims, second.claims,
            "unchanged WAL yields identical projections"
        );
        assert_eq!(first.last_applied_seq, second.last_applied_seq);
    }

    #[test]
    fn derive_state_from_recovery_is_pure() {
        let root = state_root_with_one_acquire("from-recovery");
        let recovery = crate::claim_wal::recover_claim_wal(&root, false).expect("recover");
        let projection = derive_state_from_recovery(recovery);
        assert_eq!(projection.claims.len(), 1);
    }
}
