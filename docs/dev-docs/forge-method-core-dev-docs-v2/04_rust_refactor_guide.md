# Forge Method Core v2 - Rust refactor guide

> Aligned with AGENTS.md on 2026-06-29 (R13.1). Forge rolls error enums by hand — no thiserror, no clap derive, no anyhow.

## Current problem

The repo already has good conceptual separation into crates, but some areas concentrate many responsibilities and increase the chance of agents writing bad Rust. The main goal here is to reduce the context needed for each change.

## Golden rules

1. Rust protects invariants, it does not experiment with live semantics.
2. Every new command must enter through the manual argv handling of `main.rs` (no `clap`, no derive).
3. Every public error must be a typed enum.
4. Every critical path must emit a `tracing` span or event.
5. Every large contract needs a fixture builder.
6. Every CLI JSON output needs a snapshot test.
7. No external adapter can mutate the store directly.
8. Avoid raw `String` for ids when there is domain semantics.
9. Prefer `BTreeMap` for deterministic outputs.
10. Avoid `expect` on runtime paths, except for internal invariants proven by preflight.

## CLI with manual argv (no `clap`)

Forge does NOT use `clap` or derive macros. The argv handling is manual and lives in `main.rs`, per `crates/forge-core-cli/src/main.rs`. The established pattern is:

- `main()` collects `env::args().skip(1).collect::<Vec<String>>()`.
- It matches on the first argument (subcommand) and dispatches to a `run_<command>(&args)` function.
- Each `run_<command>` does manual parsing of flags (`--foo`, `--bar value`) over `&[String]`.
- Usage errors print `usage()` to stderr and call `std::process::exit(2)`.
- Runtime errors propagate a typed enum (see next section), not `String`.

Skeleton:

```rust
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("validate");
    match command {
        "preview" => run_preview_command(&args),
        "ready" => run_ready_command(&args),
        // ...
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}
```

New subcommands enter by adding an arm to the `match` in `main.rs` and a `run_<command>(&[String])` function. Do NOT introduce `clap`, `clap_derive`, `structopt`, or any CLI derive macro.

## Errors with a hand-written typed enum (no `thiserror`)

Forge does NOT use `thiserror` or `anyhow`. Each fallible operation defines a nominal error enum next to the operation, deriving `Debug, Clone, PartialEq, Eq`. Conversions at module boundaries use `.map_err(NamedError::from)` or an explicit `impl From`. Since enums derive `Clone`, store the source as `String` (lossy) when needed — never as `Box<dyn Error>`.

Example mirroring `ExecuteOperationError` (`crates/forge-core-cli/src/execute_operation.rs`) and `ReferenceIndexBuildError` (`crates/forge-core-store/src/lib.rs`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    ReadOperation {
        path: std::path::PathBuf,
        source: String,
    },
    Validation { error_count: usize },
    ReferenceIndexBuild(forge_core_store::ReferenceIndexBuildError),
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreviewError::ReadOperation { path, source } => {
                write!(f, "read operation failed at {}: {source}", path.display())
            }
            PreviewError::Validation { error_count } => {
                write!(f, "operation validation failed with {error_count} errors")
            }
            PreviewError::ReferenceIndexBuild(err) => write!(f, "reference index build failed: {err}"),
        }
    }
}

impl From<forge_core_store::ReferenceIndexBuildError> for PreviewError {
    fn from(err: forge_core_store::ReferenceIndexBuildError) -> Self {
        PreviewError::ReferenceIndexBuild(err)
    }
}
```

Rules:

- Derive `Debug, Clone, PartialEq, Eq` on the error enum.
- Implement `Display` by hand (no `#[error(...)]`).
- NEVER `Result<_, String>` in new signatures. Use `Result<T, NamedError>`.
- NEVER `Box<dyn std::error::Error>` in public signatures.

## Tracing

```rust
#[tracing::instrument(skip(document, snapshot), fields(operation_id = %document.operation_contract.contract_id.0))]
pub fn plan_operation_with_snapshot(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
) -> RuntimePlan {
    tracing::debug!("planning operation");
    plan_operation_inner(document, Some(snapshot))
}
```

## Proposed module split for store

```txt
crates/forge-core-store/src/
  lib.rs
  paths.rs
  jsonl.rs
  reference_index.rs
  effect/
    mod.rs
    apply.rs
    wal.rs
    recovery.rs
    metadata.rs
  locks.rs
```

## Builders to reduce agent suffering

```rust
let op = OperationContractFixture::ready_mutation()
    .with_contract_id("op.write-readme")
    .with_effect_ref("contracts/effects/write-readme.yaml")
    .with_gate_status(GateStatus::Pass)
    .build();
```

## Snapshot tests

Use snapshots for CLI JSON, RuntimePlan, PreviewReport, ReadyReport, TraceEvent, and EvalComparison. This reduces invisible regression and helps agents correct output without understanding the whole system.

## Recommended codegen

The ideal pattern is to choose a single canonical source:

- If contracts are born in YAML/JSON Schema: generate Rust structs, docs, and base fixtures.
- If contracts are born in Rust: generate JSON Schema and docs automatically.

Avoid maintaining a Rust struct, YAML schema, docs, fixtures, and validator manually without generation. This multiplies agent error.
