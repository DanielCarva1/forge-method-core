# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-quality-gate-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by adding a first-class discovery closeout template or generator so agents can produce the required artifact without hand-rolled markdown.

## Latest Checkpoint

# Discovery closeout quality gate hardened

- created_at: 2026-06-15T19:49:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-quality-gate-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact discovery-check and made discovery-to-spec transition require a valid closeout with Grill Gate handoff fields.

## Decisions

- A discovery closeout must be useful agent context, not just an artifact marker; weak title/summary artifacts remain blocked.

## Checks

- unit, runtime smoke, install smoke, parity replay, full unittest, verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by adding a first-class discovery closeout template or generator so agents can produce the required artifact without hand-rolled markdown.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml
- scripts/smoke-runtime.ps1
- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md
- .forge-method/evidence/20260615-190939-validation-discovery-closeout-before-specification-validati.md
- .forge-method/evidence/20260615-191033-gate-quality-gate.md
- .forge-method/evidence/20260615-194943-validation-discovery-closeout-quality-gate-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - initial facilitation answer guidance - Unreleased notes updated with initial-facilitation answer routing, zero-story, Grill Gate, and first-question guidance coverage.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md - Discovery closeout before specification contract - Answered initial-facilitation generated projects must capture a durable discovery-intent closeout artifact before transitioning from discovery to specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout before specification - Unreleased notes updated with the generated-project discovery closeout guard before specification.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md - Discovery closeout quality gate contract - Discovery closeout artifacts must pass artifact discovery-check with source, audience, outcome, constraints, non-goals, success signal, Grill Gate handoff, and next workflow before specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout quality gate - Unreleased notes updated with artifact discovery-check and closeout quality requirements before specification.
