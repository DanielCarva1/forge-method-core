# Quickstart

This guide is the shortest path from a fresh clone to a working Forge Method project.

## Prerequisites

- Codex
- Git
- Python 3.12 or newer available as `python`, `python3`, or `py` in a new terminal

## Install

Preferred path: install Forge Method Core as a Codex plugin.

Windows:

```powershell
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
powershell -ExecutionPolicy Bypass -File .\scripts\install-plugin-local.ps1
```

macOS/Linux:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
bash scripts/install-plugin-local.sh
```

Then open Codex plugins, select Forge Method Core, and start from the plugin prompt or skill:

```txt
$forge-method
Start Forge Method in this workspace.
```

The installer prints direct `codex://` links for opening the plugin page and sharing it inside the Codex app. After a first install, start a new Codex thread so the enabled skill list is refreshed. After the self-updating package is installed, normal `$forge-method` starts can self-update the Git marketplace package and continue in the same chat. To update explicitly, run `$forge-update`; it upgrades the Git marketplace package and prints a short feature summary.

When your message includes a question, correction, brainstorm/research request, product/UX planning request, quick-dev request, story lifecycle request, new intent, or build request, `$forge-method` routes it through Guidance Engine after startup. The runtime returns the recommended phase, workflow, action, alternatives, and any state update command before following older state actions.

Fallback skill-only install:

Windows:

```powershell
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
.\install.ps1
```

macOS/Linux:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
bash install.sh
```

## Verify Install

Windows:

```powershell
& "$HOME\.agents\skills\forge-method\forge-method.ps1" --help
& "$HOME\.agents\skills\forge-method\forge-method.ps1" doctor --root . --touches runtime
```

The doctor output should include `Plugin installation` with `Status: ready` after the plugin installer has run. If the status is not ready, follow the printed `Repair:` command from the repo root and run doctor again.

macOS/Linux:

```bash
bash ~/.agents/skills/forge-method/forge-method.sh --help
bash ~/.agents/skills/forge-method/forge-method.sh doctor --root . --touches runtime
```

## Start A Project

Open the folder where projects should live, then ask Codex:

```txt
$forge-method
Start Forge Method in this workspace.
```

Forge Method should run preflight first. It will detect one of these routes:

- existing method project
- parent folder with known projects
- runtime repository
- empty workspace

If the workspace is empty or contains existing method projects, Codex should show preflight decision options. Those options tell the agent whether to open an existing project, choose a workspace outside the runtime repo, or create a new project after the project name and objective are known.

## Expected First Files

A created project gets:

```txt
AGENTS.md
.forge-method/state.yaml
.forge-method/projects.yaml
.forge-method/sprint.yaml
.forge-method/ledger.ndjson
.forge-method/stories/
.forge-method/artifacts/
.forge-method/context/
.forge-method/evidence/
```

## Daily Loop

Use the skill, not chat memory, to resume:

```txt
$forge-method
Resume this project from file state.
```

The agent should run `preflight --root .`, check `context health --root .`, then `resume --root .`, and continue from durable state.

## Release Loop

During development:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
```

For a focused loop, pass one or more unit labels:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Test tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract
```

Use `-SkipUnit` when only lightweight metadata validators are needed.

Before publishing:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-all.ps1
```

Use `release plan` and `release check` to choose version cadence and verify the local release package before tagging.

Full verification includes fixture coverage for every packaged module:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-fixtures.ps1
```

```bash
bash scripts/smoke-fixtures.sh
```

Internal guidance replay can be run directly when changing Guidance Engine routing:

```powershell
python .\skills\forge-method\scripts\forge_method_runtime.py parity replay
```

After publishing a tag, verify the published package can be cloned and installed as a plugin:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v2.0.4 -ExpectedVersion 2.0.4
```

```bash
REF=v2.0.4 EXPECTED_VERSION=2.0.4 bash scripts/smoke-plugin-clone-install.sh
```
