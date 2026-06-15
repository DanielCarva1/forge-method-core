# Discovery Closeout Before Specification Contract

- kind: runtime-guidance-contract
- created_at: 2026-06-15T18:55:00+00:00
- owner_workflow: runtime-builder

## Contract

Generated projects that have answered `initial-facilitation` remain in discovery until the accepted human intent is captured as a durable discovery closeout artifact.

The runtime blocks `transition --phase 2-specification` from `1-discovery` when:

- `initial-facilitation` exists and is answered
- no active durable artifact is classified as `discovery-intent`, `discovery-closeout`, `discovery-brief`, `accepted-intent`, or equivalent path/title hint
- the transition is not explicitly forced

## Reason

The first facilitation answer is raw discovery material. It should not silently become permission to write a spec, create stories, or start implementation. A durable artifact gives the next agent a compact source of truth before specification work begins.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay`
- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next

Continue post-parity Forge polish by auditing the content quality of the discovery closeout artifact and its Grill Gate handoff before specification.
