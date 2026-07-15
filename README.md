# Forge Method Core

Forge Method Core is a local, model-agnostic governance runtime for agent-led
product work. A human stays in chat, a host agent drives `forge-core`, and Forge
persists typed obligations, authority, evidence, and continuity.

The source workspace is `0.12.0`, but source text alone proves neither public
prebuilt availability nor complete P7 product evidence. Verify the selected
GitHub Release, exact asset sidecars, and matching CI run. `v0.4.0` was the
previous independently verified Rust prebuilt before this release candidate;
do not use that historical fact as a current availability claim. See
[Product status](docs/product-status.md) and the
[promise audit](docs/product-compliance-audit.md).

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
| **Source checkpoint** | Workspace package SemVer `0.12.0` plus the exact Git commit/working-tree state. A dirty checkout is not immutable. | `git rev-parse HEAD`, `git status --short`, and `[workspace.package].version` in `Cargo.toml` |
| **Latest prebuilt** | Not inferable from a source checkout. `v0.4.0` is the historical predecessor to this `0.12.0` release candidate; availability may change when the exact tag workflow succeeds. | Selected GitHub Release, asset checksum/Sigstore bundle, embedded CLI `--version`, and archive manifest |
| **Workflow release identity** | Compiled append-only successor `workflow-governance.release.universal-assurance-v0` / `0.5.0` (six releases, 43 policies). Each project has its own durable pin and may still be on a predecessor. | `forge-core workflow release-status --root <project> --json` and only its exact returned upgrade argv |
| **Domain Pack effective epoch** | Project-local digest joining the admitted workflow release with the active immutable Domain Pack generation. It has no global package SemVer and does not rewrite core identity. | `workflow next|resume` and `domain-pack status` against the Project Link-resolved state |

This is the canonical identity table. Package SemVer, a published asset, a
project's workflow pin, and its effective epoch answer different questions.

## Install and start

For the current source checkpoint:

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
cargo install --path crates/forge-core-cli --force
forge-core --version
```

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
| Workflow public trust registries | `<sidecar>/operator/workflow-principal-registry.yaml` and `<sidecar>/operator/workflow-broker-registry.yaml` |
| Cooperative local credential secrets | `<sidecar>/operator/workflow-secrets/<sha256-of-credential-id>.ed25519` |
| Domain Pack candidate package bytes | Operator/host-selected `--artifact-root`; admitted copies live under `<state-root>/domain-packs/objects/` and immutable generation records under `<state-root>/domain-packs/generations/` |
| External broker private keys, Domain Pack trust roots, replay anchors | Operator-selected storage outside project and state roots; Forge does not choose or print those private paths |
| Release assets | The operator's download/install directory; inside each new-format archive, binary/wrapper and `RELEASE-MANIFEST.json` are at archive root and shipped guides retain repository-relative paths |

A custom Project Link can change sidecar/state paths while preserving the same
separation rules. Use returned paths as authority; never create
`<project>/.forge-method/` manually.

## Mutation and evidence boundary

Forge can govern only **Forge-mediated writes** that pass claims/gates,
verified principal and Admission checks, WAL/recovery, and receipt persistence.
An editor, shell command, host plugin, or other process can write directly to
the filesystem; those writes are **direct/ungoverned** unless a Forge-mediated
transaction covers them. Filesystem access is not evidence of governance.

The P7F bundle checker validates only closed structure, path safety, sizes, and
SHA-256 bindings. It does not certify a production host, chat-only behavior,
actor or reviewer independence, semantic truth, publication, or P7F passage.
See [Real-host proof](docs/real-host-proof.md).

## Release and CI evidence in source

The source release workflow binds requested tag, checked-out commit, workspace
version, CLI version, archive `release_tag`, and archive `source_commit`.
Native x86_64 Linux and Windows plus Intel and Apple Silicon macOS archives are
extracted and smoked through binary/wrapper version plus `start`,
`workflow init`, `resume`, `release-status`, and `next`. Publication additionally
re-verifies manifests,
checksums, Sigstore identities, and a schema-validated release-level CycloneDX
SBOM. These controls become release evidence only for a matching successful
tag run and the exact independently verified assets.

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
