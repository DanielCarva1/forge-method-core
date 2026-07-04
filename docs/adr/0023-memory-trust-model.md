# ADR 0023: Two Orthogonal Trust Axes for Agent Memory

## Status

**Accepted** (2026-07-01). The F06.1 grill — including the
`improve-codebase-architecture` deletion-test pass and the external research
sweep (newtype/type-driven-design consensus, serde non-breaking schema
evolution, Rust enum-variant deprecation) — concluded in favour of **Model A,
Opção A** (additive, zero migration cost). This ADR is now authoritative.

**Partially superseded by [ADR 0007](../dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0007-multi-principal-governance.md)**
(2026-07-01): the *prediction* in this ADR that "F07 does not introduce a
rival `PrincipalId` type" is superseded. ADR 0007 introduces a typed
`PrincipalId` newtype for the F07 authorization structures, on the same R8
type-separation grounds this ADR itself cites (Cedar/Zanzibar). **Only that
prediction is superseded**; the Model A/Opção A schema decision, the trust-axis
orthogonality, and the rest of this ADR remain authoritative. See ADR 0007
§2 ("PrincipalId tipado") for the full rationale.

The schema delta below is implemented in `crates/forge-core-contracts/src/memory.rs`
(F06.2). `PrincipalId` references in an earlier draft of this ADR were a
mistake: the house identity type is `StableId` (`common.rs`, R8 discipline).
The decision is recorded, not still open.

## Context

Story F06 (Memory Policy) carries a hard non-functional requirement stated
verbatim in three places (`01_feature_specs.md`, the GitHub issue import, and
`feature_backlog.csv`):

> Nenhuma memória vira authority automaticamente; promote exige policy e
> evidência raw.

The F06.1 design step additionally asks to **sharpen** four terms — Memory,
Fact, Preference, Authority — and to produce an **ADR on admission policy
(hard-to-reverse)**. This is that ADR.

The central design question is how to model trust on a `MemoryDocument`. Two
models were considered:

- **Model B (single axis).** "Approved" already doubles as "authoritative".
  One ladder: raw → approved. The top rung means both "a human looked at it"
  and "the agent may act on it as ground truth".
- **Model A (two orthogonal axes).** Two independent questions:
  1. *Authority* — may the agent treat this as ground truth for autonomous
     action? (`Raw → Provisional → Authority`, gated by policy + raw evidence)
  2. *Review* — has a principal attested to this record's curation?
     (`Unreviewed → Reviewed`, gated by a principal attestation)

  Promotion of authority never implies review, and review never implies
  authority promotion. They answer different questions.

The current F06 design (F06.2 schema in `followups_v0_1_to_10.md`) already
has the authority ladder gated by evidence, but has **no review bit at all**.
So the live choice is: leave it as a single evidence-gated axis (lean B), or
add an orthogonal review axis (A).

## Decision

Adopt **Model A — two orthogonal trust axes.**

The authority ladder stays exactly as F06.2 specifies it (evidence + policy
gated, `Raw → Provisional → Authority`, never auto-promoted). A second,
independent review axis is added and modeled as a **principal attestation**
that reuses the F07 governance ledger rather than a magic
boolean. The concrete identity type is the house `StableId` newtype
(`crates/forge-core-contracts/src/common.rs`); F07 introduces the *concept*
of a principal, but its identity token is `StableId`, not a rival
`PrincipalId` type (R8 discipline: distinct types only when comparison
would be a bug). The two axes compose into a six-cell state space; all six
are meaningful and none collapse.

## Rationale

### 1. Threat model — Model B is the bug these attacks exploit

Memory/retrieval poisoning attacks all hinge on one defect: in an
undifferentiated memory, "present in the store" silently equals "true".
Splitting authority from review is the minimal structural change that lets
the system hold a record as *correctly curated* without endorsing its
*content as fact*, and hold a record as *authoritative by provenance* without
claiming a human reviewed it.

- Greshake et al., arXiv:2302.12173 — indirect prompt injection via retrieval.
- PoisonedRAG — Zou, Geng, Wang, Jia, arXiv:2402.07867 — few malicious texts
  in the RAG corpus induce attacker-chosen target responses.
- AgentPoison — Chen, Xiang, Xiao, Song, Li, arXiv:2407.12784 (NeurIPS 2024) —
  backdoors LLM/RAG agents by poisoning long-term memory or the knowledge base.
- MINJA — Dong et al., arXiv:2503.03704 — injects malicious memory records via
  query-only interaction, no corpus access required.
