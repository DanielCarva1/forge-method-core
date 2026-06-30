# Forge Method Core v2 - plano de implementacao

## Principio de sequenciamento

A ordem abaixo evita construir features de alto nivel em cima de um runtime sem trace, sem preview e sem readiness gate. Primeiro estabiliza o kernel observavel. Depois vem graph, eval, memory, protocolos e governance.

## Milestone 0 - higiene e ergonomia Rust

Objetivo: reduzir custo de alteracao para agentes e maintainers.

Escopo:

- **Manter CLI com argv manual em `main.rs`** (sem `clap`, sem derive macros). Decisão projetual em `AGENTS.md` — ver `04_rust_refactor_guide.md` para o padrão estabelecido. (Original: "Migrar CLI para `clap` derive" — revisado em R13.2.)
- Separar `forge-core-store/src/lib.rs` em modulos: `paths`, `jsonl`, `reference_index`, `effect_apply`, `effect_wal`, `effect_recovery`, `effect_metadata`, `locks`.
- **Roller error enums à mão** (sem `thiserror`, sem `anyhow`), derivando `Debug, Clone, PartialEq, Eq`. (Original: "Introduzir `thiserror`" — revisado em R13.2.)
- Introduzir `tracing` spans nos caminhos de runtime, validation, store e CLI.
- Criar builders de fixtures para `OperationContract`, `ToolEffectContract`, `CommandContract` e `RuntimePlan`.
- Adicionar snapshot tests para outputs JSON estaveis.
- Definir regra: cada subcomando novo adiciona um braço no `match` de `main.rs` e uma fn `run_<command>(&[String])` em `lib.rs`.

Entregas:

- ADR-0001 aceito.
- `forge-core-cli` mantém argv manual (sem Parser/Subcommand de `clap`).
- Modulos separados em store.
- Clippy e fmt no CI.

## Milestone 1 - preview, ready e trace

Objetivo: todo run deve ser previsivel, verificavel e explicavel.

Escopo:

- Criar crate `forge-core-trace`.
- Definir `TraceEvent` v0.
- Adicionar `forge preview` baseado no runtime planner atual.
- Adicionar `forge ready` como agregador de gates.
- Adicionar `forge explain --last-run` para explicacao humana curta.
- Ligar command evidence e effect metadata ao trace_id.

Entregas:

- `schemas/trace_event_v0.yaml` implementado.
- Snapshot tests de preview.
- E2E fixture: operation mutavel com gate pendente, operation pronta, operation bloqueada.

## Milestone 2 - WorkflowGraph v0

Objetivo: parar de depender de routing solto por prompt para workflows compostos.

Escopo:

- Criar crate `forge-core-graph`.
- Definir `WorkflowGraph`, `GraphNode`, `GraphEdge`, `GraphBudget`, `GraphStopCondition`.
- Implementar `forge graph validate`.
- Implementar `forge graph run --dry-run`.
- Integrar `OperationContract` como node type.
- Adicionar verifier node e replan boundary simples.

Entregas:

- `schemas/workflow_graph_v0.yaml` implementado.
- Graph fixture com parallel read-only branches.
- Graph fixture com verifier bloqueando mutation.

## Milestone 3 - eval baseline

Objetivo: transformar arquitetura em decisao mensuravel.

Escopo:

- Criar crate `forge-core-eval`.
- Definir `EvalCase`, `EvalRun`, `EvalMetric`, `EvalComparison`.
- Implementar `forge eval run`.
- Implementar `forge eval compare --baseline single-agent --candidate graph`.
- Medir accuracy, latency, cost proxy, tool calls, failure reasons e human interventions.

Entregas:

- Harness minimo com fixtures locais.
- Report JSON e Markdown.
- Regra de produto: MAS so vira recommended se vencer baseline em qualidade ou custo para a tarefa alvo.

## Milestone 4 - memory policy

Objetivo: permitir memoria sem criar autoridade invisivel.

Escopo:

- Criar crate `forge-core-memory`.
- Definir `MemoryRecord`, `MemoryPolicy`, `MemoryAdmission`, `MemoryPromotion`, `MemoryReadRequest`.
- Implementar `forge memory inspect`.
- Implementar `forge memory forget`.
- Implementar `forge memory promote` com approval boundary.
- Ligar memoria a source evidence e trace.

Entregas:

- Nenhuma summary vira regra sem promotion.
- Raw evidence fica recuperavel.
- Retention e redaction sao policy, nao prompt.

## Milestone 5 - protocol adapters seguros

Objetivo: expor Forge para ecossistema sem entregar autoridade para o adapter.

Escopo:

- Criar `forge-core-protocol-mcp`.
- Criar `forge protocol mcp serve` com tools read-only primeiro.
- Adicionar mutation tools apenas por OperationContract validado.
- Criar `forge-core-protocol-a2a` depois do MCP estabilizar.
- Definir Agent Card A2A com capabilities restritas.
- Modelar identity, capability e delegation chain.

Entregas:

- MCP tools: preview, ready, explain, graph validate, trace query, memory inspect.
- Mutation tool bloqueada sem authority.
- A2A task surface sem acesso direto ao store.

## Milestone 6 - multi-principal governance

Objetivo: fazer o Forge lidar com agentes e pessoas de principals diferentes no mesmo shared state.

Escopo:

- Definir `PrincipalId`, `IntentContract`, `ConflictContract`, `GovernancePolicy`.
- Implementar conflict detection por lane, target_ref, operation e state_version.
- Implementar `forge conflict list`.
- Implementar `forge conflict resolve` com arbitration record.
- Registrar principal_id em trace, effect metadata e ledger.

Entregas:

- Conflito nao vira overwrite silencioso.
- Worker de outro principal nao consegue mutar sem intent aceito.
- Arbitration fica auditavel.

## Milestone 7 - control plane local

Objetivo: dar uma superficie de produto para o kernel.

Escopo:

- Comecar com HTML estatico ou TUI lendo `.forge-method`.
- Mostrar active agents, lane claims, stale claims, gates, conflicts, ready status, costs e traces.
- Adicionar links para `forge explain` e reports.

Entregas:

- Sem SaaS obrigatorio.
- Funciona offline no repo.
- Serve tanto power user quanto QA.
