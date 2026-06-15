# Game artifact generators added

- created_at: 2026-06-15T22:44:12+00:00
- project: forge-method-core
- phase: 6-evolve
- status: game-artifact-generators-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact game-brief and artifact game-sprint-plan so game brief and playable-slice sprint planning closeouts are generated, registered, and validated before downstream game production.

## Decisions

- Use first-class runtime generators for game handoff artifacts where the validator already defines a stable contract; preserve rich human game facilitation in packs and compact state-machine handoff in workflow refs.

## Checks

- game generator test passed; game-check contract test passed; packaged workflow validation test passed; game depth compactness regression test passed; workflow validate passed; workflow compactness passed; parity replay 90/90 passed; smoke-runtime.ps1 passed; smoke-install.ps1 passed; python -m unittest discover -s tests passed; verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/game-brief.md
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/references/workflow-game-brief.md
- skills/forge-method/references/workflow-game-sprint-planning.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-game-artifact-generators-contract.md

## Next Action

Continue post-parity Forge polish by auditing remaining validator-only artifacts and converting stable handoff contracts into first-class generators where the human workflow benefits.
