# research-v0 — F14 Knowledge Orchestration fixtures

Canonical fixtures for the F14 (Knowledge Orchestration) E2E suite, the P3
research-mode feature last in the v0.1.0 → 10/10 community-features front.

## Contents

- `citation-scenario.yaml` — the "claim side" of the F14 evidence model: a
  policy-shaped document carrying `source_id` occurrences. Used by
  `crates/forge-core-cli/tests/research_cli_e2e.rs` to exercise the citation
  validator's source_id walk. The `research.source.swe-agent` id resolves in
  the runtime Source Ledger after `research source add`; `ghost.unregistered`
  is the unresolved control.

Companion fixtures live in `contracts/examples/`:

- `research-source.yaml` — a valid `ResearchSource` (admitted by the gate).
- `research-policy.yaml` — a `ResearchPolicy` (admits all kinds; requires
  content_hash + trace_ref).

## What the E2E proves

The full research loop, end-to-end against the real binary:

1. `research source add` admits the fixture source → `Admitted`, sequence 1.
2. `research source list` → the source appears, live.
3. `research cite --source-id research.source.swe-agent` → resolved in the
   `runtime` backing.
4. `research cite --source-id ghost.unregistered` → `RejectedByGate` (unresolved).
5. `research check` with the citation scenario → the unresolved id is flagged.
6. `research graph` → the evidence graph indexes the citing claim.

These fixtures are permanent (committed, not tempdir-generated) so the E2E is
deterministic and reproducible outside CI. They do not enter the
`forge-core validate` regression anchor (anchor 122): the citation check is
not wired into `run_validate` (it is opt-in via `research check` /
`--require-citation`), so adding these fixtures leaves the anchor unchanged.
