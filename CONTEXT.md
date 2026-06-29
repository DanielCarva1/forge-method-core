# Forge Method Context Glossary

## Consumer Project Repo

The application, game, library, or product repository being developed with Forge Method. It owns product source code and may carry a small Forge Project Link, but it does not own Forge runtime state. Consumer repos must not use a local `<consumer>/.forge-method` state root.

## Forge Runtime Sidecar

A sibling directory or repository that owns the Forge Method runtime state for one Consumer Project Repo. It contains the real `.forge-method/` tree, including state, artifacts, evidence, ledger, stories, and claims.

## Forge Project Link

The small `.forge-method.yaml` file stored at a Consumer Project Repo root. It points to the Forge Runtime Sidecar and its `.forge-method/` state root. Its `state_root` must resolve under `sidecar_root`, normally as `<sidecar_root>/.forge-method`.

## Bootstrap Core Exception

The temporary exception that allows `<repo-root>` to keep local `.forge-method/` state while the Forge core is still being developed by Forge itself. This exception is explicit and must not be copied to consumer projects.

## Project Link Hardening Rules

- Consumer `state_root` must be inside the configured `sidecar_root`.
- Consumer `state_root` must not be `<consumer>/.forge-method`; only the Forge core bootstrap exception may use local runtime state.
- Runtime and claim commands fail closed when the resolved `state_root` does not exist; they must not silently create consumer-local state.
- `--claims-dir` remains an explicit advanced override for tests, migrations, and emergency repair.
- The goal is isolation: projects, users, and agents must not contaminate each other's Forge data.
