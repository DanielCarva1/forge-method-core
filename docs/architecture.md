# Architecture

## System boundary

Forge's current Solo Cooperative boundary separates consumer content from
durable runtime state:

```text
consumer project/              product source + .forge-method.yaml pointer
forge-<project>/.forge-method/ runtime ledger, receipts, evidence, generations
```

The project is not the authority store. The sidecar contains sensitive runtime
continuity and must be protected and backed up. Current solo operation uses
same-owner chat provenance and does not require an external origin broker, FIDO
device, or independently administered identity. Those additional trust domains
belong to a later enterprise architecture profile.

## Layering

| Layer | Responsibility | Must not do |
|---|---|---|
| Contracts | Closed typed wire vocabulary | Grant runtime authority |
| Decisions | Pure deterministic evaluation/projection | Perform mutation |
| Authority/TCB crates | Verify bindings, typestate and transitions required by the active profile | Trust caller-shaped audit output |
| Kernel | Join admitted authority, derive gates and persist state | Accept caller-selected workflow/phase |
| CLI/MCP adapters | Parse host input and expose envelopes | Reimplement kernel policy |
| Skills/host integrations | Drive the loop and translate chat | Forge receipts or bypass gaps |

The exact crate dependency map is generated at
[`docs/generated/workspace-layout.md`](generated/workspace-layout.md).

## Authority flow

```text
typed candidate
  -> pure validation/evaluation
  -> profile-appropriate admitted evidence or authority
  -> kernel-derived transition
  -> append-only receipt/ledger
  -> new guidance projection
```

Audit JSON/YAML is intentionally not reusable as authority. Process-owned
opaque capability prevents a caller from editing a successful report and
replaying it as permission.

## Workflow governance

The universal core is an append-only reviewed release chain. Its P7b successor
contains 43 policies, including the generic universal-assurance policy. Projects pin a
release in their ledger. P6 Domain Packs contribute namespaced data and produce
a separate effective epoch. Core and effective identities remain distinct; a
core upgrade with an active pack requires coordinated rebase rather than an
unsafe cross-store partial transaction.

Human intent, its monotonic assurance epoch, all eight universal-lens states,
and evidence projections are derived from that same workflow ledger. Solo
Cooperative admits bounded same-owner evidence only for claims whose policy
allows it and never relabels that evidence as independent review, verified human
presence, or enterprise compliance. Stronger profiles may later add separated
actors without changing the solo core loop.

## Persistence

Critical state uses append-only hash-linked logs, content-addressed immutable
objects, CAS-bound heads, retained OS locks, and recovery protocols. A crash
may leave recoverable residue but must not create an ambiguous success.

## Extensibility

Domain behavior belongs in Domain Pack data. Game, legal, medical, or other
domain-specific Rust branches in universal core violate the architecture. New
executable authority belongs behind a generic typed boundary and explicit
review/admission path.
