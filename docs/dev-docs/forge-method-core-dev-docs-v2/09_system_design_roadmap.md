# System Design Roadmap — Structural Cleanup of Forge Method Core

**Date**: 2026-06-29
**Status**: Planning (after the critical conversation with Daniel)
**Replaces/extends**: `08_priority_recommendations_plan.md` (R1-R9) — R1-R9 stays as
"quick wins", this roadmap addresses the structural debt that R1-R9 does not cover.

## Context

The critical analysis of the system design (2026-06-29) identified that the *black-box*
design (crates, typed contracts, project link/sidecar) is solid (~7-8/10), but the
*internal* design has four structural debts that compromise the "excellence
in Rust" narrative:

1. **God-file** — `forge-core-cli/src/lib.rs` with 6771 lines, 113 functions, longest
   function of 493 lines. It mixes 7 domains.
2. **Crypto-in-the-CLI** — 14 heavy cryptographic verification functions
   (`run_host_adapter_*_verification`: sigstore, fulcio, rekor, CT, CRL, OCSP, TUF,
   timestamp authority) live in `forge-core-cli`, which should be presentation only.
3. **Monolithic `main.rs`** — 4116 lines with 141 `process::exit`, making the
   entrypoint non-testable as a unit.
4. **Tests coupled to the CLI** — `tests/validate.rs` with 5215 lines imports
   cryptographic logic from the CLI to test it; a format change breaks the crypto test
   and vice-versa.

R1-R9 of the original plan **partially** addresses R1 (god-file), but:
- R1 stops at "≤1500 lines" (still a god-file)
- R8 only covers `process::exit` in **lib code**, not in `main.rs`
- There is no recommendation to move crypto out of the CLI
- There is no recommendation to disconnect `tests/validate.rs` from the CLI

This roadmap completes R1-R9 with 4 new tracks (R10-R13) and reorders execution.

---

## Principles

1. **Behavior preservation first** — no step changes the observable output of the CLI.
   Snapshots of `--json` are the regression anchor.
2. **Tests migrate with the code** — when code leaves the CLI, the corresponding test
   leaves `tests/validate.rs`.
3. **One layer at a time** — structural refactor before adding
   instrumentation (tracing, bench, fuzz). Otherwise, tracing becomes churn in a
   god-file.
4. **Each commit compiles and tests green** — no big-bang refactors.
5. **Crates are the unit of isolation** — when a domain (crypto, runtime,
   store) has >3000 lines, it is a candidate for its own crate.
6. **Papers are evidence, not decoration** — each recommendation cites the paper; when
   the code that implements the paper exists, the link is bidirectional.

---

## Map of debts → papers/features

| Debt | Supporting papers/cases | Backlog feature | Track |
|---|---|---|---|
| God-file `lib.rs` | F15 (Rust ergonomics); AutoCodeRover (FEAT-04) | — | R1 |
| Crypto in the CLI | SLSA AI-agent (FEAT-14); F11 (Risk Audit Gate) | FEAT-13 sandbox | **R10** |
| Monolithic `main.rs` + exits | F15; "Testing in Production" (Torres) | F12 (Guided Start) | **R11** |
| Tests coupled to the CLI | AutoCodeRover (test isolation) | F11 | **R12** |
| Doc divergence (`04_rust_refactor_guide`) | — (housekeeping) | — | **R13** |
| Blind protocol for Rust debt | F15; dogfood CLI (2026-06-29) | — | **R14** |
| `Result<_, String>` legacy | F15 | — | R2 |
| No `tracing` | F03 (canonical TraceEvent); DEM-06 | FEAT-07 eval bank | R3 |
| No fuzz | AutoCodeRover (fault localization) | F11 | R4 |
| No `zeroize` | SLSA AI-agent (FEAT-14) | FEAT-13 | R5 |
| No benchmark | DEM-06; FEAT-07 | F13 (Budget/Cost) | R6 |
| `serde_yaml` deprecated | — (ecosystem) | — | R7 |
| `process::exit` in lib | F15 | — | R8 |
| Bootstrap Exception | `CONTEXT.md` | F12 | R9 |

---

## Phases (execution order)

### Phase 0 — Structural decomposition (R1 extended + R12)

**Goal**: `lib.rs ≤ 500 lines`, all modules ≤ 1500, `tests/validate.rs` ≤ 2000.
**Risk**: medium (touching cryptographic code is delicate, but it is a pure move).
**Estimated duration**: 8-12 commits.

