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

Current runtime version: `1.24.0`.

## Current Shape

```txt
.agents/plugins/marketplace.json    Repo marketplace for GitHub install
.codex-plugin/plugin.json          Codex plugin manifest
skills/forge-method/SKILL.md       Main runtime skill
skills/forge-method/modules/        Packaged module manifests
skills/forge-method/agents/          Packaged agent profiles
skills/forge-method/references/    Compact state-machine workflows
skills/forge-method/techniques/     Compact creative technique manifests
skills/forge-method/templates/      Reusable artifact templates
skills/forge-method/scripts/       Deterministic runtime helpers
docs/                              Product and architecture proposal
templates/                         Project state templates
examples/                          Minimal initialized project example and example notes
install.ps1                        Fallback user-skill installer for Windows
install.sh                         Fallback user-skill installer for macOS/Linux
```

## Install From GitHub In Codex

Forge Method Core is packaged as a Codex plugin and this repo is also a Codex marketplace source.

1. Install Codex.
2. Copy and run this command in a terminal, or ask Codex to run it:

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref main
```

3. Open Codex Plugins or `/plugins`.
4. Choose the `Forge Method` marketplace.
5. Install or enable `Forge Method Core` with the Codex plugin switch.
6. Start a new thread.
7. Invoke Forge Method and ask it to start:

```txt
$forge-method
Start Forge Method in this workspace.
```

From the CLI plugin browser:

```txt
codex
/plugins
```

The marketplace entry lives at `.agents/plugins/marketplace.json` and points at this repo root as the plugin source, so people can install from GitHub without manually copying files.

## Local Development Install

Use this only while developing the plugin locally or when a GitHub marketplace install is unavailable.

For Windows:

```powershell
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
powershell -ExecutionPolicy Bypass -File .\scripts\install-plugin-local.ps1
```

On macOS/Linux:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
bash scripts/install-plugin-local.sh
```

That copies the plugin source to `~/plugins/forge-method-core` and creates or updates the personal Codex marketplace file at `~/.agents/plugins/marketplace.json`. After that, restart Codex, open Plugins, select Forge Method Core, and start a new thread.

The installer also prints `codex://` links to open the plugin detail page and the workspace share flow directly in the Codex app.

Public marketplace publication is a separate external listing process. The repository is packaged and validated for plugin distribution; public availability depends on that listing/approval step.

For a fallback skill-only install on Windows:

```powershell
git clone <repo-url>
cd forge-method-core
.\install.ps1
```

Fallback skill-only install on macOS/Linux:

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

Installed helper launchers are also copied with the skill:

```powershell
& "$HOME\.agents\skills\forge-method\forge-method.ps1" preflight --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" start --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" doctor --root . --touches runtime
```

```bash
bash ~/.agents/skills/forge-method/forge-method.sh preflight --root .
bash ~/.agents/skills/forge-method/forge-method.sh start --root .
bash ~/.agents/skills/forge-method/forge-method.sh doctor --root . --touches runtime
```

The launchers resolve `$PYTHON`, then common Python commands.

## Runtime Commands

