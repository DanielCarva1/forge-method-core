# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: guidance-loop-tests-optimized
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by profiling the remaining targeted guide subprocess tests and deciding which must remain CLI coverage.

## Latest Checkpoint

# Guidance loop routing and tests optimized

- created_at: 2026-06-16T00:59:07+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-loop-tests-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Fixed the test-loop optimization prompt so it stays on runtime-builder instead of false-routing to skill-convert, added a parity replay regression fixture, and converted lifecycle/game/TEA guidance contract loops to direct runtime calls with direct replay state setup.

## Decisions

- Use direct runtime contracts for Guidance Engine matrix assertions; preserve CLI coverage in parity replay, smokes, config index, and focused human-output guide tests.

## Checks

- lifecycle guidance test passed in 8.495s; game studio guidance test passed in 2.327s; game dev mechanical route test passed in 0.357s; TEA guidance test passed in 1.778s; guidance fixture test passed; fixture family test passed; python -m unittest discover -s tests passed 99 tests in 244.008s; parity replay 91/91 passed; workflow validate passed; workflow compactness passed; verify-fast.ps1 passed with unittest at 205.593s

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-guidance-loop-routing-and-test-optimization.md

## Next Action

Continue post-parity Forge polish by profiling the remaining targeted guide subprocess tests and deciding which must remain CLI coverage.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/game-brief.md
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/references/workflow-game-brief.md
- skills/forge-method/references/workflow-game-sprint-planning.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- skills/forge-method/facilitation/test-architecture.md
- skills/forge-method/references/workflow-test-framework.md
- skills/forge-method/references/workflow-test-automation.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-224347-validation-game-artifact-generators-validation.md
- .forge-method/evidence/20260615-233832-validation-test-utility-generators-validation.md
- .forge-method/evidence/20260616-001949-validation-document-and-enterprise-generators-validation.md
- .forge-method/evidence/20260616-003551-validation-guidance-replay-test-optimization-validation.md
- .forge-method/evidence/20260616-005844-validation-guidance-loop-routing-and-test-optimization-vali.md

## Recent Artifacts

- runtime-contract [active/durable]: .forge-method/artifacts/20260616-doc-enterprise-generators-contract.md - Document and enterprise generators contract - First-class artifact doc-index, doc-shard, enterprise-track-map, enterprise-readiness, and enterprise-release-gate generators for document freshness and enterprise gate handoffs with source/install smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - document and enterprise generators - Unreleased notes updated with artifact doc-index, artifact doc-shard, artifact enterprise-track-map, artifact enterprise-readiness, artifact enterprise-release-gate, document/lifecycle handoffs, tests, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-replay-test-optimization.md - Guidance replay test optimization - Optimized Guidance Engine replay fixture testing by using the runtime replay contract directly, preserving 90-case parity coverage while cutting the slow transcript fixture test from minutes to seconds.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guidance replay optimization - Unreleased notes updated with Guidance Engine parity fixture test optimization preserving 90-case coverage while reducing replay test runtime.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-loop-routing-and-test-optimization.md - Guidance loop routing and test optimization - Fixed skill-convert false-positive routing for test-loop optimization wording and converted lifecycle/game/TEA guidance contract loops to direct runtime calls while preserving CLI coverage.
