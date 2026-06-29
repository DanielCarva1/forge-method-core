# Forge Method Core v2 - guia de refatoracao Rust

> Aligned with AGENTS.md on 2026-06-29 (R13.1). Forge rolls error enums by hand — no thiserror, no clap derive, no anyhow.

## Problema atual

O repo ja tem boa separacao conceitual em crates, mas algumas areas concentram muitas responsabilidades e aumentam a chance de agentes escreverem Rust ruim. O principal objetivo aqui e reduzir o contexto necessario para cada alteracao.

## Regras de ouro

1. Rust protege invariantes, nao experimenta semantica viva.
2. Todo comando novo deve entrar pelo argv handling manual de `main.rs` (sem `clap`, sem derive).
3. Todo erro publico deve ser enum tipado.
4. Todo caminho critico deve emitir `tracing` span ou event.
5. Todo contrato grande precisa de builder de fixture.
6. Todo output JSON de CLI precisa de snapshot test.
7. Nenhum adapter externo pode mutar store diretamente.
8. Evitar `String` crua para ids quando houver semantica de dominio.
9. Preferir `BTreeMap` para outputs deterministicos.
10. Evitar `expect` em paths de runtime, exceto invariantes internas provadas por preflight.

## CLI com argv manual (sem `clap`)

Forge NAO usa `clap` nem derive macros. O argv handling e manual e vive em `main.rs`, conforme `crates/forge-core-cli/src/main.rs`. O padrao estabelecido e:

- `main()` coleta `env::args().skip(1).collect::<Vec<String>>()`.
- Faz match no primeiro argumento (subcomando) e despacha para uma funcao `run_<command>(&args)`.
- Cada `run_<command>` faz parsing manual de flags (`--foo`, `--bar valor`) sobre `&[String]`.
- Erros de uso imprimem `usage()` em stderr e chamam `std::process::exit(2)`.
- Erros de runtime propagam um enum tipado (ver secao seguinte), nao `String`.

Esqueleto:

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

Novos subcomandos entram adicionando um braco no `match` de `main.rs` e uma funcao `run_<command>(&[String])`. NAO introduzir `clap`, `clap_derive`, `structopt` ou qualquer derive macro de CLI.

## Erros com enum tipado feito a mao (sem `thiserror`)

Forge NAO usa `thiserror` nem `anyhow`. Cada operacao falhavel define um enum de erro nominal ao lado da operacao, derivando `Debug, Clone, PartialEq, Eq`. Conversoes em fronteiras de modulo usam `.map_err(NamedError::from)` ou um `impl From` explicito. Como enums derivam `Clone`, guarde a fonte como `String` (lossy) quando precisar — nunca como `Box<dyn Error>`.

Exemplo espelhando `ExecuteOperationError` (`crates/forge-core-cli/src/execute_operation.rs`) e `ReferenceIndexBuildError` (`crates/forge-core-store/src/lib.rs`):

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

Regras:

- Derivar `Debug, Clone, PartialEq, Eq` no enum de erro.
- Implementar `Display` a mao (sem `#[error(...)]`).
- NUNCA `Result<_, String>` em assinaturas novas. Use `Result<T, NamedError>`.
- NUNCA `Box<dyn std::error::Error>` em assinaturas publicas.

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
