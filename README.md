# Forge Method Core

Forge Method extends human capacity and keeps Codex on rails: messy ideas go in, durable state, guided workflows, sharper thinking, and receipts come out.

It is a Codex-native creation runtime: a state-machine system for turning intent into artifacts, implementation, validation, release, and future evolution.

It is not just agent automation. Forge is built to make humans think better: guided flows, better questions, taste pressure, and the occasional useful slap on the wrist before a bad idea becomes an expensive artifact.

It is built around Codex primitives:

- skills for compact workflow loading
- plugins for distribution
- file-backed state instead of chat memory
- scripts for deterministic project status and transitions
- subagents for focused review, QA, research, architecture, and creative roles
- evidence logs for implementation and validation

This repository is the core runtime and distribution package.

Current runtime version: `1.31.0`.

## Current Shape

```txt
.agents/plugins/marketplace.json    Repo marketplace for GitHub install
.codex-plugin/plugin.json          Codex plugin manifest
skills/forge-method/SKILL.md       Main runtime skill
skills/forge-reload/SKILL.md       Emergency bootstrap reload skill
skills/forge-method/modules/        Packaged module manifests
skills/forge-method/catalog/        Workflow metadata for phase, required status, outputs, and follow-up routing
skills/forge-method/facilitation/   Human-facing guided conversation packs
skills/forge-method/agents/          Packaged agent profiles
skills/forge-method/references/    Compact state-machine workflows
skills/forge-method/references/workflow-guidance-engine.md  Human-intent routing state machine
skills/forge-method/techniques/     Compact creative technique manifests
skills/forge-method/templates/      Reusable artifact templates
skills/forge-method/scripts/       Deterministic runtime helpers
release-notes/                     Machine-readable and human patch notes
docs/                              Product and architecture proposal
templates/                         Project state templates
examples/                          Minimal initialized project example and example notes
install.ps1                        Fallback user-skill installer for Windows
install.sh                         Fallback user-skill installer for macOS/Linux
```

## Install From GitHub In Codex

Forge Method Core is packaged as a Codex plugin and this repo is also a Codex marketplace source.

### Tester Install

Use this for the current `1.31.0` tester build:

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref codex/script-audit-optimization
```

Then:

1. Open Codex Plugins or `/plugins`.
2. Choose the `Forge Method` marketplace.
3. Install or enable `Forge Method Core`.
4. Start a new Codex thread.
5. Invoke Forge Method:

```txt
$forge-method
Start Forge Method in this workspace.
```

If a chat seems stuck on old instructions, start a new thread or run:

```txt
$forge-reload
```

### Stable Ref

After `1.31.0` is merged or tagged, use the stable marketplace ref:

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref main
```

Then open Codex Plugins or `/plugins`, choose the `Forge Method` marketplace, install or enable `Forge Method Core`, and start a new thread:

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

After the self-updating package is installed, `$forge-method` checks for safe updates before normal startup when the plugin came from the Git marketplace. If a newer package is available, the launcher updates the marketplace copy, prints a short patch-notes summary, and continues `preflight`, `start`, or `reload` in the same chat. Update messages go to stderr so JSON output stays machine-readable. Set `FORGE_METHOD_UPDATE_POLICY=notify|off` to change this behavior, or `FORGE_METHOD_SKIP_UPDATE=1` for CI and local smoke tests.

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

Use `$forge-reload` only as an emergency bootstrap reset when a chat appears to be replaying stale Forge instructions. It reloads the current package contract and routes from filesystem state without reading broad project context.

or by asking Codex to start Forge Method in a workspace.

Installed helper launchers are also copied with the skill:

```powershell
& "$HOME\.agents\skills\forge-method\forge-method.ps1" preflight --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" start --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" reload --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" doctor --root . --touches runtime
```

```bash
bash ~/.agents/skills/forge-method/forge-method.sh preflight --root .
bash ~/.agents/skills/forge-method/forge-method.sh start --root .
bash ~/.agents/skills/forge-method/forge-method.sh reload --root .
bash ~/.agents/skills/forge-method/forge-method.sh doctor --root . --touches runtime
```

The launchers resolve `$PYTHON`, then common Python commands.

## Runtime Commands

The skill can ask Codex to run:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" preflight --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" reload --root .
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
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" guide --root . --question "this route is wrong; help me correct course" --json
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

Use `preflight --root . --json` before broad context loading. It resolves whether the folder is an existing method project, a runtime repo, a parent folder with known projects, or an empty workspace, then returns the first files, decision options, and commands an agent should use. Use `reload --root . --json` when the chat needs a fresh bootstrap contract without writing context files.

Use `guide --question --json` when the latest human message includes critique, doubt, brainstorm, research, new intent, or a build request. The Guidance Engine classifies the intent, detects signals, recommends phase/workflow/action, returns alternatives, and says whether state must update before continuing.

Guide output also includes workflow catalog metadata and a `facilitation_pack` when a richer human-guided conversation is available. Agents should load that pack before conducting the interactive workflow.

Facilitation packs are the human-depth layer. They carry stage-by-stage conversation scripts, elicitation options, facilitator moves, quality bars, and anti-patterns so the human experience can be rich without bloating the compact agent-facing `workflow-*.md` state machines. `workflow validate` rejects referenced facilitation packs that do not include this rich structure.

The packaged catalog includes specialized depth workflows for game lifecycle, test architecture, builder utility, and document utility work. `guide --question --json` should choose those specific workflows when the human asks for story creation, traceability, workflow analysis, skill conversion, doc indexing, spec distillation, or similar concrete jobs.

Guided-depth workflow metadata may include a `template` key. `workflow validate` checks that referenced `skills/forge-method/templates/*.md` files exist.

When Guidance Engine selects a narrow guided-depth workflow for an existing project, it returns a `transition-workflow` command so agents can enter the chosen workflow from durable state before continuing.

When a workspace already contains code but no `.forge-method/state.yaml`, preflight treats it as an existing codebase and routes initialization through `project create --brownfield`. Brownfield projects always start in `1-discovery` so the agent inventories current behavior, in-progress work, constraints, and risks before specification, planning, or implementation.

Use `context health` before long work blocks. Use `context recover --compact` when health is `compact` or `blocked`; it preserves state, resume guidance, read order, and commands before optional sections.

Use `doctor --root . --touches runtime` when a local setup feels slow or uncertain. It reports project/runtime detection, plugin installation status, Python/Git/GitHub CLI/WSL readiness, and the recommended development and release verification commands for the touched area.

Forge Method separates Human Experience from Agent Runtime. Human-facing guide and council output can be conversational, opinionated, and useful for thinking. Runtime artifacts, workflows, evals, guidance decisions, and recovery files stay compact so future agents can continue without rereading long discussions.

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

For focused development loops, run only the relevant unit test and the lightweight validators:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Test tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract
```

```bash
bash scripts/verify-fast.sh --test tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract
```

Use `-SkipUnit` or `--skip-unit` when only onboarding assets, workflow metadata, and agent profiles changed.

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
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v1.31.0 -ExpectedVersion 1.31.0
```

```bash
REF=v1.31.0 EXPECTED_VERSION=1.31.0 bash scripts/smoke-plugin-clone-install.sh
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
