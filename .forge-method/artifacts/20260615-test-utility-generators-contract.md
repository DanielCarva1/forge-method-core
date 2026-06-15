# Test Utility Generators Contract

- created_at: 2026-06-15T23:37:42+00:00
- status: test-utility-generators-added
- workflow: runtime-builder
- lifecycle: durable

## Problem

`test-framework`, `test-automation`, and `game-e2e-scaffold` already had stable validation contracts, but future agents still had to hand-write the artifacts. That left too much room for drift between rich human QA/game guidance and the compact machine handoff expected by downstream workflows.

## Runtime Contract

- `artifact test-framework` creates and registers durable framework artifacts for fixture architecture, runner setup, helper boundaries, cleanup, command contract, risks, and next workflow.
- `artifact test-automation` creates and registers durable automation artifacts for scope, target, setup, commands, assertions, evidence, flake controls, manual remainder, and next workflow.
- `artifact game-e2e-scaffold` creates and registers durable playable smoke artifacts for launch-to-result setup, action, assertion, teardown, evidence mode, readiness gate, and next workflow.
- All three generators reuse `artifact test-check` validation, default missing validation to `artifact test-check --path <artifact>`, rollback invalid generated files, support `--eval`, and register durable artifacts.

## Human Contract

Quality guidance can now move from "you should define this" to concrete command-backed handoff:

- framework decisions become fixture architecture and command contracts;
- automation requests become run-and-fix proof with selectors, assertions, evidence, and flake controls;
- game E2E requests become launch-to-playable-result proof before release readiness.

## Agent Contract

Agent-facing docs stay compact:

- workflow refs name the generator first, then `artifact test-check`;
- facilitation packs keep the richer human questions and quality bars;
- smoke tests cover both source runtime and installed package behavior.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_artifact_test_generators_create_framework_automation_and_game_e2e -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_artifact_test_check_validates_test_automation_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Notes

The full unit suite is now functionally green but slow: `test_guidance_engine_routes_transcript_fixtures` took about 238s alone, and full `unittest discover` took about 655s. Future cleanup should split or cache parity fixture execution so fast verification remains genuinely fast.

## Next Gap

Audit the remaining validator-only utility contracts, especially `enterprise-check` and `doc-check`, and convert only the stable handoffs that improve human guidance or agent reliability.
