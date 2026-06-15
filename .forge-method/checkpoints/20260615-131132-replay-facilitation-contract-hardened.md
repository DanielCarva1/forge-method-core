# Replay Facilitation Contract hardened

- created_at: 2026-06-15T13:11:32+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-facilitation-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed weak transcript coverage where help/confusion/correct-course replay cases verified routes but not the rich facilitation packs. Replay now requires pack assertions for human-facing guided cases, fixtures declare the packs/templates, and tests cover the negative failure path.

## Decisions

- Human-facing replay cases must protect rich guidance output, not only route/workflow classification.

## Checks

- targeted replay fixture tests: 3 OK
- parity replay: 89/89 passed
- python -m unittest discover -s tests: 80 tests OK
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
- .forge-method/artifacts/20260615-replay-facilitation-contract.md

## Artifacts

- .forge-method/artifacts/20260615-replay-facilitation-contract.md
- .forge-method/evidence/20260615-131108-validation-replay-facilitation-contract-validation.md

## Next Action

Continue post-parity Forge polish by looking for transcript-backed gaps where rich human guidance, persona lenses, templates, or automation outputs are not asserted by replay or gate coverage.
