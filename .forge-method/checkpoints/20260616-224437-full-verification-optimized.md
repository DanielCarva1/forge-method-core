# Full verification optimized

- created_at: 2026-06-16T22:44:37+00:00
- project: forge-method-core
- phase: 4-build-verify
- status: workflow-selected
- workflow: traceability-gate
- active_story: <none>

## Summary

Reduced full verification cost by caching parity replay prepared state and snapshots, making the CLI matrix use one representative case per required family, and removing duplicate full-fixture safety replay from unit tests. Kept full 92-case parity replay coverage in the in-process replay test and preserved CLI/family coverage with a smaller fixture.

## Decisions

- Use focused verify-fast modes for short work; keep full verification for runtime validation boundaries, now expected around 5-6 minutes instead of the previous 17-minute run.

## Checks

- 3 slow guidance tests passed in 8.396s
- full timed unittest passed: 125 tests in 315.549s
- smoke-runtime.ps1 passed
- verify-fast.ps1 passed in 340.3s
- parity replay CLI measured at 6.616s

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- README.md
- docs/00-quickstart.md

## Artifacts

- .forge-method/evidence/20260616-224427-validation-full-verification-runtime-optimization.md

## Next Action

Continue post-parity experience audit; use focused verification for local changes and full verification only at runtime validation boundaries.
