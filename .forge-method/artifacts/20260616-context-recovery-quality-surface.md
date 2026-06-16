# Context Recovery Quality Surface

- kind: runtime-builder
- status: context-recovery-quality-surface
- phase: 6-evolve
- workflow: runtime-builder

## Problem

`resume --json`, `context plan --json`, and `context health --json` are the commands the runtime tells agents to use after reload, interruption, or fresh-chat recovery. They carried route, context, and plugin diagnostics, but not the compact project quality surface.

That meant an agent could follow the recovery path, see context health as `ok`, and still miss workflow/config/builder/agent failures that `gate` would reject.

## Contract

- `resume --json` includes compact `quality`.
- Text `resume` prints `Quality: passed|failed` and surface-prefixed errors.
- `context plan --json` includes compact `quality`.
- `context health --json` includes compact `quality`.
- `context health` text prints `Quality`.
- When quality fails, `context health` returns `level: blocked`, recommends project quality repair, and offers `audit` and `status --brief` commands instead of context compaction.
- Budget-only context health behavior stays unchanged when quality passes.

## Human Experience

Fresh-chat recovery no longer says the context is fine when the project itself is not safe to continue. The human sees the same compact quality truth in resume/context commands that bootstrap and reload already expose.

## Agent Contract

Agents can treat `resume.quality`, `context_plan.quality`, and `context_health.quality` as the canonical compact gate-adjacent health signal. `context_health.level == blocked` now means either context budget is unsafe or project quality is unsafe; `recommended_action` and commands distinguish which one.

## Proof

- Regression fixture creates a broken local workflow.
- `resume` text prints `Quality: failed` and the workflow error.
- `resume --json` exposes `quality.surfaces.workflows.errors`.
- `context plan --json` exposes the same compact quality.
- `context health` blocks on failed quality and returns `audit` plus `status` commands.
- Clear-budget context health still returns `ok` when quality passes.

## Touched Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `CHANGELOG.md`

## Next

Continue the post-parity Forge audit by checking route diagnostics and Help Oracle surfaces where future agents may still get stale or incomplete next-step reasons.
