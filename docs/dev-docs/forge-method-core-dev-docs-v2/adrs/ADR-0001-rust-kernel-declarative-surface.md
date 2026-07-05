# ADR-0001 - Rust as deterministic kernel, declarative living semantics

- **Status**: Proposed

## Context

Forge suffers when agents have to edit hand-written Rust for every semantic change. The codebase already shows the value of Rust for contracts, runtime, store, WAL, and validation, but it also shows growing boilerplate.

## Decision

Rust stays in the deterministic kernel. Prompts, in-flow policies, templates, experimental workflows, and docs stay declarative until they stabilize.

## Consequences

- Less pain for code agents.
- More codegen and builders.
- Less duplication between YAML, Rust, docs, and tests.
- The kernel stays safe and auditable.
