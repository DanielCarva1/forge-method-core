# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: correct-course-problem-solving-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue residual parity transcript hardening: inspect game dev-story/review examples, package/distribution depth, doc utility validation, and deferred API/browser or eval-runner surfaces only if repeated projects justify them.

## Latest Checkpoint

# Correct Course and Problem Solving Depth hardened

- created_at: 2026-06-15T07:28:21+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-problem-solving-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the remaining human-guidance depth gap for correction and stuck/problem-solving flows: correct-course and problem-solving now have compact templates, catalog modes, richer facilitation packs, stronger Guidance Engine signals/text, and replay fixtures covering scope, human-experience, implementation contradiction, and messy constraints.

## Decisions

- Keep rich human recovery guidance in facilitation packs and guide output while compact agent contracts live in workflow refs, catalog metadata, templates, state, and replay fixtures.

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
- skills/forge-method/facilitation/correct-course.md
- skills/forge-method/facilitation/problem-solving.md
- skills/forge-method/templates/correct-course-artifact.md
- skills/forge-method/templates/problem-solving-artifact.md
- skills/fo
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

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

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md
- .forge-method/evidence/20260615-060843-validation-build-story-autonomy-depth-validation.md
- .forge-method/evidence/20260615-063437-validation-document-review-depth-validation.md
- .forge-method/evidence/20260615-065658-validation-council-orchestration-depth-validation.md
- .forge-method/evidence/20260615-072752-validation-correct-course-and-problem-solving-depth-validat.md

## Recent Artifacts

- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - Council Orchestration Depth audit update - Systematic parity audit now marks party-mode and subagent orchestration translated with dedicated council routing, rich live debate, compact worker/merge contracts, and fallback policy.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Council Orchestration Depth plan update - Systematic parity plan now records Council Orchestration Depth as completed post-1.29 hardening and moves next focus to remaining strong-ish transcript rows.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Council Orchestration Depth benchmark update - Guidance Engine benchmark now includes party-mode/council/subagent orchestration behavior and council-decision fixture workflow id.
- config [active/durable]: .forge-method/context/capability-index.json - Capability index regenerated for Council Orchestration Depth - Capability index regenerated after council-decision pack/template/mode metadata changes.
- changelog [active/durable]: CHANGELOG.md - Council Orchestration Depth changelog note - Unreleased changelog records the council guidance/runtime increment.
