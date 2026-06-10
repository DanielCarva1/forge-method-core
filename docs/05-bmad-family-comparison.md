# BMAD Product Family Comparison

This document compares the BMAD product family against the Forge Method Core direction. It is derived from the public `llms-full.txt` documentation links, not copied from them.

## Sources

- BMAD Method: https://docs.bmad-method.org//llms-full.txt
- BMad Game Dev Studio: https://game-dev-studio-docs.bmad-method.org/llms-full.txt
- Creative Intelligence Suite: https://cis-docs.bmad-method.org/llms-full.txt
- BMad Builder: https://bmad-builder-docs.bmad-method.org/llms-full.txt

## What BMAD Gets Right

| BMAD product | Strong idea to preserve | Runtime interpretation |
| --- | --- | --- |
| BMAD Method | Progressive planning into implementation | Keep phase gates, but make phase/state machine explicit in files |
| BMAD Method | Named roles and workflow discovery | Use Codex skills/subagents instead of large agent docs |
| BMAD Method | Sprint/status/build cycle | Make sprint state deterministic and evidence-backed |
| Game Dev Studio | Domain-specific game phases and engine guidance | Implement as a `game-studio` module with engine adapters |
| Game Dev Studio | Game designer, architect, developer, scrum roles | Use focused subagents/workflows with explicit handoff contracts |
| Creative Intelligence Suite | Creative agents for brainstorming, design thinking, innovation, storytelling, presentation | Implement as `creative-studio` workflows that produce artifacts for later phases |
| BMad Builder | Ability to create new modules, agents, workflows, evals | Make a `runtime-builder` module that creates Codex skills and validates them |

## Main Problem To Improve

BMAD is complete, but its agent-facing documentation is heavy. The core docs and Builder docs are both large enough that repeated loading can waste context and blur current state.

Forge Method should keep:

- human docs for explanation
- compact agent docs for execution
- deterministic scripts for state/status
- references loaded only by current workflow

## Product Family Map

```txt
forge-method-core
  core-runtime
    route
    discovery
    specification
    planning
    build-verify
    ready-operate
    evolve
  modules
    software-builder
    creative-studio
    game-studio
    test-architect
    runtime-builder
    launch-ops
```

## BMAD Core -> Forge Method Core

BMAD Core has strong planning and implementation sequence:

```txt
analysis -> planning -> solutioning -> implementation
```

Forge Method should expand that into:

```txt
0-route -> 1-discovery -> 2-specification -> 3-plan -> 4-build-verify -> 5-ready-operate -> 6-evolve
```

The added phases solve two problems:

- `0-route` prevents the agent from confusing the runtime repo with a child project.
- `5-ready-operate` prevents the project from staying forever in implementation.

## Game Dev Studio -> Game Module

BMAD Game Dev Studio is strong because it is domain-specific. It understands:

- prototype first
- game type and core mechanic
- game brief
- GDD
- technical architecture
- engine-specific setup
- sprint planning
- production implementation

Forge Method should not turn game work into generic software tasks too early. It should keep a separate game path:

```txt
game-intent -> prototype-loop -> game-brief -> gdd -> engine-architecture -> vertical-slice -> production
```

Codex-native improvement:

- use Browser for web games
- use engine-specific scripts/checks when available
- keep art/audio/design prompts as artifacts
- separate playable validation from code validation

## Creative Intelligence Suite -> Creative Module

CIS is useful because it is not only "write content". It includes ideation, design thinking, innovation analysis, problem solving, storytelling, and presentation.

Forge Method should preserve this as a first-class module:

```txt
creative-question -> divergent generation -> structured selection -> prototype artifact -> critique -> next action
```

Codex-native improvement:

- use Canva/Figma/image generation when the output is visual
- use artifacts and evidence instead of dumping brainstorming into chat
- turn selected ideas into specs/tasks only after a decision gate

## BMad Builder -> Runtime Builder Module

BMad Builder is the most relevant product for this runtime. It creates modules, agents, workflows, and evaluates them.

Forge Method should turn this into:

```txt
module-intent -> skill-contract -> workflow-state-machine -> scripts -> evals -> plugin package -> smoke test
```

Codex-native improvement:

- create actual Codex skills
- generate `agents/openai.yaml`
- validate skill/plugin shape
- run trigger evals and artifact evals
- ship modules as plugin-compatible packs

## Design Decision

The runtime should not have many user-facing commands.

The user should start one method entrypoint. After that, the runtime should route by state:

```txt
$forge-method
```

Then:

```txt
read state -> choose workflow -> run workflow -> update state
```

The product surface is one entrypoint plus state, not a menu full of commands.
