# Bootstrap Plugin Diagnostic Surface

- created_at: 2026-06-16T09:00:48+00:00
- project: forge-method-core
- phase: 6-evolve
- status: bootstrap-plugin-diagnostic-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the remaining bootstrap diagnostic gap: preflight, reload, context health, context plan, resume, and snapshot now share diagnostics.plugin_installation and text commands print repair guidance when the plugin is not ready.

## Decisions

- Use runtime_diagnostics() as the shared compact contract for plugin installation state across bootstrap and hot-start outputs.

## Checks

- focused bootstrap diagnostic tests: passed
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

Continue the post-parity Forge audit by checking remaining human guidance and agent runtime recovery gaps after exposing plugin diagnostics across bootstrap surfaces.
