# Document Review Depth validation

- kind: validation
- created_at: 2026-06-15T06:34:37+00:00
- checks: python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py workflow validate | python skills/forge-method/scripts/forge_method_runtime.py workflow compactness | python skills/forge-method/scripts/forge_method_runtime.py parity replay | python skills/forge-method/scripts/forge_method_runtime.py config validate --root . | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Summary

Validated Document Review Depth: editorial-review and edge-case-review now have narrow templates, catalog modes, document-utility facilitation, Guidance Engine precedence over generic quality review, parity replay coverage, compactness validation, runtime smoke, fast verification, and install smoke.
