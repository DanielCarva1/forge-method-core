# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: capability-index-gate-surface-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.

## Latest Checkpoint

# Capability index gate surface guard

- created_at: 2026-06-16T06:00:00+00:00
- project: forge-method-core
- phase: 6-evolve
- status: capability-index-gate-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the written capability-index validation surface gap: stale or misleading compact agent capability contracts now fail config validate, snapshot quality, builder validate, and gate while config index --write remains the repair path.

## Decisions

- Use capability_index_validation_errors for the written compact agent capability contract, regenerate the repo capability index, and keep config index --write based on override validation so stale files can be repaired.

## Checks

- focused capability-index regression: passed
- related config/builder regression tests: passed
- python -m unittest discover -s tests: 122 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- audit/artifact verify/workflow validate/agent validate/config validate/builder validate: passed
- parity replay: 91/91 passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/context/capability-index.json
- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- .forge-method/context/capability-index.json

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
- .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md
- .forge-method/context/capability-index.json

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

- runtime-contract [active/durable]: .forge-method/artifacts/20260616-builder-extension-gate-surface-guard.md - Builder extension gate surface guard - Builder validate, snapshots, and quality gate now consume the same local builder extension validation surface for project-local skill frontmatter and final validation evidence.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md - Capability index gate surface guard - Config validate, snapshot quality, and quality gate now consume the written capability-index validation surface for stale or misleading compact agent contracts.
- changelog [active/durable]: CHANGELOG.md - Capability index gate surface guard changelog - Unreleased notes record that written capability indexes now feed config validation, snapshot quality, and gate.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for gate surface guard - Regenerated compact capability index after adding written capability-index validation through config validate, snapshot quality, and gate.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-capability-index-gate-surface-guard.md - Capability index gate surface guard - Config validate, snapshot quality, builder validation, and quality gate now consume the written capability-index validation surface for stale or misleading compact agent contracts and final validation evidence.
