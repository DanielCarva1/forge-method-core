# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: versioned-release-ready
- workflow: ready-release
- active_story: <none>
- next_action: Push branch, merge, tag v1.30.0, then run clone/install smoke from the published ref.

## Latest Checkpoint

# 1.30.0 guided human experience versioned

- created_at: 2026-06-17T04:56:52+00:00
- project: forge-method-core
- phase: 6-evolve
- status: versioned-release-ready
- workflow: ready-release
- active_story: <none>

## Summary

Bumped Forge Method Core to 1.30.0 for the guided human experience increment, added release notes/latest metadata/marketplace listing updates, reran full source tests and install/runtime smokes, and updated state to ready-release.

## Decisions

- Treat this as a minor batch release because release check in batch mode selected the next minor version for the new guided human experience increment.

## Checks

- python -m unittest discover -s tests: passed 126 tests
- verify-fast -SkipUnit: passed
- audit: passed
- install-plugin-local: passed
- smoke-runtime: passed
- smoke-install: passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- release-notes/1.30.0.md
- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md

## Next Action

Push branch, merge, tag v1.30.0, then run clone/install smoke from the published ref.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/artifacts/20260616-post-parity-functionality-experience-audit.md
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- README.md
- docs/00-quickstart.md
- skills/forge-method/facilitation/*.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-205937-validation-guidance-correct-course-precedence-and-focused-v.md
- .forge-method/evidence/20260616-224427-validation-full-verification-runtime-optimization.md
- .forge-method/evidence/20260617-004811-validation-installed-forge-guidance-and-verification-sync.md
- .forge-method/evidence/20260617-014953-validation-guided-experience-stress-audit.md
- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md

## Recent Artifacts

- runtime-builder [active/durable]: .forge-method/artifacts/20260616-route-diagnostics-recovery-index.md - Route Diagnostics Recovery Index - Recovery briefs and capability index now preserve Help Oracle route diagnostics for future agents after reload or context recovery.
- changelog [active/durable]: CHANGELOG.md - Route Diagnostics Recovery Index Changelog - Unreleased notes record persisted route diagnostics in recovery briefs and capability index.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index refreshed for Route Diagnostics Recovery Index - Regenerated compact capability index with route_diagnostics surfaces for guide, resume, next, and context recovery.
- runtime-builder [queued/durable]: .forge-method/artifacts/20260616-post-parity-functionality-experience-audit.md - Post-Parity Functionality And Experience Audit - Post-parity audit contract covering transitions, helpers, automation scripts, area detection, human guidance, and agent runtime behavior.
- correct-course [active/durable]: .forge-method/artifacts/20260617-005824-correct-course-correct-course-continuation.md - Correct-course continuation - Human-guidance audit found that Forge claims near parity but real first-run transcripts still feel checklist-like and too accelerated compared with BMAD guided facilitation.

Impact: Human experience trust is damaged when broad early ideas are compressed into artifacts before enough brain dump, research, taste calibration, drift recovery, and energy matching are exercised..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Benchmark BMAD Help and guided flows, stress test Forge installed plugin across happy path, rushed user, confused user, misleading correction, stale/drift state, and frustrated energy; patch only concrete gaps and prove with focused and full validation..
