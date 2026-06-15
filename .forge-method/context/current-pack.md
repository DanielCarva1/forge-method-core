# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p1-parity-closure-utilities-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Review the Unreleased changelog as one coherent version batch, then decide tag/publish versus real-use transcript hardening.

## Latest Checkpoint

# P1.7 parity closure utilities closed

- created_at: 2026-06-15T02:34:58+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p1-parity-closure-utilities-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed P1.7 Parity Closure Utilities. Added investigation, working-backwards-challenge, sprint-status, checkpoint-preview, and adversarial-review as routeable Forge workflows with compact refs, templates, catalog/module membership, Guidance Engine routes, parity replay fixtures, refreshed Capability Index, and adversarial routing precedence.

## Decisions

- Use compact workflow refs/templates for agent handoff and keep human richness in existing facilitation packs plus guide output.
- Explicit adversarial/red-team requests outrank generic quality review when the document router detects assumption attack.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- workflow validate: passed
- parity replay: 58/58 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed; installed parity replay 58/58
- artifact verify: passed
- audit: passed
- config validate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts

- .forge-method/evidence/20260615-023334-validation-p
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/test-architecture.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- .forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md
- CHANGELOG.md
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-010242-validation-p1-5-game-studio-depth-validation.md
- .forge-method/evidence/20260615-013149-validation-p1-6-test-architecture-enterprise-depth-validati.md
- .forge-method/evidence/20260615-013605-planning-p2-scope-decisions-recorded.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md
- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md

## Recent Artifacts

- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine Benchmark - Internal behavior benchmark updated with Parity Closure Utilities: investigation, working-backwards challenge, sprint status, adversarial review, and checkpoint preview routes.
- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge Systematic Parity Audit - Parity audit updated with P1.7 closure utilities and stale guidance markers for quick-dev, story-creation, PRFAQ, investigation, sprint-status, checkpoint-preview, and adversarial-review.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic Parity Plan - Systematic plan updated with P1.7 Parity Closure Utilities and next release/version validation path.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Parity Closure Utility workflows, templates, and module membership.
- patch-notes [active/durable]: CHANGELOG.md - Unreleased Patch Notes - Unreleased notes updated with Parity Closure Utilities plus Guidance Engine human polish, Game Studio Depth, TEA Depth, and P2 scope decisions.
