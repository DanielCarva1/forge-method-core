# R13.1 — Align `04_rust_refactor_guide.md` with AGENTS.md

**Date:** 2026-06-29
**Scope:** docs-only. No source code (`.rs`) was modified. No other dev-docs touched.
**Target file:** `docs/dev-docs/forge-method-core-dev-docs-v2/04_rust_refactor_guide.md`

## Context

AGENTS.md forbids `thiserror`, `anyhow`, `clap` derive macros, `Result<_, String>`,
`Box<dyn Error>`, short-circuit validation, and per-crate version pins. The refactor
guide recommended exactly the forbidden crates (`clap` derive in the CLI section and
`thiserror` in the error section). This pass rewrites the offending passages to the
canonical Forge patterns documented in `AGENTS.md`.

## Passages changed

All line ranges refer to the **original** file (before R13.1).

### 1. Alignment note (new) — added under title

- **Before:** no note.
- **After (now line 3):**
  `> Aligned with AGENTS.md on 2026-06-29 (R13.1). Forge rolls error enums by hand — no thiserror, no clap derive, no anyhow.`
- **Why:** required by task spec.

### 2. Regras de ouro, rule 2 — original lines 10

- **Before:** `2. Todo comando novo deve entrar por \`clap\` derive.`
- **What was wrong:** recommends `clap` derive, forbidden by AGENTS.md.
- **After (now line 12):** `2. Todo comando novo deve entrar pelo argv handling manual de \`main.rs\` (sem \`clap\`, sem derive).`

### 3. Section "CLI com clap" — original lines 20–59

- **Before:** full `clap`-based example using `use clap::{Args, Parser, Subcommand}`,
  `#[derive(Debug, Parser)]`, `#[derive(Debug, Subcommand)]`, `#[derive(Debug, Args)]`,
  `#[command(...)]`, `#[arg(...)]`.
- **What was wrong:** end-to-end recommendation of `clap` derive macros, the exact
  pattern AGENTS.md forbids.
- **After (now lines 22–50):** section renamed "CLI com argv manual (sem `clap`)". Replaced
  the clap example with:
  - a prose description of the established `main.rs` argv-handling style
    (`env::args().skip(1).collect::<Vec<String>>()`, match on first arg, dispatch to
    `run_<command>(&args)`, `std::process::exit(2)` on usage errors);
  - a minimal skeleton mirroring `crates/forge-core-cli/src/main.rs`;
  - an explicit "NAO introduzir `clap`, `clap_derive`, `structopt` ou qualquer derive
    macro de CLI" closing line.

### 4. Section "Erros com thiserror" — original lines 61–79

- **Before:** example using `#[derive(Debug, thiserror::Error)]`,
  `#[error("...")]`, and `#[source]` / `#[from]` attributes; stored `source: std::io::Error`.
- **What was wrong:** depends on `thiserror`, which AGENTS.md forbids. Also stores the
  raw `std::io::Error` as the source, which makes the enum non-`Clone`.
- **After (now lines 52–95):** section renamed "Erros com enum tipado feito a mao (sem
  `thiserror`)". Replaced the example with:
  - a hand-rolled `#[derive(Debug, Clone, PartialEq, Eq)]` enum mirroring
    `ExecuteOperationError` and `ReferenceIndexBuildError`;
  - a hand-written `impl Display`;
  - an explicit `impl From<forge_core_store::ReferenceIndexBuildError> for PreviewError`
    demonstrating the `.map_err(NamedError::from)` / `From` convention;
  - the source stored as lossy `String` (because the enum must stay `Clone`);
  - a bullet list restating the rules: derive `Debug, Clone, PartialEq, Eq`; `Display`
    by hand; never `Result<_, String>`; never `Box<dyn std::error::Error>`.

## Passages reviewed and left unchanged

The following sections contain no AGENTS.md violations and were preserved verbatim:

- "Problema atual" (intro).
- Regras de ouro 1, 3–10 (only rule 2 was offending).
- "Tracing" section — uses `#[tracing::instrument]`, which is unrelated to the error/
  CLI conventions and is consistent with rule 4 ("Todo caminho critico deve emitir
  `tracing` span ou event").
- "Module split proposto para store" — directory layout, no crate/pattern advice.
- "Builders para reduzir sofrimento dos agentes" — fixture-builder snippet, allowed.
- "Snapshot tests" — testing guidance, allowed.
- "Codegen recomendado" — codegen strategy, allowed.

## Verification of forbidden tokens

After the edits, a full read of the file confirms none of the following tokens appear
in any **recommendation** (they appear only inside explicit "NAO usar" / "forbidden"
sentences, which is correct):

- `thiserror` — only in negation ("sem `thiserror`", "Forge NAO usa `thiserror`").
- `anyhow` — only in negation ("no anyhow", "Forge NAO usa `thiserror` nem `anyhow`").
- `clap` — only in negation ("sem `clap`", "NAO introduzir `clap`") and in the new
  section title that announces its absence.
- `#[derive(... Parser)]`, `#[derive(... Subcommand)]`, `#[derive(... Args)]`,
  `#[error(...)]`, `#[source]`, `#[from]` — gone entirely.
- `Result<_, String>` — appears only in a "NUNCA" rule.
- `Box<dyn std::error::Error>` — appears only in a "NUNCA" rule.

## No source code modified

R13.1 is docs-only. No `.rs` file was read-for-edit, written, or touched. The only
artifacts modified are:

- `docs/dev-docs/forge-method-core-dev-docs-v2/04_rust_refactor_guide.md` (edited in place)
- `docs/dev-docs/forge-method-core-dev-docs-v2/progress/r13_doc_alignment.md` (this file, new)

No commit was made, per task instructions.

## Open questions

1. **`ExecuteOperationError` derives only `Debug`.** The actual
   `crates/forge-core-cli/src/execute_operation.rs` enum derives `#[derive(Debug)]`
   only (no `Clone, PartialEq, Eq`), while `ReferenceIndexBuildError` follows the full
   `Debug, Clone, PartialEq, Eq` form. AGENTS.md states the convention is
   `Debug, Clone, PartialEq, Eq`. The rewritten doc follows AGENTS.md (the stricter
   form). If a maintainer wants the doc to acknowledge that `ExecuteOperationError`
   currently only derives `Debug`, that nuance is not captured here — left for a
   future doc/cleanup pass.

2. **"Codegen recomendado" mentions generating Rust structs from YAML/JSON Schema.**
   This is not forbidden by AGENTS.md, but it does imply a build-time codegen step
   that is not currently present in the workspace (no `build.rs` doing schema→struct
   generation was found). Not a violation; flagged only because it describes an
   aspirational pattern rather than an existing one. Left unchanged.

3. **Tracing example references `RuntimeReadSnapshot<'_>` and
   `plan_operation_with_snapshot`.** Not checked for drift against the current
   `forge-core-runtime` API. Outside the R13.1 scope (error/CLI conventions only);
   flagged for a possible future drift audit.
