# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: guidance-human-polish-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Review remaining post-parity polish surface and decide the next release/version batch.

## Latest Checkpoint

# Guidance human experience polish complete

- created_at: 2026-06-15T01:59:36+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-human-polish-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Guidance Engine human experience polish with contextual guide lede, runtime-builder routing for human-experience plus agent-doc polish, and quiet correction/runtime Reality/Evidence Gate behavior.

## Decisions

- Human-facing guide output carries the rich lede; workflow refs, state, JSON, and handoffs remain compact for agents.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- smoke-runtime: passed
- verify-fast: passed
- smoke-install: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-guidance-human-experience-polish.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md

## Next Action

Review remaining post-parity polish surface and decide the next release/version batch.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/facilitation/test-architecture.md
- .forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md
- CHANGELOG.md
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-003700-validation-p1-4-lifecycle-closure-validation.md
- .forge-method/evidence/20260615-010242-validation-p1-5-game-studio-depth-validation.md
- .forge-method/evidence/20260615-013149-validation-p1-6-test-architecture-enterprise-depth-validati.md
- .forge-method/evidence/20260615-013605-planning-p2-scope-decisions-recorded.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md

## Recent Artifacts

- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic Parity Plan - Systematic plan updated to route next work from P2 decisions into Forge human/agent experience polish and release planning.
- patch-notes [active/durable]: CHANGELOG.md - Unreleased Patch Notes - Unreleased notes updated with Game Studio Depth, TEA Depth, and P2 scope decisions.
- runtime-polish [active/durable]: .forge-method/artifacts/20260615-guidance-human-experience-polish.md - Guidance human experience polish - Added contextual guide lede, runtime-builder routing for human-experience plus agent-doc polish, and quieter Reality/Evidence Gate behavior for correction/runtime requests.
- patch-notes [active/durable]: CHANGELOG.md - Unreleased Patch Notes - Unreleased notes updated with Guidance Engine human output polish, Game Studio Depth, TEA Depth, and P2 scope decisions.
- runtime-polish [active/durable]: .forge-method/artifacts/20260615-guidance-human-experience-polish.md - Guidance human experience polish - Guidance human experience polish completed with contextual guide lede, runtime-builder routing, quieter Reality/Evidence Gate behavior, and full validation.
