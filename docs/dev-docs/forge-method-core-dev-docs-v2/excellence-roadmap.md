# Excellence Roadmap — forge-method-core

**Status:** v0.1.0 quick-wins landed (integrity, CLI/UX, docs consistency).
This document maps the remaining work toward "100% and excellent" that **does
not fit in one session**. Each item has context, files, an estimate, and an
acceptance gate. Work one item per session; commit; next.

> **Authorship:** This roadmap was produced after a brutal technical audit
> (two agents explored technical debt + first-contact UX). The items below are
> the genuinely pending ones, prioritized by impact.

---

## Priorities in summary

| # | Item | Impact | Effort | Risk |
|---|------|---------|---------|-------|
| 1 | `derive_state` layer (v0.2) | High — closes the real roadmap gap | High (2-3 sessions) | Medium |
| 2 | Migrate 5 `Result<_, String>` | Medium — consistency + safety | Low (1 session) | Low |
| 3 | Test coverage (4 crates) | High — governance/MCP without tests | High (3-4 sessions) | Low |
| 5 | Physical consolidation of ADRs | Low — cosmetic | Low (1 session) | Low |
| 6 | Granular parsers (B5) | Medium — actionable errors | Medium (1 session) | Low |

---

## Handoff to the next agent (Commit 3.1 — governance: close gaps)

> **Read this first.** This is the current, self-contained task, with
> everything a fresh agent needs to continue without rediscovering what this
> session already mapped.

### Session history: Commit 2.4 ✅ LANDED (Phase 2 complete)

Commit 2.4 covered the 6 functions of `tuf.rs` (1 `pub(crate)` + 5 private)
with **41 inline tests**, zero production churn. Details in the Phase 2
section below. **Phase 2 (`forge-core-crypto`) is 100% complete** — 4 commits,
119 lib tests + 7 zeroize_smoke + 1 KAT ignored. The next agent picks up
Phase 3.

### Next task: Commit 3.1 — close gaps in `forge-core-governance`

**Target crate:** `crates/forge-core-governance` (1447 LOC, 5 files in `src/`).

**⚠️ Reconnaissance surprise:** the original roadmap said governance had
"zero tests". **THAT WAS WRONG.** The crate **already has 22 `#[test]` + 2
`proptest!` inline** (44% of `lib.rs` LOC is tests). The `record`/
`arbitrate`/`escalate` PEPs have happy-path + gate + double-resolve covered, and
`replay`/`project`/`apply` have good coverage. **Commit 3.1 is NOT writing
tests from scratch — it is closing specific gaps** that reconnaissance
identified.

**Gaps to cover (prioritized):**

| # | Gap | File | Why |
|---|--------|---------|---------|
| 1 | `list(root, filter)` without test | `lib.rs:321` | Public, with no coverage. Test filter `None` vs `Some(ConflictResolutionState::*)` |
| 2 | `project` cold-read without direct test | `lib.rs:302` | Only exercised via PEPs. Test it isolated: lock + replay |
| 3 | `StoreError(...)` paths of the 3 PEPs | `arbitrate.rs`/`escalate.rs`/`record.rs` | No test forces `RecordError`/`ArbitrateError`/`EscalateError`. Hard without injecting fs failure — can skip or use a nonexistent root |
| 4 | `_with_durability` variants not called | `*.rs:71-85` | The 3 explicit `*_with_durability` PEPs never tested. Test with `WalDurability::default()` + explicit value |
| 5 | `EventSourced` trait methods on `GovernanceDomain` | `lib.rs:186-266` | `apply`/`record_diagnostic`/`sequence_of`/`advance_sequence`/diagnostics — some covered via replay, but not isolated |

**PEP pattern (important — DIFFERS from crypto):**

- The 3 PEPs **do NOT return `Result`, do NOT accumulate in `Vec<String>`**.
  They return a struct (`RecordResult`/`ArbitrateResult`/`EscalateResult`)
  carrying a `status: <Foo>Status` enum. Storage errors are a
  **variant of the status enum** (`StoreError(RecordError)`), not `Err`.
- `RecordStatus` variants: `Recorded{sequence}`, `AlreadyRecorded`,
  `StoreError(RecordError)` (`record.rs:33`).
- `ArbitrateStatus`: `Resolved{sequence}`, `DeniedByGate`,
  `ConflictNotFound`, `NotPending`, `StoreError(ArbitrateError)`
  (`arbitrate.rs:32`).
