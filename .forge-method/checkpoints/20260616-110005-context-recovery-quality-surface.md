# Context recovery quality surface

- created_at: 2026-06-16T11:00:05+00:00
- project: forge-method-core
- phase: 6-evolve
- status: context-recovery-quality-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Resume, context plan, and context health now expose compact quality; context health blocks on project quality failures and points agents to audit/status instead of reporting healthy recovery context.

## Decisions

- Fresh-chat recovery payloads must carry the same compact quality truth as bootstrap/reload surfaces.
- Context health uses level blocked for failed project quality and reserves context compaction commands for budget pressure.

## Checks

- Focused regression: workflow-broken fixture covers resume/context plan/context health quality.
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

- .forge-method/artifacts/20260616-context-recovery-quality-surface.md

## Next Action

Continue the post-parity Forge audit by checking route diagnostics and Help Oracle surfaces where future agents may still get stale or incomplete next-step reasons.
