# State guidance write guard

- created_at: 2026-06-16T03:25:20+00:00
- project: forge-method-core
- phase: 6-evolve
- status: state-guidance-write-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed a runtime-builder audit gap: guidance-bearing state fields now pass the same misleading-guidance safety contract before write_state persists them, and audit catches preexisting contaminated state.

## Decisions

- Treat next_action, last_route_reason, and guide_summary as durable runtime guidance; validate them at write time and audit time while leaving IDs and project metadata outside the prose scan.

## Checks

- unittest 110; smoke-runtime; verify-fast; smoke-install; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-state-guidance-write-guard.md

## Artifacts

- .forge-method/artifacts/20260616-state-guidance-write-guard.md

## Next Action

Continue the broader Forge audit by finding runtime outputs that compose durable user/project data with agent guidance and need final deterministic validation before emission.
