# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: stale-guidance-guard-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish with transcript-derived improvements only; keep artifact verify clean and avoid reopening closed parity rows without a failing transcript.

## Latest Checkpoint

# Stale Guidance Guard hardened

- created_at: 2026-06-15T12:50:22+00:00
- project: forge-method-core
- phase: 6-evolve
- status: stale-guidance-guard-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Post-parity polish audit found structurally healthy packs/refs and stale internal guidance as the main agentic risk. Added Stale Guidance Guard to artifact verification, cleaned active parity audit/plan wording, recorded a durable polish audit, and validated source plus installed runtime.

## Decisions

- Guard active parity/audit/plan/benchmark artifacts against stale closed-work markers instead of relying on future agents to notice contradictions manually.

## Checks

- artifact verify --root .: passed
- workflow validate: passed
- workflow compactness: passed
- parity replay: 89/89 passed
- python -m unittest discover -s tests: 79 tests OK
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md

## Artifacts

- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md

## Next Action

Continue post-parity Forge polish with transcript-derived improvements only; keep artifact verify clean and avoid reopening closed parity rows without a failing transcript.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/templates/spec-kernel-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/templates/research-scan-artifact.md
- skills/forge-method/personas/overlays.json
- skills/forge-method/facilitation/storytelling.md
- skills/forge-method/references/workflow-storytelling.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-103921-validation-spec-kernel-depth-validation.md
- .forge-method/evidence/20260615-110943-validation-research-guidance-depth-validation.md
- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md
- .forge-method/evidence/20260615-122252-validation-presentation-craft-fold-in-validation.md
- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md

## Recent Artifacts

- parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge Systematic Parity Audit - Cleaned stale mixed-verdict guidance after post-parity audit: Planning Tracks is translated, old missing-pack notes are replaced with current coverage, and future work is transcript-derived polish.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic Parity Plan - Updated immediate next step to post-parity Forge polish and Stale Guidance Guard work instead of old partial-row batches already closed by current evidence.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include Stale Guidance Guard and post-parity audit cleanup behavior.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-post-parity-polish-audit.md - Post-Parity Polish Audit - Audited facilitation packs, compact workflow refs, and active guidance artifacts; added Stale Guidance Guard without storing forbidden stale markers in active guidance text.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-post-parity-polish-audit.md - Post-Parity Polish Audit - Audited facilitation packs, compact workflow refs, and active guidance artifacts; added Stale Guidance Guard and documented current post-parity polish without stale marker text.
