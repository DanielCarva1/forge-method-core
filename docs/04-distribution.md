# Distribution Plan

## Goal

A friend should be able to clone the repo, install the runtime, open Codex, and start a method project without understanding the internal architecture.

## Distribution Levels

### Level 1: User Skill Install

This is the current working path.

Windows:

```powershell
git clone <repo-url>
cd forge-method-core
.\install.ps1
```

macOS/Linux:

```bash
git clone <repo-url>
cd forge-method-core
bash install.sh
```

The installer copies:

```txt
skills/forge-method
```

to:

```txt
%USERPROFILE%\.agents\skills\forge-method
```

Then the user invokes:

```txt
$forge-method
```

Acceptance:

- `SKILL.md` is installed.
- workflow references are installed.
- runtime script is installed.
- helper script responds to `--help`.
- helper script resolves startup with `start --root <workspace>`.

### Level 2: Codex Plugin

The repo already contains:

```txt
.codex-plugin/plugin.json
skills/
scripts/
assets/
```

This is the package shape for a Codex plugin. The plugin manifest validates with the Codex plugin-creator validator.

Acceptance:

- `.codex-plugin/plugin.json` validates.
- all skills have valid front matter.
- plugin can be shared or installed through a marketplace-backed flow later.

### Level 3: Project Template

Future path:

```powershell
forge-method init my-project
```

or:

```powershell
npx forge-method init my-project
```

This would create:

```txt
my-project/
  AGENTS.md
  .forge-method/
    state.yaml
    projects.yaml
    sprint.yaml
    ledger.ndjson
    stories/
    artifacts/
    context/
    evals/
    evidence/
    handoffs/
    modules/
    workflows/
```

## Current Verified Commands

From this repository:

```powershell
.\install.ps1
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py --help
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py start --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py init --project smoke-test --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py start --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py status --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py next --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py audit --root <temp-folder>
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
```

```bash
bash install.sh
python ~/.agents/skills/forge-method/scripts/forge_method_runtime.py --help
bash scripts/smoke-runtime.sh
bash scripts/smoke-install.sh
```

## What Still Needs Productization

- marketplace-backed plugin flow
- GitHub PR workflow
