# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-published
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial parity rows; do not claim full guided-flow parity until the audit rows and live transcripts prove it.

## Latest Checkpoint

# Forge Method 1.29.0 published

- created_at: 2026-06-15T03:06:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-published
- workflow: runtime-builder
- active_story: <none>

## Summary

Published Forge Method Core v1.29.0 as the guided workflow depth release batch. The release commit is tagged v1.29.0, origin has the tag, branch codex/script-audit-optimization is pushed, and clone install smoke passed from the published tag.

## Decisions

- Treat 1.29.0 as an intermediate release batch, not final guided-flow parity completion.
- Return the runtime state to 6-evolve/runtime-builder for real-use transcript hardening and remaining partial parity rows.

## Checks

- git ls-remote --tags origin v1.29.0: found
- smoke-plugin-clone-install.ps1 -Ref v1.29.0 -ExpectedVersion 1.29.0: passed

## Failed Checks

- none

## Touched Files

- .forge-method/state.yaml

## Artifacts

- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md

## Next Action

Continue real-use transcript hardening for remaining partial parity rows; do not claim full guided-flow parity until the audit rows and live transcripts prove it.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- CHANGELOG.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/context/capability-index.json

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-013605-planning-p2-scope-decisions-recorded.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md
- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md
- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md

## Recent Artifacts

- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic Parity Plan - Systematic plan updated with P1.7 Parity Closure Utilities and next release/version validation path.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Parity Closure Utility workflows, templates, and module membership.
- patch-notes [active/durable]: CHANGELOG.md - Unreleased Patch Notes - Unreleased notes updated with Parity Closure Utilities plus Guidance Engine human polish, Game Studio Depth, TEA Depth, and P2 scope decisions.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method 1.29.0 changelog - Changelog moved the guided workflow depth batch from Unreleased into Forge Method Core v1.29.0.
- release-notes [active/durable]: release-notes/1.29.0.md - Forge Method 1.29.0 release notes - Release notes for guided workflow depth: Guidance Engine, Help Oracle, Builder Factory, Capability Index, Persona Lens, lifecycle/game/quality depth, closure utilities, replay, and validation.
