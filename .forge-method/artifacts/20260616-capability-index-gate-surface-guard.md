# Capability Index Gate Surface Guard

- status: implemented
- phase: 6-evolve
- workflow: runtime-builder
- scope: written capability index validation used by config validate, snapshot, and quality gate

## Problem

`config index --write` creates `.forge-method/context/capability-index.json`, a compact agent-facing map of workflows, modules, agents, persona lenses, elicitation techniques, custom capabilities, and project conventions. The generated payload was validated at write time, but an existing written file could later become stale or misleading without blocking `config validate`, snapshot quality, or `gate`.

## Contract

- `config_override_validation_errors(root)` validates team/local config override files.
- `capability_index_validation_errors(root)` validates the written capability index when it exists.
- `config_validation_errors(root)` combines both surfaces for `config validate`, snapshot quality, builder validation, and `gate`.
- `config index --write` remains the repair path: it validates overrides, regenerates the compact index, and can overwrite a stale or broken written index.

## Implementation Notes

- Added normalization that ignores generated metadata such as `generated_at` and `written_path` before comparing the written index with the current generated payload.
- Added guidance-safety validation for `.forge-method/context/capability-index.json`.
- Added stale-index detection that tells the agent to regenerate with `config index --write`.
- Added regression coverage proving config validation, snapshot quality, and gate all consume the written capability index surface.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_written_capability_index_is_validated_by_config_snapshot_and_gate -v`: passed
- `python -m unittest tests.test_runtime.RuntimeTests.test_project_config_override_model_and_capability_index_contracts tests.test_runtime.RuntimeTests.test_config_validation_rejects_misleading_runtime_guidance_text tests.test_runtime.RuntimeTests.test_agent_profile_validation_rejects_misleading_runtime_guidance_text tests.test_runtime.RuntimeTests.test_gate_and_snapshot_use_builder_extension_validation_surface -v`: passed
- `python -m unittest discover -s tests`: 122 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py agent validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py config validate --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py builder validate --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 22/22 evals passed

## Next

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.
