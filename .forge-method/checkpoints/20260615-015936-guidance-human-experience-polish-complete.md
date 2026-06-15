# Guidance human experience polish complete

- created_at: 2026-06-15T01:59:36+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guidance-human-polish-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Guidance Engine human experience polish with contextual guide lede, runtime-builder routing for human-experience plus agent-doc polish, and quiet correction/runtime Reality/Evidence Gate behavior.

## Decisions

- Human-facing guide output carries the rich lede; workflow refs, state, JSON, and handoffs remain compact for agents.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- smoke-runtime: passed
- verify-fast: passed
- smoke-install: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-guidance-human-experience-polish.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md

## Next Action

Review remaining post-parity polish surface and decide the next release/version batch.
