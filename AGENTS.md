# forge-method-core — Project Conventions

This is a Rust workspace (10 crates under `crates/`, edition 2021, resolver 2)
for the forge-method core. It uses heavy crypto (ed25519-dalek, p256,
sigstore-tsa, rcgen, x509-parser) and canonical serde.

These conventions are **project-specific** and override the generic
`rust-conventions` skill. They are always loaded when working in this repo.

## Error handling (differs from standard Rust)

**This project does NOT use `anyhow` or `thiserror`.** Roll error enums by hand.
Do not add those crates.

- Define a named error enum per fallible operation, next to the operation.
  Derive `Debug, Clone, PartialEq, Eq`.
  Existing examples to mirror:
  - `ExecuteOperationError` (`crates/forge-core-cli/src/lib.rs`)
  - `EffectStoreLockError`, `ReferenceIndexBuildError`, `AppendJsonLineError`
    (`crates/forge-core-store/src/lib.rs`)
- Convert with `.map_err(NamedError::from)` / explicit `From` impl at module
  boundaries. Since enums derive `Clone`, store a lossy `String` for the source
  when needed.
- NEVER add new `Result<_, String>` signatures. The last residual sites
  (`parse_rekor_log_entry`, `required_string`, `parse_signed_checkpoint`)
  were migrated to typed errors (`RekorParseError` in
  `crates/forge-core-crypto/src/rekor.rs`) and moved out of the CLI as
  part of the R10 crypto-extraction. If you encounter a stray `Result<_, String>`
  anywhere in the workspace, migrate it to a named enum.

## Validation = accumulating diagnostics (do NOT short-circuit)

Validation does not bail on the first error. Collect every problem into a
`ValidationReport` and let the caller decide via `report.has_errors()`.

- `Diagnostic` fields are fixed:
  `severity: DiagnosticSeverity`, `code: DiagnosticCode`, `path: String`,
  `message: String` (`crates/forge-core-validate/src/lib.rs`).
- Use the constructors `Diagnostic::error(...)` / `Diagnostic::warning(...)`
  when they exist, rather than building the struct inline.
- Only `?`-bail out of a validation pass if an input is structurally unusable.

## Workspace dependencies

Use the shared versions in the root `Cargo.toml` `[workspace.dependencies]`
(`serde.workspace = true`, etc.). Do not add per-crate version pins that
diverge, and do not introduce rival (de)serializers.

## Toolchain & verification

Built on WSL via the Windows toolchain. WSL shims expose the bare names, so
both `cargo` and `cargo.exe` work — prefer `cargo`.

**Verification is automatic.** A `pi-green-loop` hook (config in this repo's
`pi-green-loop.json`) runs after each edit turn and reports failures back:
`cargo check --workspace`, `cargo clippy --workspace --all-targets --
-W clippy::pedantic`, `cargo test --workspace`, and `cargo fmt --all --
--check`. You normally do not need to run these manually. `/green` runs them
now; `/green on|off` toggles the auto-fix loop.

## Context hygiene (avoid context-window degradation)

Model quality degrades past ~150k tokens of accumulated context, but the
window runs to 1M. There is no auto-compaction knob in ZCode. Mitigate by
working in short, comittable sessions.

- **Work per-story, not per-epic.** One story (e.g. F06.2) per session:
  work → commit → `/clear`. Sessions are cheap; context degradation is not.
- **Self-monitor context load.** The agent cannot read the live token counter,
  but it tracks the volume of large reads (big files, dense `git log`/commit
  dumps, long tool outputs). When the agent estimates it has accumulated
  ~150-200k tokens in a session, it MUST pause and warn the user before
  continuing. Do not push past this into degradation.
- **`/clear` between unrelated tasks.** New task = clean context.

## Editor stability (WSL + Windows + rust-analyzer)

**Symptom**: the host editor process (Zed, VS Code, Codex) dies with
`memory allocation of 8388608 bytes failed` / `0xC0000409`
(STATUS_STACK_BUFFER_OVERRUN) when this workspace is open.

**Root cause (measured)**: `target/debug` accumulates ~130k files from
incremental builds. Without an explicit exclude, the rust-analyzer indexer
walks every one of them on startup, which (over WSL-on-NTFS, where each
syscall is expensive) explodes the Windows commit-charge and OOMs the host
process. The `.gitignore` already excludes `target/` from git, but
rust-analyzer does NOT read `.gitignore` for its file scan by default.

**Mitigation** (already in place):

- `rust-analyzer.toml` at the repo root sets `files.excludeDirs` to skip
  `target`, `target-test`, `.forge-method`, `fuzz/corpus`, `docs/fixtures`,
  and `contracts/risk-audits/fixtures`, plus `cargo.checkOnSave = false` so
  a second `cargo` does not run in parallel with r-a's own analysis.
- Periodically purge accumulated test tempdirs that leak under `target/`
  (they match the pattern `*-[0-9]+$`). Use:
  ```
  ls target | grep -E -- '-[0-9]+$' | (cd target && xargs -r rm -rf)
  ```
- When `target/debug` itself becomes too large to `du`, run `cargo clean`
  and rebuild.
- Never run two Rust toolchains in parallel (e.g. `cargo check` in one
  terminal while the editor's r-a is also checking). One cargo at a time.
