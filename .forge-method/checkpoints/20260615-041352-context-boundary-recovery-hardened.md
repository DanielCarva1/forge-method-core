# Context Boundary Recovery hardened

- created_at: 2026-06-15T04:13:52+00:00
- project: forge-method-core
- phase: 6-evolve
- status: context-boundary-recovery-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the Fresh chats per workflow gap. Context-recovery now has facilitation pack, template, modes, compact workflow contract, Guidance Engine replay for interrupted chat/network context, and Help Oracle context_boundary metadata in reload/resume/post-command ledger.

## Decisions

- Fresh chat, reload, network drop, and stale context messages route to context-recovery before generic lifecycle/project-context routing.
- Help Oracle now carries compact context_boundary metadata so future agents can resume from durable files without relying on prior chat memory.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- workflow validate: passed
- parity replay: 60/60 passed
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
- skills/forge-method/references/workflow-context-recovery.md
- skills/forge-method/facilitation/context-boundary.md
- skills/forge-method/templates/context-recovery-artifact.md
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
