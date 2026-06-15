# P1.5 Game Studio Depth Grill

- kind: grill
- created_at: 2026-06-15
- scope: P1.5 Game Studio Depth

## Frame

P1.5 is not a new game module. The `game-studio` module already exists, but several workflow ids are shallow or unreachable from Guidance Engine. The batch should deepen game-specific routing and artifacts so game projects do not collapse into generic software planning.

## Questions Resolved

1. Should engine support create separate Forge workflows for Godot, Unity, Unreal, Phaser, and future engines?
   Recommended answer: no. Use a single `engine-setup` workflow with an `engine_profile`. The profile carries engine-specific setup assumptions while `$forge-method` keeps one route surface.

2. What must Game Studio preserve for humans?
   Recommended answer: player fantasy, loop, reference posture, engine choice, smallest playable proof, playtest learning, and performance constraints.

3. What must Game Studio preserve for agents?
   Recommended answer: compact artifact paths, engine profile, playable-slice target, story order, validation proof, commands/checks, and next workflow.

4. Which current gap is highest risk?
   Recommended answer: routing. Workflows such as `gdd`, `narrative-design`, `mechanics-design`, `quick-prototype`, `playtest-plan`, `performance-plan`, and `game-qa-review` exist, but Guidance Engine often defaults game requests to `game-brief`.

## Decisions

- Canonical family: Game Studio Depth.
- Canonical slice term: Playable Slice.
- Add `game-context` as the game-specific context/handoff workflow.
- Add `engine-setup` as the engine setup workflow. Keep `engine-architecture` for architecture decisions and route setup before architecture when the human asks for project scaffolding, engine template, folder layout, or first run command.
- Expand existing game templates instead of creating long agent workflow docs.

## Implementation Direction

- Add/expand compact refs, catalog metadata, templates, and replay fixtures for game context, engine setup, GDD, narrative, mechanics, prototype, playtest, performance, QA, and first playable slice routing.
- Keep human depth in `game-lifecycle.md`; keep workflow refs compact.
- Add tests that prove game route depth and that runtime-builder P1.5 requests still remain builder work.
