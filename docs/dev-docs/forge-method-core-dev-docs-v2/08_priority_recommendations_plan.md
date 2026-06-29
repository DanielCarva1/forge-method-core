# Plano de Ação — 9 Recomendações Prioritárias

Data: 2026-06-29
Status: Planejado, execução iniciando pela R1
Origem: análise crítica do projeto em 2026-06-29
Vínculo: `AGENTS.md` (regras do projeto), `04_rust_refactor_guide.md`, `01_feature_specs.md`,
`contracts/research/community-trends-and-requested-features-v1.yaml`,
`contracts/research/best-features-from-papers-and-cases-v1.yaml`

## Conflito de fontes (importante)

O `04_rust_refactor_guide.md` sugere `thiserror` e `clap` derive. O `AGENTS.md`
do projeto **proíbe explicitamente `thiserror`/`anyhow`** e manda rolar enums
na mão. Este plano segue o `AGENTS.md` (regra de projeto tem precedência sobre
doc de refatoração mais antigo). `clap` derive é permitido — não está proibido.

## Princípios orientadores

1. **Rust ou Rust-compatível em tudo**: nenhuma nova dep non-Rust. Fuzz com
   `cargo-fuzz` (Rust), benchmarks com `criterion` (Rust), observabilidade com
   `tracing` (Rust).
2. **Melhores práticas e papers**: cada recomendação cita o paper/caso que
   justifica a prática (RADAR, AutoCodeRover, SLSA, CSA, CodeCRDT, etc.).
3. **Não pisar no WIP do Codex**: branch `codex/forge-frust-052-ocsp-boundary`
   tem mudanças não commitadas em `Cargo.toml`, `forge-core-cli/src/lib.rs`,
   `main.rs`, `tests/validate.rs`. Trabalho novo vai em arquivos novos ou em
   arquivos não tocados por ele.
4. **Green-loop automático**: cada etapa deve manter `cargo check`, `clippy
   pedantic`, `cargo test`, `cargo fmt --check` verdes.
5. **Passos pequenos**: cada sub-tarefa é ~1 commit, ~1 arquivo principal,
   validável isoladamente.

## Mapa de recomendações → papers/features

| Recomendação | Paper/caso de apoio | Feature do backlog |
|---|---|---|
| R1 Decompor god-file | F15 (Rust ergonomics) | — |
| R2 Migrar `Result<_, String>` | F15 | — |
| R3 Adicionar `tracing` | F03 (TraceEvent canonico) | DEM-06 (evals/cost telemetry) |
| R4 Adicionar `cargo-fuzz` | AutoCodeRover (fault localization) | F11 (Risk Audit Gate) |
| R5 Adicionar `zeroize` | SLSA AI-agent (FEAT-14) | FEAT-13 (sandbox policy) |
| R6 Benchmark `criterion` | DEM-06, FEAT-07 (eval bank) | F13 (Budget/Cost) |
| R7 Migrar `serde_yaml` → `serde_yml` | — (depreciação de ecossistema) | — |
| R8 Remover `process::exit` de lib | F15 | — |
| R9 Fechar Bootstrap Exception | `CONTEXT.md` "Remaining Bootstrap Gaps" | F12 (Guided Start) |

---

## R1 — Decompor `forge-core-cli/src/lib.rs` (7463 linhas)

**Por que**: god-file concentra 30% do código do CLI. Aumenta contexto que o
agente precisa ler pra mudar qualquer coisa. Viola "reduzir contexto necessário"
(F15).

**Papers/casos**: F15 do `feature_backlog.csv`; AutoCodeRover mostra que
estrutura de programa é fator de resolução (FEAT-04).

**Meta**: `lib.rs` ≤ 1500 linhas, módulos ≤ 1500 linhas cada, sem mudança de
behavior.

### R1.1 — Inventariar `lib.rs`
- [ ] Listar todos os `pub fn`/`pub struct`/`pub enum` com linha
- [ ] Agrupar por domínio (crypto/verify, rekor, project link, claim, etc.)
- [ ] Identificar dependências entre grupos
- [ ] Documentar em `docs/dev-docs/.../r1_lib_inventory.md`

