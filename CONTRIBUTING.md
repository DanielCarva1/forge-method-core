# Contributing

Thank you for improving Forge Method Core. Because Forge governs authority and
durable mutation, contributions must preserve the distinction between typed
candidate data, verified external evidence, opaque admitted authority, and
kernel-derived state.

Start with the full [contributor guide](docs/contributing.md), then use the
[verification ladder](docs/verification.md). Architecture and trust boundaries
are documented in [docs/architecture.md](docs/architecture.md) and
[docs/security-model.md](docs/security-model.md).

Quick development loop:

```bash
cargo fmt --all -- --check
cargo test -p <affected-package>
cargo clippy -p <affected-package> --all-targets -- -D clippy::pedantic
```

Run the default workspace gate once the coherent slice is ready, not after
every small edit. Reserve
the exact feature-gated P6d command in `docs/verification.md` for the integrated
publication checkpoint. Report commands, results, generated artifacts,
compatibility impact, and residual risks with the change.

Security vulnerabilities and secrets do not belong in public issues. Follow
[SECURITY.md](SECURITY.md).
