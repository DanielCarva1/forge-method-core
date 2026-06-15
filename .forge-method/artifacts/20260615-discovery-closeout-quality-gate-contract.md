# Discovery Closeout Quality Gate Contract

- kind: runtime-guidance-contract
- created_at: 2026-06-15T19:38:00+00:00
- owner_workflow: runtime-builder

## Contract

`transition --phase 2-specification` now requires more than a durable discovery artifact after `initial-facilitation`.

At least one active durable discovery closeout artifact must pass `artifact discovery-check`. The artifact must preserve:

- `workflow`
- `source_input`
- `audience`
- `outcome`
- `constraints`
- `non_goals`
- `success_signal`
- `open_questions`
- `grill_gate_handoff`
- `next_workflow`

The transition guard rejects weak artifacts that only contain a title and summary.

## Agent Handoff

Agents should run:

```powershell
python skills/forge-method/scripts/forge_method_runtime.py artifact discovery-check --root <project> --path .forge-method/artifacts/discovery-intent.md
```

before moving a generated project from discovery to specification.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay`
- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next

Create a first-class discovery closeout template or generator so agents do not hand-roll the required markdown fields.
