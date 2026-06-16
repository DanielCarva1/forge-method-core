# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: facilitation-specificity-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Audit compact workflow refs for misleading agent guidance and stale next-step language; add validation or replay proof before changing prose.

## Latest Checkpoint

# Facilitation specificity guard

- created_at: 2026-06-16T01:42:34+00:00
- project: forge-method-core
- phase: 6-evolve
- status: facilitation-specificity-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Added a machine-checkable domain_examples specificity guard for all human-facing facilitation packs and filled the remaining packs with situational examples.

## Decisions

- Human guidance specificity is now enforced through workflow validation; compact workflow refs remain unchanged.

## Checks

- Focused tests passed for generic pack rejection and packaged workflow validation.
- Full unittest passed: 100 tests in 177.966s.
- workflow validate, workflow compactness, parity replay, smoke-runtime, verify-fast, and smoke-install passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/*.md
- tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/20260616-facilitation-specificity-guard.md

## Next Action

Audit compact workflow refs for misleading agent guidance and stale next-step language; add validation or replay proof before changing prose.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/document-utility.md
- skills/forge-method/facilitation/lifecycle-closure.md
- skills/forge-method/references/workflow-doc-index.md
- skills/forge-method/references/workflow-doc-shard.md
- skills/forge-method/references/workflow-track-decision.md
- skills/forge-method/references/workflow-readiness-check.md
- skills/forge-method/references/workflow-release-readiness.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-001949-validation-document-and-enterprise-generators-validation.md
- .forge-method/evidence/20260616-003551-validation-guidance-replay-test-optimization-validation.md
- .forge-method/evidence/20260616-005844-validation-guidance-loop-routing-and-test-optimization-vali.md
- .forge-method/evidence/20260616-011633-validation-guidance-cli-boundary-validation.md
- .forge-method/evidence/20260616-014233-validation-facilitation-specificity-guard-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guidance loop routing and test optimization - Unreleased notes updated with skill-convert false-positive routing fix and lifecycle/game/TEA guidance contract loop optimization.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-cli-boundary-test-optimization.md - Guidance CLI boundary test optimization - Converted JSON-only Guidance Engine assertions to direct runtime calls while preserving guide subprocess coverage for human text, empty-workspace, config/tracks, and mechanical CLI behavior.
- changelog [active/durable]: CHANGELOG.md - Guidance CLI boundary changelog - Unreleased notes record the Guidance Engine CLI boundary test optimization: JSON contracts use direct runtime calls while guide subprocess coverage remains for human text and integration surfaces.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-facilitation-specificity-guard.md - Facilitation specificity guard - Human-facing facilitation packs now require domain_examples with at least three situational entries, preventing structurally valid but generic guidance from passing workflow validation.
- changelog [active/durable]: CHANGELOG.md - Facilitation specificity guard changelog - Unreleased notes record the facilitation specificity guard and domain_examples requirement for packaged human-facing packs.
