# Forge documentation

Forge is a local agent-first governance runtime. The current product milestone
is Solo Dogfood Ready: humans stay in chat and host agents operate `forge-core`
under the `solo_cooperative` profile. Enterprise trust operation is deferred.

## One guide per audience

| Audience | Canonical guide |
|---|---|
| Human using Forge through chat | [Getting started](getting-started.md) |
| Host-agent or tool integrator | [Agent integration](agent-integration.md) |
| Advanced state/recovery or future enterprise trust operator | [Operator guide](operator-guide.md) |
| Domain Pack author/operator | [Domain Packs](domain-packs.md) |
| Contributor | [Contributing](contributing.md) |
| Security or promise reviewer | [Security model](security-model.md) |
| Fork/extension maintainer | [Forking and customization](forking.md) |

## Reference index

- [Security model](security-model.md) states protected properties, the trust
  domains, the cooperative filesystem boundary, and platform support
  (Linux/macOS/WSL2 production; Windows native best-effort).
- [Architecture](architecture.md) explains layers and authority flow.
- [Verification](verification.md) defines Tier 0, focused, platform, and
  cumulative evidence with budgets, triggers, and timing artifacts.
- [Host conformance](host-conformance.md) defines the open solo journey kit and
  its integrity-versus-authenticity limit.
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
The [root README](../README.md) alone owns the current source-version statement;
`Cargo.toml` and `forge-core --version` remain the executable facts. Historical
version numbers belong in the changelog, not copied across live guides.

External-origin brokers, FIDO-backed presence, independent actor custody, and
compliance signing are later enterprise concerns. Their absence is not a Solo
Cooperative onboarding or progression gap.
