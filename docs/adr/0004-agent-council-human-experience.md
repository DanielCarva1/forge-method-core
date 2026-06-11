# ADR 0004: Agent Council As Human Experience With Compact Runtime Memory

## Status

Accepted

## Context

Forge Method should support rich human-facing collaboration without turning runtime files into long transcripts. A council-style discussion can help users see specialist perspectives and discover ideas they had not considered, but a full debate is expensive and poor future context for agents.

## Decision

Agent Council is an optional Human Experience workflow. When real Codex subagents are available and council is requested or appropriate, Forge can use them as specialist participants. When they are unavailable, the runtime helper provides a serial fallback.

The live council discussion is shown to the human. The durable project memory is a compact Council Decision Artifact, not the full transcript.

## Consequences

- Humans get a richer decision experience.
- Future agents load compact decision state instead of a long debate.
- Agent profiles may include persona and council role fields while keeping required task fields compact.
- Council is optional and should not become a mandatory blocker for normal workflows.
