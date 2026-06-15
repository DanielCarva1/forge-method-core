# Replay State Update Route Reason Contract

## Problem

The parity replay matrix already proved route, phase, workflow, facilitation pack, template, Persona Lens, auxiliary flags, and mutating commands.
It still allowed a weaker compact-agent handoff to pass when `state_updates` drifted from the actual Guidance Engine decision, or when a Persona Lens route did not preserve the selected lens inside `guidance_engine.route_reason`.

The concrete gap was the Design Thinking Coach path: it returned the correct Persona Lens payload, but the first `persona-lens` branch did not include the standard `Persona lens selected: <id>` marker in the route reason.

## Contract

- `guidance_engine.route_reason` must be non-empty for every parity replay case.
- `state_updates.last_intent_classification` must equal `intent_classification`.
- `state_updates.active_guidance_mode` must equal `recommended_workflow`.
- `state_updates.last_route_reason` must equal `guidance_engine.route_reason`.
- When a Persona Lens is returned, `route_reason` must include `Persona lens selected: <id>`.

## Runtime Change

- Added the missing Persona Lens route-reason marker for the `operate-support -> persona-lens` branch.
- Hardened `parity_case_failures` so replay fixtures fail on state-update handoff drift and missing Persona Lens route-reason markers.
- Included `route_reason` and `state_updates` in parity replay actual output for easier audit/debug.

## Proof

- Targeted replay-contract tests passed.
- Parity replay passed: 90/90.
- Manual replay audit found:
  - `missing_persona_route_reason_markers []`
  - `state_update_coherence_issues []`
- Full runtime tests passed: 89 tests.
- `smoke-runtime`, `verify-fast`, `smoke-install`, `artifact verify`, and installed parity replay passed.

## Next Audit Thread

Continue post-parity polish by auditing human prompt quality and route-reason specificity against the rich-human compact-agent contract.
