# Forge Method Core v2 - arquitetura e contratos

## Arquitetura alvo

```txt
camada de produto
  guided start
  preview
  ready
  explain
  control plane

camada declarativa
  workflow graph
  operation contracts
  memory policy
  governance policy
  eval cases
  protocol projections

kernel Rust deterministico
  validate
  runtime planner
  graph executor
  store
  WAL
  locks
  effect transaction
  trace
  eval
  memory admission
  protocol adapters

mundo externo
  MCP tools
  A2A agents
  GitHub/Jira/Slack/Linear
  CI
  local filesystem
```

## Contratos novos

### WorkflowGraph

Responsabilidade: representar workflow executavel. Nao substitui `OperationContract`; coordena multiplas operacoes e verificadores.

Campos obrigatorios:

- `graph_id`
- `schema_version`
- `nodes`
- `edges`
- `budgets`
- `stop_conditions`
- `authority_boundary`

Tipos de node v0:

- `operation`
- `verifier`
- `human_gate`
- `memory_read`
- `memory_write_candidate`
- `protocol_call`
- `eval_probe`

### TraceEvent

Responsabilidade: registrar comportamento real de runtime.

Eventos v0:

- `run_started`
- `node_planned`
- `node_started`
- `tool_requested`
- `tool_completed`
- `effect_staged`
- `effect_applied`
- `gate_passed`
- `gate_blocked`
- `verifier_passed`
- `verifier_failed`
- `replan_requested`
- `run_completed`
- `run_failed`

### MemoryPolicy

Responsabilidade: decidir o que pode ser lembrado, lido, esquecido e promovido.

Regras v0:

- Summary nao cria autoridade.
- Raw evidence precisa ser preservada ou referenciada.
- Promotion de memoria para skill/regra exige contrato de autoridade.
- Forget e redaction sao comandos de primeira classe.
- Memoria tem consumer_use: discovery, diagnostics, handoff_context, product_personalization.

### GovernancePolicy

Responsabilidade: resolver shared state com varios principals.

Entidades v0:

- `PrincipalId`
- `IntentContract`
- `OperationClaim`
- `ConflictContract`
- `ArbitrationRecord`
- `GovernanceDecision`

## Regra de autoridade

Adapters, agentes externos, context files, memory summaries e tool outputs nao sao fonte de verdade de workflow. Eles podem sugerir, reportar ou carregar contexto. Apenas contratos validados pelo kernel podem mutar estado.

## Onde encaixa no repo atual

- `forge-core-contracts`: structs e schemas para contratos novos.
- `forge-core-validate`: validators e cross-reference checks.
- `forge-core-kernel`: planner e executor de graph.
- `forge-core-store`: WAL, effects, trace append, metadata index.
- `forge-core-cli`: comandos de usuario.
- Novos crates: `forge-core-trace`, `forge-core-graph`, `forge-core-eval`, `forge-core-memory`, `forge-core-protocol-mcp`, `forge-core-protocol-a2a`.
