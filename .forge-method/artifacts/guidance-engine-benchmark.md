# Guidance Engine internal benchmark

- kind: internal-benchmark
- created_at: 2026-06-11
- sandbox: `%TEMP%/forge-bmad-sandbox`
- docs_cache: `%TEMP%/forge-bmad-docs`
- installed_modules: bmb 1.8.1, cis 0.2.1, tea 1.19.0, gds 0.6.0

Internal behavior benchmark for route-aware human guidance, correct-course, research, brainstorm, product/UX/architecture/quick-dev, story lifecycle, closure utilities, CIS/creative, game, builder, customization, lifecycle closure, and quality routing.

## Sources

- `https://bmad-builder-docs.bmad-method.org/llms-full.txt`
- `https://cis-docs.bmad-method.org/llms-full.txt`
- `https://game-dev-studio-docs.bmad-method.org/llms-full.txt`
- `https://bmad-code-org.github.io/bmad-method-test-architecture-enterprise/llms-full.txt`

## Observed behavior

- Help/orientation is route-aware. It reads configuration, available artifacts, phase metadata, required/preceded/followed relationships, and recommends the next workflow with a reason instead of dumping a catalog.
- Builder flows keep an open floor first, then ask focused questions once the agent has enough context to classify the user's goal.
- Innovation/problem-solving flows treat uncertainty as a real workflow input, not as a request for the user to choose from a technical menu.
- Game flows have game-specific entrypoints, correct-course, project context, engine setup/profile, project brief, GDD, sprint status, story cycle, prototype, playtest, performance, and review behavior instead of forcing generic software planning first.
- Test architecture flows sequence engagement model, risk strategy, fixture architecture, CI command contracts, ATDD, automation, review, NFR evidence, and two-phase traceability so quality requests route to the right artifact before implementation or release.
- Testing education requests should route to an applied teaching workflow before test strategy when the user is trying to learn or choose a quality approach.
- Builder utility flows analyze agents/workflows and convert skill material before scaffolding new runtime artifacts.
- Customization flows select team/local scope, choose supported override keys, validate merged behavior, and expose an effective capability index instead of relying on hidden prose.
- Persona/coach requests select a human-facing lens and elicitation technique set without copying long persona prose into agent profiles, state, or compact workflow docs.
- Documentation utility flows index, shard, review, stress-test, and distill source material before agents consume it.
- Lifecycle closure flows should turn project context, session handoff, track choice, code review, retrospectives, readiness, and research closeout into durable compact artifacts instead of relying on chat memory.
- Track decisions should preserve the chosen route, rejected routes, source signals, and required next workflows before more artifacts are created.
- Session prep should create read order, blockers, first command, state mutation rules, and continuation handoff for the next agent/session.
- Narrow guided workflows should be executable next steps: when selected inside an existing project, the route should include a state transition command instead of only restating the recommendation.
- Progressive disclosure should be machine-checked: agent-facing workflow refs stay compact state machines, human facilitation packs hold the richer coaching surface, and validation fails if the two layers blur.
- Correct-course is a first-class recovery path when the conversation shows the current route is wrong.
- Transcript corrections such as "do not solve the example project; understand the Forge experience" must be treated as method correct-course, even when they also mention runtime, Forge, benchmark, or guided flows.
- Runtime audit requests should be treated as guided builder work when the human asks about dead code, misleading agent docs, stale workflow behavior, or whether the guided experience is truly comparable.
- Builder creation requests should route to narrow Builder Factory workflows: module ideation before broad module build, agent builder before agent files, workflow builder before workflow files, module builder before packaging, and module validate for whole-extension checks.
- PRD requests should route to a create/update/validate product-requirements workflow with decision log, addendum, validation findings, and next workflow.
- PRFAQ and working-backwards requests should route to a customer-promise challenge before PRD, UX, architecture, or stories harden an untested promise.
- UX requests should route to UX planning with taste calibration, journeys, interaction model, accessibility, rejection log, and proof target before stories.
- Architecture requests should route to architecture planning that connects accepted product decisions to technical constraints, interfaces, risks, tradeoffs, validation hooks, and story boundaries; product architecture with PRD/UX trace should outrank generic quality routing even when the human mentions test hooks.
- Quick Dev / Quick Flow requests should route to a spec-lite workflow that clarifies scope, implements or hands off mechanically, reviews, validates, writes evidence, and names the next workflow.
- Story lifecycle requests should route to story-creation/readiness flows that require accepted decision sources, acceptance criteria, checks, evidence expectations, and a validation map before build-story.
- Sprint status requests should route to a status ritual that names story counts, active/blocked/review items, evidence gaps, and the next executable action.
- Investigation requests should diagnose symptom, hypotheses, probes, findings, and next reversible action before repair work.
- Adversarial review requests should attack assumptions and route repair, waiver, evidence, or rejection rather than hiding critique inside generic edge cases.
- Checkpoint preview requests should verify durable memory content before writing or handing off context.
- CIS/creative requests should route to the narrowest useful creative workflow before converging on specification: broad taste/direction requests stay in creative-session, design-thinking coach requests enter design-thinking, innovation strategy requests enter innovation-strategy, and storytelling requests enter storytelling.

