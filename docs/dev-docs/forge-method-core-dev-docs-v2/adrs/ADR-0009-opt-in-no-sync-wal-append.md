# ADR-0009: Opt-in `--no-sync` for WAL append (tiered durability)

- **Status**: Accepted (amended 2026-06-30: `query-effect-index` excluded — read-only)
- **Date**: 2026-06-30
- **Track**: F15.7 — Rust ergonomics + perf
- **Supersedes**: none
- **Superseded by**: none

## Context

The Forge Method WAL (`crates/forge-core-store/src/claim_wal.rs`) and the
generic JSONL append path (`append_json_line` in
`crates/forge-core-store/src/lib.rs`) call `sync_data()` / `sync_all()` on
every single record append to guarantee post-crash durability.

Benchmarking (see `r6_benchmarks.md` in the Forge-method-archive sibling
repo's dev-journals)
showed that on Windows the `fsync` dominates the append cost: a single append
takes ~32ms typical, of which 25–50ms (with 300ms spikes) is the `fsync` itself.
On Linux SSD the same call costs 5–15ms.

This is **not a bug**: the WAL genuinely needs `fsync` to honour its durability
contract — without it, a power loss after `append_json_line` returns `Ok` could
lose the record, which would corrupt the claim ledger and the effect index.

However, there are three legitimate callers that do **not** need that
durability guarantee on every append:

1. **Benchmarks** (`cargo bench`) — measuring throughput of the parse / verify
   hot path; the WAL append is fixture setup, not the subject under test.
2. **Integration tests** (`cargo test`) — assertions check correctness, not
   crash recovery; a test process that crashes mid-run has already failed.
3. **Local dev loops** — an agent iterating on a feature can trade durability
   for iteration speed when it explicitly accepts the risk.

Forcing these three callers to pay 25–50ms per append makes the benchmark suite
and the test suite artificially slow (the workspace test run is dominated by
claim/append tests that do tens or hundreds of appends each).

## Decision

Introduce **tiered durability** as an explicit, opt-in knob:

1. A new internal store config, `WalDurability`, carried alongside the WAL
   append path. It has one variant today: `WalDurability::SyncOnAppend` (the
   default, current behaviour) and `WalDurability::NoSync` (skip `sync_data` /
   `sync_all` on the append path).

2. A new CLI flag `--no-sync`, accepted by every state-bearing command that
   touches the WAL or the JSONL effect log (`claim *`, `execute-operation`,
   `rebuild-effect-index`). When present, the command constructs the store
   with `WalDurability::NoSync`.

   `query-effect-index` is intentionally excluded from this list: it is
   read-only and the flag would be a no-op. The usage text and command
   registry do not advertise `--no-sync` for it.

3. The default — when `--no-sync` is **absent** — is unchanged:
   `WalDurability::SyncOnAppend`. Existing tests, existing users, and existing
   scripts continue to get full durability.

4. The `--no-sync` flag MUST print a one-line stderr warning the first time it
   is honoured, naming the command and stating that WAL appends are not durable
   for the duration of the process. This makes misuse visible in CI logs.

5. Benchmarks and integration tests in this repo are migrated to use
   `WalDurability::NoSync` where they do not assert on durability semantics.
   Tests that **do** assert on recovery after a simulated crash keep the
   default.

## Alternatives considered

- **Do nothing.** Keep paying 25–50ms per append in every benchmark and test.
  Rejected: the workspace test run is several minutes long and is dominated by
  this cost, which slows every iteration.

- **Batch N appends into one `fsync`.** Group appends and sync once per batch.
  Rejected for now: it changes the durability contract (the caller no longer
  knows the record is durable when `append` returns `Ok`) and complicates
  recovery (partial batches after crash). It is a real optimisation but a
  bigger system-design change; it can be layered on top of this ADR later if
  needed.

- **Async `fsync` on a background thread.** Rejected: it complicates recovery
  (the WAL file on disk may lag behind what callers believe was committed) and
  introduces a shutdown-ordering hazard. The opt-in `--no-sync` gives the same
  throughput win for benchmarks without touching the recovery path.

- **Make `--no-sync` a global env var instead of a per-command flag.** Rejected:
  a flag is explicit per-invocation, so a CI run cannot accidentally inherit a
  global "no sync" setting from the shell. The flag appears in the command
  invocation that gets logged, which is auditable.

## Consequences

**Positive:**

- Benchmark and test suites that do not assert on durability become ~25–50ms
  faster per append, which compounds across hundreds of appends into minutes
  saved per workspace test run.
- The durability contract of the default path is preserved byte-for-byte; no
  existing caller changes behaviour.
- The knob is discoverable: `--help` mentions it, and the first use prints a
  warning.

**Negative:**

- A new concept (`WalDurability`) is now part of the store's public surface.
  Callers must understand it to use it correctly. Mitigation: the default is
  safe; only explicit opt-in changes behaviour.
- A user who runs `--no-sync` in production and then suffers a power loss will
  lose the un-`fsync`ed tail of the WAL. Mitigation: the flag name is
  unambiguous, the help text warns, and the runtime prints a stderr warning.
- Slightly more parameter threading through the store → runtime → CLI layers.
  Accepted as the cost of an explicit seam.

## Non-goals

- This ADR does **not** change the recovery path. Recovery continues to scan
  the WAL byte-for-byte; whether each record was `fsync`ed before the crash is
  irrelevant to recovery (the record is either there or it is not).
- This ADR does **not** introduce batched appends or async fsync. Those remain
  open as future system-design changes if the per-append cost becomes
  unacceptable even with the opt-out.
