# Reload emergency entrypoint validation

- kind: validation
- created_at: 2026-06-11T20:59:34+00:00
- checks: python -m unittest discover -s tests passed | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 passed | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 passed

## Summary

Implemented runtime reload command and forge-reload skill for stale chat recovery; validated runtime tests, fast verification, onboarding assets, and install smoke.
