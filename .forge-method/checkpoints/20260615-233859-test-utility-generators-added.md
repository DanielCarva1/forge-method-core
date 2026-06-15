# Test utility generators added

- created_at: 2026-06-15T23:38:59+00:00
- project: forge-method-core
- phase: 6-evolve
- status: test-utility-generators-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact test-framework, artifact test-automation, and artifact game-e2e-scaffold so test architecture, automation, and game E2E closeouts are generated, registered, and validated before downstream quality gates.

## Decisions

- Use first-class runtime generators for test utility handoff artifacts where the validator already defines stable contracts; preserve rich human QA/game guidance in packs and compact state-machine handoff in workflow refs.

## Checks

- test generator test passed; test-check contract test passed; packaged workflow validation test passed; workflow validate passed; workflow compactness passed; parity replay 90/90 passed; smoke-runtime.ps1 passed; smoke-install.ps1 passed; python -m unittest discover -s tests passed; verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/test-architecture.md
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/references/workflow-test-framework.md
- skills/forge-method/references/workflow-test-automation.md
- skills/forge-method/references/workflow-game-e2e-scaffold.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-test-utility-generators-contract.md

## Next Action

Continue post-parity Forge polish by adding enterprise/doc utility generators for remaining stable validator-only artifacts.
