# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: guide-cli-first-question-output-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing installed reload/guide behavior in real project starts against the richer human prompt contract.

## Latest Checkpoint

# Guide CLI first question output finalized

- created_at: 2026-06-15T17:35:29+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guide-cli-first-question-output-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Finalized the non-JSON guide output contract: facilitated workflows now print Guidance and First question lines, mechanical-build prints Status text, JSON parity remains unchanged, and CHANGELOG artifact tracking is current.

## Decisions

- The live CLI text is validated as part of the human guidance surface, not just JSON replay fixtures.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract tests.test_runtime.RuntimeTests.test_mechanical_work_order_goal_and_commit_policy_contracts -v
- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .
- python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-guide-cli-first-question-output-c
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md
- .forge-method/state.yaml
- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-152414-validation-replay-mutating-command-contract-validation.md
- .forge-method/evidence/20260615-160212-validation-replay-state-update-route-reason-contract-valida.md
- .forge-method/evidence/20260615-163924-validation-replay-human-prompt-route-specificity-contract-v.md
- .forge-method/evidence/20260615-170307-validation-replay-workflow-first-question-mechanical-status.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - human prompt route specificity replay assertions - Documented replay checks for facilitated human prompts and signal/route reason summaries.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md - Replay Workflow First Question Mechanical Status Contract - Guided replay now uses workflow-specific first questions and mechanical-build status/evidence prompts.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - workflow first question mechanical status assertions - Documented workflow-specific first questions and mechanical-build status/evidence prompt checks.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md - Guide CLI first question output contract - Non-JSON guide output now surfaces facilitated first questions as dedicated lines and mechanical-build as autonomous status text while preserving JSON shape.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guide CLI output - Unreleased notes updated with dedicated non-JSON guide First question lines and mechanical-build Status output.