- MEXTRA — Wang et al., arXiv:2502.13172 (ACL 2025) — extracts private data
  from agent memory under black-box access.

Under Model B, anything that survives the single approval gate becomes
authoritative ground truth. That is precisely the surface AgentPoison and
MINJA scale. Model A keeps a second, independent authority gate, so a record
admitted for retention can be retrieved as context **without** being
actionable as ground truth.

### 2. Novelty — no major system, Western or (2024–2025) Asian, has the ladder

A survey of agent frameworks and Asian-lab research found no system that
operationalizes a `raw → provisional → authoritative` authority boundary
*with review separated from authority*:

- **Qwen-Agent (Alibaba)** — `class Memory(Agent)`; `rag_cfg`, `source`/`content`
  fields give weak document provenance. No trust tier, no admission gate, no
  approval workflow.
- **MetaGPT** — `class Memory(BaseModel)` with `storage: list[Message]` +
  `metadata`. "Verify" appears only as an SOP workflow step, not as a trust
  attribute of memory. `metadata` could carry trust but does not, natively.
- **AgentBench (THUDM/Tsinghua)** — benchmarks tool-use and multi-turn
  reasoning across 8 environments. State persists as `history` and task/env
  state; no evaluation of memory trust or provenance.
- **Memory OS of AI Agent** — Kang, Ji, Zhao, Bai, arXiv:2506.06326
  (Tencent/BUPT, EMNLP 2025) — proposes short/mid/long-term tiers, but they
  are **temporal/functional**, not trust/authority tiers. Closest prior art
  and it still splits the wrong axis.
- **"Memory in the Age of AI Agents"** — survey, arXiv:2512.13564 — flags
  "trustworthiness issues" but proposes no operational authority ladder.
- **DeepSeek** — stateless API; context caching is not trustworthy memory.
- **Kimi / Moonshot** — has a persistent memory tool; no public
  provenance/trust/approval model.

The gap is white space. This is the contribution claim, not just a style
choice.

### 3. Cross-field theory — orthogonality is the correct primitive

Four classic results, each from a different field, converge on separating
orthogonal trust dimensions rather than collapsing them:

- Berenson et al., SIGMOD 1995 — isolation levels as orthogonal guarantees.
- Bell & LaPadula, MITRE 1973 — confidentiality as a lattice axis separate
  from access.
- Sandhu et al., IEEE Computer 1996 (RBAC96) — authorization is distinct from
  identity and from review.
- Buneman, Khanna, Tan, ICDT 2001 — provenance ("why/where") as its own
  dimension of data.

Model B collapses two questions that these results treat as distinct:
*did a principal attest to this record?* and *may the agent act on it as
ground truth?*

### 4. Internal coherence with F07 — the decisive argument

F07 (Multi-principal governance, sibling P1) introduces the *concept* of a
principal, `GovernancePolicy`, and an arbitration ledger. The concrete
identity token for a principal is the house `StableId` newtype — F07 does
not introduce a rival `PrincipalId` type. The Forge-native expression of
"a human reviewed this" is therefore not a boolean — it is **a principal
attestation** (a `StableId` + timestamp) recorded in F07's governance ledger.

Model A makes the review axis a first-class principal attestation and reuses
F07's infrastructure. Model B would collapse "reviewed" into "authoritative"
and destroy that integration, losing the ability to express two real cases:

- *Reviewed but not authoritative* — e.g. `"user said X at Y"`: a faithfully
  recorded, human-curated observation whose **content** is not endorsed as
  fact (high review, low authority).
- *Authoritative without review* — e.g. a spec or contract seeded as
  `Authority` by provenance type, not by human approval (high authority, no
  review).

The existing F06.2 schema already keeps `evidence_refs` separate from
`authority_level`. Adding `review_*` fields follows the same grain. The NFR
("promote exige policy e evidência raw") gates authority promotion on
**evidence**, not on review — Model A preserves that NFR intact; Model B
silently redefines the gate as review and contradicts it.

## Implemented schema delta (F06.2)

The two axes are added **additively** to the existing `MemoryEntry` in
`crates/forge-core-contracts/src/memory.rs`. All four new fields are
`Option<...>` with `#[serde(default)]`, so a pre-F06.2 YAML parses unchanged
under `deny_unknown_fields` (zero migration cost — the serde non-breaking
schema-evolution pattern, confirmed against serde-rs/serde#2634 and the
official container-attrs docs).

