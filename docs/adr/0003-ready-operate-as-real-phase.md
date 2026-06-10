# ADR 0003: Ready Operate As A Real Phase

## Status

Accepted

## Context

Agentic projects can stay indefinitely in implementation unless the runtime has a distinct completion state.

## Decision

`5-ready-operate` is a first-class phase. Entering it requires the Ready Gate:

- audit passes
- no story remains in progress or review
- release evidence exists
- readiness is written to state

## Consequences

- A project can be declared ready without losing future evolution paths.
- Phase 6 handles later feedback, defects, and version cycles.
- Build workflows must stop when ready criteria are met.

