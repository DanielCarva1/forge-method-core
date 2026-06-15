# Correct-course and problem-solving depth validation

- kind: validation
- created_at: 2026-06-15T07:27:52+00:00
- checks: python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py workflow validate | python skills/forge-method/scripts/forge_method_runtime.py workflow compactness | python skills/forge-method/scripts/forge_method_runtime.py parity replay | python skills/forge-method/scripts/forge_method_runtime.py config validate --root . | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Summary

Added compact templates, catalog modes, richer facilitation packs, Guidance Engine signal/text hardening, and replay fixtures for correction breadth plus stuck diagnostic problem-solving.
