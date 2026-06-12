# Guidance Engine internal benchmark

- kind: internal-benchmark
- created_at: 2026-06-11
- sandbox: `%TEMP%/forge-bmad-sandbox`
- docs_cache: `%TEMP%/forge-bmad-docs`
- installed_modules: bmb 1.8.1, cis 0.2.1, tea 1.19.0, gds 0.6.0

Internal behavior benchmark for route-aware human guidance, correct-course, research, brainstorm, game, builder, and quality routing.

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
- Documentation utility flows index, shard, review, stress-test, and distill source material before agents consume it.
- Narrow guided workflows should be executable next steps: when selected inside an existing project, the route should include a state transition command instead of only restating the recommendation.
- Correct-course is a first-class recovery path when the conversation shows the current route is wrong.

## Forge parity targets

- `guide --question --json` must classify the latest human message against durable state and available workflows.
- Human frustration or rejection of the current route must override stale `next_action` and route to `correct-course` or `6-evolve`.
- Broad ideas should receive a guided discovery, brainstorm, research, game, creative, quality, or builder workflow before technical implementation plans.
- Confusion should produce one recommended route and a small set of alternatives.
- Mechanical build requests should continue autonomously when decision artifacts and stories are already ready.
- Runtime outputs must remain compact JSON/state-machine artifacts for agents, while non-JSON guidance can be human, direct, and useful.

## Fixture workflow ids

- `correct-course`
- `game-brief`
- `problem-solving`
- `domain-scan`
- `brainstorming`
- `build-story`
- `runtime-builder`
- `game-story-creation`
- `traceability-gate`
- `teach-testing`
- `workflow-analyze`
- `doc-index`

## Non-goals

- Do not describe Forge Method publicly as a clone, fork, or variant of another framework.
- Do not copy public product language. Use this artifact only as an internal behavior benchmark.
