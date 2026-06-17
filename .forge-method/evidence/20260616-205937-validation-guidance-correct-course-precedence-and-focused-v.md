# Guidance correct-course precedence and focused verify validation

- kind: validation
- created_at: 2026-06-16T20:59:37+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract tests.test_runtime.RuntimeTests.test_guidance_engine_routes_transcript_fixtures | python -m unittest discover -s tests | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Test tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -SkipUnit | bash scripts/verify-fast.sh --skip-unit

## Summary

Guidance Engine now routes human-experience failure complaints about Forge to correct-course before runtime-builder; verify-fast supports focused -Test and -SkipUnit loops. Validated focused guidance tests, full unittest suite, smoke-runtime, verify-fast full, verify-fast -Test, verify-fast -SkipUnit, and bash --skip-unit.
