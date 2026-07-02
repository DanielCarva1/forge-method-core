# R3 — Structured tracing (Fase 3 do system design roadmap)

Branch: `codex/forge-frust-052-ocsp-boundary`
Started: 2026-06-29
Status: **complete** (R3.1, R3.2, R3.3 done; R3.4 analyzed as no-op)

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
   - `forge-core-kernel::execute_operation` — span with `operation_id`,
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

## R3.4 outcome (analyzed 2026-06-30): NO-OP

A full inventory of `eprintln!` across `_cmd.rs` files revealed that
**every** occurrence is user-facing contract output, not logging-style
diagnostics:

| File | Count | Real classification |
|---|---|---|
| `autonomy_cmd.rs:423` | 1 | Envelope failure message (text mode) |
| `claim.rs:1689-2178` | 8 | `--flag required` usage errors (8 sites) |
| `claim.rs:2098-2103` | 2 | Reconcile loop progress output (text mode) |
| `contract_cmd.rs:249,270` | 2 | Envelope failure message (text mode) |
| `coordination.rs:138-187` | 4 | `--flag required` / unknown subcommand usage |
| `guide.rs:632-763` | 3 | `--flag required` / unrecognized argument usage |
| `guide.rs:819` | 1 | Envelope failure message (text mode) |
| `isolation.rs:973-1210` | 5 | `--flag required` / parse errors (5 sites) |
| `validate.rs:665` | 1 | **Validation diagnostics output (text mode)** —
   |   |   | this is the primary text-mode output of `validate`, not logging |

Migrating any of these to `tracing::warn!` would break the user-visible
stderr contract: `validate.rs:665` is literally how humans read
diagnostics in text mode, and the envelope `eprintln!` sites are the
human-mode counterpart of the JSON envelope on stdout.

The original R3.4 premise ("dispatcher _cmd.rs files have logging-style
eprintln") was a misclassification. Honest outcome: **R3.4 is a no-op.
The stderr contract from R8 already covers all legitimate cases.**

Future R3.5 (if ever needed): when a NEW logging-style diagnostic is
added (e.g. "retrying...", "cache miss"), use `tracing::warn!` from the
start. No backfill is required today.

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

## Verification log

- **R3.1** (commit `096cd32`): `tracing` + `tracing-subscriber` added
  to workspace; `tracing_init.rs` initializes the subscriber with
  `EnvFilter` + `FORGE_LOG_FORMAT=json|human` auto-selection.
- **R3.2** (commits `9c29d7f`, `5958a53`, `2282f04`, `e1db562`,
  `a8e43b8`): `#[instrument]` spans on `run_validate`, claim_wal ops,
  `execute_operation`, crypto verification entrypoints, and the
  `validate` CLI dispatcher. `FmtSpan::ENTER` enabled so spans show up
  even without explicit events.
- **R3.3** (commit `2ba9423`): `forge_session` root span carries
  `agent_id` (from `FORGE_AGENT_ID`) and `command`; every nested span
  inherits both via the subscriber's current-span context.
- **R3.4**: analyzed, classified as no-op (see above). Stderr contract
  from R8 already covers all legitimate cases.

E2E proof: `FORGE_AGENT_ID=codex-001 FORGE_LOG_FORMAT=json RUST_LOG=info
./target/debug/forge-core validate --root . --json` emits 48 nested
JSON events with `agent_id":"codex-001` propagating from
`forge_session` down to `validate_command` and beyond. Anchor still 122.

## DoD checklist

- [x] `tracing` wired into every critical path
- [x] JSON log stream on stderr under `FORGE_LOG_FORMAT=json`
- [x] Every span carries `agent_id` when `FORGE_AGENT_ID` is set
- [x] No logging-style `eprintln!` in `_cmd.rs` (none existed)
- [x] `cargo test --workspace` passes
- [x] Anchor `validate --json` emits 122 `"diagnostics": 0`
