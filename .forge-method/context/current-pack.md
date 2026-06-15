# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: agent-compactness-guard-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; run completion audit and live transcript review before claiming full guided-flow parity.

## Latest Checkpoint

# Agent Compactness Guard hardened

- created_at: 2026-06-15T05:11:16+00:00
- project: forge-method-core
- phase: 6-evolve
- status: agent-compactness-guard-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the progressive-disclosure audit row. Workflow refs now have compactness limits, forbidden human-pack sections, root-section checks, and heading checks; facilitation packs have shape and size checks; workflow compactness, workflow validate, audit, smoke-runtime, and unit tests prove the split between compact agent docs and rich human packs.

## Decisions

- Progressive disclosure must be deterministic: agent workflow refs stay compact state machines, while human richness lives in facilitation packs.
- The guard should fail normal validation and audit when the layers blur, not depend on review taste.

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
- scripts/smoke-runtime.ps1
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benc
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-architecture.md
- skills/forge-method/facilitation/architecture-planning.md
- skills/forge-method/templates/architecture-artifact.md
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

- .forge-method/evidence/20260615-035510-validation-architecture-guidance-depth-validation.md
- .forge-method/evidence/20260615-041351-validation-context-boundary-recovery-validation.md
- .forge-method/evidence/20260615-043802-validation-brainstorming-depth-validation.md
- .forge-method/evidence/20260615-045622-validation-cis-facilitation-depth-validation.md
- .forge-method/evidence/20260615-051116-validation-agent-compactness-guard-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge systematic parity audit - Updated with Agent Compactness Guard: progressive disclosure is now enforced by workflow compactness, workflow validate, smoke runtime, and audit.
- internal-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated immediate progress with Agent Compactness Guard and clarified remaining transcript-hardening work.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Added progressive disclosure benchmark target for compact workflow refs and rich facilitation packs.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated after adding workflow compactness guard.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Added Unreleased note for Agent Compactness Guard.
