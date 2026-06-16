# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: guidance-cli-boundary-optimized
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue improving Forge human guidance depth and agent compactness; keep remaining guide subprocess checks as CLI proof unless replacement coverage is equivalent.

## Latest Checkpoint

# Guidance CLI boundary optimized

- created_at: 2026-06-16T01:16:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-cli-boundary-optimized
- workflow: runtime-builder
- active_story: <none>

## Summary

Converted remaining JSON-only Guidance Engine assertions to direct runtime calls and documented which guide subprocess checks remain intentional CLI coverage.

## Decisions

- JSON contracts are tested through build_guide_payload; human text and integration surfaces keep guide subprocess coverage.

## Checks

- Focused tests passed for Reality Gate, human lede, lifecycle closure, mechanical work order, and project create guidance.
- python -m unittest discover -s tests passed: 99 tests in 250.728s.
- verify-fast.ps1 passed: unittest, onboarding assets, workflow validation, and agent profile validation.

## Failed Checks

- none

## Touched Files

- tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/20260616-guidance-cli-boundary-test-optimization.md

## Next Action

Continue improving Forge human guidance depth and agent compactness; keep remaining guide subprocess checks as CLI proof unless replacement coverage is equivalent.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/test-architecture.md
- skills/forge-method/facilitation/game-lifecycle.md
- skills/forge-method/references/workflow-test-framework.md
- skills/forge-method/references/workflow-test-automation.md
- skills/forge-method/references/workflow-game-e2e-scaffold.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- skills/forge-method/facilitation/document-utility.md
- skills/forge-method/facilitation/lifecycle-closure.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-233832-validation-test-utility-generators-validation.md
- .forge-method/evidence/20260616-001949-validation-document-and-enterprise-generators-validation.md
- .forge-method/evidence/20260616-003551-validation-guidance-replay-test-optimization-validation.md
- .forge-method/evidence/20260616-005844-validation-guidance-loop-routing-and-test-optimization-vali.md
- .forge-method/evidence/20260616-011633-validation-guidance-cli-boundary-validation.md

## Recent Artifacts

- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-replay-test-optimization.md - Guidance replay test optimization - Optimized Guidance Engine replay fixture testing by using the runtime replay contract directly, preserving 90-case parity coverage while cutting the slow transcript fixture test from minutes to seconds.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guidance replay optimization - Unreleased notes updated with Guidance Engine parity fixture test optimization preserving 90-case coverage while reducing replay test runtime.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-loop-routing-and-test-optimization.md - Guidance loop routing and test optimization - Fixed skill-convert false-positive routing for test-loop optimization wording and converted lifecycle/game/TEA guidance contract loops to direct runtime calls while preserving CLI coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guidance loop routing and test optimization - Unreleased notes updated with skill-convert false-positive routing fix and lifecycle/game/TEA guidance contract loop optimization.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-guidance-cli-boundary-test-optimization.md - Guidance CLI boundary test optimization - Converted JSON-only Guidance Engine assertions to direct runtime calls while preserving guide subprocess coverage for human text, empty-workspace, config/tracks, and mechanical CLI behavior.
