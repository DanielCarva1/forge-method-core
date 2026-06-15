# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: sprint-planning-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect dev-story mechanical autonomy and no-procedural-confirmation transcript gaps before claiming full guided-flow parity.

## Latest Checkpoint

# Sprint Planning Depth hardened

- created_at: 2026-06-15T05:48:47+00:00
- project: forge-method-core
- phase: 6-evolve
- status: sprint-planning-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the sprint-planning guidance gap: plan-sprint now has a compact source-aware state machine, dedicated sprint plan artifact template, sequence/rebalance/validate metadata, enriched story-lifecycle facilitation, Guidance Engine precedence over generic quality wording, and parity replay coverage.

## Decisions

- Sprint planning is not a backlog dump; it must preserve sprint goal, ordered story batch, decision-source map, validation/evidence plan, and deferred/blocked reasons before build.
- Explicit sprint planning intent outranks generic validation/quality wording.

## Checks

- python -m unittest discover -s tests: passed
- workflow validate: passed
- workflow compactness: passed
- parity replay: passed
- config validate --root .: passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- artifact verify --root .: passed
- audit --root .: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-plan-sprint.md
- skills/forge-method/facilitation/story-lifecycle.md
- skills/forge-method/templates/sprint-plan-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/design-thinking.md
- skills/forge-method/facilitation/innovation-strategy.md
- skills/forge-method/facilitation/storytelling.md
- skills/forge-method/templates/design-thinking-artifact.md
- skills/forge-method/templates/innovation-strategy-artifact.md
- skills/forge-method/templates/storytelling-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-043802-validation-brainstorming-depth-validation.md
- .forge-method/evidence/20260615-045622-validation-cis-facilitation-depth-validation.md
- .forge-method/evidence/20260615-051116-validation-agent-compactness-guard-validation.md
- .forge-method/evidence/20260615-052906-validation-story-decision-source-gate-validation.md
- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge systematic parity audit - Updated with Sprint Planning Depth: plan-sprint now has template metadata, modes, story-lifecycle facilitation depth, Guidance Engine precedence, and replay proof.
- internal-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated immediate progress with Sprint Planning Depth and clarified remaining transcript-hardening work.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated sprint planning parity target and fixture workflow list for plan-sprint.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated after Sprint Planning Depth to include plan-sprint template, modes, and outputs.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Added Unreleased note for Sprint Planning Depth.
