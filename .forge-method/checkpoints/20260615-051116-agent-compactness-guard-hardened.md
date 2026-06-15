# Agent Compactness Guard hardened

- created_at: 2026-06-15T05:11:16+00:00
- project: forge-method-core
- phase: 6-evolve
- status: agent-compactness-guard-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the progressive-disclosure audit row. Workflow refs now have compactness limits, forbidden human-pack sections, root-section checks, and heading checks; facilitation packs have shape and size checks; workflow compactness, workflow validate, audit, smoke-runtime, and unit tests prove the split between compact agent docs and rich human packs.

## Decisions

- Progressive disclosure must be deterministic: agent workflow refs stay compact state machines, while human richness lives in facilitation packs.
- The guard should fail normal validation and audit when the layers blur, not depend on review taste.

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
- scripts/smoke-runtime.ps1
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
