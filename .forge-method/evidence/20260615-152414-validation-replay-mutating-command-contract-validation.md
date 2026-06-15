# Replay Mutating Command Contract validation

- kind: validation
- created_at: 2026-06-15T15:24:14+00:00
- checks: python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py parity replay | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Summary

Validated mutating command replay contract: targeted negative tests passed, unittests 87/87, parity replay 90/90, smoke-runtime, verify-fast, smoke-install with installed replay 90/90, and artifact verify passed.