- `EscalateStatus`: `Escalated{sequence}`, `DeniedByGate`,
  `ConflictNotFound`, `NotPending`, `StoreError(EscalateError)`
  (`escalate.rs:28`).
- `project`/`list` **do** return `Result` (`ProjectionResult`,
  `lib.rs:282`).
- Tests do `match` on `result.status` and assert the variant.
  Mirror the existing pattern (see `arbitrate.rs:222`).

**Errors:** governance **does not define its own error enums**. `error.rs:27-44`
defines 4 type aliases all `= forge_core_eventlog::EventLogError<ArbitrationProjectionDiagnostic>`.
The real errors live in `forge-core-eventlog`. The project convention (no
`anyhow`/`thiserror`) is followed.

**Cargo.toml:** `[dev-dependencies]` only has `proptest` (already in use). For
fs tests that need a temp dir, **do NOT add `tempfile`** — existing tests use
`forge-core-store` helpers or `std::env::temp_dir()`. Mirror the pattern of the
tests already present in the crate (see how `arbitrate.rs:222` creates the
`root`). **No `chrono`, no `rcgen`, no `rasn`.**

**Caller:** the only caller is `crates/forge-core-cli/src/governance_cmd.rs:29`
(4 subcommands: record/conflicts/arbitrate/escalate). No use in the kernel.

**Acceptance gate (Commit 3.1):**

- `cargo test -p forge-core-governance` green.
- Clippy pedantic + fmt clean (auto via the `pi-green-loop` hook).
- Gaps #1 (`list`) and #4 (`_with_durability`) **mandatory**. #2 (`project`)
  recommended. #3 (`StoreError`) optional if injecting fs failure is costly.
- Zero production churn (only `#[cfg(test)]`).

**Design decisions already made (do NOT reconsider):**

- **Do not create new error enums** — governance delegates to the eventlog.
- **Keep the struct-result + status enum pattern** of the PEPs.
- **Visibility:** all PEPs and `project`/`list` are `pub` → inline test
  OR in `tests/`. Existing tests are inline (`mod tests` in each file) —
  mirror that.

**Repo conventions to respect** (in `AGENTS.md`, always loaded):

- **No `anyhow`/`thiserror`.**
- **Editor stability (WSL+Windows+r-a):** never two cargos in parallel.
- **Context hygiene:** one story per session. Commit 3.1 = one session.
- **Commits:** the user commits explicitly when asked.

**After 3.1 (Phase 3 continues):**

- 3.2 — `forge-core-eval-harness` (decide baseline vs candidate, ADR-0023)
- 3.3 — `forge-core-research` (admission/graph; `proptest` already dev-dep)
- 3.4 — `forge-core-eventlog` (EventSourced trait mechanics)
- 3.5 — `forge-core-eval` / `forge-core-trace` (low risk)

---

## 1. `derive_state` layer (the real v0.2 gap) — ✅ LANDED

**Status:** completed in 3 commits (`f94eac45`, `d8a36c1d`, `d8a36c1d`+tests).

**What landed:**
- `crates/forge-core-store/src/derive_state.rs` — the single authority
  constructor for claim state. It wraps the already-existing projection
  (`replay_claim_wal`) and incorporates the torn-tail auto-repair dance that
  lived inline in `claim.rs`.
- `load_claims()` in `claim.rs` now routes through `derive_state` internally
  (zero churn across the 7 call sites: acquire/heartbeat/release/handoff/status/
  reconcile/check-write + graph_cmd.rs migrated transparently).
- `forge-core claim status --from-cache` added (debug/diagnostic, reads the
  legacy YAML; spec AC5).
- 5 new tests prove the ACs: tamper-fail-closed (ac1/ac4),
  cache-mutation-inert (ac7), from-cache-flag (ac5).
- The entire regression net green: 66 store + 204 CLI lib + 22 claim E2E.

**What did NOT land (optional follow-up):**
- Snapshot/rotation as a read cache (P3.3, "later perf layer" in the spec).
- Opaque `ClaimState` type with compile-time seal (defense-in-depth, option b).

---

## 1-OLD (historical archive — replaced by ✅ above)

