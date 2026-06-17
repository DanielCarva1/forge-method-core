# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>
- next_action: v1.31.0 is public on main and tag; next work is tester feedback or a new evolve cycle.

## Latest Checkpoint

# v1.31.0 merged to main

- created_at: 2026-06-17T19:53:34+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>

## Summary

Forge Method Core v1.31.0 is now available from both the version tag/GitHub Release and the default main branch; main clone/install validates as 1.31.0.

## Decisions

- Definition of ready now includes default public repo path, not only branch or tag availability.

## Checks

- main pushed to GitHub
- clone/install from main passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

v1.31.0 is public on main and tag; next work is tester feedback or a new evolve cycle.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- CHANGELOG.md
- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-isolated-eval-runner.md
- skills/forge-method/references/workflow-hook-event-plan.md
- skills/forge-method/references/workflow-api-browser-utility.md
- skills/forge-method/facilitation/runtime-utility.md
- scripts/forge-eval-runner.ps1
- scripts/forge-hook-dispatch.ps1
- scripts/forge-eval-runner.sh

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- operator (Operator): Maintain a ready project through usage notes, support status, feedback, and future backlog.
- quality-reviewer (Quality Reviewer): Review implementation, artifacts, workflows, and evidence before work is marked done or ready.

## Recent Evidence

- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md
- .forge-method/evidence/20260617-192015-validation-validation-v1-31-0-p2-parity-utility-surfaces.md
- .forge-method/evidence/20260617-192503-validation-validation-v1-31-0-release-readiness-after-push.md
- .forge-method/evidence/20260617-194235-validation-validation-v1-31-0-github-release-published.md
- .forge-method/evidence/20260617-195333-validation-validation-v1-31-0-main-published-install.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Internal benchmark now includes P2 runtime utility workflow targets: isolated eval runner, hook/event plan, and API/browser utility, with replay fixture coverage.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated compact capability index for Forge Method 1.31.0 with runtime utility workflows, route diagnostics, packs, templates, and validation metadata.
- changelog [active/durable]: CHANGELOG.md - Forge Method 1.31.0 changelog - Changelog moved parity closure and runtime utility work into Forge Method Core v1.31.0 with opt-in utility surfaces and release/version validation routing.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
