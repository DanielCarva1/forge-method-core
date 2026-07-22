//! Public compatibility facade for Governance policy-enforcement operations.
//!
//! Stream identity, retained handles, and raw mutation remain private to
//! `forge-core-eventlog`; callers receive only governance DTOs, projection
//! queries, and PEP operations.

pub use forge_core_eventlog::governance::{
    arbitrate, arbitrate_with_durability, escalate, escalate_with_durability, list, now_unix,
    project, record, record_with_durability, ArbitrateError, ArbitrateResult, ArbitrateStatus,
    ArbitrationProjection, ArbitrationProjectionDiagnostic, ArbitrationProjectionError,
    ArbitrationProjectionSeverity, EscalateError, EscalateResult, EscalateStatus, GovernanceEvent,
    ProjectionResult, RecordError, RecordResult, RecordStatus, GOVERNANCE_LOCK_RELATIVE_PATH,
    GOVERNANCE_LOG_RELATIVE_PATH,
};
