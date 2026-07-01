# ADR 0003 — `forge-core-memory`: the PEP for the memory trust model

**Status**: Accepted (2026-07-01)
**Supersedes**: none · **Extends**: [ADR 0002](0002-memory-trust-model.md) (the PDP/PEP split)
**Decides**: the architecture of the `forge-core-memory` crate (F06.3).

## Context

ADR 0002 (Candidato 1) established the PDP/PEP separation for the memory trust
model: the pure decision functions `can_admit` / `can_promote` / `mark_stale`
live in `forge-core-contracts`; a separate Policy Enforcement Point performs
the mutation **atomically**, closing the TOCTOU gap between decide and write
(CWE-367 — "atomicity at the use site, not check-fusion"). This ADR decides
how that PEP is built.

The question is not *whether* to build a PEP (ADR 0002 already mandated one),
but *how*: invent the storage/locking/durability machinery, or compose the
primitives the repo already provides.

## Decision

**`forge-core-memory` is a composition crate.** It reuses
`forge-core-store`'s primitives verbatim and mirrors `claim_wal.rs`'s
event-sourcing pattern; it invents nothing at the storage layer. The decision
is grounded in both the external literature and — decisively — the repository's
own established conventions.

### 1. Lock strategy — exclusive `fs4` OS file lock held across decide-and-write

`acquire_effect_store_lock(state_root, MEMORY_LOCK_RELATIVE_PATH)` is acquired
before the PDP call and released (RAII `Drop`) after the append + projection
commit. This is CWE-367's "atomicity at the use site" made concrete. The
lock already exists in `forge-core-store/src/lib.rs:1041` with bounded-backoff
retry and RAII release — reinventing it would diverge from house convention.

**Re-entrancy constraint (a correctness subtlety discovered during
implementation):** `fs4` locks are NOT re-entrant. A PEP that acquires the
lock and then calls a helper that *also* acquires it self-deadlocks (returns
`WouldBlock`). Therefore `project()` (which locks) and `project_locked()`
(which does not) are separate functions: the PEP entry points hold the lock
and call `project_locked`; standalone callers use `project`. This split is the
codified fix and is documented at both call sites.

### 2. Append-only JSONL event log + rebuildable projection (event sourcing)

