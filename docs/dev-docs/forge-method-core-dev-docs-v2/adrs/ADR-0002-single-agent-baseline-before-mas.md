# ADR-0002 - Single-agent baseline before multi-agent

- **Status**: Proposed

## Context

Recent papers show that a homogeneous MAS does not automatically beat a well-controlled single-agent.

## Decision

Every multi-agent architecture must be compared against a single-agent anchor with the same loader, tools, answer contract, and usage accounting.

## Consequences

- Multi-agent stops being marketing and becomes a measured decision.
- `forge eval compare` becomes a central feature.
- MAS is only recommended when there is heterogeneity, parallelism, isolation, or real governance.
