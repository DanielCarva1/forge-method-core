# Reload Quality Surface

- kind: runtime-builder
- status: reload-quality-surface
- phase: 6-evolve
- workflow: runtime-builder

## Problem

`reload` is the emergency recovery entrypoint when a chat looks stale. After bootstrap quality was added to `start`, `status --brief`, and `preflight`, `reload` still summarized existing projects with route, state, and next command only.

That left a recovery gap: a future agent could run the correct stale-context command and still miss workflow, config, builder, agent, or artifact quality failures that `gate` would reject.

## Contract

- Existing-project `reload --json` now includes the compact `quality` summary from `build_status_brief`.
- Text `reload` now prints `Quality: passed|failed` for existing projects.
- Failed quality prints compact surface-prefixed errors, matching `start`, `status --brief`, and `preflight`.
- Missing-state reload remains focused on route selection and does not load project quality before a project is selected.

## Human Experience

When the human asks for reload because the assistant feels stale, the command now says whether the project state is actually healthy enough to trust. It no longer gives a clean-looking route while hiding quality failures.

## Agent Contract

Recovery agents can use `reload --json.quality` as the same compact quality signal exposed by status/preflight. They do not need to infer health from `Audit:` or run a separate gate before noticing broken workflow/catalog/config surfaces.

## Proof

- Regression fixture creates a broken local workflow.
- `reload` text prints `Quality: failed` and a workflow-prefixed error.
- `reload --json` exposes `quality.surfaces.workflows.errors`.
- Existing `gate` still fails with the workflow-prefixed error.

## Touched Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `CHANGELOG.md`

## Next

Continue the post-parity Forge audit by checking recovery outputs that still lack compact machine-readable quality, context, or route diagnostics.
