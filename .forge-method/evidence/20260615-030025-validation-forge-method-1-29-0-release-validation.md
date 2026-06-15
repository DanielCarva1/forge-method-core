# Forge Method 1.29.0 release validation

- kind: validation
- created_at: 2026-06-15T03:00:25+00:00
- checks: python -m unittest discover -s tests: 70 tests OK | scripts/verify-onboarding-assets.py: passed | workflow validate: passed | parity replay: 58/58 passed | verify-all.ps1: passed | smoke-fixtures.ps1: passed after launch-ops decision-source fix | artifact verify: passed | audit: passed | config validate: passed

## Summary

Validated Forge Method Core v1.29.0 guided workflow depth release batch, including version metadata, onboarding assets, unit tests, workflow validation, parity replay, runtime/install/local/fixture smokes, artifact verification, audit, and config validation. Also fixed launch-ops example seeding so build/verify examples include a decision-source validation map.
