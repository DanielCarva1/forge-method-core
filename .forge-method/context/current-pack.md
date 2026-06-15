# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: replay-workflow-first-question-mechanical-status-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing live CLI guide output shape against richer prompt contracts and non-JSON first-question visibility.

## Latest Checkpoint

# Replay workflow first question mechanical status hardened

- created_at: 2026-06-15T17:03:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-workflow-first-question-mechanical-status-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added workflow-specific first questions for guided replay coverage and mechanical-build status prompts that describe autonomous build/check/evidence work instead of asking facilitation questions.

## Decisions

- Workflow-specific first questions are a runtime contract for rich human guidance; mechanical-build is status/execution handoff, not facilitation.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: unique_first_questions 67, cross_workflow_repeats [], mechanical prompt issues []
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
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md

## Next Action

Continue post-parity Forge polish by auditing live CLI guide output shape against richer prompt contracts and non-JSON first-question visibility.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/state.yaml
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-145721-validation-replay-auxiliary-guidance-contract-validation.md
- .forge-method/evidence/20260615-152414-validation-replay-mutating-command-contract-validation.md
- .forge-method/evidence/20260615-160212-validation-replay-state-update-route-reason-contract-valida.md
- .forge-method/evidence/20260615-163924-validation-replay-human-prompt-route-specificity-contract-v.md
- .forge-method/evidence/20260615-170307-validation-replay-workflow-first-question-mechanical-status.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - state update route reason replay assertions - Documented replay checks for state update handoff coherence and Persona Lens route reason markers.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md - Replay Human Prompt Route Specificity Contract - Parity replay now validates facilitated human prompts and compact signal/route reason summaries.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - human prompt route specificity replay assertions - Documented replay checks for facilitated human prompts and signal/route reason summaries.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md - Replay Workflow First Question Mechanical Status Contract - Guided replay now uses workflow-specific first questions and mechanical-build status/evidence prompts.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - workflow first question mechanical status assertions - Documented workflow-specific first questions and mechanical-build status/evidence prompt checks.
