# F13 — Budget/Cost Accounting (FECHADO)

**Data**: 2026-06-30
**Branch**: `codex/forge-frust-052-ocsp-boundary`

## Objetivo

Agregar custo (model calls, tool calls, estimated tokens) sobre TraceEvents,
permitindo que um humano ou agente pergunte "quanto custou esta run / graph /
agent / principal" via CLI `forge-core cost`.

## Entregues

### Aggregator puro em `forge-core-trace`

- `CostScope` enum: `Run`, `Graph`, `Principal`, `All` (snake_case no wire).
- `CostTotals`: `model_calls`, `tool_calls`, `estimated_tokens`, `event_count`.
- `CostBreakdownEntry`: `{ key, totals }`.
- `CostReport`: `{ schema_version: "cost-report-v0", scope, scope_id, totals, by_run, by_agent }`.
- `aggregate_costs(events, scope, scope_id) -> CostReport`: pura, sem IO.
  Breakdowns ordenados por `estimated_tokens` descendente (heaviest first).
- 3 unit tests: soma de totals, breakdown por run/agent, slice vazio.

### CLI `forge-core cost`

- Módulo `crates/forge-core-cli/src/cost_cmd.rs`.
- Flags: `--root`, `--run-id`, `--graph-id`, `--principal`, `--last-run`,
  `--allow-bootstrap-core`, `--json|--no-json`.
- Resolve state_root via `resolve_project` (mesmo padrão do `explain`).
- Query via `query_trace_events` (run-scoped); `--graph-id`/`--principal`
  aplicados como post-filters (a query do store é run-scoped).
- Output: `CliEnvelope<CostReport>` em JSON, ou texto humano compacto.
- Registrado em `command_registry::COMMANDS`.

### E2E

- `crates/forge-core-cli/tests/cost_e2e.rs` — 3 casos:
  1. `cost_aggregates_all_events_when_no_scope_given`: 3 eventos, valida
     totals (model=20, tool=8, tokens=3500) + by_run ordenado.
  2. `cost_scopes_to_a_single_run`: `--run-id` filtra a uma run.
  3. `cost_reports_empty_when_trace_log_absent`: projeto novo sem trace log
     retorna report zero (não crasha).

## Decisões de design

1. **Aggregator é puro** (em `forge-core-trace`, não no CLI): unit-testável
   sem filesystem, reusável por callers futuros (e.g. MCP adapter F08).
   Deletion test: se sumir, o CLI precisa re-derivar a lógica de agregação —
   ganha sua vida.

2. **Sem breakdown por `model`/`tool_class`**: o `TraceCost` hoje carrega
   apenas counts agregados por evento, não por model/tool. Adicionar esses
   dimensions exigiria ampliar o schema de TraceCost (escopo maior). O
   handoff mencionava essa dimensão mas ela não é suportada pelos dados
   atuais — deferido até o schema crescer. Por run/agent já cobre o caso de
   uso principal (atribuição de custo).

3. **Post-filter para graph/principal**: a query do store é run-scoped, então
   `--graph-id`/`--principal` filtram client-side sobre os events retornados.
   Simples e correto; se volume de eventos virar problema, uma query com
   filtro server-side pode ser adicionada depois.

## Validação

- `cargo check -p forge-core-trace -p forge-core-cli` ✅
- `cargo clippy ... -- -W clippy::pedantic` ✅ 0 warnings no trabalho novo
- `cargo test -p forge-core-trace --lib` ✅ 5/5 (incl. 3 do aggregator)
- `cargo test -p forge-core-cli --test cost_e2e` ✅ 3/3
- Anchor 122 ✅

## Papers / provenance

Atribuição de custo por entidade (run/agent/principal) alinha com
Microservices Saga (tracking de custos por transação distribuída) e
SLSA provenance (rastreabilidade de quem fez o quê).
