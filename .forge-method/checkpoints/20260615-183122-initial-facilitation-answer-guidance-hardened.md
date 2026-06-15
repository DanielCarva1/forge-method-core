# Initial facilitation answer guidance hardened

- created_at: 2026-06-15T18:31:22+00:00
- project: forge-method-core
- phase: 6-evolve
- status: initial-facilitation-answer-guidance-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Hardened the first human answer path after initial-facilitation: answering clears the required input but keeps zero stories, stays in discover-intent, requires Grill Gate, routes through Guidance Engine, and prints clean first-question guidance.

## Decisions

- The first answer after project creation is discovery material, not permission to create backlog or build work; agents must route it through Guidance Engine before moving phases.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing discovery closeout: accepted intent should produce a durable discovery artifact and only then transition toward specification.
