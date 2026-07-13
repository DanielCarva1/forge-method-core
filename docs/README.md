# Forge documentation

Forge is an agent-first governance runtime. Humans are expected to stay in
chat; host agents operate the `forge-core` binary and persist typed evidence in
the project sidecar. These documents are for people integrating, auditing, or
forking Forge—not for day-to-day project operation.

## Choose a path

| Audience | Start here | Then read |
|---|---|---|
| Human using Forge through chat | [Getting started](getting-started.md) | [Product status](product-status.md) |
| Host-agent or tool integrator | [Agent integration](agent-integration.md) | [Architecture](architecture.md), [Security model](security-model.md) |
| Domain Pack author/operator | [Domain Packs](domain-packs.md) | [Security model](security-model.md) |
| Contributor | [Contributing](contributing.md) | [Architecture](architecture.md), [Verification](verification.md) |
| Fork maintainer | [Forking Forge](forking.md) | [Contributing](contributing.md), [Security model](security-model.md) |
| Promise/compliance reviewer | [Product compliance audit](product-compliance-audit.md) | [Product status](product-status.md), [Verification](verification.md) |

## Authoritative sources

Forge deliberately separates product explanation from machine authority:

1. Rust code and admitted, compiled release material define executable
   behavior.
2. Closed contracts under `contracts/` define accepted wire shapes and
   invariants.
3. Generated references under `docs/generated/` describe the current command
   and workspace surfaces and must remain byte-current in CI.
4. `README.md`, `CONTEXT.md`, and these guides explain intent and operation but
   cannot grant authority.
5. Fixtures under `docs/fixtures/` are executable examples and adversarial
   proofs, not trusted templates merely because they were copied.

When prose and a machine-checked surface disagree, fail closed and report the
documentation drift.

## Stable references

- [Root README](../README.md): product overview and complete command examples.
- [Domain context](../CONTEXT.md): bounded domain language and architecture.
- [Changelog](../CHANGELOG.md): release/checkpoint history.
- [Command surface](generated/command-surface.md): generated CLI reference.
- [Workspace layout](generated/workspace-layout.md): generated crate map.
- [Product program](../contracts/plan/agent-native-guidance-plan.yaml): typed
  delivery plan and acceptance evidence through P6.
