# Guidance Replay Test Optimization

- created_at: 2026-06-16T00:35:13+00:00
- status: guidance-replay-test-optimized
- workflow: runtime-builder
- lifecycle: durable

## Problem

The Guidance Engine replay fixture test protected BMAD parity behavior, but it did so by spawning `guide --json` once per fixture. With 90 cases, that made normal unittest and `verify-fast` feedback much slower than the behavior under test required.

## Decision

Use the runtime's canonical replay contract directly in `tests/test_runtime.py`:

- `prepare_parity_replay_state`
- `build_guide_payload`
- `parity_case_failures`
- `print_guidance_engine_summary` for the text-summary regression case

This keeps the same fixture matrix, the same route assertions, and the same parity failure rules while removing per-case process startup overhead. The CLI path remains covered by `parity replay`, source/install smokes, and focused guide output tests elsewhere.

## Result

- `test_guidance_engine_routes_transcript_fixtures`: about 238s before, 6.351s after.
- `python -m unittest discover -s tests`: 99 tests passed in 259.381s after this change.
- `verify-fast.ps1`: passed in 217.6s wall time, with unittest portion at 214.861s.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_engine_routes_transcript_fixtures -v`
- `python -m unittest discover -s tests`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next Gap

Continue performance cleanup by profiling the remaining subprocess-heavy guide loops and deciding whether each protects CLI behavior or can safely use direct runtime calls.
