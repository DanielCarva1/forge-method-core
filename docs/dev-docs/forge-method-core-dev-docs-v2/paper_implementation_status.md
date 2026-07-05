# Paper Implementation Status

**Date**: 2026-06-30
**Scope**: each of the 15 papers in `contracts/research/` mapped to its implementation status in the Forge Method core codebase. Eastern (China/Korea/Japan) and Western representation tracked per `AGENTS.md` ("Search non-Western and Chinese-origin work when the domain is active there") and the `policy.geographic_coverage.rule` in `contracts/research/field-evidence-20260625.yaml`.

> Icon convention: ✅ Implemented (a concrete crate/file is committed) · 🟡 Partial (plumbing exists but there are gaps) · ❌ Pending (no reflection in code) · 🚫 Decided against (explicitly rejected).

## Summary

| Region | Papers | Implemented | Partial | Pending |
|---|---|---|---|---|
| Western | 8 | 4 | 4 | 0 |
| Oriental (CN/KR/JP) | 0 | 0 | 0 | 0 |
| Mixed (Western + Oriental) | 5 | 1 | 4 | 0 |
| Unspecified (internal audits) | 2 | 0 | 2 | 0 |
| **Total** | **15** | **6** | **9** | **0** |

Initial note: **no paper is purely Oriental**. Eastern sources live *inside* the 5 Mixed papers (notably `field-evidence-20260625` with ChatDev/AgentScope/Fudan/Tsinghua, `community-trends` F8 V2EX/Juejin/InfoQ.cn, `best-features` F7 Alibaba/TRAE/Qwen3, `protocol-scale` F7 Qwen-Agent/MegaAgent). See the *Regional representation audit* section for the impact of this asymmetry.

## Per-paper status

### `agentic-throughput-and-fast-quality-mode-v1.yaml`

- **Topic**: Taxonomy of throughput bottlenecks in MAS + design of two tracks (fast vs quality).
- **Region**: Western.
- **Key findings**: 13 findings (F1–F13); sources: 17.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-kernel/`, `crates/forge-core-decisions/phase_transition.rs`, `../Forge-method-archive/dev-journals/r6_benchmarks.md` (R6.1+R6.2).
- **Evidence**: there is a runtime with phase transition and established R6.1/R6.2 benchmarks (a measurable baseline); the notion of a "fast track" approximates the preview/ready/gate separation in `forge-core-trace::TraceEventKind` (`PreviewCompleted`, `ReadyCompleted`, `GatePassed`).
- **Gap**: F1/F2 (vendor case studies) are not actionable; F7/F8 (per-model concurrency limits) still have no codified policy; the throughput eval bank (F13) is partial — pending F05/R9.

### `best-features-from-papers-and-cases-v1.yaml`

- **Topic**: FEAT catalog of the best missing features for Forge, synthesizing literature + cases.
- **Region**: Mixed (F7 brings Eastern sources: Alibaba/TRAE/Qwen3).
- **Key findings**: 8 findings + FEAT catalog; sources: 24.
- **Implementation status**: 🟡 Partial.
- **Where**: depends on the FEAT item — `crates/forge-core-crypto/` (FEAT signature/transparency), `crates/forge-core-decisions/` (FEAT isolation/claim), pending F05–F14.
- **Evidence**: cryptography and isolation FEATs have concrete reflection (`forge-core-crypto` with rekor/ocsp/sigstore/tuf/slsa_transparency; `forge-core-decisions/isolation.rs`). FEAT-03 (self-evolving tools) and FEAT-04 (shared-state coordination) map to the pending F08 (MCP) and F09 (A2A).
- **Gap**: most of the FEAT catalog is *aspirational* — it corresponds exactly to the F08–F14 features not yet delivered in `excellence_roadmap.md`.

### `cli-llm-first-design.yaml`

- **Topic**: Machine-first CLI principles (deterministic JSON, typed contracts, structured output).
- **Region**: Western.
- **Key findings**: 19 findings (F1–F19); sources: 28.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-cli/`, `crates/forge-core-contracts/`, `crates/forge-core-schema/`, `../Forge-method-archive/dev-journals/r7_yaml_serde.md`.
- **Evidence**: the CLI exists with typed contracts in `forge-core-contracts` and canonical serde validated by R7 (yaml_serde). The `TraceEvent`/`TraceActor` model is deterministic and machine-readable. The "parse-don't-validate" pattern (F9 tau-bench policy) is reflected in the `RepoPath`/`ClaimId`/`StableId` newtypes.
- **Gap**: F8/F10/F11 (public benchmark leaderboards, human narration) are not actionable for a local core. F14–F19 were not audited finding-by-finding.

