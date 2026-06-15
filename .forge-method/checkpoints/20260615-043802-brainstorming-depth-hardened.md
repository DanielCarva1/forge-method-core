# Brainstorming Depth hardened

- created_at: 2026-06-15T04:38:02+00:00
- project: forge-method-core
- phase: 6-evolve
- status: brainstorming-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Brainstorming Depth parity gap. Brainstorming now has guided divergence, taste and anti-reference prompts, pressure testing, discard pile, selection criteria, compact template, catalog modes, replay proof, and install/runtime validation.

## Decisions

- Option-generation language outranks generic confusion so broad ideas receive guided divergence before PRD or architecture.
- Taste-heavy creative direction still routes to creative-session before generic brainstorming.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- parity replay: 61/61 passed
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
- skills/forge-method/facilitation/brainstorming.md
- skills/forge-method/templates/brainstorming-artifact.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish rows; run completion audit and live transcript review before claiming full guided-flow parity.
