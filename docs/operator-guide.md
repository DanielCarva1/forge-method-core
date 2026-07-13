# Operator guide

This guide is for the person or automation responsible for installing Forge,
connecting it to a host agent, preserving runtime state, and handling recovery.
The normal project user stays in chat and does not operate these commands.

## Choose an installation channel

Forge currently has two different distribution facts:

- the source workspace declares package version `0.10.0` and contains the
  completed P5/P6 implementation;
- the latest published prebuilt GitHub Release may lag the source checkpoint.
  At the time this guide was written, the latest prebuilt tag was `v0.4.0`.

Always check the selected tag/commit and run `forge-core --version`. Do not
assume that the newest source features exist in an older release archive. See
[Product status](product-status.md) for the maintained distinction.

### Install the current source checkpoint

Prerequisites: Git and Rust 1.85 or newer.

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
git pull --ff-only
cargo install --path crates/forge-core-cli --force
forge-core --version
```

This installs the real `forge-core` executable. Source checkout also provides
the canonical skill, documentation, fixtures, and contracts. The binary embeds
shared catalog/contract material needed by ordinary consumer projects.

### Install a tagged prebuilt archive

Download only assets belonging to one selected release. Published `v0.4.0`
archives contain only the platform executable and thin `forge`/`forge.cmd`
wrapper. The current source release design adds a checked
`RELEASE-MANIFEST.json`, canonical skill, and selected adoption/fork/security
guides to future archives. Inspect that manifest instead of assuming an older
archive contains the new payload. Full source contracts and fixture corpora are
not release payload.

POSIX:

```bash
tar xzf forge-core-<arch>-<os>.tar.gz
install -m 0755 forge forge-core ~/.local/bin/
forge-core --version
```

Windows PowerShell:

```powershell
$destination = Join-Path $env:LOCALAPPDATA 'Programs\forge-core'
Expand-Archive .\forge-core-x86_64-windows.zip -DestinationPath $destination -Force
# Add $destination to the user PATH through Windows Settings or your managed setup.
& "$destination\forge-core.exe" --version
```

Each binary archive is accompanied by its own `.sha256` and `.sigstore` files.
Older releases may omit an SBOM. The hardened source release workflow requires
one validated release-level CycloneDX SBOM before publication; it is not a
separate sibling generated per platform archive.

## Install the host skill

The canonical procedure is [`skill/start-forge/SKILL.md`](../skill/start-forge/SKILL.md).
Copy the complete `skill/start-forge/` directory to a directory your host
actually loads. A common Codex-compatible location is:

```text
~/.agents/skills/start-forge/SKILL.md
```

Host conventions differ. Confirm skill discovery in the host instead of
claiming installation from a copied file alone. Forge deliberately does not
write into host-owned skill/plugin directories.

## Provision workflow authority

Workflow observations are signed by operator-owned credentials whose location
is derived from the Project Link. The agent never selects a registry or secret
path and private key bytes are never printed:

```bash
forge-core workflow credential provision --root <repo> \
  --credential-id credential.workflow.local-human \
  --principal-id principal.workflow.local-human \
  --agent-id agent.workflow.human-console --profile human --json

forge-core workflow credential status --root <repo> --json
forge-core workflow credential sign --root <repo> \
  --credential-id credential.workflow.local-human \
  --kind applicability --request-file <request.json> \
  --output-file <attestation.json> --json
