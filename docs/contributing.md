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
- Preserve source checkpoint, published prebuilt, workflow release pin, and
  Domain Pack effective epoch as the four distinct identities in the
  [canonical table](../README.md#four-identitiesdo-not-collapse-them).
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
crate boundaries. CI ownership is Tier 0 (120-second static/doc steps), focused
(900-second package/integration steps), native Linux/Windows/Intel macOS/Apple
Silicon macOS platform gates (1,800-second steps), and one push-only cumulative
P6d journey (1,800 seconds). Every wrapped step enforces the budget, terminates
its child process tree on timeout, emits timing JSON, and preserves failures
that occur before the deadline. Do not delete a test to meet a budget; move it
only with an explicit owner, trigger, compile path, and inventory rationale.
Exact commands and artifacts are in
[Verification](verification.md).

### Inspect and clean the development build cache

Cargo keeps the reusable `dev` and `release` profiles in the configured target
directory. Inspect how much regenerable development data would be removed:

```bash
cargo clean --dry-run --profile dev
```

When disk space is tight, remove only that development profile:

```bash
cargo clean --profile dev
```

Keep `--profile dev` in the command. Running `cargo clean` without a profile
also removes the release cache. Forge never runs this cleanup automatically as
part of tests, builds, or installation.

## Generated material

CI checks at least:

```bash
cargo run --locked -p forge-core-validate --example generate_workspace_layout -- --check
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

Documentation-only changes run
`cargo test --locked -p forge-core-validate --test checked_in_markdown_links`
and `git diff --check`; do not run unrelated Rust gates merely to inflate
evidence.

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

1. reconcile source package version, exact commit/tag, changelog/status docs,
   and release name without confusing the project's workflow pin/effective epoch;
2. require successful CI for the exact release commit;
3. verify `distribution/release-payload.txt` and that `RELEASE-MANIFEST.json`
   binds exact version, `release_tag`, full `source_commit`, and every member;
4. verify each archive checksum and Sigstore bundle against the fork/repository
   workflow-and-tag identity;
5. schema-validate the mandatory release-level CycloneDX SBOM;
6. extract and smoke native x86_64 Linux/Windows plus Intel and Apple Silicon
   macOS packages through binary/wrapper version plus `start`, `init`, `resume`,
   `release-status`, and `next`;
7. ensure install docs describe the published feature level, not newer source.

Passing source checks does not mean the tag/assets were published.

## Agent skills

This section orients contributor agents (Claude Code, OpenCode, Codex, etc.)
working in this repo. Personal agent contracts and preferences live in each
contributor's global config, not here.

### Issue tracker

GitHub Issues at `DanielCarva1/forge-method-core`. Skills and scripts use the
`gh` CLI.

### Triage labels

Canonical defaults: `needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, `wontfix`. Apply `ready-for-agent` to any ticket that is
unblocked and sized for a single agent session.

### Domain docs

- Single-context: [`CONTEXT.md`](../CONTEXT.md) at the repository root is the
  authoritative ubiquitous language.
- Architectural decisions are recorded as ADRs under `docs/adr/` when a
  decision is non-obvious and durable.

### Build and gates cheat sheet

- MSRV check:
  `cargo +1.85.1 check --locked --workspace --all-targets --all-features`.
- Clippy (pedantic, deny):
  `cargo clippy --workspace --all-targets --all-features -- -D clippy::pedantic`.
- Validate repo contracts:
  `cargo run -p forge-core-cli -- validate --root .`. Local agent-tool dirs
  (`.claude/`, `.zcode/`, `.pi/`) must be moved aside first if they contain
  Markdown, since the validator scans the filesystem, not `.gitignore`.
- Workspace tests: `cargo test --workspace`. (Do not combine `--locked` and
  `--quiet`; that variant is not a real cargo flag combination.)
- Windows-native crash-replace and cross-host path identity have regression
  coverage, but native durability remains best-effort; use WSL2 when the
  production-grade durability guarantee is required.

## Security-sensitive reports

Do not place exploit details, private keys, anchors, credentials, or unredacted
operator paths in a public issue. Follow [`SECURITY.md`](../SECURITY.md).