**Context.** Today the coordination state is reconstructed by reading the claim
YAMLs on every invocation (`load_claims()` in `claim.rs:823`). The WAL
(`.forge-method/wal/claims.fmw1`) is already the authority for mutation, but
the read path still does a full replay on every call. The spec
`contracts/spec/claims-integrity-spine-spec.yaml:56` requires a
`crates/forge-core-store/src/derive_state.rs` as the **single state
constructor** — it **does not exist**.

---

## 2. Migrate `Result<_, String>` (AGENTS.md mandates it) — ✅ LANDED (5/5)

All 5 sites migrated to typed enums (Debug, Clone, PartialEq, Eq):

1. ~~`store/lib.rs` `parse_effect_wal_records_for_recovery`~~ → `EffectWalRecoveryParseError`
2. ~~`cli/mcp_cmd.rs` `parse_serve_args`~~ → `ServeArgsError`
3. ~~`cli/research_cmd.rs` `load_evidence`~~ → `EvidenceLoadError`
4. ~~`protocol-mcp/attestation.rs` `hex_decode`~~ → `HexDecodeError`
5. ~~`protocol-mcp/server.rs` `extract_attestation`~~ → `AttestationExtractError`

**Zero `Result<_, String>` in `crates/*/src/`** (grep confirms). Acceptance gate
met: clippy pedantic green, tests green.

---

## 3. Test coverage — 4 crates without tests

**Context.** The spine is well tested (store, validate, decisions, kernel,
cli have E2E + unit suites). The initial audit said 4 crates had zero tests,
but that was **wrong for MCP** — it already had ~33 inline tests. The real MCP
gap was specific attack vectors not covered.

| Crate | LOC | Risk | Status |
|-------|-----|-------|--------|
| `forge-core-crypto` | 5812 | **P0 — security-critical** | ✅ **Phase 2 complete** (4 commits: ed25519/p256/rekor/OCSP/TUF; 119 lib + 7 smoke + 1 KAT) |
| `forge-core-protocol-mcp` | 2016 | High | ✅ **Attestation gaps closed** (44 tests) |
| `forge-core-governance` | 1447 | High | Pending — arbitrate/escalate/record without proof |
| `forge-core-eval-harness` | 1371 | High | Pending — decides baseline vs candidate (ADR-0023) |
| `forge-core-research` | 1025 | Medium | Pending — admission/graph; `proptest` dev-dep but 0 tests |
| `forge-core-eventlog` | 1132 | Medium | Pending — EventSourced trait mechanics |
| `forge-core-eval` | 890 | Low | Pending — contract types |
| `forge-core-trace` | 479 | Low | Pending — trivial |

### Phase 2 — `forge-core-crypto` (P0, top priority) — Commits 2.1-2.2 ✅ LANDED

The highest-risk crate: 5812 LOC of cryptographic verification with essentially
zero coverage. A bug here is silent and catastrophic. Broad coverage per commit:

- **Commit 2.1 — ed25519/p256 signature verification.** ✅ LANDED (14
  tests). Round-trip sign→verify (Ok), tampered sig→verify (Invalid),
  wrong key→verify (Invalid). Deterministic KAT with a fixed seed pinning the
  verifying key + ed25519 signature (mirrors the MCP pattern
  `attestation.rs:568`). p256 bundle + DSSE verify tested end-to-end with the
  signing key extracted from the test certificate (rcgen bridge
  `KeyPair::serialize_der()` → `p256::ecdsa::SigningKey::from_pkcs8_der`).
  Coverage of the 3 sites: `verify_ed25519_signature`,
  `verify_bundle_signature_with_certificate`,
  `verify_dsse_signature_with_certificate`. Proptest sign/verify+tamper
  on both algorithms. `cargo test -p forge-core-crypto` green (14 lib
  + 7 zeroize_smoke), clippy pedantic clean.
