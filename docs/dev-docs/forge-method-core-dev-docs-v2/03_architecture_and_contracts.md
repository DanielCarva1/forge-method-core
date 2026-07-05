# Forge Method Core v2 - architecture and contracts

## Target architecture

```txt
product layer
  guided start
  preview
  ready
  explain
  control plane

declarative layer
  workflow graph
  operation contracts
  memory policy
  governance policy
  eval cases
  protocol projections

deterministic Rust kernel
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

external world
  MCP tools
  A2A agents
  GitHub/Jira/Slack/Linear
  CI
  local filesystem
```

## New contracts

### WorkflowGraph

Responsibility: represent an executable workflow. Does not replace `OperationContract`; it coordinates multiple operations and verifiers.

Mandatory fields:

- `graph_id`
- `schema_version`
- `nodes`
- `edges`
- `budgets`
- `stop_conditions`
- `authority_boundary`

v0 node types:

- `operation`
- `verifier`
- `human_gate`
- `memory_read`
- `memory_write_candidate`
- `protocol_call`
- `eval_probe`

### TraceEvent

Responsibility: record actual runtime behavior.

v0 events:

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

Responsibility: decide what can be remembered, read, forgotten, and promoted.

v0 rules:

- Summary does not create authority.
- Raw evidence must be preserved or referenced.
- Promotion of memory to a skill/rule requires an authority contract.
- Forget and redaction are first-class commands.
- Memory has consumer_use: discovery, diagnostics, handoff_context, product_personalization.

### GovernancePolicy

Responsibility: resolve shared state with multiple principals.

v0 entities:

- `PrincipalId`
- `IntentContract`
- `OperationClaim`
- `ConflictContract`
- `ArbitrationRecord`
- `GovernanceDecision`

## Authority rule

Adapters, external agents, context files, memory summaries, and tool outputs are not the source of truth for workflow. They can suggest, report, or carry context. Only contracts validated by the kernel can mutate state.

## Where it fits in the current repo

- `forge-core-contracts`: structs and schemas for new contracts.
- `forge-core-validate`: validators and cross-reference checks.
- `forge-core-kernel`: planner and graph executor.
- `forge-core-store`: WAL, effects, trace append, metadata index.
- `forge-core-cli`: user commands.
- New crates: `forge-core-trace`, `forge-core-graph`, `forge-core-eval`, `forge-core-memory`, `forge-core-protocol-mcp`, `forge-core-protocol-a2a`.
