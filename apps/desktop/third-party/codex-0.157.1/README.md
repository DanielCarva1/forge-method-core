# Bundled Codex CLI provenance

The Windows x64 desktop bundle includes the unmodified native files from the
optional `@openai/codex-win32-x64` package selected by `@openai/codex@0.157.1`
in `apps/desktop/package-lock.json`. The package is fetched during `npm ci`;
the binary and associated runtime resources are not committed to this repo.

The accompanying `LICENSE` and `NOTICE` were copied from the upstream
[`rust-v0.157.1` tag](https://github.com/openai/codex/tree/rust-v0.157.1).
The bundled native resource tree also carries its upstream third-party notices.
Forge has not modified Codex CLI. Its version must be reviewed and deliberately
updated with a subsequent desktop package; it is not silently auto-updated.
