# Story expansion-builder-config-evals done

- kind: story
- created_at: 2026-06-11T05:23:17+00:00
- story: expansion-builder-config-evals
- checks: python -m unittest discover -s tests | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Summary

Implemented builder scaffold/validate, config inspect/validate, eval design workflow, and unit coverage.
