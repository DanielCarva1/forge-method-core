//! Error types for the research source PEP (Policy Enforcement Point).
//!
//! Historically the PEP had its own `{Lock, Append, Serialize, Read}` enum
//! ([`ResearchAdmitError`]) plus a [`ResearchProjectionError`] for the
//! cold-replay path. Both map 1:1 onto
//! [`forge_core_eventlog::EventLogError`], so they are now **type aliases** for
//! `EventLogError<ResearchProjectionDiagnostic>` — same shape, same variants,
//! same `Display`, but a single source of truth in `forge-core-eventlog`
//! (hand-rolled, no `anyhow`/`thiserror`).
//!
//! A denied admission is [`crate::AdmissionStatus::DeniedByGate`], NOT an
//! error (the PEP enforces a pure decision; it fails on storage mechanics,
//! never on policy). A torn-write tail is also NOT an error: the projection
//! stops at the last valid record and emits a
//! [`crate::ResearchProjectionDiagnostic`] (mirrors the memory PEP's tolerance).

/// Errors raised by [`crate::admission::admit_source`] (and its
/// `*_with_durability` twin). The PEP only enforces a pure decision; it fails
/// on the storage mechanics (lock, append, serialize, read), never on policy.
pub type ResearchAdmitError =
    forge_core_eventlog::EventLogError<crate::ResearchProjectionDiagnostic>;

/// Errors raised by [`crate::project`] (the cold replay-on-read path). A
/// torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`crate::ResearchProjectionDiagnostic`] (mirrors the
/// memory PEP's tolerance). Only structural I/O / parse failures are errors.
pub type ResearchProjectionError =
    forge_core_eventlog::EventLogError<crate::ResearchProjectionDiagnostic>;
