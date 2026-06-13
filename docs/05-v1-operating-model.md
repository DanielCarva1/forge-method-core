# V1 Operating Model

Forge Method Core v1 is a file-backed runtime for long-horizon creation work. The agent may change, the context window may reset, and the terminal may close; the project state must still be recoverable.

## Runtime Shape

```txt
intent -> discovery -> specification -> planning -> build/verify -> ready/operate -> evolve
```

## Durable Runtime Surfaces

```txt
.forge-method/
  state.yaml              current phase, workflow, project, next action
  projects.yaml           project identity and registry metadata
  sprint.yaml             sprint summary and active story pointer
  stories/                one state file per story
  evidence/               proof that work happened and checks ran
  artifacts/              generated specs, briefs, plans, and release notes
  checkpoints/            structured progress memory for long-running work
  context/                compact context packs for future sessions
  evals/                  local checks for workflow routing and structure
  handoffs/               continuation notes after large work blocks
  agents/                 project-local agent profiles when packaged profiles are not enough
  inputs/                 durable human input requests and answers
  reviews/                durable review findings tied to stories
  ledger.ndjson           append-only runtime event stream
```

## Agent-Facing Docs

Agent-facing docs are state machines. They should be short, explicit, and loaded only when the current state needs them.

Required sections:

```md
trigger:
inputs:
steps:
outputs:
done_when:
blocked_when:
handoff:
```

## Human-Facing Docs

Human-facing docs explain why the runtime exists, how to install it, and how to reason about it. They should not be required during normal execution.

## Human Experience

Human Experience is allowed to be warm, specific, and conversational. Guide output, onboarding, Agent Council discussion, and user-facing explanations should help people think, compare options, and make decisions.

## Guidance Engine Rule

`guide --question --json` is the authoritative interpreter when the latest user message contains a correction, doubt, brainstorm request, research request, product/UX planning request, quick-dev request, story lifecycle request, new intent, or build request. It must classify intent, detect signals, recommend phase/workflow/action, include alternatives, and say whether state must update before continuing.

A stale `next_action` must not override a newer human correction. If a ready project receives critique or new intent, the next route is `6-evolve` or `correct-course`, not repeated release or publication work.

Guidance Engine should return workflow catalog metadata and a facilitation pack when one exists. The catalog gives agents phase, required/optional, follow-up, and output expectations. The facilitation pack gives humans a richer guided conversation without making the compact workflow state machine larger.

A facilitation pack is not just a short prompt. It must include stage-by-stage conversation flow, elicitation options, facilitator moves, quality bar, and anti-patterns. This is how Forge keeps the agent runtime compact while giving humans a guided experience with taste.

Specialized requests should route to the narrowest available depth workflow when the human names the lifecycle job. Examples: PRD validation should not collapse to generic spec writing; UX planning should not collapse to implementation; quick-dev should not skip spec-lite proof; game story creation should not collapse to generic game brief; traceability should not collapse to generic test strategy; workflow analysis should not collapse to generic runtime-builder; document indexing should not collapse to domain research.

When a narrow catalog workflow has execution modes and differs from the current active workflow, Guidance Engine should return a `transition-workflow` command and mark `state_update_required` so the next agent can enter that workflow instead of only describing it.

## Agent Runtime

Agent Runtime is compact and deterministic. Skills, workflows, manifests, artifacts, evals, gates, and recovery files should be optimized for agents that must resume work with minimal context.

## Track Rule

Tracks route the method without adding public skills. A track records complexity, project kind, and the likely module path. The public entrypoint remains `$forge-method`; helper commands such as `track set` are implementation surface for agents.

## Agent Council Rule

Agent Council is optional. It may show a rich live discussion to the human, but durable project memory should be a compact decision artifact. Future agents should load the decision artifact, not a full debate transcript.

## Mechanical Autonomy Rule

Forge should not ask for procedural confirmation when the next step is mechanical. After discovery, specification, and planning have closed, build work may create stories, implement, review, repair, test, write evidence, update sprint state, and continue to the next story without asking the user to approve each transition.

## Self-Update Rule

Git marketplace installs should check for safe package updates before normal start, preflight, guide, or resume. A successful update prints compact patch notes and continues the same startup; it must not ask the user to open another chat or invoke `$forge-method` again as part of the normal flow.

## Grill Gate Rule

Discovery, specification, and planning should close with Grill Gate before they unlock long mechanical work. Grill Gate challenges the phase artifact against glossary terms, ADRs, constraints, risks, and acceptance criteria. It asks the user only when durable artifacts cannot resolve a real product decision.

## Correct-Course Continuation Rule

