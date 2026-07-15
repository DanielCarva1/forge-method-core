# Forking and customizing Forge

Apache-2.0 permits modification and redistribution. Copying upstream signatures,
registries, digests, release claims, or names does not transfer authority.
Choose exactly which of the following three paths you own.

## Path 1: Domain Pack extension

Choose this path for domain policies, obligations, hazards, lifecycle models,
evaluators, fixtures, capabilities, or Adapter declarations that do not change
core trust semantics.

Obligations:

- own a non-core namespace and reject sealed-core shadowing;
- bind exact raw/canonical manifest and content identities;
- declare core compatibility, dependencies, conflicts, and capabilities;
- provide representative/adversarial evidence and preserve missing-pack gaps;
- establish publisher/registry trust, independent semantic review, separate
  registry authorization, operator anchors, and lifecycle preflight;
- distribute candidate package bytes from an explicit artifact root; activation
  must create immutable state-root objects/generations and an effective epoch;
- never market copied reference fixtures as a trusted public registry.

This extension does not require renaming the Forge binary or signing upstream
core releases. It does require pack-owned identity, support, license/material,
and supply-chain statements. See [Domain Packs](domain-packs.md).

## Path 2: Host Adapter integration

Choose this path for a new agent host, MCP/client bridge, shell, IDE, UI, broker,
installer, or keystore integration that leaves core authority rules unchanged.

Obligations:

- consume structured argv/envelopes and preserve argument boundaries;
- never reimplement policy, mint registries/receipts, or cache guidance across
  mutation;
- keep host installation/plugin paths outside Forge's implicit authority;
- authenticate origin subjects and protect broker private keys outside Forge;
- state the host's actual separation/identity assurance—configured labels or a
  second agent are not automatically independent actors;
- classify direct editor/shell/plugin writes as ungoverned; claim mediation only
  for the full claim/gate/principal/Admission/WAL/receipt chain;
- test fresh/existing bootstrap, stale CAS, broker absence/revocation, path spaces,
  replacement-session resume, and secret redaction;
- publish adapter-specific install, compatibility, threat-model, and support docs.

An Adapter may reference upstream release assets, but it must verify the selected
asset's upstream identity and must not claim the adapter itself was signed or
reviewed by upstream.

## Path 3: Core authority fork

Choose this only when changing the governance protocol, trust boundary,
persistence, closed contracts, binary/command surface, or shipped release
material.

Obligations:

- establish fork-owned repository/homepage/authors, remotes, binary/package name
  where confusion is possible, publisher/audience/namespace IDs, support, and
  vulnerability route;
- replace release links and Sigstore certificate identity with the fork's exact
  repository workflow-and-tag identity;
- assign new schema/protocol/version identity for incompatible authority changes
  and document migration/threat-model consequences;
- preserve deny-unknown-fields, candidate-versus-admitted separation, Project
  Link/state/operator boundaries, deterministic selection/composition, distinct
  identities, CAS/replay/freshness/signature/review gates, representative
  evidence, explicit gaps, and unambiguous crash recovery—or explicitly disclaim
  every changed invariant;
- regenerate content-addressed subjects and generated command/workspace docs;
  never patch terminal digests from a failure message;
- run Tier 0, focused, native Linux/Windows/Intel macOS/Apple Silicon macOS
  platform, and cumulative journey evidence;
  retain exact test ownership/triggers and timing reports;
- publish only after exact tag/commit/workspace/CLI/manifest agreement, packaged
  install smoke, checksums, fork-owned Sigstore bundles, and a validated SBOM.

## Release identity for a core fork

A fork's archive `RELEASE-MANIFEST.json` must bind its product, version, exact
`release_tag`, full `source_commit`, and every payload member. Verification must
accept the fork workflow/tag identity and reject upstream or unrelated
identities. Manual rebuilds must not publish branch bytes. Update skill, install,
release, and SBOM examples to fork names.

Do not collapse the [four identities](../README.md#four-identitiesdo-not-collapse-them):
a fork source checkpoint, its latest prebuilt, a project's workflow release pin,
and its Domain Pack effective epoch remain separate.

## License and notices

Retain Apache-2.0 and required attribution. Add `NOTICE` only for actual notices;
do not link to a nonexistent file. Record new dependency and bundled-asset
license obligations.

## Readiness checklist

- The selected path above is explicit; its obligations and non-obligations are
  documented.
- A clean clone/build has no private sibling-checkout paths; local links resolve.
- Version, commit/tag, package metadata, changelog, release name, manifest, and
  installed `--version` agree where the path owns them.
- Fresh bootstrap reaches `workflow next` without hand-authored authority YAML.
- Replacement agents resume only durable state.
- Direct writes, host identity limits, and residual risks are disclosed.
- No `0.12.0`, real-host, independence, or full-P7 claim is copied from source
  documentation without the fork's own executable evidence.
