# Forge Method Core v2 - implementation plan

## Sequencing principle

The order below avoids building high-level features on top of a runtime without trace, without preview, and without a readiness gate. First stabilize the observable kernel. Then come graph, eval, memory, protocols, and governance.

## Milestone 0 - Rust hygiene and ergonomics

Goal: reduce the cost of change for agents and maintainers.

Scope:

- **Keep CLI with manual argv in `main.rs`** (no `clap`, no derive macros). Design decision in `AGENTS.md` — see `04_rust_refactor_guide.md` for the established pattern. (Original: "Migrate CLI to `clap` derive" — revised in R13.2.)
- Split `forge-core-store/src/lib.rs` into modules: `paths`, `jsonl`, `reference_index`, `effect_apply`, `effect_wal`, `effect_recovery`, `effect_metadata`, `locks`.
- **Hand-rolled error enums** (no `thiserror`, no `anyhow`), deriving `Debug, Clone, PartialEq, Eq`. (Original: "Introduce `thiserror`" — revised in R13.2.)
- Introduce `tracing` spans in runtime, validation, store, and CLI paths.
- Create fixture builders for `OperationContract`, `ToolEffectContract`, `CommandContract`, and `RuntimePlan`.
- Add snapshot tests for stable JSON outputs.
- Define a rule: each new subcommand adds an arm to the `match` in `main.rs` and a `run_<command>(&[String])` fn in `lib.rs`.

Deliverables:

- ADR-0001 accepted.
- `forge-core-cli` keeps manual argv (no `clap` Parser/Subcommand).
- Modules split in store.
- Clippy and fmt in CI.

## Milestone 1 - preview, ready, and trace

Goal: every run must be predictable, verifiable, and explainable.

Scope:

- Create crate `forge-core-trace`.
- Define `TraceEvent` v0.
- Add `forge preview` based on the current runtime planner.
- Add `forge ready` as a gate aggregator.
- Add `forge explain --last-run` for a short human explanation.
- Link command evidence and effect metadata to the trace_id.

Deliverables:

- `schemas/trace_event_v0.yaml` implemented.
- Snapshot tests for preview.
- E2E fixture: mutable operation with a pending gate, ready operation, blocked operation.

## Milestone 2 - WorkflowGraph v0

Goal: stop relying on loose prompt-based routing for composite workflows.

Scope:

- Create crate `forge-core-graph`.
- Define `WorkflowGraph`, `GraphNode`, `GraphEdge`, `GraphBudget`, `GraphStopCondition`.
- Implement `forge graph validate`.
- Implement `forge graph run --dry-run`.
- Integrate `OperationContract` as a node type.
- Add a verifier node and a simple replan boundary.

Deliverables:

- `schemas/workflow_graph_v0.yaml` implemented.
- Graph fixture with parallel read-only branches.
- Graph fixture with a verifier blocking a mutation.

## Milestone 3 - eval baseline

Goal: turn architecture into a measurable decision.

Scope:

- Create crate `forge-core-eval`.
- Define `EvalCase`, `EvalRun`, `EvalMetric`, `EvalComparison`.
- Implement `forge eval run`.
- Implement `forge eval compare --baseline single-agent --candidate graph`.
- Measure accuracy, latency, cost proxy, tool calls, failure reasons, and human interventions.

Deliverables:

- Minimal harness with local fixtures.
- JSON and Markdown report.
- Product rule: MAS only becomes recommended if it beats the baseline in quality or cost for the target task.

## Milestone 4 - memory policy

Goal: allow memory without creating invisible authority.

Scope:

- Create crate `forge-core-memory`.
- Define `MemoryRecord`, `MemoryPolicy`, `MemoryAdmission`, `MemoryPromotion`, `MemoryReadRequest`.
- Implement `forge memory inspect`.
- Implement `forge memory forget`.
- Implement `forge memory promote` with an approval boundary.
- Link memory to source evidence and trace.

Deliverables:

- No summary becomes a rule without promotion.
- Raw evidence stays recoverable.
- Retention and redaction are policy, not prompt.

## Milestone 5 - secure protocol adapters

Goal: expose Forge to the ecosystem without handing authority to the adapter.

Scope:

- Create `forge-core-protocol-mcp`.
- Create `forge protocol mcp serve` with read-only tools first.
- Add mutation tools only through a validated OperationContract.
- Create `forge-core-protocol-a2a` after MCP stabilizes.
- Define an A2A Agent Card with restricted capabilities.
- Model identity, capability, and delegation chain.

Deliverables:

- MCP tools: preview, ready, explain, graph validate, trace query, memory inspect.
- Mutation tool blocked without authority.
- A2A task surface without direct access to the store.

## Milestone 6 - multi-principal governance

Goal: make Forge handle agents and people from different principals in the same shared state.

Scope:

- Define `PrincipalId`, `IntentContract`, `ConflictContract`, `GovernancePolicy`.
- Implement conflict detection by lane, target_ref, operation, and state_version.
- Implement `forge conflict list`.
- Implement `forge conflict resolve` with an arbitration record.
- Register principal_id in trace, effect metadata, and ledger.

Deliverables:

- Conflict does not become a silent overwrite.
- A worker from another principal cannot mutate without an accepted intent.
- Arbitration stays auditable.

## Milestone 7 - local control plane

Goal: give a product surface to the kernel.

Scope:

- Start with static HTML or TUI reading `.forge-method`.
- Show active agents, lane claims, stale claims, gates, conflicts, ready status, costs, and traces.
- Add links to `forge explain` and reports.

Deliverables:

- No mandatory SaaS.
- Works offline in the repo.
- Serves both power users and QA.