The skill can ask Codex to run:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" preflight --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" start --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project create --root . --name my-project --module software-builder
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project create --root .. --path . --name my-existing-project --module auto --objective "continue this existing project" --brownfield
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" init --project my-project
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" version
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status --root . --brief
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" snapshot --root . --pretty
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" next
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" resume --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" guide --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" guide --root . --question "what should we do next?"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" track list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" track recommend --objective "build a secure web app"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" track set --root . --track standard-product
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" council run --root . --topic "choose the next product direction"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" correct-course --root . --summary "late contradiction and conservative continuation"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" builder scaffold --root . --kind workflow --id custom-flow --title "Custom Flow"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" builder validate --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" config inspect --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" config validate --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" input list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" review list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" story export --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" story import --root . --file backlog.json
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" audit
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" checkpoint --summary "What changed and what to do next"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context plan --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context health --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context recover --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context recover --root . --compact
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" artifact verify --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" gate --root . --require-evals
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" release plan --root . --mode batch --touches runtime
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" release check --root . --mode batch --touches runtime
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" doctor --root . --touches runtime
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" module list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" module recommend --objective "build a web app"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" agent recommend --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example create --root ./software-example --module software-builder
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" workflow validate
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" workflow create --root . --id custom-flow --title "Custom Flow"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" eval run --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" eval add --root . --kind artifact-exists --id spec-exists --target .forge-method/artifacts/spec.md --query "spec artifact exists"
```

The script creates `.forge-method/` in the target project and keeps state out of the chat transcript.

Use `preflight --root . --json` before broad context loading. It resolves whether the folder is an existing method project, a runtime repo, a parent folder with known projects, or an empty workspace, then returns the first files, decision options, and commands an agent should use.

When a workspace already contains code but no `.forge-method/state.yaml`, preflight treats it as an existing codebase and routes initialization through `project create --brownfield`. Brownfield projects always start in `1-discovery` so the agent inventories current behavior, in-progress work, constraints, and risks before specification, planning, or implementation.

Use `context health` before long work blocks. Use `context recover --compact` when health is `compact` or `blocked`; it preserves state, resume guidance, read order, and commands before optional sections.

Use `doctor --root . --touches runtime` when a local setup feels slow or uncertain. It reports project/runtime detection, plugin installation status, Python/Git/GitHub CLI/WSL readiness, and the recommended development and release verification commands for the touched area.

Forge Method separates Human Experience from Agent Runtime. Human-facing guide and council output can be conversational, opinionated, and useful for thinking. Runtime artifacts, workflows, evals, and recovery files stay compact so future agents can continue without rereading long discussions.

Agent Council is optional. It can show a rich live discussion for the human, but it persists only a compact decision artifact under `.forge-method/artifacts/`.

Mechanical Autonomy is the default for procedural build work. `resume`, `next`, and `guide` expose a Mechanical Work Order so agents can create stories, implement, review, repair, test, write evidence, update sprint state, and move to ready without asking for procedural confirmations. Discovery, specification, and planning close with Grill Gate; late contradictions in build use compact correct-course continuation.

For long loops, Forge prepares a Codex Goal handoff instead of inventing another public command. Automatic commits are off by default and can be configured per project as `story` or `epic`.

## Smoke Tests

Use fast verification during normal development:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
```

```bash
bash scripts/verify-fast.sh
```

On Windows, the shell scripts require a registered WSL distribution. If WSL exists but no distro is installed, use the PowerShell scripts.

The PowerShell scripts use `$env:PYTHON`, then `python`/`python3`/`py`.

Run targeted smokes when the touched area needs them:

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

Use fast or targeted validation while developing. Run full verification before publishing a release, after install/package changes, or after broad runtime changes. Story-by-story versioning is valid when the product is intentionally delivered one story at a time. When several completed stories already form one coherent product increment, batch them into a larger release.

Do not run `release check` for every intermediate commit. Build and validate the batch first, then run `release check` once when the batch is ready to tag or publish.

The full verifier includes a fixture matrix smoke. It creates example and normal projects for every packaged module, runs quality gates, checks compact recovery, verifies parent preflight decisions, and validates objective-to-module recommendations.

After a tag is pushed, run a clone/install distribution smoke from the published ref:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v1.24.0 -ExpectedVersion 1.24.0
```

```bash
REF=v1.24.0 EXPECTED_VERSION=1.24.0 bash scripts/smoke-plugin-clone-install.sh
```

## Product Direction

The complete proposal is in:

- `docs/00-quickstart.md`
- `docs/01-product-proposal.md`
- `docs/02-runtime-architecture.md`
- `docs/03-v1-roadmap.md`
- `docs/05-v1-operating-model.md`
- `docs/06-runtime-improvement-backlog.md`
- `docs/07-v1-readiness-audit.md`
- `docs/08-marketplace-onboarding.md`
- `docs/09-expansion-roadmap.md`

## Example

See `examples/hello-method` for a minimal initialized project with `.forge-method/state.yaml`.

To create a runnable seed project from any packaged module:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example create --root ./software-example --module software-builder
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" gate --root ./software-example --require-evals
```

To create a normal method project from a packaged module:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project create --root . --name "My Project" --module software-builder
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project create --root . --name "My Project" --module auto --objective "build a web app"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project list --root .
```
