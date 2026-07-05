# ADR-0007 - Multi-principal governance

- **Status**: Accepted (2026-07-01; expanded from `proposto`)

## Context

Agents from different people, orgs, or vendors may work on the same shared
state. Lane claims are not enough when there are different principals — they
serialize writes by path, but they do not model *who* declares the intent, nor
do they make the conflict a structured object (the NFR of F07: "conflict
becomes a structured object, not a silent merge").

This ADR formalizes the design of F07 (expanding the original stub). Three
parallel research fronts (RBAC/ReBAC/Cedar/Zanzibar governance models; the
conflict detection seam in the codebase; and the R8 question `PrincipalId` vs
`StableId`) converge on a three-layer architecture and resolve a contradiction
with ADR 0023.

## Decision

### 1. Three-layer model (GaaS-style format)

The decisive finding: **RBAC, ReBAC, ABAC, Cedar, and Zanzibar all answer a
single-principal question** ("Can P do A on R?") and **have no containment
semantics**. The F07 requirement ("two principals with overlapping intents →
emit a structured ConflictContract, never a silent merge") is a
**coordination** problem, not an authorization problem. Conflating the two is
the central anti-pattern.

- **Authorization layer (ReBAC/Cedar):** `GovernancePolicy` + `PrincipalId`
  model *who the principal is* and *what authority it has*. Cedar
  `(principal, action, resource) → Decision` is the PDP (consistent with
  ADR 0023/0024). This layer does **not** detect A↔B conflict.
- **Coordination layer (Gray's intent-locks):** `IntentContract` = an
  intent-lock over an authority scope (subtree of paths) with **expiration**
  (lease). Conflict = overlap, detected by the lock compatibility matrix.
  Precedents: Gray 1976 (multiple-granularity intent locks); Calvin
  (deterministic ordering); Spanner (bounded temporal). The `expires_at` field
  is **load-bearing** (liveness/correctness), not optional.
- **Conflict layer (first-class object, NOT silent merge):**
  `ConflictContract` is a **first-class entity** (refs of the two intents +
  contested scope + reason + resolution state). The literature is decisive:
  systems that resolve silently (CRDTs, OT, Figma LWW, XACML combining
  algorithms, Zanzibar per-tuple LWW) **destroy the conflict signal**; systems
  that match the F07 requirement (Git markers, Apel semistructured merge,
  Berenson anomalies) make the conflict a **named, typed object that stops the
  flow**. F07 is in the Git/Apel/Berenson lineage.

No agent governance pattern covers resource containment yet (MAST NeurIPS
2025 arXiv:2503.13657; GaaS arXiv:2508.18765 are 2025-26, still forming) —
the design is research-grounded, not standards-compliant.

### 2. Typed `PrincipalId` (supersedes the ADR 0023 prediction)

ADR 0023 (Accepted) states "F07 does not introduce a rival PrincipalId type"
— a *prediction* about F07 made to decide the F06 question. The F07 spec
(`01_feature_specs.md:215`) and this ADR contradict it.

**Decision: introduce `PrincipalId`** as a distinct newtype
(`pub struct PrincipalId(pub String)`, `#[serde(transparent)]`, same derives
as `ScopeId`/`ClaimId`). R8 justification: the F07 authorization structures
(`IntentContract { principal, authority_scope }`, `ConflictContract { principal_a,
principal_b }`, `is_authorized(principal, resource)`) put a principal id and a
resource id in the same comparison, where a field/argument swap is a silent
security bug — exactly the class that the `ScopeId`/`ClaimId` split made
unrepresentable. A distinct `PrincipalId` turns that swap into a compile error.
The industrial precedents that ADR 0023 itself cites (AWS Cedar,
Google Zanzibar) impose typed Principal/Resource separation for the same
reason.

This **formally supersedes the ADR 0023 prediction** (a dated decision record,
not a silent override). The `reviewed_by` field (F06) migrates from
`Option<StableId>` to `Option<PrincipalId>` for consistency
(one-concept-one-type). Since `PrincipalId` is `#[serde(transparent)]`, legacy
YAML (`reviewed_by: principal.daniel`) still parses — zero migration cost, the
proven ScopeId pattern.

**Rejected: type alias** (`type PrincipalId = StableId`). Aliases are
transparent — the compiler treats the two as identical, so
`f(reviewer: PrincipalId)` still accepts a `run_id: StableId`. Zero R8
protection, an illusion of distinction.

### 3. Conflict detection seam (for F07.4)

Detection lives **in the claim engine acquire**
(`crates/forge-core-decisions/src/claim_engine.rs:317`, called from
`claim.rs:295`). Two principals with overlapping intents on repo-paths are
**already blocked there** (`PathAlreadyClaimed`/`AlreadyClaimedByOther`) —
F07.4 merely reformulates that flat rejection into a structured
`ConflictContract`, reusing the assignment data that acquire already computes
(`holder`, `blocking_claim_id`, `expires_at`, overlapping path). The WAL layer
(`claim_wal.rs`) remains a dumb serializer — no policy in the IO. Memory
writes are a *separate* capability-governance gap (the deferred `memory review`
verb), not a path conflict.

## Consequences

- Conflicts become structured objects (`ConflictContract`), not silent manual
  merges. The F07 NFR is satisfied at the schema layer.
- Silent overwrite is blocked by design (`ConflictPolicy::EmitContract`
  is the default; `SilentLastWriterWins` produces a validation warning).
- Human arbitration becomes auditable (`ConflictResolutionState::{Pending, Resolved,
  Escalated}` + append-only ledger in F07.5).
- Forge becomes a differentiated layer for shared agentic state —
  research-grounded, aligned with the emerging 2025-26 direction (MAST, GaaS).
- The typed `PrincipalId` makes the principal↔resource bug class
  unrepresentable at compile time (R8).
- `reviewed_by` migrates to `PrincipalId` at no cost (serde-transparent); the
  `memory review` verb (F06, deferred) is conceptually unblocked — it now only
  depends on F07.4 (governance wiring) and F07.6 (CLI).

## Scope of this story (F07.1-F07.3)

- ✅ F07.1: this ADR (Accepted; supersedes the ADR 0023 prediction).
- ✅ F07.2: `PrincipalId` newtype in `common.rs`; migration of `reviewed_by`.
- ✅ F07.3: `governance.rs` (`GovernancePolicy`, `IntentContract`,
  `ConflictContract` + enums) + validator with typed diagnostics + fixtures.
- ⏳ F07.4: wire the `ConflictContract` into `claim_engine.rs:317`.
- ⏳ F07.5: arbitration ledger (append-only).
- ⏳ F07.6: CLI `forge-core governance intent/conflicts/arbitrate`.
- ⏳ F07.7: fixtures + E2E (2 principals disputing → ConflictContract emitted).

## References

- Gray 1976 — Granularity of Locks (intent locks, MGL):
  https://www.cs.cmu.edu/~natassa/courses/15-721/papers/GrayLocks.pdf
- Sandhu RBAC96 (1996); NIST RBAC (2000):
  http://www.cs.toronto.edu/~jm/2507S/Readings/13.Sandhu96.pdf
- Berenson et al. 1995 — A Critique of ANSI SQL Isolation Levels (anomalies
  as named phenomena): https://doi.org/10.1145/568271.223785
- Calvin (Thomson et al. 2012):
  https://cs.yale.edu/homes/thomson/publications/calvin-sigmod12.pdf
- Zanzibar (Pang et al. 2019):
  https://www.usenix.org/system/files/atc19-pang.pdf
- Spanner (TrueTime / commit-wait):
  https://docs.cloud.google.com/spanner/docs/true-time-external-consistency
- Apel et al. — Semistructured merge:
  https://www.se.cs.uni-saarland.de/publications/docs/CBS%252B19.pdf
- XACML 3.0 (combining algorithms = silent resolution, the anti-pattern):
  https://docs.oasis-open.org/xacml/3.0/xacml-3.0-core-spec-cd-03-en.html
- Cedar (arXiv 2403.04651, 2024): https://arxiv.org/abs/2403.04651
- MAST (arXiv 2503.13657, NeurIPS 2025 — multi-agent failure modes):
  https://arxiv.org/pdf/2503.13657
- GaaS (arXiv 2508.18765, 2025): https://arxiv.org/abs/2508.18765
- Tian Pan — Conflict Resolution Patterns for Parallel AI Systems (2026):
  https://tianpan.co/blog/2026-05-02-multi-agent-conflict-resolution-disagreement-patterns
- In-repo: ADR 0023 (memory trust model — superseded prediction); ADR 0024
  (PDP/PEP); `common.rs` (R8 + `ScopeId`/`ClaimId` precedent);
  `claim_engine.rs:317` (the F07.4 seam);
  `conflict_detection.rs:253` (`repo_paths_overlap`, the reusable primitive).
