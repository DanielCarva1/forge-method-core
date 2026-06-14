# Guidance Engine internal benchmark

- kind: internal-benchmark
- created_at: 2026-06-11
- sandbox: `%TEMP%/forge-bmad-sandbox`
- docs_cache: `%TEMP%/forge-bmad-docs`
- installed_modules: bmb 1.8.1, cis 0.2.1, tea 1.19.0, gds 0.6.0

Internal behavior benchmark for route-aware human guidance, correct-course, research, brainstorm, product/UX/architecture/quick-dev, story lifecycle, CIS/creative, game, builder, customization, and quality routing.

## Sources

- `https://bmad-builder-docs.bmad-method.org/llms-full.txt`
- `https://cis-docs.bmad-method.org/llms-full.txt`
- `https://game-dev-studio-docs.bmad-method.org/llms-full.txt`
- `https://bmad-code-org.github.io/bmad-method-test-architecture-enterprise/llms-full.txt`

## Observed behavior

- Help/orientation is route-aware. It reads configuration, available artifacts, phase metadata, required/preceded/followed relationships, and recommends the next workflow with a reason instead of dumping a catalog.
- Builder flows keep an open floor first, then ask focused questions once the agent has enough context to classify the user's goal.
- Innovation/problem-solving flows treat uncertainty as a real workflow input, not as a request for the user to choose from a technical menu.
- Game flows have game-specific entrypoints, correct-course, project brief, sprint status, and story cycle behavior instead of forcing generic software planning first.
- Test architecture flows sequence risk, strategy, framework, CI, automation, review, and traceability so quality requests route to the right quality artifact before implementation.
- Testing education requests should route to an applied teaching workflow before test strategy when the user is trying to learn or choose a quality approach.
- Builder utility flows analyze agents/workflows and convert skill material before scaffolding new runtime artifacts.
- Customization flows select team/local scope, choose supported override keys, validate merged behavior, and expose an effective capability index instead of relying on hidden prose.
- Documentation utility flows index, shard, review, stress-test, and distill source material before agents consume it.
- Narrow guided workflows should be executable next steps: when selected inside an existing project, the route should include a state transition command instead of only restating the recommendation.
- Correct-course is a first-class recovery path when the conversation shows the current route is wrong.
- Transcript corrections such as "do not solve the example project; understand the Forge experience" must be treated as method correct-course, even when they also mention runtime, Forge, benchmark, or guided flows.
- Runtime audit requests should be treated as guided builder work when the human asks about dead code, misleading agent docs, stale workflow behavior, or whether the guided experience is truly comparable.
- Builder creation requests should route to narrow Builder Factory workflows: module ideation before broad module build, agent builder before agent files, workflow builder before workflow files, module builder before packaging, and module validate for whole-extension checks.
- PRD requests should route to a create/update/validate product-requirements workflow with decision log, addendum, validation findings, and next workflow.
- UX requests should route to UX planning with taste calibration, journeys, interaction model, accessibility, rejection log, and proof target before stories.
- Architecture requests should route to architecture planning that connects accepted product decisions to technical constraints, interfaces, risks, and story boundaries.
- Quick Dev / Quick Flow requests should route to a spec-lite workflow that clarifies scope, implements or hands off mechanically, reviews, validates, writes evidence, and names the next workflow.
- Story lifecycle requests should route to story-creation/readiness flows that require accepted decision sources, acceptance criteria, checks, evidence expectations, and a validation map before build-story.
- CIS/creative requests should route to creative-session/concept-selection style flows before converging on specification.

## Forge parity targets

- `guide --question --json` must classify the latest human message against durable state and available workflows.
- Human frustration or rejection of the current route must override stale `next_action` and route to `correct-course` or `6-evolve`.
- Corrections about the method's own experience must route to `correct-course` first; `runtime-builder` is the repair path after the failed behavior is named.
- Broad ideas should receive a guided discovery, brainstorm, research, game, creative, quality, or builder workflow before technical implementation plans.
- Confusion should produce one recommended route and a small set of alternatives.
- Mechanical build requests should continue autonomously when decision artifacts and stories are already ready.
- Method/runtime audit requests should route to runtime-builder instead of generic operate/support, especially when they mention scripts, dead code, misleading docs, agent behavior, or human-guided experience.
- Product planning, UX design, and quick-dev requests should route to narrow executable workflows rather than generic build-story or stale state.
- Story creation requests should not create ready build stories from vague intent; they need accepted source artifacts and a validation map.
- Mechanical build loops should continue through story start/review/fix/evidence/ready gate without asking for procedural "ok" once stories are ready.
- Runtime outputs must remain compact JSON/state-machine artifacts for agents, while non-JSON guidance can be human, direct, and useful.
- Builder Factory outputs must keep coached human creation in facilitation packs and compact agent contracts in workflow refs, templates, catalog metadata, and validation reports.
- Project Configuration outputs must make customization visible through inspect, Guidance Engine metadata, validation, and the generated Capability Index.

## Fixture workflow ids

- `correct-course`
- `game-brief`
- `problem-solving`
- `domain-scan`
- `brainstorming`
- `build-story`
- `runtime-builder`
- `product-requirements`
- `ux-plan`
- `architecture`
- `quick-dev`
- `story-creation`
- `creative-session`
- `game-story-creation`
- `traceability-gate`
- `teach-testing`
- `workflow-analyze`
- `module-ideation`
- `agent-builder`
- `workflow-builder`
- `module-builder`
- `module-validate`
- `config-customization`
- `doc-index`

## Non-goals

- Do not describe Forge Method publicly as a clone, fork, or variant of another framework.
- Do not copy public product language. Use this artifact only as an internal behavior benchmark.
