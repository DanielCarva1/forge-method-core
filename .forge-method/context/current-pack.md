# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-generator-added
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by adding research-scan generator coverage for market/domain/technical evidence closeouts.

## Latest Checkpoint

# Spec kernel generator added

- created_at: 2026-06-15T21:25:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact spec-kernel so write-spec can generate, register, and validate compact spec kernel handoff artifacts instead of hand-written markdown.

## Decisions

- Use first-class generators for phase-closing artifacts when a workflow has a stable template plus validator; research-scan is the next shared generator candidate.

## Checks

- focused tests, workflow validate, workflow compactness, parity replay, smoke-runtime, smoke-install, full unittest, and verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-phase-closeout-generator-audit.md

## Next Action

Continue post-parity Forge polish by adding research-scan generator coverage for market/domain/technical evidence closeouts.

## Recovery Signals

### Failed Checks

- initial smoke-runtime run failed because it still expected the old generic first question; smoke assertions were updated and the rerun passed
- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/discovery-closeout-artifact.md
- skills/forge-method/references/workflow-discover-intent.md
- skills/forge-method/facilitation/discover-intent.md
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-195024-gate-quality-gate.md
- .forge-method/evidence/20260615-202504-validation-discovery-closeout-generator-validation.md
- .forge-method/evidence/20260615-202609-gate-quality-gate.md
- .forge-method/evidence/20260615-205726-validation-discovery-closeout-human-guidance-validation.md
- .forge-method/evidence/20260615-212525-validation-spec-kernel-generator-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout generator - Unreleased notes updated with artifact discovery-closeout, packaged template, workflow metadata, and smoke coverage.
- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-human-guidance-contract.md - Discovery closeout human guidance contract - Discover-intent guidance now asks for fields needed by artifact discovery-closeout before specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout human guidance - Unreleased notes updated with discover-intent human guidance that shapes first facilitation answers into discovery-closeout fields.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-phase-closeout-generator-audit.md - Phase closeout generator audit - Audited phase-closing workflows for generator plus guided-field extraction coverage and selected spec-kernel as the next central closeout gap.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - spec kernel generator - Unreleased notes updated with artifact spec-kernel, write-spec handoff, product-planning guidance, and source/install smoke coverage.
