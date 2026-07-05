# ADR-0004 - Trace and eval as part of the product

- **Status**: Proposed

## Context

Without trace, there is no reproducibility, debug, governance, or reliable eval.

## Decision

Every relevant run generates a `TraceEvent`. Every architecture feature must have comparable eval.

## Consequences

- Debug improves.
- QA becomes measurable.
- External agents can be audited.
- Power users gain replay and metrics.
