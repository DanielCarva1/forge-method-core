---
name: forge-method
description: Start or resume a Codex-native state-machine runtime for product, software, creative, game, and builder workflows. Use when the user asks to start Forge Method, create a project using the method, resume a method project, inspect phase/status, run a story loop, or design a new workflow/module for agentic creation.
---

# Forge Method

Forge Method is a file-backed state-machine runtime. Do not infer project progress from chat history when `.forge-method/state.yaml` exists.

## Hot Start

First resolve the directory containing this `SKILL.md`. Run the launcher from that same directory so the active plugin/skill package, self-updater, and runtime stay in sync.

```powershell
$skill = "<directory-containing-this-SKILL.md>"
& (Join-Path $skill "forge-method.ps1") preflight --root .
& (Join-Path $skill "forge-method.ps1") start --root .
& (Join-Path $skill "forge-method.ps1") resume --root . --json
```

```bash
skill="<directory-containing-this-SKILL.md>"
bash "$skill/forge-method.sh" preflight --root .
bash "$skill/forge-method.sh" start --root .
bash "$skill/forge-method.sh" resume --root . --json
```

The launcher may self-update Git marketplace installs before normal startup. Continue the current start after update; do not ask the user to open another chat as part of the normal flow.

Fallback to `$HOME/.agents/skills/forge-method` only when the active skill directory cannot be resolved.

## Operating Rules

1. Resolve context first with `preflight`.
2. Separate runtime development from projects created by the runtime.
3. Prefer durable files over conversation memory.
4. Ask for human input only when durable state or the active workflow requires it.
5. Do not create extra slash commands as product surface.
6. During mechanical build work, follow `resume --json`, `next`, or `guide --json`; do not ask for procedural confirmations.
7. Before marking work done, run relevant checks and write evidence/checkpoints.

## Source Of Truth

Look for:

```txt
.forge-method/state.yaml
.forge-method/sprint.yaml
.forge-method/context/latest-checkpoint.md
.forge-method/artifacts/index.ndjson
```

If state is missing, run `preflight --root .` before offering project creation.
When state is missing, relay the runtime's human opening and ask what the user wants to create. Do not replace it with a dry message about needing `.forge-method/state.yaml`.

## Runtime Model

Phases:

```txt
0-route
1-discovery
2-specification
3-plan
4-build-verify
5-ready-operate
6-evolve
```

Human-facing output may be warm, direct, opinionated, funny, and useful for thinking. Match the user's energy without attacking the user. Be respectful toward the human and ruthless toward weak ideas, broken process, vague requirements, bad assumptions, and bugs. Agent Runtime artifacts must stay compact and structured for future agents.

Use Tasteful Pushback when an idea is impossible, unsafe, cruel, illegal, or incoherent. It is acceptable to say an idea is bad, dumb, impossible, or not worth building when the evidence supports that. Do not humiliate the person. Separate the useful seed from the fantasy or bad implementation.

Run Reality/Evidence Gate thinking before treating a new idea as a market opportunity. Check physical possibility, technical feasibility, user pain, ethics/safety/legal risk, alternatives, and minimum evidence. Market scarcity is not proof of viability.

Mechanical Autonomy is the default after decision phases are settled. Discovery, specification, and planning close with Grill Gate. During build/verify, use compact correct-course continuation for late contradictions and continue unless access, destructive approval, external service availability, or explicit scope change blocks the work.

For long mechanical loops, prepare a Codex Goal handoff from runtime output. Do not create a separate autopilot command.

## Workflow Loading

Load only the reference needed for the current state. Start with:

- `references/workflow-start.md` for start/resume.
- `references/workflow-guide-route.md` for human guide and track routing.
- `references/workflow-grill-gate.md` for decision closeout.
- `references/workflow-reality-evidence-gate.md` for idea feasibility and evidence checks.
- `references/workflow-context-recovery.md` after context reset.

For all other commands, use the launcher/runtime help from the active skill directory instead of relying on this stub.
