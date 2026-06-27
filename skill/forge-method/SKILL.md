---
name: forge-method
description: Use when the user wants to use Forge Method, start or resume a method project, build something with the protocol, or asks "use the forge / use the method". Activates the forge-core runtime — a typed-contract protocol that governs multi-agent product/software/creative work. The runtime is self-describing, so this skill only bootstraps it and then defers; it does not re-document the protocol.
---

# Forge Method

Forge Method is a **typed-contract protocol runtime**, shipped as a single
binary `forge-core`. The binary is the authority: it knows its workflow catalog,
its 7 phases, and its governance rules, and it will tell you any of them on
request. **This skill's only job is to locate the binary and then defer to it.**

Do not re-document the protocol here. Re-documenting it would create a second
source of truth that drifts from the binary — exactly what this protocol rejects.

## Step 0 — Route the workspace first

When `/forge method` is invoked, do **one** glance at the folder before anything
else. The goal: tell greenfield, brownfield, and already-forge apart.

1. **Already forge?** A `.forge-method/` directory or a `contracts/workflows/`
   catalog is present → this workspace already uses Forge. **Skip the rest of
   this step** — run `guide status --phase 0-route` and act on what it returns.
2. **Brownfield (not Forge)?** The folder has existing work (code, docs, a repo)
   but no Forge artifacts → ask one thing: *what do you want to continue doing
   here?* Forge sets up `.forge-method/` automatically when work starts.
3. **Greenfield (empty)?** The folder is essentially empty → ask one thing:
   *what do you intend to build?* Then start at `0-route` and let the guide
   drive.

One question max. Do not narrate the method, do not explain phases unprompted.

## Step 1 — Locate the binary (once per machine)

The binary may be named **`forge-core`** (Linux/macOS, pure-WSL installs) **or**
**`forge-core.exe`** (Windows, or WSL using the Windows toolchain via interop).
**Do not assume one name.** Resolve the binary with this exact fallback chain,
in order — the first hit wins:

```bash
# 1. Anything already on PATH (works on every platform):
forge-core --version 2>/dev/null \
  || forge-core.exe --version 2>/dev/null \
  || ~/.cargo/bin/forge-core --version 2>/dev/null \
  || ~/.cargo/bin/forge-core.exe --version 2>/dev/null \
  || echo "NOT_FOUND"
```

If that prints `NOT_FOUND`, install it (pick the line that matches the host —
`cargo install` builds a **native** binary for the OS it runs on, so under WSL
it produces a Linux `forge-core`, under Windows it produces `forge-core.exe`):

```bash
git clone https://github.com/DanielCarva1/Forge-method-core.git
cd Forge-method-core
cargo install --path crates/forge-core-cli   # → ~/.cargo/bin/forge-core (POSIX) | forge-core.exe (Windows)
```

After installing, **re-run the Step 1 probe** to capture the resolved binary
path/name into a shell variable and reuse it for the rest of the session:

```bash
FORGE="$(command -v forge-core || command -v forge-core.exe)"
[ -z "$FORGE" ] && FORGE="$HOME/.cargo/bin/forge-core"
[ ! -x "$FORGE" ] && FORGE="$HOME/.cargo/bin/forge-core.exe"
# sanity:
"$FORGE" --version
```

From here on, every command below uses **`"$FORGE"`** — never hardcode the
`.exe`. Verify with: `"$FORGE" validate` prints `forge_core_validation_passed`.

## Step 2 — Enter the protocol, then DEFER

Run the entry point and **follow what forge-core returns**:

```bash
"$FORGE" guide status --phase 0-route
```

It returns `eligible_workflows`, `pending_gates`, and `next_phases`. Act on
those. From here on, **forge-core is the guide, not this skill.** Ask it:

- `"$FORGE" guide describe` — see the full workflow catalog
- `"$FORGE" guide status --phase <phase>` — what the current phase needs
- `"$FORGE" guide decide --decision-file <repo-relative.yaml>` — validate a routing decision

The workflow catalog (the 110 workflow documents) is **embedded in the binary**,
so `guide describe` / `status` / `decide` work with **no `--catalog-dir` flag**
on any machine — including a brand-new greenfield folder. Only pass
`--catalog-dir <path>` if a project ships its own custom workflows and you want
to override the embedded set.

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
   `"$FORGE" validate`. Fail-closed means fix it, not paper over it.

## Path caveat (multi-platform)

The resolved binary may be a Windows `.exe` (callable from WSL via interop) or a
native POSIX binary. A Windows `.exe` **cannot read `/tmp/...` or `~`**; use
**repo-relative paths** (run commands from the repo root) or absolute **Windows
paths** (`C:\...`). A native POSIX binary has no such limit. Since you do not
know which you got until Step 1 resolved it, **prefer repo-relative paths and
run from the repo root** — that works for both. A `--claims-dir /tmp/x` will
silently fail under a Windows `.exe`; use `.forge-method/claims-active` instead.

## When a workflow is done

Not when the agent "feels" done. When:

- its typed outputs exist under `contracts/` or `.forge-method/artifacts/`,
- `"$FORGE" validate` passes with zero diagnostics, and
- any claim held is released.
