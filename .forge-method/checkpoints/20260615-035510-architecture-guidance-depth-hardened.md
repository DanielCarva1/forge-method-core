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
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish rows; do not claim full guided-flow parity until the completion audit and live transcripts prove it.
