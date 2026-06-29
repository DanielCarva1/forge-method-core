# R11 — main.rs Decomposition

## Goal

Decompose the 4117-line god-file `crates/forge-core-cli/src/main.rs` into
per-family `*_cmd.rs` modules, leaving `main.rs` as a pure top-level dispatcher
(< 200 lines).

## Status: COMPLETE

- **main.rs: 4117 → 107 lines** (-4010, **97.4% reduction**)
- All 9 commits green: `cargo check`, `cargo test -p forge-core-cli --lib`
  (104/104), `cargo fmt --check`, `cargo clippy --pedantic` (baseline parity)
- Regression anchor: `validate --root . --json` → `"diagnostics": []`

## Commits

| Hash | Step | Description | main.rs |
|---|---|---|---|
| `fc69caa` | R11.1 | `cli_util.rs` — 24 shared helpers (next_arg, parse_*, usage, emit_envelope, resolve_stateful_*, StatefulCommandRoots, resolve_now_unix) | 4117 → 3807 |
| `89fdfa1` | R11.2 | `host_adapter_verify_cmd.rs` — 13 verify dispatchers (artifact, provenance, rekor, sigstore, fulcio, CT, CRL, OCSP, TUF) | 3807 → 2689 |
| `e6bbc5a` | R11.3 | `host_adapter_policy_cmd.rs` — 6 policy/admit/projection/manifest dispatchers | 2689 → 2355 |
| `e11fe30` | R11.4 | Extended `graph_cmd.rs` with 7 graph dispatchers | 2355 → 2197 |
| `9cfd608` | R11.5-R11.7 | Extended `eval_cmd.rs` (5) + `telemetry_cmd.rs` (5) + `m1_cmd.rs` (5) with dispatchers | 2197 → 1760 |
| `ec4ca97` | R11.8-R11.10 | Extended `validate.rs` (1) + `execute_operation.rs` (1) + `effect_index.rs` (3) with dispatchers | 1760 → 1429 |
| `4903194` | R11.11 | Extended `guide.rs` with 9 guide dispatchers | 1429 → 1199 |
| `72f263c` | R11.12 | Extended `claim.rs` with 12 claim dispatchers; moved `resolve_now_unix` to cli_util | 1199 → 572 |
| `debb56e` | R11.13-R11.14 | Extended `isolation.rs` (6) + `project_cmd.rs` (1) + `coordination.rs` (1); cleaned unused imports; migrated guide_value test | 572 → 107 |

## Architecture after R11

`main.rs` (107 lines) is now a pure top-level dispatcher:

```rust
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("validate");
    match command {
        "guide" => forge_core_cli::guide::run_guide_command(&args),
        "claim" => forge_core_cli::claim::run_claim_command(&args),
        // ... 34 more arms, each a single delegation to module::run_*_command
        _ => { eprintln!("{}", usage()); std::process::exit(2); }
    }
}
```

All business logic (argv parsing, typed input construction, entrypoint calls,
JSON/text output, exit-code propagation) now lives in the `*_cmd.rs` modules
or their sibling logic modules.

### Module layout (crates/forge-core-cli/src/)

| Module | Lines | Owns |
|---|---|---|
| `main.rs` | 107 | Top-level dispatcher only |
| `lib.rs` | 97 | mod declarations + re-exports |
| `cli_util.rs` | 341 | Shared helpers (25 pub fns) |
| `host_adapter_verify_cmd.rs` | 1150 | 13 verify dispatchers |
| `host_adapter_policy_cmd.rs` | 353 | 6 policy/admit dispatchers |
| `claim.rs` | 2141 | Claim logic + 12 dispatchers |
| `telemetry_cmd.rs` | 1401 | Telemetry logic + dispatchers |
| `project_cmd.rs` | 1263 | Project logic + dispatcher |
| `isolation.rs` | 1192 | Isolation logic + 6 dispatchers |
| `graph_cmd.rs` | 1100 | Graph logic + 7 dispatchers |
| `guide.rs` | 818 | Guide logic + 9 dispatchers |
| `m1_cmd.rs` | 732 | M1 logic + 5 dispatchers |
| `validate.rs` | 669 | Validate logic + dispatcher |
| `eval_cmd.rs` | 563 | Eval logic + 5 dispatchers |
| `execute_operation.rs` | 493 | Execute-op logic + dispatcher |
| `effect_index.rs` | 300 | Effect-index logic + 3 dispatchers |
| Others (coordination, autonomy_cmd, contract_cmd, etc.) | — | Pre-existing modules |

## Key decisions

1. **Helpers are `pub`, not `pub(crate)`**: `main.rs` is the binary entrypoint,
   which treats `forge_core_cli` as an external crate. `pub(crate)` items are
   invisible to the binary. All items in `cli_util.rs` and the dispatcher
   functions in `*_cmd.rs` are `pub`. This matches the existing pattern in
   `eval_cmd`/`graph_cmd`/`telemetry_cmd`/`m1_cmd`.

2. **Modules promoted to `pub`**: `validate`, `execute_operation`, and
   `effect_index` were `pub(crate)` and had to be promoted to `pub` so
   `main.rs` can resolve them.

3. **`resolve_now_unix` moved to `cli_util.rs`**: Was in the claim block but
   is used by both claim and isolation dispatchers. Moved to the shared
   helpers module to avoid cross-module coupling.

4. **Behavior preservation**: Every commit verified with
   `cargo run -q -p forge-core-cli -- validate --root . --json` →
   `"diagnostics": []`. No observable change in CLI behavior.

5. **Test migration**: The `guide_value_requires_present_non_flag_value` unit
   test was in `main.rs`'s `#[cfg(test)] mod tests` and tested `guide_value`,
   which moved to `guide.rs`. The test was migrated to `guide.rs`'s test
   module (104 tests total, +1 vs original 103).

## Pitfalls encountered

1. **`use crate::cli_util::*` does NOT pick up `pub(crate)` items** when the
   consumer is the binary entrypoint (main.rs). Solution: make items `pub`.

2. **`sed -i 's|run_X|module::run_X|g'` also replaces the function DEFINITION
   name**, producing invalid syntax like `fn module::run_X(...)`. This is
   harmless because the definition is deleted in the same step, but worth
   noting.

3. **`cargo fix --bin` is invaluable** for cleaning up unused imports after
   bulk extraction. Applied once at the end (R11.14) to remove 10 stale
   imports from main.rs.

4. **Heredoc with `'EOF'` quoting** is the safest way to write Rust doc
   comments with backticks to a temp file via shell. Unquoted heredocs
   mangle backticks; `printf` and `echo` have their own quoting issues.

## Next steps

R11 is complete. The roadmap (`09_system_design_roadmap.md`) suggests:

- **R8** (Phase 2): Remove `process::exit` from lib code (5 sites in
  `autonomy_cmd.rs` and `contract_cmd.rs`). Now that all dispatchers are in
  lib modules, a `CliError` enum can be defined and used.
- **R12** (Phase 0): Migrate tests from `tests/validate.rs` (5215 lines) to
  `forge-core-crypto/tests/` where they test crypto verification.
- **R3** (Phase 3): Structured tracing.
- **R7** (Phase 5): `serde_yaml` → `serde_yml`.
- **R5** (Phase 5): `zeroize` for crypto material.
