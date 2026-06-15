# Discovery closeout generator validation

- kind: validation
- created_at: 2026-06-15T20:25:04+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v | python -m unittest tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v | python skills/forge-method/scripts/forge_method_runtime.py workflow validate | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py parity replay | python -m unittest discover -s tests | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Summary

Validated artifact discovery-closeout generation, template/catalog metadata, source and installed smoke coverage, parity replay, full unittest, and verify-fast.
