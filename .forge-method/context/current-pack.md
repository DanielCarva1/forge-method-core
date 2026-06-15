# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-generator-added
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by improving human-facing discovery closeout guidance so artifact discovery-closeout arguments can be derived from a guided conversation cleanly.

## Latest Checkpoint

# Discovery closeout generator added

- created_at: 2026-06-15T20:25:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact discovery-closeout, a packaged discovery-closeout-artifact template, discover-intent template metadata, and workflow handoff docs.

## Decisions

- Discovery closeout creation is now a first-class runtime command; agents should not hand-roll the required markdown fields.

## Checks

- focused tests, workflow validate, smokes, parity replay, full unittest, verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/discovery-closeout-artifact.md
- skills/forge-method/references/workflow-discover-intent.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-generator-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by improving human-facing discovery closeout guidance so artifact discovery-closeout arguments can be derived from a guided conversation cleanly.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260615-initial-facilitation-answer-guidance-contract.md
- .forge-method/evidence/20260615-183009-validation-initial-facilitation-answer-guidance-validation.md
- .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-190939-validation-discovery-closeout-before-specification-validati.md
- .forge-method/evidence/20260615-191033-gate-quality-gate.md
- .forge-method/evidence/20260615-194943-validation-discovery-closeout-quality-gate-validation.md
- .forge-method/evidence/20260615-195024-gate-quality-gate.md
- .forge-method/evidence/20260615-202504-validation-discovery-closeout-generator-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout before specification - Unreleased notes updated with the generated-project discovery closeout guard before specification.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md - Discovery closeout quality gate contract - Discovery closeout artifacts must pass artifact discovery-check with source, audience, outcome, constraints, non-goals, success signal, Grill Gate handoff, and next workflow before specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout quality gate - Unreleased notes updated with artifact discovery-check and closeout quality requirements before specification.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-generator-contract.md - Discovery closeout generator contract - Agents can now run artifact discovery-closeout to generate, register, and validate the accepted discovery closeout artifact without hand-written markdown.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout generator - Unreleased notes updated with artifact discovery-closeout, packaged template, workflow metadata, and smoke coverage.
