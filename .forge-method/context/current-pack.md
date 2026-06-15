# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-human-guidance-improved
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by auditing other phase-closing workflows for first-class generators and guided field extraction.

## Latest Checkpoint

# Discovery closeout human guidance finalized

- created_at: 2026-06-15T20:59:29+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-human-guidance-improved
- workflow: runtime-builder
- active_story: <none>

## Summary

Finalized discover-intent human guidance after state transition so the durable checkpoint matches the improved status.

## Decisions

- The next post-parity polish should audit other phase-closing workflows for the same generator plus guided-field extraction pattern.

## Checks

- focused tests, workflow validate, parity replay, full unittest, smoke-runtime, smoke-install, verify-fast, artifact verify, audit, and gate passed

## Failed Checks

- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/discover-intent.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-human-guidance-contract.md

## Next Action

Continue post-parity Forge polish by auditing other phase-closing workflows for first-class generators and guided field extraction.

## Recovery Signals

### Failed Checks

- initial smoke-runtime run failed because it still expected the old generic first question; smoke assertions were updated and the rerun passed
- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- .forge-method/artifacts/20260615-discovery-closeout-before-specification-contract.md
- CHANGELOG.md
- .forge-method/state.yaml
- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/discovery-closeout-artifact.md
- skills/forge-method/references/workflow-discover-intent.md
- skills/forge-method/facilitation/discover-intent.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-194943-validation-discovery-closeout-quality-gate-validation.md
- .forge-method/evidence/20260615-195024-gate-quality-gate.md
- .forge-method/evidence/20260615-202504-validation-discovery-closeout-generator-validation.md
- .forge-method/evidence/20260615-202609-gate-quality-gate.md
- .forge-method/evidence/20260615-205726-validation-discovery-closeout-human-guidance-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout quality gate - Unreleased notes updated with artifact discovery-check and closeout quality requirements before specification.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-generator-contract.md - Discovery closeout generator contract - Agents can now run artifact discovery-closeout to generate, register, and validate the accepted discovery closeout artifact without hand-written markdown.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout generator - Unreleased notes updated with artifact discovery-closeout, packaged template, workflow metadata, and smoke coverage.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-human-guidance-contract.md - Discovery closeout human guidance contract - Discover-intent guidance now asks for fields needed by artifact discovery-closeout before specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout human guidance - Unreleased notes updated with discover-intent human guidance that shapes first facilitation answers into discovery-closeout fields.
