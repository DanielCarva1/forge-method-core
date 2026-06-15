# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: game-artifact-generators-added
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing remaining validator-only artifacts and converting stable handoff contracts into first-class generators where the human workflow benefits.

## Latest Checkpoint

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

## Recovery Signals

### Failed Checks

- initial smoke-runtime run failed because it still expected the old generic first question; smoke assertions were updated and the rerun passed
- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/discover-intent.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/references/workflow-market-scan.md
- skills/forge-method/references/workflow-domain-scan.md
- skills/forge-method/references/workflow-technical-feasibility-scan.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-202609-gate-quality-gate.md
- .forge-method/evidence/20260615-205726-validation-discovery-closeout-human-guidance-validation.md
- .forge-method/evidence/20260615-212525-validation-spec-kernel-generator-validation.md
- .forge-method/evidence/20260615-220809-validation-research-scan-generator-validation.md
- .forge-method/evidence/20260615-224347-validation-game-artifact-generators-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260615-phase-closeout-generator-audit.md - Phase closeout generator audit - Audited phase-closing workflows for generator plus guided-field extraction coverage and selected spec-kernel as the next central closeout gap.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - spec kernel generator - Unreleased notes updated with artifact spec-kernel, write-spec handoff, product-planning guidance, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260615-research-scan-generator-contract.md - Research scan generator contract - First-class artifact research-scan generator for market, domain, and technical evidence closeouts with source/install smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - research scan generator - Unreleased notes updated with artifact research-scan, Evidence Research handoff, market/domain/technical workflow coverage, tests, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260615-game-artifact-generators-contract.md - Game artifact generators contract - First-class artifact game-brief and game-sprint-plan generators for game brief and playable-slice sprint planning closeouts with source/install smoke coverage.