### `community-trends-and-requested-features-v1.yaml`

- **Topic**: Community trends 2025–2026 and most-requested features.
- **Region**: Mixed (F8 aggregates Chinese sources: V2EX, Juejin, InfoQ.cn).
- **Key findings**: 8 findings + DEM catalog; sources: 28.
- **Implementation status**: 🟡 Partial.
- **Where**: demands map to F06 (memory), F10 (multi-agent ops dashboard), F08 (secure MCP).
- **Evidence**: the paper is mainly a prioritization input — it aligns with the roadmap's Track F. Demand for memory with provenance (DEM-04) and for an ops panel (DEM-01) is recorded as pending.
- **Gap**: no DEM demand has a direct implementation in the core yet; all pending in F06/F08/F10.

### `field-evidence-20260625.yaml`

- **Topic**: Field evidence policy + ~90 tiered sources (T1/T2/T3) with `confirmed_origin`.
- **Region**: Mixed (many confirmed CN origins: Fudan, Tsinghua ChatDev, AgentScope, Alibaba, etc.).
- **Key findings**: sources block + `plan_level_implications`; sources: ~90.
- **Implementation status**: 🟡 Partial.
- **Where**: the policy grounds the geographic coverage rule; `plan_level_implications` cross-references R-tracks and features.
- **Evidence**: the policy is the **canonical foundation** of the *Regional representation audit* section of this document. The plan implications have been partially absorbed (R5 zeroize, R10 crypto crate).
- **Gap**: the paper is meta — its "implementation" is the continuous intentional inclusion of non-Western sources in the other papers. There is no single codepath; the gap is procedural (see regional audit).

### `multi-agent-collaboration-governance-research.yaml`

- **Topic**: Verification that multi-agent governance contracts are real (grite/Limen/preclaim + Cursor/CAID/Devin).
- **Region**: Mixed (cross-regional synthesis of industrial cases).
- **Key findings**: verdict + 4-layer pattern; sources: synthetic.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-decisions/src/conflict_detection.rs`, `crates/forge-core-decisions/src/isolation.rs`, `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-decisions/src/claim_engine.rs`.
- **Evidence**: `conflict_detection.rs` implements exactly the 4-layer pattern — pure classification `WriteCheck::Ok { governed_by_self, ungoverned } | Blocked { blocks }` with `BlockDetail { blocked_path, blocking_claim_id, claimant, conflict_code }`. DD8/DD10/DD19/DD26/DD27/DD28 are codified as documented invariants in the module comments. The WAL materializes the semantic reservation (S4.3 cited in the paper itself).
- **Gap**: multi-principal handoff governance (more than 2 agents) is still partial — see F07.

### `protocol-scale-with-model-v1.yaml`

- **Topic**: Typed contract as amplifier vs tax — when it scales with the model and when it limits.
- **Region**: Mixed (F7: Qwen-Agent, MegaAgent).
- **Key findings**: 8 findings (F1–F8); sources: 18.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-contracts/`, `crates/forge-core-validate/`, `crates/forge-core-decisions/catalog.rs`.
- **Evidence**: the typed contracts (Claim, Effect, Operation) are the core of Forge — `protocol-scale` validates this design choice (F3/F8 "hard gates + freedom within gates"). The validator accumulates `Diagnostic` (no short-circuit) per the paper's philosophy.
- **Gap**: F1/F8 (empirical scale-with-model evidence via benchmarks) is exactly the pending F05/R9 — still no comparative baseline.

