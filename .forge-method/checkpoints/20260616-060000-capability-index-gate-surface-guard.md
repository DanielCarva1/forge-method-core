# Capability index gate surface guard

- created_at: 2026-06-16T06:00:00+00:00
- project: forge-method-core
- phase: 6-evolve
- status: capability-index-gate-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the written capability-index validation surface gap: stale or misleading compact agent capability contracts now fail config validate, snapshot quality, builder validate, and gate while config index --write remains the repair path.

## Decisions

- Use capability_index_validation_errors for the written compact agent capability contract, regenerate the repo capability index, and keep config index --write based on override validation so stale files can be repaired.

## Checks

- focused capability-index regression: passed
- related config/builder regression tests: passed
- python -m unittest discover -s tests: 122 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- audit/artifact verify/workflow validate/agent validate/config validate/builder validate: passed
- parity replay: 91/91 passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/context/capability-index.json
- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- .forge-method/context/capability-index.json

## Next Action

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.