- **Commit 2.2 — rekor log entry parse + inclusion proof verify.** ✅
  LANDED (30 lib tests + 1 KAT regenerator `#[ignore]`d). Direct coverage
  of the 4 entrypoints of `rekor.rs` (397 LOC), all inline `#[cfg(test)]`
  (the 2 `pub(crate)` require it):
  - `parse_rekor_log_entry` — happy path + each variant of
    `RekorParseError` (8 variants: invalid JSON, invalid base64 body,
    non-JSON body, missing `verification`/`inclusionProof`/`hashes`,
    non-string hash, and each `MissingField` via field removal by path).
  - `parse_signed_checkpoint` — happy-path KAT (pins `tree_size` + root
    hash), note lines extension, and 6 format-error variants
    (`CheckpointFormatInvalid`, `NoteInvalid`, `OriginMissing`,
    `TreeSizeInvalid`, `RootHashBase64Invalid`).
  - `verify_rekor_checkpoint` — Ok + 4 variants (`TreeSizeMismatch`,
    `RootHashMismatch`, `SignatureMissing`, `SignatureInvalid` via wrong
    key). The p256 KAT pins the verifying key sec1-hex derived from the seed
    `[8u8;32]` (regenerator `#[ignore]`d).
  - `verify_merkle_inclusion` — tree_size=1 trivial match/mismatch,
    tree_size=0 / log_index≥tree_size reject, 2-leaf tree (both
    indices), 4-leaf tree (all indices + tamper + malformed hash),
    proptest over random 4-leaf trees (fail-closed for impostor
    leaf and wrong root).
  - Plus: regression guard of `RekorParseError::display()` (legacy strings).
  Zero production churn (+752 LOC, only `#[cfg(test)]`). `cargo test -p
  forge-core-crypto` green (44 lib + 7 zeroize_smoke + 1 ignored KAT),
  clippy pedantic clean, fmt clean. Workspace: 1 pre-existing failure
  (`operation_sidecar_e2e::execute_operation_rejects_outside_root_operation_path_before_read`)
  already fails at `b46d0bf2` — not a regression from this commit.
- **Commit 2.3 — OCSP helpers: direct unit coverage of the `pub(crate)`.**
  ✅ LANDED (34 inline tests). The crate already had complete E2E coverage of
  the public OCSP entrypoint (17 integration tests in `validate.rs` covering
  good/revoked/unknown/expired/future/nonce/sig/responder-mismatch via signed
  rcgen DER). The gap was direct unit coverage of the 11 `pub(crate)` helpers
  of `ocsp.rs` — only exercised indirectly. Coverage by constructing
  `rasn-ocsp` structs in pure Rust (no signed DER):
  - `decode_ocsp_response`/`decode_basic_ocsp_response` — round-trip
    (encode→decode) + invalid DER → `None` + reason.
  - `verify_ocsp_single_response_freshness` — valid window, this_update in the
    future, next_update expired, next_update absent.
  - `apply_ocsp_cert_status` — Good/Revoked (revoked_at + reason Debug)/Unknown.
  - `extract_ocsp_response_nonce_hex` — nonce present (double-wrapped
    OCTET STRING), extensions absent, non-nonce OID.
  - `verify_ocsp_nonce` — match/mismatch/missing/present-without-expectation/
    neither-supplied (all 5 branches).
  - `normalize_expected_ocsp_nonce_hex` — lowercase, separators (`:`/`-`/
    space), odd-length, invalid character, empty.
  - `rasn_oid_matches` — match, prefix-only, different arcs.
  - `ocsp_responder_id_matches_issuer` (ByKey) + `find_matching_ocsp_single_response`
    — match, serial mismatch, unsupported hash algorithm (with a real rcgen issuer
    cert).
  - `verify_basic_ocsp_signature_with_issuer` — negative path (synthetic sig;
    happy-path already covered in E2E).
  Added dev-deps `chrono` + `rasn-pkix` (workspace). Zero production churn.
  `cargo test -p forge-core-crypto` green (78 lib + 7 zeroize_smoke
  + 1 ignored KAT), clippy pedantic clean, fmt clean.
