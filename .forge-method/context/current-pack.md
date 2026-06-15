# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-auxiliary-guidance-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing remaining replay surfaces for human guidance, compact artifact handoff, and automation flags that still pass on route-only evidence.

## Latest Checkpoint

# Replay auxiliary guidance contract hardened

- created_at: 2026-06-15T14:57:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-auxiliary-guidance-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added replay assertions for council recommendations, Codex Goal handoff, and autonomous work-order flags; narrowed council recommendation behavior; and protected runtime meta-audit prompts from council keyword routing.

## Decisions

- Route-only success is no longer enough for auxiliary guidance behavior; replay fixtures must declare behavior-changing handoff flags.

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

- .forge-method/artifacts/20260615-replay-auxiliary-guidance-contract.md

## Next Action

Continue post-parity Forge polish by auditing remaining replay surfaces for human guidance, compact artifact handoff, and automation flags that still pass on route-only evidence.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260615-replay-facilitation-contract.md
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

- .forge-method/evidence/20260615-131108-validation-replay-facilitation-contract-validation.md
- .forge-method/evidence/20260615-132818-validation-replay-template-contract-validation.md
- .forge-method/evidence/20260615-141222-validation-replay-persona-lens-contract-validation.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md
- .forge-method/evidence/20260615-145721-validation-replay-auxiliary-guidance-contract-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the replay template contract requiring expected_template assertions for human-facing guided parity cases.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-persona-lens-contract.md - Replay Persona Lens Contract - Persona lens replay contract now requires explicit expected_persona_lens assertions and fixes alias precedence for architecture, QA, and problem-solving signals.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the Replay Persona Lens Contract and alias precedence hardening.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-auxiliary-guidance-contract.md - Replay Auxiliary Guidance Contract - Parity replay now asserts council, Codex Goal handoff, and autonomous work-order flags, and keeps runtime meta-audit wording on runtime-builder.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - auxiliary replay assertions - Documented replay assertions for council, Codex Goal handoff, autonomous work orders, and runtime-builder meta-audit routing.
