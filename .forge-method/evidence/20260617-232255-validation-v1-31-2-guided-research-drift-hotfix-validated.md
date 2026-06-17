# v1.31.2 guided research drift hotfix validated

- kind: validation
- created_at: 2026-06-17T23:22:55+00:00
- checks: python -m unittest discover -s tests: 128 tests passed | parity replay: 101/101 passed | verify-fast targeted guidance regression: passed | smoke-runtime: passed | smoke-install: passed

## Summary

Fixed guidance drift where standalone app stack/interface/codebase-standard conversations could collapse into generic automation or fast-path. Validated targeted transcript regression, parity replay, verify-fast, smoke-runtime, smoke-install, and full unit suite.
