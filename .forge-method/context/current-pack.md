# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-human-prompt-route-specificity-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing workflow-specific first-question quality and mechanical-build human/status wording.

## Latest Checkpoint

# Replay human prompt route specificity hardened

- created_at: 2026-06-15T16:39:49+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-human-prompt-route-specificity-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added Guidance Engine normalization so facilitated routes ask a concrete human first question, remove internal I-should phrasing, and append compact Signals/Route summaries for agent handoff.

## Decisions

- Guided human prompts are part of the runtime contract, not decorative copy; parity replay must fail when facilitated guidance reads like internal agent notes.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: cases 90, facilitated 88, missing_first_question 0, internal_i_should 0, missing_signals_route 0
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
- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md

## Next Action

Continue post-parity Forge polish by auditing workflow-specific first-question quality and mechanical-build human/status wording.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md
- .forge-method/state.yaml

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-142530-validation-replay-persona-lens-contract-final-validation.md
- .forge-method/evidence/20260615-145721-validation-replay-auxiliary-guidance-contract-validation.md
- .forge-method/evidence/20260615-152414-validation-replay-mutating-command-contract-validation.md
- .forge-method/evidence/20260615-160212-validation-replay-state-update-route-reason-contract-valida.md
- .forge-method/evidence/20260615-163924-validation-replay-human-prompt-route-specificity-contract-v.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - mutating command replay assertions - Documented expected_commands replay contract for multiple state-changing Guidance Engine commands.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md - Replay State Update Route Reason Contract - Parity replay now validates compact state_update handoff coherence and Persona Lens route_reason markers.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - state update route reason replay assertions - Documented replay checks for state update handoff coherence and Persona Lens route reason markers.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md - Replay Human Prompt Route Specificity Contract - Parity replay now validates facilitated human prompts and compact signal/route reason summaries.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - human prompt route specificity replay assertions - Documented replay checks for facilitated human prompts and signal/route reason summaries.
