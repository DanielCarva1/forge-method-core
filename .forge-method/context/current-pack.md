# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p1-builder-factory-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P1.2 Customization and Capability Index from the systematic parity plan.

## Latest Checkpoint

# P1.1 Builder Factory closed

- created_at: 2026-06-14T23:12:53+00:00
- project: forge-method-core
- phase: 6-evolve
- status: systematic-parity-plan-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Builder Factory parity batch. Added five narrow builder workflows, shared human facilitation, plan/manifest/validation templates, Guidance Engine routes, replay fixtures, benchmark/audit/plan updates, and installed validation. Next batch is P1.2 Customization and Capability Index.

## Decisions

- Builder Factory is the canonical Forge term for the guided creation/validation family.
- Analysis/conversion remain builder-utility; creation/package/whole-module validation route to builder-factory workflows.

## Checks

- python -m unittest discover -s tests
- workflow validate
- builder validate
- parity replay
- smoke-runtime.ps1
- verify-fast.ps1
- smoke-install.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/catalog/workflows.json
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/builder-factory.md
- skills/forge-method/references/workflow-module-ideation.md
- skills/forge-method/references/workflow-agent-builder.md
- skills/forge-method/references/workflow-workflow-builder.md
- skills/forge-method/references/workflow-module-builder.md
- skills/forge-method/references/workflow-module-validate.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- CONTEXT.md
- .forge-method/artifacts/20260614-builder-factory-grill.md

## Artifacts

- none

## Next Action

Implement P1.2 Customization and Capability Index from the systematic parity plan.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-product-requirements.md
- skills/forge-method/references/workflow-ux-plan.md
- skills/forge-method/references/workflow-quick-dev.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/facilitation/ux-design.md
- skills/forge-method/facilitation/quick-dev.md
- skills/forge-method/templates/*product*|*ux*|*quick*
- tests/fixtures/guidance_transcripts.json
- tests/test_runtime.py
- skills/forge-method/references/workflow-story-creation.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-204726-validation-prd-ux-quick-dev-parity-validation.md
- .forge-method/evidence/20260612-211014-validation-story-lifecycle-guard-validation.md
- .forge-method/evidence/20260613-024610-validation-parity-replay-harness-validation.md
- .forge-method/evidence/20260613-031940-planning-systematic-parity-plan-validation.md
- .forge-method/evidence/20260614-231253-validation-p1-1-builder-factory-validation.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Updated parity audit to mark P1.1 Builder Factory rows translated while preserving P1.2+ as remaining parity work.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated systematic parity plan after P1.1 Builder Factory implementation; next planned batch is P1.2 Customization and Capability Index.
- grill-with-docs [active/durable]: .forge-method/artifacts/20260614-builder-factory-grill.md - Builder Factory grill with docs - Grill closed P1.1 architecture decisions: single entrypoint, Builder Factory glossary term, rich human pack, compact agent workflows, templates, routes, and validation proof.
- correct-course [active/durable]: .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md - Correct-course continuation - Method-experience correction artifact preserving the decision to route method failures through correct-course before runtime-builder repair.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated systematic parity plan after P1.1 Builder Factory validation; next planned batch is P1.2 Customization and Capability Index.
