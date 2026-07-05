# ADR-0012 - Canonical diagnostic accumulator + const-table of codes

- **Status**: Accepted (V1.B foundation implemented — `DiagnosticCodeDef`, `declare_diagnostic_code!`, `DiagnosticRegistry`; V2.B migrated 4 families: graph/eval/eval-harness/CLI to the canonical `ValidationReport`/`Diagnostic`)
- **Date**: 2026-07-02
- **Track**: V1.B / V2.B — canonical diagnostic accumulator
- **Supersedes**: none
- **Superseded by**: none

## Context

There were **five cloned diagnostic type families** in the workspace:
`forge-core-validate` (`ValidationReport`/`Diagnostic`/`DiagnosticCode`),
`forge-core-graph` (`GraphValidationReport` with its own `error_count`),
`forge-core-eval`, `forge-core-eval-harness`, and the CLI. Each declared its own
severity/code/message structure and its own counts.

Worse: the canonical `DiagnosticCode` was an enum of ~90 variants, and at the CLI
boundary it was **degraded to `format!("{:?}")`** — a `Debug` string. The result was
ugly wire-strings (`YamlReadFailed`) instead of stable identifiers (`yaml_read_failed`),
and the type information was lost crossing the JSON/MCP boundary. Every consumption site
(MCP, agent) had to re-parse strings or lose the code.

Research across the largest Rust linters and rust-analyzer showed clear convergence:

- **rustc** (`DiagCtxt`): accumulate-then-`abort_if_errors`, with phase gates. Lint
  metadata is a **`static` table** (struct `Lint`), not an enum variant carrying its
  own description/severity.
- **`rust-analyzer`'s own source** *recommends against* a strongly-typed enum for
  diagnostics: it couples the set of known diagnostics to a single compilation unit and
  makes external/config-driven diagnostics impossible.
- **`clippy` / `deno_lint` / `dprint`**: none uses an enum for `code`. All declare codes
  as a `const`/`static` table of `&'static str` identifiers, with severity/category as
  **data** alongside the id, emitted by a declaration macro
  (`declare_clippy_lint!`, `deno_lint::declare_deno_lint!`).

## Decision

The `forge-core-validate`'s `Diagnostic` / `ValidationReport` become the **canon**. The
other four families migrate to it (V2.B).

### 1. `DiagnosticCodeDef` const-table + `declare_diagnostic_code!`

The const-table entry is a struct of `&'static str` + default severity, modeled on
rustc's `Lint` and the `clippy`/`deno_lint` approach:

```rust
pub struct DiagnosticCodeDef {
    pub code: &'static str,           // "memory_authority_floor" — stable wire format
    pub description: &'static str,
    pub category: &'static str,       // "memory", "graph", "risk-audit"
    pub default_severity: DiagnosticSeverity,
}
```

The `declare_diagnostic_code!` macro emits a `pub static <NAME>: DiagnosticCodeDef` per
row — it mirrors rustc's `declare_tool_lint!` and `declare_clippy_lint!`. The `$sev` is an
unqualified variant name (`Error`/`Warning`) so the call site reads like rustc's lint-level
decl. **Not** a proc-macro: `macro_rules!`, zero build cost.

The wire-format `code` is the stable snake_case `&'static str` from the start — never the
`Debug` of a variant. This keeps type information across the JSON/MCP boundary.

### 2. `DiagnosticRegistry` — the lookup seam

A registry built from a `&'static [&'static DiagnosticCodeDef]` slice,
const-constructable, read-only. Consumers query by code string → metadata. This replaces
the giant enum-`match` that forced `format!("{:?}")` at the CLI boundary.

### 3. `ValidationReport::abort_if_errors` — the phase-gate

`ValidationReport` gains `abort_if_errors(self) -> Result<(), Self>` modeled on
rustc's `DiagCtxt::abort_if_errors`: it runs the entire phase, accumulates *all*
diagnostics, then decides whether to stop. Instead of short-circuiting on the first error,
it lets the full phase run and collect every problem before stopping. It consumes `self`
so the success path can drop the empty/warnings-only report.

### 4. Migration without a flag-day

The 4 clones migrate via **type aliases + `From` impls**, not a flag-day. The
`GraphValidationReport` (with its own `error_count`) becomes an alias for the canonical
`ValidationReport`; `error_count`/`warning_count` were added to the canonical to preserve
the counters. The accumulator rule from `AGENTS.md` ("accumulate all diagnostics before
stopping") is now enforced at a single seam.

## Consequences

**Positive:**

- `DiagnosticCode` stops being stringified at boundaries. A programmatic consumer (MCP,
  agent) receives a stable `code` like `yaml_read_failed`, not `format!("{:?}")`.
- The `AGENTS.md` accumulator rule is enforced at one seam: `ValidationReport::extend` +
  `abort_if_errors`. Four `has_errors` clones become one.
- Future config-driven severity overrides (the ESLint/rustc lint-level model: a code
  declares its *default* level, and a consumer config can promote/demote) have a ready
  lookup seam — the `DiagnosticRegistry`. A config entry `codes: { memory_authority_floor: warn }`
  resolves through the registry.
- The ~90-variant enum and the const-table coexist (the const-table is an additive
  foundation); V2.B migrates callers and the enum variants shrink as each one migrates to
  the corresponding const-table entry.

**Negative:**

- Two sources of truth (the ~90-variant enum + const-table seed) coexist until full
  migration. Mitigation: the seed is additive, the enum is untouched until V2.B; V2.B
  deletes variants as it migrates each caller to the corresponding const entry.
- `DiagnosticCodeDef` requires new codes to be declared **and** added to the registry slice
  (`SEED_ENTRIES`). Forgetting the last leaves the code non-lookupable. Mitigation: a test
  iterates the registry and verifies that every declared resolvable `code` appears.

## Anti-goals

- **Does not** replace the existing enum in V1.B (additive foundation); migration is V2.B,
  caller by caller.
- **Does not** introduce config-driven severity in this ADR — it only provides the lookup
  seam (`DiagnosticRegistry`) that will make it possible later.
- **Is not** a proc-macro: `declare_diagnostic_code!` is `macro_rules!`.

## References

- rustc `DiagCtxt` (accumulate-then-`abort_if_errors`, phase gates):
  https://doc.rust-lang.org/nightly/nightly-rustc/rustc_errors/diagnostic.html
- rust-analyzer `AnyDiagnostic` self-critique (recommendation against a strongly-typed enum for
  diagnostics).
- `clippy` `declare_clippy_lint!` (const-table): https://github.com/rust-lang/rust-clippy
- `deno_lint` const-table: https://docs.rs/deno_lint
- `garde::Report` (canonical validation accumulator): https://docs.rs/garde
- `jsonschema` `OutputFormat::Basic` (canonical report): https://docs.rs/jsonschema
- In-repo: ADR-0001 (deterministic kernel, validation is a pure decision),
  `crates/forge-core-validate/src/{lib.rs (ValidationReport/abort_if_errors), codes.rs}`.
