# ADR-0005 - Memory as policy, not a vector DB

- **Status**: Proposed

## Context

Recent papers show that memory depends on admission, routing, compression level, and evidence support.

## Decision

Create `MemoryPolicy` before rich storage. A summary does not create authority. Promotion requires a boundary.

## Consequences

- Reduces memory poisoning.
- Enables forget and redaction.
- Keeps raw evidence.
- Separates episodic memory, skills, and rules.
