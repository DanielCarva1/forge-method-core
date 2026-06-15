# Replay Facilitation Contract validation

- kind: validation
- created_at: 2026-06-15T13:11:08+00:00
- checks: targeted replay fixture tests: 3 OK | parity replay: 89/89 passed | python -m unittest discover -s tests: 80 tests OK | artifact verify --root .: passed | smoke-runtime.ps1: passed | verify-fast.ps1: passed | smoke-install.ps1: passed

## Summary

Strengthened parity replay so human-facing guided cases must assert expected facilitation packs; added missing assertions for help/confusion/correct-course transcripts and a negative replay test.
