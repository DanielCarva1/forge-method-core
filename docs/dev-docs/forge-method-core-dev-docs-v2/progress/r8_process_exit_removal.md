# R8 — Remove `process::exit` from lib code — **COMPLETE**

Branch: `codex/forge-frust-052-ocsp-boundary`
Started: 2026-06-29
Completed: 2026-06-29
Status: **complete** (146/146 calls retired)

## Goal

Eliminate every `std::process::exit` call from `crates/forge-core-cli/src/`
except the single top-level call in `main.rs` that converts an
`ExitError` into the right shell exit code. Behavior must remain
byte-identical (same stdout JSON, same stderr text, same exit codes).

## Result

The crate now has **exactly one** `std::process::exit` call (in `main.rs`)
and **zero** in lib code. Every dispatcher returns `Result<(), ExitError>`
and the binary entrypoint converts the typed error back into a shell exit
code in a single match block.

## Approach

1. **R8.1** — Added a typed `ExitError` enum (`cli_error.rs`) that mirrors
   the historical exit-code contract (`Usage`=2, `Failed`=1,
   `InvalidValue`=3, `Conflict`=4, `EnvConfig`=5, `WithCode`=n).
2. **R8.2** — Added `*_or_err` variants of the legacy exit-on-error helpers
   in `cli_util.rs`. The legacy helpers stayed in place while each
   dispatcher migrated independently.
3. **R8.3–R8.17** — Migrated one `*_cmd.rs` per commit, changing the
   dispatcher signature to `Result<(), ExitError>` and propagating with
   `?`. `main.rs` mapped the `Err` back to `std::process::exit(code)` at
   each dispatcher arm.
4. **R8.18** — Deleted the legacy exit-on-error helpers from `cli_util.rs`
   now that every dispatcher uses the `*_or_err` variants.
5. **R8.final** — Rewrote `main.rs` to capture every dispatcher's
   `Result<(), ExitError>` into a single binding, then convert it to a
   shell exit code in one place. Removed the duplicated per-arm
   Ok/Err blocks.

## Inventory

Initial count: **146** `process::exit` calls in lib code (excluding
`main.rs` and tests).

| File                          | Initial | Retired | Commit (R8.x) |
|-------------------------------|---------|---------|---------------|
| `autonomy_cmd.rs`             | 2       | 2       | R8.3          |
| `validate.rs`                 | 3       | 3       | R8.4          |
| `contract_cmd.rs`             | 4       | 4       | R8.5          |
| `execute_operation.rs`        | 4       | 4       | R8.6          |
| `effect_index.rs`             | 5       | 5       | R8.7          |
| `coordination.rs`             | 1       | 1       | R8.8          |
| `project_cmd.rs`              | 1       | 1       | R8.8          |
| `guide.rs`                    | 6       | 6       | R8.9          |
| `isolation.rs`                | 6       | 6       | R8.10         |
| `m1_cmd.rs`                   | 9       | 9       | R8.11         |
| `telemetry_cmd.rs`            | 8       | 8       | R8.12         |
| `eval_cmd.rs`                 | 10      | 10      | R8.13         |
| `graph_cmd.rs`                | 11      | 11      | R8.14         |
| `claim.rs`                    | 11      | 11      | R8.15         |
| `host_adapter_policy_cmd.rs`  | 10      | 10      | R8.16         |
| `host_adapter_verify_cmd.rs`  | 39      | 39      | R8.17         |
| `cli_util.rs` (legacy helpers)| 16      | 16      | R8.18         |
| **Total**                     | **146** | **146** | **R8.final**  |

## Validation

Each commit was verified with:

- `cargo check -p forge-core-cli` — passed
- `cargo test -p forge-core-cli --lib` — 109 tests passed (104 original + 5 new in `cli_error.rs`)
- `cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic` — no errors, ~436 warnings (down from ~590 baseline; the legacy helpers were contributing warnings)
- `cargo run -q -p forge-core-cli -- validate --root . --json` — 122 occurrences of `"diagnostics": 0` (the regression anchor)
- `cargo fmt --all -- --check` — passed
- `cargo test --workspace` — passes, except `validate_binary_outputs_json_summary` which is a **pre-existing** failure (case mismatch `Passed` vs `passed`) unrelated to R8

## Behavior preservation

The migration is **byte-identical** at the shell level:

- stdout JSON envelopes keep the same shape and field order
- stderr text-mode failures keep the same `command failed: message` format
- exit codes match the historical contract (1 = failed, 2 = usage,
  3 = invalid value, 4 = conflict, 5 = env/config)

The only behavioral change is internal: dispatchers no longer kill the
process from inside library code. They return `Result<(), ExitError>`
and `main.rs` is the single place that calls `std::process::exit`.

## Pitfalls encountered and resolved

1. **`emit_envelope` callers** needed a `_or_err` variant because the
   legacy helper exits with a code derived from the envelope, which can
   be 0 (success). The `_or_err` variant returns `Ok(())` on code 0 and
   `Err(ExitError::WithCode)` otherwise, so callers can short-circuit
   with `?` without losing the success path.

2. **`--help` mid-parser** was awkward to express when the parser returns
   a fully-formed input struct. Resolved by checking for `--help` /
   `-h` in the dispatcher (before calling the parser) so the parser
   never has to fabricate a dummy input.

3. **`pub` visibility**: `main.rs` is the binary entrypoint and treats
   `forge_core_cli` as an external crate. All migrated helpers and
   types must be `pub` (not `pub(crate)`) so the binary can see them.

4. **Local `require_value`**: `autonomy_cmd.rs` and `contract_cmd.rs`
   have their OWN local `require_value` (different signature from the
   cli_util one). They were not affected by the deletion of the cli_util
   legacy helper.

5. **`emit_envelope` cross-crate references**: when migrating claim.rs
   and isolation.rs, the legacy `emit_envelope` had to be called as
   `crate::cli_util::emit_envelope_or_err` to disambiguate from the
   crate-level re-export that no longer exists.

6. **`run_claim_reconcile_loop_or_exit` was `-> !`**: this signature
   means the function never returns. Migration to `Result` required
   renaming to `run_claim_reconcile_loop_or_err` and returning
   `Result<(), ExitError>` with the dynamic tick-loop exit code
   propagated via `ExitError::WithCode`.

## DoD for Fase 2 / R8 — achieved

- ✅ zero `process::exit` in `crates/*/src/` (except 1 in `main.rs` top)
- ✅ zero `Result<_, String>` in code touched by R8 (the legacy sites in `forge-core-store` and `forge-core-crypto` are tracked separately under R2)
- ✅ `main.rs` reduced to a single dispatcher with a single error mapping
- ✅ every dispatcher returns `Result<(), ExitError>` and can be unit-tested as a plain function