### `robustness-observability-multiagent-v1.yaml`

- **Topic**: Robustness/observability for MAS with file-backed WAL.
- **Region**: Western (k8s/ARIES/Postgres/MongoDB/Bazel/LumiMAS).
- **Key findings**: 9 findings (F1–F9); sources: 30.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-trace/src/lib.rs`, `../Forge-method-archive/dev-journals/r6_benchmarks.md`.
- **Evidence**: F1 (ARIES) → `claim_wal.rs` has `ClaimWalRecovery` with `last_good_offset`, `stop_reason: ClaimWalStopReason` (`TruncatedHeader`, `PayloadChecksumMismatch`, `SequenceGap`, …). F2 (level-triggered reconcile) → operation `ClaimWalOperation::ReconcileStatus` (record type 7). F5 (observability) → `TraceEvent` with `TraceActor { principal_id, agent_id, role }` and `TraceEventKind` covering RunStarted/PreviewCompleted/ReadyCompleted/GatePassed/GateBlocked/EffectStaged/EffectApplied. R6.1/R6.2 benchmarks establish the recovery window (DEFAULT_ROTATE_MAX_REPLAY_MILLIS = 250ms).
- **Gap**: F9 (chaos/fault injection in CI) partial — R4 (fuzz via ADR-0008 Linux CI) covers part, but failpoint injection in the claim path is pending.

### `rust-observability-selfhealing-v1.yaml`

- **Topic**: Observability + self-healing layer in Rust.
- **Region**: Western.
- **Key findings**: 10 findings (F1–F10) + crate review; sources: 18.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-trace/src/lib.rs`, `crates/forge-core-store/src/claim_wal.rs`.
- **Evidence**: the observability layer exists (`forge-core-trace` with `TraceRisk { risk_level, destructive }`, `TraceCost { model_calls, tool_calls, estimated_tokens }`, `TraceAuthority { operation_id, capability_ids }`). Basic self-healing via `ClaimWalRecovery::repaired` and `ClaimWalStopReason`.
- **Gap**: reactive self-healing (automatic restart with preserved state, F8–F10) partial; R5.10/R5.11 (zeroize finalization, secret hygiene in tracing) still pending.

### `rust-state-integrity-wal-concurrency-v1.yaml`

