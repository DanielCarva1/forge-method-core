# ADR-0008 - Fuzz runs on Linux CI, not Windows local

- **Status**: Accepted

## Context

The R4 track (`cargo-fuzz`) needs an environment where fuzz runs in a
reproducible and continuous way. Attempts on the Windows-MSVC development host
showed two known limitations of cargo-fuzz on that platform:

1. **Default ASAN (`-Zsanitizer=address`)** fails at runtime with
   `STATUS_DLL_NOT_FOUND (0xc0000135)`. The `nightly-x86_64-pc-windows-msvc`
   toolchain does not ship `clang_rt.asan_dynamic.dll`; only the static lib
   exists, and the MSVC linker cannot resolve the dynamic DLL expected by the
   instrumented binary.

2. **Coverage-only (`-s none`)** fails at link with undefined
   `LNK2001: __stop___sancov_pcs` errors. `-Zbuild-std` rebuilds the
   stdlib without coverage instrumentation, but external crates (tokio, hyper,
   reqwest, etc.) are still compiled with coverage and reference the section
   symbols that no longer exist at final link.

WSL2 Ubuntu is available on the host but has no Rust toolchain installed;
installing Rust/nightly there just to run fuzz would add an extra environment
to maintain with no benefit over a GitHub Actions Linux CI.

## Decision

Fuzz runs on **GitHub Actions Linux CI (`ubuntu-latest`)**, not
locally on Windows. The configuration lives in
`.github/workflows/fuzz.yml` and runs:

- Nightly schedule (cron) — catches silent regressions
- `workflow_dispatch` — manual on-demand execution
- Pull requests with the `fuzz` label — optional gate when a PR touches
  critical parsers

Each of the 4 targets (`parse_signed_checkpoint`,
`parse_rekor_log_entry`, `decode_ocsp_response`, `decode_prefix`) runs for
a limited time (5 min each on CI) with the seeds committed in
`fuzz/corpus/<target>/`.

Harnesses `.rs` + `fuzz/Cargo.toml` + `fuzz/.gitignore` + corpus seeds
remain committed in the repo. Developers on Linux/WSL can run
`cargo +nightly fuzz run <target>` locally; developers on Windows
should use the CI or a Linux environment.

## Consequences

- **Mature adoption**: cargo-fuzz is upstream Linux-first; this choice
  aligns with upstream maintenance.
- **CI cost**: the nightly workflow consumes ~20 min of GitHub Actions per day.
  Acceptable given the value of continuous regression.
- **Feedback loop**: normal PRs do not block on fuzz (only with the `fuzz`
  label). A bug found in cron becomes a separate issue.
- **Documentation**: the dev-docs README needs to explain "how to run fuzz
  locally on Linux" and "how to interpret a crash artifact".
- **Reversibility**: if cargo-fuzz Windows support matures (shipped ASAN
  DLL, fix for `__stop___sancov_pcs` in `-s none`), this ADR can be
  reverted with no code change — only by removing the documentation of the
  limitation.
