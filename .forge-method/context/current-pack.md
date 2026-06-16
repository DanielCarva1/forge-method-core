# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: workflow-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Audit runtime help/oracle output for the same safety boundary while preserving rich human guidance.

## Latest Checkpoint

# Workflow guidance safety guard

- created_at: 2026-06-16T01:58:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Added validation that blocks misleading compact workflow refs from relying on chat memory, stale state, procedural continue prompts, or catalog dumps.

## Decisions

- Agent-facing workflow refs now have line-level safety validation in addition to compact structure validation.

## Checks

- Focused workflow guidance safety and packaged workflow tests passed.
- Full unittest passed: 101 tests in 239.976s.
- workflow validate, workflow compactness, parity replay, verify-fast, smoke-runtime, and smoke-install passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/20260616-workflow-guidance-safety-guard.md

## Next Action

Audit runtime help/oracle output for the same safety boundary while preserving rich human guidance.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/facilitation/*.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-003551-validation-guidance-replay-test-optimization-validation.md
- .forge-method/evidence/20260616-005844-validation-guidance-loop-routing-and-test-optimization-vali.md
- .forge-method/evidence/20260616-011633-validation-guidance-cli-boundary-validation.md
- .forge-method/evidence/20260616-014233-validation-facilitation-specificity-guard-validation.md
- .forge-method/evidence/20260616-015842-validation-workflow-guidance-safety-guard-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Guidance CLI boundary changelog - Unreleased notes record the Guidance Engine CLI boundary test optimization: JSON contracts use direct runtime calls while guide subprocess coverage remains for human text and integration surfaces.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-facilitation-specificity-guard.md - Facilitation specificity guard - Human-facing facilitation packs now require domain_examples with at least three situational entries, preventing structurally valid but generic guidance from passing workflow validation.
- changelog [active/durable]: CHANGELOG.md - Facilitation specificity guard changelog - Unreleased notes record the facilitation specificity guard and domain_examples requirement for packaged human-facing packs.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-workflow-guidance-safety-guard.md - Workflow guidance safety guard - Compact workflow refs now fail validation if they instruct agents to rely on chat memory, follow stale state, ask procedural continue confirmations, or dump catalogs.
- changelog [active/durable]: CHANGELOG.md - Workflow guidance safety guard changelog - Unreleased notes record the workflow guidance safety guard for compact agent-facing workflow refs.
