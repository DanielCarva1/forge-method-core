# Agent orchestration and proportional product journeys

**Research date:** 2026-08-12

**Purpose:** inform the Forge action catalog and the journey from one user-facing
start command to proportionate, evidence-backed work.

**Status:** research and recommendation, not runtime authority or an accepted ADR.

## Question

How can Forge help a host agent choose useful actions, modules, and tools without
making the human learn a delivery method, loading the whole catalog into context,
or forcing a large-project ceremony onto a small task?

The key design question is not "goal-driven or staged?" Both solve different
problems:

- a durable objective says what outcome the work is trying to reach;
- stages expose different questions that may need answers before that outcome is
  trustworthy;
- proportional guidance decides how much of each stage needs to be made explicit.

## Current Forge premises

These are repository facts, not findings from the external sources:

- The accepted product constitution says the host agent owns research, planning,
  implementation, and verification; Forge owns durable truth, obligations,
  evidence requirements, continuity, and next-best-action guidance
  ([constitution](../../contracts/policies/agent-native-product-constitution.yaml)).
- It also says governance density scales with impact, uncertainty, reversibility,
  exposure, and evidence quality. A human should not need to know which workflow
  or skill to invoke.
- Lifecycle phases are projections of maturity, not the source of readiness.
  Evidence-backed obligations are the source
  ([constitution](../../contracts/policies/agent-native-product-constitution.yaml)).
- The assurance architecture ranks work by risk-weighted evidence gaps and keeps
  domain-specific methods outside the cross-domain core
  ([assurance architecture](../../contracts/spec/agent-native-assurance-architecture.yaml)).

These premises already support a proportional journey. The gap under discussion
is how to expose that journey and the available capabilities to the host agent.

### Historical Forge signal: the intelligent orchestrator existed in the design

This idea did not come only from memory. A now-superseded Forge slice plan dated
2026-06-26 explicitly defined the Guide as an "intelligent orchestrator": the
host LLM would read typed catalog plus project state, choose a workflow, and emit
an auditable `GuideDecision`; the model-free engine would validate catalog
membership, phase eligibility, and hard gates
([superseded orchestrator plan](../../contracts/plan/slice-2-orchestrator-plan.yaml)).

