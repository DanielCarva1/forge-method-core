# ADR 0002: Two Orthogonal Trust Axes for Agent Memory

## Status

**Accepted** (2026-07-01). The F06.1 grill — including the
`improve-codebase-architecture` deletion-test pass and the external research
sweep (newtype/type-driven-design consensus, serde non-breaking schema
evolution, Rust enum-variant deprecation) — concluded in favour of **Model A,
Opção A** (additive, zero migration cost). This ADR is now authoritative.

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

## References

- F06 spec: `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md`
- F06 epic: `docs/dev-docs/forge-method-core-dev-docs-v2/progress/followups_v0_1_to_10.md`
- F07 spec: same dir, `01_feature_specs.md` F07
- Threat evidence: arXiv 2302.12173, 2402.07867, 2407.12784, 2503.03704,
  2502.13172
- Prior-art survey: arXiv 2506.06326, 2512.13564
- Cross-field theory: Berenson SIGMOD'95; Bell-LaPadula MITRE'73;
  Sandhu RBAC96 IEEE'96; Buneman ICDT'01
