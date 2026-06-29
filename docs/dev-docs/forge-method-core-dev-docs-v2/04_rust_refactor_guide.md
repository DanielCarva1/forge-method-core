# Forge Method Core v2 - guia de refatoracao Rust

## Problema atual

O repo ja tem boa separacao conceitual em crates, mas algumas areas concentram muitas responsabilidades e aumentam a chance de agentes escreverem Rust ruim. O principal objetivo aqui e reduzir o contexto necessario para cada alteracao.

## Regras de ouro

1. Rust protege invariantes, nao experimenta semantica viva.
2. Todo comando novo deve entrar por `clap` derive.
3. Todo erro publico deve ser enum tipado.
4. Todo caminho critico deve emitir `tracing` span ou event.
5. Todo contrato grande precisa de builder de fixture.
6. Todo output JSON de CLI precisa de snapshot test.
7. Nenhum adapter externo pode mutar store diretamente.
8. Evitar `String` crua para ids quando houver semantica de dominio.
9. Preferir `BTreeMap` para outputs deterministicos.
10. Evitar `expect` em paths de runtime, exceto invariantes internas provadas por preflight.

## CLI com clap

Antes: parsing manual por `env::args`, loop de indice e `process::exit`.

Depois:

```rust
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "forge-core")]
#[command(version, about = "Forge Method Core runtime")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate(ValidateArgs),
    Preview(PreviewArgs),
    Ready(ReadyArgs),
    Explain(ExplainArgs),
    Graph(GraphCommand),
    Memory(MemoryCommand),
    Protocol(ProtocolCommand),
}

#[derive(Debug, Args)]
pub struct PreviewArgs {
    #[arg(long, default_value = ".")]
    pub root: std::path::PathBuf,

    #[arg(long)]
    pub operation: std::path::PathBuf,

    #[arg(long)]
    pub json: bool,
}
```

## Erros com thiserror

```rust
#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("read operation failed at {path}: {source}")]
    ReadOperation {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("operation validation failed with {error_count} errors")]
    Validation { error_count: usize },

    #[error("reference index build failed: {0}")]
    ReferenceIndex(#[from] forge_core_store::ReferenceIndexBuildError),
}
```

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

## Module split proposto para store

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

## Builders para reduzir sofrimento dos agentes

```rust
let op = OperationContractFixture::ready_mutation()
    .with_contract_id("op.write-readme")
    .with_effect_ref("contracts/effects/write-readme.yaml")
    .with_gate_status(GateStatus::Pass)
    .build();
```

## Snapshot tests

Usar snapshots para CLI JSON, RuntimePlan, PreviewReport, ReadyReport, TraceEvent e EvalComparison. Isso reduz regressao invisivel e ajuda agentes a corrigirem output sem entender todo o sistema.

## Codegen recomendado

O padrao ideal e escolher uma fonte canonica:

- Se contratos nascem em YAML/JSON Schema: gerar Rust structs, docs e fixtures base.
- Se contratos nascem em Rust: gerar JSON Schema e docs automaticamente.

Evitar manter manualmente Rust struct, schema YAML, docs, fixtures e validator sem geracao. Isso multiplica erro de agente.
