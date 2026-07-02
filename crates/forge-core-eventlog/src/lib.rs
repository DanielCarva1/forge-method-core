//! `forge-core-eventlog` — generic append-only event-sourced log mechanics.
//!
//! Four crates in this workspace (`forge-core-memory`, `forge-core-research`,
//! `forge-core-governance`, and the JSONL half of `forge-core-store`) copy-paste
//! near-identical event-sourcing boilerplate: an `<X>Event` enum with a
//! `sequence`/`at_unix` envelope, an `<X>Projection { sequence, BTreeMap,
//! superseded, diagnostics }`, an `apply_event` with an out-of-order guard, a
//! free `replay` fold, `project`/`project_locked` (cold-read NDJSON with
//! torn-tail tolerance), `next_sequence`, `now_unix`, an `append_bytes` shim,
//! and a `{Lock, Append, Serialize, Read}` error quartet.
//!
//! This crate is the **generic abstraction** that absorbs that boilerplate. A
//! domain implements [`EventSourced`] (providing its `Event`, `Projection`,
//! `Diagnostic`, and the `apply` fold); this crate provides the mechanics:
//!
//! - [`replay`] — the pure Fowler fold (discard and rebuild the read model).
//! - [`project_locked`] — cold read NDJSON with torn-tail tolerance, no lock.
//! - [`apply_event`] — the shared out-of-order-guarded fold body.
//! - [`next_sequence`] — sequence allocation (`sequence_of + 1`, saturating).
//! - [`now_unix`] — best-effort wall-clock seconds since epoch.
//! - [`append_event`] — serialize → `append_json_line_with_durability` shim.
//! - [`EventLogLock`] — RAII wrapper over `forge_core_store::acquire_effect_store_lock`.
//! - [`EventLogError`] — the generic `{Lock, Append, Serialize, Read, Parse,
//!   ProjectionDiagnostic}` error quartet (now sextet).
//!
//! # Design (eventsourced-inspired, associated types — NOT GATs)
//!
//! The [`EventSourced`] trait is modelled on the [`eventsourced`] crate's
//! eponymous trait but deliberately simpler: no `Command` type, no async,
//! no persistence-of-evolved-state. A domain provides its `Event` (with
//! [`EventEnvelope::sequence`]/[`EventEnvelope::at_unix`] accessors), its
//! `Projection` (the rebuildable read model), and the `apply` fold; the
//! associated types are plain `type` aliases, not GATs, so the trait works on
//! stable Rust 1.85 without lifetime gymnastics.
//!
//! [`eventsourced`]: https://docs.rs/eventsourced
//!
//! # Log separation preserved (ADR-0010)
//!
//! Each domain keeps its **own** log file, lock file, and projection — this
//! crate collapses the *mechanics*, not the *separation*. ADR-0010 (research
//! source ledger separate from memory) establishes that fusing two trust
//! domains into one event log is a Model-B class of bug; this crate honours
//! that by being parameterised over `log_relative_path` / `lock_relative_path`
//! per call, never baking a single shared log. The research-vs-memory split
//! stays; what goes is the triplicated `project_locked` body.
//!
//! # Macros
//!
//! [`event_envelope!`] generates the `sequence()`/`at_unix()` accessor
//! `impl` for a domain's `Event` enum (the most mechanical triplication). See
//! [`macros`] for why a declarative `macro_rules!` was chosen over a proc-macro
//! (Rust 2024H2/2025H1 alignment: zero build-time cost, no extra crate-type).

pub mod error;
pub mod lock;
pub mod macros;
pub mod projection;

use serde::de::DeserializeOwned;

pub use error::EventLogError;
pub use lock::{resolve_lock_path, EventLogLock};
pub use projection::{
    apply_event, next_sequence, now_unix, project_locked, replay, CODE_OUT_OF_ORDER_EVENT_IGNORED,
    CODE_TORN_FINAL_LINE_SKIPPED,
};
// Re-export the store durability tier so PEP callers can write
// `forge_core_eventlog::WalDurability` without a second dependency.
pub use forge_core_store::WalDurability;

use std::path::{Path, PathBuf};

/// The envelope every event in an `EventSourced` log carries.
///
/// All four copied `Event` enums (`MemoryEvent`, `ResearchEvent`,
/// `ArbitrationProjection`'s event, …) expose `sequence()` and `at_unix()`
/// accessors that pattern-match over variants to pull out the shared envelope
/// fields. This trait captures that shape once, so the generic
/// [`apply_event`]/[`project_locked`] mechanics can read the sequence for the
/// out-of-order guard. The [`event_envelope!`] macro generates the impl.
pub trait EventEnvelope {
    /// The per-log monotonic sequence number of this event. Used by the
    /// out-of-order guard in [`apply_event`] and by [`next_sequence`].
    fn sequence(&self) -> u64;

    /// `at_unix` timestamp (seconds since epoch). Audit metadata only.
    fn at_unix(&self) -> u64;
}

