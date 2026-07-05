# Action Plan — 9 Priority Recommendations

Date: 2026-06-29
Status: Planned, execution starting with R1
Origin: critical analysis of the project on 2026-06-29
Link: `AGENTS.md` (project rules), `04_rust_refactor_guide.md`, `01_feature_specs.md`,
`contracts/research/community-trends-and-requested-features-v1.yaml`,
`contracts/research/best-features-from-papers-and-cases-v1.yaml`

## Source conflict (resolved in R13.2, 2026-06-30)

Originally `04_rust_refactor_guide.md` suggested `thiserror` and `clap` derive.
After the R13.2 audit, all dev-docs were aligned with `AGENTS.md`:
- **`thiserror` and `anyhow` are forbidden** — error enums are rolled by hand,
  deriving `Debug, Clone, PartialEq, Eq`.
- **`clap` derive is not used** — Forge keeps manual argv parsing in
  `main.rs` (established pattern). `clap` is not forbidden by `AGENTS.md`,
  but the project chose not to adopt it; new subcommands follow the existing
  pattern of `match` + `run_<command>(&[String])`.

See `04_rust_refactor_guide.md` for the canonical pattern.

## Guiding principles

1. **Rust or Rust-compatible in everything**: no new non-Rust dep. Fuzz with
   `cargo-fuzz` (Rust), benchmarks with `criterion` (Rust), observability with
   `tracing` (Rust).
2. **Best practices and papers**: each recommendation cites the paper/case that
   justifies the practice (RADAR, AutoCodeRover, SLSA, CSA, CodeCRDT, etc.).
3. **Do not step on Codex WIP**: branch `codex/forge-frust-052-ocsp-boundary`
   has uncommitted changes in `Cargo.toml`, `forge-core-cli/src/lib.rs`,
   `main.rs`, `tests/validate.rs`. New work goes into new files or into files
   not touched by it.
4. **Automatic green-loop**: each step must keep `cargo check`, `clippy
   pedantic`, `cargo test`, `cargo fmt --check` green.
5. **Small steps**: each sub-task is ~1 commit, ~1 main file, validatable in
   isolation.

## Map of recommendations → papers/features

| Recommendation | Supporting paper/case | Backlog feature |
|---|---|---|
| R1 Decompose god-file | F15 (Rust ergonomics) | — |
| R2 Migrate `Result<_, String>` | F15 | — |
| R3 Add `tracing` | F03 (canonical TraceEvent) | DEM-06 (evals/cost telemetry) |
| R4 Add `cargo-fuzz` | AutoCodeRover (fault localization) | F11 (Risk Audit Gate) |
| R5 Add `zeroize` | SLSA AI-agent (FEAT-14) | FEAT-13 (sandbox policy) |
| R6 Benchmark `criterion` | DEM-06, FEAT-07 (eval bank) | F13 (Budget/Cost) |
| R7 Migrate `serde_yaml` → `serde_yml` | — (ecosystem deprecation) | — |
| R8 Remove `process::exit` from lib | F15 | — |
| R9 Close Bootstrap Exception | `CONTEXT.md` "Remaining Bootstrap Gaps" | F12 (Guided Start) |

---

## R1 — Decompose `forge-core-cli/src/lib.rs` (7463 lines)

**Why**: the god-file concentrates 30% of the CLI code. It increases the context
the agent needs to read to change anything. It violates "reduce required
context" (F15).

**Papers/cases**: F15 from `feature_backlog.csv`; AutoCodeRover shows that
program structure is a resolution factor (FEAT-04).

**Goal**: `lib.rs` ≤ 1500 lines, modules ≤ 1500 lines each, with no behavior
change.

### R1.1 — Inventory `lib.rs`
- [ ] List all `pub fn`/`pub struct`/`pub enum` with line numbers
- [ ] Group by domain (crypto/verify, rekor, project link, claim, etc.)
- [ ] Identify dependencies between groups
- [ ] Document in `docs/dev-docs/.../r1_lib_inventory.md`

