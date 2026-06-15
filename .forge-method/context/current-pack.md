# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-mutating-command-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing remaining replay surfaces for state update contents, route reasons, and human prompt quality that still pass on indirect evidence.

## Latest Checkpoint

# Replay mutating command contract hardened

- created_at: 2026-06-15T15:25:03+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-mutating-command-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added expected_commands replay support, exact mutating command sequence validation, and replay output for mutating_commands so multi-command correct-course routes cannot pass by asserting only one state-changing command.

## Decisions

- Guidance replay must prove the full state-changing command sequence, not just one command presence.

## Checks

- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- .forge-method/artifacts/20260615-replay-mutating-command-contract.md

## Next Action

Continue post-parity Forge polish by auditing remaining replay surfaces for state update contents, route reasons, and human prompt quality that still pass on indirect evidence.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260615-replay-template-contract.md
- skills/forge-method/personas/overlays.json
- CHANGELOG.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-132818-validation-replay-template-contract-validation.md
- .forge-method/evidence/20260615-141222-validation-replay-persona-lens-contract-validation.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md
- .forge-method/evidence/20260615-145721-validation-replay-auxiliary-guidance-contract-validation.md
- .forge-method/evidence/20260615-152414-validation-replay-mutating-command-contract-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the Replay Persona Lens Contract and alias precedence hardening.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-auxiliary-guidance-contract.md - Replay Auxiliary Guidance Contract - Parity replay now asserts council, Codex Goal handoff, and autonomous work-order flags, and keeps runtime meta-audit wording on runtime-builder.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - auxiliary replay assertions - Documented replay assertions for council, Codex Goal handoff, autonomous work orders, and runtime-builder meta-audit routing.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-mutating-command-contract.md - Replay Mutating Command Contract - Parity replay now requires full mutating command sequences when guidance returns multiple state-changing commands.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - mutating command replay assertions - Documented expected_commands replay contract for multiple state-changing Guidance Engine commands.
