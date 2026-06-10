# Forge Method Core

Forge Method is a Codex-native framework for "the thing that creates things": a state-machine runtime for product, software, creative, and game workflows.

It is inspired by BMAD, but designed for Codex primitives:

- skills for compact workflow loading
- plugins for distribution
- file-backed state instead of chat memory
- scripts for deterministic project status and transitions
- subagents for focused review, QA, research, architecture, and creative roles
- evidence logs for implementation and validation

This repository is intentionally a working prototype, not just a document.

## Current Shape

```txt
.codex-plugin/plugin.json          Codex plugin manifest
skills/forge-method/SKILL.md       Main runtime skill
skills/forge-method/references/    Compact state-machine workflows
skills/forge-method/scripts/       Deterministic runtime helpers
docs/                              Product and architecture proposal
templates/                         Project state templates
examples/                          Minimal initialized project example
install.ps1                        Simple user-skill installer for Windows
```

## Local Skill Install

For a simple install without a plugin marketplace:

```powershell
git clone <repo-url>
cd forge-method-core
.\install.ps1
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
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" init --project my-project
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" next
```

The script creates `.forge-method/` in the target project and keeps state out of the chat transcript.

## Smoke Tests

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
```

## Product Direction

The complete proposal is in:

- `docs/01-product-proposal.md`
- `docs/02-runtime-architecture.md`
- `docs/03-mvp-roadmap.md`
- `docs/05-bmad-family-comparison.md`
- `docs/06-runtime-improvement-backlog.md`

## Example

See `examples/hello-method` for a minimal initialized project with `.forge-method/state.yaml`.
