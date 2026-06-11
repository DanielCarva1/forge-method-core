# Repository Instructions

Forge Method Core is a Codex-native runtime. Keep product-facing docs independent: do not describe the product as a clone, fork, or variant of another framework.

Runtime state is file-backed. Behavior that changes project progress must update `.forge-method/state.yaml`, `sprint.yaml`, story files, evidence, or `ledger.ndjson`.

Validation expectations:

- Run `python -m unittest discover -s tests` after changing runtime code.
- Run `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1` after changing workflows or state transitions.
- Run `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1` after changing install or skill packaging.
- Use `scripts/verify-fast.ps1` or `scripts/verify-fast.sh` for normal development validation.
- Use `scripts/verify-all.ps1` or `scripts/verify-all.sh` only before a release, after install/package changes, or when a broad runtime change needs full coverage.
- Versioning may happen per story when the project is intentionally being delivered story by story. When multiple completed stories already form one coherent product increment, batch them before tagging or creating a GitHub release.
- Do not run `release check` after every small change or every intermediate commit. During active development, batch related work, use fast or targeted validation, then run `release check` once when the batch is actually ready to tag/publish.

Agent-facing workflow docs should stay compact state machines with `trigger`, `inputs`, `steps`, `outputs`, `done_when`, `blocked_when`, and `handoff`.
