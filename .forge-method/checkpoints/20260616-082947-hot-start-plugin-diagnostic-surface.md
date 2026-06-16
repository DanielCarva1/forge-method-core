# Hot Start Plugin Diagnostic Surface

- created_at: 2026-06-16T08:29:47+00:00
- project: forge-method-core
- phase: 6-evolve
- status: hot-start-plugin-diagnostic-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the hot-start plugin diagnostic gap: snapshot, resume --json, context plan --json, and text resume now surface plugin installation status and repair commands while keeping plugin readiness diagnostic-only.

## Decisions

- Use diagnostics.plugin_installation as the shared compact contract across snapshot, resume, and context plan instead of requiring agents to run doctor separately.

## Checks

- python -m unittest discover -s tests -v: 125 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- gate --require-evals: 22/22 passed
- parity replay --json: 91/91 passed
- smoke-install.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-snapshot-plugin-diagnostic-surface.md

## Artifacts

- .forge-method/artifacts/20260616-snapshot-plugin-diagnostic-surface.md

## Next Action

Continue the post-parity Forge audit by checking remaining validation surfaces and human/agent experience gaps after exposing plugin diagnostics in hot-start surfaces.
