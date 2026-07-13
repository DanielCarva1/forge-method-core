# Forking and customizing Forge

Apache-2.0 permits modification and redistribution, but a fork must establish
its own product and supply-chain identity. Copying upstream signatures,
registries, digests, or release claims does not transfer authority.

## Prefer an extension when possible

Use a Domain Pack when the change is domain knowledge: policies, obligations,
hazards, lifecycle models, evaluators, fixtures, capabilities, or Adapter
declarations. Use a host Adapter when the change is integration with an agent,
MCP client, shell, or UI. Fork universal core only when changing the governance
protocol, trust boundary, persistence model, or shipped command surface.

See [Domain Packs](domain-packs.md) before adding domain-specific Rust.

## Establish fork identity

Review and replace upstream identity in at least:

- `[workspace.package]` repository/homepage/authors in `Cargo.toml`;
- Git remotes and README/release links;
- `.github/workflows/release.yml` release body and expected Sigstore certificate
  identity shown to users;
- package/binary naming if the fork should not be confused with upstream;
- publisher, registry, reviewer, audience, namespace, and trust-policy IDs;
- support and vulnerability-reporting routes.

Do not keep verification examples that accept only the upstream repository's
GitHub OIDC identity while claiming they verify fork builds. The fork's release
workflow will receive its own certificate identity.

## Preserve protocol invariants

A compatible fork should continue to enforce:

- deny-unknown-fields on closed authority-bearing inputs;
- caller input/audit output never becoming admitted authority;
- Project Link, sidecar, and operator-secret separation;
- deterministic policy selection and Domain Pack composition;
- distinct raw/canonical/release/policy/generation/effective identities;
- CAS, replay, freshness, signature, and independent-review boundaries;
- representative evidence rather than artifact-presence completion;
- explicit gaps after removal or missing capability;
- crash recovery without ambiguous success.

If an invariant changes, document the threat-model and wire-compatibility break
and use a new schema/protocol/version identity.

## Generated and content-addressed material

Changing a contract, embedded release, fixture, or registry commonly changes
multiple derived digests. Use repository generators and semantic validators;
do not hand-edit only the terminal digest. Generated command/workspace docs must
also match the fork's actual surface.

Follow [Verification](verification.md) and [Contributing](contributing.md).

## Release workflow

A fork should:

1. choose an unambiguous version/tag policy, especially because upstream history
   contains legacy Python `v1.x`/`v2.x` tags as well as Rust `v0.x` tags;
2. ensure the selected tag points at the declared workspace version;
3. build and smoke-test every advertised target;
4. sign assets with the fork workflow's OIDC identity;
5. publish and verify the fork-owned payload plus `RELEASE-MANIFEST.json`;
6. publish per-asset checksums and Sigstore bundles;
7. preserve the mandatory validated release-level SBOM, or document an
   intentional fork policy change without claiming upstream-equivalent provenance;
8. update install and verification commands to the fork's repository and asset
   names.

## License and notices

Retain the Apache-2.0 license and required attribution. Add a `NOTICE` file only
when the fork has notices that must be distributed; do not link to a nonexistent
file. Record third-party license obligations introduced by new dependencies or
bundled assets.

## Fork readiness checklist

- A clean clone builds without paths to an upstream sibling checkout.
- All local Markdown links resolve.
- `--version`, package metadata, changelog, tag, and release name agree.
- Release verification accepts the fork identity and rejects an unrelated one.
- Fresh-project bootstrap reaches `workflow next` without manual YAML.
- The skill points to the fork binary and supported command surface.
- A replacement agent resumes only from durable fork state.
- Security/support routes are owned by the fork maintainer.