```rust
/// Axis 1 — authority. Gated by policy + raw evidence. Never auto-promoted.
pub enum AuthorityLevel { Raw, Provisional, Authority }

/// Axis 2 — review. Orthogonal to authority. Modelled as a principal
/// attestation (StableId + timestamp), not a magic boolean.
pub enum ReviewState { Unreviewed, Reviewed }

pub struct MemoryEntry {
    // ... existing fields (entry_id, kind, content, provenance, freshness,
    //     confidence, approval, supersedes, invalidation_reason) ...

    /// Axis 1. None = legacy record; resolved via authority_level_effective().
    #[serde(default)]
    pub authority_level: Option<AuthorityLevel>,

    /// Axis 2. None = legacy record; treated as Unreviewed.
    #[serde(default)]
    pub review_state: Option<ReviewState>,

    /// Who attested (F07 principal attestation). Reuses StableId, never a
    /// PrincipalId type (R8: the type does not exist; reviewer identity is
    /// comparable to any other StableId).
    #[serde(default)]
    pub reviewed_by: Option<StableId>,

    /// When the attestation was recorded (unix seconds string).
    #[serde(default)]
    pub reviewed_at: Option<String>,
}

impl MemoryEntry {
    /// Bridge from legacy single-axis ApprovalState to the new AuthorityLevel.
    /// An explicit authority_level field always wins. AutoPromoted collapses
    /// to Raw (never Authority) — honouring the F06 NFR.
    pub fn authority_level_effective(&self) -> AuthorityLevel { /* ... */ }
    pub fn review_state_effective(&self) -> ReviewState { /* ... */ }
}
```

### Coexistence with the legacy `ApprovalState` (Opção A)

The legacy single-axis `ApprovalState { Proposed, InReview, Approved,
Rejected, AutoPromoted }` is **retained, not removed**. The bridge
(`authority_level_effective`) maps it to the new axis in one co-localised
function:

| legacy `approval` | effective `authority_level` |
|---|---|
| `Proposed` / `InReview` / `Rejected` | `Raw` |
| `Approved` | `Provisional` |
| `AutoPromoted` | `Raw` (deprecated anti-pattern) |

`AutoPromoted` is forbidden by the `deny_auto_promoted` risk-audit rule
(`contracts/risk-audits/deny-auto-promoted.yaml`), which fails closed on the
YAML token — a stronger gate than a `#[deprecated]` compile warning. The
variant is not annotated `#[deprecated]` because the six derives on the enum
trip clippy spuriously (rust-lang/rust#92313). See CONTEXT.md
"AutoPromoted Anti-pattern".

An explicit `authority_level` field always overrides the legacy mapping —
this is how a migrated record opts into the new axis without editing its
`approval` value.

Invariants the validator (F06.4) must enforce:

- `review_state == Reviewed` requires `reviewed_by` and `reviewed_at` both set.
- `review_state == Unreviewed` requires `reviewed_by == None`.
- `authority_level == Authority` requires non-empty evidence AND a satisfied
  promote policy (F06.6). It does **not** require `Reviewed`.
- `reviewed_by` must resolve to a `StableId` permitted to attest under the
  F07 `GovernancePolicy`.
- `forget` (F06.5) removes the document from both axes atomically; the
  append-only forget log records the prior `(authority_level, review_state)`.

## Six-cell state space

| | Unreviewed | Reviewed |
|---|---|---|
| **Raw** | freshly ingested observation | curated observation, not endorsed as fact |
| **Provisional** | evidence-backed candidate, pending authority | curated candidate |
| **Authority** | authoritative by provenance/policy (spec seed) | curated AND authoritative ground truth |

All six are reachable and meaningful. Model B can express only three.

## Consequences

Positive:

- Closes the memory-poisoning attack surface structurally, not just by policy.
- Preserves the F06 NFR verbatim — authority promotion stays evidence-gated.
- Reuses the house `StableId` newtype + F07's governance ledger instead of
  inventing a parallel trust mechanism or a rival `PrincipalId` type.
- Occupies an unclaimed design space (no surveyed system separates these axes).

Negative / risks:

- **Two-dimensional state space** — 3 authority × 2 review = 6 cells. Requires
  an explicit state machine in the validator to prevent illegal combinations
  (the invariants above) and avoid combinatorial sprawl in policy.
- **Authority-promote contract is still underspecified.** This ADR fixes the
  *axes* but not the full *promotion policy* — e.g. whether some authority
  promotions additionally require review for high-impact `kind`s. That belongs
  in a follow-up ADR or the F06.2 `MemoryPolicy` rules.
- **CLI/UX complexity.** `forge-core memory promote` (authority axis) and a
  new `forge-core memory review` (review axis) must be distinct commands;
  conflating them in the CLI would re-introduce Model B through the back door.
- **Reviewer authorization** depends on F07 governance maturity; until F07
  lands, review attestations may need a bootstrap principal policy.

## Open questions for the F06.1 grill

1. May an `Authority`-by-provenance record skip review forever, or does any
   path require `Reviewed` eventually?
2. Is `review_state` monotonic (`Unreviewed → Reviewed` only) or reversible
   (revoke review)?
3. Does a high-impact `kind` (e.g. credential, irreversible action seed)
   require **both** `Authority` and `Reviewed` before the agent acts?
4. Should `forget` require a reviewed attestation, or is it purely the
   owner-principal's right?

## Addendum — Candidato 1: trust gates as pure PDP predicates (2026-07-01)

F06.2 Candidato 1 implements the gates the model above promised. Two low-level
design questions were resolved by **external research, not intuition**, because
they are exactly the kind of decision that should follow precedent.

### Decision 1 — the gates are pure predicates (PDP/PEP separation)

`can_admit(entry, policy)` and `can_promote(entry, policy, evidence)` return a
typed `AdmissionDecision` and **mutate nothing**. The actual store write — the
TOCTOU-safe admit/promote/forget — is the Policy Enforcement Point and lives in
the `forge-core-memory` crate (Candidato 2 / F06.3+).

This is the convergent design of every system that has solved "(policy,
evidence) → allow/deny":

