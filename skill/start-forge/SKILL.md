---
name: start-forge
description: Start or resume Forge Method for the current project. Use when the user invokes /start-forge, asks to run Forge start, onboard a repo into Forge Method, initialize a Consumer Project Repo with a Forge Project Link, diagnose the next Forge bootstrap step, or resume work on a Forge project in a new chat/session.
---

# Start Forge

This is the single entry point for Forge Method in a project. Run it once per
chat/session — it bootstraps a fresh repo, repairs a broken one, or routes an
in-progress project to the next step.

## Core rules

- Use the `forge-core` binary. Never create `.forge-method/` manually inside a
  Consumer Project Repo. Consumer repos get only a `.forge-method.yaml` pointer;
  runtime state belongs in the sibling Forge Runtime Sidecar created by
  `forge-core start`.
- The runtime owns the current phase (`state.yaml`). Do not pass `--phase`
  unless you intentionally override the authoritative record.

## Workflow

1. **Resolve the project root.** Default to the current working directory. If
   the user names a path, use it as `--root`.

2. **Locate `forge-core`** from PATH or Cargo bin.

   ```bash
   forge-core --version 2>/dev/null \
     || forge-core.exe --version 2>/dev/null \
     || ~/.cargo/bin/forge-core --version 2>/dev/null \
     || echo "NOT_FOUND"
   ```

   If missing, report that Forge is not installed and do not invent a fallback.

3. **Run `forge-core start`.** This is the bootstrap entry point. On a fresh
   repo it creates the Project Link + sidecar; on a broken repo it repairs; on
   a healthy repo it reports the current state and next step.

   ```bash
   forge-core start --root "<project-root>" --json
   ```

   Read `data.state` and `data.actions_performed` from the response. When the
   state is `sidecar_ready_no_contract` or later, the project is ready for the
   orchestrator loop.

4. **Run the orchestrator loop** (when the project is bootstrapped):

   - **`forge-core guide decide --root "<project-root>" --decision-file <intent.yaml> --json`**
     — returns the recommended workflow plus a binding `enforcement_policy`
     (`claim_required`, `lane`, `automatic_gates`). If `claim_required` is true,
     acquire a claim covering the operation's write targets before executing.
   - **`forge-core execute-operation --root "<project-root>" --operation <op.yaml> --json`**
     — the kernel runs `ClaimCoverageGate` + `PhaseGate` before any write. If
     the operation is rejected, read the `gate` code (`claim_coverage_missing`,
     `claim_collision`, `phase_blocks_mutation`) and comply: acquire/release a
     claim, advance the phase, or pick a lower-autonomy operation.

5. **Keep output practical.** Show the exact commands run, the final `state`,
   the `enforcement_policy` from `guide decide`, and any gate rejections.
   Mention files or sidecar directories created.

## Safety checks

- Do not run broad cleanup or delete any existing Forge state unless the user
  explicitly asks for cleanup.
- Do not pass `--allow-bootstrap-core` for ordinary consumer projects; it is
  reserved for the Forge core repo itself.
- Do not initialize inside system folders, package caches, or temporary folders
  unless the user explicitly selected that root.

## Installing this skill

This file is the canonical source. Save it wherever your host agent reads skills
from. Forge does not assume a directory: common conventions include
`~/.agents/skills/` (Codex, Zed), an MCP tool, or a project-local `.skills/`.
Pick the location your agent runtime expects.
