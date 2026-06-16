# Builder extension gate surface guard

- created_at: 2026-06-16T05:31:29+00:00
- project: forge-method-core
- phase: 6-evolve
- status: builder-extension-gate-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the builder extension validation surface gap: local skill frontmatter validation now feeds builder validate, snapshot quality, and the quality gate.

## Decisions

- Use builder_extension_validation_errors as the canonical local extension validation surface instead of keeping the check inside builder validate only.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_gate_and_snapshot_use_builder_extension_validation_surface -v: passed
- python -m unittest discover -s tests: 121 passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1: passed
- audit/artifact verify/workflow validate/agent validate/builder validate: passed
- parity replay: 91/91 passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-builder-extension-gate-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-builder-extension-gate-surface-guard.md

## Next Action

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.