### R1.2 — Create target modules (skeleton)
- [ ] `crates/forge-core-cli/src/crypto_rekor.rs` (parse_rekor, verify_rekor)
- [ ] `crates/forge-core-cli/src/crypto_x509.rs` (cert/crl/ocsp verify)
- [ ] `crates/forge-core-cli/src/project_link.rs` (resolve, init helpers)
- [ ] `crates/forge-core-cli/src/execute_operation.rs` (ExecuteOperationError + flow)
- [ ] Each module: only `pub use` in `lib.rs`, no logic

### R1.3 — Move `parse_rekor_log_entry` and helpers
- [ ] Move `parse_rekor_log_entry`, `required_string`, `required_i64`,
      `required_u64`, `parse_signed_checkpoint` to `crypto_rekor.rs`
- [ ] Keep `pub use` in `lib.rs` so as not to break callers
- [ ] Run `cargo test --workspace`

### R1.4 — Move X.509/CRL/OCSP verification
- [ ] Move functions `verify_signature`, `verify_crl`, `verify_ocsp` to
      `crypto_x509.rs`
- [ ] `pub use` in `lib.rs`
- [ ] Run tests

### R1.5 — Move `execute_operation` flow
- [ ] Move `ExecuteOperationError` and main function to
      `execute_operation.rs`
- [ ] `pub use` in `lib.rs`
- [ ] Run tests

### R1.6 — Move project link resolve/init
- [ ] To `project_link.rs` (careful: `project_cmd.rs` already exists — coordinate)
- [ ] Run tests

### R1.7 — Validate
- [ ] `lib.rs` ≤ 1500 lines
- [ ] `cargo clippy --workspace --all-targets -- -W clippy::pedantic` green
- [ ] `cargo test --workspace` green
- [ ] Snapshot of CLI output unchanged

---

## R2 — Migrate 17 `Result<_, String>` to named enums

**Why**: `AGENTS.md` forbids it explicitly. `String` errors are not
exhaustive, do not carry structured context, break `?` at boundaries.

**Papers/cases**: F15; "structural bug prevention type-level"
(`structural-bug-prevention-typelevel-v1.yaml`).

**Goal**: zero `Result<_, String>` in production code (excludes `#[cfg(test)]`).

### R2.1 — Inventory
- [ ] List 17 sites with file:line:signature
- [ ] Classify by domain (parse, validate, isolation, store, engine)
- [ ] Document in `docs/dev-docs/.../r2_string_result_inventory.md`

### R2.2 — `contract_cmd.rs` (3 sites)
- [ ] `validate_kind` → `ContractValidationError` enum
- [ ] `parse_document` → reuse `ContractValidationError`
- [ ] Update callers in `contract_cmd.rs` and `main.rs`
- [ ] Tests

### R2.3 — `isolation.rs` (2 sites)
- [ ] `parse_merge_policy` → `MergePolicyParseError`
- [ ] `parse_status` → `IsolationStatusParseError`
- [ ] Update callers
- [ ] Tests

### R2.4 — `lib.rs` rekor parsers (5 sites)
- [ ] `parse_rekor_log_entry` → `RekorParseError`
- [ ] `required_string`/`required_i64`/`required_u64` → `RekorFieldError`
- [ ] `parse_signed_checkpoint` → `CheckpointParseError`
- [ ] `verify_rekor_*` (2 sites with `Result<(), String>`) → reuse
- [ ] Update callers
- [ ] Tests

### R2.5 — `main.rs` (1 site)
- [ ] `StatefulCommandRoots` builder → `StatefulCommandRootsError`
- [ ] Tests

### R2.6 — `forge-core-decisions` (3 sites)
- [ ] `catalog.rs::load_one` → `CatalogLoadError`
- [ ] `catalog.rs::parse_workflow_yaml` → reuse
- [ ] `eval.rs::load_eval_corpus` → `EvalCorpusLoadError`
- [ ] `isolation.rs::shell_metachar_check` → `ShellMetacharError`
- [ ] Tests

### R2.7 — `forge-core-store` (1 site)
- [ ] `lib.rs:1371` → `EffectWalReadError` (`ReferenceIndexBuildError` already
      exists as the pattern)
