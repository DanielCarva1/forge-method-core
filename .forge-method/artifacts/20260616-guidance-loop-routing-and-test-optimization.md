# Guidance Loop Routing And Test Optimization

- created_at: 2026-06-16T00:58:07+00:00
- status: guidance-loop-tests-optimized
- workflow: runtime-builder
- lifecycle: durable

## Problem

The next optimization prompt exposed a Guidance Engine nuance bug: "convert only tests" was routed to `skill-convert`, even though the human intent was runtime test optimization. That is the same class of defect Forge is meant to prevent: literal keyword routing overriding context.

Several lifecycle/game/TEA tests also still spawned `guide --json` and setup commands through subprocesses for contract assertions that can be checked directly against the runtime API.

## Decision

- Restrict `skill-convert` routing to conversion of skills, workflows, modules, agents, BMAD/source material, or prompts.
- Add a parity replay fixture proving test-loop optimization wording remains on `runtime-builder`.
- Convert lifecycle, game, game mechanical build, and TEA guidance contract loops to direct `build_guide_payload` plus direct replay state setup.
- Keep CLI coverage where CLI behavior is the point: `parity replay`, smokes, config index, and focused human-output guide tests.

## Result

- `parity replay`: 91/91 passed after adding the false-positive regression fixture.
- Focused loop tests now avoid subprocess guide/setup paths:
  - lifecycle closure guidance: 8.495s
  - game studio depth: 2.327s
  - game dev story mechanical route: 0.357s
  - TEA depth: 1.778s
- Full unittest passed: 99 tests in 244.008s.
- `verify-fast.ps1` passed with unittest portion at 205.593s.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_lifecycle_closure_guidance_and_compact_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_game_studio_depth_guidance_and_compact_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_game_dev_story_routes_to_mechanical_build_when_ready -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_tea_depth_guidance_and_compact_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_engine_routes_transcript_fixtures -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_parity_replay_fixture_covers_required_families -v`
- `python -m unittest discover -s tests`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next Gap

Continue optimizing remaining targeted guide subprocess tests only where direct runtime contracts preserve the behavior under test. Keep enough CLI tests to prove installed and source command behavior.
