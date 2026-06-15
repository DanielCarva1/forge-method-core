# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: build-story-autonomy-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect editorial/edge-case/party-mode human guidance gaps before claiming full guided-flow parity.

## Latest Checkpoint

# Build Story Autonomy Depth hardened

- created_at: 2026-06-15T06:09:15+00:00
- project: forge-method-core
- phase: 6-evolve
- status: build-story-autonomy-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the dev-story mechanical-autonomy gap: build-story now has a compact workflow contract, structured work-order template, catalog modes, full mechanical command map, JSON loop/do_not_prompt fields, compact recovery priority protection, and Codex Goal handoff that forbids procedural ok/continue prompts.

## Decisions

- Mechanical story loops must expose the full start/resume -> implement -> check -> review -> evidence -> done -> next-story/ready-gate contract in JSON, not only prose.
- Compact recovery must prioritize Read First over long command maps so fresh chats stay usable under small context budgets.

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
- skills/forge-method/references/workflow-build-story.md
- skills/forge-method/templates/build-story-work-order.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifa
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

- .forge-method/evidence/20260615-045622-validation-cis-facilitation-depth-validation.md
- .forge-method/evidence/20260615-051116-validation-agent-compactness-guard-validation.md
- .forge-method/evidence/20260615-052906-validation-story-decision-source-gate-validation.md
- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md
- .forge-method/evidence/20260615-060843-validation-build-story-autonomy-depth-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD Forge systematic parity audit - Updated with Build Story Autonomy Depth: build-story now has structured work order, loop, command map, no-procedural-prompt contract, and replay proof.
- internal-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Updated immediate progress with Build Story Autonomy Depth and clarified remaining transcript-hardening work.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated mechanical build benchmark target for structured work orders and no-procedural-prompt autonomy.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated after Build Story Autonomy Depth to include build-story template, modes, and outputs.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Added Unreleased note for Build Story Autonomy Depth.
