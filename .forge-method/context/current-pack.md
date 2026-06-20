# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: validated
- workflow: guideline-audit
- active_story: <none>
- next_action: decide whether to optimize the slowest runtime tests or run release readiness

## Latest Checkpoint

# Smart test suite observability

- created_at: 2026-06-18T04:35:51+00:00
- project: forge-method-core
- phase: 6-evolve
- status: validated
- workflow: guideline-audit
- active_story: <none>

## Summary

Added debug mode, JSON/JUnit reports, retained logs, match filtering, and report-driven failure/slowest reruns to the responsive test suite.

## Decisions

- Keep full test coverage, but make the suite observable through per-test reports and debug reruns instead of opaque unittest discovery.

## Checks

- py_compile passed
- runner self-tests passed
- verify-fast debug path passed
- bash wrapper syntax passed
- full responsive suite passed 133/133 in 199.4s

## Failed Checks

- none

## Touched Files

- scripts/test-runner.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- scripts/verify-all.ps1
- scripts/verify-all.sh
- tests/test_test_runner.py
- AGENTS.md
- docs/07-v1-readiness-audit.md
- assets/marketplace/listing.json

## Artifacts

- .forge-method/evidence/20260618-013448-smart-test-suite-observability.md

## Next Action

decide whether to optimize the slowest runtime tests or run release readiness

## Recovery Signals

### Failed Checks

- Legacy direct `python -m unittest discover -s tests` timed out during this work. Replaced in verification scripts with `scripts/test-runner.py`, which preserves coverage while adding progress, per-test timeouts, and slow-test reporting.

### Touched Files

- CHANGELOG.md
- scripts/test-runner.py
- scripts/verify-all.ps1
- scripts/verify-all.sh
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- skills/forge-guideline-auditor/**
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/guideline-audit.md
- skills/forge-method/modules/runtime-builder.yaml
- skills/forge-method/references/workflow-guideline-audit.md
- skills/forge-method/scripts/forge_method_runtime.py

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-205618-publication-v1-31-1-public-install-hotfix-published.md
- .forge-method/evidence/20260617-232255-validation-v1-31-2-guided-research-drift-hotfix-validated.md
- .forge-method/evidence/20260617-232617-publication-v1-31-2-guided-research-drift-hotfix-published.md
- .forge-method/evidence/20260617-235258-documentation-guideline-auditor-integrated.md
- .forge-method/evidence/20260618-013448-smart-test-suite-observability.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
- skill [active/durable]: skills/forge-guideline-auditor/SKILL.md - Forge Guideline Auditor - Reusable Forge Guideline Auditor skill for turning gaps into guidelines, work-order candidates, and acceptance evidence before durable implementation.
- workflow [active/durable]: skills/forge-method/references/workflow-guideline-audit.md - Guideline Audit workflow - Compact guideline-audit workflow for routing guideline, work-order, acceptance-evidence, and guarded implementation requests before build.
- changelog [active/durable]: CHANGELOG.md - Forge Guideline Auditor changelog - Unreleased notes record Forge Guideline Auditor, guideline-audit routing, work-order fields, and regression coverage.
- evidence [active/durable]: .forge-method/evidence/20260618-013448-smart-test-suite-observability.md - Smart test suite observability - Debug/report/JUnit runner observability added and validated with full responsive unit run.
