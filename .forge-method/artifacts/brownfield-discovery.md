# Brownfield Discovery: Forge Method Core

## Intent

Improve Forge Method Core using Forge Method itself as a self-hosting runtime. The product is a Codex-native state-machine workflow framework with durable file-backed state, plugin distribution, compact agent workflow references, evidence, context recovery, and project isolation.

## Existing Inventory

- Runtime package: `.codex-plugin/plugin.json`, `.agents/plugins/marketplace.json`, `skills/forge-method/SKILL.md`, launcher scripts, and Python runtime helper.
- Runtime engine: `skills/forge-method/scripts/forge_method_runtime.py` at version `1.22.0`.
- Workflow references: 12 compact state-machine markdown workflows under `skills/forge-method/references/`.
- Module packs: 7 manifests under `skills/forge-method/modules/`, including `runtime-builder`, `software-builder`, `creative-studio`, `game-studio`, `test-architect`, and launch/ops support.
- Agent profiles: 7 profiles under `skills/forge-method/agents/profiles/`.
- Distribution docs and onboarding: `README.md`, `docs/04-distribution.md`, marketplace listing metadata, and onboarding assets.
- Validation: Python unit tests, runtime smoke, install smoke, fast/full verification scripts, fixture matrix smokes, and CI workflow.
- Self-hosting state: `.forge-method/` now tracks this repo as a brownfield runtime-builder project.

## Current Behavior

- `preflight --root .` correctly detects the repo as `runtime-repo` and refuses accidental project-state initialization.
- Existing codebases without `.forge-method/state.yaml` route to brownfield discovery before planning or implementation.
- Public GitHub plugin installation is documented through `codex plugin marketplace add DanielCarva1/forge-method-core --ref main`, then Plugins UI install/enable.
- Project progress is tracked through `.forge-method/state.yaml`, `sprint.yaml`, story files, artifacts, evals, evidence, checkpoints, and ledger entries.

## In-Progress Work

- Added explicit self-hosting support by allowing `project create --allow-runtime-state` for intentional runtime repo initialization.
- Fixed `story start` so non-build stories preserve their phase workflow instead of always jumping to `4-build-verify / build-story`.
- Created the first self-hosting story, `project-kickoff`, to capture brownfield discovery before further runtime improvements.

## Constraints

- Product-facing docs must stay independent and must not describe Forge Method Core as a clone, fork, or variant of another framework.
- Agent-facing workflow docs should remain compact state machines with `trigger`, `inputs`, `steps`, `outputs`, `done_when`, `blocked_when`, and `handoff`.
- Runtime progress changes must update durable state, sprint, story, evidence, artifacts, or ledger.
- During active development, use targeted or fast validation. Do not run release checks repeatedly.
- Release/version batches should be coherent increments, not one tiny release per micro-change.

## Risks

- Self-hosting can confuse runtime code with projects created by the runtime if preflight and state routing drift.
- Plugin-installed skill paths and local `.agents` skill paths can diverge if the installed skill is stale.
- Story lifecycle commands can accidentally move work to the wrong phase if phase/workflow mapping is too implicit.
- Brownfield discovery can become verbose if it turns into broad documentation instead of a concise state artifact.

## Safe Change Boundaries

- Safe now: runtime parser/command fixes, state routing fixes, workflow doc compaction, tests for routing/lifecycle regressions, README/install clarification, context recovery improvements.
- Needs targeted validation: changes to `forge_method_runtime.py`, workflow transitions, project creation, plugin packaging, installer behavior.
- Needs release validation only at batch end: version bump, tag/release, marketplace/public distribution proof.
- Avoid: rewriting product identity docs around external frameworks, adding extra slash-command surfaces, or changing project state formats without compatibility tests.

## Success Criteria

- Self-hosting remains explicit and protected: runtime repo initialization requires `--allow-runtime-state`.
- Starting a discovery/planning/spec/ready story keeps the matching workflow.
- A user can install from the public repo, enable the plugin in Codex, start with `$forge-method`, and route correctly for empty, existing, parent, and runtime workspaces.
- Agents can resume from `.forge-method` state without relying on long chat history.

## Next Workflow

Move from discovery to specification after this artifact is linked as evidence. The next specification should define the next coherent runtime hardening batch, likely focused on self-hosting safety, plugin-native skill path behavior, and compact workflow/state-machine docs.
