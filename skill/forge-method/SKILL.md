---
name: forge-method
description: Use when the user wants to use Forge Method, start or resume a method project, build something with the protocol, or asks to use Forge/the method. Activates the forge-core runtime and defers to the binary as authority.
---

# Forge Method

Forge Method is a typed-contract protocol runtime shipped as `forge-core`. The
binary is the authority for workflow catalog, phases, gates, and governance. The
skill bootstraps the runtime and then defers to the binary; do not re-document
protocol rules here.

## Step 0 - Resolve the workspace first

When Forge is invoked, inspect the folder once before acting:

1. Consumer project repo: `.forge-method.yaml` exists. Run
   `forge-core project resolve --root . --json` and use its `state_root` for
   claims, state, artifacts, evidence, and ledger. Do not create local runtime
   state in the consumer repo.
2. Forge Runtime Sidecar: `.forge-method/` exists and the directory is named
   like `forge-<project>`. This folder owns runtime state for its sibling
   consumer repo. Inspect `.forge-method/state.yaml` and related files there.
3. Bootstrap Core Exception: this is `<repo-root>` with the Rust
   workspace and local `.forge-method/`. Run
   `forge-core project resolve --root . --allow-bootstrap-core --json`.
4. Brownfield/greenfield without link: do not silently create `.forge-method/`.
   Create a Forge Project Link plus sibling sidecar first, or ask for the
   intended project id/root if it cannot be inferred.

One question max. Do not explain phases unprompted.

## Step 1 - Locate the binary

Resolve `forge-core` from PATH or Cargo bin. It may be `forge-core` or
`forge-core.exe` depending on host/toolchain.

```bash
forge-core --version 2>/dev/null \
  || forge-core.exe --version 2>/dev/null \
  || echo "NOT_FOUND"
```

If the binary is installed but not on PATH, check the Cargo bin dir before
giving up:

```bash
~/.cargo/bin/forge-core --version 2>/dev/null \
  || ~/.cargo/bin/forge-core.exe --version 2>/dev/null \
  || echo "NOT_FOUND"
```

If missing, install from the core repo:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
cargo install --path crates/forge-core-cli
```

## Step 2 - Resolve state, then govern writes

Before edits:

1. Resolve the state root:
   - consumer repo: `forge-core project resolve --root . --json`
   - core repo only: `forge-core project resolve --root . --allow-bootstrap-core --json`
2. Use `<state_root>/claims-active` as the claims directory.
3. Acquire a claim covering every planned target path.
4. Run `claim check-write` for every target before editing.
5. If check-write fails, do not edit. Acquire a covering claim, narrow scope, or
   report the blocker.
6. Heartbeat during long work and release the claim when done.

Current strict policy: unclaimed writes are rejected; overlapping live path
claims are rejected.

## Defer to forge-core

Ask the binary instead of inventing state from memory:

- `forge-core guide describe`
- `forge-core guide status --phase <phase>`
- `forge-core guide decide --decision-file <repo-relative.yaml>`
- `forge-core validate --root . --json`

## Path caveat

A Windows `.exe` cannot reliably read POSIX-only paths such as `/tmp/...`; prefer
repo-relative paths or Windows absolute paths. For claims, use the resolved
`state_root` and its `claims-active` directory.

## Done means

- Typed outputs exist in the expected contract/runtime state location.
- Validation passes with zero diagnostics where applicable.
- Held claims are released.
