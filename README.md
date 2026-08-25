# Forge Method Core

Forge Method Core is a local, model-agnostic governance runtime for agent-led
product work. A human stays in chat, a host agent drives `forge-core`, and Forge
persists typed obligations, authority, evidence, and continuity.

The active source line is **`0.12.0-alpha.47`**. `Cargo.toml` is the single
package-version authority; live documentation does not use older prerelease
numbers to describe the current product. Historical versions remain only in
`CHANGELOG.md` and retained audit evidence. Verify the selected executable with
`forge-core --version` and the checkout with Git before relying on either.

## Current product scope

The active milestone is **Solo Dogfood Ready**: one developer working through
chat with one or more cooperative host agents. The typed authority is
[`contracts/spec/solo-dogfood-readiness-v0.yaml`](contracts/spec/solo-dogfood-readiness-v0.yaml).

External human-origin brokers, FIDO-backed presence, independent-reviewer
custody, and compliance-grade signing belong to a later enterprise profile.
They are not part of normal installation, `/start forge`, everyday local work,
or Solo Dogfood closure, and their absence must not block `solo_cooperative`.
Release-asset verification is a separate software-supply-chain concern.

## Choose one guide

| Audience | Canonical guide |
|---|---|
| Human using Forge through chat | [Getting started](docs/getting-started.md) |
| Host-agent or tool integrator | [Agent integration](docs/agent-integration.md) |
| Installation/trust/state operator | [Operator guide](docs/operator-guide.md) |
| Domain Pack author or operator | [Domain Packs](docs/domain-packs.md) |
| Contributor | [Contributing](docs/contributing.md) |
| Security or promise reviewer | [Security model](docs/security-model.md) |
| Fork or extension maintainer | [Forking and customization](docs/forking.md) |

The [documentation index](docs/README.md) links architecture, verification,
generated references, real-host evidence format, and status material without
turning prose into runtime authority.

## Four identities—do not collapse them

| Identity | Current source fact | How to verify |
|---|---|---|
| **Source checkpoint** | Workspace package SemVer `0.12.0-alpha.47` plus the exact Git commit/working-tree state. A dirty checkout is not immutable. | `git rev-parse HEAD`, `git status --short`, and `[workspace.package].version` in `Cargo.toml` |
| **Installed executable** | The binary actually selected by `PATH`; it may differ from the checkout until reinstalled. | `command -v forge-core` and `forge-core --version` |
| **Workflow release identity** | Compiled append-only successor `workflow-governance.release.universal-assurance-v0` / `0.5.0` (six releases, 43 policies). Each project has its own durable pin. | `forge-core workflow release-status --root <project> --json` and only its exact returned upgrade argv |
| **Domain Pack effective epoch** | Project-local digest joining the admitted workflow release with the active immutable Domain Pack generation. It has no global package SemVer and does not rewrite core identity. | `workflow next|resume` and `domain-pack status` against the Project Link-resolved state |

This is the canonical identity table. Source SemVer, the executable on `PATH`,
a project's workflow pin, and its effective epoch answer different questions.

## Install and start

For the current source checkpoint:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
cargo run --locked -p forge-core-xtask -- source-install
```

The Rust repository tool requires a clean checkout, builds the release binary, and reports
its exact installed path, Git commit, package version, and SHA-256. It keeps one
rollback and removes its own staging files instead of accumulating timestamped
backup copies. Add the reported `bin` directory to `PATH`, then verify with
`forge-core --version`. It reuses Cargo's effective target directory, including
`.cargo/config.toml`; set `CARGO_TARGET_DIR` to override that cache location.

For a prebuilt, select one verified release and follow
[Getting started](docs/getting-started.md). Do not infer source features from a
historical archive or assume that a tag without its verified assets is usable.

The host agent then runs:

```bash
forge-core start --root <project> --json
```

It executes the returned structured argv, initializes or resumes workflow
state, checks the durable workflow release, and asks `workflow next`. The
canonical procedure is [`skill/start-forge/SKILL.md`](skill/start-forge/SKILL.md).

## Storage boundaries

With the default project id, Forge uses these exact locations:

| Material | Location |
|---|---|
| Consumer source and pointer | `<project>/` and `<project>/.forge-method.yaml` |
| Runtime sidecar and state | `<project-parent>/forge-<project-id>/` and `<project-parent>/forge-<project-id>/.forge-method/` |
| Solo Cooperative runtime state | Ledger, receipts, evidence, WAL/recovery material, and immutable generations remain under the sidecar/state root; no external-origin broker is required. |
| Domain Pack candidate package bytes | Operator/host-selected `--artifact-root`; admitted copies live under `<state-root>/domain-packs/objects/` and immutable generation records under `<state-root>/domain-packs/generations/` |
| Release assets | The operator's download/install directory; inside each new-format archive, binary/wrapper and `RELEASE-MANIFEST.json` are at archive root |

A custom Project Link can change sidecar/state paths while preserving the same
separation rules. Use returned paths as authority; never create
`<project>/.forge-method/` manually.

## Mutation and evidence boundary

Forge can govern only **Forge-mediated writes** that pass the applicable
Solo Cooperative claims/gates, WAL/recovery, and receipt path. This is a
promotion boundary, not a signature or approval ceremony for every local edit.
An editor, shell command, host plugin, or other process can still write directly
to the filesystem; those writes are **direct/ungoverned** unless a
Forge-mediated transaction covers them. Filesystem access is not evidence of
governance.

The P7F bundle checker validates only closed structure, path safety, sizes, and
SHA-256 bindings. It does not certify a production host, chat-only behavior,
actor or reviewer independence, semantic truth, publication, or P7F passage.
See [Real-host proof](docs/real-host-proof.md).

## Release and CI evidence in source

The source release workflow binds requested tag, checked-out commit, workspace
version, CLI version, archive `release_tag`, and archive `source_commit`.
Every native release archive is extracted into a clean installation and must
complete the packaged Solo Dogfood journey: one `start`, a synthetic cooperative
objective supplied by the gate (not derived from a real conversation),
cooperative evidence, exact Git-isolated work, governed promotion and readback,
post-commit `recover` plus exact `apply` retry idempotence, replacement-process
continuity, finalized claim/isolation/worktree/branch cleanup, and zero retained
delete debt. This does not inject or prove a real crash-recovery window. The
x86_64 Linux reference archive must pass three consecutive fresh
journeys; machine-readable results are retained. Publication additionally
re-verifies manifests, checksums, Sigstore identities, and a schema-validated
release-level CycloneDX SBOM. This proves the packaged runtime, not that Codex,
Cursor, Claude, or another host performed the journey. These controls become
release evidence only for a matching successful tag run and the exact
independently verified assets.

CI separates Tier 0 (120-second static/doc step budgets), focused evidence
(900-second step budgets), native Linux, Windows, Intel macOS, and Apple Silicon
macOS platform evidence (1,800-second step budgets), and one push-only
cumulative P6d journey (1,800 seconds). Every wrapped step emits JSON timing
evidence and preserves command failure. Exact
triggers, commands, artifact names, and limitations are in
[Verification](docs/verification.md).

## License and security

Apache-2.0. Report vulnerabilities through [SECURITY.md](SECURITY.md), not a
public issue containing exploit details or secrets.
