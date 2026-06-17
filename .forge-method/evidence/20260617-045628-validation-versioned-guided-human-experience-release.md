# Versioned guided human experience release

- kind: validation
- created_at: 2026-06-17T04:56:28+00:00
- checks: python -m unittest discover -s tests: passed 126 tests | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -SkipUnit: passed | python skills\forge-method\scripts\forge_method_runtime.py audit --root .: passed | powershell -ExecutionPolicy Bypass -File .\scripts\install-plugin-local.ps1: installed 1.30.0 plugin locally | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1: passed | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1: passed

## Summary

Versioned the guided human experience increment as 1.30.0 with release notes, marketplace listing metadata, latest release metadata, runtime/plugin version surfaces, and validation coverage for source and installed package behavior.
