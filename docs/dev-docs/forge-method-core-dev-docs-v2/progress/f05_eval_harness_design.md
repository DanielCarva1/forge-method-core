# F05 — Eval Compare single-agent baseline (harness)

**Data**: 2026-07-01
**Branch**: `master`
**Status**: design (F05.1) — grill resolvido, pronto para F05.2

## Objetivo

Executar arms (single-agent, multi-agent, graph, manual) contra um mesmo
corpus de tasks sob um answer contract uniforme, medir accuracy/cost/latency/
trajectory, e produzir `EvalRunContractDocument` canônicos que alimentam o
`compare_eval_runs` já existente em `forge-core-eval`. Realiza o ADR-0002:
toda arquitetura multi-agent precisa ser comparada contra single-agent anchor
com mesmo loader, tools, answer contract e usage accounting.

## ADrs governantes (não re-litigar)

- **ADR-0002** (single-agent baseline antes de MAS): define que o single-agent
  é o anchor canônico e que a comparação precisa das mesmas variáveis
  controladas em todos os arms.
- **ADR-0004** (trace e eval como parte do produto): todo run relevante gera
  `TraceEvent`. F05.6 (trace integration) é mandatório, não opcional.

## Decisões de design (resolvidas em grill)

1. **O executor vive numa crate nova `forge-core-eval-harness`** (Opção B do
   deletion test). `forge-core-eval` permanece **pura** (comparação
   determinística, `#[must_use]`, testável sem IO); o harness é **impuro**
   (subprocess + FS + timing) e mora atrás de uma seam própria.
   Deletion test: se o harness sumir, callers precisam re-derivar spawn +
   grader + canonicalização — concentra complexidade, ganha sua vida.
   Profundidade: interface pequena (`run(config) -> Vec<EvalRunContractDocument>`),
   implementação complexa (spawn isolado por arm, grader por tipo de task,
   recuperação de erro de subprocess).

2. **O arm (subprocess) só produz raw `{output, usage}` num path acordado.**
   O harness é o **único** que monta e emite o `EvalRunContractDocument`
   canônico. Isso realiza o "mesmo answer contract entre arms" do ADR-0002:
   arms externos (Claude Code, Codex, qualquer CLI) não precisam saber do
   schema Forge — só produzem output, o harness traduz. Arms não se
   auto-avaliam (auto-avaliação seria variável não-controlada, violando o
   controle que é o ponto do ADR).

3. **O verdict é computado por um grader no harness**, não declarado à parte.
   O grader é **inferido da estrutura do corpus**, não é campo novo no config:
   - `router-eval-corpus.yaml` (cases `utterance -> expected_workflow`) =>
     `ExactMatch`.
   - `coordination-eval-suite.yaml` (dimensions com `fixture_refs` +
     `metric_kind: fixture_pass`) => `FixturePass`.
   - Sem grader automático => `Manual` (humano revisa; contrato sai com
     `EvalVerdict::Error` até revisão).
   - `LlmJudge` é possível no futuro mas fora de escopo de F05.

4. **1 run por (arm, task).** O schema existente trata `DuplicateTaskRun`
   como erro e a policy tem `minimum_task_count`. Variância estatística vem
   da diversidade de tasks no corpus (router já tem ~50 cases), não de
   repetições do mesmo (arm, task). Conforme ao modelo existente — não
   inventar repetição.

5. **Limitação honesta (measurement gap).** Tokens/turns/tool_calls são
   **self-report do arm** (o harness não enxerga dentro da API call do
   modelo). `wall_time_ms` é medido externamente pelo harness. O self-report
   vira `measurement_gap` documentado no report — única opção possível, não
   controlável sem instrumentar o arm.

## Divisão de responsabilidades

| Responsabilidade | Dono |
|---|---|
| Rodar a task, produzir output bruto + usage (tokens/turns/tool_calls) | Arm (subprocess) |
| Medir `wall_time_ms` (tempo externo do spawn) | Harness |
| Computar verdict via grader (ExactMatch / FixturePass / Manual) | Harness |
| Montar e emitir `EvalRunContractDocument` canônico | Harness (único produtor) |
| Comparar arms e emitir recomendação | `compare_eval_runs` (já existe) |

## Termos novos (adicionados ao CONTEXT.md)

- **EvalArm** — condição experimental rotulada que roda o mesmo corpus de
  tasks. Hoje: single-agent, graph, mas, manual. Cada arm é uma variável
  no experimento de comparação.
- **EvalHarness** — executor que roda arms sob controle, coleta o output
  raw de cada um, aplica o grader uniforme e canonicaliza os
  `EvalRunContractDocument` que alimentam a comparação.

`EvalRunner` foi descartado — não ganha lugar no glossário, seria redundante
com EvalHarness.

## Stories (roadmap)

| Story | Entrega |
|---|---|
| F05.1 | Este design doc + termos no CONTEXT.md |
| F05.2 | `EvalHarnessConfig` YAML schema + validator tipado (seguir `forge-core-validate`) |
| F05.3 | Arm executor: spawn subprocess por arm, grader, canonicalização |
| F05.4 | Report generator (reusa `compare_eval_runs`) |
| F05.5 | CLI `forge-core eval-compare --config <yaml>` (F15 pattern, 2 edit points) |
| F05.6 | Trace integration (`EvalCompareStarted/Passed/Failed`) |
| F05.7 | Fixtures válida + inválida + E2E + anchor 122 |

## Validação (a cumprir ao fim de F05)

- `cargo check --workspace` ✅
- `cargo clippy --workspace --all-targets -- -W clippy::pedantic` ✅ 0 warnings
- `cargo test --workspace` ✅ (1x no fim do épico)
- Anchor 122: `diagnostics: []`

## Papers / provenance

ADR-0002 e ADR-0004 governam esta feature. Subprocess-per-arm com mesmo
answer contract alinha com SWE-agent / OpenDev / CoAgent (harness
engineering) e com o controle experimental de tau-bench (mesmo loader,
tools, contract em todos os arms).
