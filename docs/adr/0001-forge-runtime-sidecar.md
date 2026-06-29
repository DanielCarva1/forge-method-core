# ADR 0001: Forge Runtime Sidecar for Consumer Projects

## Status

Accepted

## Context

Consumer projects need Forge Method state, but storing that state inside the Forge core repository or mixing it into the product repository confuses agents about authority, claims, evidence, and project boundaries. The `darkest-roguelite` nested repository exposed this failure mode: a consumer project and its Forge state appeared inside `<repo-root>`, making the core look responsible for another project's runtime state.

## Decision

A consumer project uses a sibling Forge Runtime Sidecar. The consumer repo contains only a small `.forge-method.yaml` Forge Project Link. The sidecar contains the real `.forge-method/` state tree.

Canonical layout:

```txt
<parent>/
  <project>/
    .forge-method.yaml
  forge-<project>/
    .forge-method/
```

`<repo-root>` is a Bootstrap Core Exception and may keep local `.forge-method/` state temporarily while the core develops itself.

## Consequences

- Agents in consumer repos can resolve the authoritative Forge state without guessing.
- Consumer source trees stay separate from Forge runtime artifacts and ledgers.
- The Forge core no longer accumulates unrelated project state.
- Bootstrap scripts must fail closed when a consumer repo lacks `.forge-method.yaml`; they must not silently create local runtime state.
