# ADR 0002: Single Entrypoint With State Routing

## Status

Accepted

## Context

Many commands create routing confusion for users and agents. The runtime should feel like one method that chooses the correct workflow from durable state.

## Decision

The public Codex product entrypoint is `$forge-method`. The helper script exposes deterministic subcommands for state operations, but users should not need to memorize many workflow commands. Operational maintenance exceptions are documented in ADR 0014.

Workflow selection is based on:

- current phase
- active workflow
- active story
- module
- audit result
- next action

## Consequences

- The user experience stays simple.
- Workflow routing must be precise and file-backed.
- Helper commands remain implementation surface, not product surface.
