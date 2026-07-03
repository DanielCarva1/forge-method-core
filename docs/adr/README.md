# Architecture Decision Records (ADRs)

This repository uses **two complementary ADR registries** that together form
the canonical decision record for forge-method-core. Numbers never collide.

## Registries

### Registry A — `docs/adr/` (this directory)

Product-layout and memory-model decisions. Numbered from **0022** upward to
avoid collision with the planned-ADR block (0016–0021) reserved by
`contracts/migration/markdown-debt-inventory.yaml`.

| ADR | Title |
|-----|-------|
| [0022](0022-forge-runtime-sidecar.md) | Forge Runtime Sidecar for Consumer Projects |
| [0023](0023-memory-trust-model.md) | Two Orthogonal Trust Axes for Agent Memory |
| [0024](0024-memory-pep-store.md) | `forge-core-memory`: the PEP for the memory trust model |

### Registry B — `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/`

Core architecture decisions: kernel surface, protocol adapters, governance,
eventlog, diagnostics, the OperationGate typestate, and the runtime/engine
rename. Numbered **0001–0014**.

| ADR | Title |
|-----|-------|
| 0001 | Rust kernel declarative surface |
| 0002 | Single-agent baseline before MAS |
| 0003 | WorkflowGraph as first-class entity |
| 0004 | Trace and eval as first-class |
| 0005 | Memory policy, not vector DB |
| 0006 | Secure protocol adapters |
| 0007 | Multi-principal governance |
| 0008 | Fuzz runs on Linux CI, not Windows local |
| 0009 | Opt-in no-sync WAL append |
| 0010 | Research source ledger separate from memory |
| 0011 | EventLog EventSourced trait |
| 0012 | Canonical diagnostic accumulator + const-table codes |
| 0013 | OperationGate trait + typestate context |
| 0014 | Rename runtime→kernel, engine→decisions |

## Numbering rules

- **0001–0014**: Registry B (core architecture). Do not reuse.
- **0015–0021**: reserved for planned ADRs tracked in
  `contracts/migration/markdown-debt-inventory.yaml` (agent-facing contracts,
  Rust-only core, CLI/JSON+MCP, funnel autonomy, guide orchestrator,
  no-generic-advance). Not yet written.
- **0022+**: Registry A (this directory). Next free number is **0025**.

## Citation convention

When an ADR cites another by number, the number alone is unambiguous because
the two registries occupy disjoint ranges. Example: "per ADR-0001" always
means the kernel (Registry B); "per ADR-0023" always means the memory trust
model (Registry A).