- [ ] Tests

### R2.8 — Validate
- [ ] `grep -rn "Result<.*String>" crates --include="*.rs" | grep -v /tests/`
      returns 0
- [ ] `cargo test --workspace` green

---

## R3 — Add structured `tracing`

**Why**: `eprintln!` is not observability. Multi-agent in CI needs correlated
spans. Paper: F03 (canonical TraceEvent); DEM-06 (evals/cost telemetry);
FEAT-15 (prompt-injection detection telemetry requires runtime observability).

**Papers/cases**: F03, DEM-06, FEAT-15, `rust-observability-selfhealing-v1.yaml`.

**Goal**: spans on every critical path (claim acquire/release, WAL append,
execute_operation, verify_rekor). Default `tracing_subscriber` with env-filter.
No `eprintln!` in lib code (exception: `main.rs` before subscriber init).

### R3.1 — Add deps
- [ ] `tracing`, `tracing-subscriber` in `[workspace.dependencies]`
- [ ] Add to deps of `forge-core-store`, `forge-core-kernel`,
      `forge-core-cli`, `forge-core-validate`
- [ ] `cargo check`

### R3.2 — Init subscriber in `main.rs`
- [ ] `tracing_subscriber::fmt().with_env_filter().with_writer(std::io::stderr).init()`
- [ ] Gate with feature `tracing` (always on for now)
- [ ] Test: run CLI with `RUST_LOG=info` and see spans

### R3.3 — Spans in `claim_wal.rs`
- [ ] `#[instrument(skip(self), fields(claim_id, seq))]` in `append`,
      `read`, `rotate`
- [ ] Events on CRC errors, lock contention
- [ ] Test

### R3.4 — Spans in `execute_operation`
- [ ] Span around spawn, validate, verify
- [ ] Events on `RuntimeCommandEvaluationStatus::Blocked`/`Failed`
- [ ] Test

### R3.5 — Spans in `verify_rekor`/`verify_x509`
- [ ] `#[instrument(skip(...))]` on each public function
- [ ] Events on signature mismatch
- [ ] Test

### R3.6 — Remove `eprintln!` from lib code
- [ ] Replace with `tracing::warn!`/`tracing::error!`
- [ ] Keep `eprintln!` only in pre-init `main.rs`
- [ ] `cargo test`

