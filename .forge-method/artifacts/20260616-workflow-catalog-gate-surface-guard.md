# Workflow Catalog Gate Surface Guard

- status: implemented
- phase: 6-evolve
- workflow: agent-analyze
- scope: workflow validation surface used by quality gate

## Problem

`workflow validate` checked the packaged workflow catalog, but the shared `workflow_validation_errors()` function used by `gate` did not. That left a gap where catalog-level mistakes, such as a missing template referenced by workflow metadata, could pass the quality gate if the explicit workflow command was not run separately.

## Contract

- `workflow_validation_errors()` is the canonical workflow validation surface for gate usage.
- The quality gate now receives workflow file, catalog, facilitation pack, and template-reference errors from the same path.
- A broken catalog route should fail workflow validation before a release or ready gate can claim workflow proof.

## Implementation Notes

- Added `validate_workflow_catalog(root)` to `workflow_validation_errors()`.
- Added a focused regression test with a temporary catalog entry that references a missing template.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_workflow_validation_errors_include_catalog_surface -v`: passed
- `python -m unittest discover -s tests`: 119 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 22/22 evals passed

## Next

Continue the post-parity Forge audit by checking remaining packaged surfaces where command-specific validation and gate/audit validation may still differ.
