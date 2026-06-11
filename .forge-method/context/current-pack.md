# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 5-ready-operate
- status: story-done
- workflow: ready-release
- active_story: <none>
- next_action: publish 1.25.0 batch to GitHub

## Latest Checkpoint

# Forge Method 1.25 self-update batch

- created_at: 2026-06-11T15:26:56+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: story-done
- workflow: ready-release
- active_story: <none>

## Summary

Implemented single-pass self-update for Git marketplace installs, compact Hot Start Stub, release notes feed, updater tests, docs, and installer packaging updates. Validation passed: unit tests, workflow validate, verify-fast, smoke-install, smoke-runtime, and final gate.

## Decisions

- none

## Checks

- none

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

publish 1.25.0 batch to GitHub

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-grill-gate.md
- docs/adr/0005-mechanical-autonomy-and-grill-gates.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- operator (Operator): Maintain a ready project through usage notes, support status, feedback, and future backlog.
- quality-reviewer (Quality Reviewer): Review implementation, artifacts, workflows, and evidence before work is marked done or ready.

## Recent Evidence

- .forge-method/evidence/20260611-062639-story-story-mechanical-autonomy-grill-gate-done.md
- .forge-method/evidence/20260611-062657-gate-quality-gate.md
- .forge-method/evidence/20260611-063437-gate-quality-gate.md
- .forge-method/evidence/20260611-152452-story-story-self-update-hot-start-done.md
- .forge-method/evidence/20260611-152635-gate-quality-gate.md

## Recent Artifacts

- evidence [active/durable]: .forge-method/artifacts/public-plugin-install-proof.md - Public plugin install proof - README public Codex install flow is present; package contents are covered; verify-fast and smoke-install passed for repo-based plugin use.
- story-link [active/durable]: .forge-method/artifacts/public-plugin-install-proof.md - .forge-method/artifacts/public-plugin-install-proof.md -> public-plugin-install-proof - Artifact linked to story.
- plan [active/durable]: .forge-method/artifacts/forge-expansion-backlog.json - Forge expansion backlog - Four-story implementation batch for guide/tracks/council, builder/config/evals, creative/game/enterprise packs, and docs/install proof.
- roadmap [active/durable]: docs/09-expansion-roadmap.md - Expansion roadmap - Public roadmap defining Human Experience, Agent Runtime, tracks, Agent Council, and v1.23-v1.26 delivery packages.
- adr [active/durable]: docs/adr/0004-agent-council-human-experience.md - Agent Council ADR - Decision to show rich council debate to humans while persisting compact council decision artifacts for future agents.