- **Commit 2.4 — TUF trusted-root freshness.** ✅ LANDED (41 inline
  tests). Coverage of the 6 functions of `tuf.rs` (207 LOC): 1 `pub(crate)`
  (`verify_tuf_metadata_freshness_role`) + 5 private helpers
  (`parse_tuf_datetime_utc_to_unix`, `parse_fixed_i32`, `days_in_month`,
  `is_leap_year`, `days_from_civil`). The crate already had 6 E2E integration
  tests in `validate.rs` (lines 4576-4742) covering the public entrypoint;
  Commit 2.4 = direct unit coverage of the helpers, focusing on datetime
  parsing edge cases that E2E does not isolate:
  - `verify_tuf_metadata_freshness_role` — fresh (correct evidence),
    expired (expires < update_start), rollback (version < floor),
    version missing, version present without floor, role type mismatch,
    expires missing, invalid expires format (partial entry), read failure
    (partial entry, label `tuf_metadata_read_failed`), invalid JSON,
    no `signed` envelope (all fields missing).
  - `parse_tuf_datetime_utc_to_unix` — KATs (epoch=0, 2030-01-01=
    1893456000, 2020-01-01T12:30:45Z=1577881845, pre-epoch=-1), rejection
    of wrong length, missing Z, wrong separators, non-numeric,
    month 0/13, day out of month (incl. feb-29 in a common year), feb-29 in a
    leap year (2024), H/M/S overflow, reason with correct role-scope.
  - `parse_fixed_i32` — decimal, non-digit, out-of-range, negative.
  - `days_in_month` — 31/30-day months, feb common/leap (1900 non-leap,
    2000 leap), invalid month = 0.
  - `is_leap_year` — common div-by-4, century non-div-by-400 (1900/2100),
    century div-by-400 (1600/2000).
  - `days_from_civil` — KAT table of 10 dates (epoch, pre-epoch, 1900,
    2000 leap day, 2024 leap day, 2030 root date) + full-year spans
    (365 common, 366 leap).
  Calendar KATs computed independently (Python `datetime`) and
  pinned as regression guards. ScopedTempDir RAII for fs fixtures
  (no `tempfile` dev-dep). Zero production churn (+~500 LOC, only
  `#[cfg(test)]`). `cargo test -p forge-core-crypto` green (119 lib +
  7 zeroize_smoke + 1 ignored KAT), clippy pedantic clean, fmt clean.
  **Phase 2 complete.**

Each commit: `cargo test -p forge-core-crypto` passing.

### Phase 3 — crates without tests (FOLLOWING sessions, order by risk)

1. `forge-core-governance` (arbitrate/escalate/record)
2. `forge-core-eval-harness` (decide baseline vs candidate)
3. `forge-core-research` (admission/graph; `proptest` dev-dep available)
4. `forge-core-eventlog` (EventSourced trait mechanics)
5. `forge-core-eval` / `forge-core-trace` (low risk)

### `forge-core-protocol-mcp` — ✅ LANDED (partial)

The attestation/authorization gaps were closed (3 commits, session
after derive_state):
- 7 new tests: RequireAll gate, present-but-invalid on read-only
  (defense-in-depth), malformed `_meta.attestation`, unauthorized-key
  pin of the documented contract, proptest sign/verify+tamper.
- Deterministic KAT (fixed seed) pinning canonical bytes + ed25519
  signature — catches canonicalization regressions that were flaky on OsRng.
