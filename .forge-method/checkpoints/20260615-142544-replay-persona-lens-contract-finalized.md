# Replay Persona Lens Contract finalized

- created_at: 2026-06-15T14:25:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-persona-lens-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Persona Lens replay assertions and alias scoring are finalized after the raw-token guard: generic words like strategist/designer no longer select QA/UX by accident, while explicit QA/UX and test-framework signals still route correctly.

## Decisions

- Persona ID and alias subset scoring must preserve short role tokens such as qa and ux, so generic words do not hijack the human guidance lens.

## Checks

- python -m unittest discover -s tests: passed (82 tests)
- python skills\\forge-method\\scripts\\forge_method_runtime.py parity replay: passed (89/89)
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py gate --root . --require-evals: passed (9/9 evals)

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/personas/overlays.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-replay-persona-lens-contract.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md

## Next Action

Continue post-parity Forge polish by checking remaining automation, council, and persona handoff assertions; do not count route-only success as parity.
