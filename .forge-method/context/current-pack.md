# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: state-guidance-write-guard
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the broader Forge audit by finding runtime outputs that compose durable user/project data with agent guidance and need final deterministic validation before emission.

## Latest Checkpoint

# State guidance write guard

- created_at: 2026-06-16T03:25:20+00:00
- project: forge-method-core
- phase: 6-evolve
- status: state-guidance-write-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed a runtime-builder audit gap: guidance-bearing state fields now pass the same misleading-guidance safety contract before write_state persists them, and audit catches preexisting contaminated state.

## Decisions

- Treat next_action, last_route_reason, and guide_summary as durable runtime guidance; validate them at write time and audit time while leaving IDs and project metadata outside the prose scan.

## Checks

- unittest 110; smoke-runtime; verify-fast; smoke-install; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-state-guidance-write-guard.md

## Artifacts

- .forge-method/artifacts/20260616-state-guidance-write-guard.md

## Next Action

Continue the broader Forge audit by finding runtime outputs that compose durable user/project data with agent guidance and need final deterministic validation before emission.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-state-guidance-write-guard.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-021756-validation-help-oracle-guidance-safety-guard-final-gate.md
- .forge-method/evidence/20260616-023621-validation-runtime-guidance-payload-safety-guard-validation.md
- .forge-method/evidence/20260616-023724-validation-runtime-guidance-payload-safety-guard-final-gate.md
- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md
- .forge-method/evidence/20260616-032451-validation-state-guidance-write-guard-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Runtime guidance payload safety guard changelog - Unreleased notes record the Runtime Guidance Payload safety guard for Guidance Engine, preflight, reload, and guide JSON output.
- runtime-audit [active/durable]: .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md - Config capability index guidance safety - Config validation, agent profile validation, and config index now reject misleading runtime-visible guidance text before future agents can consume it.
- changelog [active/durable]: CHANGELOG.md - Config capability index guidance safety changelog - Unreleased notes record the config, agent profile, and capability index guidance safety guard.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-state-guidance-write-guard.md - State guidance write guard - State next-action and route-reason fields are now validated before write and during audit so misleading durable guidance cannot become future agent context.
- changelog [active/durable]: CHANGELOG.md - State guidance write guard changelog - Unreleased notes record the state guidance write guard for durable next-action and route-reason fields.
