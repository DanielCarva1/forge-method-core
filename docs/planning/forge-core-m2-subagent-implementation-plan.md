# Forge Core M2 subagent implementation plan

Date: 2026-06-29
Branch: `codex/forge-m2-graph`

## Status

M2 is **now story-ready** for a thin vertical implementation. The imported
dev-docs define the target, but they were not already broken into reviewed
implementation stories. This file is the binding M2 story pack for this repo.

## Source-of-truth inputs

- `docs/dev-docs/forge-method-core-dev-docs-v2/02_implementation_plan.md`
- `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md#f04---workflowgraph-v0`
- `docs/dev-docs/forge-method-core-dev-docs-v2/03_architecture_and_contracts.md#workflowgraph`
- `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0003-workflow-graph-first-class.md`
- M1 commit `d304559`: preview, ready, trace, explain are available and must not regress.

## M2 product goal

Give agents a deterministic, declarative workflow graph so composed work does
not depend on prompt routing. M2 must support validating graph structure and
dry-running graph execution order without applying effects.

## Non-goals

- No real mutation execution through graph in M2.
- No multi-agent recommendation/eval decision; that remains M3.
- No memory/protocol/governance implementation beyond explicit enum placeholders.
- No `anyhow`, `thiserror`, or broad CLI parser migration.

## Stories

### M2-S1 — `forge-core-graph` contract crate

Ownership:

- `Cargo.toml`
- `Cargo.lock`
- `crates/forge-core-graph/`

Acceptance:

- Defines `WorkflowGraph`, `GraphNode`, `GraphEdge`, `GraphBudget`,
  `GraphStopCondition`, `GraphAuthorityBoundary`.
- Node kinds include at least `operation`, `verifier`, and `human_gate`; other
  ADR-listed kinds may exist as inert enum variants.
- Serde shape is deterministic, `deny_unknown_fields` where appropriate.
- Manual error enums derive `Debug, Clone, PartialEq, Eq`.
- Unit tests cover valid graph parse, unknown/empty shape rejection, cycle
  detection, and verifier-blocked dry-run.

### M2-S2 — graph validation and dry-run semantics

Ownership:

- `crates/forge-core-graph/src/lib.rs`
- `crates/forge-core-graph/tests/`

Acceptance:

- `validate_graph(&WorkflowGraph) -> GraphValidationReport` accumulates all
  diagnostics instead of short-circuiting.
- Validation rejects duplicate node ids, missing edge endpoints, cycles, empty
  graph, and invalid operation nodes with empty `operation_ref`.
- `dry_run_graph(&WorkflowGraph) -> GraphDryRunReport` is deterministic and
  non-mutating.
- Dry-run returns ordered node steps; verifier nodes can block downstream
  mutation-capable operation nodes.

### M2-S3 — CLI `forge-core graph validate|run --dry-run`

Ownership:

- `crates/forge-core-cli/src/main.rs`
- `crates/forge-core-cli/src/lib.rs`
- `crates/forge-core-cli/src/graph_cmd.rs`
- `crates/forge-core-cli/Cargo.toml`
- `crates/forge-core-cli/tests/graph_cli_e2e.rs`

Acceptance:

- `forge-core graph validate --root <project> --graph <path> --json`.
- `forge-core graph run --root <project> --graph <path> --dry-run --json`.
- Commands resolve sidecar/project root before reading relative graph paths.
- Commands exit non-zero when validation or dry-run is blocked.
- CLI errors follow manual enum convention and do not add parser frameworks.
- E2E covers sidecar consumer repo and no local `.forge-method` creation.

### M2-S4 — fixtures and docs

Ownership:

- `docs/fixtures/workflow-graph-v0/`
- `docs/planning/forge-core-m2-subagent-implementation-plan.md`

Acceptance:

- Valid graph fixture with parallel read-only branches.
- Graph fixture with verifier blocking a mutation-capable operation.
- Invalid graph fixture for duplicate/missing/cycle validation.
- README documents CLI examples and expected status.

## Integration order

1. S1/S2 first: graph crate compiles and has local tests.
2. S4 can run in parallel with S1/S2 because it writes only docs/fixtures.
3. S3 starts after S1 public APIs are stable enough, but can scaffold CLI in
   parallel using expected API names.
4. Reviewers validate graph semantics and CLI behavior before full suite.

## Verification

Targeted:

```powershell
cargo test -p forge-core-graph
cargo test -p forge-core-cli --test graph_cli_e2e
cargo check -p forge-core-cli
forge-core validate --root . --json
```

Closing:

```powershell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::pedantic
cargo fmt --all -- --check
```

## Fan-out lanes

| Lane | Agent | Write scope | Output |
|---|---|---|---|
| L1 Graph crate | worker | `Cargo.toml`, `Cargo.lock`, `crates/forge-core-graph/` | Contract types, validator, dry-run, tests |
| L2 Fixtures/docs | worker | `docs/fixtures/workflow-graph-v0/`, this plan | Fixture pack and usage docs |
| L3 CLI graph | worker | `crates/forge-core-cli/` | CLI command module and E2E |
| R1 Graph review | validator | read-mostly L1/L2 | Semantics, diagnostics, determinism |
| R2 CLI review | validator | read-mostly L3 | Project resolution, error conventions, sidecar isolation |

## Risk controls

- Do not allow graph dry-run to execute commands or apply effects.
- Treat verifier failed/blocked as fail-closed.
- Keep M2 thin: graph runtime produces plan/dry-run reports, not execution.
- Do not let a graph command use `.forge-method` in a consumer repo; use
  resolved project roots as M0/M1 do.
