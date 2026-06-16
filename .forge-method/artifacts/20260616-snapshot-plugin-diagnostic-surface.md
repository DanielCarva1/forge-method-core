# Snapshot Plugin Diagnostic Surface

## Gap

`doctor --json` could detect stale or broken local plugin installation state, but `snapshot --root .` did not expose that diagnosis.

That meant future agents relying on the compact snapshot could miss an installed plugin mismatch and continue debugging guidance behavior as if runtime state were the only source of truth.

## Change

`snapshot` now includes:

```json
"diagnostics": {
  "plugin_installation": {
    "available": false,
    "status": "plugin version mismatch",
    "expected_version": "1.29.0",
    "installed_version": "1.22.0",
    "plugin_path": "...",
    "skill_exists": true,
    "codex_deeplink": "...",
    "repair_commands": {
      "windows": ["powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1"],
      "posix": ["bash scripts/install-plugin-local.sh"]
    }
  }
}
```

This is intentionally diagnostic, not a quality gate blocker. A project can still be valid while the local Codex plugin install is stale; the agent just should not miss the repair path.

## Proof

Added an isolated marketplace fixture where the local plugin manifest reports version `1.22.0` while the runtime expects `1.29.0`.

Expected behavior:

- `doctor --json` continues to report `plugin version mismatch`.
- `snapshot` reports the same plugin status under `diagnostics.plugin_installation`.
- `snapshot.quality.audit.passed` remains true.

During validation, the real workspace snapshot reported `plugin version mismatch` with installed version `1.22.0`, proving this surface exposes useful environment state that was previously hidden from the agent.

Validation passed:

- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
