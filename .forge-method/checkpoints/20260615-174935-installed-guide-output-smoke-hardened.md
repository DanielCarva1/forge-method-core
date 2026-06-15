# Installed guide output smoke hardened

- created_at: 2026-06-15T17:49:35+00:00
- project: forge-method-core
- phase: 6-evolve
- status: installed-guide-output-smoke-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Extended install smoke so the packaged Forge runtime is tested against the live non-JSON guide contract: installed guide must print Guidance and First question lines for a real initialized project start and must not regress to the old Prompt blob.

## Decisions

- Installed smoke is now responsible for proving that the distributed skill preserves the richer human guidance surface, not just JSON parity replay.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .
- python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals

## Failed Checks

- none

## Touched Files

- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- .forge-method/evidence/20260615-174911-validation-installed-guide-output-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-installed-guide-output-smoke-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing generated project open/reload selection and first-run facilitation prompts against the richer human guidance contract.
