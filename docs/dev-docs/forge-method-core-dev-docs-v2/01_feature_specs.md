# Forge Method Core v2 - feature specs

Date: 2026-06-28

This file details each recommended feature. Use it as a basis for epics, issues, and acceptance criteria.


## F01 - forge preview

Priority: P0  
Users: all  
Evidence: P21,P22,P28,O03,C06  
Main crates: forge-core-kernel, forge-core-store, forge-core-cli

Demand: Fear of wrong mutation, need to understand impact before action.

Product: Command and API that show plan, files, commands, authority, gates, effects, and rollback before applying.

Acceptance criteria:

- Given a mutable OperationContract, preview returns deterministic JSON with status, touched_refs, risk, gates, rollback_available, and next_human_action.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F02 - forge ready

Priority: P0  
Users: common user, QA, dev, company  
Evidence: P22,P23,P30,C06  
Main crates: forge-core-kernel, forge-core-validate, forge-core-cli

Demand: Operational confidence and validation before declaring ready.

Product: Unified gate for tests, lint, typecheck, evals, security checks, and readiness report.

Acceptance criteria:

- A run only passes if all mandatory gates pass; failures return typed reasons and evidence.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F03 - Canonical TraceEvent and forge explain

Priority: P0  
Users: all  
Evidence: P04,P07,P17,P24,P26,C06  
Main crates: new forge-core-trace, runtime, cli

Demand: Know what happened, why it happened, and how to audit it.

Product: Machine-readable NDJSON trace and human explanation per run.

Acceptance criteria:

- Every operation generates trace_id, node_id, actor_agent_id, principal_id, input_refs, output_refs, decision_reason, and cost.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F04 - WorkflowGraph v0

Priority: P0  
Users: power user, company  
Evidence: P04,P05,P06,C03,C06  
Main crates: new forge-core-graph, runtime

Demand: Orchestrate without loose prompt-based routing.

Product: Declarative graph with nodes, edges, budgets, verifier nodes, and replan boundaries.

Acceptance criteria:

- forge graph validate and forge graph run --dry-run work without executing effects; the executor respects dependencies and stop conditions.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F05 - Eval Compare single-agent baseline

Priority: P1  
Users: power user, research, company  
Evidence: P01,P02,P03,P07  
Main crates: new forge-core-eval

Demand: Prove when multi-agent is worth the cost.

Product: Harness to compare single-agent anchor, graph workflow, and MAS under the same loader, tools, output contract, and usage accounting.

Acceptance criteria:

- Report shows accuracy, cost, latency, trajectory length, failures, and delta against baseline.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F06 - Memory Policy

Priority: P1  
Users: all  
Evidence: P09,P10,P11,P28,O03  
Main crates: new forge-core-memory

Demand: Personalization without opaque or dangerous memory.

Product: Memory admission, retention, forget, promote, raw evidence, and authority boundary.

Acceptance criteria:

- No memory becomes authority automatically; promote requires policy and raw evidence.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F07 - Multi-principal governance

Priority: P1  
Users: teams, companies, open source  
Evidence: P08,P24,P25,P26,C01  
Main crates: contracts, validate, runtime, store

Demand: Several agents and people in the same state without silent overwrites.

Product: PrincipalId, IntentContract, ConflictContract, GovernancePolicy, and arbitration ledger.

Acceptance criteria:

- Conflict between principals becomes a structured object, not a silent manual merge.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F08 - Secure MCP adapter

Priority: P1  
Users: power user, companies  
Evidence: O01,P17,P18,P19,P20,O03  
Main crates: new forge-core-protocol-mcp

Demand: Connect real tools securely.

Product: MCP server for preview, ready, graph, trace, memory, and effect application with allowlist and attestation.

Acceptance criteria:

- No MCP tool mutates state without an OperationContract and validated authority.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F09 - Secure A2A adapter

Priority: P2  
Users: power user, companies  
Evidence: O02,P08,P17,P19,P20  
Main crates: new forge-core-protocol-a2a

Demand: Interoperability between agents from different vendors.

Product: A2A agent card and task surface for controlled delegation.

Acceptance criteria:

- A2A does not replace MCP or become an internal subagent protocol; an external task always has PrincipalId and a delegation chain.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F10 - Local Control Plane

Priority: P2  
Users: power user, QA, teams  
Evidence: P06,P07,P21,P28,O03,C01  
Main crates: new forge-core-ui or cli

Demand: See lanes, claims, traces, gates, and risk on a single screen.

Product: TUI or static HTML reading .forge-method without a mandatory SaaS.

Acceptance criteria:

- Shows run status, active claims, stale claims, conflicts, gates, cost, and next action.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F11 - Risk Audit Gate for AI code

Priority: P1  
Users: QA, dev, companies  
Evidence: P22,P23,P30  
Main crates: validate, runtime, cli

Demand: Detect fail-soft, exception swallowing, security slop, and fake tests.

Product: Gate with deterministic checks and extension for SAST/linters.

Acceptance criteria:

- Risk gate fails closed on prohibited patterns and generates a report with evidence per file.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F12 - Guided Start and Product UX

Priority: P2  
Users: common user, founder, beginner dev  
Evidence: P28,P29,O03  
Main crates: cli, docs, templates

Demand: Enter the product without understanding agents, YAML, or protocol.

Product: Guided flow with choice of objective, risk, scaffold, and first preview.

Acceptance criteria:

- User creates a Forge project, sees a minimal spec, preview, and ready without editing YAML manually.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F13 - Budget and Cost Accounting

Priority: P2  
Users: power user, companies  
Evidence: P01,P02,P16,O03  
Main crates: trace, eval, runtime

Demand: Control cost, rounds, model calls, and tool calls.

Product: Budget per run, graph node, agent, principal, and tool.

Acceptance criteria:

- Run blocks or asks for confirmation when a budget threshold is reached.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F14 - Knowledge Orchestration mode

Priority: P3  
Users: research, product, analysts  
Evidence: P13,P14,P15  
Main crates: memory, trace, eval

Demand: Research agents need sources, claims, and evidence, not just a summary.

Product: Research mode with evidence graph, source ledger, and citation checks.

Acceptance criteria:

- Each important claim points to a source_id and local or registered web evidence.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.

## F15 - Rust ergonomics and codegen track

Priority: P0  
Users: maintainers and code agents  
Evidence: O04,O05,O06,O07,P31,C04,C05  
Main crates: all

Demand: Reduce the suffering of the agent writing repetitive hand-written Rust.

Product: manual argv in `main.rs` (no `clap`, no derive macros — design decision in `AGENTS.md`), hand-rolled error enums (no `thiserror`, no `anyhow`), `tracing`, builders, fixtures, module split, contract codegen, and snapshots.

Acceptance criteria:

- A new command or contract does not require editing more than two manual points outside of tests and docs.
- Must produce JSON output for use by agents and human output for the CLI.
- Must register trace_id when participating in a mutable or evaluable run.
- Must fail closed when missing authority, required input, evidence, or gate.

Risks:

- Creating pretty UX without a real authority boundary.
- Increasing Rust boilerplate without codegen or builders.
- Allowing an external adapter to reinterpret Forge state.

Minimum implementation:

1. Define the YAML contract or Rust struct with schema.
2. Create a valid fixture and an invalid fixture.
3. Create a validator with typed diagnostics.
4. Expose a CLI or internal API.
5. Add an output snapshot.
6. Register an event in the trace when applicable.
