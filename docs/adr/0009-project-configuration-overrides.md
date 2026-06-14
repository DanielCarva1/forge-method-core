# ADR 0009: Project Configuration Overrides

## Status

Accepted

## Context

Forge Method already has packaged workflow metadata, agent profiles, facilitation packs, templates, and project state. Project-level customization existed only as a small set of loose config keys, so future agents could not reliably know which behavior was intentionally customized.

## Decision

Project Configuration is the canonical override surface for a Method Project. Overrides are narrow, validated, and applied in deterministic precedence: packaged defaults first, team config second, local config last.

The Capability Index is generated from the effective runtime view rather than manually maintained. This keeps human/team customization visible to Guidance Engine and future agents without turning workflow docs into a second mutable registry.

## Consequences

Invalid or stale override references must fail loudly during `config validate`, `builder validate`, and normal runtime validation. Valid overrides must be visible through `config inspect`, `config index`, and Guidance Engine metadata.
