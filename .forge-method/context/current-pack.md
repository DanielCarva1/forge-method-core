# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: hotfix-validated
- workflow: release-readiness
- active_story: <none>
- next_action: v1.31.1 is public; next work is tester feedback or a new evolve cycle.

## Latest Checkpoint

# v1.31.1 public install hotfix published

- created_at: 2026-06-17T20:56:19+00:00
- project: forge-method-core
- phase: 6-evolve
- status: hotfix-validated
- workflow: release-readiness
- active_story: <none>

## Summary

Published Forge Method 1.31.1 to main, tag, and GitHub Release. Validated public clone/install by tag and main, then updated the local Codex plugin to 1.31.1.

## Decisions

- Treat public install leakage of core state as a release-blocking distribution bug; maintainer core-edit mode now requires local marker/env.

## Checks

- GitHub Release v1.31.1 created
- Clone/install smoke passed for v1.31.1 and main
- Local plugin preflight no longer reports version mismatch

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

v1.31.1 is public; next work is tester feedback or a new evolve cycle.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- none

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-192503-validation-validation-v1-31-0-release-readiness-after-push.md
- .forge-method/evidence/20260617-194235-validation-validation-v1-31-0-github-release-published.md
- .forge-method/evidence/20260617-195333-validation-validation-v1-31-0-main-published-install.md
- .forge-method/evidence/20260617-205038-validation-validation-v1-31-1-public-install-core-state-gua.md
- .forge-method/evidence/20260617-205618-publication-v1-31-1-public-install-hotfix-published.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Internal benchmark now includes P2 runtime utility workflow targets: isolated eval runner, hook/event plan, and API/browser utility, with replay fixture coverage.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated compact capability index for Forge Method 1.31.0 with runtime utility workflows, route diagnostics, packs, templates, and validation metadata.
- changelog [active/durable]: CHANGELOG.md - Forge Method 1.31.0 changelog - Changelog moved parity closure and runtime utility work into Forge Method Core v1.31.0 with opt-in utility surfaces and release/version validation routing.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
