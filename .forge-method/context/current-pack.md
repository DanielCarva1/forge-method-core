# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: initial-facilitation-answer-guidance-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing discovery closeout: accepted intent should produce a durable discovery artifact and only then transition toward specification.

## Latest Checkpoint

# Initial facilitation answer guidance hardened

- created_at: 2026-06-15T18:31:22+00:00
- project: forge-method-core
- phase: 6-evolve
- status: initial-facilitation-answer-guidance-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Hardened the first human answer path after initial-facilitation: answering clears the required input but keeps zero stories, stays in discover-intent, requires Grill Gate, routes through Guidance Engine, and prints clean first-question guidance.

## Decisions

- The first answer after project creation is discovery material, not permission to create backlog or build work; agents must route it through Guidance Engine before moving phases.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing discovery
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/state.yaml
- .forge-method/artifacts/index.ndjson
- scripts/smoke-install.ps1
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- scripts/smoke-runtime.ps1
- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-170307-validation-replay-workflow-first-question-mechanical-status.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - installed guide output smoke - Unreleased notes updated with install smoke assertions for installed guide Guidance and First question output.
- runtime-smoke-contract [active/durable]: .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md - Generated project open/reload smoke contract - Runtime and install smokes now assert generated projects require first facilitation and workspace parent preflight/reload keeps explicit project selection instead of stale or automatic progression.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - generated project open reload smoke - Unreleased notes updated with runtime/install smoke assertions for generated project first facilitation and workspace open/reload selection output.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md - Initial facilitation answer guidance contract - Initial-facilitation answers now stay in guided discovery with zero stories, Grill Gate required, clean first-question lede output, and source/installed smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - initial facilitation answer guidance - Unreleased notes updated with initial-facilitation answer routing, zero-story, Grill Gate, and first-question guidance coverage.