```

Closed profiles are `human`, `agent` (alias `reviewer`), and `runtime`; each
receives only its role-compatible grants. Use `rotate --replaces <old-id>` or
`revoke --credential-id <id>` for lifecycle changes. P7a is not yet a one-call
broker: the agent still supplies the exact typed request and passes the emitted
attestation to the corresponding existing `workflow *-authorize` command.

This command is a **cooperative local signing proxy**, not proof that a physical
human, independent reviewer, or runtime was present. A process running as the
same OS principal can invoke it. Do not treat a configured `human` role as
human-presence evidence; high-authority profiles require an operator/host
approval boundary outside the agent process. That broker remains a P7a.2 exit
condition. The current command validates request semantics and profile grants,
serializes lifecycle mutation with a lock, and rejects existing path aliases,
but Windows files still inherit the operator directory's ACL.

## Operate the agent-native loop

The host agent runs once per chat:

```bash
forge-core start --root <project> --json
```

For a fresh repo, `start` creates the Project Link and sibling sidecar. Do not
run `project init` as a second required bootstrap step; it is an idempotent
advanced/migration surface.

The integration then executes structured argv and follows:

```text
start
  -> workflow init (first use) or workflow resume
  -> workflow release-status
  -> exact upgrade_argv, when returned and approved by policy
  -> workflow next
  -> perform and verify the governed action
  -> record through an authorized surface
  -> workflow next
```

Never split or shell-evaluate a display command string. Never reconstruct a
CAS-bound upgrade from prose. See [Agent integration](agent-integration.md).

## State and ownership

```text
<parent>/
  <project>/
    .forge-method.yaml
  forge-<project>/
    .forge-method/
<operator-owned-root>/
  trust anchors, registries, private material
```

- Product source belongs in the consumer repo.
- Workflow ledgers, receipts, evidence, claims, and Domain Pack generations
  belong in the sibling sidecar.
- Private keys, monotonic anchors, and operator registries stay outside both.
- Do not edit ledgers, receipts, active pointers, registries, or signatures by
  hand.

### Preflight profile storage

In the current source checkpoint, `forge-core preflight init` resolves a valid
Project Link and writes `preflight.yaml` under the sibling state root. It must
not create `<consumer>/.forge-method/`. A standalone repository without a
Project Link uses `<repo>/.forge-preflight.yaml` for pre-bootstrap use.

This correction is newer than the latest tagged prebuilt release. Verify the
binary version and inspect the returned path. If a linked consumer receives a
local `.forge-method/`, stop and upgrade or report the mismatch; do not normalize
that output as an exception to the sidecar invariant.

## Update safely

Replacing a binary does not move the durable workflow release or effective
Domain Pack epoch. After updating:

1. run `forge-core --version`;
2. run `start` and `workflow release-status`;
3. execute only an exact returned `upgrade_argv`;
4. call `workflow next` again.

A core release upgrade with an active Domain Pack generation can correctly
return a rebase requirement. Do not bypass it by editing the ledger or active
pointer.

## Backup and recovery

Stop processes that may mutate Forge state before taking a filesystem backup.
Preserve together:

- `.forge-method.yaml` from the consumer;
- the complete sibling sidecar;
- separately protected operator registries/anchors needed to verify it;
- the exact binary version or source commit.

Restoring an old sidecar or anchor independently can be detected as rollback.
Prefer typed recovery commands over restoring individual files:

```bash
forge-core start --root <project> --json
forge-core workflow resume --root <project> --json
forge-core domain-pack status --state-root <sidecar>/.forge-method --json
forge-core domain-pack recover --state-root <sidecar>/.forge-method --json
```

Preserve evidence before remediation. Never delete the sidecar, truncate a WAL,
or provision a new trust root merely to make an integrity error disappear.

## Troubleshooting order

1. Confirm `forge-core --version` and the intended source/tag.
2. Confirm the canonical project root and `.forge-method.yaml` target.
3. Run `start`; do not reinitialize over an integrity failure.
4. Run `workflow release-status` and `workflow resume`.
5. For Domain Packs, run `status`, then `recover` only when reported necessary.
6. Treat stale snapshot/head errors as a request to obtain fresh guidance, not
   as permission to remove CAS arguments.
7. Treat missing capability/evidence/domain output as a governed gap.
8. Retain redacted output and exact command argv when reporting a defect.

## Security boundary

Local confinement assumes cooperating processes under the same OS principal.
Use separate OS principals, permissions/sandboxing, and remote immutable storage
when another same-user process is hostile. Read [Security model](security-model.md)
and the repository [security policy](../SECURITY.md) before trusted mutation.
