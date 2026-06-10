# ADR 0001: File-Backed State Ledger

## Status

Accepted

## Context

Forge Method Core must survive context resets, terminal restarts, and agent handoffs. Conversation history is not reliable enough to be the source of project truth.

## Decision

Project state is stored in `.forge-method/` using small files:

- `state.yaml` for current phase, workflow, story, and next action
- `projects.yaml` for identity and registry metadata
- `sprint.yaml` for sprint summary
- `stories/*.yaml` for executable work
- `evidence/*.md` for proof of completion
- `context/*.md` for recovery packs
- `ledger.ndjson` for append-only runtime events

## Consequences

- Agents can recover by reading files instead of replaying chat history.
- Users can inspect and edit state without specialized infrastructure.
- The runtime must validate transitions and evidence because files are easy to mutate.

