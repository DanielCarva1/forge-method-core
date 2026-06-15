# Discovery closeout quality gate validation

- kind: validation
- created_at: 2026-06-15T19:49:43+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py parity replay | python -m unittest discover -s tests | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Summary

Validated artifact discovery-check, transition rejection for weak closeouts, transition success for valid closeouts, and installed smoke coverage.