### R1.2 — Criar módulos-alvo (esqueleto)
- [ ] `crates/forge-core-cli/src/crypto_rekor.rs` (parse_rekor, verify_rekor)
- [ ] `crates/forge-core-cli/src/crypto_x509.rs` (cert/crl/ocsp verify)
- [ ] `crates/forge-core-cli/src/project_link.rs` (resolve, init helpers)
- [ ] `crates/forge-core-cli/src/execute_operation.rs` (ExecuteOperationError + flow)
- [ ] Cada módulo: só `pub use` no `lib.rs`, sem lógica

### R1.3 — Mover `parse_rekor_log_entry` e helpers
- [ ] Mover `parse_rekor_log_entry`, `required_string`, `required_i64`,
      `required_u64`, `parse_signed_checkpoint` para `crypto_rekor.rs`
- [ ] Manter `pub use` em `lib.rs` pra não quebrar callers
- [ ] Rodar `cargo test --workspace`

### R1.4 — Mover verificação X.509/CRL/OCSP
- [ ] Mover funções de `verify_signature`, `verify_crl`, `verify_ocsp` para
      `crypto_x509.rs`
- [ ] `pub use` em `lib.rs`
- [ ] Rodar testes

### R1.5 — Mover `execute_operation` flow
- [ ] Mover `ExecuteOperationError` e função principal para
      `execute_operation.rs`
- [ ] `pub use` em `lib.rs`
- [ ] Rodar testes

### R1.6 — Mover project link resolve/init
- [ ] Para `project_link.rs` (cuidado: `project_cmd.rs` já existe — coordenar)
- [ ] Rodar testes

### R1.7 — Validar
- [ ] `lib.rs` ≤ 1500 linhas
- [ ] `cargo clippy --workspace --all-targets -- -W clippy::pedantic` verde
- [ ] `cargo test --workspace` verde
- [ ] Snapshot de output de CLI inalterado

---

## R2 — Migrar 17 `Result<_, String>` para enums nomeados

**Por que**: `AGENTS.md` proíbe explicitamente. Erros `String` não são
exaustivos, não carregam contexto estruturado, quebram `?` em boundaries.

**Papers/casos**: F15; "structural bug prevention type-level"
(`structural-bug-prevention-typelevel-v1.yaml`).

**Meta**: zero `Result<_, String>` em código de produção (exclui `#[cfg(test)]`).

### R2.1 — Inventariar
- [ ] Listar 17 sites com arquivo:linha:assinatura
- [ ] Classificar por domínio (parse, validate, isolation, store, engine)
- [ ] Documentar em `docs/dev-docs/.../r2_string_result_inventory.md`

### R2.2 — `contract_cmd.rs` (3 sites)
- [ ] `validate_kind` → `ContractValidationError` enum
- [ ] `parse_document` → reusar `ContractValidationError`
- [ ] Atualizar callers em `contract_cmd.rs` e `main.rs`
- [ ] Testes

### R2.3 — `isolation.rs` (2 sites)
- [ ] `parse_merge_policy` → `MergePolicyParseError`
- [ ] `parse_status` → `IsolationStatusParseError`
- [ ] Atualizar callers
- [ ] Testes

### R2.4 — `lib.rs` rekor parsers (5 sites)
- [ ] `parse_rekor_log_entry` → `RekorParseError`
- [ ] `required_string`/`required_i64`/`required_u64` → `RekorFieldError`
- [ ] `parse_signed_checkpoint` → `CheckpointParseError`
- [ ] `verify_rekor_*` (2 sites com `Result<(), String>`) → reusar
- [ ] Atualizar callers
- [ ] Testes

### R2.5 — `main.rs` (1 site)
- [ ] `StatefulCommandRoots` builder → `StatefulCommandRootsError`
- [ ] Testes

### R2.6 — `forge-core-engine` (3 sites)
- [ ] `catalog.rs::load_one` → `CatalogLoadError`
- [ ] `catalog.rs::parse_workflow_yaml` → reusar
- [ ] `eval.rs::load_eval_corpus` → `EvalCorpusLoadError`
- [ ] `isolation.rs::shell_metachar_check` → `ShellMetacharError`
- [ ] Testes

### R2.7 — `forge-core-store` (1 site)
- [ ] `lib.rs:1371` → `EffectWalReadError` (já existe `ReferenceIndexBuildError`
      como padrão)
- [ ] Testes

### R2.8 — Validar
- [ ] `grep -rn "Result<.*String>" crates --include="*.rs" | grep -v /tests/`
      retorna 0
