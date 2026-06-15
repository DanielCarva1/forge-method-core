# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue residual parity hardening; prioritize research and game-brief strong-ish rows where transcript evidence still shows drift.

## Latest Checkpoint

# Spec Kernel Depth hardened

- created_at: 2026-06-15T10:39:45+00:00
- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the generic spec-kernel gap by turning write-spec into the Forge-native compact WHAT contract: spec-kernel template, create/update/distill/validate modes, stable CAP ID and intent/success rules, companion/source map, decision log, preservation map, validation verdict, artifact spec-check, Guidance Engine routing, replay proof, and product-planning facilitation depth.

## Decisions

- write-spec is the lean spec-kernel workflow; product-requirements remains the richer PRD/addendum workflow.
- Spec-kernel requests outrank document-utility distillation when the human asks for create/update/validate/distill spec, SPEC.md, stable capabilities, or machine contract.
- Spec kernels must preserve load-bearing source claims in the kernel, companions, adopted sources, or open questions; silent drops are invalid.

## Checks

- Targeted unittest: 6 tests OK
- Parity replay: 83/83 passed
- Workflow validate, compactness, and config validate passed
- python -m unittest discover -s tests: 76 tests OK
- smoke-runtime.ps1, smoke-install.ps1, and verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/templates/spec-kernel-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-103921-validation-spec-kernel-depth
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- Guidance Engine routing, workflow catalog, runtime-builder module, builder facilitation, module builder/validate workflows, distribution template, benchmark/audit docs, and runtime tests.
- Guidance Engine document routing, artifact doc-check runtime command, doc-index/doc-shard workflows, document-utility pack/template, catalog modes, replay fixtures, benchmark/audit/plan/changelog, and runtime tests.
- Guidance Engine quality routing, artifact test-check runtime command, test framework/automation/game E2E workflows and templates, game/test facilitation packs, workflow catalog, replay fixture, benchmark/audit/plan/changelog, runtime tests, capability index
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references
- skills/forge-method/facilitation
- skills/forge-method/templates
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-083039-validation-module-distribution-depth-validation.md
- .forge-method/evidence/20260615-090424-validation-document-utility-freshness-validation.md
- .forge-method/evidence/20260615-093459-validation-e2e-test-automation-depth-validation.md
- .forge-method/evidence/20260615-101549-validation-enterprise-artifact-map-depth-validation.md
- .forge-method/evidence/20260615-103921-validation-spec-kernel-depth-validation.md

## Recent Artifacts

- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark spec kernel target - Updated benchmark target to require write-spec routing for create/update/validate/distill spec requests and a compact spec kernel with stable capabilities, constraints, non-goals, success signal, preservation map, companions, decision log, and spec-check proof.
- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD parity audit spec kernel closure - Updated systematic parity audit to mark the generic spec token as translated through Forge write-spec, quick-dev, and artifact spec-check, while keeping residual strong-ish research/game-brief proof open.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan spec kernel update - Updated parity plan current status and next focus after Spec Kernel Depth, with remaining focus on research and game-brief strong-ish transcript proof.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index after Spec Kernel Depth - Regenerated capability index after adding write-spec spec-kernel template/modes and artifact spec-check command coverage.
- changelog [active/durable]: CHANGELOG.md - Changelog Spec Kernel Depth - Recorded Unreleased changelog entry for Spec Kernel Depth, including artifact spec-check, write-spec template/modes, preservation-map and stable capability ID contract, and replay coverage.