#### R1.A — Complete the `lib.rs` split (already in progress)

Remaining sub-tracks (in order of increasing risk):

- [x] R1.3 — `crypto_rekor.rs` ✓
- [x] R1.5 — `execute_operation.rs` ✓
- [x] R1.EffectIndex — `effect_index.rs` ✓
- [x] R1.CryptoHashing — `crypto_hashing.rs` ✓
- [ ] **R1.HostAdapterTypes** — lines 81-900 (~70 `HostAdapter*` types, no logic).
      Low risk, high value: −820 lines.
- [ ] **R1.HostCommand** — `host_command`, `command_process_admission`,
      `argv_has_shell_control`, `env_key_is_forbidden`, `source_ref_is_immutable`,
      `version_like`. ~200 lines.
- [ ] **R1.HostAdapterManifest** — `run_host_adapter_manifest` (493 lines!).
      **Split before moving**: break into `build_command_section`,
      `build_distribution_section`, `build_security_section` etc.
- [ ] **R1.HostAdapterProjection** — `run_host_adapter_projection`,
      `process_security_policy`, `invocation_admission`, `project_host_command`,
      `mcp_annotations`, `command_input_schema`.
- [ ] **R1.Validate** — `run_validate`, `validate_operation_fixtures`,
      `validate_side_contracts`, `validate_runtime_contracts`, helpers. ~400 lines.
      Goes together with `validate_helpers.rs` (`read_yaml`, `yaml_files`).
- [ ] **R1.CryptoOCSP** — `decode_ocsp_response` + 12 OCSP helpers.
      **Careful**: Codex WIP (FRUST-052) touched here. Confirm stable state
      before.
- [ ] **R1.CryptoTUFDateTime** — `verify_tuf_metadata_freshness_role`,
      `parse_tuf_datetime_utc_to_unix`, Gregorian calendar helpers.
- [ ] **R1.CryptoSigstore** — `verify_sigstore_*`, `verify_fulcio_chain`, etc.
      ~1270 lines. **Largest isolated track** — likely candidate for its own crate
      (see R10).
- [ ] **R1.CryptoSLSATransparency** — `verify_slsa_statement`,
      `verify_transparency_log_proof`, `verify_merkle_inclusion`.
- [ ] **R1.HostAdapterVerification** — the 14 public `run_host_adapter_*_verification`.
      **These go to the new crate in R10**, they do not stay in the CLI.

#### R12 — Decouple `tests/validate.rs` from the CLI

`tests/validate.rs` (5215 lines) imports from the CLI things that are not CLI:
contract parsing, crypto verify, etc.

- [ ] **R12.1** — Inventory what `tests/validate.rs` actually tests:
      contract flows (should go to `forge-core-contracts`), crypto flows (go to
      `forge-core-crypto` in R10), CLI flows (stay).
- [ ] **R12.2** — Move contract parsing tests to
      `crates/forge-core-contracts/tests/` or `forge-core-validate/tests/`.
- [ ] **R12.3** — Move crypto verification tests to the future
      `forge-core-crypto/tests/` (after R10).
- [ ] **R12.4** — Reduce `tests/validate.rs` to tests of **CLI presentation**:
      JSON shape, exit codes, help text, argv parsing.
- [ ] **R12.5** — Snapshot test of the `--json` output of each subcommand as a
      regression anchor.

**Phase 0 DoD**: `lib.rs ≤ 500 lines`, `tests/validate.rs ≤ 2000 lines`, all
gates green, CLI output snapshot unchanged.

---

### Phase 1 — Move crypto out of the CLI (R10)

**Goal**: create the `forge-core-crypto` crate and move the 14 verification
functions + OCSP/CRL/sigstore/CT/TSA helpers.
**Risk**: high (largest structural refactor; touches tests, main.rs, lib.rs).
**Estimated duration**: 6-10 commits.

#### R10.1 — Create `crates/forge-core-crypto/` skeleton

- [ ] `Cargo.toml` with crypto deps (`asn1-rs`, `base64`, `ed25519-dalek`, `p256`,
      `rasn`, `rasn-ocsp`, `sha1`, `sha2`, `sct`, `sigstore-tsa`,
      `rustls-pki-types`, `x509-parser`).
