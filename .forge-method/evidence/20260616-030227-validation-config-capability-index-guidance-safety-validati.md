# Config capability index guidance safety validation

- kind: validation
- created_at: 2026-06-16T03:02:27+00:00
- checks: python -m unittest discover -s tests; .\scripts\smoke-runtime.ps1; .\scripts\verify-fast.ps1; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate --require-evals; .\scripts\smoke-install.ps1

## Summary

Passed unittest 108, smoke-runtime, verify-fast, parity replay 91/91, workflow validate, workflow compactness, artifact verify, audit, gate 20/20, and smoke-install after adding deterministic guidance safety validation for config, agent profiles, and capability index output.
