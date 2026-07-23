//! Public compatibility facade for the Memory policy-enforcement API.
//!
//! Mutation authority, stream descriptors, and retained transactions live in
//! `forge-core-eventlog`; this crate intentionally exposes only Memory DTOs,
//! read projection, and policy-shaped PEP operations.

pub use forge_core_eventlog::memory::{
    admit, admit_with_durability, forget, forget_with_durability, list_now,
    list_now_with_durability, now_unix, project, promote, promote_with_durability, AdmissionResult,
    AdmissionStatus, AdmitError, DenialReasons, ForgetError, ForgetResult, ForgetStatus,
    ListResult, ListStatus, MemoryEvent, MemoryProjection, MemoryProjectionDiagnostic,
    MemoryProjectionError, MemoryProjectionSeverity, ProjectionResult, PromoteError, PromoteResult,
    PromoteStatus, MEMORY_LOCK_RELATIVE_PATH, MEMORY_LOG_RELATIVE_PATH,
};