- [ ] Depends on `forge-core-contracts` (for verification contract types).
- [ ] Does not depend on `forge-core-cli` or `forge-core-kernel`.
- [ ] Add to workspace `members`.

#### R10.2 — Move crypto modules from the CLI

In order:

- [ ] `crypto_hashing.rs` (already isolated in the CLI) → `forge-core-crypto/src/hashing.rs`
- [ ] `crypto_rekor.rs` → `forge-core-crypto/src/rekor.rs`
- [ ] `crypto_ocsp.rs` (to be created in R1.CryptoOCSP) → `forge-core-crypto/src/ocsp.rs`
- [ ] `crypto_sigstore.rs` (to be created in R1.CryptoSigstore) →
      `forge-core-crypto/src/sigstore.rs`
- [ ] `crypto_slsa_transparency.rs` → `forge-core-crypto/src/slsa.rs`
- [ ] The 14 `run_host_adapter_*_verification` →
      `forge-core-crypto/src/host_adapter_verification.rs`

#### R10.3 — Move corresponding tests

- [ ] From `tests/validate.rs` to `crates/forge-core-crypto/tests/`.
- [ ] Update imports in tests: from `forge_core_cli::*` to
      `forge_core_crypto::*`.

#### R10.4 — CLI becomes a thin client

- [ ] `forge-core-cli/Cargo.toml` adds `forge-core-crypto` as a dep.
- [ ] `lib.rs` does `pub use forge_core_crypto::*` (transitive) or call sites
      updated.
- [ ] `main.rs` calls `forge_core_crypto::run_host_adapter_*_verification`.

#### R10.5 — DoD

- [ ] `forge-core-cli/src/lib.rs` < 1500 lines (only host adapter types + manifest
      + validate).
- [ ] `forge-core-crypto` has zero deps on `forge-core-cli` or `forge-core-kernel`.
- [ ] All gates green.
- [ ] CLI output snapshot unchanged.

---

### Phase 2 — Error discipline (R2 + R8 + R11 partial)

**Goal**: zero new `Result<_, String>`, zero `process::exit` in lib **and** in
main.rs, errors propagate via `Result` to the top.
**Risk**: medium (R11 changes error flow but not behavior).
**Estimated duration**: 8-12 commits.

#### R2 — Migrate residual `Result<_, String>`

A recent inventory showed that only **1 site** remains in `forge-core-store/src/lib.rs`.
The original 17 from the plan were partially migrated by previous work.

- [ ] **R2.1** — Confirm the current inventory (grep for `Result<.*, String>`).
- [ ] **R2.2** — Migrate the site in `forge-core-store`.
- [ ] **R2.3** — Add a `clippy::result_large_err` lint or custom CI check
      rejecting new `Result<_, String>`.

#### R8 — Remove `process::exit` from lib code

- [ ] **R8.1** — Inventory (grep `process::exit` in `crates/*/src/`).
- [ ] **R8.2** — `contract_cmd.rs`, `autonomy_cmd.rs` (mentioned in the plan).
- [ ] **R8.3** — Replace with `Result<T, CliError>` propagating up to `main.rs`.

#### R11 — Decompose `main.rs` (4116 lines, 141 exits)

`main.rs` is the **monolithic entrypoint**: parse argv, dispatch, format output,
exit. Today everything is in a single file.

- [ ] **R11.1** — Inventory sub-commands in `main.rs`.
- [ ] **R11.2** — Create `crates/forge-core-cli/src/commands/` with one module per
      family: `validate_cmd.rs`, `execute_operation_cmd.rs`,
      `claim_cmd.rs`, `host_adapter_cmd.rs`, etc.
- [ ] **R11.3** — Each `*_cmd.rs` exposes `fn run(args: &[String]) -> Result<ExitCode,
      CliError>`.
- [ ] **R11.4** — `main.rs` is reduced to: init tracing → parse top-level → dispatch →
      match error → `process::exit(code)`. The **only** `process::exit` in the crate
      stays here.
- [ ] **R11.5** — Define a typed `CliError` enum (hand-rolled, no thiserror):
      `InvalidArgs(String)`, `SubcommandFailed(any error)`, `Io(std::io::Error)`.
- [ ] **R11.6** — `tests/cli_smoke.rs` tests each subcommand via `assert_cmd` and
      checks exit code + stderr shape (not cryptographic content).

