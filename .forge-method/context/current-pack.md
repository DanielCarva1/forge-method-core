# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: parity-p2-ready
- workflow: release-readiness
- active_story: <none>
- next_action: v1.31.0 is published; next work is tester feedback or a new evolve cycle.

## Latest Checkpoint

# v1.31.0 published

- created_at: 2026-06-17T19:42:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-p2-ready
- workflow: release-readiness
- active_story: <none>

## Summary

Forge Method Core v1.31.0 is now publicly versioned on GitHub with tag and release, and the tagged package installs successfully by ref.

## Decisions

- Release definition is public versioned availability, not just a pushed branch; v1.31.0 meets that definition now.

## Checks

- tag v1.31.0 points to cc2f99118063d7e3b6661c758ef602858c6b6a43
- GitHub Release URL exists
- clone/install smoke from v1.31.0 passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

v1.31.0 is published; next work is tester feedback or a new evolve cycle.

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

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-175907-validation-validate-published-version-1-30-0.md
- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md
- .forge-method/evidence/20260617-192015-validation-validation-v1-31-0-p2-parity-utility-surfaces.md
- .forge-method/evidence/20260617-192503-validation-validation-v1-31-0-release-readiness-after-push.md
- .forge-method/evidence/20260617-194235-validation-validation-v1-31-0-github-release-published.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Internal benchmark now includes P2 runtime utility workflow targets: isolated eval runner, hook/event plan, and API/browser utility, with replay fixture coverage.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated compact capability index for Forge Method 1.31.0 with runtime utility workflows, route diagnostics, packs, templates, and validation metadata.
- changelog [active/durable]: CHANGELOG.md - Forge Method 1.31.0 changelog - Changelog moved parity closure and runtime utility work into Forge Method Core v1.31.0 with opt-in utility surfaces and release/version validation routing.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
