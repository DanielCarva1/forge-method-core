# Product-Facing Docs Independence Guard

- status: implemented
- phase: 6-evolve
- workflow: agent-analyze
- scope: public/product-facing Markdown in the Forge Method runtime repo

## Problem

Forge uses external methods as private benchmark evidence, but product-facing docs must present Forge as its own Codex-native runtime. The repo instruction already said not to describe the product as a clone, fork, or variant of another framework, but that rule depended on human discipline.

## Contract

- `audit` checks product-facing Markdown only when the audited root is the Forge Method runtime repo.
- The guard scans README, AGENTS, docs, examples, templates, skill stubs, and generated-project guidance assets.
- It blocks descriptions that frame Forge as a clone, fork, or variant of BMAD, ZICO, or another framework.
- It allows legitimate Git/install language such as `git clone`, clone/install smoke instructions, and explicit negative policy text such as "do not describe Forge as a clone...".
- It does not apply to user projects created with Forge, because those projects may legitimately describe their own product as a fork.

## Implementation Notes

- Added product-facing doc path discovery and line-level independence checks to the runtime.
- Wired the check into `audit_project` for runtime repo roots only.
- Added tests for unsafe runtime docs, safe Git clone/policy language, and user-project non-interference.

## Validation

- `python -m unittest discover -s tests`: 118 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 22/22 evals passed

## Next

Continue the post-parity Forge audit for dead code and remaining runtime surfaces that still rely on convention instead of deterministic validation.
