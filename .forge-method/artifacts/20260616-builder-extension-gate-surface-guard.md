# Builder Extension Gate Surface Guard

- status: implemented
- phase: 6-evolve
- workflow: runtime-builder
- scope: local builder extension validation used by snapshot and quality gate

## Problem

`builder validate` checked project-local method skills under `.forge-method/skills/*/SKILL.md` for required frontmatter. The quality gate did not consume that check, so a broken local extension could pass `gate` and later confuse agents that load or validate runtime extensions.

## Contract

- `builder_extension_validation_errors(root)` is the canonical validation surface for local builder extension files.
- `builder validate`, runtime snapshots, and `gate` consume the same builder extension validation function.
- A malformed local skill now appears in `snapshot.quality.builder.errors` and blocks the quality gate.

## Implementation Notes

- Added `builder_extension_validation_errors(root)`.
- Reused it in `cmd_builder_validate`, `build_snapshot`, and `cmd_gate`.
- Added a regression test proving a local skill without frontmatter fails builder validation, appears in snapshot quality, and blocks the gate.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_gate_and_snapshot_use_builder_extension_validation_surface -v`: passed
- `python -m unittest discover -s tests`: 121 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py builder validate --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py agent validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 22/22 evals passed

## Next

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.
