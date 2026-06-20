# Forge Update Migration Hotfix Work Order

## Summary

Forge Method Core 1.34.1 fixes `$forge-update` for real user installs. The update skill must not make users understand install shape before they can update. It should detect the current package shape, try the supported Codex marketplace update path, fall back to refreshing `main`, compare versions, and print human patch notes.

## Decisions

- Manual `$forge-update` may migrate legacy or local installs through `codex plugin marketplace add DanielCarva1/forge-method-core --ref main`.
- If `codex plugin marketplace upgrade` fails, `$forge-update` tries the same main-package refresh before reporting failure.
- Patch notes may be fetched from the public `release-notes/latest.json` feed so migration can summarize changes even when the current install is old.
- Failed migration must be non-destructive and must print the exact manual command.

## Validation Plan

- `python -m unittest -v tests.test_updater`
- `python scripts\test-runner.py --workers 4 --timeout 120 --report .forge-method\test-runs\manual.json`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Release Target

Version `1.34.1`.
