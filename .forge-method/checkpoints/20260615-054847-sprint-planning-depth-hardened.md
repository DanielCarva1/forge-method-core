# Sprint Planning Depth hardened

- created_at: 2026-06-15T05:48:47+00:00
- project: forge-method-core
- phase: 6-evolve
- status: sprint-planning-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the sprint-planning guidance gap: plan-sprint now has a compact source-aware state machine, dedicated sprint plan artifact template, sequence/rebalance/validate metadata, enriched story-lifecycle facilitation, Guidance Engine precedence over generic quality wording, and parity replay coverage.

## Decisions

- Sprint planning is not a backlog dump; it must preserve sprint goal, ordered story batch, decision-source map, validation/evidence plan, and deferred/blocked reasons before build.
- Explicit sprint planning intent outranks generic validation/quality wording.

## Checks

- python -m unittest discover -s tests: passed
- workflow validate: passed
- workflow compactness: passed
- parity replay: passed
- config validate --root .: passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- artifact verify --root .: passed
- audit --root .: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-plan-sprint.md
- skills/forge-method/facilitation/story-lifecycle.md
- skills/forge-method/templates/sprint-plan-artifact.md
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
- .forge-method/evidence/20260615-054633-validation-sprint-planning-depth-validation.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect dev-story mechanical autonomy and no-procedural-confirmation transcript gaps before claiming full guided-flow parity.