- [ ] `cargo test --workspace` verde

---

## R3 — Adicionar `tracing` estruturado

**Por que**: `eprintln!` não é observabilidade. Multi-agente em CI precisa de
spans correlacionados. Paper: F03 (TraceEvent canonico); DEM-06 (evals/cost
telemetry); FEAT-15 (prompt-injection detection telemetry exige runtime
observability).

**Papers/casos**: F03, DEM-06, FEAT-15, `rust-observability-selfhealing-v1.yaml`.

**Meta**: spans em todo caminho crítico (claim acquire/release, WAL append,
execute_operation, verify_rekor). Default `tracing_subscriber` com env-filter.
Sem `eprintln!` em lib code (exceção: `main.rs` antes de subscriber init).

### R3.1 — Adicionar deps
- [ ] `tracing`, `tracing-subscriber` em `[workspace.dependencies]`
- [ ] Adicionar às deps de `forge-core-store`, `forge-core-runtime`,
      `forge-core-cli`, `forge-core-validate`
- [ ] `cargo check`

### R3.2 — Init subscriber em `main.rs`
- [ ] `tracing_subscriber::fmt().with_env_filter().with_writer(std::io::stderr).init()`
- [ ] Gatear com feature `tracing` (sempre on por enquanto)
- [ ] Teste: rodar CLI com `RUST_LOG=info` e ver spans

### R3.3 — Spans em `claim_wal.rs`
- [ ] `#[instrument(skip(self), fields(claim_id, seq))]` em `append`,
      `read`, `rotate`
- [ ] Eventos em erros de CRC, lock contention
- [ ] Teste

### R3.4 — Spans em `execute_operation`
- [ ] Span em torno de spawn, validate, verify
- [ ] Eventos em `RuntimeCommandExecutionStatus::Blocked`/`Failed`
- [ ] Teste

### R3.5 — Spans em `verify_rekor`/`verify_x509`
- [ ] `#[instrument(skip(...))]` em cada função pública
- [ ] Eventos em mismatch de assinatura
- [ ] Teste

### R3.6 — Remover `eprintln!` de lib code
- [ ] Substituir por `tracing::warn!`/`tracing::error!`
- [ ] Manter `eprintln!` só em `main.rs` pré-init
- [ ] `cargo test`

