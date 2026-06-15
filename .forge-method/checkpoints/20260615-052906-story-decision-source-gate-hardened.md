# Story Decision Source Gate hardened

- created_at: 2026-06-15T05:29:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-decision-source-gate-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the epics/stories decision-source invariant gap. Story add/import/start now prevents implementation-ready build stories without approved source artifacts, autoattaches a single clear source, requires --source when several artifacts could justify different stories, persists decision_sources, and audit verifies the source map before build-story.

## Decisions

- Stories are not a substitute for accepted decisions; build-ready stories must carry explicit decision_sources.
- Automation can continue only after the source map is durable; ambiguous sources require explicit selection.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- workflow compactness: passed
- parity replay: 63/63 passed
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
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
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

Continue real-use transcript hardening for remaining partial and strong-ish rows; run completion audit and live transcript review before claiming full guided-flow parity.
