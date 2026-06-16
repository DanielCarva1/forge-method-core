# Workflow guidance safety guard

- created_at: 2026-06-16T01:58:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Added validation that blocks misleading compact workflow refs from relying on chat memory, stale state, procedural continue prompts, or catalog dumps.

## Decisions

- Agent-facing workflow refs now have line-level safety validation in addition to compact structure validation.

## Checks

- Focused workflow guidance safety and packaged workflow tests passed.
- Full unittest passed: 101 tests in 239.976s.
- workflow validate, workflow compactness, parity replay, verify-fast, smoke-runtime, and smoke-install passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/20260616-workflow-guidance-safety-guard.md

## Next Action

Audit runtime help/oracle output for the same safety boundary while preserving rich human guidance.
