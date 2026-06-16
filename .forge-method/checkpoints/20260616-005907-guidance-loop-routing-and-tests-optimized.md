# Guidance loop routing and tests optimized

- created_at: 2026-06-16T00:59:07+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-loop-tests-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Fixed the test-loop optimization prompt so it stays on runtime-builder instead of false-routing to skill-convert, added a parity replay regression fixture, and converted lifecycle/game/TEA guidance contract loops to direct runtime calls with direct replay state setup.

## Decisions

- Use direct runtime contracts for Guidance Engine matrix assertions; preserve CLI coverage in parity replay, smokes, config index, and focused human-output guide tests.

## Checks

- lifecycle guidance test passed in 8.495s; game studio guidance test passed in 2.327s; game dev mechanical route test passed in 0.357s; TEA guidance test passed in 1.778s; guidance fixture test passed; fixture family test passed; python -m unittest discover -s tests passed 99 tests in 244.008s; parity replay 91/91 passed; workflow validate passed; workflow compactness passed; verify-fast.ps1 passed with unittest at 205.593s

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-guidance-loop-routing-and-test-optimization.md

## Next Action

Continue post-parity Forge polish by profiling the remaining targeted guide subprocess tests and deciding which must remain CLI coverage.
