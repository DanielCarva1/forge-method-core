//! Public compatibility facade for the Research source-ledger PEP.
//!
//! The closed ledger stream and mutation transaction live in
//! `forge-core-eventlog`; this facade retains the established DTO and PEP API.

pub mod graph;

pub use forge_core_eventlog::research::{
    admit_source, admit_source_with_durability, now_unix, project, AdmissionResult,
    AdmissionStatus, ProjectionResult, ResearchAdmitError, ResearchEvent, ResearchProjection,
    ResearchProjectionDiagnostic, ResearchProjectionError, ResearchProjectionSeverity,
    RESEARCH_LOCK_RELATIVE_PATH, RESEARCH_LOG_RELATIVE_PATH,
};
pub use graph::{evidence_graph, ClaimRef};
