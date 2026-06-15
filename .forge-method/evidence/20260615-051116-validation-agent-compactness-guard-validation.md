# Agent Compactness Guard validation

- kind: validation
- created_at: 2026-06-15T05:11:16+00:00
- checks: python -m unittest discover -s tests: 71 tests OK | workflow validate: passed | workflow compactness: passed | parity replay: 63/63 passed | config validate --root .: passed | smoke-runtime.ps1: passed | verify-fast.ps1: passed | smoke-install.ps1: passed | artifact verify --root .: passed | audit --root .: passed

## Summary

Validated Agent Compactness Guard: workflow refs and facilitation packs now have deterministic progressive-disclosure checks, workflow compactness exposes the contract, workflow validate and audit enforce it, and smoke-runtime runs the guard.