- **Kubernetes admission control** splits validating webhooks (pure, return
  Allow/Deny + reason, fail-closed) from mutating webhooks, and orders them
  deliberately (mutate → validate) so validators reason over final state.
- **Open Policy Agent / Gatekeeper**: Rego is side-effect-free by design —
  evaluation produces a decision; a separate controller enforces. Quoted in
  their docs as "decouple policy decision-making from policy enforcement"
  (determinism, auditability, enforcement-location independence).
- **AWS Cedar** (Rust-implemented, same language, same problem):
  `is_authorized()` is a pure `(Request, Entities) → Response{Decision, reasons}`,
  and Cedar treats **order-independence of evaluation as a correctness
  invariant** that any mutation-during-evaluation would destroy.
- **XACML / Zero-Trust** names the pattern: Policy Decision Point (PDP)
  evaluates, Policy Enforcement Point (PEP) acts; "these functions are normally
  separated" (NIST Policy Machine).

The only counter-argument is TOCTOU (the gap between decide and write). It is
real but resolved by **atomicity at the write site** (transaction / lock / CAS
in Candidato 2), not by fusing policy into the mutator — which would couple
policy to storage internals, kill testability/replayability, and still not be
atomic without the same locking.

The Rust idiom confirms it: `tower::Service` separates `poll_ready` (decide)
from `call` (act); returning `enum AdmissionDecision { Allowed,
Blocked(Vec<Reason>) }` forces the caller to pattern-match and handle the
blocked case before the mutation — a free exhaustiveness check.

### Decision 2 — `policy` is a typed `MemoryPolicy` struct (not primitives, not hardcoded)

The `policy` parameter is one typed object, not scattered primitives, not
hardcoded rules. Same convergent evidence:

- **Google Zanzibar** (the foundation of Auth0 FGA / SpiceDB / OpenFGA):
  "a uniform data model and configuration language for expressing a wide range
  of access control policies" across hundreds of services — policy as
  first-class data.
- **AWS Cedar**: a policy must be backed by a **typed schema** so it can be
  "validated… to ensure… no type errors" and subjected to "automated reasoning"
  (performance, correctness, safety, analyzability).
- **OPA**: "policy is often a hard-coded feature of the software service…
  decouple policy" — declarative, updateable without recompile/redeploy.
- **Kubernetes CEL** (`x-kubernetes-validations`): validation rules are
  **co-located with the resource schema**, not scattered across controllers.
- **Microsoft Agent Control Specification (2026)**: "controls scattered across
  prompts, code, gateways, and frameworks make it risky" → "standard policy
  YAML, portable, versionable, auditable".

And the software-design principle: `can_admit(&[MemoryKind], &[String], usize)`
is Ousterhout's **shallow-module / wide-interface** anti-pattern
(*A Philosophy of Software Design*, ch. 4); `can_admit(&MemoryPolicy, &Evidence)`
is a **deep module** (small interface, hides growing policy complexity).

### Convention note — `Vec`, not `Set` (a deliberate divergence from Cedar)