- **Topic**: Rust state integrity + WAL + cross-process concurrency.
- **Region**: Western.
- **Key findings**: 8 findings (F1–F8) + crate review; sources: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-store/src/lib.rs`.
- **Evidence**: F4/F8 (ARIES + recovery) → `ClaimWalRecovery` with offsets, `ClaimWalProjection` rebuilds `active_by_claim_id`, `active_claim_ids_by_agent`, `active_claim_ids_by_scope`, `active_claim_ids_by_path`. Header CRC at `HEADER_CRC_OFFSET = 20`. Level-triggered rotation (`ClaimWalRotationReason::{WalSizeBytes, RecordCount, ReplayDurationMillis}`) with `DEFAULT_ROTATE_MAX_WAL_BYTES = 64MB`. Lock/snapshot/manifest/archive paths defined as public constants.
- **Gap**: F1–F3 (comparative crate analysis) was absorption-only — no code to add. F6 (cross-platform FS advisory locks) — no grep confirmed `fs4`/`file_lock`; gap to verify.

### `rust-testing-defenses-v1.yaml`

- **Topic**: Test-based defenses against the R8 circular-oracle bug.
- **Region**: Western.
- **Key findings**: 7 findings (F1–F7); sources: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `../Forge-method-archive/dev-journals/r4_fuzz_plan.md`, tests in `crates/*/tests/`, ADR-0008.
- **Evidence**: R8 was closed by the combination of newtype + proptest + trycmd + fuzz (ADR-0008 Linux CI). The paper is the theoretical foundation of this stack — F1 (parse-don't-validate), F2 (property tests), F3 (snapshot/CLI golden), F4 (fuzz), F5 (mutation), F6 (invariants), F7 (fixtures).
- **Gap**: F5 (mutation testing) is not in CI; F6 (formal invariants) partial.

### `selfhealing-failpoint-audit-v1.yaml`

- **Topic**: Audit of fail-points/panics in the claim path.
- **Region**: Unspecified (internal audit).
- **Key findings**: 15 occurrences (does not use `key_findings`; uses `occurrences`).
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-decisions/src/claim_engine.rs`.
- **Evidence**: the occurrences were triaged; the critical ones addressed via typed `ClaimWalStopReason` (not panic) and `Result` with named enums per `AGENTS.md`. The WAL now fail-closes on unknown record type (`FLAG_SKIPPABLE_UNKNOWN`, `from_record_type` returns `None`).
- **Gap**: active failpoint injection (simulate disk full, partial write, crash mid-append) still pending — correlates with R4 fuzz and F11 (risk audit).

### `selfhealing-wal-crc-design-v1.yaml`

- **Topic**: Design of the WAL binary format with CRC32C.
- **Region**: Western (LevelDB/RocksDB/Postgres/SQLite as reference).
- **Key findings**: 10 design decisions (D1–D10; does not use `key_findings`; uses `design_decisions`).
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs` (constants and structs above).
- **Evidence**: implementation corresponds 1-to-1 with the decisions: 24-byte header with CRC at offset 20 (D1), 4-byte CRC trailer, `ClaimWalCheckpointPayload { snapshot_path, snapshot_crc32c, last_seq_in_snapshot }` (D2), `ClaimWalManifestPayload` with snapshot+archive+checkpoint_seq (D3), `ClaimWalRotationOptions` with three limits (D4), record types 1–7 reserved with fail-closed on unknown (D5), `ClaimWalSnapshotPayload` with `latest_claims` (D6), `ClaimWalOperation::ReconcileStatus` in type 7 to avoid colliding with 4/5/6 (D7), path `wal/claims.fmw1` + lock/snapshot/archive (D8), `ClaimWalRecovery::last_good_offset` (D9), discriminated `ClaimWalStopReason` (D10).
- **Gap**: no architectural gap — all 10 decisions are materialized. Only production recovery telemetry is pending.

### `selfhealing-writepath-audit-v1.yaml`

- **Topic**: E2E map of the claims read/write path.
- **Region**: Unspecified (internal audit).
- **Key findings**: `io_operations` + `answers` (Q1/Q2/Q3; does not use `key_findings`).
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-decisions/src/conflict_detection.rs`, `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-cli/src/lib.rs`.
- **Evidence**: the paper's Q1/Q2/Q3 (who writes, when, where) are answered in code: the write-set enters `conflict_detection::check_write_set`, is signaled via WAL `Acquire`/`Release`/`Heartbeat`/`HandoffRecorded`, effects staged via `TraceEventKind::EffectStaged` → `EffectApplied`.
- **Gap**: the audit identified corners where the path bifurcates (CLI vs runtime vs validator); not all have end-to-end telemetry.

### `structural-bug-prevention-typelevel-v1.yaml`

- **Topic**: Structural (type-level) prevention of the id coupling that R8 exposed.
- **Region**: Western (parse-don't-validate, proptest, Pact).
- **Key findings**: 8 findings (F1–F8); sources: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-contracts/src/` (newtypes `RepoPath`, `ClaimId`, `StableId`), `crates/forge-core-validate/src/lib.rs`.
- **Evidence**: the paper is the direct foundation of the newtypes used in `conflict_detection.rs` (`blocked_path: RepoPath`, `blocking_claim_id: ClaimId`, `claimant: StableId`) — impossible to pass a raw string where typing requires. `Diagnostic::error/warning` accumulate without short-circuit (compatible with F1 parse-don't-validate).
- **Gap**: F8 (Pact/contract tests between crates) partial — there are tests in `crates/*/tests/` but no formal Pact framework.

## Regional representation audit

`AGENTS.md` requires: *"Search non-Western and Chinese-origin work when the domain is active there."* `field-evidence-20260625.yaml` operationalizes this via `policy.geographic_coverage.rule`. The picture found:

**Where Eastern representation is strong**: the `field-evidence-20260625` paper (Mixed) does this exemplarily — ~90 sources with `confirmed_origin` identifying Fudan, Tsinghua (ChatDev), AgentScope (Alibaba), Qwen, DeepGLM, among others. `community-trends` F8 and `best-features` F7 and `protocol-scale` F7 also bring Eastern sources intentionally.

**Where there is a gap**: **none of the 15 papers is purely Eastern-led**. In domains where Eastern research is particularly active — Chinese coding agents (TRAE, Qwen3-Coder, DeepSeek), MAS infrastructure at scale (MegaAgent, Alibaba LLM-OS), and Chinese evaluation (Tsinghua's AgentBench, Fudan's T-Eval) — coverage appears only *inside* Mixed papers as sub-items (F7/F8), not as standalone papers.

**Recommendation**: for R15/R16, consider dedicated papers on (a) Chinese MAS frameworks in production (Alibaba, ByteDance), (b) Eastern agent benchmarks (AgentBench, T-Eval, ToolBench-CN), (c) Korean/Japanese research (LMArena-JP, LocalLLM-JP). This aligns Track F with the `AGENTS.md` principle and closes the 0/0/0/0 pure-Eastern asymmetry in the table above.

**Honesty**: the absence of Eastern-led papers is not necessarily an implementation deficit — the Mixed papers already import the relevant Eastern conclusions. It is a *documentation coverage* deficit, which this audit records for the backlog.

## Cross-cutting observations

**Convergence: "hard gates + freedom within gates".** This pattern appears explicitly in `protocol-scale` F3/F8, `cli-llm-first` F9 (tau-bench policy), `best-features` F1 (RADAR), and implicitly in `agentic-throughput` F7/F8 and `multi-agent-collaboration` (4-layer pattern). It is the central thesis of Forge's design: strong typed contracts (`ClaimContract`, `OperationContract`, `EffectContract`) that block unsafe operations in the pure engine (`conflict_detection::WriteCheck::Blocked`), but leave the agent free *within* the reserved scope (`WriteCheck::Ok::governed_by_self`). The WAL (`claim_wal.rs`) materializes this boundary: append-only, fail-closed on unknown, recoverable. Three independent papers arriving at the same shape is strong evidence that the architecture is well-grounded.

**The R8 bug class unifies four papers.** `rust-testing-defenses`, `structural-bug-prevention`, `selfhealing-failpoint-audit`, and `selfhealing-writepath-audit` are all responses to the same structural problem — id coupling between parser/validator/engine that creates a circular oracle. The implemented stack (newtypes in `forge-core-contracts` + cumulative `Diagnostic` in `forge-core-validate` + proptest + trycmd + fuzz ADR-0008) is the consolidated answer. This explains why these four papers are among the most "Implemented" — they were the papers *that motivated* the work, not post-facto papers.

**Pending items converge in F05–F14.** The non-actionable or pending findings map clearly to Track F of `excellence_roadmap.md`: (a) `best-features` FEAT-03 → F08 secure MCP; (b) FEAT-04 + `protocol-scale` F8 → F09 A2A; (c) `community-trends` DEM-04 → F06 memory with provenance; (d) DEM-01 → F10 multi-agent control plane; (e) `protocol-scale` F1 + `agentic-throughput` F13 → F05 eval bank (partial) and R9 comparative benchmarks. The Western "audit/ARIES/observability" papers are largely exhausted (6 ✅); the innovation frontier is in the Mixed papers that point to not-started features.

**Non-actionable findings**: `cli-llm-first` F8/F10/F11 (public benchmark leaderboards) and `agentic-throughput` F1/F2 (vendor case studies) have no direct reflection in code and probably never will — they are design inputs, not specifications. Marking them ❌ would be dishonest; they are better classified as "absorbed into the design, with no own codepath".
