# Forge Method Core

Forge Method Core is a Codex-native runtime for turning messy intent into durable work: project state, guided workflows, artifacts, implementation, validation, and a usable next step.

It is not just agent automation. Forge is built to make the human think better before Codex builds faster. It bullies weak assumptions, surfaces alternative paths, and pushes users out of autopilot before big decisions or immature ideas harden into expensive plans.

Current runtime version: `1.33.0`

## What It Does

Forge gives Codex a file-backed operating model for long-horizon creation work.

- It starts with the current workspace instead of stale chat memory.
- It asks better questions before rushing into implementation.
- It turns fuzzy intent into project state, stories, artifacts, evidence, and checkpoints.
- It keeps future agents oriented with compact recovery files instead of long transcripts.
- It validates progress with local checks and quality gates before calling work ready.

The result is a workflow that can handle product thinking, software build-out, creative work, game design, QA, release planning, and runtime extension without losing the thread every time a chat resets.

## Why It Exists

Most AI coding workflows are good at motion. Forge is built for direction.

When the idea is vague, it slows the agent down. When the user is about to make a large call, it asks for tradeoffs and alternatives. When the work becomes mechanical, it lets Codex keep moving from durable state instead of asking for ceremonial approval on every step.

The goal is simple: keep the human in the decisions that matter, and let the agent carry the operational load.

## Install

Preferred install path: add this repository as a Codex plugin marketplace source.

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref main
```

Then:

1. Open Codex Plugins or `/plugins`.
2. Choose the `Forge Method` marketplace.
3. Install or enable `Forge Method Core`.
4. Start a new Codex thread.
5. Run:

```txt
$forge-method
Start Forge Method in this workspace.
```

If a chat seems stuck on old instructions, start a new thread or run:

```txt
$forge-reload
```

To update an existing Git marketplace install, run:

```txt
$forge-update
```

If you prefer the CLI directly:

```powershell
codex plugin marketplace upgrade forge-method-core
```

### Pinned Version

Use this when you want to stay on exactly `1.33.0` instead of following `main`:

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref v1.33.0
```

## Start A Project

Open the folder where the project should live and ask Codex:

```txt
$forge-method
Start Forge Method in this workspace.
```

Forge will run a preflight step first. It detects whether the folder is:

- an existing Forge project
- a parent folder with known projects
- the runtime repository itself
- an empty workspace
- a brownfield codebase that needs discovery before planning

From there it guides the next move instead of guessing from chat history.

## What Gets Created

A Forge project stores its working state in files under `.forge-method/`.

Typical project files include:

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

These files are the source of truth for future agents. Chat memory is useful, but it is not treated as durable state.

## How It Feels

Forge has two modes that matter in practice.

When the work needs judgment, it acts like a demanding collaborator: it asks what would make the idea fail, what alternatives exist, what proof is missing, and which decision is actually being made.

When the work is already defined, it acts like a runtime: it creates stories, follows state, writes evidence, checks gates, records handoffs, and keeps going until the next real decision or blocker appears.

## Included Runtime Pieces

```txt
.codex-plugin/plugin.json           Codex plugin manifest
skills/forge-method/SKILL.md        Main runtime skill
skills/forge-reload/SKILL.md        Emergency reload skill
skills/forge-update/SKILL.md        Manual update skill
skills/forge-method/modules/        Packaged module manifests
skills/forge-method/catalog/        Workflow metadata
skills/forge-method/facilitation/   Human-facing guided conversation packs
skills/forge-method/agents/         Packaged agent profiles
skills/forge-method/references/     Compact workflow state machines
skills/forge-method/templates/      Reusable artifact templates
skills/forge-method/scripts/        Deterministic runtime helpers
docs/                               Product, architecture, and operating docs
examples/                           Minimal initialized project example
release-notes/                      Patch notes and release metadata
```

## Fallback Install

Use this only when plugin marketplace install is unavailable.

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

That installs the skills under the user's local Codex skill directory. After that, start a new Codex thread and invoke `$forge-method`.

## Local Plugin Development

Use this path when developing the plugin itself.

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

The local installer copies the plugin source into a personal Codex plugin marketplace and prints `codex://` links for opening the plugin in the Codex app.

## Maintainer Checks

For normal development:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
```

```bash
bash scripts/verify-fast.sh
```

Before release or after broad runtime/install changes:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-all.ps1
```

```bash
bash scripts/verify-all.sh
```

After publishing a tag, verify that the published package can be cloned and installed:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v1.33.0 -ExpectedVersion 1.33.0
```

```bash
REF=v1.33.0 EXPECTED_VERSION=1.33.0 bash scripts/smoke-plugin-clone-install.sh
```

## Docs

- [Quickstart](docs/00-quickstart.md)
- [Product Proposal](docs/01-product-proposal.md)
- [Runtime Architecture](docs/02-runtime-architecture.md)
- [Operating Model](docs/05-v1-operating-model.md)
- [Marketplace Onboarding](docs/08-marketplace-onboarding.md)

## Example

See [`examples/hello-method`](examples/hello-method) for a minimal initialized project.
