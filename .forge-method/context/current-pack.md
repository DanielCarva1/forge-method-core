# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: game-brief-sprint-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Decide whether /cis-agent-presentation-master becomes a Forge workflow, folds into storytelling/presentation craft, or is explicitly deferred; keep deferred API/browser or eval-runner surfaces out unless repeated projects justify them.

## Latest Checkpoint

# Game Brief Sprint Depth finalized

- created_at: 2026-06-15T11:51:38+00:00
- project: forge-method-core
- phase: 6-evolve
- status: game-brief-sprint-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

State finalized after Game Brief & Sprint Depth hardening. Runtime now routes game brief as a living guided workflow, uses dedicated game-sprint-planning for playable-slice planning, validates both with artifact game-check, and proves the guidance matrix with 88/88 parity replay plus full repo validation.

## Decisions

- Residual game brief, brainstorm-game, and game sprint planning rows are no longer treated as strong-ish; next parity decision is /cis-agent-presentation-master or explicit deferral.

## Checks

- python -m unittest discover -s tests => 78 tests OK
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 => passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 => passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 => passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md

## Next Action

Decide whether /cis-agent-presentation-master becomes a Forge workflow, folds into storytelling/presentation craft, or is explicitly deferred; keep deferred API/browser or eval-runner surfaces out unless repeated projects justify them.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references
- skills/forge-method/facilitation
- skills/forge-method/templates
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/templates/spec-kernel-artifact.md
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/templates/research-scan-artifact.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-093459-validation-e2e-test-automation-depth-validation.md
- .forge-method/evidence/20260615-101549-validation-enterprise-artifact-map-depth-validation.md
- .forge-method/evidence/20260615-103921-validation-spec-kernel-depth-validation.md
- .forge-method/evidence/20260615-110943-validation-research-guidance-depth-validation.md
- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md

## Recent Artifacts

- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed - Regenerated capability index after adding game-sprint-planning workflow and game brief/sprint templates.
- changelog [active/durable]: CHANGELOG.md - Game Brief Sprint Depth changelog - Recorded the unreleased Game Brief & Sprint Depth increment.
- parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - Game Brief Sprint Depth audit update - Marked residual game brainstorm, game brief, and game sprint planning rows translated after living brief, game-sprint-planning, game-check, and replay proof.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance benchmark updated for game sprint planning - Added game-sprint-planning to internal Guidance Engine benchmark targets and fixture workflow IDs.
- parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan updated - Updated next focus after closing residual game brief, brainstorm-game, and game sprint planning rows.