/// A domain whose state is derived from an append-only event log.
///
/// Implementors provide their `Event` type (with [`EventEnvelope`]
/// `sequence()`/`at_unix()` accessors), their `Projection` (the rebuildable
/// read model), their `Diagnostic` (the projection-level warning type), and the
/// `apply` fold. The event-log mechanics (cold read with torn-tail tolerance,
/// replay, sequence allocation, lock+append) are provided by this crate's free
/// functions and the [`EventLogLock`] wrapper.
///
/// # Why these associated types (not GATs)
///
/// `Event`, `Projection`, and `Diagnostic` are plain `type` aliases — the
/// trait works on stable Rust 1.85 with no lifetime parameters. A projection
/// may borrow domain data (e.g. `&'a str` keys) only if it owns it; the
/// `'static`-free design here just requires `Clone`, `Default`, and (for
/// `Event`) `Serialize` + `DeserializeOwned`.
pub trait EventSourced {
    /// The append-only event type. Must round-trip through serde (the log is
    /// NDJSON) and be `Clone` for the projection to hold onto event data.
    type Event: serde::Serialize + DeserializeOwned + Clone + EventEnvelope;

    /// The rebuildable read model. `Default` gives the empty projection (the
    /// starting point for [`replay`]); `Clone` so callers can snapshot it.
    type Projection: Default + Clone;

    /// The projection-level warning type (e.g. out-of-order event ignored,
    /// torn final line skipped). Carried in the projection's diagnostics and
    /// surfaced via [`EventLogError::ProjectionDiagnostic`].
    type Diagnostic: Clone;

    /// Fold one event into the projection (mutates in place). Called by
    /// [`apply_event`] only after the out-of-order guard passes, so
    /// implementations may assume `event.sequence() > projection.sequence`.
    fn apply(projection: &mut Self::Projection, event: &Self::Event);

    /// Record a projection-level diagnostic (e.g. out-of-order event ignored).
    /// Called by the generic [`apply_event`] and [`project_locked`] mechanics;
    /// implementations push onto the projection's diagnostics vector.
    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic);

    /// The highest sequence number applied so far. Read by [`next_sequence`]
    /// and by the out-of-order guard in [`apply_event`]. Making this a trait
    /// responsibility (rather than a caller-supplied closure) keeps
    /// `next_sequence(&projection)` signature-stable across domains.
    fn sequence_of(projection: &Self::Projection) -> u64;

    /// Advance the projection's sequence watermark to `new_sequence`. Called by
    /// [`apply_event`] after a successful [`apply`](Self::apply). Factored out
    /// because `Projection` is an opaque associated type; the domain knows
    /// which field is the watermark.
    fn advance_sequence(projection: &mut Self::Projection, new_sequence: u64);

    /// Build the out-of-order-event-ignored diagnostic (code
    /// [`projection::CODE_OUT_OF_ORDER_EVENT_IGNORED`]). Factored into the
    /// trait so the generic [`apply_event`] can construct a domain-typed
    /// diagnostic without knowing the domain's `Diagnostic` shape.
    fn diagnostic_out_of_order_event_ignored(
        event_seq: u64,
        projection_seq: u64,
    ) -> Self::Diagnostic;

    /// Build the torn-final-line-skipped diagnostic (code
    /// [`projection::CODE_TORN_FINAL_LINE_SKIPPED`]). Factored into the trait
    /// for the same reason; `line_number` is 1-based and `source` is the
    /// `serde_json::Error` from the failed parse.
    fn diagnostic_torn_final_line_skipped(
        line_number: usize,
        source: &serde_json::Error,
    ) -> Self::Diagnostic;
}

/// Serialize an event to JSON and append it as a line to the event log under
/// the given durability tier.
///
/// This is the shared body of the `append_bytes` shims copied across the PEP
/// crates. It serializes the event to bytes, deserializes back into a
/// [`serde_json::Value`] (to satisfy `append_json_line_with_durability`'s
/// `T: Serialize` bound without re-serializing through the domain type), then
/// routes through [`forge_core_store::append_json_line_with_durability`], which
/// owns all the framing / path / per-path lock / create-dir / flush / sync
/// conventions. The extra serialize pass is the documented trade — correctness
/// and convention-adherence over micro-optimization (event logs are low-volume,
/// human-scale).
///
/// `lock` is taken as a `&EventLogLock` **capability witness**: this function
/// does not itself acquire the domain lock, but its signature makes "the caller
/// must already hold the lock across read-sequence-then-write" loud at the type
/// level. (The store helper takes its own separate per-path lock internally;
/// the two compose — see [`EventLogLock`].)
///
/// # Errors
///
/// Returns [`EventLogError::Serialize`] if the event cannot be serialized to
/// JSON (or the bytes cannot be re-parsed as a `Value`); [`EventLogError::Append`]
/// if the store helper fails to write/flush/sync.
pub fn append_event<E>(
    root: &Path,
    log_relative_path: &str,
    event: &E::Event,
    durability: WalDurability,
    _lock: &EventLogLock,
) -> Result<PathBuf, EventLogError<E::Diagnostic>>
where
    E: EventSourced,
{
    // Serialize once, then re-wrap as a Value for the store helper.
    let serialized = serde_json::to_vec(event).map_err(|source| EventLogError::Serialize {
        source: source.to_string(),
    })?;
    let value: serde_json::Value =
        serde_json::from_slice(&serialized).map_err(|source| EventLogError::Serialize {
            source: source.to_string(),
        })?;
    forge_core_store::append_json_line_with_durability(root, log_relative_path, &value, durability)
        .map_err(|source| EventLogError::Append {
            path: root.join(log_relative_path),
            source: source.to_string(),
        })
}

#[cfg(test)]
mod tests;
