# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-release-prepared
- workflow: release-readiness
- active_story: <none>
- next_action: Run clean release check after commit, tag v1.29.0 if clean, then continue real-use transcript hardening for remaining partial parity rows.

## Latest Checkpoint

# Forge Method 1.29.0 release prepared

- created_at: 2026-06-15T03:01:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-release-prepared
- workflow: release-readiness
- active_story: <none>

## Summary

Prepared Forge Method Core v1.29.0 as a coherent guided workflow depth release batch. Bumped runtime/package/listing/docs metadata, moved Unreleased changelog entries into 1.29.0, added release notes, fixed launch-ops example seeding with a validation-map decision source, and validated the package with full release verification.

## Decisions

- Ship this as an intermediate release batch; do not claim full BMAD/Forge parity completion while audit rows still show partial/deferred surfaces.
- Build/verify example projects must include a decision-source artifact instead of weakening the implementation-ready story guard.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- scripts/verify-onboarding-assets.py: passed
- workflow validate: passed
- parity replay: 58/58 passed
- verify-all.ps1: passed
- artifact verify: passed
- audit: passed
- config validate: passed

## Failed Checks

- none

## Touched Files

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

## Artifacts

- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- release-notes/1.29.0.md
- CHANGELOG.md

## Next Action

Run clean release check after commit, tag v1.29.0 if clean, then continue real-use transcript hardening for remaining partial parity rows.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md
- CHANGELOG.md
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-013149-validation-p1-6-test-architecture-enterprise-depth-validati.md
- .forge-method/evidence/20260615-013605-planning-p2-scope-decisions-recorded.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md
- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md
- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md

## Recent Artifacts

- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic Parity Plan - Systematic plan updated with P1.7 Parity Closure Utilities and next release/version validation path.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Generated capability index refreshed with Parity Closure Utility workflows, templates, and module membership.
- patch-notes [active/durable]: CHANGELOG.md - Unreleased Patch Notes - Unreleased notes updated with Parity Closure Utilities plus Guidance Engine human polish, Game Studio Depth, TEA Depth, and P2 scope decisions.
- patch-notes [active/durable]: CHANGELOG.md - Forge Method 1.29.0 changelog - Changelog moved the guided workflow depth batch from Unreleased into Forge Method Core v1.29.0.
- release-notes [active/durable]: release-notes/1.29.0.md - Forge Method 1.29.0 release notes - Release notes for guided workflow depth: Guidance Engine, Help Oracle, Builder Factory, Capability Index, Persona Lens, lifecycle/game/quality depth, closure utilities, replay, and validation.
