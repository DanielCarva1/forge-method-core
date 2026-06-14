# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p1-customization-index-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P1.3 Persona Layer from the systematic parity plan.

## Latest Checkpoint

# Checkpoint

- created_at: 2026-06-14T23:38:30+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p1-customization-index-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed P1.2 Customization and Capability Index. Added validated Project Configuration overrides, config-customization workflow pack/template, Capability Index generation, Guidance Engine route/replay case, ADR/glossary updates, and tests for valid and stale overrides.

## Decisions

- Project Configuration is the canonical override surface with packaged defaults < team config < local config precedence.
- Capability Index is generated from effective metadata instead of manually maintained.

## Checks

- python -m unittest discover -s tests
- workflow validate; builder validate; config validate; config index --write --json; parity replay; audit; artifact verify
- smoke-runtime.ps1; verify-fast.ps1; smoke-install.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/config-customization.md
- skills/forge-method/references/workflow-config-customization.md
- skills/forge-method/templates/config-customization-artifact.md
- tests/test_runtime.py
- docs/adr/0009-project-configuration-overrides.md

## Artifacts

- .forge-method/artifacts/20260614-customization-capability-index-grill.md

## Next Action

Implement P1.3 Persona Layer from the systematic parity plan.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
- skills/forge-method/templates/story-creation-artifact.md
- tests/test_runtime.py
- tests/fixtures/guidance_transcripts.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- scripts/smoke-install.ps1
- scripts/smoke-install.sh
- docs/00-quickstart.md
- docs/05-v1-operating-model.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-211014-validation-story-lifecycle-guard-validation.md
- .forge-method/evidence/20260613-024610-validation-parity-replay-harness-validation.md
- .forge-method/evidence/20260613-031940-planning-systematic-parity-plan-validation.md
- .forge-method/evidence/20260614-231253-validation-p1-1-builder-factory-validation.md
- .forge-method/evidence/20260614-233818-validation-p1-2-customization-and-capability-index-validati.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Updated parity audit to mark P1.1 Builder Factory rows translated while preserving P1.2+ as remaining parity work.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated systematic parity plan after P1.1 Builder Factory implementation; next planned batch is P1.2 Customization and Capability Index.
- grill-with-docs [active/durable]: .forge-method/artifacts/20260614-builder-factory-grill.md - Builder Factory grill with docs - Grill closed P1.1 architecture decisions: single entrypoint, Builder Factory glossary term, rich human pack, compact agent workflows, templates, routes, and validation proof.
- correct-course [active/durable]: .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md - Correct-course continuation - Method-experience correction artifact preserving the decision to route method failures through correct-course before runtime-builder repair.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated systematic parity plan after P1.1 Builder Factory validation; next planned batch is P1.2 Customization and Capability Index.
