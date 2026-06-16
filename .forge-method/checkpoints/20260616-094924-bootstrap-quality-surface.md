# Bootstrap quality surface

- created_at: 2026-06-16T09:49:24+00:00
- project: forge-method-core
- phase: 6-evolve
- status: bootstrap-quality-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed bootstrap quality surface: start, status --brief, and existing-project preflight now expose compact full-quality status so agents do not trust audit-only health.

## Decisions

- Keep Audit as a compatibility line, but add Quality as the bootstrap truth for workflow/config/builder/agent health.

## Checks

- 125 tests passed
- smoke-runtime, smoke-install, verify-fast, parity replay, artifact verify, audit, and gate passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-bootstrap-quality-surface.md

## Next Action

Continue the post-parity Forge audit by checking remaining bootstrap and recovery surfaces where narrow status signals can mislead future agents or humans.
