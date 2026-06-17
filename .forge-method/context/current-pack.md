# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-in-progress
- workflow: agent-analyze
- active_story: <none>
- next_action: Decide and/or implement P2 parity surfaces, then rerun focused replay plus relevant install/runtime validation before any release claim.

## Latest Checkpoint

# Current systematic parity audit checkpoint

- created_at: 2026-06-17T18:18:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-in-progress
- workflow: agent-analyze
- active_story: <none>

## Summary

Current systematic parity audit advanced with external source snapshot, Forge inventory, registered audit artifact, release/version skepticism routing patch, replay fixture, and state runtime_version corrected to 1.30.0.

## Decisions

- Do not mark full parity objective complete while P2 surfaces remain deferred: isolated eval runner, hook/event wrapper surface, generic API/browser utility layer.

## Checks

- python -m unittest discover -s tests: 126 tests passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -SkipUnit: passed
- python skills/forge-method/scripts/forge_method_runtime.py parity replay --json: 97/97 passed
- audit and gate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- CHANGELOG.md
- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md

## Artifacts

- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md
- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md

## Next Action

Decide and/or implement P2 parity surfaces, then rerun focused replay plus relevant install/runtime validation before any release claim.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- README.md
- docs/00-quickstart.md
- skills/forge-method/facilitation/*.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260617-004811-validation-installed-forge-guidance-and-verification-sync.md
- .forge-method/evidence/20260617-014953-validation-guided-experience-stress-audit.md
- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md
- .forge-method/evidence/20260617-175907-validation-validate-published-version-1-30-0.md
- .forge-method/evidence/20260617-181658-validation-current-systematic-parity-audit-and-release-guid.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Route Diagnostics Recovery Index Changelog - Unreleased notes record persisted route diagnostics in recovery briefs and capability index.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for Route Diagnostics Recovery Index - Regenerated compact capability index with route_diagnostics surfaces for guide, resume, next, and context recovery.
- runtime-builder [queued/durable]: .forge-method/artifacts/20260616-post-parity-functionality-experience-audit.md - Post-Parity Functionality And Experience Audit - Post-parity audit contract covering transitions, helpers, automation scripts, area detection, human guidance, and agent runtime behavior.
- correct-course [active/durable]: .forge-method/artifacts/20260617-005824-correct-course-correct-course-continuation.md - Correct-course continuation - Human-guidance audit found that Forge claims near parity but real first-run transcripts still feel checklist-like and too accelerated compared with BMAD guided facilitation.

Impact: Human experience trust is damaged when broad early ideas are compressed into artifacts before enough brain dump, research, taste calibration, drift recovery, and energy matching are exercised..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Benchmark BMAD Help and guided flows, stress test Forge installed plugin across happy path, rushed user, confused user, misleading correction, stale/drift state, and frustrated energy; patch only concrete gaps and prove with focused and full validation..
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md - Current systematic parity completion audit - Current external-to-Forge parity audit for Forge Method 1.30.0. Records translated/proved families, deferred P2 surfaces, and the release/version validation routing patch.
