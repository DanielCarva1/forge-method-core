# Generated project open reload smoke validation

- kind: validation
- created_at: 2026-06-15T18:03:31+00:00
- checks: powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Summary

Validated source and installed smokes now fail if generated projects skip initial facilitation or if parent workspace preflight/reload stops presenting explicit project selection.
