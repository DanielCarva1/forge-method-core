# Game Brief Sprint Depth validation

- kind: validation
- created_at: 2026-06-15T11:50:18+00:00
- checks: python -m unittest discover -s tests => 78 tests OK | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 => passed | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 => passed | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 => passed

## Summary

Hardened game brief and game sprint planning guidance with living game-brief artifact, game-sprint-planning workflow, artifact game-check, 88/88 parity replay, workflow compactness, install smoke, runtime smoke, unit suite, and verify-fast.
