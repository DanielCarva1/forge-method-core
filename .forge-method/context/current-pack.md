# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: document-review-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect party-mode/council and subagent orchestration gaps before claiming full guided-flow parity.

## Latest Checkpoint

# Document Review Depth hardened

- created_at: 2026-06-15T06:35:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: document-review-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the editorial-review and edge-case hunter guidance gap: added specialized templates, richer document-utility facilitation, catalog modes, Guidance Engine routing/precedence, replay fixtures, benchmark/audit/plan/changelog updates, regenerated capability index, and validation evidence.

## Decisions

- Specialized document review outranks generic quality review only for explicit document-review intents or non-quality edge/adversarial/editorial wording; strong ATDD/test/QA/CI intent stays in quality-flow.

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
- skills/forge-method/facilitation/document-utility.md
- skills/forge-method/references/workflow-editorial-review.md
- skills/forge-method/references/workflow-edge-case-review.md
- skills/forge-method/templates/editorial-review-artifact.md
- skills/forge-method/templates/edge-case-review-artifact.md
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- scripts/smoke-runtime.ps1
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
- skills/forge-method/references/workflow-plan-sprint.md
- skills/forge-method/templates/sprint-plan-artifact.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-051116-validation-agent-compactness-guard-validation.md
- .forge-method/evidence/20260615-052906-validation-story-decision-source-gate-validation.md
- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md
- .forge-method/evidence/20260615-060843-validation-build-story-autonomy-depth-validation.md
- .forge-method/evidence/20260615-063437-validation-document-review-depth-validation.md

## Recent Artifacts

- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - Document Review Depth audit update - Systematic parity audit now marks editorial review and edge-case hunter rows translated with specialized routing, templates, modes, and replay proof.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Document Review Depth plan update - Systematic parity plan now records Document Review Depth as completed post-1.29 hardening and sets party-mode/subagent orchestration as the next focus.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Document Review Depth benchmark update - Guidance Engine benchmark now includes editorial review and edge-case review target behaviors and fixture workflow ids.
- config [active/durable]: .forge-method/context/capability-index.json - Capability index regenerated for Document Review Depth - Capability index regenerated after catalog/template/mode changes for editorial-review and edge-case-review.
- changelog [active/durable]: CHANGELOG.md - Document Review Depth changelog note - Unreleased changelog records the document review guidance/runtime increment.
