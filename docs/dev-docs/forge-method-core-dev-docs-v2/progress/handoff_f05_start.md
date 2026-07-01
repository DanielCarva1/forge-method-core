# Handoff — F05 Eval Compare Harness (start point)

**Data**: 2026-07-01
**Sessão**: interrompida por crashes recorrentes do Zed (rust-analyzer
indexando `target/`, mesmo com `rust-analyzer.toml` já excluindo — problema
ambiental, não do código). Nenhuma edição feita na codebase nesta sessão:
apenas exploração de estado e leitura de contexto.

---

## Estado global do projeto

### Branch e remotes

- **Branch atual**: `master` (working tree **limpa**, anchor 122 preservada)
- **Últimos 3 commits**:
  - `060a5a96` R-SCM: add sigstore signing + CycloneDX SBOM to release pipeline
  - `e1439c6b` R-LINT.6: flip CI clippy from -W to -D (deny warnings)
  - `f95cd54e` R-LINT.2: clear all 41 pedantic lints, document audit
- **Remotes** (ambos sincronizados em `master`):
  - `origin` → `https://github.com/Stable-Studio/forge-method-rust.git` (branch `master`)
  - `personal` → `https://github.com/DanielCarva1/forge-method-core.git` (branches `master` e `main`)
- **Push pattern autorizado**:
  ```
  git push origin master && \
  git push personal master:master && \
  git push personal master:main --force
  ```

### Scores de excelência (9 de 11 frentes em 10/10)

| Frente | Hoje | Meta |
|---|---|---|
| Rápido | 9 | 10 (R-FAST, **último a fazer**) |
| Robusto | 10 | ✅ |
| Performativo | 10 | ✅ |
| Protocolo guia | 10 | ✅ |
| Workflows | 10 | ✅ |
| Agente-guia / Não-novela / Docs | 10 | ✅ |
| Rust best practices | 10 | ✅ (CI agora `-D clippy::pedantic`) |
| Segurança supply chain | 10 | ✅ (sigstore + SBOM no release.yml) |
| **Features comunidade** | **9.7** | **10 (F05/F06/F07/F08/F12/F14)** |

---

## Próxima tarefa: F05 — Eval Compare single-agent baseline (harness)

Próxima feature na trilha. Documentada em
`docs/dev-docs/forge-method-core-dev-docs-v2/progress/followups_v0_1_to_10.md`
linhas 111-150. Ordem: F05 → F06 → F07 → F08 → F12 → F14 → R-FAST.

### O que JÁ EXISTE (não recriar)

- **`crates/forge-core-eval/src/lib.rs`** (934 linhas): lib de **comparação**
  madura. Não executa nada — apenas compara runs pré-computados.
  - `compare_eval_runs(suite, baseline_label, candidate_label, baseline_runs, candidate_runs) -> EvalComparisonReport` (linha 251)
  - `compare_eval_runs_with_diagnostics(...)` (linha 271) — igual + diagnostics extras
  - Tipos prontos: `EvalArmLabel { SingleAgent, Graph, Mas, Manual }`,
    `EvalCompareSuite`, `EvalArmSpec { label, run_refs }`,
    `EvalComparePolicy`, `EvalComparisonReport`, `EvalArmSummary`,
    `EvalMetricDeltas`, `EvalDiagnostic`, `EvalDiagnosticCode` (14 códigos).
  - `EVAL_COMPARE_SCHEMA_VERSION = "0.1"`

- **`crates/forge-core-contracts/src/eval_run.rs`** (297 linhas): contrato
  que cada run produz. Estrutura central:
  ```
  EvalRunContractDocument {
    schema_version,
    eval_run_contract: EvalRunContract {
      run_id, task_id, model_ref, router_decision: Option<_>,
      outcome: EvalOutcome { value: EvalVerdict, failure_cluster, ... },
      cost: EvalCost { prompt_tokens, completion_tokens, total_tokens,
                       estimated_cost_usd_micros, wall_time_ms,
                       num_tool_calls, num_turns },
      quality_signals: QualitySignals,
      evidence_refs: Vec<String>,
    }
  }
  ```
  `EvalVerdict`: Passed/Failed/Partial/Flaky/Error
  `EvalFailureCluster`: BuildFailure/TestFailure/LintFailure/Timeout/
                        WrongLocation/OverfitPatch/SemanticMismatch/ToolError/None

