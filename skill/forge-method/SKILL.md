---
name: forge-method
description: Use when the user wants to use Forge Method, start or resume a method project, build something with the protocol, or asks "use the forge / use the method". Activates the forge-core runtime — a typed-contract protocol that governs multi-agent product/software/creative work. The runtime is self-describing, so this skill only bootstraps it and then defers; it does not re-document the protocol.
---

# Forge Method

Forge Method is a **typed-contract protocol runtime**, shipped as a single
binary `forge-core`. The binary is the authority: it knows its workflow catalog,
its 7 phases, and its governance rules, and it will tell you any of them on
request. **This skill's only job is to install the binary and then defer to it.**

Do not re-document the protocol here. Re-documenting it would create a second
source of truth that drifts from the binary — exactly what this protocol rejects.

## Step 0 — Route the workspace first

When `/forge method` is invoked, do **one** glance at the folder before anything
else. The goal: tell greenfield, brownfield, and already-forge apart.

1. **Already forge?** A `.forge-method/` directory or a `contracts/workflows/`
   catalog is present → this workspace already uses Forge. **Skip the rest of
   this step** — run `forge-core.exe guide status --phase 0-route` and act on
   what it returns. You already know the state; no questions needed.
2. **Brownfield (not Forge)?** The folder has existing work (code, docs, a
   repo) but no Forge artifacts → ask one thing: *what do you want to continue
   doing here?* Forge sets up `.forge-method/` automatically when work starts
   (the first `claim acquire` creates the coordination bus).
3. **Greenfield (empty)?** The folder is essentially empty → ask one thing:
   *what do you intend to build?* Then start at `0-route` and let the guide
   drive.

One question max. Do not narrate the method, do not explain phases unprompted.

## Step 1 — Bootstrap (once per machine)

Check whether the binary exists, and install it if not:

```bash
forge-core.exe --version          # WSL/Windows  (forge-core on POSIX)
```

If missing, install from the forge-method-rust repo:

```bash
git clone https://github.com/DanielCarva1/forge-method-rust.git
cd forge-method-rust
cargo install --path crates/forge-core-cli     # → ~/.cargo/bin/forge-core
```

Verify: `forge-core.exe validate` prints `forge_core_validation_passed`.

## Step 2 — Enter the protocol, then DEFER

Run the entry point and **follow what forge-core returns**:

```bash
forge-core.exe guide status --phase 0-route
```

It returns `eligible_workflows`, `pending_gates`, and `next_phases`. Act on
those. From here on, **forge-core is the guide, not this skill.** Ask it:

- `forge-core.exe guide describe` — see the full workflow catalog
- `forge-core.exe guide status --phase <phase>` — what the current phase needs
- `forge-core.exe guide decide --decision-file <repo-relative.yaml>` — validate a routing decision

Never invent a workflow, phase, or gate from memory. If forge-core can answer
it, ask it.

## Relationship rules (host behavior only)

The rules below are about how **you**, the host agent, relate to forge-core.
The protocol's own rules live in the binary — do not duplicate them.

1. **Defer.** If `guide describe` / `status` / `decide` can answer a question,
   ask the binary. Don't re-derive phases, workflows, or gates from memory.
2. **Disk over chat.** The `contracts/` tree and the claims bus are truth. Do
   not infer phase, story, or readiness from conversation history.
3. **Govern before write.** Before editing any file, `claim acquire` then
   `check-write`. If the claim is refused, do not overwrite.
4. **Validate before trust.** After creating or changing any contract, run
   `forge-core.exe validate`. Fail-closed means fix it, not paper over it.

## Path caveat (WSL/Windows interop)

`forge-core.exe` is a Windows binary. It cannot read `/tmp/...` or `~`. Use
**repo-relative paths** (run commands from the repo root) or absolute
**Windows paths** (`C:\...`). A `--claims-dir /tmp/x` will silently fail; use
`.forge-method/claims-active` instead.

## When a workflow is done

Not when the agent "feels" done. When:

- its typed outputs exist under `contracts/` or `.forge-method/artifacts/`,
- `forge-core.exe validate` passes with zero diagnostics, and
- any claim held is released.
