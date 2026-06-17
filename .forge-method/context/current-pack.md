# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: parity-p2-validated
- workflow: release-readiness
- active_story: <none>
- next_action: Branch codex/script-audit-optimization contains v1.31.0 and is pushed; next release step is explicit tag/GitHub Release publication or tester install feedback.

## Latest Checkpoint

# v1.31.0 pushed and locally installed

- created_at: 2026-06-17T19:25:03+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-p2-validated
- workflow: release-readiness
- active_story: <none>

## Summary

v1.31.0 parity closure/runtime utility batch is committed, pushed, release-check ready, and locally installed for Codex.

## Decisions

- Do not claim a GitHub tag or GitHub Release until the explicit tag/publish step runs; branch distribution and local plugin install are ready.

## Checks

- release check: Ready yes
- local plugin installed version refreshed to 1.31.0

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

Branch codex/script-audit-optimization contains v1.31.0 and is pushed; next release step is explicit tag/GitHub Release publication or tester install feedback.

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

- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md
- .forge-method/evidence/20260617-175907-validation-validate-published-version-1-30-0.md
- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md
- .forge-method/evidence/20260617-192015-validation-validation-v1-31-0-p2-parity-utility-surfaces.md
- .forge-method/evidence/20260617-192503-validation-validation-v1-31-0-release-readiness-after-push.md

## Recent Artifacts

- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Internal benchmark now includes P2 runtime utility workflow targets: isolated eval runner, hook/event plan, and API/browser utility, with replay fixture coverage.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability Index - Regenerated compact capability index for Forge Method 1.31.0 with runtime utility workflows, route diagnostics, packs, templates, and validation metadata.
- changelog [active/durable]: CHANGELOG.md - Forge Method 1.31.0 changelog - Changelog moved parity closure and runtime utility work into Forge Method Core v1.31.0 with opt-in utility surfaces and release/version validation routing.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current systematic parity audit now records remaining P2 utility surfaces as translated into opt-in Forge contracts, with validation and release metadata for 1.31.0.
