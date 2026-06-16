# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: route-diagnostics-recovery-index
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge audit with the next Guidance Engine-selected gap; persisted route diagnostics in recovery and capability index are complete.

## Latest Checkpoint

# Route Diagnostics Recovery Index

- created_at: 2026-06-16T12:31:05+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-selected
- workflow: config-customization
- active_story: <none>

## Summary

Recovery briefs and capability index now persist Help Oracle route diagnostics, including required workflow, reason, context boundary, stale-state guard, and route surfaces.

## Decisions

- Keep route diagnostics as compact runtime surfaces in recovery artifacts and generated capability index, not as chat-only guidance.

## Checks

- python -m unittest discover -s tests: 125 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- artifact verify: passed
- audit: passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/context/capability-index.json

## Artifacts

- .forge-method/artifacts/20260616-route-diagnostics-recovery-index.md
- .forge-method/evidence/20260616-122951-validation-route-diagnostics-recovery-index.md

## Next Action

Continue post-parity Forge audit with the next Guidance Engine-selected gap; persisted route diagnostics in recovery and capability index are complete.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/state.yaml
- .forge-method/context/capability-index.json

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-094923-validation-bootstrap-quality-surface-validation.md
- .forge-method/evidence/20260616-103204-validation-reload-quality-surface-validation.md
- .forge-method/evidence/20260616-110005-validation-context-recovery-quality-surface-validation.md
- .forge-method/evidence/20260616-112855-validation-next-help-oracle-surface-validation.md
- .forge-method/evidence/20260616-122951-validation-route-diagnostics-recovery-index.md

## Recent Artifacts

- runtime-builder [active/durable]: .forge-method/artifacts/20260616-next-help-oracle-surface.md - Next Help Oracle surface - next --json now preserves compact Help Oracle route diagnostics, quality, commands, context boundary, and mechanical goal handoff.
- changelog [active/durable]: CHANGELOG.md - Next Help Oracle surface changelog - Unreleased notes record next --json and route diagnostics in text next.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-route-diagnostics-recovery-index.md - Route Diagnostics Recovery Index - Recovery briefs and capability index now preserve Help Oracle route diagnostics for future agents after reload or context recovery.
- changelog [active/durable]: CHANGELOG.md - Route Diagnostics Recovery Index Changelog - Unreleased notes record persisted route diagnostics in recovery briefs and capability index.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for Route Diagnostics Recovery Index - Regenerated compact capability index with route_diagnostics surfaces for guide, resume, next, and context recovery.
