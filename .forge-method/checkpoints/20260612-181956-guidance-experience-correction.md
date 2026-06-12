# Guidance experience correction

- created_at: 2026-06-12T18:19:56+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: evolve-project
- active_story: <none>

## Summary

Fixed the Forge human guidance failure exposed by the transcript: new projects no longer start with ready stories, method-experience criticism routes to correct-course, and autonomous mechanical resume now recommends Codex Goal handoff whenever resume is autonomous.

## Decisions

- Initial project creation must gate on human facilitation before stories, architecture, or build.
- Criticism that Forge skipped facilitation or failed the guided experience must route to correct-course before runtime-builder.
- Runtime-builder remains the repair path after the failed behavior is named and recorded.

## Checks

- python -m unittest discover -s tests: passed 59 tests
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- tests/fixtures/guidance_transcripts.json
- .forge-method/artifacts/guidance-engine-benchmark.md
- docs/adr/0008-guidance-engine.md

## Artifacts

- .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md
- .forge-method/evidence/20260612-181924-validation-guidance-experience-correct-course-validation.md

## Next Action

Use /forge-reload in a fresh project to verify the human-facing opening and initial facilitation feel right in live use.