**Phase 2 DoD**: zero `process::exit` in `crates/*/src/` (except 1 in the top-level
`main.rs`), zero `Result<_, String>` in new code, `main.rs < 200` lines, each
`*_cmd.rs < 500` lines.

---

### Phase 3 — Observability (R3)

**Goal**: structured `tracing` on every critical path, JSON subscriber default
for consumption by agents.
**Risk**: low (additive).
**Estimated duration**: 5-8 commits.

#### R3.1 — Deps and init

- [ ] Add `tracing`, `tracing-subscriber` to workspace deps.
- [ ] `main.rs` init subscriber with `EnvFilter` and JSON formatter default.
- [ ] Flag `--log-format human|json` (default json for agents).

#### R3.2 — Spans on critical paths

In order of value:

- [ ] `forge-core-store::claim_wal` (append, rotate, replay) — span per operation
      with `tx_id`, `claim_id`.
- [ ] `forge-core-kernel::execute_operation` — span with `operation_id`,
      `effect_count`.
- [ ] `forge-core-crypto::run_host_adapter_*_verification` — span with
      `verification_kind`, `subject_ref`, `result`.
- [ ] `forge-core-validate::run_validate` — span with `root`, `diagnostic_count`.
- [ ] `forge-core-cli::run_execute_operation` — span with `root`, `payload_count`.

#### R3.3 — Multi-agent correlation

- [ ] Each agent session receives an `agent_id` (from claim or CLI arg).
- [ ] Spans carry `agent_id` as a field.
- [ ] JSON log allows filtering `agent_id=X` to see only what one agent did.

#### R3.4 — Remove `eprintln!` from lib code

- [ ] grep `eprintln!` in `crates/*/src/`, migrate to `tracing::warn!`/`error!`.
- [ ] `println!` in lib code only where it is the output contract (JSON to stdout).

**Phase 3 DoD**: structured JSON logs on all critical paths, zero
`eprintln!` in `crates/*/src/` (except main.rs fallback without subscriber).

---

### Phase 4 — Quality evidence (R6 + R4)

**Goal**: benchmarks for hot paths, fuzz harness for parsers.
**Risk**: very low (additive, does not touch production code).
**Estimated duration**: 4-6 commits.

#### R6 — `criterion` benchmarks

- [ ] **R6.1** — Add `criterion` to the workspace. Create
      `crates/forge-core-store/benches/claim_wal.rs`.
- [ ] **R6.2** — Bench: WAL append (1, 100, 1000 entries), WAL replay, CRC verify.
- [ ] **R6.3** — Bench: `build_reference_index` on a repo of varying size.
- [ ] **R6.4** — Bench: `serde_yaml::from_str` vs `serde_yml::from_str` (after R7)
      of a contract document.
- [ ] **R6.5** — Bench: `verify_rekor_checkpoint`, `verify_merkle_inclusion`.
- [ ] **R6.6** — CI runs bench on PR with label `perf` and compares with `main`.

#### R4 — `cargo-fuzz`

- [ ] **R4.1** — Create `fuzz/` directory in the workspace (cargo-fuzz requires this).
- [ ] **R4.2** — Target: `parse_rekor_log_entry` (parse of adversarial JSON).
- [ ] **R4.3** — Target: `parse_signed_checkpoint` (decode of adversarial base64).
- [ ] **R4.4** — Target: `claim_wal_decode` (adversarial NDJSON).
- [ ] **R4.5** — Target: `ocsp_response_decode` (adversarial DER).
- [ ] **R4.6** — Document execution in `docs/dev-docs/.../fuzzing.md` with
      command `cargo fuzz run <target> -- -max_total_time=60`.

**Phase 4 DoD**: `cargo bench` runs without error, `cargo fuzz run` on each target
for ≥1 min without panic.

---

### Phase 5 — Supply chain and security (R7 + R5)

**Goal**: `serde_yaml` removed, crypto material zeroized.
**Risk**: R7 medium (API diff), R5 low.
**Estimated duration**: 4-6 commits.

#### R7 — `serde_yaml` → `serde_yml`

- [ ] **R7.1** — Inventory all uses (`grep -r "serde_yaml"` in crates/).
- [ ] **R7.2** — Swap the dep in the workspace `Cargo.toml`. `serde_yml` is an active
      API-compatible fork in most cases.
