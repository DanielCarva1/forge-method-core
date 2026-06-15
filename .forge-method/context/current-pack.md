# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: story-decision-source-gate-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; run completion audit and live transcript review before claiming full guided-flow parity.

## Latest Checkpoint

# Story Decision Source Gate hardened

- created_at: 2026-06-15T05:29:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-decision-source-gate-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the epics/stories decision-source invariant gap. Story add/import/start now prevents implementation-ready build stories without approved source artifacts, autoattaches a single clear source, requires --source when several artifacts could justify different stories, persists decision_sources, and audit verifies the source map before build-story.

## Decisions

- Stories are not a substitute for accepted decisions; build-ready stories must carry explicit decision_sources.
- Automation can continue only after the source map is durable; ambiguous sources require explicit selection.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- workflow compactness: passed
- parity replay: 63/63 passed
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
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-sys
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-context-recovery.md
- skills/forge-method/facilitation/context-boundary.md
- skills/forge-method/templates/context-recovery-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-041351-validation-context-boundary-recovery-validation.md
- .forge-method/evidence/20260615-043802-validation-brainstorming-depth-validation.md
- .forge-method/evidence/20260615-045622-validation-cis-facilitation-depth-validation.md
- .forge-method/evidence/20260615-051116-validation-agent-compactness-guard-validation.md
- .forge-method/evidence/20260615-052906-validation-story-decision-source-gate-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge systematic parity audit - Updated with Story Decision Source Gate: build-ready stories now require approved explicit decision sources before mechanical build.
- internal-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated immediate progress with Story Decision Source Gate and clarified remaining transcript-hardening work.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Added story decision-source benchmark target for explicit source maps before build-story.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated after Story Decision Source Gate.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Added Unreleased note for Story Decision Source Gate.
