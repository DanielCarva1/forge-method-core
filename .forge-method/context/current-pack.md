# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: installed-guide-output-smoke-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing generated project open/reload selection and first-run facilitation prompts against the richer human guidance contract.

## Latest Checkpoint

# Installed guide output smoke hardened

- created_at: 2026-06-15T17:49:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: installed-guide-output-smoke-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Extended install smoke so the packaged Forge runtime is tested against the live non-JSON guide contract: installed guide must print Guidance and First question lines for a real initialized project start and must not regress to the old Prompt blob.

## Decisions

- Installed smoke is now responsible for proving that the distributed skill preserves the richer human guidance surface, not just JSON parity replay.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .
- python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals

## Failed Checks

- none

## Touched Files

- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing generated project open/reload selection and first-run facilitation prompts against the richer human guidance contract.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md
- .forge-method/state.yaml
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson
- scripts/smoke-install.ps1
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-160212-validation-replay-state-update-route-reason-contract-valida.md
- .forge-method/evidence/20260615-163924-validation-replay-human-prompt-route-specificity-contract-v.md
- .forge-method/evidence/20260615-170307-validation-replay-workflow-first-question-mechanical-status.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - workflow first question mechanical status assertions - Documented workflow-specific first questions and mechanical-build status/evidence prompt checks.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md - Guide CLI first question output contract - Non-JSON guide output now surfaces facilitated first questions as dedicated lines and mechanical-build as autonomous status text while preserving JSON shape.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guide CLI output - Unreleased notes updated with dedicated non-JSON guide First question lines and mechanical-build Status output.
- runtime-install-smoke-contract [active/durable]: .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md - Installed guide output smoke contract - Install smoke now captures installed guide output in a real project start and asserts Guidance plus First question lines while blocking the old Prompt blob shape.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - installed guide output smoke - Unreleased notes updated with install smoke assertions for installed guide Guidance and First question output.
