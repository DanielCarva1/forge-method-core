# Forge Core M1 subagent implementation plan

Status: planned, not started
Date: 2026-06-29
Depends on: `docs/planning/forge-core-m0-reconciliation.md`
Target branch after M0 closes: `codex/forge-m1-kernel`

## M1 goal

Build a thin, deterministic, additive vertical for:

- `forge-core preview`
- `forge-core ready`
- canonical `TraceEvent` v0 NDJSON
- `forge-core explain --last-run`

M1 should make the current runtime planner predictable, auditable, and user-explainable before WorkflowGraph or eval compare are implemented.

## Non-goals

Do not include these in M1:

- WorkflowGraph runtime/executor.
- Eval compare harness.
- Memory policy implementation.
- MCP/A2A protocol adapters.
- Full `clap` migration.
- Store module split.
- `thiserror`/`anyhow`.
- Broad tracing dependency rollout.

## Design constraints

- Respect `AGENTS.md`: manual named error enums; no `anyhow`; no `thiserror`.
- Validation accumulates diagnostics instead of short-circuiting.
- Resolve project state via `forge-core project resolve`; do not assume local `.forge-method` except explicit bootstrap core exception.
- Keep CLI thin; policy belongs in runtime/contracts/store.
- Prefer additive public APIs so existing tests and consumers keep compiling.
- Use optional serde fields for trace metadata to preserve old NDJSON compatibility.
- Full workspace compile/test happens only after lane-level implementation and reviewer passes.

## Proposed M1 semantics

### `preview`

Inputs:

- `--root <path>` default `.`
- `--operation <path>` required
- `--json` default true, with existing envelope style where possible

Output should be deterministic and include:

- status
- operation id/ref
- touched refs / target refs
- commands that would run
- effects that would stage/apply
- gates and blockers
- risk/destructive summary
- rollback/inverse availability where known
- next human action
- trace id/run id if trace is enabled for preview

Rules:

- Must not mutate state.
- Must fail closed on invalid operation or unresolved refs.
- Must not execute commands/effects.

### `ready`

Inputs:

- `--root <path>` default `.`
- optional `--operation <path>`
- optional `--json`

Output should aggregate readiness gates and return non-zero when not ready.

Rules:

- Fail closed: unknown, missing, pending, skipped, or structurally invalid gates are not ready.
- Ready is not just validation success; it must report concrete reasons/evidence.
- If scoped to an operation, readiness uses the runtime planner and store snapshot.
- If repo-level, readiness can initially report supported checks and known blockers, but must be explicit about unsupported checks rather than greenwashing.

### `TraceEvent` v0

Initial crate boundary:

`crates/forge-core-trace/`

Fields should start from imported schema:

- `schema_version`
- `kind`
- `trace_id`
- `event_id`
- `run_id`
- `graph_id`
- `node_id`
- `event_kind`
- `recorded_at`
- `actor`
- `authority`
- `inputs`
- `outputs`
- `risk`
- `cost`
- `message`

Reconcile with sidecar:

- include or derive `project_id`
- persist under resolved `state_root`
- no consumer repo local `.forge-method` writes unless resolver says so

Initial event kinds for M1:

- `run_started`
- `operation_planned`
- `preview_completed`
- `ready_completed`
- `gate_passed`
- `gate_blocked`
- `effect_staged`
- `effect_applied`
- `run_completed`
- `run_failed`

### `explain --last-run`

Inputs:

- `--root <path>` default `.`
- `--last-run`
- optional `--run-id <id>` if cheap to add
- optional `--json`

Rules:

- Reads trace NDJSON from resolved `state_root`.
- Produces short human explanation: what happened, why blocked/passed, what evidence was used, what next action is safe.
- Does not invent missing facts; if trace incomplete, say so.

## Subagent lane split

Use one integration branch plus narrow worktrees/branches per lane. Each implementation worker should be followed by a reviewer subagent before integration.

