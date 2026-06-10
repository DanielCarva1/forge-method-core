# Forge Method Core

Forge Method is a Codex-native creation runtime: a state-machine system for turning intent into artifacts, implementation, validation, release, and future evolution.

It is built around Codex primitives:

- skills for compact workflow loading
- plugins for distribution
- file-backed state instead of chat memory
- scripts for deterministic project status and transitions
- subagents for focused review, QA, research, architecture, and creative roles
- evidence logs for implementation and validation

This repository is the core runtime and distribution package.

Current runtime version: `1.4.0`.

## Current Shape

```txt
.codex-plugin/plugin.json          Codex plugin manifest
skills/forge-method/SKILL.md       Main runtime skill
skills/forge-method/modules/        Packaged module manifests
skills/forge-method/references/    Compact state-machine workflows
skills/forge-method/scripts/       Deterministic runtime helpers
docs/                              Product and architecture proposal
templates/                         Project state templates
examples/                          Minimal initialized project example
install.ps1                        User-skill installer for Windows
install.sh                         User-skill installer for macOS/Linux
```

## Local Skill Install

For a simple install without a plugin marketplace on Windows:

```powershell
git clone <repo-url>
cd forge-method-core
.\install.ps1
```

On macOS/Linux:

```bash
git clone <repo-url>
cd forge-method-core
bash install.sh
```

That copies the skill to:

```txt
%USERPROFILE%\.agents\skills\forge-method
```

After that, use it in Codex by mentioning:

```txt
$forge-method
```

or by asking Codex to start Forge Method in a workspace.

## Runtime Commands

The skill can ask Codex to run:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" start --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" init --project my-project
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" version
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" next
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" audit
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" checkpoint --summary "What changed and what to do next"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" artifact verify --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" module list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" workflow validate
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" workflow create --root . --id custom-flow --title "Custom Flow"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" eval run --root .
```

The script creates `.forge-method/` in the target project and keeps state out of the chat transcript.

## Smoke Tests

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
```

```bash
bash scripts/smoke-runtime.sh
bash scripts/smoke-install.sh
```

Full local verification:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-all.ps1
```

```bash
bash scripts/verify-all.sh
```

## Product Direction

The complete proposal is in:

- `docs/01-product-proposal.md`
- `docs/02-runtime-architecture.md`
- `docs/03-v1-roadmap.md`
- `docs/05-v1-operating-model.md`
- `docs/06-runtime-improvement-backlog.md`

## Example

See `examples/hello-method` for a minimal initialized project with `.forge-method/state.yaml`.
