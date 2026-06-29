# Forge sidecar-safe state-bearing commands

Branch: `codex/forge-sidecar-state-commands`

## Problem

After ProjectLink/sidecar hardening, claim, trace, graph, and eval paths resolved
consumer project state correctly. The older operation/effect commands still used
`--root` directly as the effect store root:

- `execute-operation`
- `rebuild-effect-index`
- `query-effect-index`

That meant a linked consumer repo could still receive a local
`<consumer>/.forge-method` tree through WAL, evidence, effect metadata index, or
artifact/evidence effect writes.

## Decision

State-bearing operation/effect commands must resolve the Forge Project Link
before touching Forge state:

- product contracts and payload files stay relative to the Consumer Project Repo;
- Forge WAL, locks, metadata index, command evidence, and
  `.forge-method/artifacts/*` / `.forge-method/evidence/*` effect writes use the
  resolved Forge Runtime Sidecar state;
- missing `.forge-method.yaml` or missing sidecar `.forge-method` fails closed;
- the Forge core local `.forge-method` remains available only through explicit
  `--allow-bootstrap-core`.

## Implementation notes

- `RuntimeOperationExecutionContext` now separates command execution root from
  `effect_store_root`.
- CLI `execute-operation` passes the consumer root to contract/payload loading
  and the sidecar root to the runtime effect store.
- CLI `rebuild-effect-index` and `query-effect-index` default to the same
  resolved sidecar-backed store root.
- Existing library helpers keep their explicit root behavior for direct unit
  tests and low-level callers.

## Acceptance evidence

Targeted tests added or updated:

```bash
cargo test -p forge-core-cli --test operation_sidecar_e2e
cargo test -p forge-core-cli --test validate
cargo test -p forge-core-runtime
cargo test -p forge-core-cli --test claim_cli_sidecar_e2e --test project_link_hardening_e2e --test project_resolve_e2e
```

The new E2E asserts:

- `execute-operation --root <consumer>` writes artifacts/evidence/WAL/index under
  `<sidecar>/.forge-method`.
- `rebuild-effect-index --root <consumer>` rebuilds from sidecar WAL into
  sidecar index.
- `query-effect-index --root <consumer>` reads sidecar index.
- missing Project Link fails closed and does not create consumer-local
  `.forge-method`.

## Remaining adjacent gaps

This slice does not implement first-run project bootstrap. The next product
readiness slice should add `forge-core project init` so a fresh/brownfield repo
can create `.forge-method.yaml` and the sibling sidecar from the CLI.
