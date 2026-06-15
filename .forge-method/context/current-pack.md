# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-template-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by checking persona lens and command/automation assertions; a route only counts when human guidance, compact artifact shape, and required automation handoff are protected by replay or gate evidence.

## Latest Checkpoint

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

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/personas/overlays.json
- skills/forge-method/facilitation/storytelling.md
- skills/forge-method/references/workflow-storytelling.md
- skills/forge-method/templates/storytelling-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/20260615-replay-facilitation-contract.md
- .forge-method/artifacts/20260615-replay-template-contract.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md
- .forge-method/evidence/20260615-122252-validation-presentation-craft-fold-in-validation.md
- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md
- .forge-method/evidence/20260615-131108-validation-replay-facilitation-contract-validation.md
- .forge-method/evidence/20260615-132818-validation-replay-template-contract-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260615-post-parity-polish-audit.md - Post-Parity Polish Audit - Audited facilitation packs, compact workflow refs, and active guidance artifacts; added Stale Guidance Guard and documented current post-parity polish without stale marker text.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the replay facilitation contract requiring pack assertions for human-facing guided parity cases.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-facilitation-contract.md - Replay Facilitation Contract - Strengthened parity replay so human-facing guided cases must assert expected facilitation packs, preventing route-only passes from hiding rich human guidance regressions.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-template-contract.md - Replay Template Contract - Strengthened parity replay so human-facing guided cases must assert expected artifact templates when catalog workflows define them, protecting compact agent handoff artifacts.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the replay template contract requiring expected_template assertions for human-facing guided parity cases.
