# Discovery Closeout Generator Contract

- kind: runtime-guidance-contract
- created_at: 2026-06-15T20:15:00+00:00
- owner_workflow: runtime-builder

## Contract

Agents no longer need to hand-write the accepted discovery closeout markdown.

Use:

```powershell
python skills/forge-method/scripts/forge_method_runtime.py artifact discovery-closeout --root <project> --audience "<audience>" --outcome "<outcome>" --constraints "<constraints>" --non-goals "<non-goals>" --success-signal "<success signal>"
```

The command:

- reads answered `initial-facilitation` by default
- writes `.forge-method/artifacts/discovery-intent.md`
- registers it as a durable `discovery-intent` artifact
- validates it with the same `artifact discovery-check` contract used by phase transition
- prints the artifact path and next workflow

## Metadata

`discover-intent` now advertises `discovery-closeout-artifact` as its template, and `workflow-discover-intent.md` names the generator plus `artifact discovery-check` as the canonical handoff before specification.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v`
- `python skills/forge-method/scripts/forge_method_runtime.py workflow validate`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay`
- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next

Continue post-parity Forge polish by improving the human-facing discovery closeout guidance so the generator arguments can be derived from a guided conversation cleanly.
