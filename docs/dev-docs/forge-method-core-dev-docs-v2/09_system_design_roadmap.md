# System Design Roadmap — Saneamento Estrutural do Forge Method Core

**Data**: 2026-06-29
**Status**: Planejamento (após conversa crítica com Daniel)
**Substitui/estende**: `08_priority_recommendations_plan.md` (R1-R9) — R1-R9 fica como
"quick wins", este roadmap endereça os débitos estruturais que R1-R9 não cobre.

## Contexto

A análise crítica do system design (2026-06-29) identificou que o design *de caixa-preta*
(crates, contratos tipados, project link/sidecar) é sólido (~7-8/10), mas o design
*interno* tem quatro débitos estruturais que comprometem a narrativa de "excelência
em Rust":

1. **God-file** — `forge-core-cli/src/lib.rs` com 6771 linhas, 113 funções, função
   mais longa de 493 linhas. Mistura 7 domínios.
2. **Cripto-na-CLI** — 14 funções de verificação criptográfica pesada
   (`run_host_adapter_*_verification`: sigstore, fulcio, rekor, CT, CRL, OCSP, TUF,
   timestamp authority) vivem em `forge-core-cli`, que deveria ser só presentation.
3. **`main.rs` monolítico** — 4116 linhas com 141 `process::exit`, tornando o
   entrypoint não-testável como unidade.
4. **Testes acoplados à CLI** — `tests/validate.rs` com 5215 linhas importa lógica
   criptográfica da CLI pra testar; mudança de formato quebra teste de cripto e
   vice-versa.

R1-R9 do plano original endereça **parcialmente** R1 (god-file), mas:
- R1 para em "≤1500 linhas" (ainda é god-file)
- R8 só cobre `process::exit` em **lib code**, não em `main.rs`
- Não há recomendação para mover cripto pra fora da CLI
- Não há recomendação para desconectar `tests/validate.rs` da CLI

Este roadmap completa R1-R9 com 4 novas faixas (R10-R13) e reordena a execução.

---

## Princípios

1. **Behavior preservation first** — nenhuma etapa muda o output observável da CLI.
   Snapshots de `--json` são âncora de regressão.
2. **Testes migram com o código** — quando código sai da CLI, teste correspondente
   sai de `tests/validate.rs`.
3. **Uma camada por vez** — refactor estrutural antes de adicionário
   instrumentação (tracing, bench, fuzz). Caso contrário, tracing vira churn num
   god-file.
4. **Cada commit compila e testa verde** — sem big-bang refactors.
5. **Crates são a unidade de isolamento** — quando um domínio (crypto, runtime,
   store) tem >3000 linhas, é candidato a crate próprio.
6. **Papers são evidência, não decoração** — cada recomendação cita o paper; quando
   o código que implementa o paper existe, o link é bidirecional.

---

## Mapa de débitos → papers/features

| Débito | Papers/casos de apoio | Feature do backlog | Faixa |
|---|---|---|---|
| God-file `lib.rs` | F15 (Rust ergonomics); AutoCodeRover (FEAT-04) | — | R1 |
| Cripto na CLI | SLSA AI-agent (FEAT-14); F11 (Risk Audit Gate) | FEAT-13 sandbox | **R10** |
| `main.rs` monolítico + exits | F15; "Testing in Production" (Torres) | F12 (Guided Start) | **R11** |
| Testes acoplados à CLI | AutoCodeRover (test isolation) | F11 | **R12** |
| Doc divergence (`04_rust_refactor_guide`) | — (housekeeping) | — | **R13** |
| `Result<_, String>` legacy | F15 | — | R2 |
| Sem `tracing` | F03 (TraceEvent canônico); DEM-06 | FEAT-07 eval bank | R3 |
| Sem fuzz | AutoCodeRover (fault localization) | F11 | R4 |
| Sem `zeroize` | SLSA AI-agent (FEAT-14) | FEAT-13 | R5 |
| Sem benchmark | DEM-06; FEAT-07 | F13 (Budget/Cost) | R6 |
| `serde_yaml` deprecated | — (ecossistema) | — | R7 |
| `process::exit` em lib | F15 | — | R8 |
| Bootstrap Exception | `CONTEXT.md` | F12 | R9 |

---

## Fases (ordem de execução)