- [ ] **R7.3** — Migrate imports `serde_yaml::` → `serde_yml::`.
- [ ] **R7.4** — Run fuzz (R4) and bench (R6) to validate equivalence.
- [ ] **R7.5** — Remove `serde_yaml` from the workspace.

#### R5 — `zeroize`

- [ ] **R5.1** — Inventory crypto material: decoded public keys
      (`VerifyingKey`, `ed25519_dalek::VerifyingKey`), raw signatures, OCSP
      nonces, payload content before hashing.
- [ ] **R5.2** — Add `zeroize` (1.x) to the workspace.
- [ ] **R5.3** — Wrap in `Zeroizing<Vec<u8>>` where applicable. For external crate
      types (ed25519, p256), use `Zeroizing<Box<[u8]>>` for intermediate bytes.
- [ ] **R5.4** — Manual hash/nonce comparisons in constant-time
      (`subtle::ConstantTimeEq` if not already).
- [ ] **R5.5** — Fuzz (R4) re-run to confirm zero panics after wraps.

**Phase 5 DoD**: `cargo tree | grep serde_yaml` empty, zero `Vec<u8>` with crypto
material without `Zeroizing<>`.

---

### Phase 6 — Documentation and traceability (R13 + R14 + R9)

**Goal**: docs aligned with `AGENTS.md`, traceable papers, Bootstrap Exception
removed.
**Risk**: low.
**Estimated duration**: 3-5 commits.

#### R13 — Align docs with reality

- [ ] **R13.1** — `04_rust_refactor_guide.md`: remove mentions of `thiserror` and
      `clap` derive (forbidden by `AGENTS.md`). Replace with "roll error enums
      by hand, derive `Debug, Clone, PartialEq, Eq`".
- [ ] **R13.2** — Audit all dev-docs for recommendations that contradict
      `AGENTS.md`.
- [ ] **R13.3** — For each paper in `contracts/research/`, create an entry in
      `docs/dev-docs/.../paper_implementation_status.md`:
      ```
      | Paper | Status | Where in code | Next step |
      |---|---|---|---|
      | selfhealing-wal-crc-design-v1 | ✅ implemented | claim_wal.rs L400-500 | — |
      | AutoCodeRover | 🟡 partial | — | Fuzz targets (R4) |
      | rust-observability-selfhealing | 🔴 not started | — | R3 tracing |
      ```
- [ ] **R13.4** — `README.md`: revisit the "best practices and scientific papers"
      claim. Add an "Evidence" section linking to
      `paper_implementation_status.md`.

#### R14 — Rust technical debt audit (self-dogfood)

**Context**: the dogfood via CLI (`forge-core-cli validate --root .` on the repo
itself) revealed a structural gap: Forge only validates **YAML contracts**, it does not
audit the **structural quality of Rust code**. Today `lib.rs` with 6782 lines,
legacy `Result<_, String>`, `serde_yaml` in maintenance, functions of 493 lines —
all pass `validate` with zero diagnostics. The protocol is blind to what hurts
most.

- [ ] **R14.1** — Inventory the categories of debt Forge should flag:
      god-files (>1500 lines), god-functions (>200 lines), `Result<_, String>`,
      `process::exit` in lib code, deps in maintenance mode, absence of
      `tracing`/`zeroize` on sensitive paths.
- [ ] **R14.2** — Design a `rust_structural_health` validator in
      `forge-core-validate` that emits `Diagnostic::warning`/`error` for each
      category. Use `syn` for parsing (already a transitive dep via `serde_derive`).
- [ ] **R14.3** — Add the validator to the `run_validate` pipeline as
      *opt-in* (gate `--include-rust-health` or YAML policy), so as not to break
      consumers who only want to validate YAML contracts.
- [ ] **R14.4** — Dogfood: run the new validator on the repo itself. The goal is
      that it emits diagnostics about Forge itself — it becomes living proof of
      the protocol.
- [ ] **R14.5** — Document in `README.md` (Evidence section) that Forge audits its
      own structural health, with a screenshot of the output.

**R14 DoD**: there is a `rust_structural_health` validator in `forge-core-validate`,
running on the repo itself, emitting at least 1 warning per known category.

#### R9 — Close Bootstrap Core Exception

