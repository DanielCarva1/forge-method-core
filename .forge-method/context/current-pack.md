# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: help-oracle-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Audit remaining agent-facing runtime surfaces for stale-route safety without flattening human-facing guidance.

## Latest Checkpoint

# help-oracle-guidance-safety-guard-final

- created_at: 2026-06-16T02:18:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: help-oracle-guidance-safety-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Help Oracle guidance safety guard increment with clean artifact verification, audit, and gate after registering changelog and runtime-contract artifacts.

## Decisions

- Keep the next P2 gap focused on remaining agent-facing runtime surfaces, not on example projects.

## Checks

- python -m unittest discover -s tests passed: 104 tests
- verify-fast, smoke-runtime, and smoke-install passed
- artifact verify, audit, and gate --require-evals passed cleanly

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-help-oracle-guidance-safety-guard.md
- .forge-method/evidence/20260616-021756-validation-help-oracle-guidance-safety-guard-final-gate.md

## Next Action

Audit remaining agent-facing runtime surfaces for stale-route safety without flattening human-facing guidance.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- tests/test_runtime.py
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/*.md
- CHANGELOG.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-011633-validation-guidance-cli-boundary-validation.md
- .forge-method/evidence/20260616-014233-validation-facilitation-specificity-guard-validation.md
- .forge-method/evidence/20260616-015842-validation-workflow-guidance-safety-guard-validation.md
- .forge-method/evidence/20260616-021614-validation-help-oracle-guidance-safety-guard-validation.md
- .forge-method/evidence/20260616-021756-validation-help-oracle-guidance-safety-guard-final-gate.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Facilitation specificity guard changelog - Unreleased notes record the facilitation specificity guard and domain_examples requirement for packaged human-facing packs.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-workflow-guidance-safety-guard.md - Workflow guidance safety guard - Compact workflow refs now fail validation if they instruct agents to rely on chat memory, follow stale state, ask procedural continue confirmations, or dump catalogs.
- changelog [active/durable]: CHANGELOG.md - Workflow guidance safety guard changelog - Unreleased notes record the workflow guidance safety guard for compact agent-facing workflow refs.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-help-oracle-guidance-safety-guard.md - Help Oracle guidance safety guard - Runtime Help Oracle and audit output now share the misleading-guidance safety contract with compact workflow refs.
- changelog [active/durable]: CHANGELOG.md - Help Oracle guidance safety guard changelog - Unreleased notes record the Help Oracle guidance safety guard for runtime resume, snapshot, and audit output.
