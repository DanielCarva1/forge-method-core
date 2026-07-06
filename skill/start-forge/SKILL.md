---
name: start-forge
description: Start or resume Forge Method for the current project. Use when the user invokes /start-forge, asks to run Forge start, onboard a repo into Forge Method, initialize a Consumer Project Repo with a Forge Project Link, diagnose the next Forge bootstrap step, or resume work on a Forge project in a new chat/session.
---

# Start Forge

This is the single entry point for Forge Method in a project. Run it once per
chat/session — it inspects the real project state, performs the safe first
action, and orients the agent on what to do next.

## Core rules

- Use the `forge-core` binary. Never create `.forge-method/` manually inside a
  Consumer Project Repo. Consumer repos get only a `.forge-method.yaml` pointer;
  runtime state belongs in the sibling Forge Runtime Sidecar created by
  `forge-core project init`.
- Treat `data.next_step.argv` as the execution contract (a pre-tokenized argv
  you can run directly). Treat `data.next_step.command` as display-only text for
  humans — do not shell-split it, paths may contain spaces.

## Workflow

1. **Resolve the project root.** Default to the current working directory. If
   the user names a path, use it as `--root`.

2. **Locate `forge-core`** from PATH or Cargo bin. It may be `forge-core` or
   `forge-core.exe` depending on host/toolchain.

   ```bash
   forge-core --version 2>/dev/null \
     || forge-core.exe --version 2>/dev/null \
     || ~/.cargo/bin/forge-core --version 2>/dev/null \
     || ~/.cargo/bin/forge-core.exe --version 2>/dev/null \
     || echo "NOT_FOUND"
   ```

   If missing, report that Forge is not installed and do not invent a fallback.

3. **Run the read-only bootstrap diagnostic.** This never writes anything and is
   safe on dirty repos:

   ```bash
   forge-core start --root "<project-root>" --json
   ```

4. **Branch on `data.state`.**

   - `no_link` — no Forge Project Link yet. If the user asked to start/onboard:
     run `forge-core project init --root "<project-root>" --json`, then re-run
     `forge-core start --root "<project-root>" --json` to see the advanced state.
     When `data.next_step.argv` is present, execute that argv vector directly
     (append `--json` only if the vector lacks an output mode). Summarize the
     created `.forge-method.yaml`, sidecar path, and next step.
   - `link_present_no_sidecar` — the link parses but the sidecar/state root is
     missing. Do not guess. Run or suggest
     `forge-core project resolve --root "<project-root>" --json` and report the
     broken sidecar plus the repair command.
   - `sidecar_ready_no_contract` — Forge is initialized; the next step is
     authoring the first operation contract. Do not invent product goals. Ask the
     user for the intended first operation if one is not obvious from context.
   - `contract_present` — hand off to `forge-core guide describe` (or the argv in
     `data.next_step.argv`). Show `data.next_step.command` only as display text.
   - `preview_run` — bootstrap is complete. Use `forge-core guide` for ongoing
     phase/workflow routing.

5. **Keep output practical.** Show the exact commands run, the final `state`,
   `next_step.argv` when present, and `next_step.command` for display. Mention
   any files or sidecar directories created. If a command fails, preserve the
   Forge error and usage text verbatim.

## Safety checks

- Do not run broad cleanup or delete any existing Forge state unless the user
  explicitly asks for cleanup.
- Do not pass `--allow-bootstrap-core` for ordinary consumer projects; it is
  reserved for the Forge core repo itself.
- Do not initialize inside system folders, package caches, or temporary folders
  unless the user explicitly selected that root.
- `forge-core start` is read-only and safe on dirty repos. `forge-core project
  init` writes — be more cautious before running it and mention the write.

## Installing this skill

This file is the canonical source. Save it wherever your host agent reads skills
from. Forge does not assume a directory: common conventions include
`~/.agents/skills/` (Codex, Zed), an MCP tool, or a project-local `.skills/`.
Pick the location your agent runtime expects.