If a contradiction appears during mechanical work, the agent should write a compact correct-course artifact, choose the conservative interpretation that preserves the approved spec, and continue. Human input is reserved for missing access, destructive approval, explicit user scope changes, or contradictions that cannot be resolved from artifacts.

## Codex Goal Rule

When a mechanical work order is long-running, Forge should prepare a Codex Goal handoff instead of inventing a separate autopilot surface. If Goal mode is unavailable, the user can enable Codex goals or the agent can continue in the current thread.

## Completion Rule

A task is complete only when:

- required output exists
- acceptance criteria are satisfied
- evidence is written
- state is updated
- next action is known

## Quality Gate Rule

Before declaring a project ready, the agent should run the quality gate. The gate combines project audit, artifact verification, workflow validation, and local evals so the agent does not accidentally skip a required check.

## Start Rule

Starting the method is a routing operation, not implementation work. The agent must resolve whether the current folder already has project state, whether it is the runtime repo, and which known project roots exist before asking the user to choose or create a project.
When the user chooses a new project, the agent should use project creation so state, kickoff story, brief, eval, checkpoint, and context load plan are created together.

When the selected workspace already contains code but no `.forge-method/state.yaml`, the agent should treat it as brownfield. Brownfield initialization must start in `1-discovery`, inventory existing behavior and in-progress work, and only then move toward specification, planning, or implementation.

The agent should run preflight before broad reading. Preflight is read-only: it may compute the selected context files and next commands, but it must not write state, checkpoint, ledger, or context artifacts.

After preflight resolves an existing project, the agent should use resume guidance to choose the next safe action. Resume guidance is also read-only and must prefer durable inputs, review findings, story state, audit results, and readiness state over chat memory.

## Human Input Rule

Human input must be requested as durable state when it blocks the project route, discovery, specification, or a risky decision. Required open input sets `human_input_required`. The agent may continue autonomous work only after the input is answered, deferred, or marked non-required.

Procedural confirmations such as continue next story, accept review loop, run tests, or move to ready are not human input.

## Review Finding Rule

Review findings must be stored as durable state when review discovers a defect, missing proof, incomplete acceptance coverage, or risky exception. A story cannot be marked `done` while a linked finding is open. Findings may be resolved with a short resolution or waived with an explicit reason.

## Context Rule

The agent should not load all project documentation. It should build a compact context pack from:

- current state
- active workflow
- active story
- latest checkpoint
- relevant artifacts
- recent evidence
- open review findings
- failing checks
- recommended agent profiles
- next action

Context packs must stay bounded. If a pack exceeds the configured character budget, it is truncated and the next run should regenerate a more selective artifact or handoff.

The agent should prefer the machine-readable preflight and context load plan before reading files. The load plan ranks selected files by current state and defers lower-priority files when the budget is full.

The agent should check context health before long work blocks. `ok` means continue, `watch` means checkpoint soon, `compact` means write compact recovery before broad work, and `blocked` means split work or recover before loading more context.

For handoff under a tight budget, the agent should write compact recovery. Compact recovery keeps state, resume guidance, read order, commands, done conditions, and blocking conditions before optional history so a new context does not lose the next safe action.

## Checkpoint Rule

After a meaningful work block, the agent should write a checkpoint with the decisions, checks, touched files, artifacts, and next action needed for a future session. A checkpoint is durable memory; it is not a transcript.

## Artifact Lifecycle Rule

Artifacts are either durable or ephemeral. Durable artifacts must remain readable while referenced. Ephemeral artifacts may be deleted only after `artifact capture` records the result and, when relevant, links it to a story, evidence item, or checkpoint.

## Generation Rule

New workflows and modules may be generated by the runtime, but they must validate immediately. A generated workflow must include every required state-machine section before it can be used.

## Eval Rule

Every generated workflow should have at least one local eval. Evals are small project files that check whether a workflow target exists, validates structurally, and matches the expected route.
Eval kinds should stay objective and cheap: workflow routing, workflow trigger coverage, and artifact availability are the default local checks.

## Fixture Rule

Packaged modules must be covered by fixture projects before release. The fixture matrix creates an example and a normal project for each module, runs quality gates, generates compact recovery, checks parent preflight choices, and verifies representative objective routing.

Guidance Engine changes must also pass the packaged parity replay fixture. The replay fixture lives inside the Forge skill package so source tests and installed-smoke validation exercise the same human-intent routing matrix without relying on chat memory.

## Agent Profile Rule

Agent profiles are compact routing manifests. They must define when to use the profile, required inputs, required outputs, and handoff content. The quality gate validates packaged and project-local profiles before ready/release.

## Ready Rule

Phase 5 means the project is ready for use. It is not a pause in implementation; it is a distinct operating state with release evidence, usage notes, support status, and future backlog.
