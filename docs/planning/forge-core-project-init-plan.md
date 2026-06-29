# Forge Core Project Init Plan

Status: docs/planning note for the project-init bootstrap slice. This does not claim Forge is fully product-ready.

## Bootstrap story

Consumer repositories should start Forge with:

```bash
forge-core project init --root <repo>
```

Expected layout after init:

```txt
<parent>/
  <project>/
    .forge-method.yaml
  forge-<project>/
    .forge-method/
```

The consumer repo keeps only the `.forge-method.yaml` pointer. The runtime state lives in sibling sidecar `../forge-<project>/.forge-method`; the consumer repo must not get a local `.forge-method/` directory.

Forge core itself remains the bootstrap exception: local state under `<repo-root>\.forge-method` is allowed only when commands explicitly opt in with `--allow-bootstrap-core`.

## Acceptance criteria

- `forge-core project init --root <repo>` creates `.forge-method.yaml` in the consumer repo and `../forge-<project>/.forge-method` as the state root.
- Re-running init for the same resolved link is idempotent and does not rewrite unrelated state.
- Init fails closed on an existing `.forge-method.yaml` that points to a different sidecar/state root.
- Init fails closed when `state_root` does not end in `.forge-method`, when it points outside `sidecar_root`, or when a consumer repo already has unsafe local runtime state at `<repo>/.forge-method`.
- Normal consumer commands resolve through the pointer and write claims, handoffs, WAL, metadata, artifacts, and evidence under the sidecar state root.
- `--allow-bootstrap-core` is documented as a Forge-core-only bootstrap exception, not a consumer-project flag.
- Docs avoid claiming Forge is fully done until clean install -> init -> resolve -> state-bearing command flow is verified.

## Verification checklist

This story needs both focused tests and a clean consumer-repo exercise:

- Install or locate `forge-core` on PATH from a clean environment.
- Create a temporary consumer repo with no Forge files.
- Run `forge-core project init --root <repo> --json`.
- Confirm `<repo>/.forge-method.yaml` exists and points to `../forge-<project>/.forge-method`.
- Confirm `<repo>/.forge-method/` does not exist.
- Confirm `../forge-<project>/.forge-method/` exists with the expected state directories.
- Re-run the same init command and confirm it is idempotent.
- Seed a conflicting `.forge-method.yaml` and confirm init rejects it without mutation.
- Seed unsafe consumer-local `.forge-method/` state and confirm init rejects it without adopting that state root.
- Run `forge-core project resolve --root <repo> --json` and confirm it resolves to the sidecar state root.
- Run one claim/write-check flow and one state-bearing operation/effect flow from the consumer repo, then confirm outputs land under the sidecar.
- Run the Forge core local-state path only with `--allow-bootstrap-core` and confirm ordinary consumer repos do not need or accept that exception.
- Verify the global Forge skill/start script auto-inits first-use repos and still fails closed with `-NoInit` or unsafe local state.
- For docs-only edits, run `git diff --check`.

## Remaining gaps

- Global skill/start script now calls `project init` for first-use repos unless `-NoInit` is passed.
- Product readiness still depends on keeping clean install/init/resolution and sidecar write flow evidence current as more commands are added.
