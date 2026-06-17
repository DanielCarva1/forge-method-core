# Validation: v1.31.1 public install core-state guard

- kind: validation
- created_at: 2026-06-17T20:50:38+00:00
- checks: python -m unittest discover -s tests: 127 tests passed | installed-package simulation with committed state and BOM/no-BOM plugin manifest: passed | workflow validate, config validate, audit: passed | parity replay: 100/100 passed | verify-onboarding-assets: passed | smoke-runtime: passed | smoke-install: passed

## Summary

Hotfix hides committed Forge core state when running from an installed public package, requires maintainer intent for core runtime state, and tolerates UTF-8 BOM in plugin manifests. Validated unit suite, targeted installed-package simulation with and without BOM, workflow/config/audit/parity, onboarding assets, runtime smoke, and install smoke.
