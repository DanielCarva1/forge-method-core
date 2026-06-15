# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-state-update-route-reason-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing human_prompt quality and route_reason specificity against the rich-human compact-agent contract.

## Latest Checkpoint

# Replay state update route reason contract hardened

- created_at: 2026-06-15T16:02:36+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-state-update-route-reason-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added parity replay checks that state_updates mirror classification, workflow, and route_reason, and that Persona Lens route reasons persist the selected lens marker for compact agent handoff.

## Decisions

- Guidance replay must prove compact state-update handoff coherence, not just route and phase.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: missing_persona_route_reason_markers [] and state_update_coherence_issues []
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md

## Next Action

Continue post-parity Forge polish by auditing human_prompt quality and route_reason specificity against the rich-human compact-agent contract.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md
- .forge-method/state.yaml

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-141222-validation-replay-persona-lens-contract-validation.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md
- .forge-method/evidence/20260615-145721-validation-replay-auxiliary-guidance-contract-validation.md
- .forge-method/evidence/20260615-152414-validation-replay-mutating-command-contract-validation.md
- .forge-method/evidence/20260615-160212-validation-replay-state-update-route-reason-contract-valida.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - auxiliary replay assertions - Documented replay assertions for council, Codex Goal handoff, autonomous work orders, and runtime-builder meta-audit routing.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-mutating-command-contract.md - Replay Mutating Command Contract - Parity replay now requires full mutating command sequences when guidance returns multiple state-changing commands.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - mutating command replay assertions - Documented expected_commands replay contract for multiple state-changing Guidance Engine commands.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md - Replay State Update Route Reason Contract - Parity replay now validates compact state_update handoff coherence and Persona Lens route_reason markers.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - state update route reason replay assertions - Documented replay checks for state update handoff coherence and Persona Lens route reason markers.
