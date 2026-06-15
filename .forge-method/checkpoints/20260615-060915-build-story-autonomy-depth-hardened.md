# Build Story Autonomy Depth hardened

- created_at: 2026-06-15T06:09:15+00:00
- project: forge-method-core
- phase: 6-evolve
- status: build-story-autonomy-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the dev-story mechanical-autonomy gap: build-story now has a compact workflow contract, structured work-order template, catalog modes, full mechanical command map, JSON loop/do_not_prompt fields, compact recovery priority protection, and Codex Goal handoff that forbids procedural ok/continue prompts.

## Decisions

- Mechanical story loops must expose the full start/resume -> implement -> check -> review -> evidence -> done -> next-story/ready-gate contract in JSON, not only prose.
- Compact recovery must prioritize Read First over long command maps so fresh chats stay usable under small context budgets.

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
- skills/forge-method/references/workflow-build-story.md
- skills/forge-method/templates/build-story-work-order.md
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
- .forge-method/evidence/20260615-060843-validation-build-story-autonomy-depth-validation.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect editorial/edge-case/party-mode human guidance gaps before claiming full guided-flow parity.
