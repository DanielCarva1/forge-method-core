# Distribution Plan

## Goal

A friend should be able to clone the repo, install the runtime, open Codex, and start a method project without understanding the internal architecture.

## Distribution Levels

### Level 1: Codex Plugin

This is the primary product shape.

The repo already contains:

```txt
.codex-plugin/plugin.json
skills/
scripts/
assets/
```

This is the package shape for a Codex plugin. The plugin manifest validates with the Codex plugin-creator validator.

When installed through a plugin-backed flow, the user should choose Forge Method Core in Codex, then start from the plugin prompt or skill:

```txt
$forge-method
Start Forge Method in this workspace.
```

The current validated manifest exposes `defaultPrompt` as:

```txt
Start Forge Method in this workspace.
```

Local personal distribution uses:

```txt
~/.agents/plugins/marketplace.json
~/plugins/forge-method-core
```

Repo or team distribution uses:

```txt
<marketplace-root>/.agents/plugins/marketplace.json
<marketplace-root>/plugins/forge-method-core
```

Register a non-default marketplace root with:

```powershell
codex plugin marketplace add "<marketplace-root>"
```

Workspace sharing and public marketplace listing are separate distribution stages. A validated local plugin can be shared in a workspace through the Codex app; public directory availability requires the external publication/listing process.

The local installer prints Codex deeplinks for the plugin detail page and share flow:

```txt
codex://plugins/forge-method-core?marketplacePath=<encoded-marketplace-json-path>
codex://plugins/forge-method-core?marketplacePath=<encoded-marketplace-json-path>&mode=share
```

Acceptance:

- `.codex-plugin/plugin.json` validates.
- all skills have valid front matter.
- local plugin installer copies the package to `~/plugins/forge-method-core`.
- local plugin installer writes the personal marketplace entry for `forge-method-core`.
- local plugin installer prints Codex plugin detail and share deeplinks.
- clone/install smoke verifies a Git-cloned package can install as a plugin and create a project.
- the plugin default prompt starts the method without requiring internal architecture knowledge.
- the installed skill can run preflight and project creation from file-backed state.
- marketplace metadata can point at this package without changing the runtime surface.

### Level 2: User Skill Install

This is the fallback local install path when a plugin-backed install is not available.

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

The installed skill also includes launchers that resolve Python automatically:

```powershell
& "$HOME\.agents\skills\forge-method\forge-method.ps1" start --root <workspace>
```

```bash
bash ~/.agents/skills/forge-method/forge-method.sh start --root <workspace>
```

Fallback acceptance:

- `SKILL.md` is installed.
- workflow references are installed.
- runtime script is installed.
- runtime launcher is installed.
- helper script responds to `--help`.
- launcher responds to `--help`.
- helper script resolves preflight with `preflight --root <workspace>`.
- helper script resolves startup with `start --root <workspace>`.

### Level 3: Project Template

Current helper path:

```powershell
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py project create --root <parent-folder> --name my-project --module software-builder
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
    checkpoints/
    context/
    evals/
    evidence/
    handoffs/
    modules/
    workflows/
```

## Current Verified Commands

From this repository:

During normal development:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
```

```powershell
.\install.ps1
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py --help
& "$HOME\.agents\skills\forge-method\forge-method.ps1" --help
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py preflight --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py start --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py example list
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py example create --root <example-folder> --module software-builder
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py gate --root <example-folder> --require-evals
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py init --project smoke-test --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py preflight --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py resume --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py start --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py project create --root <parent-folder> --name generated-project --module software-builder
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py project create --root <parent-folder> --name generated-project --module auto --objective "build a web app"
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py project list --root <parent-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py module recommend --objective "build a web app"
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py snapshot --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py agent list --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py agent recommend --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py input add --root <temp-folder> --id audience --prompt "Who is this for?"
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py input answer --root <temp-folder> --id audience --answer "Smoke users"
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py checkpoint --root <temp-folder> --summary "Progress memory"
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py context plan --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py context recover --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py context recover --root <temp-folder> --compact
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py artifact verify --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py story export --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py story import --root <temp-folder> --file backlog.json
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py gate --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py release plan --root <repo-or-project-folder> --mode batch --touches runtime
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py release check --root <repo-folder> --mode batch --touches runtime
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py status --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py status --root <temp-folder> --brief
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py resume --root <temp-folder> --json
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py next --root <temp-folder>
python $HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py audit --root <temp-folder>
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-local.ps1
```

```bash
bash install.sh
python ~/.agents/skills/forge-method/scripts/forge_method_runtime.py --help
bash ~/.agents/skills/forge-method/forge-method.sh --help
bash scripts/smoke-runtime.sh
bash scripts/smoke-install.sh
bash scripts/smoke-plugin-local.sh
```

On Windows, shell verification requires a registered WSL distribution. WSL version 2 without a distro is not enough to run `bash`; use the PowerShell verification scripts in that environment.

## Release And Validation Policy

Development changes should be grouped according to delivery mode. If the project is being delivered one story at a time, each story may have its own version. If several completed stories already form a coherent product increment, ship them as one batch. A batch should ship when it changes a meaningful product capability, public command surface, installation behavior, or user-facing workflow.

Validation tiers:

- fast: unit tests, workflow validation, and agent profile validation during normal development
- targeted smoke: runtime smoke after workflow/state-transition changes; install smoke after install or packaging changes
- full: both platform verifiers, plugin/skill validation, CI, and clean install proof before a published release

Use `release plan` before publishing to choose story, batch, hotfix, or breaking cadence and to confirm the validation tier. Use `release check` after the batch is ready to verify local release readiness before full verification. Both commands are intentionally non-publishing; neither creates a tag nor a GitHub release.

After a tag or branch is available from a Git-clonable source, run the clone/install distribution smoke:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -RepoUrl https://github.com/DanielCarva1/forge-method-core.git -Ref v1.21.0 -ExpectedVersion 1.21.0
```

```bash
REPO_URL=https://github.com/DanielCarva1/forge-method-core.git REF=v1.21.0 EXPECTED_VERSION=1.21.0 bash scripts/smoke-plugin-clone-install.sh
```

This smoke does not use the GitHub API. It clones the requested ref, installs the plugin into an isolated temporary marketplace, verifies manifest and marketplace metadata, runs preflight, creates a project, and runs the quality gate.

Do not create a tag or GitHub release for every small story when the work is already being accumulated as a package. Use patch releases for urgent fixes, story releases for intentional story-by-story delivery, minor releases for grouped backward-compatible capabilities, and major releases only for incompatible public surface changes.

## What Still Needs Productization

- marketplace listing/publication metadata
- GitHub PR workflow