### Fase 0 — Decomposição estrutural (R1 estendido + R12)

**Meta**: `lib.rs ≤ 500 linhas`, todos os módulos ≤ 1500, `tests/validate.rs` ≤ 2000.
**Risco**: médio (mexer em código criptográfico é delicado, mas é move puro).
**Duração estimada**: 8-12 commits.

#### R1.A — Completar split do `lib.rs` (já em andamento)

Sub-faixas restantes (em ordem de risco crescente):

- [x] R1.3 — `crypto_rekor.rs` ✓
- [x] R1.5 — `execute_operation.rs` ✓
- [x] R1.EffectIndex — `effect_index.rs` ✓
- [x] R1.CryptoHashing — `crypto_hashing.rs` ✓
- [ ] **R1.HostAdapterTypes** — linhas 81-900 (~70 tipos `HostAdapter*`, sem lógica).
      Baixo risco, alto valor: −820 linhas.
- [ ] **R1.HostCommand** — `host_command`, `command_process_admission`,
      `argv_has_shell_control`, `env_key_is_forbidden`, `source_ref_is_immutable`,
      `version_like`. ~200 linhas.
- [ ] **R1.HostAdapterManifest** — `run_host_adapter_manifest` (493 linhas!).
      **Dividir antes de mover**: quebrar em `build_command_section`,
      `build_distribution_section`, `build_security_section` etc.
- [ ] **R1.HostAdapterProjection** — `run_host_adapter_projection`,
      `process_security_policy`, `invocation_admission`, `project_host_command`,
      `mcp_annotations`, `command_input_schema`.
- [ ] **R1.Validate** — `run_validate`, `validate_operation_fixtures`,
      `validate_side_contracts`, `validate_runtime_contracts`, helpers. ~400 linhas.
      Vai junto com `validate_helpers.rs` (`read_yaml`, `yaml_files`).
- [ ] **R1.CryptoOCSP** — `decode_ocsp_response` + 12 helpers OCSP.
      **Cuidado**: WIP do Codex (FRUST-052) tocou aqui. Confirmar estado estável
      antes.
- [ ] **R1.CryptoTUFDateTime** — `verify_tuf_metadata_freshness_role`,
      `parse_tuf_datetime_utc_to_unix`, helpers de calendário gregoriano.
- [ ] **R1.CryptoSigstore** — `verify_sigstore_*`, `verify_fulcio_chain`, etc.
      ~1270 linhas. **Maior faixa isolada** — provável candidato a crate próprio
      (ver R10).
- [ ] **R1.CryptoSLSATransparency** — `verify_slsa_statement`,
      `verify_transparency_log_proof`, `verify_merkle_inclusion`.
- [ ] **R1.HostAdapterVerification** — as 14 `run_host_adapter_*_verification`
      públicas. **Estas vão para o crate novo em R10**, não ficam na CLI.

#### R12 — Desacoplar `tests/validate.rs` da CLI

`tests/validate.rs` (5215 linhas) importa da CLI coisas que não são de CLI:
contrato parsing, cripto verify, etc.

- [ ] **R12.1** — Inventariar o que `tests/validate.rs` realmente testa:
      contract flows (deve ir pra `forge-core-contracts`), crypto flows (vai pra
      `forge-core-crypto` em R10), CLI flows (fica).
- [ ] **R12.2** — Mover testes de contract parsing para
      `crates/forge-core-contracts/tests/` ou `forge-core-validate/tests/`.
- [ ] **R12.3** — Mover testes de crypto verification para o futuro
      `forge-core-crypto/tests/` (após R10).
- [ ] **R12.4** — Reduzir `tests/validate.rs` a testes de **apresentação da CLI**:
      JSON shape, exit codes, help text, argv parsing.
- [ ] **R12.5** — Snapshot test da saída `--json` de cada subcomando como
      âncora de regressão.

**DoD Fase 0**: `lib.rs ≤ 500 linhas`, `tests/validate.rs ≤ 2000 linhas`, todos os
gates verdes, snapshot de CLI output inalterado.

---

### Fase 1 — Mover cripto para fora da CLI (R10)

**Meta**: criar `forge-core-crypto` crate e mover as 14 funções de verificação +
helpers OCSP/CRL/sigstore/CT/TSA.
**Risco**: alto (maior refactor estrutural; toca testes, main.rs, lib.rs).
**Duração estimada**: 6-10 commits.

