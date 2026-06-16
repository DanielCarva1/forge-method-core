# Bootstrap Quality Surface

- kind: runtime-builder
- status: bootstrap-quality-surface
- phase: 6-evolve
- workflow: runtime-builder

## Problem

Hot start surfaces were still using `audit` as the visible health signal. That was too narrow: `gate` and `snapshot` already validated workflows, facilitation packs, config, local builder extensions, agents, and artifacts, but `start`, `status --brief`, and text `preflight` could say `Audit: passed` while a broken workflow made `gate` fail.

That is bad agent experience. A future agent can enter through the correct launcher, see a green-looking bootstrap, and only discover the real problem later.

## Contract

- `build_status_brief` now includes a compact `quality` summary derived from snapshot quality.
- Text `start`, `status --brief`, and existing-project `preflight` print `Quality: passed|failed`.
- When quality fails, the bootstrap text prints the first errors grouped by surface: audit, artifacts, workflows, agents, config, or builder.
- The existing `Audit:` line remains for compatibility, but it is no longer the only visible health signal.

## Human Experience

The human does not need to know that `audit` is narrower than `gate`. Bootstrap now says when the project is actually not healthy enough to trust, using the same vocabulary already present in snapshot/gate quality.

## Agent Contract

Future agents can inspect `preflight --json` and read `status.quality` without running a separate gate first. They can also see the same failure in text `start`, `status --brief`, and `preflight`.

## Proof

- Regression fixture creates a broken project workflow.
- `workflow validate` fails.
- `snapshot.quality.workflows.errors` includes the workflow failure.
- `start`, `status --brief`, and text `preflight` print `Quality: failed`.
- `preflight --json` exposes `status.quality.surfaces.workflows.errors`.
- `gate` still fails with the workflow-prefixed error.

## Touched Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `CHANGELOG.md`

## Next

Continue the post-parity Forge audit by checking the remaining bootstrap and recovery surfaces where a narrow status signal could mislead future agents or humans.
