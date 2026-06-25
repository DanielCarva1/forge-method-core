# ADR 0015: Product Areas And Team Collaboration

## Status

Accepted

## Context

Forge already uses `Module` for packaged runtime capabilities and workflow families. Teams also need product modularization, ownership, GitHub conventions, trunk-based work, and eventual repo split support. Reusing `module` for product boundaries would make agents route product architecture work into runtime module workflows.

## Decision

Product modularization uses the term `Product Area`. Forge Module remains reserved for runtime/workflow packaging.

The default collaboration topology is monorepo first: one Root Integrator Project coordinates Product Areas, owners, trunk-based policies, evidence, and integration contracts. A Product Area may be split into a separate repo only when owner, contract, validation boundary, release boundary, and integration cost justify it.

When a Product Area is split, the new repo becomes a Standalone Method Project with its own `.forge-method/` state. The Root Integrator Project keeps only the integration contract, repo pointer, expected version or interface, validation evidence, and owner.

## Consequences

Guidance Engine must route team, trunk-based, Product Area, and repo split language to collaboration workflows before story/build automation. Agents must not use Forge Module commands as the product modularization answer. Repo split planning must preserve compact context and contracts, not copy the full integrator history into the standalone repo.
