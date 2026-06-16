# help-oracle-guidance-safety-guard

- created_at: 2026-06-16T02:16:24+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Added reusable guidance safety validation for Help Oracle/runtime output and wired it into audit so unsafe stale-chat or stale-state instructions fail validation.

## Decisions

- Share the misleading-guidance detector between workflow refs and structured Help Oracle payloads; keep executable command strings excluded.
- Make instead-of safety position-aware so durable-state-first guidance passes and chat-memory-first guidance fails.

## Checks

- focused Help Oracle safety tests passed
- python -m unittest discover -s tests passed: 104 tests
- workflow validate, workflow compactness, parity replay, verify-fast, smoke-runtime, and smoke-install passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-help-oracle-guidance-safety-guard.md

## Next Action

Audit remaining agent-facing runtime surfaces for stale-route safety without flattening human-facing guidance.
