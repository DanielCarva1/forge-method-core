# ADR-0003 - WorkflowGraph as a first-class entity

- **Status**: Proposed

## Context

Loose prompt routing produces loops, hallucinated routing, and non-reproducible execution. Recent literature points toward executable graphs.

## Decision

Create `WorkflowGraph` v0. `OperationContract` continues to exist, but enters as a node or as a node payload.

## Consequences

- Better dry-run.
- Better parallelism.
- Verifier and replan become structural.
- Trace now binds to node_id and graph_id.