### R3.7 — Validar
- [ ] `grep -rn "eprintln!" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
      retorna 0
- [ ] `cargo test --workspace` verde
- [ ] Snapshot de CLI output inalterado (logs vão pra stderr, não stdout)

---

## R4 — Adicionar `cargo-fuzz`

**Por que**: parsers de YAML não-confiável (`parse_rekor_log_entry`,
`parse_signed_checkpoint`) e decode de WAL binário são superfície de ataque
clássica pra panic/DoS. Sem fuzz, "excelência em segurança" é alegação.

**Papers/casos**: AutoCodeRover (fault localization, FEAT-05); CSA/ARMO (FEAT-15);
`rust-testing-defenses-v1.yaml`.

**Meta**: 3 fuzz targets rodando 60s sem panic.

### R4.1 — Setup
- [ ] `cargo install cargo-fuzz` (instrução no README, não no CI)
- [ ] Criar `crates/fuzz/` como workspace member separado (não atrapalha build
      normal)
- [ ] `Cargo.toml` com `cargo-fuzz` e `libfuzzer-sys`
- [ ] Adicionar ao workspace com `default-members` excluindo `fuzz`

### R4.2 — Fuzz target 1: `parse_rekor_log_entry`
- [ ] `fuzz_targets/parse_rekor.rs`
- [ ] Seed corpus com 3 entradas reais (válida, malformada, adversarial)
- [ ] Rodar 60s, sem panic

### R4.3 — Fuzz target 2: `parse_signed_checkpoint`
- [ ] `fuzz_targets/parse_checkpoint.rs`
- [ ] Seed com checkpoint real do Rekor
- [ ] Rodar 60s, sem panic

### R4.4 — Fuzz target 3: `claim_wal` decode
- [ ] `fuzz_targets/claim_wal_decode.rs`
- [ ] Feed de bytes arbitrários no decoder
- [ ] Rodar 60s, sem panic (erros tipados ok, panic não)

### R4.5 — Documentar
- [ ] Seção no README de como rodar fuzz
- [ ] Adicionar ao `06_protocol_security_plan.md`

---

## R5 — Adicionar `zeroize` para material cripto

**Por que**: chaves/assinaturas ficam na memória até GC. Pra um runtime que
verifica assinaturas de agentes externos, isso é higiene mínima. SLSA AI-agent
(FEAT-14) exige proveniência de material cripto.

**Papers/casos**: SLSA AI-agent proposal (FEAT-14); ARMO/CSA (FEAT-13, FEAT-15).

**Meta**: qualquer `SigningKey`/`VerifyingKey`/`Signature` que entre em escopo
de função é `Zeroize`-on-drop.

### R5.1 — Inventariar material cripto
- [ ] Listar todos os sites de `SigningKey`, `VerifyingKey`, `Signature`,
      `SecretKey` em `crates/`
- [ ] Confirmar: Forge só verifica (não assina)? Se sim, `VerifyingKey`/`Signature`
      são públicas — `zeroize` é nice-to-have, não crítico
- [ ] Documentar em `docs/dev-docs/.../r5_crypto_inventory.md`

### R5.2 — Adicionar dep
- [ ] `zeroize = { version = "1.8", features = ["derive"] }` em
      `[workspace.dependencies]`
- [ ] Adicionar a `forge-core-cli`
- [ ] `cargo check`

### R5.3 — Wrap em `Zeroizing<>`
- [ ] Em `verify_rekor_*`, `verify_x509_*`: wrap `VerifyingKey` temporários em
      `Zeroizing`
- [ ] Em `parse_rekor_log_entry`: `Signature` parsed em `Zeroizing`
- [ ] Testes existentes continuam verdes

### R5.4 — Constant-time em comparações manuais
- [ ] Procurar `==` em bytes cripto (fora de `verify()` das crates)
- [ ] Substituir por `subtle::ConstantTimeEq` se houver
- [ ] Se não houver (provável), documentar no plano

### R5.5 — Validar
- [ ] `cargo test --workspace` verde
- [ ] Seção no `06_protocol_security_plan.md`

---

## R6 — Benchmark `criterion`

**Por que**: alegação de "performance" sem métrica é marketing. DEM-06 pede
evals com cost/latency. FEAT-07 pede eval bank com latency.

**Papers/casos**: DEM-06, FEAT-07, `agentic-throughput-and-fast-quality-mode-v1.yaml`.

**Meta**: 3 benchmarks rodando em <30s, números no README.

### R6.1 — Setup
- [ ] `criterion = "0.5"` em `[workspace.dependencies]`
- [ ] `[[bench]]` em `forge-core-store`, `forge-core-validate`
- [ ] `benches/` dir com harness
- [ ] `cargo bench --no-run` compila

### R6.2 — Bench WAL append
- [ ] `benches/claim_wal_append.rs`
- [ ] Cenários: 100, 1000, 10000 records
- [ ] Medir: append + fsync + projection
- [ ] Rodar, registrar baseline em `docs/dev-docs/.../r6_bench_baseline.md`

### R6.3 — Bench validate
- [ ] `benches/validate_report.rs`
- [ ] Cenários: 10, 100, 1000 diagnostics
- [ ] Medir: build + serialize
- [ ] Rodar, registrar baseline

### R6.4 — Bench rekor verify
- [ ] `benches/rekor_verify.rs`
- [ ] Cenário: 1 entry real
- [ ] Medir: parse + verify
- [ ] Rodar, registrar baseline

### R6.5 — Documentar
- [ ] Seção no README com números
- [ ] Adicionar ao `05_eval_and_quality_plan.md`

---

## R7 — Migrar `serde_yaml` → `serde_yml`

**Por que**: `serde_yaml` 0.9.34 está em maintenance mode, descontinuado pelo
dtolnay. `serde_yml` é o sucessor mantido.

**Meta**: zero `serde_yaml` no `Cargo.lock`, todos os `use serde_yaml` viram
`use serde_yml`.

### R7.1 — Inventariar
- [ ] `grep -rn "serde_yaml" crates --include="*.rs" --include="Cargo.toml"`
- [ ] Listar APIs usadas (`from_str`, `to_string`, `Value`, etc.)
- [ ] Confirmar compatibilidade em `serde_yml` 0.0.12+

### R7.2 — Adicionar `serde_yml` e remover `serde_yaml`
- [ ] `serde_yml = "0.0.12"` em `[workspace.dependencies]`
- [ ] Remover `serde_yaml` de `[workspace.dependencies]`
- [ ] Atualizar cada crate `Cargo.toml`
- [ ] `cargo check`

### R7.3 — Migrar imports
- [ ] `sed -i 's/serde_yaml/serde_yml/g'` em cada `.rs` (cuidado com
      `serde_yaml::Value` vs `serde_yml::Value`)
- [ ] Rodar `cargo check --workspace`
- [ ] Rodar `cargo test --workspace`

### R7.4 — Validar
- [ ] `grep -rn "serde_yaml" crates` retorna 0
- [ ] Snapshot de output inalterado

---

## R8 — Remover `std::process::exit` de lib code

**Por que**: `exit` em lib code quebra testes, impõe controle de fluxo
não-local, impede composição. Anti-pattern em Rust.

**Meta**: `std::process::exit` só em `main.rs` e `bin/`.

### R8.1 — Inventariar
- [ ] `grep -rn "process::exit" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
- [ ] Lista: `autonomy_cmd.rs:404,426`, `contract_cmd.rs:43,75,187,215`

