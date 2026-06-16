# Guidance CLI boundary validation

- kind: validation
- created_at: 2026-06-16T01:16:33+00:00
- checks: python -m unittest discover -s tests: passed, 99 tests in 250.728s | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1: passed, 99 tests plus onboarding, workflow, and agent profile validation

## Summary

Focused tests, full unittest discover, and verify-fast passed after converting JSON-only guide assertions to direct runtime calls.
