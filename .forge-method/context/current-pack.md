# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: research-scan-generator-added
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue post-parity Forge polish by adding game-check generator coverage for game brief and sprint planning closeouts.

## Latest Checkpoint

# Research scan generator added

- created_at: 2026-06-15T22:08:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: research-scan-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact research-scan so market/domain/technical research closeouts are generated, registered, and validated before downstream planning.

## Decisions

- Use first-class runtime generators for stable phase-closeout artifacts; research scans now share validator, command, workflow handoff, tests, and source/install smoke coverage.

## Checks

- focused research-scan tests passed; workflow validate passed; workflow compactness passed; parity replay 90/90 passed; smoke-runtime.ps1 passed; smoke-install.ps1 passed; python -m unittest discover -s tests passed; verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/references/workflow-market-scan.md
- skills/forge-method/references/workflow-domain-scan.md
- skills/forge-method/references/workflow-technical-feasibility-scan.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-research-scan-generator-contract.md

## Next Action

Continue post-parity Forge polish by adding game-check generator coverage for game brief and sprint planning closeouts.

## Recovery Signals

### Failed Checks

- initial smoke-runtime run failed because it still expected the old generic first question; smoke assertions were updated and the rerun passed
- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/discovery-closeout-artifact.md
- skills/forge-method/references/workflow-discover-intent.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- skills/forge-method/facilitation/discover-intent.md
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/facilitation/evidence-research.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-202504-validation-discovery-closeout-generator-validation.md
- .forge-method/evidence/20260615-202609-gate-quality-gate.md
- .forge-method/evidence/20260615-205726-validation-discovery-closeout-human-guidance-validation.md
- .forge-method/evidence/20260615-212525-validation-spec-kernel-generator-validation.md
- .forge-method/evidence/20260615-220809-validation-research-scan-generator-validation.md

## Recent Artifacts

- runtime-guidance-contract [active/durable]: .forge-method/artifacts/20260615-discovery-closeout-human-guidance-contract.md - Discovery closeout human guidance contract - Discover-intent guidance now asks for fields needed by artifact discovery-closeout before specification.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - discovery closeout human guidance - Unreleased notes updated with discover-intent human guidance that shapes first facilitation answers into discovery-closeout fields.
- internal-audit [active/durable]: .forge-method/artifacts/20260615-phase-closeout-generator-audit.md - Phase closeout generator audit - Audited phase-closing workflows for generator plus guided-field extraction coverage and selected spec-kernel as the next central closeout gap.
- changelog [active/durable]: CHANGELOG.md - Unreleased changelog - spec kernel generator - Unreleased notes updated with artifact spec-kernel, write-spec handoff, product-planning guidance, and source/install smoke coverage.
- runtime-contract [active/durable]: .forge-method/artifacts/20260615-research-scan-generator-contract.md - Research scan generator contract - First-class artifact research-scan generator for market, domain, and technical evidence closeouts with source/install smoke coverage.
