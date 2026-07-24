# AGENTS.md — forge-method-core

Project-specific instructions. Narrows/overrides the global
`C:\Users\User\.zcode\AGENTS.md` for this repo.

## Project

Forge Method Core. Rust workspace, 23 crates. Version `0.12.0`.
Repo: `D:\Forge-method-core`. Remote: `github.com/DanielCarva1/forge-method-core`.

## Agent skills

### Issue tracker

GitHub Issues (`DanielCarva1/forge-method-core`). Skills use `gh` CLI.
See `docs/agents/issue-tracker.md` if/when created.

### Triage labels

Canonical defaults: `needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, `wontfix`. (Repo has no custom labels yet; defaults apply.)

### Domain docs

Single-context. `CONTEXT.md` at repo root (25KB, authoritative ubiquitous
language). `docs/adr/` to be created (ADRs referenced in code but not yet
committed — ADR-0008, ADR-0009).

### Project memory (second brain)

Governed store: `D:\Gerenciador de vaults\10-Vaults\Agents-Memory\forge-method-core\`.
Read its `_index.md` first. Schema: `_decisions/adr-NNN-*.md`, `_facts/`,
`_sessions/`. Frontmatter `[scope, status, created, updated, source, confidence]`.
Format: contrato denso (caveman-friendly).

## Working agreements (owner, 2026-07-24)

- Token cost is NOT a constraint; time is. Parallelize aggressively with subagents.
- Always 2 adversarial reviewers (compared) for any verification/review/judge.
- Quality bar: excellent. Done = tests written+passing, errors handled,
  verified (run, don't claim), two-axis review done.
- New behavior always gets a test. Change => validate (build/lint/typecheck).
- If done twice, script it. Leave no scratch/temp behind.
- Product is owner's; agent may and should merge/tag/publish/release when green.
- Caveman communication style for chat; prose for artifacts (README/doc/report).

## Build/gates cheat sheet (see project memory _facts/build-and-gates.md for full)

- MSRV: `cargo +1.85.1 check --locked --workspace --all-targets --all-features`.
- Clippy: `cargo clippy --workspace --all-targets --all-features -- -D clippy::pedantic`.
- Validate: `cargo run -p forge-core-cli -- validate --root .` (move `target-*`/`.pi`/`.claude` out first — they cause false `markdown_not_allowlisted` locally).
- Test: `cargo test --workspace` (NO `--locked --quiet` — that variant doesn't exist).
- Windows native has known failures in crash-replace (os error 5) and path-normalization — campaign in progress.
