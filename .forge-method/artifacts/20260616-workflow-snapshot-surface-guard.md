# Workflow Snapshot Surface Guard

- status: implemented
- phase: 6-evolve
- workflow: runtime-builder
- scope: workflow validation errors exposed through snapshot quality

## Problem

`workflow validate` and `gate` already consumed workflow reference, workflow catalog, facilitation pack, and template validation errors. Runtime snapshots did not expose that same surface, so agents using `snapshot`, `resume --json`, or context packs could see a clean quality object while workflow validation would still fail later.

## Contract

- `workflow_validation_errors(root)` remains the canonical workflow validation surface.
- `build_snapshot(root, state)` exposes those errors at `snapshot.quality.workflows.errors`.
- `gate` continues to block on the same workflow validation surface.
- Future agents can inspect compact snapshot quality before trusting workflow, catalog, facilitation, or template state.

## Implementation Notes

- Added `workflow_errors = workflow_validation_errors(root)` to snapshot construction.
- Added `quality.workflows.errors` to the machine-readable snapshot.
- Added a regression test proving a malformed project-local workflow fails `workflow validate`, appears in snapshot quality, and blocks `gate`.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_snapshot_uses_workflow_validation_surface tests.test_runtime.RuntimeTests.test_gate_and_snapshot_use_builder_extension_validation_surface -v`: passed
- `python -m unittest discover -s tests`: 123 tests passed
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
