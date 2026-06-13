# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p0-parity-replay-harness-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P1.1 Builder parity from the BMAD parity audit: module ideation, agent builder, workflow builder, module builder, and module validation.

## Latest Checkpoint

# Parity replay harness closed

- created_at: 2026-06-13T02:46:34+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-parity-replay-harness-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.5: Forge now ships a packaged parity replay harness inside the forge-method skill. The replay covers help, confusion, brainstorm, research, PRD, UX, architecture, quick-dev, story cycle, correct-course, builder, CIS/creative, game, and TEA-shaped guidance prompts, expecting Forge-native workflow/phase/action outputs. Install smoke now runs the installed replay fixture. Full parity goal remains active; next work is P1.1 Builder parity.

## Decisions

- Make the skill-packaged fixture the canonical transcript matrix so source tests and installed smoke exercise the same guidance routes.
- Use Forge-native expected outputs only; benchmark family labels are internal coverage metadata, not public product language.
- Keep P0 closure reflected in the internal parity audit while preserving P1/P2 as unfinished work.

## Checks

- python skills\\forge-method\\scripts\\forge_method_runtime.py parity replay: passed 20/20 cases
- python -m unittest discover -s tests: passed 64 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py audit --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed with only pre-existing correct-course stale-summary warning
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\s
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/*.md
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/references/workflow-product-requirements.md
- skills/forge-method/references/workflow-ux-plan.md
- skills/forge-method/references/workflow-quick-dev.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/facilitation/ux-design.md
- skills/forge-method/facilitation/quick-dev.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-200602-audit-bmad-forge-systematic-parity-audit.md
- .forge-method/evidence/20260612-203044-validation-help-oracle-and-facilitation-coverage-validation.md
- .forge-method/evidence/20260612-204726-validation-prd-ux-quick-dev-parity-validation.md
- .forge-method/evidence/20260612-211014-validation-story-lifecycle-guard-validation.md
- .forge-method/evidence/20260613-024610-validation-parity-replay-harness-validation.md

## Recent Artifacts

- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include product requirements, UX planning, and quick-dev routing expectations alongside correct-course, research, brainstorm, game, builder, document, and quality routes.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include story lifecycle routing expectations: story-creation/readiness flows require decision-source maps, validation maps, and mechanical loops without procedural continue prompts.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include architecture and CIS/creative parity replay expectations, backed by packaged parity replay fixtures.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Updated parity audit current status: P0.1-P0.5 are implemented and validated, while P1 builder/customization/persona/game/TEA depth remains the next parity work.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include architecture and CIS/creative parity replay expectations, backed by packaged parity replay fixtures.
