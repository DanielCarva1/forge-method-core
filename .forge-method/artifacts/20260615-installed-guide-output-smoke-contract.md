# Installed Guide Output Smoke Contract

- kind: runtime-install-smoke-contract
- created_at: 2026-06-15T17:48:13Z
- phase: 6-evolve
- workflow: runtime-builder
- status: installed-guide-output-smoke-hardened

## Problem

The source runtime and parity replay protected the richer Guidance Engine output, but the installed `$forge-method` smoke did not fail if a packaged `guide` command regressed back to a generic `Prompt:` blob during a real project start.

## Contract

- `scripts/smoke-install.ps1` captures installed `guide` output after install, reload, init, resume, and start.
- The installed output must include `Guidance: Let's use \`brainstorming\` as the guided path.`
- The installed output must include `First question:`.
- The installed output must not include the old `Prompt: Let's use \`brainstorming\`` shape.

## Proof

- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .`
- `python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals`

## Next

Continue post-parity Forge polish by auditing generated project open/reload selection and first-run facilitation prompts against the richer human guidance contract.
