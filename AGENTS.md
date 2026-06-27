# forge-method-rust — Project Conventions

This is a Rust workspace (7 crates under `crates/`, edition 2021, resolver 2)
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
- NEVER add new `Result<_, String>` signatures. The existing ones in parsers
  (`parse_rekor_log_entry`, `required_string`, etc.) are legacy; do not
  propagate the pattern. When you touch one, consider migrating it to a named
  enum.

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
