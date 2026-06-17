# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>
- next_action: Commit v1.31.1 hotfix, create GitHub tag/release, and validate clone/install by v1.31.1 and main.

## Latest Checkpoint

# v1.31.1 public install hotfix validated

- created_at: 2026-06-17T20:50:38+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>

## Summary

Implemented and validated public install routing guard: installed Forge packages no longer expose core project state to normal users; core state requires maintainer marker/env plus explicit allow-runtime-state.

## Decisions

- Ship as patch release 1.31.1 because this affects public install/start behavior.

## Checks

- unit full suite passed
- runtime/package smokes passed
- installed package simulation no longer leaks continue_current_project

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

Commit v1.31.1 hotfix, create GitHub tag/release, and validate clone/install by v1.31.1 and main.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-isolated-eval-runner.md
- skills/forge-method/references/workflow-hook-event-plan.md
- skills/forge-method/references/workflow-api-browser-utility.md
- skills/forge-method/facilitation/runtime-utility.md
- scripts/forge-eval-runner.ps1
- scripts/forge-hook-dispatch.ps1
- scripts/forge-eval-runner.sh
- scripts/forge-hook-dispatch.sh

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- operator (Operator): Maintain a ready project through usage notes, support status, feedback, and future backlog.
- quality-reviewer (Quality Reviewer): Review implementation, artifacts, workflows, and evidence before work is marked done or ready.

## Recent Evidence

- .forge-method/evidence/20260617-192015-validation-validation-v1-31-0-p2-parity-utility-surfaces.md
- .forge-method/evidence/20260617-192503-validation-validation-v1-31-0-release-readiness-after-push.md
- .forge-method/evidence/20260617-194235-validation-validation-v1-31-0-github-release-published.md
- .forge-method/evidence/20260617-195333-validation-validation-v1-31-0-main-published-install.md
- .forge-method/evidence/20260617-205038-validation-validation-v1-31-1-public-install-core-state-gua.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Internal benchmark now includes P2 runtime utility workflow targets: isolated eval runner, hook/event plan, and API/browser utility, with replay fixture coverage.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated compact capability index for Forge Method 1.31.0 with runtime utility workflows, route diagnostics, packs, templates, and validation metadata.
- changelog [active/durable]: CHANGELOG.md - Forge Method 1.31.0 changelog - Changelog moved parity closure and runtime utility work into Forge Method Core v1.31.0 with opt-in utility surfaces and release/version validation routing.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
