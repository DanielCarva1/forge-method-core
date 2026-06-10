# Repository Instructions

Forge Method Core is a Codex-native runtime. Keep product-facing docs independent: do not describe the product as a clone, fork, or variant of another framework.

Runtime state is file-backed. Behavior that changes project progress must update `.forge-method/state.yaml`, `sprint.yaml`, story files, evidence, or `ledger.ndjson`.

Validation expectations:

- Run `python -m unittest discover -s tests` after changing runtime code.
- Run `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1` after changing workflows or state transitions.
- Run `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1` after changing install or skill packaging.

Agent-facing workflow docs should stay compact state machines with `trigger`, `inputs`, `steps`, `outputs`, `done_when`, `blocked_when`, and `handoff`.
