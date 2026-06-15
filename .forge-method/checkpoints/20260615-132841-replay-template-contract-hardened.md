# Replay Template Contract hardened

- created_at: 2026-06-15T13:28:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-template-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the remaining human-facing replay gap where correct-course routing asserted pack but not the compact artifact template. Replay now requires template assertions for guided cases with catalog templates, and tests cover fixture/catalog consistency plus negative replay failure.

## Decisions

- Route parity must include the compact agent artifact shape when the catalog defines one; otherwise a green transcript can still drop handoff quality.

## Checks

- targeted replay template tests: 3 OK
- parity replay: 89/89 passed
- python -m unittest discover -s tests: 81 tests OK
- artifact verify --root .: passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260615-replay-template-contract.md

## Artifacts

- .forge-method/artifacts/20260615-replay-template-contract.md
- .forge-method/evidence/20260615-132818-validation-replay-template-contract-validation.md

## Next Action

Continue post-parity Forge polish by checking persona lens and command/automation assertions; a route only counts when human guidance, compact artifact shape, and required automation handoff are protected by replay or gate evidence.
