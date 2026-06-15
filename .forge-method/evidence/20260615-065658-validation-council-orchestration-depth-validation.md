# Council Orchestration Depth validation

- kind: validation
- created_at: 2026-06-15T06:56:58+00:00
- checks: python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py workflow validate | python skills/forge-method/scripts/forge_method_runtime.py workflow compactness | python skills/forge-method/scripts/forge_method_runtime.py parity replay | python skills/forge-method/scripts/forge_method_runtime.py config validate --root . | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Summary

Validated Council Orchestration Depth: council-decision now has dedicated pack/template/modes, party-mode Guidance Engine routing, richer live debate output, compact dissent/orchestration artifact, JSON worker/merge contract, replay coverage, workflow compactness, runtime smoke, fast verification, and install smoke.
