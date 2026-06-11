---
name: forge-method
description: Start or resume a Codex-native state-machine runtime for product, software, creative, game, and builder workflows. Use when the user asks to start Forge Method, create a project using the method, resume a method project, inspect phase/status, run a story loop, or design a new workflow/module for agentic creation.
---

# Forge Method

Forge Method is a state-machine runtime. Do not infer runtime state from chat history when a `.forge-method/state.yaml` file exists.

## Operating Rules

1. Resolve context first.
2. Separate runtime development from projects created by the runtime.
3. Load only the workflow reference needed for the current state.
4. Prefer state files and evidence over conversation memory.
5. Ask for human input only when the workflow marks it required.
6. For implementation work, run checks and update evidence before marking done.
7. Temporary task docs may be deleted only after `artifact capture` records their result in state, story, evidence, or checkpoint.
8. Do not create extra slash commands as product surface unless explicitly requested.

## Source Of Truth

Look for:

```txt
.forge-method/state.yaml
.forge-method/sprint.yaml
.forge-method/context/latest-checkpoint.md
.forge-method/artifacts/index.ndjson
```

If missing, offer to initialize the workspace.
Before offering, run the preflight helper so project choices and context files come from disk rather than chat memory.

## Runtime Helper

First resolve the skill directory that contains this `SKILL.md`. In a plugin install this is the plugin cache skill directory; in a local user install it is usually `$HOME\.agents\skills\forge-method`. Prefer running helpers relative to that directory so the active plugin and the runtime script stay in sync.

Use the launcher from the current skill directory:

```powershell
$skill = "<directory-containing-this-SKILL.md>"
& (Join-Path $skill "forge-method.ps1") preflight --root .
& (Join-Path $skill "forge-method.ps1") start --root .
& (Join-Path $skill "forge-method.ps1") doctor --root . --touches runtime
```

Use the helper script from the current skill directory when direct Python execution is useful:

```powershell
$skill = "<directory-containing-this-SKILL.md>"
python (Join-Path $skill "scripts\forge_method_runtime.py") preflight --root . --json
python (Join-Path $skill "scripts\forge_method_runtime.py") guide --root .
python (Join-Path $skill "scripts\forge_method_runtime.py") track recommend --objective "<project objective>"
python (Join-Path $skill "scripts\forge_method_runtime.py") council run --root . --topic "<decision topic>"
python (Join-Path $skill "scripts\forge_method_runtime.py") status --root . --brief
python (Join-Path $skill "scripts\forge_method_runtime.py") resume --root . --json
python (Join-Path $skill "scripts\forge_method_runtime.py") context plan --root .
python (Join-Path $skill "scripts\forge_method_runtime.py") gate --root . --require-evals
```

Fallback to the legacy user install path only when the current skill directory cannot be resolved:

```powershell
& "$HOME\.agents\skills\forge-method\forge-method.ps1" preflight --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" start --root .
& "$HOME\.agents\skills\forge-method\forge-method.ps1" doctor --root . --touches runtime
```

Legacy direct helper examples:

```powershell
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" preflight --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" preflight --root . --json
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" start --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" project create --root . --name <name> --module <module-id>
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" status --root . --brief
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" snapshot --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" next
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" resume --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" resume --root . --json
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" guide --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" track list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" track recommend --objective "<project objective>"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" council run --root . --topic "<decision topic>"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" builder validate --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" config inspect --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" input list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" review list --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" init --project <name>
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" story list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" story export --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" story import --root . --file backlog.json
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" artifact list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" artifact verify
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" module list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" module recommend --objective "<project objective>"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" agent recommend --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example list
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" example create --root <path> --module <module-id>
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" workflow validate
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" eval run
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" eval add --kind artifact-exists --id <id> --target <path> --query "<objective check>"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" checkpoint --summary "<progress memory>"
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context plan --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context health --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context recover --root .
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" context recover --root . --compact
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" audit
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" gate --root . --require-evals
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" release plan --root . --mode batch --touches runtime
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" release check --root . --mode batch --touches runtime
python "$HOME\.agents\skills\forge-method\scripts\forge_method_runtime.py" doctor --root . --touches runtime
```

## Phase Model

```txt
0-route
1-discovery
2-specification
3-plan
4-build-verify
5-ready-operate
6-evolve
```

## Human Experience And Agent Runtime

Human Experience may be warm, direct, funny, opinionated, and discussion-oriented when talking to the user.
Agent Runtime outputs must stay compact: state files, workflows, artifacts, evals, and handoffs should be structured for future agents.

For Agent Council:

- use real Codex subagents when the current environment exposes them and the user asked for council or the decision is high-risk;
- otherwise use `council run` as the serial fallback;
- show the full debate to the human;
- persist only the compact council decision artifact;
- never require the future agent to reread a full transcript.

## Workflow Selection

- Start/resume project: read `references/workflow-start.md`.
- Human guide and track routing: read `references/workflow-guide-route.md`.
- Optional Agent Council: read `references/workflow-council-decision.md`.
- Discovery: read `references/workflow-discover-intent.md`.
- Specification: read `references/workflow-write-spec.md`.
- Product requirements: read `references/workflow-product-requirements.md`.
- Architecture: read `references/workflow-architecture.md`.
- UX planning: read `references/workflow-ux-plan.md`.
- Planning: read `references/workflow-plan-sprint.md`.
- Epics/story planning: read `references/workflow-create-epics.md`.
- Implementation readiness: read `references/workflow-readiness-check.md`.
- Correct course or pivot: read `references/workflow-correct-course.md`.
- Validation strategy: read `references/workflow-test-strategy.md`.
- Build a story: read `references/workflow-build-story.md`.
- Creative/domain ideation: read `references/workflow-creative-session.md`, then the specific creative workflow.
- Game project path: read `references/workflow-game-project.md`, then the specific game workflow.
- Create method modules/skills/workflows: read `references/workflow-runtime-builder.md` or `references/workflow-builder-scaffold.md`.
- Project customization: read `references/workflow-config-customization.md`.
- Eval design: read `references/workflow-eval-design.md`.
- Enterprise planning: read the specific security, privacy, DevOps, compliance, observability, risk, or release-readiness workflow.
- Ready/release: read `references/workflow-ready-release.md`.
- Evolve an existing project: read `references/workflow-evolve-project.md`.
- Recover after context reset: read `references/workflow-context-recovery.md`.
- Diagnose local setup or verification tier: run `doctor --root . --touches <area>` before changing environment.
- If no workflow matches, report current state and ask one concise routing question.

## Completion Standard

Never mark a workflow done because the agent "feels" done. Mark done only when the workflow `done_when` conditions are satisfied and evidence is written.
After meaningful progress, write a checkpoint so future sessions can resume without replaying the chat.
When `context health` returns `compact` or `blocked`, write compact recovery before continuing broad work.
Before ready/release, run the quality gate and preserve the result as evidence when it passes.