### O que FALTA criar (harness executor)

O `compare_eval_runs` recebe `EvalRunInput { source_ref, document }` pronto.
**Quem produz esses documentos é o harness novo.** Ele precisa:

1. **Executar cada arm** (subprocess por arm — decisão recomendada no
   followup, isolamento + mesma CLI que produção)
2. **Padronizar args** entre arms (mesmo loader, tools, output contract = controle)
3. **Medir** por run: accuracy (verdict), cost (tokens/custo/tempo/tool calls),
   latency (wall_time_ms), trajectory length (num_turns, num_tool_calls),
   failures (failure_cluster)
4. **Produzir `EvalRunContractDocument`** por run em disco
5. **Alimentar `compare_eval_runs`** e gerar report
6. **Output JSON** (agent-facing) + humano (CLI)

### Stories planejados (F05.1 a F05.7)

| Story | O que | Status |
|---|---|---|
| **F05.1** | Design do harness (grill + improve) | **← começar aqui** |
| F05.2 | `EvalHarnessConfig` YAML schema + validator tipado | pendente |
| F05.3 | Arm executor (subprocess) | pendente |
| F05.4 | Report generator (reusa `compare_eval_runs`) | pendente |
| F05.5 | CLI `forge-core eval-compare --config <yaml>` (F15 pattern, 2 edit points) | pendente |
| F05.6 | Trace integration (`EvalCompareStarted/Passed/Failed`) | pendente |
| F05.7 | Fixtures válida + inválida + E2E + anchor 122 | pendente |

### Perguntas de design ainda abertas (resolver no F05.1)

- **In-process vs subprocess?** Recomendação: subprocess (isolamento, mesma
  CLI que produção). Justificativa: papers SWE-agent/OpenDev/CoAgent citados.
- **Onde o harness vive?** Provável: ampliar `forge-core-eval` com módulo
  `harness` (não precisa de crate nova, a menos que grill diga o contrário).
- **Termos novos pro glossário**: "EvalArm", "EvalHarness", "EvalRunner"
  não estão em `CONTEXT.md`. Adicionar via `grill-with-docs` quando definidos.
- **ADR?** Só se hard-to-reverse + surprising + real-tradeoff. Provável que
  não precise — subprocess é decisão pragmaticamente correta.

---

## Contexto — CONTEXT.md atual

Ler antes de iniciar grill. Hoje o glossário cobre:
Consumer Project Repo, Forge Runtime Sidecar, Forge Project Link,
Project Init Bootstrap, Bootstrap Core Exception, Risk Audit,
Anti-pattern (AI Code), Project Link Hardening Rules, Remaining Bootstrap Gaps.

**Não há termos de "Eval" ainda.** Quando grill definir EvalArm/EvalHarness/
EvalRunner, adicionar no formato existente (definição em prosa, sem detalhe
de implementação).

---

## Hard constraints (NÃO violar)

### Ambiente

- **Shell é MSYS/Git Bash** (`OSTYPE=msys`), **NÃO WSL**. `cargo` → `cargo.exe`.
  Paths Windows (`D:\...`) ou relativos.
- **`bash: warning: setlocale: LC_ALL`** aparece em todo comando — inofensivo, ignorar.
- **NÃO usar subagentes** — crasharam o Zed repetidamente (memory allocation
  8MB, 0xC0000409). Trabalhar **sequencialmente**.
