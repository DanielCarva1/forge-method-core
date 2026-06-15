# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-persona-lens-contract-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by checking remaining automation, council, and persona handoff assertions; do not count route-only success as parity.

## Latest Checkpoint

# Replay Persona Lens Contract finalized

- created_at: 2026-06-15T14:25:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-persona-lens-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Persona Lens replay assertions and alias scoring are finalized after the raw-token guard: generic words like strategist/designer no longer select QA/UX by accident, while explicit QA/UX and test-framework signals still route correctly.

## Decisions

- Persona ID and alias subset scoring must preserve short role tokens such as qa and ux, so generic words do not hijack the human guidance lens.

## Checks

- python -m unittest discover -s tests: passed (82 tests)
- python skills\\forge-method\\scripts\\forge_method_runtime.py parity replay: passed (89/89)
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py gate --root . --require-evals: passed (9/9 evals)

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/personas/overlays.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-replay-persona-lens-contract.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md

## Next Action

Continue post-parity Forge polish by checking remaining automation, council
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- skills/forge-method/fixtures/guidance-parity-replay.json
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

- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md
- .forge-method/evidence/20260615-131108-validation-replay-facilitation-contract-validation.md
- .forge-method/evidence/20260615-132818-validation-replay-template-contract-validation.md
- .forge-method/evidence/20260615-141222-validation-replay-persona-lens-contract-validation.md
- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md

## Recent Artifacts

- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-facilitation-contract.md - Replay Facilitation Contract - Strengthened parity replay so human-facing guided cases must assert expected facilitation packs, preventing route-only passes from hiding rich human guidance regressions.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-template-contract.md - Replay Template Contract - Strengthened parity replay so human-facing guided cases must assert expected artifact templates when catalog workflows define them, protecting compact agent handoff artifacts.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the replay template contract requiring expected_template assertions for human-facing guided parity cases.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-persona-lens-contract.md - Replay Persona Lens Contract - Persona lens replay contract now requires explicit expected_persona_lens assertions and fixes alias precedence for architecture, QA, and problem-solving signals.
- changelog [active/durable]: CHANGELOG.md - CHANGELOG - Unreleased notes include the Replay Persona Lens Contract and alias precedence hardening.
