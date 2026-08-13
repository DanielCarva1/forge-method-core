# Domain docs

This repository uses a single domain context.

## Read before planning or editing

- Read `CONTEXT.md` at the repository root.
- Read relevant ADRs under `docs/adr/` when that directory exists.
- If a future `CONTEXT-MAP.md` is introduced, follow it instead of assuming the
  repository still has one context.

Missing ADR files are not an error. Create an ADR only for a decision that is
hard to reverse, surprising without its history, and based on a real tradeoff.

## Use the glossary vocabulary

Use terms as defined in `CONTEXT.md` in specifications, issue titles, tests, and
implementation. If a proposed term conflicts with the glossary, resolve the
meaning with the user before changing code or publishing tickets.

`CONTEXT.md` is a glossary, not a specification, backlog, implementation guide,
or chat transcript. Record implementation decisions in the appropriate spec or
ADR instead.

## Conflict handling

If proposed work conflicts with an existing ADR or accepted contract, surface
the conflict explicitly. Do not silently overwrite or reinterpret the earlier
decision.
