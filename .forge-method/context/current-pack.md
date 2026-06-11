# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 5-ready-operate
- status: ready
- workflow: ready-release
- active_story: <none>
- next_action: use, support, observe, and maintain the ready product

## Latest Checkpoint

# Checkpoint

- created_at: 2026-06-11T04:21:27+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: ready
- workflow: ready-release
- active_story: <none>

## Summary

Self-hosting Forge Method run completed. Brownfield discovery, spec, plan, implementation stories, gate, and ready transition completed. Runtime fixes: project create --allow-runtime-state, phase-preserving story start. Skill packaging fix: prefer active skill/plugin directory. Validations: python -m unittest discover -s tests passed; smoke-install passed; verify-fast passed; gate --require-evals passed with 6/6 evals.

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

use, support, observe, and maintain the ready product

## Recovery Signals

### Failed Checks

- none

### Touched Files

- state.yaml
- sprint.yaml
- .forge-method/stories/project-kickoff.yaml
- .forge-method/artifacts/project-brief.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- operator (Operator): Maintain a ready project through usage notes, support status, feedback, and future backlog.
- quality-reviewer (Quality Reviewer): Review implementation, artifacts, workflows, and evidence before work is marked done or ready.

## Recent Evidence

- .forge-method/evidence/20260611-041618-story-story-runtime-self-hosting-guards-done.md
- .forge-method/evidence/20260611-041731-story-story-compact-workflow-docs-done.md
- .forge-method/evidence/20260611-041845-story-story-plugin-native-skill-path-done.md
- .forge-method/evidence/20260611-042030-story-story-public-plugin-install-proof-done.md
- .forge-method/evidence/20260611-042058-release-ready-gate.md

## Recent Artifacts

- plan [active/durable]: .forge-method/artifacts/self-hosting-hardening-backlog.json - Self-hosting hardening backlog - Four-story implementation batch for self-hosting guardrails, plugin-native skill paths, compact workflow docs, and public plugin install proof.
- audit [active/durable]: .forge-method/artifacts/workflow-compactness-audit.md - Workflow compactness audit - Confirmed all packaged workflow references use the required compact state-machine sections and pass workflow validation.
- story-link [active/durable]: .forge-method/artifacts/workflow-compactness-audit.md - .forge-method/artifacts/workflow-compactness-audit.md -> compact-workflow-docs - Artifact linked to story.
- evidence [active/durable]: .forge-method/artifacts/public-plugin-install-proof.md - Public plugin install proof - README public Codex install flow is present; package contents are covered; verify-fast and smoke-install passed for repo-based plugin use.
- story-link [active/durable]: .forge-method/artifacts/public-plugin-install-proof.md - .forge-method/artifacts/public-plugin-install-proof.md -> public-plugin-install-proof - Artifact linked to story.