- `hex_decode` migrated from `Result<_, String>` to typed `HexDecodeError`
  (also closes item #2 partially for the MCP crate).

**What did NOT land:** allowlist has 11 tests (good coverage); server.rs
has 17 tests (gate covered). The `run_stdio` live loop stays implicit.

**Acceptance gate.** Each crate has ≥1 E2E test + unit coverage on
critical paths; `cargo test -p <crate>` green.

**Estimate.** Phase 2: 1-2 sessions. Phase 3: 4-5 sessions (1 per crate).

---

## 5. Physical consolidation of the ADRs

**Context.** The numbering collision was resolved (Phase 3 landed: Registry A
in `docs/adr/` 0022-0024, Registry B in `docs/dev-docs/.../adrs/` 0001-0014,
documented in `docs/adr/README.md`). But the 14 Registry B ADRs are physically
separate from the 3 of Registry A. Moving all of them to `docs/adr/`
unifies the registry physically.

**File.** `git mv docs/dev-docs/forge-method-core-dev-docs-v2/adrs/*.md docs/adr/`

**Risk.** Low. ADRs are cited by number (not by path) in code.
But verify: grep for `dev-docs/.../adrs/` in `crates/` and `contracts/` —
if there are path refs, update them.

**Acceptance gate.** All ADRs in `docs/adr/`; `docs/adr/README.md`
updated to reflect a single location; no broken path ref.

**Estimate.** 1 session (cosmetic).

---

## 6. Granular parsers (B5 of the CLI/UX plan)

**Context.** The 10+ `parse_*_or_err` helpers in
`crates/forge-core-cli/src/cli_util.rs` (lines 196-405) return the 10KB usage
dump for an invalid enum value. E.g.: `--target-kind foo` returns the entire
usage instead of `"unknown --target-kind 'foo'; expected
file_path|glob|state_key|..."`.

**File.** `crates/forge-core-cli/src/cli_util.rs`.

**Approach.** For each enum parser, list the valid variants in the error
message. The generic helper `parse_strict_or_err` (line 405) already does
this partially — mirror the pattern.

**Acceptance gate.** Each invalid enum shows the valid options; no
parser returns the global usage dump for a single-value error.

**Estimate.** 1 session.

---

## Non-items (decisions made, do not reconsider)

- **First-use skill wiring** is out of scope for this repo. The `SKILL.md` Step 0
  handles repos already linked via `project resolve`; bootstrapping a repo without
  a link (running `forge-core start` and following the `next_step`) is the
  responsibility of the host/operator that invokes the skill, not the skill itself. The `start` command emits
  `next_step.argv` as the agent/host execution contract and `next_step.command`
  as display-only text for humans.
- **110 workflows** stay. Each product uses a handful; the wide catalog is
  intentional (it serves a broader range of products).
- **Repo URLs** (DanielCarva1/forge-method-core is the canonical in the README and
  SKILL since the distribution migration; Stable-Studio/forge-method-rust is
  the org's historical mirror).
- **The claims WAL** is append-only by design (audit log). Do not truncate.

## History

### Session Phase 2 / Commit 2.4 (TUF trusted-root freshness tests)

Last commit of Phase 2. Covered the 6 functions of `tuf.rs` (207 LOC, zero
inline tests before) with 41 tests, zero production churn.

**Technical finding:** the real label of `read_required_file` on the read-failure
path is the literal `"tuf_metadata"` (not `"tuf_root"`) — the reason
produced is `tuf_metadata_read_failed:...`. The role-scoped reasons only
appear after successful parse. A read-failure test must assert
against `tuf_metadata_read_failed`, not `tuf_{role}_read_failed`.

**Approach:** calendar KAT table (10 dates) computed
independently via Python `datetime` and pinned as a regression guard of the
`days_from_civil` algorithm. Homemade ScopedTempDir RAII (no `tempfile`
dev-dep) for fs fixtures, isolating each test with
`forge-tuf-test-<label>-<pid>` and cleaning up on `Drop`.

Gate: `cargo test -p forge-core-crypto` green (119 lib + 7 zeroize_smoke
+ 1 ignored KAT), clippy pedantic clean, fmt clean. **Phase 2 complete.**

### Session Phase 2 / Commit 2.1 (ed25519/p256 signature tests) — `21f0840d`

Broke the zero coverage of `forge-core-crypto` at the 3 signature verification
sites. 14 new tests, zero production churn:

- **`slsa_transparency.rs`** (ed25519, 7 tests): round-trip Ok, tampered
  signature, tampered message, wrong key, malformed lengths, deterministic KAT
  (seed `[7u8;32]`, pin verifying key + signature in hex),
  proptest sign/verify+tamper.
- **`sigstore.rs`** (p256 ECDSA, 7 tests): bundle + DSSE verify
  end-to-end with the signing key extracted from the test cert via the rcgen
  bridge `KeyPair::serialize_der()` (PKCS#8) →
  `p256::ecdsa::SigningKey::from_pkcs8_der`. Round-trip Ok, tampered DER,
  wrong-message, single-byte digest mutation, DSSE tampered payload,
  proptest.

**Technical finding:** `validate.rs` (CLI E2E) signed with
`P256SigningKey::from_slice(&[8u8;32])` *unrelated* to the certificate's public
key — the unit tests now cover the real path where the keys match.

**Contract finding:** `verify_ed25519_signature` only promises
fail-closed on *structural* errors (key/sig length). Degenerate keys
(all-zero) encode a valid point in ed25519 and are NOT rejected —
tested and documented honestly in the `ed25519_malformed_*` test.

Gate: `cargo test -p forge-core-crypto` green (14 lib + 7 zeroize_smoke),
clippy pedantic clean, fmt clean.

### Original session (Phases A–D)

See `git log` between `1ebcdc06` (Phase A) and `d9dbe1d9` (Phase C). The 4 phases
landed:
- **A:** 89 orphan claims cleaned + AGENTS.md handoff removed + 12 pointers
  repaired.
- **B:** CLI/UX — `--version`, no-args→help, `--help` framing, `start`
  no_link guidance, unknown-command diagnosis, `--no-sync` stderr in JSON.
- **C:** README MCP corrected, VERSION aligned, inventory rewritten,
  `--json` consistency, SKILL URL.
- **D:** this document.
