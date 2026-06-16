# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: workflow-selected
- workflow: config-customization
- active_story: <none>
- next_action: Continue the broader Forge audit by finding other composed runtime-visible payloads that need final deterministic validation before emission.

## Latest Checkpoint

# Config capability index guidance safety guard

- created_at: 2026-06-16T03:03:17+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-selected
- workflow: config-customization
- active_story: <none>

## Summary

Closed a config-customization audit gap: config validation, agent profile validation, and config index now apply guidance safety to runtime-visible text before future agents consume conventions, custom capabilities, agent summaries, or generated capability indexes.

## Decisions

- Treat composed capability index output as a runtime guidance payload and validate it before print/write.

## Checks

- unittest 108; smoke-runtime; verify-fast; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20; smoke-install

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md

## Artifacts

- .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md

## Next Action

Continue the broader Forge audit by finding other composed runtime-visible payloads that need final deterministic validation before emission.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-021614-validation-help-oracle-guidance-safety-guard-validation.md
- .forge-method/evidence/20260616-021756-validation-help-oracle-guidance-safety-guard-final-gate.md
- .forge-method/evidence/20260616-023621-validation-runtime-guidance-payload-safety-guard-validation.md
- .forge-method/evidence/20260616-023724-validation-runtime-guidance-payload-safety-guard-final-gate.md
- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Help Oracle guidance safety guard changelog - Unreleased notes record the Help Oracle guidance safety guard for runtime resume, snapshot, and audit output.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-runtime-guidance-payload-safety-guard.md - Runtime guidance payload safety guard - Guidance Engine parity payloads, preflight JSON, reload JSON, and guide payloads now share the stale-route safety contract.
- changelog [active/durable]: CHANGELOG.md - Runtime guidance payload safety guard changelog - Unreleased notes record the Runtime Guidance Payload safety guard for Guidance Engine, preflight, reload, and guide JSON output.
- runtime-audit [active/durable]: .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md - Config capability index guidance safety - Config validation, agent profile validation, and config index now reject misleading runtime-visible guidance text before future agents can consume it.
- changelog [active/durable]: CHANGELOG.md - Config capability index guidance safety changelog - Unreleased notes record the config, agent profile, and capability index guidance safety guard.