#### R10.1 — Criar `crates/forge-core-crypto/` esqueleto

- [ ] `Cargo.toml` com deps cripto (`asn1-rs`, `base64`, `ed25519-dalek`, `p256`,
      `rasn`, `rasn-ocsp`, `sha1`, `sha2`, `sct`, `sigstore-tsa`,
      `rustls-pki-types`, `x509-parser`).
- [ ] Depende de `forge-core-contracts` (para tipos de contrato de verificação).
- [ ] Não depende de `forge-core-cli` nem de `forge-core-runtime`.
- [ ] Adicionar ao workspace `members`.

#### R10.2 — Mover módulos crypto da CLI

Em ordem:

- [ ] `crypto_hashing.rs` (já isolado na CLI) → `forge-core-crypto/src/hashing.rs`
- [ ] `crypto_rekor.rs` → `forge-core-crypto/src/rekor.rs`
- [ ] `crypto_ocsp.rs` (a criar em R1.CryptoOCSP) → `forge-core-crypto/src/ocsp.rs`
- [ ] `crypto_sigstore.rs` (a criar em R1.CryptoSigstore) →
      `forge-core-crypto/src/sigstore.rs`
- [ ] `crypto_slsa_transparency.rs` → `forge-core-crypto/src/slsa.rs`
- [ ] As 14 `run_host_adapter_*_verification` →
      `forge-core-crypto/src/host_adapter_verification.rs`

#### R10.3 — Mover testes correspondentes

- [ ] De `tests/validate.rs` para `crates/forge-core-crypto/tests/`.
- [ ] Atualizar imports nos testes: de `forge_core_cli::*` para
      `forge_core_crypto::*`.

#### R10.4 — CLI vira cliente fino

- [ ] `forge-core-cli/Cargo.toml` adiciona `forge-core-crypto` como dep.
- [ ] `lib.rs` faz `pub use forge_core_crypto::*` (transitivo) ou call sites
      atualizados.
- [ ] `main.rs` chama `forge_core_crypto::run_host_adapter_*_verification`.

#### R10.5 — DoD

- [ ] `forge-core-cli/src/lib.rs` < 1500 linhas (só host adapter types + manifest
      + validate).
- [ ] `forge-core-crypto` tem zero deps em `forge-core-cli` ou `forge-core-runtime`.
- [ ] Todos os gates verdes.
- [ ] CLI output snapshot inalterado.

---

### Fase 2 — Disciplina de erro (R2 + R8 + R11 parcial)

**Meta**: zero `Result<_, String>` novo, zero `process::exit` em lib **e** em
main.rs, erros propagam por `Result` até o topo.
**Risco**: médio (R11 muda fluxo de erro mas não behavior).
**Duração estimada**: 8-12 commits.

#### R2 — Migrar `Result<_, String>` residuais

Inventário recente mostrou que só **1 site** sobra em `forge-core-store/src/lib.rs`.
Os 17 originais do plano foram parcialmente migrados por trabalho anterior.

- [ ] **R2.1** — Confirmar inventário atual (grep por `Result<.*, String>`).
- [ ] **R2.2** — Migrar o site em `forge-core-store`.
- [ ] **R2.3** — Adicionar lint `clippy::result_large_err` ou custom check CI
      rejeitando novos `Result<_, String>`.

#### R8 — Remover `process::exit` de lib code

- [ ] **R8.1** — Inventariar (grep `process::exit` em `crates/*/src/`).
- [ ] **R8.2** — `contract_cmd.rs`, `autonomy_cmd.rs` (mencionados no plano).
- [ ] **R8.3** — Substituir por `Result<T, CliError>` propagando até `main.rs`.

#### R11 — Decompor `main.rs` (4116 linhas, 141 exits)

`main.rs` é o **entrypoint monolítico**: parse argv, dispatch, format output,
exit. Hoje tudo num arquivo só.

- [ ] **R11.1** — Inventariar sub-comandos em `main.rs`.
- [ ] **R11.2** — Criar `crates/forge-core-cli/src/commands/` com um módulo por
      família: `validate_cmd.rs`, `execute_operation_cmd.rs`,
      `claim_cmd.rs`, `host_adapter_cmd.rs`, etc.
