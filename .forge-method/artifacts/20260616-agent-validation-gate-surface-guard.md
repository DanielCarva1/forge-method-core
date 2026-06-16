# Agent Validation Gate Surface Guard

- status: implemented
- phase: 6-evolve
- workflow: runtime-builder
- scope: agent validation surface used by quality gate and builder validation

## Problem

`agent validate` checked agent profiles, elicitation techniques, and Persona Lens overlays. The quality gate only checked agent profiles, so it could print `Agents: passed` while the dedicated agent validation command would fail on a broken technique or Persona Lens.

## Contract

- `agent_validation_errors(root)` is the canonical agent validation surface.
- `agent validate`, `builder validate`, runtime snapshots, and `gate` consume the same agent validation function.
- Persona Lens and elicitation technique failures now block the quality gate before a ready/release claim can say agent guidance is valid.

## Implementation Notes

- Added `agent_validation_errors(root)` to combine profile, elicitation technique, and Persona Lens validation.
- Replaced duplicated or narrower call sites with the shared function.
- Added a regression test proving `cmd_gate` fails when the full agent validation surface reports a technique error.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_gate_uses_full_agent_validation_surface -v`: passed
- `python -m unittest discover -s tests`: 120 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py agent validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 22/22 evals passed

## Next

Continue the post-parity Forge audit by checking remaining command-specific validation surfaces that can still diverge from `gate`, `audit`, snapshots, or installed smoke coverage.