The source of truth is `<state_root>/memory/events.ndjson` — append-only JSONL,
one `MemoryEvent` per line (`Admitted` / `Promoted` / `Forgotten`). The
`MemoryProjection` (`entry_id → current entry`, `superseded` set) is a
**disposable read model** rebuilt by replay (Fowler event-sourcing: "discard
and rebuild the projection"). Last-event-wins per `entry_id`, mirroring
`claim_wal.rs`'s `apply_record`.

JSONL (not the binary CRC32C framing of the claim WAL) is the right grain for
memory events: human-auditable, matches the `serde_json` + `yaml_serde` house
convention, and memory-event volume is low (human-scale). Binary framing is
reserved for the high-throughput claim WAL.

A torn final line is skipped with a `torn_final_line_skipped` diagnostic
(mirrors `claim_wal.rs`'s `last_good_offset` recovery); a well-formed-JSON
line that fails to deserialize as a `MemoryEvent` is a hard
`MemoryProjectionError::Parse` (schema drift, not a torn write).

### 3. Lazy TTL sweep on read (no background thread)

`list_now(now_unix)` calls `MemoryContract::mark_stale(now)` (the pure,
already-tested PDP from Candidato 1) under the read lock, persists any
newly-flipped `stale` flags by appending corrective `Admitted` events, and
returns only the non-stale entries. This is the Redis passive-expiry model
("a key that has expired will simply be ignored when accessed") — no daemon.

A background TTL thread is explicitly **forbidden**: it would be a second
writer that must be reconciled with the lock discipline, reintroducing the
stale-read race (Algomaster; CWE-367) the under-lock sweep eliminates.

### 4. Before-image on forget (audit + reversibility-by-replay)

A `Forgotten` event carries the **full** prior `MemoryEntry` plus a
`"sha256:{hex}"` content hash. ADR 0002 required recording the prior
`(authority_level, review_state)`; the Debezium `before` / Postgres
`REPLICA IDENTITY FULL` / in-repo `EffectWalOriginal` precedent says capture
the *whole* prior entry — this doubles the log as the audit trail and makes
a forget reversible-by-replay (drop the event, re-project).

### 5. Per-operation hand-rolled error enums (no `anyhow`, no `thiserror`)

`AdmitError`, `PromoteError`, `ForgetError`, `MemoryProjectionError` — each
`#[derive(Debug, Clone, PartialEq, Eq)]`, struct variants `{path, source}`
with lossy `String` sources at crate boundaries, manual `Display`, empty
`Error` impl. Mirrors `ClaimWal*Error` exactly. A single mega-enum is rejected
(it accumulates phantom variants and defeats exhaustive matching — redb keeps
per-operation enums for the same reason).

### 6. The PEP never re-evaluates policy

`admit` calls `MemoryContract::can_admit`; if `Blocked`, returns
`AdmissionStatus::DeniedByGate(reasons)` and **appends nothing**. `promote`
calls `can_promote`; `Blocked` ⇒ `PromoteStatus::DeniedByGate`. This is the
ADR-0002 Decision-1 invariant (Cedar / OPA / XACML: the PEP only enforces; it
does not re-evaluate thresholds). A denial is an outcome, never an error.

## Alternatives considered

- **In-process `std::sync::Mutex` instead of an OS file lock.** Rejected: it
  does not survive cross-process (the CLI is invoked as fresh processes; two
  `forge-core memory ingest` invocations are two processes). The `fs4` OS lock
  is the only primitive that serializes across processes — which is the threat
  model (concurrent CLI invocations, not just threads).
- **Background TTL sweeper thread.** Rejected (§3): stale-read race, second
  writer, lock-reconciliation complexity. Redis passive-expiry proves it is
  unnecessary.
- **A single crate-level error enum.** Rejected (§5): phantom variants,
  defeats exhaustive matching.
- **Binary CRC32C framing for the memory log.** Rejected (§2): over-engineered
  for low-volume human-scale events; JSONL matches house convention. Reserved
  for the claim WAL where throughput justifies it.
- **In-place mutation of the event log.** Rejected: "the dataset only grows"
  (rerun.io). In-place edits destroy auditability and replay-determinism — the
  two properties that make the projection disposable/rebuildable.

## Consequences

- **Auditability**: every state change is an immutable, replayable event; the
  projection can be discarded and rebuilt deterministically (pinned by the
  `replay_is_deterministic` proptest — the Fowler replay guarantee).
- **Durability is a knob, not a hardcode**: every append routes through
  `append_json_line_with_durability`; production uses `SyncOnAppend`, tests/benches
  use `NoSync` (ADR-0009, already in `forge-core-store`).
- **No daemon**: the store is passive; correctness does not depend on a
  background process being alive.
- **Cross-process safe**: the OS lock serializes concurrent CLI invocations.
- **CLI-ready**: the public API (`admit` / `promote` / `forget` / `list_now` /
  `project`) returns typed result structs (`AdmissionResult`, `PromoteResult`,
  `ForgetResult`, `ListResult`) shaped for the `CliEnvelope` dual-output pattern;
  F06.7's `forge-core memory` verbs will be thin wrappers (ADR-0002 Decision-1).

## Scope (this story vs. the next)

**In (F06.3, this commit):** the `forge-core-memory` crate — PEP engine, event
log, rebuildable projection, per-operation error enums, 29 unit + property
tests, and this ADR.
**Out (separate stories):** the CLI verbs (F06.7 — 5 subcommands × JSON+text
dual output) and the fixtures/E2E harness (F06.8). The crate's public API is
designed so the CLI is a thin wrapper.

## References

- [ADR 0002](0002-memory-trust-model.md) — the PDP/PEP split this crate enforces.
- CWE-367 (TOCTOU) — https://cwe.mitre.org/data/definitions/367.html
- Martin Fowler — Event Sourcing — https://martinfowler.com/eaaDev/EventSourcing.html
- Martin Fowler — CQRS — https://martinfowler.com/bliki/CQRS.html
- Redis — passive (lazy) key expiry — https://redis.io/docs/reference/eviction/
- rerun.io datastore ("the dataset only grows") — https://rerun.io
- redb error design (per-operation enums) — https://docs.rs/redb/latest/redb/enum.Error.html
- Debezium `before`/`after` (before-image on delete) — https://debezium.io/documentation/
- PostgreSQL `REPLICA IDENTITY FULL` — https://www.postgresql.org/docs/current/logical-replication-publication.html
- In-repo (primary, decisive): `crates/forge-core-store/src/lib.rs`
  (`acquire_effect_store_lock`, `append_json_line_with_durability`,
  `WalDurability`, `EffectWalOriginal`); `crates/forge-core-store/src/claim_wal.rs`
  (`ProjectionAccumulator` / `apply_record` / torn-write recovery /
  `ClaimWal*Error` per-op enums); ADR-0009 (durability knob).
