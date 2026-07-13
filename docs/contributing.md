# Contributing to Forge Method Core

Forge is an authority-bearing Rust runtime. A change is complete only when its
behavior, contracts, generated projections, evidence, documentation, and
compatibility claim agree.

The short repository entry point is [`CONTRIBUTING.md`](../CONTRIBUTING.md).

## Prerequisites

- Rust 1.85 or newer with `rustfmt` and `clippy`;
- Git;
- Python 3 for repository generators and documentation/link checks;
- platform build tools required by the Rust target.

The repository follows stable Rust for development while crate metadata keeps
the supported minimum at Rust `1.85`. The release workflow pins its toolchain
separately so a tagged checkpoint does not silently change compiler identity.

## Orient before editing

Read, in order:

1. [`CONTEXT.md`](../CONTEXT.md) for domain language and trust boundaries;
2. [`docs/architecture.md`](architecture.md) for layering and durable state;
3. the nearest contract/spec and tests for the surface being changed;
4. [`docs/verification.md`](verification.md) for the proportional test ladder.

Use [`docs/generated/workspace-layout.md`](generated/workspace-layout.md) to
find crate ownership and [`docs/generated/command-surface.md`](generated/command-surface.md)
to check the public CLI/MCP projection.

## Change discipline

- Keep untrusted DTOs separate from opaque admitted authority.
- Do not let YAML, audit output, fixtures, or caller-selected paths mint
  authority.
- Keep domain-specific behavior in Domain Packs rather than universal core.
- Preserve raw-byte, canonical-document, release, policy-set, generation, and
  effective-epoch identities as distinct concepts.
- Make additive schema changes inside a compatible minor line or provide an
  explicit migration.
- Never weaken a test merely to make a new behavior pass.
- Do not edit generated files without running their generator.
- Update public docs and changelog claims in the same checkpoint as behavior.

## Efficient development loop

Do not run the entire workspace after every edit. Prefer the smallest command
that proves the changed boundary:

```bash
cargo fmt --all -- --check
cargo test -p <affected-package> [--test <integration-test>]
cargo clippy -p <affected-package> --all-targets -- -D clippy::pedantic
```

Run affected schema, fixture, CLI, and validator tests when a contract crosses
crate boundaries. When the coherent slice is ready, run the default workspace
once with `cargo test --workspace`; this deliberately excludes the expensive
P6d journey. At the integrated publication boundary, add the exact filtered
P6d command, or run `cargo test --workspace --features expensive-p6d-e2e` once
as a combined gate when the default workspace was not already run. The exact
tiers are in [Verification](verification.md).

## Generated material

CI checks at least:

```bash
python scripts/generate-workspace-layout.py --check
cargo run -p forge-core-command-surface --example generate_command_surface_docs -- --check
```

Workflow release/evidence/admission generators live under
`crates/forge-core-decisions/examples/` and are enumerated in
`.github/workflows/ci.yml`. Run the generator associated with changed embedded
release material, then review the semantic diff; do not copy expected digests
from a failure message without recomputing the full subject.

## Fixtures and docs

- Valid fixtures demonstrate closed wire shapes, not automatic authority.
- Adversarial fixtures should fail for the intended reason.
- Reference corpora must state whether they are candidate, reviewed, installed,
  or merely test evidence.
- Runnable snippets must identify whether they target a consumer repo, the Forge
  core bootstrap exception, a sidecar, or an operator-owned directory.
- Keep local Markdown links valid and do not link to private sibling checkouts.

## Pull request evidence

A reviewable change should report:

- intent and affected trust boundary;
- files/crates/contracts changed;
- focused commands and results;
- final cumulative gate result when required;
- generated artifacts and why they changed;
- migration and compatibility impact;
- unresolved security or operability risks.

Keep commits coherent and pullable. Do not combine unrelated formatting,
fixture regeneration, and behavioral changes unless they are inseparable.

## Release changes

Before a tag:

1. reconcile workspace version, changelog section, product-status docs, and tag;
2. run the complete publication gate;
3. verify `distribution/release-payload.txt`, `RELEASE-MANIFEST.json`, and
   archive contents on each supported platform;
4. verify each checksum and Sigstore bundle;
5. verify the mandatory release-level SBOM was produced and structurally validated;
6. smoke-test `--version`, `start`, and validation from the packaged binary;
7. ensure installation docs point to the released feature level rather than a
   newer source-only checkpoint.

## Security-sensitive reports

Do not place exploit details, private keys, anchors, credentials, or unredacted
operator paths in a public issue. Follow [`SECURITY.md`](../SECURITY.md).
