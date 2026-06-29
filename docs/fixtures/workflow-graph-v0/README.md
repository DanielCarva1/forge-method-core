# WorkflowGraph v0 fixtures

These fixtures are the M2-S4 documentation set for `forge-core graph` work.
They follow the current v0 shape from
`docs/dev-docs/forge-method-core-dev-docs-v2/schemas/workflow_graph_v0.yaml`:
`schema_version`, `kind`, `graph_id`, `authority_boundary`, `nodes`, `edges`,
and `stop_conditions`.

## Fixtures

| Fixture | Purpose | `graph validate` | `graph run --dry-run` |
| --- | --- | --- | --- |
| `valid-parallel-readonly.yaml` | Three independent read-only operation nodes converge into one verifier. | exit 0 / `passed` | exit 0 / `passed`; read-only nodes are eligible for the same dependency level. |
| `verifier-blocks-mutation.yaml` | Read-only state check flows into a verifier, then a mutation-capable operation. The verifier uses `missing_required_evidence` to model fail-closed behavior. | exit 0 / `passed` | non-zero / `blocked`; `write_artifact` is not planned for mutation. |
| `invalid-duplicate-node.yaml` | Two nodes share `node_id: read_status`. | non-zero / duplicate-node diagnostic | not expected to run. |
| `invalid-missing-edge-endpoint.yaml` | A verifier and edge reference `missing_context`, which is not declared as a node. | non-zero / missing-endpoint diagnostic | not expected to run. |
| `invalid-cycle.yaml` | Two read-only nodes depend on each other. | non-zero / cycle diagnostic | not expected to run. |

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

Validate the invalid graph fixtures:

```powershell
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-duplicate-node.yaml --allow-bootstrap-core --json
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-missing-edge-endpoint.yaml --allow-bootstrap-core --json
forge-core graph validate --root . --graph docs/fixtures/workflow-graph-v0/invalid-cycle.yaml --allow-bootstrap-core --json
```

Expected invalid status: each command exits non-zero and reports accumulated
validation diagnostics. The invalid fixtures are intentionally valid YAML so the
validator can report graph-level problems rather than YAML parse failures.