- **Leituras ≤80 linhas por `read_file`** pra evitar context bloat.
- **Outputs**: sempre pipear via `tail`/`head`/`grep`.
- **Nunca rodar 2 cargos em paralelo** (contention com rust-analyzer).
- **`rust-analyzer.toml`** exclui `target/` (commit `7867adc`) — **não remover**.
- **Anchor 122**: após cada mudança, rodar
  `cargo run -q -p forge-core-cli -- validate --root . --json`
  e confirmar `diagnostics: []`.
- **Limpeza periódica de tempdirs vazados**:
  ```
  ls target | grep -E -- '-[0-9]+$' | (cd target && xargs -r rm -rf)
  ```

### Convenções de código (AGENTS.md, sobrepõe defaults)

- **SEM `anyhow`/`thiserror`** — enums de erro escritos à mão, derivando
  `Debug, Clone, PartialEq, Eq`.
- **SEM novos `Result<_, String>`** — legados em parsers não devem propagar.
- **SEM `clap`/macros derive** — parsing de argv manual.
- **Validação é acumulativa** via `ValidationReport`, nunca short-circuit.
- **`crates/forge-core-crypto/src/ocsp.rs` e `host_adapter_verification.rs`
  quebram sob `clippy --fix`** — editar manualmente.
- **Padrão F15**: comandos novos precisam de exatamente 2 edit points
  (criar módulo + registrar em `command_registry::COMMANDS`).

### Skills a aplicar (instrução explícita do Daniel)

- **`improve-codebase-architecture`**: deletion test, vocabulário depth/shallowness.
  Aplicar **antes** de desenhar cada feature.
- **`grill-with-docs`**: stress-test designs contra `CONTEXT.md`/ADRs.
  CONTEXT.md é glossário puro (zero detalhe de implementação).
  ADR gate = (hard-to-reverse + surprising + real-tradeoff), os 3.

---

## Crates existentes (13)

`forge-contract-validator`, `forge-core-cli`, `forge-core-contracts`,
`forge-core-crypto`, `forge-core-engine`, `forge-core-eval` (934 linhas,
lib de comparação pronta, **precisa de harness**), `forge-core-graph`,
`forge-core-runtime`, `forge-core-schema`, `forge-core-store`,
`forge-core-trace`, `forge-core-validate`.

**Crates que ainda precisam ser criadas** (features futuras):
- `forge-core-memory` (F06)
- `forge-core-protocol-mcp` (F08)

---

## CI layout

- `.github/workflows/ci.yml`: push em master → `cargo check`,
  `clippy -D clippy::pedantic`, `test`, `fmt --check`, anchor 122.
- `.github/workflows/release.yml`: tag `v*` → cross-compile 5 targets +
  cosign sign + SBOM CycloneDX (novo na sessão R-SCM).
- `.github/workflows/perf.yml`: cron diário + label `perf`.

---

## Pitfalls — coisas que NÃO funcionaram em sessões anteriores

1. **Lint names importam**: `clippy::struct_field_same_postfix` e
   `clippy::naive_byte_count` **NÃO existem** em clippy 1.94. Corretos:
   `struct_field_names`, `naive_bytecount`. Confirmar via
   `cargo clippy ... 2>&1 | grep "help:.*clippy"`.

2. **`//![allow(...)]` ≠ `#![allow(...)]`**: o primeiro é doc-comment
   (ignorado), o segundo é inner attribute. Usar `#!` no início dos testes.

3. **`PathBuf` não implementa `Display`**: usar `path.display()` ou
   `{} with .display()`. `{path}` direto não compila.

4. **Refatorar > `#[allow]`** em código de produção: introduzir um
   parameter struct é melhor que silenciar `too_many_arguments`.

5. **`edit_file` matching múltiplas localizações**: falha. Fornecer contexto
   mais único no `old_text`.

6. **`edit_file` com old_text/new_text idênticos**: reporta
   "No edits were made". Re-ler o conteúdo real do arquivo primeiro.

