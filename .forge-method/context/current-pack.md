# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p0-story-lifecycle-guard-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P0.5 Parity replay harness from the BMAD parity audit.

## Latest Checkpoint

# Story lifecycle guard closed

- created_at: 2026-06-12T21:10:40+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-story-lifecycle-guard-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.4: Forge now has a story-creation workflow, story-flow Guidance Engine routing, a story lifecycle facilitation pack upgrade, and an audit guard that blocks implementation-ready build stories without accepted decision-source artifacts. This does not complete full BMAD parity; next P0 is a parity replay harness.

## Decisions

- Treat stories as execution artifacts generated from accepted decisions, not as substitutes for PRD/spec/UX/architecture/test/validation decisions.
- Route story lifecycle requests through Guidance Engine story-flow to story-creation, readiness-check, create-epics, or plan-sprint before build-story.
- Keep rich human story facilitation in facilitation/story-lifecycle.md and compact agent contract in references/workflow-story-creation.md.

## Checks

- python -m unittest discover -s tests: passed 62 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py audit --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed with only pre-existing correct-course stale-summary warning
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
- installed forge-method guide story-flow route: passed

## Failed Checks

- none

## Touched Files

- skills/fo
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- install.ps1
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/*.md
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/references/workflow-product-requirements.md
- skills/forge-method/references/workflow-ux-plan.md
- skills/forge-method/references/workflow-quick-dev.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/facilitation/ux-design.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md
- .forge-method/evidence/20260612-200602-audit-bmad-forge-systematic-parity-audit.md
- .forge-method/evidence/20260612-203044-validation-help-oracle-and-facilitation-coverage-validation.md
- .forge-method/evidence/20260612-204726-validation-prd-ux-quick-dev-parity-validation.md
- .forge-method/evidence/20260612-211014-validation-story-lifecycle-guard-validation.md

## Recent Artifacts

- research-reference [archived-reference/durable]: .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md - Independent app research and experiment reference - Preserves the TS/Rust/hooks experiment findings, records that Codex instability is outside the Forge plugin boundary, and defers independent app work to a future Rust-core product track.
- correct-course [active/durable]: .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md - Correct-course continuation - Forge human guidance treated a critique of the method experience as generic builder work, and new project creation could seed ready stories before facilitated discovery.

Impact: New users could receive technical artifacts and stories before taste, pain, theme, UX, or route facilitation, then get procedural confirmations instead of guided or autonomous progress.

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Keep initial projects gated by facilitation input, route method-experience criticism to correct-course first, and validate with transcript fixtures plus runtime smoke.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Systematic first-pass parity audit comparing BMAD Method, Builder, CIS, Game Dev Studio, and TEA against Forge principles, workflows, facilitation packs, runtime contracts, scripts, state, and validation.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include product requirements, UX planning, and quick-dev routing expectations alongside correct-course, research, brainstorm, game, builder, document, and quality routes.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include story lifecycle routing expectations: story-creation/readiness flows require decision-source maps, validation maps, and mechanical loops without procedural continue prompts.
