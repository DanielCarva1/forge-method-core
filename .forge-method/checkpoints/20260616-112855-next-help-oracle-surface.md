# Next Help Oracle surface

- created_at: 2026-06-16T11:28:55+00:00
- project: forge-method-core
- phase: 6-evolve
- status: next-help-oracle-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

next now has a compact JSON surface and text route diagnostics, preserving Help Oracle reason, context boundary, quality, commands, state update hints, and mechanical goal handoff after resume.

## Decisions

- next remains the terse human continuation command, but next --json is now the compact agent follow-up to resume --json.
- Text next prints reason and context boundary so stale-state overrides are explainable without full snapshot parsing.

## Checks

- Focused regressions cover human input, ready stale next_action, active evolve workflow, broken workflow quality, and mechanical goal handoff.
- python -m unittest discover -s tests passed, 125 tests.
- smoke-runtime, smoke-install, verify-fast, parity replay 91/91, artifact verify, audit, and gate 22/22 passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260616-next-help-oracle-surface.md

## Next Action

Continue the post-parity Forge audit by checking whether guide and Help Oracle route diagnostics are consistently mirrored in persisted recovery artifacts and capability indexes.
