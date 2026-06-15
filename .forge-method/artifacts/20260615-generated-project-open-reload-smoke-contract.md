# Generated Project Open/Reload Smoke Contract

- kind: runtime-smoke-contract
- created_at: 2026-06-15T18:02:39Z
- phase: 6-evolve
- workflow: runtime-builder
- status: generated-project-open-reload-smoke-hardened

## Problem

Generated projects already entered `1-discovery` with `initial-facilitation` and workspace parent commands already printed project selection. The smoke scripts exercised those commands but did not fail if the human-facing route regressed into automatic story creation, missing first facilitation, or stale reload copy.

## Contract

- `project create` output must show `Story: <none - facilitation required>`.
- `project create` output must show `required_next_workflow: discover-intent`.
- `project create` output must show `initial-facilitation` and the first prompt beginning with `Antes de criar stories ou desenvolver`.
- `project create` output must not show `Story: project-kickoff`.
- `project list` must show the generated slug and `waiting-human-input`.
- workspace parent `preflight` must show `Route: workspace-with-projects`, known projects, the existing-project question, and the open option.
- workspace parent `reload` must show `Forge Reload`, `Route: workspace-with-projects`, known projects, and the stale-copy guard next step.

## Proof

- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next

Continue post-parity Forge polish by auditing the first human answer path after `initial-facilitation`, ensuring it routes through Guidance Engine instead of creating premature stories.