### R8.2 — `contract_cmd.rs`
- [ ] Trocar `process::exit(2)` por `return Err(...)` propagado
- [ ] Caller em `main.rs` decide exit code
- [ ] Testes

### R8.3 — `autonomy_cmd.rs`
- [ ] Mesma abordagem
- [ ] Testes

### R8.4 — Validar
- [ ] `grep -rn "process::exit" crates --include="*.rs" | grep -v /tests/ | grep -v main.rs`
      retorna 0
- [ ] `cargo test --workspace` verde
- [ ] Exit codes de CLI inalterados (testar com `assert_cmd`)

---

## R9 — Fechar Bootstrap Core Exception

**Por que**: `CONTEXT.md` admite gap. Promessa "consumer-ready" depende disso.
Sem fechar, todo consumer repo precisa da exceção, o que viola isolamento.

**Papers/casos**: `CONTEXT.md` "Remaining Bootstrap Gaps"; F12 (Guided Start).

**Meta**: `--allow-bootstrap-core` removido (ou só pra testes internos),
consumer repo init funciona sem state local.

### R9.1 — Inventariar uso da exceção
- [ ] `grep -rn "allow_bootstrap_core\|allow-bootstrap-core" crates contracts`
- [ ] Listar todos os sites que dependem da exceção
- [ ] Documentar em `docs/dev-docs/.../r9_bootstrap_inventory.md`

### R9.2 — Confirmar sidecar init funciona
- [ ] Rodar `forge-core project init --root <tmp consumer>` num repo limpo
- [ ] Verificar se `.forge-method.yaml` aponta pra sidecar
- [ ] Verificar se state-bearing commands resolvem sem `--allow-bootstrap-core`

### R9.3 — Migrar tests que usam a exceção
- [ ] Para cada teste com `--allow-bootstrap-core`, criar versão sidecar
- [ ] Manter a exceção só em `#[cfg(test)]` interno do forge-core

### R9.4 — Remover exceção de paths de produção
- [ ] `project_cmd.rs`: negar `state_root` consumer-local sem flag de teste
- [ ] Runtime/claim commands: fail-closed sem sidecar
- [ ] Atualizar `CONTEXT.md` removendo "Remaining Bootstrap Gaps"

### R9.5 — Validar
- [ ] `cargo test --workspace` verde
- [ ] E2E: consumer repo limpo → init → execute-operation → sem exceção

---

## Ordem de execução

1. **R1** (decompor god-file) — libera contexto pra todas as outras
2. **R2** (migrar `Result<_, String>`) — mais fácil depois de R1
3. **R8** (remover `process::exit`) — mais fácil depois de R1
4. **R7** (migrar `serde_yaml`) — mecânico, independente
5. **R3** (adicionar `tracing`) — depois de R1/R8
6. **R6** (benchmark) — depois de R3 pra medir com spans
7. **R4** (fuzz) — depois de R2 (erros tipados)
8. **R5** (zeroize) — independente, mas depois de R1
9. **R9** (bootstrap) — mais estrutural, por último

## Tracking

Cada recomendação terá um arquivo de progresso em
`docs/dev-docs/forge-method-core-dev-docs-v2/progress/` conforme for iniciada.
