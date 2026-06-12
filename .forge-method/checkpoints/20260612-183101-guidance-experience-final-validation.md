# Guidance experience final validation

- created_at: 2026-06-12T18:31:01+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>

## Summary

Finalized the guided-experience correction. New projects are blocked on initial facilitation instead of ready stories; method-experience criticism is detected as correct-course; correct-course now updates active workflow and route metadata so agents do not inherit stale guidance; mechanical autonomous resume recommends Codex Goal handoff.

## Decisions

- Keep BMAD as internal benchmark evidence only; do not commit sandbox bootstrap files.
- Use correct-course as the first response to method-experience failures, then runtime-builder for repair.

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
- .forge-method/evidence/20260612-183040-validation-guidance-experience-final-validation.md

## Next Action

Use /forge-reload in a fresh project and judge the live first-run facilitation; if it still feels thin, deepen facilitation packs and transcript replay rather than creating stories early.
