# Replay Template Contract validation

- kind: validation
- created_at: 2026-06-15T13:28:18+00:00
- checks: targeted replay template tests: 3 OK | parity replay: 89/89 passed | python -m unittest discover -s tests: 81 tests OK | artifact verify --root .: passed | smoke-runtime.ps1: passed | verify-fast.ps1: passed | smoke-install.ps1: passed

## Summary

Strengthened parity replay so human-facing guided cases must assert expected artifact templates when catalog workflows define them; added missing correct-course template assertion and negative replay test.