| Lane | Worker ownership | Reviewer focus | Targeted verification |
| --- | --- | --- | --- |
| L0 manifest/dependency gate | root `Cargo.toml`, `Cargo.lock`, per-crate `Cargo.toml` only | dependency creep, workspace deps, no forbidden crates | `cargo metadata --format-version 1 --no-deps` |
| L1 trace contract crate | `crates/forge-core-trace/**` | schema/serde stability, deterministic IDs, manual errors | `cargo test -p forge-core-trace`; `cargo clippy -p forge-core-trace --all-targets -- -W clippy::pedantic` |
| L2 runtime preview/ready models | `crates/forge-core-runtime/src/lib.rs`, runtime tests | fail-closed semantics, no API breakage | targeted runtime tests around `preview`, `ready`, `trace` |
| L3 store trace persistence | `crates/forge-core-store/src/lib.rs`, store tests | append-only NDJSON, sidecar-relative state path, backward compatibility | targeted store tests for trace append/query and metadata optionality |
| L4 CLI commands | `crates/forge-core-cli/src/main.rs`, new CLI module/tests | thin CLI, error envelopes, no full parser refactor | targeted CLI tests for `preview`, `ready`, `explain` |
| L5 schema/docs fixtures | `crates/forge-core-schema/**`, focused fixtures/docs only | schema matches wire format, fixtures stable | targeted schema tests and fixture validation |

Docs-only L5 fixture anchor:

- Existing validated OperationContract fixtures and CLI acceptance examples are
  pinned in `docs/fixtures/m1-preview-ready-trace/`.
- This pass intentionally adds no new OperationContract YAML because the current
  fixture set already covers preview, ready-pass, ready-missing, ready-pending,
  destructive-blocked, and explain/no-mutation cases.
- M1 trace/explain tests must first resolve `data.state_root` via
  `forge-core project resolve`; trace NDJSON and explain last-run state belong
  below that resolved state root, never under a guessed consumer-repo
  `.forge-method/` path.

## Suggested execution order

1. L0 + L1 first: manifest and trace crate.
2. L2 runtime models on top of existing planner.
3. L3 store persistence after trace type stabilizes.
4. L4 CLI after runtime/store APIs are stable.
5. L5 schema/fixtures once wire format is known.
6. Reviewer agents for L1/L2/L3/L4 before full workspace verification.
7. Integration full suite only once lane tests pass.

## Reviewer subagents

Spawn reviewers with read-heavy instructions:

1. Schema/serde reviewer:
   - Check `TraceEvent` wire compatibility and deterministic JSON.
   - Verify no ambiguous `inputs`/`outputs` shape leaks into public API.
2. Runtime semantics reviewer:
   - Check `ready` fail-closed behavior.
   - Check `preview` cannot mutate.
3. Store/sidecar reviewer:
   - Check trace append/query writes only under resolved `state_root`.
   - Check optional `trace_id` does not break old metadata records.
4. CLI/errors reviewer:
   - Check manual error enums, standard envelopes, no `thiserror`/`anyhow`.
   - Check parsing changes are minimal.

## Targeted verification before full suite

Run per lane, not all at once:

```powershell
cargo metadata --format-version 1 --no-deps
cargo test -p forge-core-trace
cargo clippy -p forge-core-trace --all-targets -- -W clippy::pedantic
cargo test -p forge-core-runtime --test operation_plan preview
cargo test -p forge-core-runtime --test operation_plan ready
cargo test -p forge-core-store trace
cargo test -p forge-core-cli m1
cargo test -p forge-core-schema generated_schemas_cover_v0_contract_surface
```

Adjust exact test filters to actual test names created by each lane.

## Final integration verification

Only after lane workers and reviewers finish:

```powershell
cargo check --workspace
cargo test -p forge-core-trace -p forge-core-runtime -p forge-core-store -p forge-core-schema
cargo test -p forge-core-cli
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::pedantic
cargo fmt --all -- --check
forge-core validate --root . --json
```

## M1 acceptance criteria

- `forge-core preview --root . --operation <fixture> --json` returns deterministic JSON and does not mutate state.
- `forge-core ready --root . --operation <fixture> --json` fails closed on blocked/missing/pending gates and passes only when concrete required evidence exists.
- Runtime paths can emit canonical `TraceEvent` v0 NDJSON under resolved `state_root`.
- `forge-core explain --root . --last-run` produces a truthful short explanation from trace data.
- Consumer repos use `.forge-method.yaml` sidecar resolution for trace/explain storage.
- No `anyhow`/`thiserror` introduced.
- Full suite green at integration close.
