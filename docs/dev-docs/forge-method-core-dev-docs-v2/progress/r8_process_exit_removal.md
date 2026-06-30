# R8 — Remove `process::exit` from lib code (in progress)

Branch: `codex/forge-frust-052-ocsp-boundary`
Started: 2026-06-29
Status: **in progress** (~48% complete)

## Goal

Eliminate every `std::process::exit` call from `crates/forge-core-cli/src/`
except the single top-level call in `main.rs` that converts an
`ExitError` into the right shell exit code. Behavior must remain
byte-identical (same stdout JSON, same stderr text, same exit codes).

## Approach

1. **R8.1** — Add a typed `ExitError` enum (`cli_error.rs`) that mirrors
   the historical exit-code contract (`Usage`=2, `Failed`=1,
   `InvalidValue`=3, `Conflict`=4, `EnvConfig`=5, `WithCode`=n).
2. **R8.2** — Add `*_or_err` variants of the legacy exit-on-error helpers
   in `cli_util.rs`. The legacy helpers stay in place so each dispatcher
   can be migrated independently; they will be deleted once every caller
   uses the new variants.
3. **R8.3+** — Migrate one `*_cmd.rs` per commit, changing the
   dispatcher signature to `Result<(), ExitError>` and propagating with
   `?`. `main.rs` maps the `Err` back to `std::process::exit(code)` at
   each dispatcher arm.
4. **R8.final** — Update `main.rs` to call `std::process::exit` exactly
   once (a single error-mapping block at the top of `main`), then delete
   the legacy exit-on-error helpers in `cli_util.rs`.

## Inventory and progress

Initial count: **146** `process::exit` calls in lib code (excluding
`main.rs` and tests).

| File                          | Initial | Retired | Status     | Commit (R8.x) |
|-------------------------------|---------|---------|------------|---------------|
| `autonomy_cmd.rs`             | 2       | 2       | done       | R8.3          |
| `validate.rs`                 | 3       | 3       | done       | R8.4          |
| `contract_cmd.rs`             | 4       | 4       | done       | R8.5          |
| `execute_operation.rs`        | 4       | 4       | done       | R8.6          |
| `effect_index.rs`             | 5       | 5       | done       | R8.7          |
| `coordination.rs`             | 1       | 1       | done       | R8.8          |
| `project_cmd.rs`              | 1       | 1       | done       | R8.8          |
| `guide.rs`                    | 6       | 6       | done       | R8.9          |
| `isolation.rs`                | 6       | 6       | done       | R8.10         |
| `m1_cmd.rs`                   | 9       | 9       | done       | R8.11         |
| `telemetry_cmd.rs`            | 8       | 8       | done       | R8.12         |
| `eval_cmd.rs`                 | 10      | 10      | done       | R8.13         |
| `graph_cmd.rs`                | 11      | 11      | done       | R8.14         |
| `claim.rs`                    | 11      | 0       | pending    | R8.15         |
| `host_adapter_policy_cmd.rs`  | 10      | 0       | pending    | R8.16         |
| `cli_util.rs`                 | 16      | 0       | pending    | R8.17         |
| `host_adapter_verify_cmd.rs`  | 39      | 0       | pending    | R8.18         |
| **Total**                     | **146** | **70**  | **48%**    |               |

## Validation

Each commit is verified with:

- `cargo check -p forge-core-cli`
- `cargo test -p forge-core-cli --lib` (109 tests, +5 from R8.1)
- `cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic`
- `cargo run -q -p forge-core-cli -- validate --root . --json` → 122
  occurrences of `"diagnostics": 0` (the regression anchor)
- Manual smoke test of the migrated subcommand (success path + at least
  one error path with the expected exit code)

## Behavior preservation

The migration is **byte-identical** at the shell level:

- stdout JSON envelopes keep the same shape and field order
- stderr text-mode failures keep the same `command failed: message` format
- exit codes match the historical contract (1 = failed, 2 = usage,
  3 = invalid value, 4 = conflict, 5 = env/config)

The only behavioral change is internal: dispatchers no longer kill the
process from inside library code. They return `Result<(), ExitError>`
and `main.rs` is the single place that calls `std::process::exit`.

## Pitfalls encountered

1. **`emit_envelope` callers** needed a `_or_err` variant because the
   legacy helper exits with a code derived from the envelope, which can
   be 0 (success). The `_or_err` variant returns `Ok(())` on code 0 and
   `Err(ExitError::WithCode)` otherwise, so callers can short-circuit
   with `?` without losing the success path.

2. **`--help` mid-parser** is awkward to express when the parser returns
   a fully-formed input struct. Resolved by checking for `--help` /
   `-h` in the dispatcher (before calling the parser) so the parser
   never has to fabricate a dummy input.

3. **`pub` visibility**: `main.rs` is the binary entrypoint and treats
   `forge_core_cli` as an external crate. All migrated helpers and
   types must be `pub` (not `pub(crate)`) so the binary can see them.

4. **Pre-existing warnings**: `ClaimReconcileLoopConfig` private-interface
   warning in `claim.rs` is unrelated to R8 and not a regression.