### R3.7 — Validate
- [ ] `grep -rn "eprintln!" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
      returns 0
- [ ] `cargo test --workspace` green
- [ ] Snapshot of CLI output unchanged (logs go to stderr, not stdout)

---

## R4 — Add `cargo-fuzz`

**Why**: parsers of untrusted YAML (`parse_rekor_log_entry`,
`parse_signed_checkpoint`) and binary WAL decoding are a classic attack
surface for panic/DoS. Without fuzz, "security excellence" is just a claim.

**Papers/cases**: AutoCodeRover (fault localization, FEAT-05); CSA/ARMO (FEAT-15);
`rust-testing-defenses-v1.yaml`.

**Goal**: 3 fuzz targets running 60s without panic.

### R4.1 — Setup
- [ ] `cargo install cargo-fuzz` (instruction in README, not in CI)
- [ ] Create `crates/fuzz/` as a separate workspace member (does not get in the
      way of normal build)
- [ ] `Cargo.toml` with `cargo-fuzz` and `libfuzzer-sys`
- [ ] Add to workspace with `default-members` excluding `fuzz`

### R4.2 — Fuzz target 1: `parse_rekor_log_entry`
- [ ] `fuzz_targets/parse_rekor.rs`
- [ ] Seed corpus with 3 real entries (valid, malformed, adversarial)
- [ ] Run 60s, no panic

### R4.3 — Fuzz target 2: `parse_signed_checkpoint`
- [ ] `fuzz_targets/parse_checkpoint.rs`
- [ ] Seed with a real Rekor checkpoint
- [ ] Run 60s, no panic

### R4.4 — Fuzz target 3: `claim_wal` decode
- [ ] `fuzz_targets/claim_wal_decode.rs`
- [ ] Feed arbitrary bytes to the decoder
- [ ] Run 60s, no panic (typed errors ok, panic not)

### R4.5 — Document
- [ ] Section in README on how to run fuzz
- [ ] Add to `06_protocol_security_plan.md`

---

## R5 — Add `zeroize` for crypto material

**Why**: keys/signatures stay in memory until GC. For a runtime that verifies
signatures of external agents, this is minimum hygiene. SLSA AI-agent
(FEAT-14) requires provenance of crypto material.

**Papers/cases**: SLSA AI-agent proposal (FEAT-14); ARMO/CSA (FEAT-13, FEAT-15).

**Goal**: any `SigningKey`/`VerifyingKey`/`Signature` that enters a function
scope is `Zeroize`-on-drop.

### R5.1 — Inventory crypto material
- [ ] List all sites of `SigningKey`, `VerifyingKey`, `Signature`,
      `SecretKey` in `crates/`
- [ ] Confirm: Forge only verifies (does not sign)? If so, `VerifyingKey`/`Signature`
      are public — `zeroize` is nice-to-have, not critical
- [ ] Document in `docs/dev-docs/.../r5_crypto_inventory.md`

### R5.2 — Add dep
- [ ] `zeroize = { version = "1.8", features = ["derive"] }` in
      `[workspace.dependencies]`
- [ ] Add to `forge-core-cli`
- [ ] `cargo check`

### R5.3 — Wrap in `Zeroizing<>`
- [ ] In `verify_rekor_*`, `verify_x509_*`: wrap temporary `VerifyingKey` in
      `Zeroizing`
- [ ] In `parse_rekor_log_entry`: parsed `Signature` in `Zeroizing`
- [ ] Existing tests stay green

### R5.4 — Constant-time in manual comparisons
- [ ] Look for `==` on crypto bytes (outside crates' `verify()`)
- [ ] Replace with `subtle::ConstantTimeEq` if any
- [ ] If none (likely), document in the plan

### R5.5 — Validate
- [ ] `cargo test --workspace` green
- [ ] Section in `06_protocol_security_plan.md`

---

## R6 — `criterion` benchmark

**Why**: a "performance" claim without a metric is marketing. DEM-06 asks for
evals with cost/latency. FEAT-07 asks for an eval bank with latency.

**Papers/cases**: DEM-06, FEAT-07, `agentic-throughput-and-fast-quality-mode-v1.yaml`.

**Goal**: 3 benchmarks running in <30s, numbers in README.

### R6.1 — Setup
- [ ] `criterion = "0.5"` in `[workspace.dependencies]`
- [ ] `[[bench]]` in `forge-core-store`, `forge-core-validate`
- [ ] `benches/` dir with harness
- [ ] `cargo bench --no-run` compiles

### R6.2 — Bench WAL append
- [ ] `benches/claim_wal_append.rs`
- [ ] Scenarios: 100, 1000, 10000 records
- [ ] Measure: append + fsync + projection
- [ ] Run, record baseline in `docs/dev-docs/.../r6_bench_baseline.md`

### R6.3 — Bench validate
- [ ] `benches/validate_report.rs`
- [ ] Scenarios: 10, 100, 1000 diagnostics
- [ ] Measure: build + serialize
- [ ] Run, record baseline

### R6.4 — Bench rekor verify
- [ ] `benches/rekor_verify.rs`
- [ ] Scenario: 1 real entry
- [ ] Measure: parse + verify
- [ ] Run, record baseline

### R6.5 — Document
- [ ] Section in README with the numbers
- [ ] Add to `05_eval_and_quality_plan.md`

---

## R7 — Migrate `serde_yaml` → `serde_yml`

**Why**: `serde_yaml` 0.9.34 is in maintenance mode, discontinued by
dtolnay. `serde_yml` is the maintained successor.

**Goal**: zero `serde_yaml` in `Cargo.lock`, all `use serde_yaml` become
`use serde_yml`.

### R7.1 — Inventory
- [ ] `grep -rn "serde_yaml" crates --include="*.rs" --include="Cargo.toml"`
- [ ] List APIs used (`from_str`, `to_string`, `Value`, etc.)
- [ ] Confirm compatibility in `serde_yml` 0.0.12+

### R7.2 — Add `serde_yml` and remove `serde_yaml`
- [ ] `serde_yml = "0.0.12"` in `[workspace.dependencies]`
- [ ] Remove `serde_yaml` from `[workspace.dependencies]`
- [ ] Update each crate `Cargo.toml`
- [ ] `cargo check`

### R7.3 — Migrate imports
- [ ] `sed -i 's/serde_yaml/serde_yml/g'` in each `.rs` (careful with
      `serde_yaml::Value` vs `serde_yml::Value`)
- [ ] Run `cargo check --workspace`
- [ ] Run `cargo test --workspace`

### R7.4 — Validate
- [ ] `grep -rn "serde_yaml" crates` returns 0
- [ ] Snapshot of output unchanged

---

## R8 — Remove `std::process::exit` from lib code

**Why**: `exit` in lib code breaks tests, imposes non-local flow control,
prevents composition. Anti-pattern in Rust.

**Goal**: `std::process::exit` only in `main.rs` and `bin/`.

### R8.1 — Inventory
- [ ] `grep -rn "process::exit" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
- [ ] List: `autonomy_cmd.rs:404,426`, `contract_cmd.rs:43,75,187,215`

