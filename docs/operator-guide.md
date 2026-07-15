# Operator guide

This guide is for the person or automation responsible for installing Forge,
connecting it to a host agent, preserving runtime state, and handling recovery.
The normal project user stays in chat and does not operate these commands.

## Choose an installation channel

Forge has four distinct identities. The canonical table is in the
[root README](../README.md#four-identitiesdo-not-collapse-them). For installation:

- source workspace `0.12.0` means package SemVer plus exact source commit;
- selected prebuilt availability is verified from its release assets (`v0.4.0`
  is only the historical predecessor to this candidate);
- each project separately pins a workflow-governance release;
- each active Domain Pack generation produces a project-local effective epoch.

Always inspect the selected commit/tag, run `forge-core --version`, verify the
archive, and query `workflow release-status`; never substitute one identity for
another. Release/CI hardening is implemented in source; only a matching
successful tag run and verified assets establish publication, and neither
publication nor source alone establishes full P7 completion.

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

Download only assets belonging to one selected release. Historical `v0.4.0`
archives contain only the platform executable and thin `forge`/`forge.cmd`
wrapper. A new-format archive places the binary,
wrapper, and `RELEASE-MANIFEST.json` at archive root; allowlisted skill/docs keep
their repository-relative paths. The manifest binds product, package version,
exact `release_tag`, full `source_commit`, source epoch, and each payload file's
path, SHA-256, size, and mode. Full source contracts are not implied unless
listed in that archive's manifest.

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
Older releases may omit an SBOM. Current source requires one schema-validated
release-level CycloneDX SBOM before publication. It also extracts native
x86_64 Linux/Windows and Intel/Apple Silicon macOS archives into clean temporary
roots and runs binary and wrapper `--version` plus `start`, `workflow init`,
`workflow resume`, `workflow release-status`, and `workflow next` against a
consumer path with a space. This packaged-install smoke is a future tag gate, not proof that
`0.12.0` has been published.

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
`revoke --credential-id <id>` for lifecycle changes. The low-level `sign` plus
`*-authorize` lane remains an expert compatibility surface, not the normal
agent-native path.

For a packet whose returned approval boundary is exactly
`operator_credential_broker`, the cooperative local lane can avoid intermediate
request and attestation files:

```bash
forge-core workflow action authorize --root <repo> \
  --packet-digest <sha256> --input-file <closed-input.json> \
  --credential-id <operator-credential-id> --json
```

Forge rejects this command for `human_approval_broker`,
`independent_reviewer_broker`, and `trusted_runtime_broker` packets before
signing. Those boundaries require `workflow action apply` with a signed event
from the external broker described below.

This command is a **cooperative local signing proxy**, not proof that a physical
human, independent reviewer, or runtime was present. A process running as the
same OS principal can invoke it. Do not treat a configured `human` role as
human-presence evidence; high-authority profiles require an operator/host
approval boundary outside the agent process.

### Enroll an external origin broker

The P7a.2 source surface stores only a broker public key and a content digest
of the operator's out-of-agent enrollment record. The host retains the private
key and authenticates the inbound human, reviewer, or runtime subject:

```bash
forge-core workflow broker trust --root <repo> \
  --issuer-id broker.host.human.v1 --profile human \
  --public-key-file <ed25519-public-key-hex> \
  --ceremony-ref operator://enrollment/human/v1 \
  --ceremony-file <operator-enrollment-record> --json

forge-core workflow action-packets --root <repo> --json
forge-core workflow action apply --root <repo> \
  --origin-envelope-file <host-signed-origin-event.json> --json
```

Use `workflow broker rotate|revoke|status` for lifecycle changes. The broker
event carries only a closed semantic answer. Forge regenerates the current
packet and derives policy, phase, evaluator, target, digests, clock fields, and
the exact authority request. A stale packet, wrong project/profile/kind,
expired event, changed registry, or replay fails closed. The apply command
does not emit a reusable workflow signature. Its bounded replay WAL and ledger
provenance form a crash-recoverable saga: an exact retry can finish a reserved
post-ledger replay index or return its prior durable result. Forge writes no
pre-ledger reservation that could strand a current packet after expiry; the
ledger lock serializes the mutation, and its origin companion is the recovery
authority. The two stores are not presented as one cross-filesystem atomic
transaction.

Enrollment metadata is a trust declaration, not proof that Forge observed the
ceremony. A configured broker proves only that a configured external key
signed an event containing an authenticated-origin claim; physical presence
and identity assurance remain properties of the host deployment. Windows
operator files inherit the operator directory's ACL.

### Operate durable intent and representative assurance

Only an external broker enrolled with the `human` profile may originate an
intent revision. The host asks for a closed semantic answer in chat and calls:

```bash
forge-core workflow intent record --root <repo> \
  --origin-envelope-file <human-broker-signed-intent-event.json> --json
```

The event contains the desired outcome, constraints, preferences,
unacceptable outcomes, uncertainties, and conversation reference/digest. It
must not choose an intent id, revision, assurance epoch, phase, policy, target,
or status; Forge derives those coordinates. An amendment opens the next epoch
and prevents stale evidence from remaining current.

The host agent, not the human, drafts the typed representative-slice manifest.
Treat it as untrusted content until an independently enrolled `reviewer` broker
admits the exact digest through the evidence action packet. Execution evidence
then requires a `runtime` broker in a different configured separation domain
and exact bindings to the latest accepted definition, runtime subject, current
snapshot/effective epoch, and every declared scenario. Keep using `workflow
action apply`; there is no separate slice database or mutation command.

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
     (includes authorization.action_packets and typed setup_gaps)
  -> workflow action-packets (optional standalone read-only packet projection)
  -> perform and verify the governed action or request the irreducible choice
  -> workflow action authorize (operator-credential packets only)
     OR host signs the inbound origin event and workflow action apply
  -> workflow next
```

Never split or shell-evaluate a display command string. Never reconstruct a
CAS-bound upgrade from prose. See [Agent integration](agent-integration.md).

Only writes committed through Forge's admitted claim/gate, principal, Admission,
WAL/recovery, and receipt path are **Forge-mediated**. Editor, shell, installer,
or host-plugin writes are direct/ungoverned unless that transaction covers them;
record them as such in evidence and never infer mediation from a transcript.
## State and ownership

For a default Project Link, paths are exact and derived from `project_id`:

| Material | Canonical/default location | Ownership |
|---|---|---|
| Product source | `<project>/` | Consumer |
| Project Link | `<project>/.forge-method.yaml` | Consumer pointer only |
| Runtime sidecar | `<project-parent>/forge-<project-id>/` | Forge runtime container |
| State root | `<sidecar>/.forge-method/` | Ledgers, receipts, evidence, WALs, Domain Pack lifecycle |
| Workflow principal registry | `<sidecar>/operator/workflow-principal-registry.yaml` | Operator public trust metadata |
| Workflow broker registry | `<sidecar>/operator/workflow-broker-registry.yaml` | Operator public broker trust metadata |
| Cooperative local secrets | `<sidecar>/operator/workflow-secrets/<sha256-of-credential-id>.ed25519` | Operator; never printed |
| Domain Pack candidate package bytes | Host-selected `--artifact-root` | Candidate/untrusted input |
| Admitted Domain Pack objects | `<state-root>/domain-packs/objects/<digest-token>` | Immutable runtime state |
| Admitted Domain Pack generations | `<state-root>/domain-packs/generations/<generation>-<record-token>/` | Immutable runtime state |
| Domain Pack trust/learning files and monotonic anchors | Explicit external `--operator-root`, direct children of that root where required | Operator; outside project/artifact/state controlled roots |
| External broker private keys | Host keystore/path outside Forge roots | Host/operator; Forge stores only public key + ceremony digest |
| Replay anchor | Explicit operator-selected path outside state root | Operator-protected monotonic authority |
| Downloaded release assets | Operator-selected download directory | Keep archive, `.sha256`, `.sigstore`, and release-level SBOM together |
| Installed release | Operator-selected `PATH` directory | Binary and matching wrapper from one archive |

A custom Project Link may change sidecar/state paths, but returned resolved paths
are authoritative and the state root must remain a `.forge-method` directory
inside the sidecar and outside the consumer project. Forge-derived workflow
registry/secret paths then follow the resolved sidecar parent. Do not edit
ledgers, receipts, active pointers, registries, anchors, or signatures by hand.

### Preflight profile storage

In the current source checkpoint, `forge-core preflight init` resolves a valid
Project Link and writes `preflight.yaml` under the sibling state root. It must
not create `<consumer>/.forge-method/`. A standalone repository without a
Project Link uses `<repo>/.forge-preflight.yaml` for pre-bootstrap use.

Verify that the selected binary contains this correction by checking its
version, manifest, and returned path. If a linked consumer receives a
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
