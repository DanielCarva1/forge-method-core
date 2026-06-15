# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: brainstorming-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; run completion audit and live transcript review before claiming full guided-flow parity.

## Latest Checkpoint

# Brainstorming Depth hardened

- created_at: 2026-06-15T04:38:02+00:00
- project: forge-method-core
- phase: 6-evolve
- status: brainstorming-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Brainstorming Depth parity gap. Brainstorming now has guided divergence, taste and anti-reference prompts, pressure testing, discard pile, selection criteria, compact template, catalog modes, replay proof, and install/runtime validation.

## Decisions

- Option-generation language outranks generic confusion so broad ideas receive guided divergence before PRD or architecture.
- Taste-heavy creative direction still routes to creative-session before generic brainstorming.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- parity replay: 61/61 passed
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
- skills/forge-method/facilitation/brainstorming.md
- skills/forge-method/templates/brainstorming-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Co
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/state.yaml
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- CHANGELOG.md
- skills/forge-method/references/workflow-architecture.md
- skills/forge-method/facilitation/architecture-planning.md
- skills/forge-method/templates/architecture-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- .forge-method/artifacts/guidance-engine-benchmark.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md
- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md
- .forge-method/evidence/20260615-035510-validation-architecture-guidance-depth-validation.md
- .forge-method/evidence/20260615-041351-validation-context-boundary-recovery-validation.md
- .forge-method/evidence/20260615-043802-validation-brainstorming-depth-validation.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Audit updated after Brainstorming Depth: brainstorming and CIS brainstorm rows now reflect guided divergence/convergence pack, compact template, modes, and replay proof.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Plan updated with Brainstorming Depth and remaining transcript hardening work.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Benchmark updated so option-generation language routes to brainstorming before generic confusion while taste-heavy creative requests remain creative-session.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Brainstorming Depth workflow template, modes, outputs, and followed-by metadata.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method changelog - Unreleased notes updated with Brainstorming Depth.