That plan is marked superseded by the agent-native guidance plan. The old guide
contracts still exist, but current integration documentation says
`guide describe`, `guide status`, and `guide decide` are diagnostics and do not
select the authoritative P5/P6 workflow. New integrations use
`workflow init|resume|next`
([agent integration](../agent-integration.md#compatibility-surfaces)). Current
workflow selection builds eligible candidates and orders them by configured
policy priority and id; a selected `when_applicable` policy pauses for the host
agent's applicability assessment rather than being semantically chosen by a host
router
([workflow governance adapter](../../crates/forge-core-kernel/src/workflow_governance/adapter.rs)).

**Inference:** the original product idea was not absent; its live routing role was
replaced during the move to deterministic workflow governance. The useful part to
recover is the split "host reasons, Forge validates," not the old assumption that
phase eligibility alone is the right routing model. The newer objective,
obligation, evidence, and risk model is a stronger input to orchestration.

## Findings from primary sources

### 1. Current BMAD combines a broad lifecycle with one adaptive build entry

The current BMAD Method documentation still presents four progressive phases:
Analysis, Planning, Solutioning, and Implementation. The phases can produce
research, a product brief or PRD, UX work, architecture, epics/stories, readiness
results, and implementation evidence. Analysis is explicitly optional
([BMAD workflow map, pinned snapshot](https://github.com/bmad-code-org/BMAD-METHOD/blob/b70486b9bdcb0a404d329e2a763b57964e7f1360/docs/reference/workflow-map.md)).

The important current behavior is more nuanced than "small work skips the method."
Every implementation path now converges on `bmad-build`. It accepts direct intent,
an issue, a specification, or a planned story and chooses how much clarification,
planning, implementation, and review that input needs. Clear work may enter it
directly; larger initiatives can add PRD, UX, architecture, decomposition,
readiness, and sprint artifacts first
([BMAD workflow map, pinned snapshot](https://github.com/bmad-code-org/BMAD-METHOD/blob/b70486b9bdcb0a404d329e2a763b57964e7f1360/docs/reference/workflow-map.md)).

BMAD also separates execution from discovery of the method. `bmad-help` inspects
the project, understands a natural-language question, and recommends the next
required or optional step based on installed modules. Generated skill directories
remain the complete list, while help is the context-aware entry point
([BMAD skills reference, pinned snapshot](https://github.com/bmad-code-org/BMAD-METHOD/blob/b70486b9bdcb0a404d329e2a763b57964e7f1360/docs/reference/commands.md)).

Its small-work tutorial uses the same `bmad-build` entry for a small bug or clear
change: the agent clarifies missing intent, proposes a plan, implements, checks,
fixes, and reports. The user chooses desired behavior and reviews the plan rather
than selecting every internal workflow
([BMAD getting started, pinned snapshot](https://github.com/bmad-code-org/BMAD-METHOD/blob/b70486b9bdcb0a404d329e2a763b57964e7f1360/docs/tutorials/getting-started.md)).

**Fact boundary:** BMAD describes itself as scale-adaptive, but that is a product
claim, not independent evidence that its routing is optimal. The useful evidence
here is the documented shape of its current interface. BMAD `main` was inspected
at commit `b70486b9bdcb0a404d329e2a763b57964e7f1360`; later behavior may differ.

### 2. Progressive disclosure is a practical response to catalog growth

Anthropic's Agent Skills design loads only skill name and description first,
loads the main instructions when relevant, and lets the agent navigate linked
files only when it needs deeper detail. Anthropic calls progressive disclosure
the core principle that lets the system scale without putting every instruction
in the active context
([Anthropic, Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)).

Anthropic applies the same idea to tool discovery: tool definitions can be found
on demand, and a search interface may return different detail levels such as name
only, name plus description, or full schema. This reduces context use while
preserving access to a large tool surface
([Anthropic, code execution with MCP](https://www.anthropic.com/engineering/code-execution-with-mcp)).

**Fact boundary:** these sources describe Anthropic's designs, not a universal
standard. They do directly demonstrate that a large capability set does not need
to be injected into every agent turn.

### 3. The agent should mediate the workflow, but deterministic boundaries remain

OpenAI defines an agent as a system in which an LLM manages workflow execution,
selects tools based on current state, recognizes completion, and can transfer
control back to the user. It recommends an incremental orchestration approach,
usually maximizing one agent with clear tools before introducing multi-agent
complexity. Tool clarity and overlap matter more than raw tool count
([OpenAI, A practical guide to building agents](https://openai.com/business/guides-and-resources/a-practical-guide-to-building-ai-agents/)).

OpenAI's manager pattern keeps one agent in control of the user interaction while
specialized agents are available as tools. Its guardrail guidance separately
classifies tools by properties such as read/write behavior, reversibility,
permissions, and impact, using higher risk to trigger stronger checks or human
oversight
([OpenAI, A practical guide to building agents](https://openai.com/business/guides-and-resources/a-practical-guide-to-building-ai-agents/)).

Anthropic describes the same basic control tension: agent autonomy is valuable,
but humans should retain control over goals and high-stakes decisions
([Anthropic, safe and trustworthy agents](https://www.anthropic.com/news/our-framework-for-developing-safe-and-trustworthy-agents)).

**Fact boundary:** these sources support agent-led tool selection and risk-aware
boundaries. They do not imply that a model's confidence is evidence or that every
governance decision should be probabilistic.

### 4. Durable intent and staged artifacts are complementary

GitHub's Spec Kit treats the specification as a living source of truth rather
than a document written once and forgotten. It separates specification, technical
planning, task breakdown, and implementation, while the human steers and verifies
the generated artifacts. Tasks are intended to be small and independently
testable
([GitHub, Spec-driven development with AI](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/)).

This is evidence for preserving intent across the journey, not for requiring a
full PRD and architecture document for every fix. The source describes explicit
phases; it does not define a universal threshold for when each artifact is worth
its cost.

### 5. Work depth should follow appetite and risk, not a fixed ceremony

Shape Up asks teams to decide how much time and attention an idea deserves before
expanding the solution. Its "appetite" can describe a small or big batch; the
available appetite constrains the solution, and scope remains variable. It also
states that appetite limits how much research is worthwhile
([Shape Up, Set Boundaries](https://basecamp.com/shapeup/1.2-chapter-03)).

The Agile Manifesto favors working software over comprehensive documentation and
responding to change over following a plan. Its principles prefer shorter delivery
timescales, working software as the primary progress measure, and simplicity as
the amount of work not done
([Agile Manifesto](https://agilemanifesto.org/),
[principles](https://agilemanifesto.org/principles.html)).

**Fact boundary:** Shape Up's one/two-week and six-week batches belong to that
method. They should not become Forge thresholds. The transferable principle is to
bound investment before expanding scope.

## Synthesis and inference for Forge

This section is an inference from the sources and Forge's accepted premises.

### Objective and stages should form one model

> **Human clarification (2026-08-12):** every greenfield, brownfield, and Evolve
> cycle starts again at Analysis and Discovery. Stage depth is proportional, but
> the stages themselves are not optional. This clarification supersedes the
> earlier inference that a small task could skip explicit stages entirely.

Forge should not choose between a goal-only mode and a phase-heavy mode. A better
model is:

> **The lifecycle is the durable path. Every stage has an objective, and evidence
> determines how much work that stage needs in this cycle.**

Every task still passes through the same minimum questions:

1. What outcome is wanted, and what must not happen?
2. What is already true in the product and codebase?
3. What uncertainty could change the solution?
4. What is the smallest coherent action toward the outcome?
5. What evidence would show that action worked?
6. What changed in durable state, and what remains?

For small, clear, reversible work, the host agent still enters each stage, but a
stage may close after a short exchange and a compact durable result. It does not
need to manufacture a large PRD or architecture document when a few precise
fields are sufficient.

For large, uncertain, cross-cutting, or risky work, the same stages expand into
explicit research, discovery, PRD/specification, UX, architecture, decomposition,
implementation, review, and representative verification. Those artifacts are
useful because they reduce consequential uncertainty. They are not paperwork for
its own sake.

`Evolve` is not autonomous product growth. It is a user-triggered transition from
a stable product into a new lifecycle, starting again at Analysis and Discovery
with the existing product, evidence, feedback, and constraints as input.

### Project size alone is the wrong router

A one-line permission change can be high risk. A large mechanical rename can be
low risk. Routing should consider at least:

- impact if wrong;
- uncertainty about user need, domain, or existing system;
- reversibility and external side effects;
- security, privacy, financial, legal, or operational exposure;
- number of product and architecture boundaries crossed;
- quality and freshness of available evidence;
- expected work/appetite and cost of additional discovery.

These factors match Forge's current `minimum_sufficient_governance` principle more
closely than story count or lines of code.

### Three proportional journey shapes

These are recommended depths inside the same fixed lifecycle, not separate tracks:

| Shape | Typical signals | Visible durable material | Work still required |
| --- | --- | --- | --- |
| **Compact** | Clear outcome, local/reversible effect, low uncertainty | Objective, constraints, focused evidence, result/remaining gap | Inspect, choose a small plan, implement, verify, record |
| **Expanded** | Several boundaries or meaningful design uncertainty | Objective plus short discovery/spec/plan and explicit decisions | Research as needed, compare approaches, implement in slices, verify journeys |
| **Assurance-heavy** | High impact, weak evidence, external/irreversible effects, novel domain | Explicit research, claims, architecture, hazards, decomposed plan, approvals/waivers, representative evidence | Multiple challenge views, staged execution, recovery, independent or stronger verification where required |

The host agent should be allowed to expand or contract the journey as evidence
changes. Forge should explain why extra depth is recommended and what risk or gap
it addresses.

## Recommended orchestration shape

### One human entry, progressively disclosed agent interfaces

Keep one normal human command: `start-forge`. Do not make the human browse a
workflow catalog.

Behind it, expose four levels to the host agent:

1. **Bootstrap:** resolve project and executable identity, recover the durable
   objective and state, and enter the normal resume loop.
2. **Current guidance:** return the objective, material gaps, one ranked next
   action, a short reason, and only the few viable alternatives for this state.
3. **Capability discovery:** let the agent query the installed action/module
   catalog by intent, problem, stage, effect, or evidence need.
4. **Action detail:** return the full contract only for a chosen capability:
   inputs, structured invocation, outputs, effects, safety class, verification,
   and related actions.

This makes `start-forge` the thin bootstrap adapter, `workflow resume` the guide
for what matters now, and a runtime-owned catalog the guide for what Forge can do
when the current packet is not enough.

### The catalog should describe capabilities, not dictate reasoning

Each catalog action should be able to answer:

- What user/product problem can this help with?
- What signals make it applicable or unnecessary?
- What prerequisites and durable objects does it need?
- Is it read-only or mutating? Reversible or irreversible? Local or external?
- What authority is needed?
- What does it produce, and what counts as successful readback?
- What evidence does it create or require?
- What are its likely context, latency, and operational costs?
- Which adapters expose it in this installed version?

Names and short descriptions can be listed cheaply. Full schemas and playbooks
should load only after the agent selects or investigates an action. This is the
Forge equivalent of progressive disclosure; it avoids moving the entire catalog
from a large skill into a large startup response.

### "Intelligent orchestrator" should be a partnership

The recommended split is:

- **Forge deterministically derives:** durable objective, known obligations,
  evidence gaps, policy boundaries, action compatibility, risk metadata, and a
  ranked recommendation with reasons.
- **The host agent reasons about:** the conversation, repository reality, which
  uncertainty matters, which research or implementation tactic fits, and whether
  to accept an alternative catalog action.
- **The human decides:** desired outcome, preferences, material tradeoffs, risk
  acceptance, and irreversible or external effects.

The agent reports observations and results back to Forge; Forge recomputes the
next guidance from admitted state. Neither side pretends that a recommendation is
proof. This preserves the agent as mediator while keeping governance portable
across future models.

## Options and tradeoffs

### A. Keep the full catalog in `start-forge`

- **Benefit:** simplest implementation and no new runtime interface.
- **Cost:** startup context grows with every capability; version drift and stale
  prose become more likely; most loaded detail is irrelevant to the current task.
- **Assessment:** acceptable only as a temporary compatibility layer.

### B. Separate the large skill into linked reference files

- **Benefit:** matches the documented Agent Skills progressive-disclosure model
  and can reduce initial context quickly.
- **Cost:** the catalog remains adapter-owned prose unless generated from runtime
  authority; support varies by host; installed files can drift from the binary.
- **Assessment:** useful transition, not the final source of truth.

### C. Runtime catalog plus progressive guidance

- **Benefit:** capability metadata follows the installed Forge version; the same
  source can project to CLI, MCP, generated docs, and host skills; the agent loads
  only relevant detail.
- **Cost:** requires a clean capability schema, ranking semantics, adapter
  projections, compatibility rules, and evaluations for discovery quality.
- **Assessment:** recommended destination.

### D. Make Forge's core model-select all tools and steps

- **Benefit:** could appear more autonomous.
- **Cost:** conflicts with Forge's model-agnostic role, duplicates host-agent
  reasoning, risks brittle rules, and may turn recommendations into unexplained
  authority.
- **Assessment:** reject. Forge should constrain and inform selection, not replace
  the host agent's reasoning.

## Recommended next experiment

Do not migrate the whole catalog first. Prove one thin vertical slice:

1. Start with Analysis and Discovery after a new or materially changed objective.
2. Define compact stage and capability metadata from existing runtime owners, not
   in the `start-forge` prose.
3. Let `workflow resume` expose the stage objective and a bounded set of eligible
   capability summaries, with detailed readback for one chosen capability.
4. Let the host agent use the conversation to choose among those capabilities;
   Forge validates compatibility without deciding the semantic intent itself.
5. Exercise three journeys: a trivial fix with a very short discovery stage, a
   medium feature with targeted research, and a high-uncertainty product idea
   needing expanded brainstorming and research before planning.
6. Measure context size, time to first useful action, wrong/unused recommendations,
   successful resume by a replacement agent, and whether the human had to learn
   a Forge command.
7. Only after the runtime path proves useful, remove equivalent catalog prose
   from `start-forge`.

## BMAD Build and the former Quick Dev

The current BMAD release checked for this decision was `v6.11.0`. Its old
`bmad-quick-dev` entry is a compatibility shim that redirects to the single
`bmad-build` workflow:

- https://github.com/bmad-code-org/BMAD-METHOD/releases/tag/v6.11.0
- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/src/bmm-skills/v6-shims/bmad-quick-dev/SKILL.md
- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/docs/explanation/build.md

`bmad-build` clarifies one coherent intent and routes it to the smallest safe
path. Its one-shot route is limited to clear work with no architectural choice
and effectively zero blast radius. Other work still gets codebase investigation,
a compact spec, implementation, review, and final human review. Review findings
are classified so an intent gap or bad spec returns upstream instead of being
hidden by repeated code patches:

- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/src/bmm-skills/ship/bmad-build/step-01-clarify-and-route.md
- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/src/bmm-skills/ship/bmad-build/step-02-plan.md
- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/src/bmm-skills/ship/bmad-build/step-04-review.md
- https://github.com/bmad-code-org/BMAD-METHOD/blob/v6.11.0/src/bmm-skills/ship/bmad-build/step-oneshot.md

Forge should reuse the useful mechanics without copying BMAD's user-facing
phase skipping. A small Forge change still visits every lifecycle stage, but
stores the closeouts together in one compact Quick Cycle record. It expands the
affected stage when uncertainty, impact, architecture, risk, or review evidence
shows that the compact treatment is no longer enough. The human does not need to
choose a process mode; the host explains the evidence that caused expansion.

## Recommendation

Forge should preserve a single human-facing initialization command while moving
toward a runtime-owned, queryable capability catalog with progressive disclosure.
`workflow resume` should remain the context-specific guide, but it should expose
the current lifecycle stage, its objective, unresolved questions, and relevant
capabilities. The host agent interprets the conversation and chooses among those
capabilities; Forge validates compatibility, boundaries, and durable evidence.

Small tasks still pass through discovery, planning, solution definition,
implementation, validation/delivery, and later Evolve. Their stages are short and
their durable outputs compact. Large or risky work expands the same journey until
the consequential uncertainty is managed. This combines stage guidance with
objectives inside each stage instead of choosing one and discarding the other.

## Source limitations

- All external sources are first-party, but first-party product documentation can
  describe intended behavior more strongly than measured behavior.
- BMAD is evolving quickly. The links above pin its repository snapshot where
  possible; the research does not assume its present interface will remain stable.
- No source supplies a universal formula for planning depth. The proportional
  routing factors and three journey shapes are recommendations derived for Forge.
- This research did not benchmark BMAD, Spec Kit, Claude, or OpenAI agents and
  should not be read as a comparative performance evaluation.
