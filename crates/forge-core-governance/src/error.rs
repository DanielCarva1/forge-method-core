//! Error types for the governance arbitration PEP (Policy Enforcement Point).
//!
//! Historically each fallible operation had its own `{Lock, Append, Serialize,
//! Read}` enum (`RecordError`/`ArbitrateError`/`EscalateError`, plus
//! `ArbitrationProjectionError` for the cold-replay path). All four map 1:1
//! onto [`forge_core_eventlog::EventLogError`], so they are now **type
//! aliases** for `EventLogError<ArbitrationProjectionDiagnostic>` — same shape,
//! same variants, same `Display`, but a single source of truth in
//! `forge-core-eventlog` (hand-rolled, no `anyhow`/`thiserror`).
//!
//! Per-operation aliases (rather than one shared name) are kept so call-site
//! signatures stay self-documenting — `RecordError` reads as "an error from
//! `record`" even though it is structurally identical to `ArbitrateError`.
//! Exhaustive matching on these is still honest: the variants a given call can
//! produce are exactly the four the PEP touches (Lock/Append/Serialize/Read);
//! `Parse` and `ProjectionDiagnostic` are produced only by the cold-replay path
//! (`project`/`project_locked`).
//!
//! As in the memory PEP, the PEP only enforces a pure decision; it fails on the
//! storage mechanics, never on policy (a denied arbitration is `DeniedByGate`,
//! not an error). A torn-write tail is NOT an error here: the projection stops
//! at the last valid record and emits a [`crate::ArbitrationProjectionDiagnostic`].

/// Errors raised by [`crate::record::record`] (and its `*_with_durability`
/// twin). The PEP only enforces a pure idempotency check; it fails on the
/// storage mechanics, never on policy.
pub type RecordError = forge_core_eventlog::EventLogError<crate::ArbitrationProjectionDiagnostic>;

/// Errors raised by [`crate::arbitrate::arbitrate`]. A denied arbitration is
/// [`crate::ArbitrateStatus::DeniedByGate`], not an error.
pub type ArbitrateError =
    forge_core_eventlog::EventLogError<crate::ArbitrationProjectionDiagnostic>;

/// Errors raised by [`crate::escalate::escalate`].
pub type EscalateError = forge_core_eventlog::EventLogError<crate::ArbitrationProjectionDiagnostic>;

/// Errors raised by [`crate::project`] (the cold replay-on-read path).
///
/// A torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`crate::ArbitrationProjectionDiagnostic`] (mirrors
/// `ClaimWalProjectionError::RecoveryStopped` and the memory PEP). Only
/// structural I/O / parse failures are errors.
pub type ArbitrationProjectionError =
    forge_core_eventlog::EventLogError<crate::ArbitrationProjectionDiagnostic>;