- [ ] **R11.3** — Cada `*_cmd.rs` expõe `fn run(args: &[String]) -> Result<ExitCode,
      CliError>`.
- [ ] **R11.4** — `main.rs` reduz a: init tracing → parse top-level → dispatch →
      match error → `process::exit(code)`. **Único** `process::exit` do crate fica
      aqui.
- [ ] **R11.5** — Define `CliError` enum tipado (hand-rolled, sem thiserror):
      `InvalidArgs(String)`, `SubcommandFailed(any error)`, `Io(std::io::Error)`.
- [ ] **R11.6** — `tests/cli_smoke.rs` testa cada subcomando via `assert_cmd` e
      verifica exit code + stderr shape (não conteúdo criptográfico).

**DoD Fase 2**: zero `process::exit` em `crates/*/src/` (exceto 1 no `main.rs`
topo), zero `Result<_, String>` em código novo, `main.rs < 200` linhas, cada
`*_cmd.rs < 500` linhas.

---

### Fase 3 — Observabilidade (R3)

**Meta**: `tracing` estruturado em todo caminho crítico, JSON subscriber default
para consumo por agentes.
**Risco**: baixo (additivo).
**Duração estimada**: 5-8 commits.

#### R3.1 — Deps e init

- [ ] Adicionar `tracing`, `tracing-subscriber` ao workspace deps.
- [ ] `main.rs` init subscriber com `EnvFilter` e JSON formatter default.
- [ ] Flag `--log-format human|json` (default json para agentes).

#### R3.2 — Spans em caminhos críticos

Em ordem de valor:

- [ ] `forge-core-store::claim_wal` (append, rotate, replay) — span por operação
      com `tx_id`, `claim_id`.
- [ ] `forge-core-runtime::execute_operation` — span com `operation_id`,
      `effect_count`.
- [ ] `forge-core-crypto::run_host_adapter_*_verification` — span com
      `verification_kind`, `subject_ref`, `result`.
- [ ] `forge-core-validate::run_validate` — span com `root`, `diagnostic_count`.
- [ ] `forge-core-cli::run_execute_operation` — span com `root`, `payload_count`.

#### R3.3 — Correlação multi-agente

- [ ] Cada agent session recebe um `agent_id` (de claim ou CLI arg).
- [ ] Spans carregam `agent_id` como field.
- [ ] JSON log permite filtrar `agent_id=X` para ver só o que um agente fez.

#### R3.4 — Remover `eprintln!` de lib code

- [ ] grep `eprintln!` em `crates/*/src/`, migrar para `tracing::warn!`/`error!`.
- [ ] `println!` em lib code só onde é o contrato de output (JSON para stdout).

**DoD Fase 3**: logs estruturados JSON em todos os caminhos críticos, zero
`eprintln!` em `crates/*/src/` (exceto main.rs fallback sem subscriber).

---

### Fase 4 — Evidência de qualidade (R6 + R4)

**Meta**: benchmarks para hot paths, fuzz harness para parsers.
**Risco**: muito baixo (additivo, não toca em código de produção).
**Duração estimada**: 4-6 commits.

#### R6 — `criterion` benchmarks

- [ ] **R6.1** — Adicionar `criterion` ao workspace. Criar
      `crates/forge-core-store/benches/claim_wal.rs`.
- [ ] **R6.2** — Bench: WAL append (1, 100, 1000 entries), WAL replay, CRC verify.
- [ ] **R6.3** — Bench: `build_reference_index` em repo de tamanho variado.
- [ ] **R6.4** — Bench: `serde_yaml::from_str` vs `serde_yml::from_str` (após R7)
      de contract documento.
- [ ] **R6.5** — Bench: `verify_rekor_checkpoint`, `verify_merkle_inclusion`.
- [ ] **R6.6** — CI roda bench em PR com label `perf` e compara com `main`.

#### R4 — `cargo-fuzz`

