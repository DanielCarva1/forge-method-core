//! The cold-read replay and sequence/timestamp primitives.
//!
//! Generalized from `forge-core-memory::lib.rs` (the canonical copy) and
//! `forge-core-research::lib.rs` (its near-identical twin), which were
//! byte-for-byte the same except for the domain types. The out-of-order guard,
//! the Fowler "discard and rebuild" fold, and the torn-tail NDJSON cold read
//! live here once.
//!
//! # The torn-tail fix
//!
//! `project_locked` hoists the `total_lines` count **outside** the per-line
//! loop. The memory crate's copy recomputed `text.lines().count()` on every
//! parse error (a latent O(n²) when the tail is torn and every line errors);
//! the research crate already hoisted it. We use the fixed version.

use std::path::Path;

use crate::{EventEnvelope, EventLogError, EventSourced};

/// A standard projection-diagnostic code: the event's sequence was `<=` the
/// projection's already-applied sequence, so it was ignored rather than
/// regressing state (defence against a partial re-read / duplicate writer).
pub const CODE_OUT_OF_ORDER_EVENT_IGNORED: &str = "out_of_order_event_ignored";

/// A standard projection-diagnostic code: the final line of the log was
/// incomplete (a torn write from a crashed appender) and was skipped.
pub const CODE_TORN_FINAL_LINE_SKIPPED: &str = "torn_final_line_skipped";

/// Fold one event into a projection, applying the **out-of-order guard** that
/// every copied PEP shares.
///
/// If the event's [`EventEnvelope::sequence`] is `<=` the projection's current
/// sequence ([`EventSourced::sequence_of`], and the projection is non-empty),
/// the event is ignored and a diagnostic is recorded via
/// [`EventSourced::record_diagnostic`] (built by
/// [`EventSourced::diagnostic_out_of_order_event_ignored`]) — a partial re-read
/// must never regress state. Otherwise the domain's [`EventSourced::apply`]
/// fold runs and the projection's sequence is advanced.
///
/// This is the shared body of the `MemoryProjection::apply_event` /
/// `ResearchProjection::apply_event` / `ArbitrationProjection::apply_event`
/// methods that were triplicated. Domains keep a thin `apply_event` wrapper
/// that delegates here if they want the same observable behaviour, or call it
/// directly.
pub fn apply_event<E>(projection: &mut E::Projection, event: &E::Event)
where
    E: EventSourced,
{
    let event_seq = event.sequence();
    let current = E::sequence_of(projection);
    if event_seq <= current && current > 0 {
        // Out-of-order / duplicate — do not regress. Diagnose and continue.
        E::record_diagnostic(
            projection,
            E::diagnostic_out_of_order_event_ignored(event_seq, current),
        );
        return;
    }
    E::apply(projection, event);
    E::advance_sequence(projection, event_seq);
}

/// Replay a stream of events into a fresh projection (Fowler: discard and
/// rebuild the read model from the event log). Events are applied in order via
/// [`apply_event`] (so the out-of-order guard is in force); the resulting
/// projection's sequence is the max applied sequence.
///
/// This does NOT read the file — it is the pure fold used by both
/// [`project_locked`] (cold read) and tests. File-reading + torn-line
/// tolerance lives in [`project_locked`].
#[must_use]
pub fn replay<E>(events: impl IntoIterator<Item = E::Event>) -> E::Projection
where
    E: EventSourced,
{
    let mut projection = E::Projection::default();
    for event in events {
        apply_event::<E>(&mut projection, &event);
    }
    projection
}

/// Read the event log at `<root>/<log_relative_path>` and rebuild the
/// projection by replay — **without** acquiring the lock. For callers that
/// already hold the [`crate::EventLogLock`] (the PEP entry points). Does not
/// mutate the filesystem.
///
/// # Torn-final-line tolerance
///
/// A trailing line that fails to parse as JSON is skipped with a diagnostic
/// (built by [`EventSourced::diagnostic_torn_final_line_skipped`], mirroring
/// `claim_wal.rs`'s `last_good_offset` recovery). A line that parses as JSON
/// but fails to deserialize as the domain's `Event` is a hard
/// [`EventLogError::Parse`] (it indicates schema drift, not a torn write).
///
/// # Errors
///
/// Returns [`EventLogError::Read`] if the log file cannot be read (other than
/// `NotFound`, which yields an empty projection); [`EventLogError::Parse`] if a
/// well-formed JSON line fails to deserialize as the domain's `Event`.
pub fn project_locked<E>(
    root: &Path,
    log_relative_path: &str,
) -> Result<E::Projection, EventLogError<E::Diagnostic>>
where
    E: EventSourced,
{
    let log_path = root.join(log_relative_path);
    let bytes = match std::fs::read(&log_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // No log yet — empty projection. Not an error.
            return Ok(E::Projection::default());
        }
        Err(source) => {
            return Err(EventLogError::Read {
                path: log_path,
                source: source.to_string(),
            });
        }
    };
    let text = String::from_utf8_lossy(&bytes);

    // FIX (vs the memory crate's copy): hoist the line count OUTSIDE the loop.
    // The memory version recomputed `text.lines().count()` on every parse error
    // — O(n²) when a torn tail causes repeated errors. The research version
    // already hoisted it; we use the fixed shape.
    let total_lines = text.lines().count();

    let mut projection = E::Projection::default();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let event: E::Event = match serde_json::from_str(line) {
            Ok(event) => event,
            Err(source) => {
                // Distinguish a torn final line (JSON incomplete) from a
                // structurally-valid-JSON-but-wrong-shape line. The former is
                // skipped with a diagnostic; the latter is a hard error.
                let is_last = idx + 1 >= total_lines;
                let looks_torn =
                    is_last && (line.is_empty() || !line.starts_with('{') || !line.ends_with('}'));
                if looks_torn {
                    E::record_diagnostic(
                        &mut projection,
                        E::diagnostic_torn_final_line_skipped(idx + 1, &source),
                    );
                    break;
                }
                return Err(EventLogError::Parse {
                    path: log_path.clone(),
                    line_number: idx + 1,
                    source: source.to_string(),
                });
            }
        };
        apply_event::<E>(&mut projection, &event);
    }
    Ok(projection)
}

/// The next sequence number to write, given a projection. Computed under the
/// lock (by the caller) so concurrent writers cannot both pick the same
/// sequence. Sequence starts at 1 (the empty log has projection sequence == 0).
/// Saturates at `u64::MAX` rather than overflowing.
///
/// This reads the projection's current sequence via
/// [`EventSourced::sequence_of`], so it works for any `EventSourced` domain
/// without the caller having to know the projection's field name — answering
/// the spec's "how does `next_sequence` read the projection's sequence" by
/// making it a trait responsibility rather than a caller-supplied closure.
#[must_use]
pub fn next_sequence<E>(projection: &E::Projection) -> u64
where
    E: EventSourced,
{
    E::sequence_of(projection).saturating_add(1)
}

/// Best-effort wall-clock seconds since the UNIX epoch. Tests override this by
/// passing `at_unix` directly to a PEP; production callers use this. Returns 0
/// on clock failure (fail-closed to a deterministic value rather than
/// panicking — the timestamp is audit metadata, not a security boundary).
///
/// Byte-for-byte identical to the copies in `forge-core-memory` /
/// `forge-core-research` / `forge-core-governance`.
#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
