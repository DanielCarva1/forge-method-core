# WorkflowGraph v0 fixtures

These fixtures are the M2-S4 documentation set for `forge-core graph` work.
They follow the current v0 shape from
`docs/dev-docs/forge-method-core-dev-docs-v2/schemas/workflow_graph_v0.yaml`:
`schema_version`, `kind`, `graph_id`, `authority_boundary`, `nodes`, `edges`,
and `stop_conditions`.

## Fixtures

| Fixture | Purpose | `graph validate` | `graph run --dry-run` |
| --- | --- | --- | --- |
| `valid-parallel-readonly.yaml` | Three independent contract-safe read-only operation nodes converge into one verifier. | exit 0 / `passed` | exit 0 / `passed`; read-only nodes are eligible for the same dependency level. |
| `verifier-blocks-mutation.yaml` | Read-only state check flows into a verifier, then a mutation-capable operation. The verifier uses `missing_required_evidence` to model fail-closed behavior. | exit 0 / `passed` | non-zero / `blocked`; `write_artifact` is not planned for mutation. |
| `operation-aware-blocked.yaml` | Graph-valid operation node whose referenced OperationContract requires a pending release gate. | exit 0 / `passed` | non-zero / `blocked`; `release_gate` reports OperationContract not-ready details. |
| `operation-aware-valid.yaml` | Graph YAML says a downstream write is not mutation-capable, but the OperationContract derives mutation from `write_project_files`. | exit 0 / `passed` | non-zero / `blocked`; effective mutation comes from the OperationContract and the failed verifier blocks it. |
| `invalid-duplicate-node.yaml` | Two nodes share `node_id: read_status`. | non-zero / duplicate-node diagnostic | not expected to run. |
| `invalid-missing-edge-endpoint.yaml` | A verifier and edge reference `missing_context`, which is not declared as a node. | non-zero / missing-endpoint diagnostic | not expected to run. |
| `invalid-cycle.yaml` | Two read-only nodes depend on each other. | non-zero / cycle diagnostic | not expected to run. |

`graph validate` is graph-shape-only: it validates schema, node ids, edges,
and acyclicity without opening OperationContract files. `graph run --dry-run`
is still non-mutating, but is contract-aware: it reads operation refs relative
to the resolved project root, derives effective mutation capability from the
OperationContract, reports per-node runtime preview/readiness metadata, and
fails closed when a referenced operation is missing, invalid, or not safe to
plan.

Claim preflight is part of graph dry-run for effective mutations. Read-only
graphs can pass without an agent id. Any mutating OperationContract requires a
live claim check before the dry-run can be green:

- pass `--agent <id>` to identify the writer;
- by default the claim bus is `<resolved_state_root>/claims-active`;
- `--claims-dir <path>` is an advanced override for tests/migrations;
- `--now-unix <epoch>` makes claim-expiry checks deterministic in tests;
- missing agent, missing coverage, expired coverage, peer-owned coverage, or
  unexpandable glob write targets all block the dry-run.

Claim targets come from referenced ToolEffect contracts using the same
file-backed physical-target mapping as the effect store: `file_path`,
`artifact_id`, `evidence_id`, `ledger_stream`, and `request_stream` writes are
resolved into repo-relative claim targets before checking the live claim bus. If
no file-backed write target exists, dry-run falls back to the OperationContract
coordination target paths. Unsupported or unexpandable write targets block
fail-closed.

## CLI examples

These examples run from the Forge core checkout, which is still using the
explicit bootstrap exception. Consumer projects with a `.forge-method.yaml`
project link should omit `--allow-bootstrap-core`.

Validate the valid graph:

```powershell
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/valid-parallel-readonly.yaml --allow-bootstrap-core --json
```

Dry-run the valid graph without applying effects:

```powershell
forge-core graph run --root . --graph docs/fixtures/workflow-graph-v0/valid-parallel-readonly.yaml --dry-run --allow-bootstrap-core --json
```

Validate the verifier-block fixture, then confirm dry-run blocks the downstream
mutation-capable operation:

```powershell
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/verifier-blocks-mutation.yaml --allow-bootstrap-core --json
forge-core graph run --root . --graph docs/fixtures/workflow-graph-v0/verifier-blocks-mutation.yaml --dry-run --allow-bootstrap-core --json
```

Confirm OperationContract-aware dry-run behavior:

```powershell
forge-core graph run --root . --graph docs/fixtures/workflow-graph-v0/operation-aware-blocked.yaml --dry-run --allow-bootstrap-core --json
forge-core graph run --root . --graph docs/fixtures/workflow-graph-v0/operation-aware-valid.yaml --dry-run --allow-bootstrap-core --json
```

Run a mutating graph as a concrete agent after acquiring the needed claim
(the published fixture still blocks on its failed verifier):

```powershell
forge-core claim acquire --root . --allow-bootstrap-core --scope story --id graph-demo --agent codex-main --path .forge-method/artifacts/ --path .forge-method/evidence/
forge-core graph run --root . --graph docs/fixtures/workflow-graph-v0/operation-aware-valid.yaml --dry-run --agent codex-main --allow-bootstrap-core --json
```

Validate the invalid graph fixtures:

```powershell
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-duplicate-node.yaml --allow-bootstrap-core --json
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-missing-edge-endpoint.yaml --allow-bootstrap-core --json
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-cycle.yaml --allow-bootstrap-core --json
```

Expected invalid status: each command exits non-zero and reports accumulated
validation diagnostics. The invalid fixtures are intentionally valid YAML so the
validator can report graph-level problems rather than YAML parse failures.
