# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-facilitation-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by looking for transcript-backed gaps where rich human guidance, persona lenses, templates, or automation outputs are not asserted by replay or gate coverage.

## Latest Checkpoint

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

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/templates/research-scan-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- skills/forge-method/personas/overlays.json
- skills/forge-method/facilitation/storytelling.md
- skills/forge-method/references/workflow-storytelling.md
- skills/forge-method/templates/storytelling-artifact.md
- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-110943-validation-research-guidance-depth-validation.md
- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md
- .forge-method/evidence/20260615-122252-validation-presentation-craft-fold-in-validation.md
- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md
- .forge-method/evidence/20260615-131108-validation-replay-facilitation-contract-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include Stale Guidance Guard and post-parity audit cleanup behavior.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-post-parity-polish-audit.md - Post-Parity Polish Audit - Audited facilitation packs, compact workflow refs, and active guidance artifacts; added Stale Guidance Guard without storing forbidden stale markers in active guidance text.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-post-parity-polish-audit.md - Post-Parity Polish Audit - Audited facilitation packs, compact workflow refs, and active guidance artifacts; added Stale Guidance Guard and documented current post-parity polish without stale marker text.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the replay facilitation contract requiring pack assertions for human-facing guided parity cases.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-facilitation-contract.md - Replay Facilitation Contract - Strengthened parity replay so human-facing guided cases must assert expected facilitation packs, preventing route-only passes from hiding rich human guidance regressions.
