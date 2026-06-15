# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: context-boundary-recovery-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish rows; do not claim full guided-flow parity until the completion audit and live transcripts prove it.

## Latest Checkpoint

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

- .forge-method/artifacts
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- VERSION
- .codex-plugin/plugin.json
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- README.md
- docs/00-quickstart.md
- docs/04-distribution.md
- assets/marketplace/listing.json
- release-notes/latest.json
- release-notes/1.29.0.md
- .forge-method/state.yaml

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md
- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md
- .forge-method/evidence/20260615-035510-validation-architecture-guidance-depth-validation.md
- .forge-method/evidence/20260615-041351-validation-context-boundary-recovery-validation.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Audit updated after Context Boundary Recovery: fresh-chat/context-reset parity row now reflects reload, Help Oracle context boundary, context-recovery pack/template, and replay proof.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Plan updated with Context Boundary Recovery and the remaining transcript hardening batch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Benchmark updated so fresh chat, reload, network drop, and context reset messages route to context-recovery with a compact context boundary.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Context Boundary Recovery workflow metadata, pack, template, and modes.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method changelog - Unreleased notes updated with Context Boundary Recovery.
