# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-in-progress
- workflow: agent-analyze
- active_story: <none>
- next_action: Commit and push the v1.31.0 parity closure batch; after the worktree is clean, rerun release check before tagging or publishing.

## Latest Checkpoint

# v1.31.0 P2 parity utility surfaces validated

- created_at: 2026-06-17T19:20:16+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-in-progress
- workflow: agent-analyze
- active_story: <none>

## Summary

Implemented and validated opt-in runtime utility parity surfaces: isolated eval runner, hook/event plan, API/browser utility, runtime routing, replay fixtures, templates, scripts, release notes, and version metadata.

## Decisions

- Ship this as the 1.31.0 parity closure/runtime utility batch, not as part of the already published 1.30.0 scoped guidance release.

## Checks

- python -m unittest discover -s tests: 126 tests passed
- workflow/config/audit validation: passed
- parity replay: 100/100 passed
- smoke-runtime and smoke-install: passed
- release metadata aligned to 1.31.0; release check only blocked by dirty worktree before commit

## Failed Checks

- none

## Touched Files

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

## Artifacts

- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Commit and push the v1.31.0 parity closure batch; after the worktree is clean, rerun release check before tagging or publishing.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/*.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-isolated-eval-runner.md
- skills/forge-method/references/workflow-hook-event-plan.md
- skills/forge-method/references/workflow-api-browser-utility.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-014953-validation-guided-experience-stress-audit.md
- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md
- .forge-method/evidence/20260617-175907-validation-validate-published-version-1-30-0.md
- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md
- .forge-method/evidence/20260617-192015-validation-validation-v1-31-0-p2-parity-utility-surfaces.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Route Diagnostics Recovery Index Changelog - Unreleased notes record persisted route diagnostics in recovery briefs and capability index.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for Route Diagnostics Recovery Index - Regenerated compact capability index with route_diagnostics surfaces for guide, resume, next, and context recovery.
- runtime-builder [queued/durable]: .forge-method/artifacts/20260616-post-parity-functionality-experience-audit.md - Post-Parity Functionality And Experience Audit - Post-parity audit contract covering transitions, helpers, automation scripts, area detection, human guidance, and agent runtime behavior.
- correct-course [active/durable]: .forge-method/artifacts/20260617-005824-correct-course-correct-course-continuation.md - Correct-course continuation - Human-guidance audit found that Forge claims near parity but real first-run transcripts still feel checklist-like and too accelerated compared with BMAD guided facilitation.

Impact: Human experience trust is damaged when broad early ideas are compressed into artifacts before enough brain dump, research, taste calibration, drift recovery, and energy matching are exercised..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Benchmark BMAD Help and guided flows, stress test Forge installed plugin across happy path, rushed user, confused user, misleading correction, stale/drift state, and frustrated energy; patch only concrete gaps and prove with focused and full validation..
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