## Forge parity targets

- `guide --question --json` must classify the latest human message against durable state and available workflows.
- Human frustration or rejection of the current route must override stale `next_action` and route to `correct-course` or `6-evolve`.
- Corrections about the method's own experience must route to `correct-course` first; `runtime-builder` is the repair path after the failed behavior is named.
- Broad ideas should receive a guided discovery, brainstorm, research, game, creative, quality, or builder workflow before technical implementation plans; option-generation language should route to brainstorming before generic confusion, while taste-heavy broad creative requests still route to creative-session and specific CIS strategy/story/design requests route to their narrow packs.
- Brainstorming should preserve option lanes, selection criteria, taste anchors, anti-goals, discard pile, risk/evidence needs, top candidates, and next workflow in a compact artifact.
- CIS Facilitation Depth should preserve design-thinking user/opportunity/prototype proof, innovation option/evidence/adoption/reversibility proof, and storytelling audience/pressure/payoff/rejected-path proof in compact artifacts while the human pack stays rich.
- Agent Compactness Guard should expose a deterministic `workflow compactness` check and wire it into normal workflow validation, runtime smoke, audit, and unit coverage.
- Confusion should produce one recommended route and a small set of alternatives.
- Mechanical build requests should continue autonomously when decision artifacts and stories are already ready.
- Method/runtime audit requests should route to runtime-builder instead of generic operate/support, especially when they mention scripts, dead code, misleading docs, agent behavior, or human-guided experience.
- Product planning, UX design, and quick-dev requests should route to narrow executable workflows rather than generic build-story or stale state.
- Story creation requests should not create ready build stories from vague intent; they need accepted source artifacts and a validation map.
- Mechanical build loops should continue through story start/review/fix/evidence/ready gate without asking for procedural "ok" once stories are ready.
- Fresh chat, network drop, reload, or context-reset messages should route to context-recovery and expose a compact context boundary: trust launcher output and durable state, load only read-first files, then run Guidance Engine for fresh human intent.
- Runtime outputs must remain compact JSON/state-machine artifacts for agents, while non-JSON guidance can be human, direct, and useful.
- Builder Factory outputs must keep coached human creation in facilitation packs and compact agent contracts in workflow refs, templates, catalog metadata, and validation reports.
- Project Configuration outputs must make customization visible through inspect, Guidance Engine metadata, validation, and the generated Capability Index.
- Persona Lens outputs must preserve a compact `persona_lens` object, route PM/Architect/UX/QA/Game/Builder/Tech Writer and coach requests, and keep default agent recommendations compact.
- Lifecycle Closure outputs must preserve route, source material, findings, decisions, checks, next workflow, and load hints without storing full transcripts.
- Runtime-builder/systematic parity batch names must outrank loose domain words such as "product", "context", "review", or "retro" when the state is `6-evolve` or runtime-builder.
- Game Studio Depth outputs must route game-specific requests to game-context, engine-setup, GDD, narrative, mechanics, quick-prototype, playtest, performance, or game QA workflows before generic software planning; artifacts must preserve player fantasy, engine profile, playable-slice proof, decision sources, and validation evidence compactly.
- Test Architecture Enterprise Depth outputs must route quality requests to the right engagement mode and workflow, preserve fixture architecture and command contracts, and make gate outcomes distinguish pass, concerns, fail, missing evidence, and explicit waiver.

## Fixture workflow ids

- `correct-course`
- `game-brief`
- `game-context`
- `engine-setup`
- `gdd`
- `narrative-design`
- `mechanics-design`
- `engine-architecture`
- `quick-prototype`
- `playtest-plan`
- `performance-plan`
- `game-qa-review`
- `problem-solving`
- `investigation`
- `domain-scan`
- `brainstorming`
- `build-story`
- `runtime-builder`
- `product-requirements`
- `working-backwards-challenge`
- `ux-plan`
- `architecture`
- `quick-dev`
- `story-creation`
- `sprint-status`
- `context-recovery`
- `creative-session`
- `game-story-creation`
- `traceability-gate`
- `teach-testing`
- `test-strategy`
- `test-engagement-model`
- `test-framework`
- `ci-quality-pipeline`
- `atdd-plan`
- `test-automation`
- `test-review`
- `nfr-evidence-audit`
- `workflow-analyze`
- `module-ideation`
- `agent-builder`
- `workflow-builder`
- `module-builder`
- `module-validate`
- `config-customization`
- `doc-index`
- `adversarial-review`
- `design-thinking`
- `track-decision`
- `project-context`
- `session-prep`
- `checkpoint-preview`
- `code-review`
- `retrospective`
- `research-closeout`
- `readiness-check`

## Non-goals

- Do not describe Forge Method publicly as a clone, fork, or variant of another framework.
- Do not copy public product language. Use this artifact only as an internal behavior benchmark.
