# Installed guide output smoke validation

- kind: validation
- created_at: 2026-06-15T17:49:11+00:00
- checks: powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root . | python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals

## Summary

Validated that the installed Forge package now fails smoke-install if non-JSON guide output does not expose Guidance and First question lines during a real initialized project start.
