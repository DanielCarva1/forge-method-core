# Architecture

## System boundary

Forge separates consumer content, runtime state, cooperative operator material,
and external trust authority:

```text
consumer project/              product source + .forge-method.yaml pointer
forge-<project>/.forge-method/ runtime ledger, receipts, evidence, generations
forge-<project>/operator/      cooperative registries and local credential secrets
external operator/host roots   replay/trust anchors and external broker private keys
```

The project is not the authority store. The sidecar contains runtime state and
cooperative same-principal operator material, so it must be protected and
backed up as sensitive data. External broker private keys, Domain Pack trust
roots, and replay anchors remain outside project and state roots. A local
credential secret in the sidecar is only a cooperative operator proxy; its
location cannot prove human presence or independent review.

## Layering

| Layer | Responsibility | Must not do |
|---|---|---|
| Contracts | Closed typed wire vocabulary | Grant runtime authority |
| Decisions | Pure deterministic evaluation/projection | Perform mutation |
| Authority/TCB crates | Verify signatures, anchors, typestate and transitions | Trust caller-shaped audit output |
| Kernel | Join admitted authority, derive gates and persist state | Accept caller-selected workflow/phase |
| CLI/MCP adapters | Parse host input and expose envelopes | Reimplement kernel policy |
| Skills/host integrations | Drive the loop and translate chat | Forge receipts or bypass gaps |

The exact crate dependency map is generated at
[`docs/generated/workspace-layout.md`](generated/workspace-layout.md).

## Authority flow

```text
typed candidate
  -> pure validation/evaluation
  -> verified external/operator authority
  -> opaque admitted capability
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
and the representative-slice projection are derived from that same workflow
ledger. A slice definition is ordinary content-addressed evidence until a
Reviewer-origin evaluator observation admits its exact bytes. Representative
execution is another existing evidence observation, from a Runtime origin in a
different configured separation domain, and must bind the latest definition
and every scenario. There is deliberately no second evidence store or special
representative-slice mutation log.

## Persistence

Critical state uses append-only hash-linked logs, content-addressed immutable
objects, CAS-bound heads, retained OS locks, and recovery protocols. A crash
may leave recoverable residue but must not create an ambiguous success.

## Extensibility

Domain behavior belongs in Domain Pack data. Game, legal, medical, or other
domain-specific Rust branches in universal core violate the architecture. New
executable authority belongs behind a generic typed boundary and explicit
review/admission path.
