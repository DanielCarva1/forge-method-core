//! Error types for the memory PEP (Policy Enforcement Point).
//!
//! Historically each fallible operation had its own `{Lock, Append, Serialize,
//! Read}` enum (three near-identical copies of `AdmitError`/`PromoteError`/
//! `ForgetError`, plus `MemoryProjectionError` for the cold-replay path). All
//! four map 1:1 onto [`forge_core_eventlog::EventLogError`], so they are now
//! **type aliases** for `EventLogError<MemoryProjectionDiagnostic>` — same
//! shape, same variants, same `Display`, but a single source of truth in
//! `forge-core-eventlog` (per AGENTS.md: hand-rolled, no `anyhow`/`thiserror`).
//!
//! Per-operation aliases (rather than one shared name) are kept so call-site
//! signatures stay self-documenting — `AdmitError` reads as "an error from
//! `admit`" even though it is structurally identical to `ForgetError`.
//! Exhaustive matching on these is still honest: the variants a given call can
//! produce are exactly the four the PEP touches (Lock/Append/Serialize/Read);
//! `Parse` and `ProjectionDiagnostic` are produced only by the cold-replay path
//! (`project`/`project_locked`).
//!
//! A torn-write tail is NOT an error here: the projection stops at the last
//! valid record and emits a [`crate::MemoryProjectionDiagnostic`] (mirrors
//! `ClaimWalProjectionError::RecoveryStopped`). Only structural I/O / parse
//! failures are errors.

/// Errors raised by [`crate::admission::admit`] (and its `*_with_durability`
/// twin). The PEP only enforces a pure decision; it fails on the storage
/// mechanics (lock, append, serialize, read), never on policy (a denied
/// admission is [`crate::AdmissionStatus::DeniedByGate`], not an error).
pub type AdmitError = forge_core_eventlog::EventLogError<crate::MemoryProjectionDiagnostic>;

/// Errors raised by [`crate::promote::promote`]. A denied promotion is
/// [`crate::PromoteStatus::DeniedByGate`], not an error.
pub type PromoteError = forge_core_eventlog::EventLogError<crate::MemoryProjectionDiagnostic>;

/// Errors raised by [`crate::retention::forget`].
pub type ForgetError = forge_core_eventlog::EventLogError<crate::MemoryProjectionDiagnostic>;

/// Errors raised by [`crate::project`] (the cold replay-on-read path) and by
/// the list/sweep path in [`crate::retention::list_now`] when it must rebuild.
///
/// A torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`crate::MemoryProjectionDiagnostic`] (mirrors
/// `ClaimWalProjectionError::RecoveryStopped`). Only structural I/O / parse
/// failures are errors.
pub type MemoryProjectionError =
    forge_core_eventlog::EventLogError<crate::MemoryProjectionDiagnostic>;
