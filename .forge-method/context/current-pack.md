# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: council-orchestration-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining strong-ish rows; next inspect correct-course breadth, problem-solving depth, and any game/code-review examples that still feel generic before claiming full guided-flow parity.

## Latest Checkpoint

# Council Orchestration Depth hardened

- created_at: 2026-06-15T06:57:40+00:00
- project: forge-method-core
- phase: 6-evolve
- status: council-orchestration-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed party-mode and subagent-orchestration parity gaps: council-decision now has a dedicated pack/template/modes, natural Guidance Engine routing, richer live debate rounds, compact dissent/orchestration artifact contract, JSON worker/merge plan, catalog/fixture/test coverage, regenerated capability index, and validation evidence.

## Decisions

- Council is Human Experience first: show useful live specialist debate to the human, but persist only compact decision, dissent, evidence, worker-output, merge, and next-action contracts.
- Subagent/parallel mode changes orchestration style only; artifact contracts stay stable and the runtime falls back to sequential council when real subagents are unavailable or outputs are not independent.

## Checks

- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py workflow validate
- python skills/forge-method/scripts/forge_method_runtime.py workflow compactness
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- python skills/forge-method/scripts/forge_method_runtime.py config validate --root .
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-council-decision
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md
- skills/forge-method/references/workflow-plan-sprint.md
- skills/forge-method/templates/sprint-plan-artifact.md
- skills/forge-method/catalog/workflows.json

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-052906-validation-story-decision-source-gate-validation.md
- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md
- .forge-method/evidence/20260615-060843-validation-build-story-autonomy-depth-validation.md
- .forge-method/evidence/20260615-063437-validation-document-review-depth-validation.md
- .forge-method/evidence/20260615-065658-validation-council-orchestration-depth-validation.md

## Recent Artifacts

- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - Council Orchestration Depth audit update - Systematic parity audit now marks party-mode and subagent orchestration translated with dedicated council routing, rich live debate, compact worker/merge contracts, and fallback policy.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Council Orchestration Depth plan update - Systematic parity plan now records Council Orchestration Depth as completed post-1.29 hardening and moves next focus to remaining strong-ish transcript rows.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Council Orchestration Depth benchmark update - Guidance Engine benchmark now includes party-mode/council/subagent orchestration behavior and council-decision fixture workflow id.
- config [active/durable]: .forge-method/context/capability-index.json - Capability index regenerated for Council Orchestration Depth - Capability index regenerated after council-decision pack/template/mode metadata changes.
- changelog [active/durable]: CHANGELOG.md - Council Orchestration Depth changelog note - Unreleased changelog records the council guidance/runtime increment.
