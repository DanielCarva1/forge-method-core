# Config and Capability Index Guidance Safety

Date: 2026-06-16
Workflow: config-customization
Phase: 6-evolve

## Audit Finding

Project configuration already rejected unsupported keys, broken workflow references, missing templates, and invalid capability kinds. It did not apply the runtime guidance safety contract to text fields that become agent-visible guidance through config inspect, guide metadata, agent recommendations, or the generated capability index.

Affected runtime-visible surfaces:

- `convention.<slug>` values
- `project_conventions` and `human_tone`
- workflow override `outputs` and `modes`
- agent override title, purpose, when, inputs, outputs, and handoff
- capability title and summary
- packaged and project agent profile purpose, when, inputs, outputs, and handoff
- final capability index payload before print or write

## Change

The runtime now reuses the existing guidance safety contract for config and agent profile validation. `config index` also validates the final payload before emitting it, so unsafe guidance cannot be generated through a composed index even when individual config files pass structural checks.

The command field remains excluded from this text guidance scan because command strings are executable snippets rather than prose guidance and are already ignored by runtime payload safety.

## Regression Proof

New tests:

- `test_config_validation_rejects_misleading_runtime_guidance_text`
- `test_agent_profile_validation_rejects_misleading_runtime_guidance_text`

Validation passed:

- `python -m unittest discover -s tests` - 108 tests
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --fixture skills\forge-method\fixtures\guidance-parity-replay.json --json` - 91/91
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals` - 20/20
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`

## Handoff

Continue the broader audit by looking for other runtime-visible generated payloads that are composed from several validated sources but do not validate the final payload before emitting it.
