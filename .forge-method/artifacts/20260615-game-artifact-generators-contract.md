# Game Artifact Generators Contract

- created_at: 2026-06-15T22:35:00+00:00
- project: forge-method-core
- phase: 6-evolve
- workflow: runtime-builder
- status: game-artifact-generators-added

## Problem

Game Brief and Game Sprint Planning already had templates and `artifact game-check`, but future agents still had to hand-write the artifacts. That left the human-guided game flow vulnerable to generic backlog output instead of player-fantasy, playable-slice, and sprint-proof contracts.

## Runtime Contract

`artifact game-brief` writes, validates, and registers a durable `game-brief` artifact with:

- `source_material`, `player_fantasy`, `core_loop`, `player_verbs`, `target_player`, `platform_or_engine`
- `pillars`, `references`, `mvp_playable_proof`, `dream_game`, `vertical_slice`, `playable_slice`
- `parked_scope`, `rejected_directions`, `decision_log`, `assumptions`, `open_questions`, `research_needed`
- `validation`, `validation_verdict`, `next_workflow`

`artifact game-sprint-plan` writes, validates, and registers a durable `game-sprint-plan` artifact with:

- `source_material`, `player_fantasy`, `playable_slice`, `playable_slice_goal`, `decision_sources`
- `story_batch`, `player_value_order`, `risk_order`, `dependencies`, `engine_or_asset_constraints`
- `validation_plan`, `manual_playtest_plan`, `deferred_scope`, `blocked_items`, `next_story`, `sprint_update`
- `validation`, `next_workflow`

Both commands use `game_artifact_findings`, default validation to `artifact game-check --path <artifact>`, roll back invalid generated files, register durable artifacts, and can emit artifact existence evals.

## Human Guidance Contract

Game guidance now has an executable path:

1. let the human dump the fantasy and references
2. extract concrete player fantasy, loop, verbs, pillars, and proof
3. use `artifact game-brief` before GDD/prototype/sprint planning
4. order playable-slice work with `artifact game-sprint-plan`
5. run `artifact game-check`
6. hand off the next workflow or next story

## Validation

- new generator unit test for game brief and game sprint plan passed
- existing `artifact game-check` contract test passed
- packaged workflow/facilitation validation test passed
- game depth compactness regression test passed
- `workflow validate` passed
- `workflow compactness` passed
- `parity replay` passed with 90/90 cases
- `smoke-runtime.ps1` passed with source checkout generator coverage
- `smoke-install.ps1` passed with installed skill generator coverage
- full unittest discovery passed with 96 tests
- `verify-fast.ps1` passed

## Next Gap

Continue post-parity Forge polish by auditing remaining validator-only artifacts and converting stable handoff contracts into first-class generators where the human workflow benefits.