- [ ] **R4.1** — Criar `fuzz/` diretório no workspace (cargo-fuzz exige isso).
- [ ] **R4.2** — Target: `parse_rekor_log_entry` (parse de JSON adversarial).
- [ ] **R4.3** — Target: `parse_signed_checkpoint` (decode de base64 adversarial).
- [ ] **R4.4** — Target: `claim_wal_decode` (NDJSON adversarial).
- [ ] **R4.5** — Target: `ocsp_response_decode` (DER adversarial).
- [ ] **R4.6** — Documentar execução em `docs/dev-docs/.../fuzzing.md` com
      comando `cargo fuzz run <target> -- -max_total_time=60`.

**DoD Fase 4**: `cargo bench` roda sem erro, `cargo fuzz run` em cada target por
≥1 min sem panic.

---

### Fase 5 — Supply chain e segurança (R7 + R5)

**Meta**: `serde_yaml` removido, material cripto zeroizado.
**Risco**: R7 médio (API diff), R5 baixo.
**Duração estimada**: 4-6 commits.

#### R7 — `serde_yaml` → `serde_yml`

- [ ] **R7.1** — Inventariar todos os usos (`grep -r "serde_yaml"` em crates/).
- [ ] **R7.2** — Trocar dep no workspace `Cargo.toml`. `serde_yml` é fork ativo
      API-compatível na maioria dos casos.
- [ ] **R7.3** — Migrar imports `serde_yaml::` → `serde_yml::`.
- [ ] **R7.4** — Rodar fuzz (R4) e bench (R6) para validar equivalência.
- [ ] **R7.5** — Remover `serde_yaml` do workspace.

#### R5 — `zeroize`

- [ ] **R5.1** — Inventariar material cripto: chaves públicas decodificadas
      (`VerifyingKey`, `ed25519_dalek::VerifyingKey`), assinaturas brutas, nonces
      OCSP, conteúdo de payload antes do hash.
- [ ] **R5.2** — Adicionar `zeroize` (1.x) ao workspace.
- [ ] **R5.3** — Wrap em `Zeroizing<Vec<u8>>` onde aplicável. Para tipos de crate
      externo (ed25519, p256), usar `Zeroizing<Box<[u8]>>` pra bytes intermediários.
- [ ] **R5.4** — Comparações manuais de hash/nonce em constant-time
      (`subtle::ConstantTimeEq` se já não estiver).
- [ ] **R5.5** — Fuzz (R4) re-rodado para confirmar zero panics após wraps.

**DoD Fase 5**: `cargo tree | grep serde_yaml` vazio, zero `Vec<u8>` com material
cripto sem `Zeroizing<>`.

---

### Fase 6 — Documentação e rastreabilidade (R13 + R9)

**Meta**: docs alinhadas com `AGENTS.md`, papers rastreáveis, Bootstrap Exception
removido.
**Risco**: baixo.
**Duração estimada**: 3-5 commits.

#### R13 — Alinhar docs com realidade

- [ ] **R13.1** — `04_rust_refactor_guide.md`: remover menções a `thiserror` e
      `clap` derive (proibidos por `AGENTS.md`). Substituir por "roll error enums
      by hand, derive `Debug, Clone, PartialEq, Eq`".
- [ ] **R13.2** — Auditar todos os dev-docs por recomendações que contrariam
      `AGENTS.md`.
- [ ] **R13.3** — Para cada paper em `contracts/research/`, criar entrada em
      `docs/dev-docs/.../paper_implementation_status.md`:
      ```
      | Paper | Status | Onde no código | Próximo passo |
      |---|---|---|---|
      | selfhealing-wal-crc-design-v1 | ✅ implementado | claim_wal.rs L400-500 | — |
      | AutoCodeRover | 🟡 parcial | — | Fuzz targets (R4) |
      | rust-observability-selfhealing | 🔴 não iniciado | — | R3 tracing |
      ```
- [ ] **R13.4** — `README.md`: revisitar "best practices and scientific papers"
      claim. Adicionar seção "Evidence" linkando para
      `paper_implementation_status.md`.

#### R9 — Fechar Bootstrap Core Exception

- [ ] **R9.1** — Inventariar uso de `--allow-bootstrap-core` em testes e scripts.
- [ ] **R9.2** — Configurar sidecar real para o repo do Forge (`D:/Forge-method-core`
      aponta pra sidecar separado).
- [ ] **R9.3** — Migrar testes que usam `--allow-bootstrap-core` para resolver
      sidecar real.
