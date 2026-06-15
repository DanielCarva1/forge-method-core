# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: architecture-guidance-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; do not claim full guided-flow parity until the completion audit and live transcripts prove it.

## Latest Checkpoint

# Architecture Guidance Depth hardened

- created_at: 2026-06-15T03:55:10+00:00
- project: forge-method-core
- phase: 6-evolve
- status: architecture-guidance-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Architecture Guidance Depth increment. The architecture workflow now has a compact artifact template, create/update/validate/tradeoff catalog metadata, deeper facilitation tied to PRD/UX/security/interfaces/test hooks/story impact, and Guidance Engine precedence for product architecture over generic quality routing. Audit stale PRD/UX/architecture partial rows were corrected without claiming full parity.

## Decisions

- Treat product architecture with PRD/UX trace and test hooks as architecture planning, while preserving test architecture and fixture architecture as quality-flow routes.
- Keep full guided-flow parity open until remaining partial/strong-ish rows are proven by real-use transcript hardening.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- parity replay: 59/59 passed
- config validate: passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- artifact verify: passed
- audit: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-architecture.md
- skills/forge-method/facilitation/architecture-planning.md
- skills/forge-method/templates/architecture-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/context/capability-index.json
- CHANGELOG.md
- VERSION

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md
- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md
- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md
- .forge-method/evidence/20260615-035510-validation-architecture-guidance-depth-validation.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Audit updated after Architecture Guidance Depth: PRD, UX, and product architecture rows now reflect implemented packs, templates, modes, and replay proof while remaining partial/strong-ish rows stay open.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Plan updated with Architecture Guidance Depth and the next real-use transcript hardening batch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Benchmark updated so product architecture with PRD/UX trace and test hooks outranks generic quality routing.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method changelog - Unreleased notes updated with Architecture Guidance Depth.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Architecture Guidance Depth metadata, including architecture template, modes, followed-by routes, and product architecture routing expectations.
