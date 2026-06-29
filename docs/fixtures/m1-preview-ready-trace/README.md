# M1 preview/ready/trace acceptance fixtures

This directory pins the docs-only L5 acceptance surface for the M1 vertical:
`forge-core preview`, `forge-core ready`, canonical trace NDJSON, and
`forge-core explain --last-run`.

No new `OperationContract` YAML files are required for this lane. The existing
fixtures under `docs/fixtures/operation-contract-v0/` already validate and cover
the M1 preview/ready cases below.

## Existing OperationContract fixtures to reuse

| Acceptance use | Fixture | Why it is suitable |
| --- | --- | --- |
| Preview a mutating operation with a command and an effect | `docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml` | Valid operation, `current_gate_status: pass`, command `cmd.validate.story_fast`, effect `contracts/effects/story-artifact-write-effect.yaml`. |
| Preview a minimal mutating operation | `docs/fixtures/operation-contract-v0/execute-trivial-write.yaml` | Small valid write fixture with one effect and lane-claim authority evidence. |
| Preview a blocked destructive operation | `docs/fixtures/operation-contract-v0/destructive-effect-missing-inverse-blocked.yaml` | Valid blocked fixture with destructive effect and missing inverse gate behavior. |
| Ready pass | `docs/fixtures/operation-contract-v0/gate-review-story-ready.yaml` | Required lane gate is present and passing with evidence refs in `contracts/gates/story-ready-lane-gate.yaml`. |
| Ready fail-closed on missing integration gate | `docs/fixtures/operation-contract-v0/integration-gate-required.yaml` | Required integration gate is `missing`; ready must be non-green. |
| Ready fail-closed on pending release gate | `docs/fixtures/operation-contract-v0/release-gate-required.yaml` | Release readiness is `pending`; ready must be non-green. |
| Explain a read-only/no-mutation run | `docs/fixtures/operation-contract-v0/observe-project-status.yaml` | Simple valid observe fixture for trace/explain paths that should not report mutations. |

## CLI acceptance examples

The machine-readable case list is in
`docs/fixtures/m1-preview-ready-trace/acceptance-cases.yaml`.

Resolve project state before asserting trace or explain paths:

```powershell
forge-core project resolve --root . --allow-bootstrap-core --json
```

The implementation may only persist trace/explain state under the resolved
`data.state_root` returned by `project resolve`. Tests must not assume
`<project_root>/.forge-method` unless the resolver explicitly returns that path
for the bootstrap-core exception.

Preview example:

```powershell
forge-core preview --root . --operation docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml --json
```

Ready examples:

```powershell
forge-core ready --root . --operation docs/fixtures/operation-contract-v0/gate-review-story-ready.yaml --json
forge-core ready --root . --operation docs/fixtures/operation-contract-v0/integration-gate-required.yaml --json
forge-core ready --root . --operation docs/fixtures/operation-contract-v0/release-gate-required.yaml --json
```

Explain example:

```powershell
forge-core explain --root . --last-run --json
```

`explain --last-run` reads the latest run trace from the resolved `state_root`
and must say when no trace exists or when a trace is incomplete. It must not
invent gates, effects, commands, or human next actions missing from the trace.
