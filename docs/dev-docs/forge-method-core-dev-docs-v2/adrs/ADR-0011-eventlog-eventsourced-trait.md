# ADR-0011 - `forge-core-eventlog` and the `EventSourced` trait

- **Status**: Accepted (V1.A implemented — crate `forge-core-eventlog` with `trait EventSourced`, `EventEnvelope`, `event_envelope!`; V2.A migrated `forge-core-memory`/`-research`/`-governance`/`-store` JSONL half)
- **Date**: 2026-07-02
- **Track**: V1.A / V2.A — event-sourcing boilerplate collapse
- **Supersedes**: none
- **Superseded by**: none

## Context

Four crates in the workspace had copied nearly identical event-sourcing boilerplate:
`forge-core-memory`, `forge-core-research`, `forge-core-governance`, and the JSONL
half of `forge-core-store`. Each had: an `<X>Event` enum with a `sequence`/`at_unix`
envelope, an `<X>Projection { sequence, BTreeMap, superseded, diagnostics }`, an
`apply_event` with an out-of-order guard, a free `replay` fold,
`project`/`project_locked` (cold-read NDJSON with torn-tail tolerance),
`next_sequence`, `now_unix`, an `append_bytes` shim, and a quartet of
`{Lock, Append, Serialize, Read}` errors. Measurement in `forge-core-research`
showed that ~62% of the crate was the template, not the domain.

The temptation would be to merge the logs into a single shared log to save code. But
ADR-0010 pinned that **logs must remain separate** — merging distinct trust domains
(memory = trust; research = citation provenance) into a single event-sourced log
reopens the Model B bug class that ADR-0023 made unrepresentable. What was duplicated
were the **mechanics**, not the **separation**.

There was also a latent bug: the `forge-core-memory` copy recomputed
`text.lines().count()` inside the parse loop, on every error — O(n²) when the tail
was torn and every line errored. The `forge-core-research` copy had already hoisted
the count outside; the two copies diverged silently.

## Decision

A new crate `forge-core-eventlog` that absorbs the mechanics (not the separation). The
heart is the `EventSourced` trait:

```rust
pub trait EventSourced {
    type Event: Serialize + DeserializeOwned + Clone + EventEnvelope;
    type Projection: Default + Clone;
    type Diagnostic: Clone;
    fn apply(projection: &mut Self::Projection, event: &Self::Event);
    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic);
    fn sequence_of(projection: &Self::Projection) -> u64;
    fn advance_sequence(projection: &mut Self::Projection, new_sequence: u64);
    fn diagnostic_out_of_order_event_ignored(...) -> Self::Diagnostic;
    fn diagnostic_torn_final_line_skipped(...) -> Self::Diagnostic;
}
```

The crate provides the generic mechanics over this trait:

- `replay` — Fowler's pure fold (discard and rebuild).
- `project_locked` — cold-read NDJSON with torn-tail tolerance, no lock.
- `apply_event` — the shared fold body with an out-of-order guard.
- `next_sequence` / `now_unix` — sequence allocation and wall-clock.
- `append_event` — serialize → `append_json_line_with_durability` (reuse of `forge-core-store`).
- `EventLogLock` — RAII wrapper over `acquire_effect_store_lock`.
- `EventLogError<D>` — the sextet `{Lock, Append, Serialize, Read, Parse, ProjectionDiagnostic}`, generic over the domain's `Diagnostic` type (default `String`).

The **associated types are plain `type` aliases, not GATs** — the trait works on stable Rust
1.85 with no lifetime gymnastics. This is the eventsourced/evented pattern (hseeberger's
`eventsourced` is the model, but deliberately simpler: no `Command` type, no async, no
evolved-state persistence).

The `event_envelope!` macro generates the `sequence()`/`at_unix()` accessors + the
`EventEnvelope` impl for the domain's `Event` enum. It is `macro_rules!`, **not a
proc-macro** — zero build-time cost, no extra `*-derive` crate, aligned with the Rust
Project Goal 2025H1 direction of reducing proc-macro build cost. The `apply` fold stays
hand-written (it is domain-specific).

## Rationale (the real trade-off)

The alternative — merging the logs into one — was rejected by ADR-0010: the semantic
boundary between trust domains is the entire product of F14. This ADR collapses the
**mechanics**
(triplicated `project_locked`, the error quartet ×7, `event_envelope` ×N) while leaving
each domain with its own log file, lock, and projection. The generic `project_locked` is
parameterized by `log_relative_path`/`lock_relative_path` per call — it never assumes a
shared log.

The memory O(n²) bug is fixed in the single `project_locked`: the `total_lines` count is
hoisted outside the loop. The four copies can no longer diverge on how to count lines or
how to handle the torn tail.

## Consequences

**Positive:**

- The mechanics collapse into a single place. A 5th PEP (Policy Enforcement Point) becomes
  a PDP (the `apply` function) + 2 arms of `apply_event`, not a ~1200-line crate.
- The memory O(n²) bug is fixed in the single `project_locked` copy; all migrated domains
  inherit the fix.
- The error quartets ×7 become a single generic sextet; the domain's `Diagnostic` type
  travels through `EventLogError<D>` to the boundary.
- ADR-0010 is honored byte for byte: each domain keeps its log, lock, and projection. This
  crate collapses mechanics, not separation.

**Negative:**

- `EventSourced` has "factored out" methods (`sequence_of`, `advance_sequence`, the
  `diagnostic_*`) that expose projection details to the trait. Accepted because the
  `Projection` is an opaque associated type; the domain knows which field is the watermark,
  the trait does not.
- `append_event` does a double serialize (event → bytes → `Value` → store helper). Documented
  trade-off: correctness and adherence to store conventions over micro-optimization (event
  logs are low-volume, human-scale).

## Anti-goals

- **Does not** merge logs: each domain keeps its file, lock, and projection (ADR-0010).
- **Does not** introduce a `Command` type or async — the kernel stays deterministic (ADR-0001).
- **Is not** a proc-macro: `event_envelope!` is `macro_rules!` by design.

## References

- `eventsourced` (Heiko Seeberger): https://docs.rs/eventsourced
- `evented` (successor to eventsourced): https://docs.rs/evented
- Capital One — building an event-sourcing crate (case study):
  https://www.capitalone.com/tech/software-engineering/event-sourcing-implementation/
- Rust Project Goals 2025H1 (reduce proc-macro build cost).
- In-repo: ADR-0010 (log separation honored), ADR-0001 (deterministic kernel, no async),
  `crates/forge-core-eventlog/src/{lib,projection,error,lock,macros}.rs`.
