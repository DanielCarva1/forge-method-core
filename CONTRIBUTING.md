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

Use the smallest owning command while editing. CI then owns four explicit
evidence classes: Tier 0 (120-second steps), focused (900-second steps), native
Linux/Windows/Intel macOS/Apple Silicon macOS platform gates (1,800-second
steps), and one push-only cumulative P6d journey (1,800 seconds). Do not silently
remove tests to meet a budget; preserve
an explicit owner, trigger, timing report, and compile path. Report commands,
results, generated artifacts, compatibility impact, and residual risks.

For documentation-only changes, the local minimum is:

```bash
python scripts/check-doc-links.py
git diff --check
```

Security vulnerabilities and secrets do not belong in public issues. Follow
[SECURITY.md](SECURITY.md).
