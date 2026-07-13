---
name: start-forge
description: Start or resume Forge Method for the current project. Use when the user invokes /start-forge, asks to run Forge start, onboard a repo into Forge Method, initialize a Consumer Project Repo with a Forge Project Link, diagnose the next Forge bootstrap step, or resume work on a Forge project in a new chat/session.
---

# Start Forge

This is the single entry point for Forge Method in a project. Run it once per
chat/session — it bootstraps a fresh repo, repairs a broken one, or routes an
in-progress project into agent-native workflow governance.

## Core rules

- Use the `forge-core` binary. Never create `.forge-method/` manually inside a
  Consumer Project Repo. Consumer repos get only a `.forge-method.yaml` pointer;
  runtime state belongs in the sibling Forge Runtime Sidecar created by
  `forge-core start`.
- The P5 workflow ledger owns governed phase and progression. `state.yaml` and
  `start.data.next_step` remain bootstrap/compatibility projections; never use
  them to select workflow, phase, policy bundle, readiness target, completion,
  or evidence authority.
- Execute structured command arrays as argv. `data.next_step.command` is a
  human-readable display string, not a shell-safe command source. Never split,
  concatenate, or shell-evaluate it.

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

3. **Run `forge-core start`.** This is the zero-config bootstrap entry point.
   On a fresh repo it creates the Project Link + sidecar; on a broken repo it
   repairs; on a healthy repo it reports the current bootstrap state.

   ```bash
   forge-core start --root "<project-root>" --json
   ```

   Read `data.state`, `data.actions_performed`, `data.project`, and
   `data.next_step` from the response. Use `data.next_step.command` only when
   explaining the action to a human; agents execute the structured argv.

4. **Enter agent-native workflow governance** when the Project Link and sidecar
   are healthy.

   First verify that the installed binary exposes `forge-core workflow --help`.
   For a current binary, every healthy `start` response supplies an exact argv
   shaped as `forge-core workflow init --root <project-root>`. Confirm its root
   matches `data.project.project_root` and execute that returned argv directly.
   `workflow init` is idempotent: a fresh ledger returns initialized and an
   existing ledger returns already initialized without discarding continuity.

   If command discovery exposes `workflow release-status`, run
   **`forge-core workflow release-status --root "<project-root>" --json`** next.
   The ledger-derived release is authoritative; the newest installed binary is
   not. When `data.upgrade_argv` is present, verify it is an argv array shaped
   as `forge-core workflow release-upgrade --root <same-project-root> ...`, has
   no registry/manifest/batch/bundle/release path flags, and execute that exact
   array directly. Do not rebuild it from display text or ask the human to
   select a release. Repeat `release-status` and require the returned active
   release to match the upgrade target. If no successor is available, continue
   without mutation. An older P5c binary that genuinely lacks this subcommand
   may continue on its existing implicit release; ordinary status/upgrade
   errors still fail closed rather than triggering fallback.

   Then run **`forge-core workflow next --root "<project-root>" --json`**.

   Any unexpected healthy-state argv, integrity, binding, snapshot, ledger, or
   environment error fails closed. Report it; do not erase state, reinitialize
   over an error, reconstruct argv from the display command, or fall back to
   caller-selected routing.

   Read obligations, evidence/capability gaps, Decision Requests, ranked next
   actions, and `data.authorization` from the workflow response. Its action
   packets are read-only, current-state authority offers; they are not
   permission to act. `forge-core workflow action-packets --root
   "<project-root>" --json` exposes the packet set and registry status as an
   optional standalone diagnostic. Select only the packet that matches the
   governed next action, satisfy its evidence work, and provide only the
   packet's closed semantic input. Never supply policy, phase, evaluator,
   target, registry, request, attestation, or digest fields yourself. The host
   agent performs the action and asks `workflow next` again. The human stays in
   chat and never operates Forge commands or edits Forge artifacts.

   A packet whose approval boundary is exactly `operator_credential_broker`
   may use the cooperative local one-call lane. The host materializes the
   packet's closed input without asking the human to edit JSON, then runs:

   ```bash
   forge-core workflow action authorize --root "<project-root>" \
     --packet-digest "<packet-sha256>" --input-file "<closed-input.json>" \
     --credential-id "<operator-credential-id>" --json
   ```

   Never use that lane for a human, independent-reviewer, or trusted-runtime
   packet; Forge rejects those boundaries before local signing.

   For a human Decision Request, ask the irreducible question in chat after its
   prerequisites are verified. For human/reviewer/runtime authority, the host
   must authenticate the inbound subject outside the governed agent process,
   sign an origin envelope for that exact packet, and invoke:

   ```bash
   forge-core workflow action apply --root "<project-root>" \
     --origin-envelope-file "<host-created-signed-event.json>" --json
   ```

   The host, not the model, creates that signed event. Never self-assert a
   human/reviewer/runtime identity, obtain the broker private key, or silently
   fall back to a cooperative local credential. A missing or revoked broker is
   a typed setup gap: report its registry setup status and stop. Do not run
   `workflow broker trust|rotate|revoke` unless the operator explicitly asks
   and performs the out-of-agent trust decision.

   After authorize/apply, run `workflow next` again. An exact broker-event retry
   can recover a reserved or already-recorded event, but any stale packet or
   changed state requires fresh guidance; do not edit and resubmit an old
   envelope.

5. **Fallback for an older binary without the `workflow` command.** Use this
   only when command discovery proves the installed binary does not implement
   `workflow`; do not treat an ordinary workflow error as version fallback.

   - Report that executable P5 workflow governance is unavailable and recommend
     upgrading Forge Core.
   - `guide describe`, `guide status`, `guide decide`, and a compatible
     `start.data.next_step.argv` may be used only for read-only legacy
     orientation or diagnostics.
   - Label their output `legacy_compatibility_only`. It cannot authorize a P5
     workflow, phase transition, completion, readiness claim, or mutation.
   - Do not invent an authoritative workflow choice from legacy output. Stop
     before authority-bearing work and tell the user what capability is absent.

6. **Keep output practical.** Show the exact executable plus argv used, the
   bootstrap `state`, workflow initialization status, active release/upgrade
   result when supported, and the governed status/next action or blocking gap.
   Mention Project Link or sidecar paths created. Do not expose private
   attestation material, present a legacy recommendation as authority, or ask
   the human to select a workflow or release.

## Safety checks

- Do not run broad cleanup or delete any existing Forge state unless the user
  explicitly asks for cleanup.
- Never execute `data.next_step.command` through a shell. Prefer the response's
  argv array and preserve every argument boundary, especially roots containing
  whitespace or shell metacharacters.
- Do not pass `--allow-bootstrap-core` for ordinary consumer projects; it is
  reserved for the Forge core repo itself.
- Do not pass workflow, phase, readiness target, registry, manifest, batch,
  bundle, or release-path selectors to agent-native workflow commands. A
  release id is permitted only inside the exact CAS-bound `upgrade_argv`
  returned by `release-status`.
- Treat broker enrollment as operator-owned trust configuration. Do not create
  enrollment records, public-key files, or origin envelopes by hand; do not
  read, request, copy, or store an external broker private key.
- Do not initialize inside system folders, package caches, or temporary folders
  unless the user explicitly selected that root.

## Installing this skill

This file is the canonical source. Save it wherever your host agent reads skills
from. Forge does not assume a directory: common conventions include
`~/.agents/skills/` (Codex, Zed), an MCP tool, or a project-local `.skills/`.
Pick the location your agent runtime expects.
