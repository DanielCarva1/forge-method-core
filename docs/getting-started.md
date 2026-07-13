# Getting started

## What the human does

The human talks to a host agent. They do not need to select workflows, edit
Forge YAML, operate the sidecar, or run governance commands. A typical request
is:

> Start Forge for this project, tell me what you need from me, and keep using
> Forge as you work.

The host agent must have the `forge-core` executable and canonical
`start-forge` skill available. The skill lives at
`skill/start-forge/SKILL.md` in this repository.

## Install the executable

### Prebuilt release

At this documentation checkpoint, the latest tagged prebuilt is `v0.4.0` and
does not contain the source-only P5/P6 and early P7 `0.10.0` feature level. Download the archive for the host platform from GitHub Releases and place both
`forge-core` and its `forge` wrapper on `PATH`. Verify the checksum and Sigstore
bundle as described in the [root README](../README.md#install).

Published `v0.4.0` archives contain the executable and wrapper only. The
current source release design adds a checked manifest, canonical skill, and
selected adoption documentation to future archives. Inspect
`RELEASE-MANIFEST.json`; do not assume older assets contain the new payload.

### Source checkout

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
cargo install --path crates/forge-core-cli --force
forge-core --version
```

The workspace package version and latest published binary tag can differ. For
an exact checkpoint, use a named commit/tag and verify
`forge-core --version`.

## Install the host skill

Copy `skill/start-forge/SKILL.md` to a location recognized by the host agent.
For Codex-compatible runtimes this is commonly:

```text
~/.agents/skills/start-forge/SKILL.md
```

Other hosts use different skill/plugin locations. Forge does not silently
write there because that would cross a host-owned trust boundary. The host
agent should confirm both the skill path and binary version before claiming
readiness.

## Start or resume a project

The agent runs one idempotent bootstrap command per chat:

```bash
forge-core start --root <project> --json
```

It executes returned structured argv—not the display string—then follows:

```text
start
  -> workflow init (first time) or workflow resume
  -> workflow release-status
  -> exact returned upgrade_argv, if present
  -> workflow next
  -> perform/verify/record the governed action
  -> workflow next
```

The consumer repository receives only `.forge-method.yaml`. Runtime state is
kept in the sibling `forge-<project>/.forge-method/` sidecar. Never create a
consumer-local `.forge-method/` directory manually. Current-source `preflight
init` follows the Project Link and stores its profile in the sidecar; older
binaries must not be used to justify local consumer state.

## What healthy output means

Healthy bootstrap proves that Project Link and sidecar resolve. It does not
prove every product capability, domain method, credential, or piece of evidence
exists. `workflow next` may correctly return obligations, evidence/capability
gaps, a gated human decision, a Domain Pack requirement, or a rebase/upgrade
requirement. These are useful outcomes, not installation failures.

## Updating

Source installs:

```bash
git pull --ff-only
cargo install --path crates/forge-core-cli --force
forge-core --version
```

Prebuilt installs must replace binary and wrapper from the same release. A
newer binary never silently migrates project authority; the agent follows
`workflow release-status` and only executes its exact CAS-bound `upgrade_argv`.

## Recovery

- Re-run `start` in a new chat or after repairing the sidecar path.
- Use `workflow resume` after agent/process replacement.
- Use `domain-pack status` and `domain-pack recover` for lifecycle recovery.
- Do not delete the sidecar to fix an integrity error; preserve and inspect it.


For installation, state ownership, backup, and recovery details, see the
[Operator guide](operator-guide.md).
