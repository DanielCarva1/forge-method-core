# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-before-specification-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing discovery closeout artifact content quality and Grill Gate handoff before specification.

## Latest Checkpoint

# Discovery closeout before specification hardened

- created_at: 2026-06-15T19:09:40+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-before-specification-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Blocked generated-project transition from answered discovery to specification until a durable discovery-intent closeout artifact exists.

## Decisions

- The first facilitation answer is discovery material and must be compacted into a durable closeout artifact before specification.

## Checks

- unit, runtime smoke, install smoke, parity replay, full unittest, verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md
- CHANGELOG.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing discovery closeout artifact content quality and Grill Gate handoff before specification.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml
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

- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md
- .forge-method/evidence/20260615-190939-validation-discovery-closeout-before-specification-validati.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - generated project open reload smoke - Unreleased notes updated with runtime/install smoke assertions for generated project first facilitation and workspace open/reload selection output.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md - Initial facilitation answer guidance contract - Initial-facilitation answers now stay in guided discovery with zero stories, Grill Gate required, clean first-question lede output, and source/installed smoke coverage.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - initial facilitation answer guidance - Unreleased notes updated with initial-facilitation answer routing, zero-story, Grill Gate, and first-question guidance coverage.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md - Discovery closeout before specification contract - Answered initial-facilitation generated projects must capture a durable discovery-intent closeout artifact before transitioning from discovery to specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout before specification - Unreleased notes updated with the generated-project discovery closeout guard before specification.
