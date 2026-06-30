# R3 — Structured tracing (Fase 3 do system design roadmap)

Branch: `codex/forge-frust-052-ocsp-boundary`
Started: 2026-06-29
Status: **in progress**

## Goal

Wire the `tracing` crate into every critical path of the workspace so that
agents (and humans debugging on their behalf) get a deterministic JSON event
stream they can filter, replay, and reason over. This is the foundation that
unlocks:

- F01 `forge preview` (decisions explained before they are applied)
- F03 `forge explain` (per-run TraceEvent narration)
- R4/R6 benchmarks with spans
- Multi-agent correlation (one `agent_id` field per span)

## Non-goals

- Do NOT touch the stderr-byte contract from R8. `eprintln!` in
  `cli_error.rs`, `cli_util.rs`, and `main.rs` is the user-visible error
  channel and stays. Only `eprintln!` in dispatcher `_cmd.rs` files
  migrates, and only where it is logging-style diagnostics (not the
  `Err(ExitError)` payload that the binary entrypoint prints).
- Do NOT replace `forge-core-trace::TraceEvent` (the durable NDJSON ledger).
  `tracing` is the *live* observability stream; `TraceEvent` is the
  *auditable* artifact. They are complementary.
- Do NOT introduce `slog`, `log`, or `env_logger`. `tracing` +
  `tracing-subscriber` only.

## Approach (R8-style: small commits, one concern each)

1. **R3.1** — Add `tracing` + `tracing-subscriber` to workspace deps.
   Initialize subscriber in `main.rs` with `EnvFilter` and a JSON formatter
   default. Add `--log-format human|json` flag (default `json` for agents).
2. **R3.2** — Spans on critical paths (one file per commit, biggest value
   first):
   - `forge-core-store::claim_wal` — span per op with `tx_id`, `claim_id`.
   - `forge-core-runtime::execute_operation` — span with `operation_id`,
     `effect_count`.
   - `forge-core-crypto::*_verification` — span with `verification_kind`,
     `subject_ref`, `result`.
   - `forge-core-validate::run_validate` — span with `root`,
     `diagnostic_count`.
   - `forge-core-cli::run_*` dispatchers — span with `root`, command name.
3. **R3.3** — Multi-agent correlation. Each agent session carries an
   `agent_id` (from claim or CLI `--agent-id`). Spans include it as a
   field. `agent_id=X` filter produces a single-agent timeline.
4. **R3.4** — Migrate logging-style `eprintln!` in dispatcher `_cmd.rs`
   files to `tracing::warn!`/`tracing::error!`. The stderr contract
   (typed `ExitError` payloads) is untouched.

## Inventory (initial)

| File | `eprintln!` count | Migration target |
|---|---|---|
| `cli_error.rs` | n | **keep** (stderr contract) |
| `cli_util.rs` | n | **keep** (stderr contract) |
| `main.rs` | n | **keep** (stderr contract) |
| `autonomy_cmd.rs` | n | migrate to `tracing::warn!`/`error!` |
| `claim.rs` | n | migrate |
| `contract_cmd.rs` | n | migrate |
| `coordination.rs` | n | migrate |
| `guide.rs` | n | migrate |
| `isolation.rs` | n | migrate |
| `validate.rs` | n | migrate |
| `forge-contract-validator/src/main.rs` | n | migrate (binary, owns its stderr) |
| `forge-core-schema/src/main.rs` | n | migrate (binary, owns its stderr) |

## Definition of Done

- `cargo bench` (after R4) can attach spans.
- `forge validate --root . --json` produces a JSON log stream on stderr
  when `--log-format json`.
- Every span carries `agent_id` when provided.
- Zero logging-style `eprintln!` in dispatcher `_cmd.rs` files.
- Workspace `cargo test --workspace` still passes.
- `cargo run -- validate --root . --json` anchor: 122 occurrences of
  `"diagnostics": 0` preserved (no behavior drift).
