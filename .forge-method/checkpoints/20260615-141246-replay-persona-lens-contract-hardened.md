# Replay Persona Lens Contract hardened

- created_at: 2026-06-15T14:12:46+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-persona-lens-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Parity replay now treats Persona Lens output as a protected human-guidance contract. The runtime requires expected_persona_lens when a lens is returned, and alias scoring avoids substring architecture hijacks while preserving QA/problem-solving precedence.

## Decisions

- Route-only success is not enough for persona-guided flows; replay fixtures must assert the selected Persona Lens whenever guidance returns one.

## Checks

- python -m unittest discover -s tests: passed (82 tests)
- python skills\\forge-method\\scripts\\forge_method_runtime.py parity replay: passed (89/89)
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
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
- .forge-method/evidence/20260615-141222-validation-replay-persona-lens-contract-validation.md

## Next Action

Continue post-parity Forge polish by checking remaining automation, council, and persona handoff assertions; do not count route-only success as parity.