7. **Subagentes crasharam o Zed** — memory allocation failures
   (8MB, 0xC0000409). Trabalhar **sequencialmente**, sem `agent-orchestrator`.

8. **`target/` acumula** — `target/debug` chegou a 134k arquivos, OOMa o
   editor. Limpeza periódica:
   `ls target | grep -E -- '-[0-9]+$' | (cd target && xargs -r rm -rf)`.

9. **`cargo clippy --fix` em crypto crates** — quebra `ocsp.rs` e
   `host_adapter_verification.rs`. Excluir crypto do `--fix`.

10. **`macos-13` (Intel) runner** — trava 30+ min pra `x86_64-apple-darwin`.
    Usar `macos-14` cross-compile.

11. **`sha256sum` em macOS runners** — não existe. macOS tem `shasum -a 256`.
    Detectar portável.

12. **`cd ../..` POSIX do `target/<target>/release`** — cai em `target/`,
    não na raiz. Usar `working-directory:` directive.

13. **`git push personal master:main` sem `--force`** — falha non-fast-forward.
    Force necessário (Daniel autorizou).

14. **Dois blocos `permissions:` em release.yml**: job-level sobrepõe
    workflow-level. Build job herda workflow (ganha `id-token: write`),
    release job sobrepõe (só `contents: write`).

15. **Daniel prefere PT-BR, conciso, sem over-narration** — match a diretividade
    dele. Não perguntar "should I proceed?" em decisões reversíveis delegadas.
    **PAUSAR** em não-reversíveis (licença, branch strategy).

---

## Estilo de trabalho que funcionou bem

- **Por épico**: ler o épico no followups, fazer grill+improve, executar
  stories sequencialmente, validar anchor 122 + `cargo test --workspace`
  1x por épico (não por story), commitar no fim do épico.
- **Green-loop desligado durante sessões ativas**, rodar gates manualmente
  no fim de cada épico.
- **Ler primeiro, editar depois**: nunca editar arquivo sem antes ler as
  linhas relevantes (≤80 por vez).
- **Small commits**: 1 épico = 1 commit com mensagem descritiva.

---

## Sugestão de primeira ação no novo editor

1. Confirmar estado: `git status --short && git branch --show-current`
2. Confirmar anchor: `cargo run -q -p forge-core-cli -- validate --root . --json | tail -3`
   (deve mostrar `"diagnostics": []`)
3. Aplicar `improve-codebase-architecture` em `crates/forge-core-eval`:
   ele é a candidata principal ao "deepening". O harness provavelmente
   vira um módulo novo dentro dele (`mod harness`) ou nova crate — decidir
   via deletion test: se o harness for shallow (só orquestra subprocess),
   fica como módulo; se tiver política/runner/metrics aggregator reutilizável,
   vira crate.
4. `grill-with-docs` para sharpenar termos: EvalArm, EvalHarness, EvalRunner.
   Adicionar ao `CONTEXT.md` quando estiverem resolvidos.
5. Implementar F05.2 → F05.7 sequencialmente.
6. Commit no fim de F05 completo + push para os 2 remotes.

---

## One-line summary for next agent

> 9 de 11 frentes em 10/10 (R-LINT + R-SCM completos, CI agora `-D`,
> sigstore+SBOM no release). Próximo: **F05 Eval Compare harness** — a lib
> `forge-core-eval` tem `compare_eval_runs` pronta (compara runs
> pré-computados), falta o **executor** que roda subprocess por arm, mede
> accuracy/cost/latency/trajectory, e produz `EvalRunContractDocument` por
> run. Aplicar `improve-codebase-architecture` + `grill-with-docs` primeiro
> (instrução permanente do Daniel). Trabalhar **sequencialmente (sem
> subagentes)**, PT-BR com Daniel, respeitar AGENTS.md (no anyhow/thiserror,
> erros manuais, validação acumulativa, F15 pattern para comandos novos).
> Anchor 122 deve continuar com `diagnostics: []`.
