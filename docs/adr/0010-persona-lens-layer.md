# ADR 0010: Persona Lens Layer

## Status

Accepted

## Context

Forge needs richer human guidance from specialist roles without bloating Agent Profiles, workflow docs, state, or recovery packs. The benchmark exposes named agents and coaches as a human experience surface, while Forge's runtime must stay compact and file-backed.

## Decision

Create Persona Lens as the human-facing overlay for named roles and coach styles. Persona Lens entries reference compact Agent Profiles, workflows, and elicitation techniques; Guidance Engine and Agent Council may select them, but future agents continue to consume compact profiles and workflow contracts.

## Consequences

Persona text must stay out of default `recommended_agents`, state files, workflow references, and recovery packs. Runtime-visible persona behavior is proven through `guide --question`, council participant routing, validation, replay fixtures, and the generated Capability Index.
