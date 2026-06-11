# Specification: Self-Hosting Runtime Hardening

## Target Outcome

Forge Method Core can safely improve itself inside its own repository while still protecting normal users from confusing the runtime package with projects created by the runtime.

## Functional Requirements

- Runtime repo initialization must remain blocked by default and require explicit `--allow-runtime-state`.
- The `project create` command must support every flag named by its own refusal messages and preflight decisions.
- Story start must preserve the story phase and route to the matching workflow for discovery, specification, planning, build, ready, and evolve work.
- `$forge-method` must work from an installed skill and from a plugin checkout without requiring a hard-coded user-machine path.
- Preflight must keep three identities distinct: runtime repo, existing brownfield project, and normal Forge Method project.
- Agent-facing workflow references must stay compact state machines and avoid broad narrative docs.
- Progress-changing commands must write durable state, sprint, story, artifact, evidence, checkpoint, or ledger data.

## Non-Functional Requirements

- Runtime changes must be covered by Python unit tests.
- Install or packaging changes must be covered by install smoke tests before release, not after every small runtime edit.
- Development validation should use unit or fast checks; release checks should be reserved for a coherent batch.
- Product-facing documentation must describe Forge Method Core on its own terms.

## Acceptance Criteria

- A runtime repo cannot create `.forge-method` state accidentally.
- An intentional self-hosting command can create `.forge-method/state.yaml` in the runtime repo.
- Starting a discovery story leaves state in `1-discovery / discover-intent`.
- Starting a build story leaves state in `4-build-verify / build-story`.
- The installed local skill exposes the same runtime behavior as the repo version after `install.ps1`.
- README guidance for public plugin installation remains accurate for Codex users.
- Context and recovery commands select state, active workflow, active story, evidence, and checkpoint files without loading broad docs by default.

## Risks And Assumptions

- Plugin marketplace snapshots can lag behind the repo until users run marketplace upgrade or reinstall.
- Local `.agents` installs can shadow plugin installs in the UI; the package should tolerate both but documentation should prefer the plugin path for external users.
- Some Codex plugin behavior is UI-controlled, so the repo can make installation possible but cannot force an official marketplace listing or switch state.

## Planning Inputs

Recommended next stories:

1. Harden self-hosting guardrails and tests.
2. Make skill runtime path resolution plugin-native.
3. Compact workflow docs and enforce state-machine structure.
4. Refresh public installation docs and smoke the plugin install path once at batch end.
