# State Guidance Write Guard

Date: 2026-06-16
Workflow: runtime-builder
Phase: 6-evolve

## Audit Finding

The runtime already validated several emitted guidance payloads, but durable state itself could still be written with misleading agent-facing guidance in fields that later feed preflight, resume, snapshot, Help Oracle, context packs, and checkpoints.

The important boundary is `.forge-method/state.yaml`: future agents read it before broader context, so state fields that describe the next action or route reason must be treated as runtime guidance, not ordinary metadata.

## Change

`write_state` now validates guidance-bearing state fields before persisting:

- `next_action`
- `last_route_reason`
- `guide_summary`

`audit_project` also validates those fields so preexisting state files are caught by `audit` and `gate --require-evals`.

The scan remains narrow by design. Project names, status values, workflow IDs, and other identifiers are not scanned as prose guidance.

## Regression Proof

New and updated tests:

- `test_state_guidance_safety_rejects_misleading_next_action_write`
- `test_audit_rejects_preexisting_misleading_state_guidance`
- `test_audit_rejects_unsafe_help_oracle_next_action` now simulates legacy contaminated state directly because `transition` correctly refuses to write it.

Validation passed:

- `python -m unittest discover -s tests` - 110 tests
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- source parity replay - 91/91
- workflow validate
- workflow compactness
- artifact verify
- audit
- gate --require-evals - 20/20

## Handoff

Continue the broader audit by looking for runtime outputs that compose durable user/project data with agent guidance and should either validate the final payload or keep user data clearly outside the guidance scan.
