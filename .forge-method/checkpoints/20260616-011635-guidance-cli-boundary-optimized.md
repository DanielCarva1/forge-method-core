# Guidance CLI boundary optimized

- created_at: 2026-06-16T01:16:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-cli-boundary-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Converted remaining JSON-only Guidance Engine assertions to direct runtime calls and documented which guide subprocess checks remain intentional CLI coverage.

## Decisions

- JSON contracts are tested through build_guide_payload; human text and integration surfaces keep guide subprocess coverage.

## Checks

- Focused tests passed for Reality Gate, human lede, lifecycle closure, mechanical work order, and project create guidance.
- python -m unittest discover -s tests passed: 99 tests in 250.728s.
- verify-fast.ps1 passed: unittest, onboarding assets, workflow validation, and agent profile validation.

## Failed Checks

- none

## Touched Files

- tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/20260616-guidance-cli-boundary-test-optimization.md

## Next Action

Continue improving Forge human guidance depth and agent compactness; keep remaining guide subprocess checks as CLI proof unless replacement coverage is equivalent.
