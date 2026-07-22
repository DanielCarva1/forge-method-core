# Product lifecycle

The product-lifecycle command family owns a local product installation root without
taking ownership of any consumer project, Project Link, Forge sidecar, operator
anchor, backup, or externally managed key. Every command requires an explicit
`--install-root`; the root is never inferred from the current repository.

## Public commands

```text
forge-core lifecycle setup --install-root <path>
forge-core lifecycle status --install-root <path>
forge-core lifecycle doctor --install-root <path>
forge-core lifecycle install --install-root <path> --release-file <yaml> --trusted-verification-file <yaml>
forge-core lifecycle update --install-root <path> --release-file <yaml> --trusted-verification-file <yaml>
forge-core lifecycle rollback --install-root <path>
forge-core lifecycle uninstall --install-root <path>
```

Add `--json` for the shared `CliEnvelope` JSON representation or `--no-json` for
human-readable envelope output. Canary release documents additionally require
`--explicit-canary-opt-in`. Dev-channel bundles are not admitted for general
installation.

## Setup and ownership

`setup` creates or reuses one exact ownership marker. A fresh empty root can be
claimed; a nonempty unmarked root is rejected. A mismatched marker, symlinked
path component, special file, or malformed lifecycle state fails closed.
Lifecycle state lives only beneath the explicit installation root and the
Store-owned `product-lifecycle` namespace:

```text
<install-root>/
  .forge-product-lifecycle.json
  .lifecycle.lock
  product-lifecycle/
    owner.json
    state.json
    staging/
    generations/
      generation-<release-digest>/
        generation.json
        receipt.json
        assets/
          ... exact installed assets ...
```

`.forge-product-lifecycle.json` is the explicit installation-root ownership
marker, while `product-lifecycle/owner.json` seals the retained Store namespace.
The effect lock is `.lifecycle.lock`; the only durable active/previous selection
and inventory authority is `product-lifecycle/state.json`. Each immutable
generation owns its `generation.json`, `receipt.json`, and exact asset bytes;
there is no parallel top-level lifecycle state or receipt tree. These records
mean only that Forge may manage exact inventory below that root. They are not
project, release, signing, trust, protected-anchor, or host-selection authority.

## Release bundles and verification

A release file is a closed `ProductLifecycleReleaseDocument`. It binds semantic
version and core compatibility, channel, immutable source and provenance
references, rollback reference, typed release notes, and a bounded asset list.
Every asset binds one safe relative source path, one safe relative install path,
one exact lowercase SHA-256 digest, executable intent, typed kind, and an
optional host target where the kind requires it.

Before any installation mutation, the lifecycle adapter:

1. validates the closed release document;
2. reads release and asset files with bounded regular-file/no-follow checks and
   rejects hard-link ambiguity;
3. reuses existing host-adapter distribution admission;
4. reuses existing local artifact verification;
5. independently rechecks the exact bytes it will install against the declared
   SHA-256; and
6. checks the release's semantic-version compatibility with the running core.

Passing this boundary does not by itself establish signer identity, complete
provenance-predicate semantics, transparency-log inclusion, publication,
support, host selection, or release authority. Those remain separately owned.
The valid source fixture is
[`docs/fixtures/product-lifecycle-v0/valid/release.yaml`](fixtures/product-lifecycle-v0/valid/release.yaml).

## Install, update, and rollback

Assets are first written under `product-lifecycle/staging` and then published as
an immutable content-addressed generation. `product-lifecycle/state.json`
changes only after the complete generation and its generation-local receipt
exist.

- `install` requires no distinct active release. Retrying the exact active
  release is idempotent.
- `update` requires an active release, rejects downgrade and same-version content
  drift, and retains the previous working generation.
- `rollback` verifies the prior generation's exact metadata and asset inventory
  before switching active and previous generation identities.
- Typed release notes in `status` come from the active generation's retained
  change records.

## Status and doctor

`status` reports marker validity, active and previous generation/version, typed
release notes, and exact host-configuration observations. `selected_host`
remains `none`; observing a host-targeted asset does not select, support, or
field-verify that host.

`doctor` additionally verifies immutable generation manifests,
generation-local audit receipts, and the retained asset inventory. Missing,
modified, or unsafe paths produce typed degraded diagnostics rather than silent
repair.

## Uninstall boundary

`uninstall` removes only exact product-owned regular files whose current digest
still matches the installed inventory. Modified files, symlinks, special files,
and unknown files are preserved. Directories are removed only when empty.

Consumer projects, Project Links, Forge sidecars, operator trust/replay anchors,
backups, restore receipts, public broker registries, and all external private
keys are outside lifecycle custody. Private external broker keys remain in their
owner-specific backup procedure and are never copied into Forge state or Forge
backups.

## Evidence status

The contracts, command surface, CLI implementation, and source-test targets
compile with targeted locked all-target checks. Rust test execution, interrupted
and mixed-version scenarios, failure injection, stress/fuzz/benchmarks, MSRV and
platform matrices, hosted CI, publication, downloaded-asset verification,
real-host execution, and field review remain deferred to their owning campaigns.
