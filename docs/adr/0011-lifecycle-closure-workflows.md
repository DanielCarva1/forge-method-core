# ADR 0011: Lifecycle Closure Workflows

## Status

Accepted

## Context

Forge already has low-level helpers for context packs, review findings, checkpoints, status, and readiness. The parity gap is not raw capability; it is that humans need guided rituals for "document this project", "prep next session", "review this code", "retro this increment", and "close research" while future agents need compact handoffs.

## Decision

Model those rituals as Lifecycle Closure workflows with facilitation, catalog routing, templates, replay coverage, and compact artifacts. The helpers remain implementation mechanisms, but Guidance Engine routes human intent to workflows such as `project-context`, `session-prep`, `code-review`, `retrospective`, `readiness-check`, `track-decision`, and `research-closeout`.

## Consequences

Lifecycle Closure workflows must preserve next workflow state and avoid becoming long transcripts. Runtime command output remains compact, while human guidance explains why the closure ritual matters and what future agents should load next.