Cedar deliberately uses **sets, not lists** and rejects **stringly-typed
attributes** to keep automated analysis tractable. This codebase **cannot**
apply that refinement without diverging from convention: the contracts crate
derives only `PartialEq, Eq` (never `Ord, Hash`) and uses **zero**
`BTreeSet`/`HashSet` anywhere. So:

- `MemoryPolicy.permitted_kinds: Vec<MemoryKind>` (not a set) — order is
  irrelevant because the gate **membership-checks** (not position-matches); the
  order-independence property Cedar needs is honoured by the gate's logic, not
  by the collection type.
- `required_evidence_fields: Vec<EvidenceField>` where `EvidenceField` is an
  **enum** (not `Vec<String>`) — a typo is a compile error, keeping the
  "no stringly-typed" spirit within the codebase's derive discipline.

### Gate semantics

- `can_admit`: fail-closed; an empty `permitted_kinds` denies all (the deny-all
  default, not a permissive one — `MemoryPolicy` deliberately has **no
  `Default`**); each absent required `EvidenceField` adds a denial; denials
  **accumulate** (matches the repo's no-short-circuit validation convention);
  it never consults the authority/review axes (admission decides ENTRY, at the
  `Raw`/`Unreviewed` floor — orthogonality NFR).
- `can_promote`: authority-axis **only**; never auto-promotes (a zero threshold
  still demands ≥1 distinct non-empty raw evidence ref — the F06 NFR); counts
  distinct non-empty refs (order-independent; empty/whitespace/duplicates
  collapse); never touches the review axis. The
  `AdmissionDenialReason::PromoteTargetsReviewAxis` variant is a **structural
  guard**: it cannot fire from today's pure API, but it exists so any future
  caller that conflates the axes has a named denial to emit (the
  Model-B-back-door guard).

### Scope (this story vs. the next)

Candidato 1 delivers the **decision functions** and their supporting contract
types in `forge-core-contracts`. The **mutating** admit/promote/forget, the new
crate `forge-core-memory`, the CLI verbs, and fixtures/E2E are Candidato 2 /
F06.3–F06.8 (separate stories, separate sessions). This keeps the
"complexity of deciding trust" concentrated in one deep module (deletion-test
unit) and the mutation in another.

### Novelty (extended threat model, 2026)

Confirmed by a 2026 prior-art sweep: **no** production agent-memory framework
(Letta/MemGPT, Zep/Graphiti, Cognee, LangGraph, Mem0, Memobase, A-MEM) ships a
graded authority tier gated on policy + raw evidence, and the field's first
unified taxonomy (arXiv:2512.13564, Dec 2025) has **no authority axis**. The
design is ahead of shipping systems but aligned with a 2026 emergent direction
(Moltbook "provenance class = trust tier"; Daly "promotion gate"; Huang "raw
traces are evidence, promoted memory is operational context") and
security-justified by the poisoning literature already cited in this ADR
(AgentPoison 2407.12784, PoisonedRAG 2402.07867 / USENIX Security 2025, MINJA
2503.03704). Gating promotion on **raw, independently-checkable** evidence
(logs/test output/diffs) rather than LLM-inferred summaries is the
poisoning-resistant substrate: an attacker can forge a plausible memory summary
but not a passing test run.

## References

- F06 spec: `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md`
- F06 epic: `../Forge-method-archive/dev-journals/followups_v0_1_to_10.md`
- F07 spec: same dir, `01_feature_specs.md` F07
- Threat evidence: arXiv 2302.12173, 2402.07867 (USENIX Security 2025),
  2407.12784, 2503.03704, 2502.13172; FilterRAG defense arXiv 2508.02835
- Prior-art survey: arXiv 2506.06326, 2512.13564 (Dec 2025 unified taxonomy,
  no authority axis); MemTrust 2601.07004 (cryptographic, orthogonal)
- Cross-field theory: Berenson SIGMOD'95; Bell-LaPadula MITRE'73;
  Sandhu RBAC96 IEEE'96; Buneman ICDT'01
- PDP/PEP & policy-as-data (Candidato 1): Zanzibar USENIX ATC'19;
  AWS Cedar (arXiv 2403.04651; docs.cedarpolicy.com); OPA philosophy;
  Kubernetes admission controllers & CEL (`x-kubernetes-validations`); Microsoft
  Agent Control Specification 2026; Ousterhout *A Philosophy of Software
  Design* ch. 4 (deep modules); XACML PDP/PEP (NIST Policy Machine)
- TOCTOU resolution: CWE-367 (atomicity at the write site, not check-fusion)