- [ ] **R9.4** — Remover flag de production code paths.
- [ ] **R9.5** — Atualizar `CONTEXT.md` "Bootstrap Gaps" → mark as resolved.

**DoD Fase 6**: dev-docs 100% alinhadas com `AGENTS.md`, every paper has status,
`--allow-bootstrap-core` removido de production paths.

---

## Ordem de execução consolidada

```
Fase 0  ── R1 estendido + R12     (decomposição estrutural)
            │
            ▼
Fase 1  ── R10                    (criar forge-core-crypto)
            │
            ▼
Fase 2  ── R2 + R8 + R11          (disciplina de erro)
            │
            ▼
Fase 3  ── R3                     (tracing)
            │
            ▼
Fase 4  ── R6 + R4                (bench + fuzz)
            │
            ▼
Fase 5  ── R7 + R5                (deps + zeroize)
            │
            ▼
Fase 6  ── R13 + R9               (docs + bootstrap)
```

**Rationale da ordem**:
1. Fase 0 primeiro: decompõe god-file para que as fases seguintes apliquem
   mudanças em módulos pequenos, não num monólito.
2. Fase 1 (R10) depois de Fase 0: move cripto para seu crate **antes** de
   adicionar tracing/fuzz — caso contrário, instrumentação fica na CLI e tem que
   migrar de novo.
3. Fase 2 antes de Fase 3: remover `process::exit` permite que tracing captures
   erros propagados, em vez de silenciados por exit.
4. Fase 3 antes de Fase 4: tracing permite que benchmarks tenham spans;
   fuzzing beneficia de error types tipados (Fase 2).
5. Fase 5 independente, mas depois de Fase 0 pra reduzir churn.
6. Fase 6 por último: docs refletem realidade final, não intermediária.

---

## Estimativa total

| Fase | Faixas | Commits | Sessões (~2h) |
|---|---|---|---|
| 0 | R1 estendido + R12 | 8-12 | 4-6 |
| 1 | R10 | 6-10 | 3-5 |
| 2 | R2 + R8 + R11 | 8-12 | 4-6 |
| 3 | R3 | 5-8 | 2-4 |
| 4 | R6 + R4 | 4-6 | 2-3 |
| 5 | R7 + R5 | 4-6 | 2-3 |
| 6 | R13 + R9 | 3-5 | 1-2 |
| **Total** | R1-R13 | **38-59** | **18-29** |

**Trade-off**: dá pra paralelizar Fase 4 (bench/fuzz) e Fase 5 (deps/zeroize)
com Fase 2-3, mas **não** dá pra paralelizar nada com Fase 0 ou Fase 1.

---

## Tracking

Cada faixa (R1-R13) terá arquivo de progresso em
`docs/dev-docs/forge-method-core-dev-docs-v2/progress/`. Convenção:

- `r1_lib_inventory.md` (existe)
- `r10_crypto_crate.md`
- `r11_main_rs_decomposition.md`
- `r12_test_decoupling.md`
- etc.

Status de cada sub-tarefa marcado em linha com commits. Quando uma fase termina,
atualizar este doc com data e link para commits.

---

## Riscos e mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|---|---|---|---|
| R1.CryptoOCSP pisa em WIP Codex | Média | Alto | Confirmar `37aa52d` estável; esperar Codex confirmar antes |
| R10 quebra callers externos | Baixa | Alto | Re-exports preservam API; smoke test de CLI output |
| R11 muda exit codes | Média | Médio | Snapshot de exit code antes/depois; documentar mudanças |
| R7 `serde_yml` drop-in falha | Baixa | Médio | Fazer em branch separada; fuzz valida equivalência |
| Fuzz encontra panic | Alta | Médio | **Esperado** — é o objetivo. Documentar como bug separado |
| Scope creep em R13 | Alta | Baixo | Limitar a 1 sessão; papers sem código viram issue, não trabalho |

---

## Não-escopo (explicitamente fora)

- Rewriting `forge-core-store` em DB real (SQLite/LMDB) — não agora.
- Async runtime em todo lugar — `tokio` só onde já está (reconcile loop).
- GUI/observability dashboard — Forge é CLI/library only.
- Multi-tenancy no sidecar — um sidecar por consumer repo, por design.
- Substituir `ed25519-dalek`/`p256` por `RustCrypto` unified — sem benefício claro.
