# 01 — Repair the authoritative static validator

**What to build:** Restore the repository's static validation entrypoint so it runs every declared structured-text, policy, release, and diff check exactly once and produces an actionable pass or failure instead of crashing before validation.

**Blocked by:** None — can start immediately.

**Status:** implemented-local-verified

- [x] The static validation entrypoint completes without an indexing or command-inventory exception.
- [x] Its declared check inventory and the checks it executes are identical, ordered, and duplicate-free.
- [x] A regression test fails when a required check is missing, duplicated, or referenced out of bounds.
- [x] The current repository can run the repaired validator and retain the complete command/result evidence.
- [x] No failed check is converted into a warning or allowed failure.

## Evidence — 2026-07-27

- python3 -I scripts/test-check-static-structured-text.py: 9/9 PASS.
- The literal authoritative PI-loop entrypoint reached the controlled diagnostic `record GAP-010.codex-conformance.notes must be a string list`; correcting that malformed authoritative list lets the full static validator continue without weakening failure semantics.
- `forge-core validate --root /home/user/Forge-method-core --json`: PASS, 188 checks, zero diagnostics.
- Two independent adversarial reviewers completed before a separate fix agent.
- No stage, commit, push, hosted CI, or external mutation occurred.