### R8.2 — `contract_cmd.rs`
- [ ] Replace `process::exit(2)` with propagated `return Err(...)`
- [ ] Caller in `main.rs` decides exit code
- [ ] Tests

### R8.3 — `autonomy_cmd.rs`
- [ ] Same approach
- [ ] Tests

### R8.4 — Validate
- [ ] `grep -rn "process::exit" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
      returns 0
- [ ] `cargo test --workspace` green
- [ ] CLI exit codes unchanged (test with `assert_cmd`)

---

## R9 — Close Bootstrap Core Exception

**Why**: `CONTEXT.md` admits the gap. The "consumer-ready" promise depends on
it. Without closing it, every consumer repo needs the exception, which
violates isolation.

**Papers/cases**: `CONTEXT.md` "Remaining Bootstrap Gaps"; F12 (Guided Start).

**Goal**: `--allow-bootstrap-core` removed (or only for internal tests),
consumer repo init works without local state.

### R9.1 — Inventory use of the exception
- [ ] `grep -rn "allow_bootstrap_core\|allow-bootstrap-core" crates contracts`
- [ ] List all sites that depend on the exception
- [ ] Document in `docs/dev-docs/.../r9_bootstrap_inventory.md`

### R9.2 — Confirm sidecar init works
- [ ] Run `forge-core project init --root <tmp consumer>` in a clean repo
- [ ] Check that `.forge-method.yaml` points to the sidecar
- [ ] Check that state-bearing commands resolve without `--allow-bootstrap-core`

### R9.3 — Migrate tests that use the exception
- [ ] For each test with `--allow-bootstrap-core`, create a sidecar version
- [ ] Keep the exception only in forge-core's internal `#[cfg(test)]`

### R9.4 — Remove the exception from production paths
- [ ] `project_cmd.rs`: deny consumer-local `state_root` without test flag
- [ ] Runtime/claim commands: fail-closed without sidecar
- [ ] Update `CONTEXT.md` removing "Remaining Bootstrap Gaps"

### R9.5 — Validate
- [ ] `cargo test --workspace` green
- [ ] E2E: clean consumer repo → init → execute-operation → no exception

---

## Execution order

1. **R1** (decompose god-file) — frees up context for all the others
2. **R2** (migrate `Result<_, String>`) — easier after R1
3. **R8** (remove `process::exit`) — easier after R1
4. **R7** (migrate `serde_yaml`) — mechanical, independent
5. **R3** (add `tracing`) — after R1/R8
6. **R6** (benchmark) — after R3 to measure with spans
7. **R4** (fuzz) — after R2 (typed errors)
8. **R5** (zeroize) — independent, but after R1
9. **R9** (bootstrap) — more structural, last

## Tracking

Each recommendation has a progress file in `dev-journals/` in the sibling
repo `Forge-method-archive` as it is started.
