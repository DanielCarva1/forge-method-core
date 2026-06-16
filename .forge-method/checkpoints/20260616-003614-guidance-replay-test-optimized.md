# Guidance replay test optimized

- created_at: 2026-06-16T00:36:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-replay-test-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Optimized Guidance Engine replay fixture testing by using the runtime replay contract directly instead of spawning guide --json per fixture, preserving 90-case parity assertions while cutting the slow transcript replay test from minutes to seconds.

## Decisions

- Use direct runtime calls for parity fixture matrix tests when the behavior under test is Guidance Engine routing, and keep CLI coverage in parity replay, smokes, and focused guide output tests.

## Checks

- fixture replay test passed in 6.351s; python -m unittest discover -s tests passed 99 tests in 259.381s; parity replay 90/90 passed; workflow validate passed; workflow compactness passed; verify-fast.ps1 passed in 217.6s wall time

## Failed Checks

- none

## Touched Files

- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-guidance-replay-test-optimization.md

## Next Action

Continue post-parity Forge polish by profiling remaining subprocess-heavy guide loops and deciding which should stay CLI coverage versus direct runtime contract tests.