- [ ] **R9.1** — Inventory use of `--allow-bootstrap-core` in tests and scripts.
- [ ] **R9.2** — Configure a real sidecar for the Forge repo (`<repo-root>`
      points to a separate sidecar).
- [ ] **R9.3** — Migrate tests that use `--allow-bootstrap-core` to resolve a
      real sidecar.
- [ ] **R9.4** — Remove the flag from production code paths.
- [ ] **R9.5** — Update `CONTEXT.md` "Bootstrap Gaps" → mark as resolved.

**Phase 6 DoD**: dev-docs 100% aligned with `AGENTS.md`, every paper has status,
`--allow-bootstrap-core` removed from production paths, structural health
validator (`R14`) running on the repo itself.

---

## Consolidated execution order

```
Phase 0  ── R1 extended + R12     (structural decomposition)
            │
            ▼
Phase 1  ── R10                    (create forge-core-crypto)
            │
            ▼
Phase 2  ── R2 + R8 + R11          (error discipline)
            │
            ▼
Phase 3  ── R3                     (tracing)
            │
            ▼
Phase 4  ── R6 + R4                (bench + fuzz)
            │
            ▼
Phase 5  ── R7 + R5                (deps + zeroize)
            │
            ▼
Phase 6  ── R13 + R9               (docs + bootstrap)
```

**Rationale for the order**:
1. Phase 0 first: decomposes the god-file so that the following phases apply
   changes to small modules, not to a monolith.
2. Phase 1 (R10) after Phase 0: moves crypto to its crate **before**
   adding tracing/fuzz — otherwise, instrumentation stays in the CLI and has to
   migrate again.
3. Phase 2 before Phase 3: removing `process::exit` allows tracing to capture
   propagated errors, instead of silencing them via exit.
4. Phase 3 before Phase 4: tracing allows benchmarks to have spans;
   fuzzing benefits from typed error types (Phase 2).
5. Phase 5 independent, but after Phase 0 to reduce churn.
6. Phase 6 last: docs reflect the final reality, not the intermediate one.

---

## Total estimate

| Phase | Tracks | Commits | Sessions (~2h) |
|---|---|---|---|
| 0 | R1 extended + R12 | 8-12 | 4-6 |
| 1 | R10 | 6-10 | 3-5 |
| 2 | R2 + R8 + R11 | 8-12 | 4-6 |
| 3 | R3 | 5-8 | 2-4 |
| 4 | R6 + R4 | 4-6 | 2-3 |
| 5 | R7 + R5 | 4-6 | 2-3 |
| 6 | R13 + R14 + R9 | 5-8 | 2-3 |
| **Total** | R1-R14 | **40-64** | **19-31** |

**Trade-off**: it is possible to parallelize Phase 4 (bench/fuzz) and Phase 5 (deps/zeroize)
with Phase 2-3, but it is **not** possible to parallelize anything with Phase 0 or Phase 1.

---

## Tracking

Each track (R1-R13) has a progress file in `dev-journals/` in the sibling
repo `Forge-method-archive`. Convention:

- `r1_lib_inventory.md` (exists)
- `r10_crypto_crate.md`
- `r11_main_rs_decomposition.md`
- `r12_test_decoupling.md`
- etc.

Status of each sub-task is marked inline with commits. When a phase ends,
update this doc with date and link to commits.

---

## Risks and mitigations

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| R1.CryptoOCSP steps on Codex WIP | Medium | High | Confirm `37aa52d` stable; wait for Codex to confirm before |
| R10 breaks external callers | Low | High | Re-exports preserve the API; CLI output smoke test |
| R11 changes exit codes | Medium | Medium | Snapshot of exit code before/after; document changes |
| R7 `serde_yml` drop-in fails | Low | Medium | Do it in a separate branch; fuzz validates equivalence |
| Fuzz finds a panic | High | Medium | **Expected** — that is the goal. Document as a separate bug |
| Scope creep in R13 | High | Low | Limit to 1 session; papers without code become an issue, not work |

---

## Non-scope (explicitly out)

- Rewriting `forge-core-store` into a real DB (SQLite/LMDB) — not now.
- Async runtime everywhere — `tokio` only where it already is (reconcile loop).
- GUI/observability dashboard — Forge is CLI/library only.
- Multi-tenancy in the sidecar — one sidecar per consumer repo, by design.
- Replacing `ed25519-dalek`/`p256` with unified `RustCrypto` — no clear benefit.
