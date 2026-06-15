# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: generated-project-open-reload-smoke-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing the first human answer path after initial-facilitation, ensuring it routes through Guidance Engine instead of creating premature stories.

## Latest Checkpoint

# Generated project open reload smoke hardened

- created_at: 2026-06-15T18:03:58+00:00
- project: forge-method-core
- phase: 6-evolve
- status: generated-project-open-reload-smoke-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Hardened source and install smokes so generated projects must show first facilitation before stories, project list must expose waiting-human-input, and parent workspace preflight/reload must keep explicit project selection with stale-copy guard text.

## Decisions

- Generated project creation and parent workspace reload are part of the human guided experience; smoke tests now protect them as product behavior, not incidental console output.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Failed Checks

- none

## Touched Files

- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing the first human answer path after initial-facilitation, ensuring it routes through Guidance Engine instead of creating premature stories.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md
- .forge-method/state.yaml
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson
- scripts/smoke-install.ps1
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- scripts/smoke-runtime.ps1

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-163924-validation-replay-human-prompt-route-specificity-contract-v.md
- .forge-method/evidence/20260615-170307-validation-replay-workflow-first-question-mechanical-status.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - guide CLI output - Unreleased notes updated with dedicated non-JSON guide First question lines and mechanical-build Status output.
- runtime-install-smoke-contract [active/durable]: .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md - Installed guide output smoke contract - Install smoke now captures installed guide output in a real project start and asserts Guidance plus First question lines while blocking the old Prompt blob shape.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - installed guide output smoke - Unreleased notes updated with install smoke assertions for installed guide Guidance and First question output.
- runtime-smoke-contract [active/durable]: .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md - Generated project open/reload smoke contract - Runtime and install smokes now assert generated projects require first facilitation and workspace parent preflight/reload keeps explicit project selection instead of stale or automatic progression.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - generated project open reload smoke - Unreleased notes updated with runtime/install smoke assertions for generated project first facilitation and workspace open/reload selection output.
