# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: guidance-replay-test-optimized
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by profiling remaining subprocess-heavy guide loops and deciding which should stay CLI coverage versus direct runtime contract tests.

## Latest Checkpoint

# Guidance replay test optimized

- created_at: 2026-06-16T00:36:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-replay-test-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Optimized Guidance Engine replay fixture testing by using the runtime replay contract directly instead of spawning guide --json per fixture, preserving 90-case parity assertions while cutting the slow transcript replay test from minutes to seconds.

## Decisions

- Use direct runtime calls for parity fixture matrix tests when the behavior under test is Guidance Engine routing, and keep CLI coverage in parity replay, smokes, and focused guide output tests.

## Checks

- fixture replay test passed in 6.351s; python -m unittest discover -s tests passed 99 tests in 259.381s; parity replay 90/90 passed; workflow validate passed; workflow compactness passed; verify-fast.ps1 passed in 217.6s wall time

## Failed Checks

- none

## Touched Files

- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-guidance-replay-test-optimization.md

## Next Action

Continue post-parity Forge polish by profiling remaining subprocess-heavy guide loops and deciding which should stay CLI coverage versus direct runtime contract tests.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/references/workflow-market-scan.md
- skills/forge-method/references/workflow-domain-scan.md
- skills/forge-method/references/workflow-technical-feasibility-scan.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- skills/forge-method/facilitation/game-brief.md
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/references/workflow-game-brief.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-220809-validation-research-scan-generator-validation.md
- .forge-method/evidence/20260615-224347-validation-game-artifact-generators-validation.md
- .forge-method/evidence/20260615-233832-validation-test-utility-generators-validation.md
- .forge-method/evidence/20260616-001949-validation-document-and-enterprise-generators-validation.md
- .forge-method/evidence/20260616-003551-validation-guidance-replay-test-optimization-validation.md

## Recent Artifacts

- runtime-contract [active/durable]: .forge-method/artifacts/20260615-test-utility-generators-contract.md - Test utility generators contract - First-class artifact test-framework, test-automation, and game-e2e-scaffold generators for quality and playable smoke handoffs with source/install smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - test utility generators - Unreleased notes updated with artifact test-framework, artifact test-automation, artifact game-e2e-scaffold, Test Architecture/game lifecycle handoffs, tests, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-doc-enterprise-generators-contract.md - Document and enterprise generators contract - First-class artifact doc-index, doc-shard, enterprise-track-map, enterprise-readiness, and enterprise-release-gate generators for document freshness and enterprise gate handoffs with source/install smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - document and enterprise generators - Unreleased notes updated with artifact doc-index, artifact doc-shard, artifact enterprise-track-map, artifact enterprise-readiness, artifact enterprise-release-gate, document/lifecycle handoffs, tests, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-replay-test-optimization.md - Guidance replay test optimization - Optimized Guidance Engine replay fixture testing by using the runtime replay contract directly, preserving 90-case parity coverage while cutting the slow transcript fixture test from minutes to seconds.
