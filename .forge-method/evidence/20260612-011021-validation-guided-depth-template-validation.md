# Guided-depth template validation

- kind: validation
- created_at: 2026-06-12T01:10:21+00:00
- story: guided-depth-templates-p1
- checks: workflow validate: passed; python -m unittest discover -s tests: passed; scripts/verify-fast.ps1: passed; scripts/smoke-runtime.ps1: passed; scripts/smoke-install.ps1: passed; gate --require-evals: passed

## Summary

Validated optional workflow catalog template references and family artifact templates for game lifecycle, test architecture, builder utility, and document utility guided-depth workflows.
