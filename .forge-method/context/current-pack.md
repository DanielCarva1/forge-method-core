# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>
- next_action: Publish or commit Forge Method Core 1.33.0 after reviewing diff.

## Latest Checkpoint

# Forge 1.33.0 MDA Lens and manual update

- created_at: 2026-06-20T21:25:56+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>

## Summary

Implemented MDA Lens/MDA Trace for Game Studio and forge-update manual maintenance skill. Validation passed: test-runner 139/139, smoke-runtime, smoke-install, verify-fast, eval run 24/24, gate passed with stale-summary warnings only.

## Decisions

- MDA Lens is integrated into existing Game Studio workflows, not a separate workflow.
- forge-update is an Operational Maintenance Skill and does not mutate project progress.

## Checks

- python scripts\test-runner.py --workers 4 --timeout 120 --report .forge-method\test-runs\manual.json
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals --summary Forge 1.33.0 MDA Lens and forge-update validation passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/scripts/forge_method_updater.py
- skills/forge-update/SKILL.md
- skills/forge-method/facilitation/game-brief.md
- skills/forge-method/references/workflow-game-brief.md

## Artifacts

- .forge-method/artifacts/20260620-mda-game-lens-and-manual-update-work-order.md
- release-notes/1.33.0.md

## Next Action

Publish or commit Forge Method Core 1.33.0 after reviewing diff.

## Recovery Signals

### Failed Checks

- Legacy direct `python -m unittest discover -s tests` timed out during this work. Replaced in verification scripts with `scripts/test-runner.py`, which preserves coverage while adding progress, per-test timeouts, and slow-test reporting.

### Touched Files

- CHANGELOG.md
- scripts/test-runner.py
- scripts/verify-all.ps1
- scripts/verify-all.sh
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- skills/forge-guideline-auditor/**
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/guideline-audit.md
- skills/forge-method/modules/runtime-builder.yaml
- skills/forge-method/references/workflow-guideline-audit.md
- skills/forge-method/scripts/forge_method_runtime.py

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-232617-publication-v1-31-2-guided-research-drift-hotfix-published.md
- .forge-method/evidence/20260617-235258-documentation-guideline-auditor-integrated.md
- .forge-method/evidence/20260618-013448-smart-test-suite-observability.md
- .forge-method/evidence/20260620-193531-gate-quality-gate.md
- .forge-method/evidence/20260620-212517-gate-quality-gate.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Forge Guideline Auditor changelog - Unreleased notes record Forge Guideline Auditor, guideline-audit routing, work-order fields, and regression coverage.
- evidence [active/durable]: .forge-method/evidence/20260618-013448-smart-test-suite-observability.md - Smart test suite observability - Debug/report/JUnit runner observability added and validated with full responsive unit run.
- correct-course [active/durable]: .forge-method/artifacts/20260620-181432-correct-course-human-guidance-coverage-for-platform-ops-and-vis.md - Human guidance coverage for platform ops and visual alignment - User feedback showed Forge still under-guided infra, CI/CD, database, deploy, observability, and early visible prototype alignment during initial product shaping.

Impact: Without first-class routes, agents could skip operational architecture and let user-facing products reach build without enough visual proof for humans to correct course..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Use platform-ops-plan and visual-alignment-prototype as guided workflows before build when the human intent or project surface requires them..
- correct-course [active/durable]: .forge-method/artifacts/20260620-193511-correct-course-early-visual-proof-as-initial-stage-cadence.md - Early visual proof as initial-stage cadence - User clarified that visual alignment/prototype preview must be part of all early product shaping stages, not only an explicit route when the user asks for mockups.

Impact: Without a recurring early visual proof loop, agents can guide conversation and write requirements while the human still has no visible product direction to accept or correct..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Keep early_visual_proof active for initial product, game, UX, creative, and brainstorm workflows; route accepted visuals into requirements and mismatches into UX/product/correct-course before stories or build..
- runtime-builder [active/durable]: .forge-method/artifacts/20260620-mda-game-lens-and-manual-update-work-order.md - MDA Game Lens And Manual Update Work Order - Work order for Forge Method 1.33.0: add MDA Lens and MDA Trace to Game Studio guidance/artifacts, and add forge-update as an operational maintenance skill for manual updates.
