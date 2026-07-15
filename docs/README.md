# Forge documentation

Forge is a local agent-first governance runtime. Humans normally stay in chat;
host agents operate `forge-core`; operators own installation and trust.

## One guide per audience

| Audience | Canonical guide |
|---|---|
| Human using Forge through chat | [Getting started](getting-started.md) |
| Host-agent or tool integrator | [Agent integration](agent-integration.md) |
| Installation, trust, state, or recovery operator | [Operator guide](operator-guide.md) |
| Domain Pack author/operator | [Domain Packs](domain-packs.md) |
| Contributor | [Contributing](contributing.md) |
| Security or promise reviewer | [Security model](security-model.md) |
| Fork/extension maintainer | [Forking and customization](forking.md) |

## Reference index

- [Product status](product-status.md) and [promise audit](product-compliance-audit.md)
  state what source proves and what remains open.
- [Architecture](architecture.md) explains layers and authority flow.
- [Verification](verification.md) defines Tier 0, focused, platform, and
  cumulative evidence with budgets, triggers, and timing artifacts.
- [Real-host proof](real-host-proof.md) defines the structural-only P7F bundle
  checker boundary.
- [Generated command surface](generated/command-surface.md) and
  [workspace layout](generated/workspace-layout.md) are machine-checked references.
- [Root README](../README.md) contains the canonical four-identity and storage
  tables; [CONTEXT](../CONTEXT.md) defines domain language; the
  [Changelog](../CHANGELOG.md) records source checkpoints.

## Authority order

1. Admitted compiled material and runtime receipts define executable behavior.
2. Closed contracts under `contracts/` define accepted wire shapes/invariants.
3. Generated references must remain byte-current with code.
4. Prose explains use and limits but cannot grant authority.
5. Fixtures are examples/adversarial evidence, not trusted merely because copied.

When prose and a machine-checked surface disagree, fail closed and report drift.
The source workspace is `0.12.0`; `v0.4.0` is its historical prebuilt
predecessor, while current availability must be checked on GitHub Releases. No page
in this set claims a published `0.12.0`, a completed real-host run, actor
independence, or full P7 completion.
