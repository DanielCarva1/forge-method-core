# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: workflow-snapshot-surface-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.

## Latest Checkpoint

# Workflow snapshot surface guard

- created_at: 2026-06-16T06:06:33+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-snapshot-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the workflow snapshot visibility gap: snapshot quality now exposes workflow validation errors that workflow validate and gate already consume.

## Decisions

- Use workflow_validation_errors as the shared workflow validation surface for snapshot quality as well as gate and workflow validate.

## Checks

- focused workflow snapshot regression: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md

## Next Action

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-workflow-catalog-gate-surface-guard.md
- CHANGELOG.md
- .forge-method/artifacts/20260616-agent-validation-gate-surface-guard.md
- .forge-method/artifacts/20260616-builder-extension-gate-surface-guard.md
- .forge-method/context/capability-index.json
- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md
- .forge-method/evidence/20260616-032451-validation-state-guidance-write-guard-validation.md
- .forge-method/evidence/20260616-034215-validation-recovery-memory-guidance-guard-validation.md
- .forge-method/evidence/20260616-041434-validation-durable-runtime-guidance-source-guard-validation.md
- .forge-method/evidence/20260616-043253-validation-product-facing-docs-independence-guard-validatio.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Capability index gate surface guard changelog - Unreleased notes record that written capability indexes now feed config validation, snapshot quality, and gate.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for gate surface guard - Regenerated compact capability index after adding written capability-index validation through config validate, snapshot quality, and gate.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md - Capability index gate surface guard - Config validate, snapshot quality, builder validation, and quality gate now consume the written capability-index validation surface for stale or misleading compact agent contracts and final validation evidence.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md - Workflow snapshot surface guard - Snapshot quality now exposes workflow validation errors so agents can see workflow, catalog, facilitation, and template failures before relying on compact runtime state.
- changelog [active/durable]: CHANGELOG.md - Workflow snapshot surface guard changelog - Unreleased notes record that workflow validation errors now appear in snapshot quality.
