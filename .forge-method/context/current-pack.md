# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: post-command-help-oracle-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue real-use transcript hardening for remaining partial and strong-ish parity rows; prioritize human guidance depth where routing is correct but the conversation still feels thin.

## Latest Checkpoint

# Post-command Help Oracle hardened

- created_at: 2026-06-15T03:29:08+00:00
- project: forge-method-core
- phase: 6-evolve
- status: post-command-help-oracle-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the post-command Help Oracle hardening increment. Progress-changing runtime commands now record compact next-workflow guidance in ledger.ndjson, and interactive mutations emit the next required workflow, recommended phase, alternatives, facilitation, and stale-state guard for the human/agent immediately after the mutation.

## Decisions

- Treat the bmad-help audit row as translated for the post-command next-step contract, while keeping full parity open for remaining human-experience depth rows.
- Keep path-output commands stdout-stable; they record guidance in the ledger instead of printing extra text.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- parity replay: 58/58 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- artifact verify, audit, config validate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- CHANGELOG.md

## Artifacts

- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish parity rows; prioritize human guidance depth where routing is correct but the conversation still feels thin.

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

- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md
- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md
- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md
- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md

## Recent Artifacts

- patch-notes [active/durable]: CHANGELOG.md - Forge Method 1.29.0 changelog - Changelog moved the guided workflow depth batch from Unreleased into Forge Method Core v1.29.0.
- release-notes [active/durable]: release-notes/1.29.0.md - Forge Method 1.29.0 release notes - Release notes for guided workflow depth: Guidance Engine, Help Oracle, Builder Factory, Capability Index, Persona Lens, lifecycle/game/quality depth, closure utilities, replay, and validation.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method changelog - Changelog Unreleased records post-command Help Oracle guidance for progress-changing runtime commands, including emitted human guidance and compact ledger records.
- internal-parity-plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan - Plan updated after Forge Method 1.29.0: post-command Help Oracle hardening is recorded, and the next implementation batch is real-use transcript hardening for remaining partial and strong-ish rows.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Audit updated after post-command Help Oracle hardening: bmad-help now has a translated record/emit contract for progress-changing commands, while remaining partial/strong-ish rows still require real-use transcript hardening.
