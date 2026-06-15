# Replay Auxiliary Guidance Contract validation

- kind: validation
- created_at: 2026-06-15T14:57:21+00:00
- checks: python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py parity replay | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Summary

Validated auxiliary guidance replay contract: unittests 85/85, parity replay 90/90, smoke-runtime, verify-fast, smoke-install with installed replay 90/90, and artifact verify passed.
