# ADR-0010 - The research Source Ledger is a log separate from memory

- **Status**: Accepted (F14.2 implemented — crate `forge-core-research` + contract `ResearchSource`/`ResearchPolicy` + PDP `ResearchContract::can_admit_source` + append-only PEP)

## Context

F14 (Knowledge Orchestration mode) needs to track sources that back
research agent claims at runtime (paper, URL, local doc with `fetched_at`,
`content_hash`, `trace_ref`). The repo already has:

- `FieldEvidenceRegistry` + `EvidenceSource` (`forge-core-contracts/src/evidence.rs`)
  as a curated/static `ContractDocument` backing **Forge's own design
  decisions** (validated at anchor 122).
- `validate_yaml_source_id_references` (`forge-core-validate/src/lib.rs:280`),
  which already rejects an unknown `source_id` against the registry.

The temptation would be to merge the runtime source ledger into
`forge-core-memory` (treat `ResearchSource` as a new `MemoryEvent`, reuse one
log, one lock, one projection) to save boilerplate (aligned with F15).

## Decision

The F14 Source Ledger lives in **its own log** (`<state_root>/research/sources.ndjson`,
lock `locks/research.sources.lock`, projection `ResearchProjection`), in a new
crate `forge-core-research`, **mirroring the PEP pattern of `forge-core-memory`**.
It is not a `MemoryEvent` kind; it does not share a log/projection/lock with memory.

## Rationale (the real trade-off)

Reusing the memory log (the alternative considered) reintroduces the Model B
bug class one layer down: it merges **trust** (the Authority/Review axes of F06,
"this is actionable ground-truth") with **citation provenance** ("this points
to a source") in a single event-sourced log. Mixing the two semantics in a
`MemoryProjection::apply_event` violates the orthogonality that ADR-0023 pinned
for memory and reopens the memory/citation poisoning surface.

The cost is boilerplate: one more crate, one more lock, one more projection
(against the F15 NFR of less manual pain). Accepted because preserving the
semantic boundary between trust and citation is the entire product of F14 —
losing the boundary to gain conciseness trades excellence for convenience.

## Consequences

- `forge-core-research` crate with an append-only PEP templated on memory (admit
  source under `ResearchPolicy`, rebuildable projection, deterministic replay).
- Citation check resolves `source_id` against the **backing union**:
  `FieldEvidenceRegistry` (curated) union Source Ledger (runtime); fail-closed
  if it resolves in neither. It extends `validate_yaml_source_id_references`,
  it does not duplicate it.
- `EvidenceGraph` is not a first-class type nor populated by the agent: it is a
  projection `SourceId -> citing claims`, computed by walking artifacts (same
  pattern as the `reference_index` of `forge-core-store`).
- A research claim is polymorphic: any node that carries `source_id`. F14
  defines the source side, not a new claim type (avoids type inflation and
  respects the deletion test).
- Fixtures in `docs/fixtures/research-v0/` + `contracts/examples/research-source.yaml`.
- Does not block F08 (MCP): F14 is citation semantics orthogonal to transport.
  Exposure via MCP becomes a post-merge story of both.

## Anti-goals

- Do not reimplement the store PEP: `forge-core-research` composes with
  `forge-core-store` (`append_json_line_with_durability`,
  `acquire_effect_store_lock`), it does not duplicate it.
- Do not force workflow-graph semantics on the evidence graph (different
  domain).
- Do not opine on source tier/quality in the citation gate MVP: the gate
  attests only to **resolution** of the `source_id`; tier-min is future policy
  (a separate trust axis, analogous to the Review axis of F06).
- Do not introduce `research run` (linear pipeline): the "research mode" is the
  active `ResearchPolicy`, not a flow. G1 anti-novel-script.
