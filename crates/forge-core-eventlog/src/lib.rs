//! Trusted `EventLog` runtime for Forge's three policy-enforcement streams.
//!
//! This crate owns the closed Memory, Research, and Governance stream
//! descriptors, their event folds, and all mutation critical sections.  The
//! public API is deliberately policy-shaped: consumers use the domain facades
//! (`forge-core-memory`, `forge-core-research`, and `forge-core-governance`).
//! There is no public raw lock, descriptor, projection-under-lock, or append
//! capability.
//!
//! # Boundary
//!
//! The retained descriptor-relative I/O and leaf checks below are confinement
//! and fail-closed defenses for cooperating local producers. They serialize
//! projection, policy decision, sequence allocation, append, and requested
//! sync on one designated advisory stream lock. They are **not** a filesystem
//! MAC boundary: concurrent same-user namespace replacement/direct mutation is
//! outside this cooperative model and yields no logical-success claim.
//!
//! # Compile-negative API boundary
//!
//! Each independent doctest fails only when its specific authority remains
//! private, preventing a combined import from masking an accidental export.
//!
//! ```compile_fail
//! use forge_core_eventlog::append_event;
//! ```
//!
//! ```compile_fail
//! use forge_core_eventlog::EventLogLock;
//! ```
//!
//! ```compile_fail
//! use forge_core_eventlog::StreamId;
//! ```

pub mod error;
#[path = "governance/lib.rs"]
pub mod governance;
pub mod macros;
#[path = "memory/lib.rs"]
pub mod memory;
#[path = "research/lib.rs"]
pub mod research;

mod tcb;

pub use error::EventLogError;
/// Durability requested by the domain PEPs.
pub use forge_core_store::WalDurability;
pub use tcb::{capture_quiesced_event_logs, QuiescedEventLogMember, QuiescedEventLogSnapshot};

/// Non-authoritative envelope accessors exposed for event inspection.
pub trait EventEnvelope {
    fn sequence(&self) -> u64;
    fn at_unix(&self) -> u64;
}

/// Stable diagnostic code for an ignored non-monotonic event.
pub const CODE_OUT_OF_ORDER_EVENT_IGNORED: &str = "out_of_order_event_ignored";
/// Stable diagnostic code for an accepted torn final JSONL line.
pub const CODE_TORN_FINAL_LINE_SKIPPED: &str = "torn_final_line_skipped";

/// Best-effort wall-clock seconds since the UNIX epoch.
#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

// Domain modules are public only as DTO/PEP namespaces. `tcb` remains private;
// its closed stream identity and retained transaction cannot be named by a
// downstream crate.
