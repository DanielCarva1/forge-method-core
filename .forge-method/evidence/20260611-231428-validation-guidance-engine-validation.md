# Guidance Engine validation

- kind: validation
- created_at: 2026-06-11T23:14:28+00:00
- checks: python -m unittest discover -s tests | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Summary

Validated Guidance Engine redesign and reload packaging with python -m unittest discover -s tests, smoke-runtime.ps1, verify-fast.ps1, and smoke-install.ps1.
