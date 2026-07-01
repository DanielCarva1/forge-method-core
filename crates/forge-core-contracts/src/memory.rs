use crate::common::StableId;
use schemars::JsonSchema;
use serde::de::{Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};

const MAX_CONFIDENCE: u64 = 100;

fn deserialize_confidence<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_percent(u64::deserialize(deserializer)?, "confidence")
}

fn bounded_percent<E>(value: u64, field: &str) -> Result<u8, E>
where
    E: DeError,
{
    match u8::try_from(value) {
        Ok(percent) if value <= MAX_CONFIDENCE => Ok(percent),
        _ => Err(E::custom(format!(
            "{field} must be in the inclusive range 0..=100; got {value}"
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryContractDocument {
    pub schema_version: String,
    pub memory_contract: MemoryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryContract {
    pub id: StableId,
    pub scope: MemoryScope,
    pub entries: Vec<MemoryEntry>,
    pub superseded: Vec<StableId>,
}

impl MemoryContract {
    pub fn approved(&self) -> impl Iterator<Item = &MemoryEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.approval == ApprovalState::Approved)
    }

    pub fn pending_review(&self) -> impl Iterator<Item = &MemoryEntry> {
        self.entries.iter().filter(|entry| {
            matches!(
                entry.approval,
                ApprovalState::Proposed | ApprovalState::InReview
            )
        })
    }

    pub fn mark_stale(&mut self, now_unix_seconds: u64) {
        for entry in &mut self.entries {
            entry.freshness.stale = match entry.freshness.ttl_seconds {
                None => entry.freshness.stale,
                Some(ttl_seconds) => entry
                    .freshness
                    .last_confirmed_at
                    .parse::<u64>()
                    .ok()
                    .and_then(|last_confirmed_at| last_confirmed_at.checked_add(ttl_seconds))
                    .is_none_or(|expires_at| {
                        entry.freshness.stale || expires_at <= now_unix_seconds
                    }),
            };
        }
    }

    // --- F06.2 (Candidato 1) trust gates (ADR 0002 addendum) ---
    //
    // Pure predicates (Policy Decision Points). They decide whether an entry
    // may be admitted / promoted; they do NOT mutate the contract. The actual
    // write (the Policy Enforcement Point) is a TOCTOU-safe operation in the
    // `forge-core-memory` crate (Candidato 2 / F06.3+). Separation rationale
    // (Cedar, OPA, Kubernetes validating webhooks, XACML PDP/PEP): a pure
    // decision is deterministic, replayable, auditable, and order-independent;
    // fusing it into the mutation destroys those properties.

    /// F06.2 admission gate (PDP). Decides whether `entry` may ENTER the store
    /// at the trust floor (`Raw`, `Unreviewed`). Fail-closed: any missing
    /// policy or required evidence blocks. Admitting NEVER raises authority
    /// above `Raw` or review above `Unreviewed` — those transitions have their
    /// own gates (`can_promote`, and the review attestation in F07). See
    /// `CONTEXT.md` "Admission".
    pub fn can_admit(entry: &MemoryEntry, policy: &MemoryPolicy) -> AdmissionDecision {
        let mut denials = Vec::new();

        // An empty allow-list is the deny-all default; a non-empty list that
        // omits the entry's kind is a misconfigured policy. Both fail to admit
        // — checked as a single predicate to keep the two failure modes
        // indistinguishable at the decision boundary (the caller sees one
        // KindNotPermitted, never the policy's internal contents).
        if !kind_is_permitted(entry.kind, &policy.permitted_kinds) {
            denials.push(AdmissionDenialReason::KindNotPermitted);
        }

        for field in &policy.required_evidence_fields {
            if !field.is_present(&entry.provenance) {
                denials.push(AdmissionDenialReason::MissingRequiredEvidence);
            }
        }

        AdmissionDecision::from_denials(denials)
    }

    /// F06.2 promote gate (PDP). Authority-axis ONLY: decides whether the raw
    /// evidence offered clears the threshold to promote `entry`'s authority.
    /// **Never auto-promotes** — a zero threshold still requires at least one
    /// distinct non-empty evidence ref (the F06 NFR). Never consults or touches
    /// the review axis; the [`AdmissionDenialReason::PromoteTargetsReviewAxis`]
    /// variant is the structural guard documenting that invariant (it cannot
    /// fire from this pure API). See `CONTEXT.md` "Promote".
    pub fn can_promote(
        entry: &MemoryEntry,
        policy: &MemoryPolicy,
        evidence: &AdmissionEvidence,
    ) -> AdmissionDecision {
        // NFR: never auto-promote. Even a policy with threshold 0 demands some
        // raw evidence; an Authority with no evidence is the AutoPromoted bug.
        let distinct_refs = count_distinct_non_empty(&evidence.evidence_refs);
        if distinct_refs == 0 || distinct_refs < policy.min_evidence_refs_for_authority {
            return AdmissionDecision::Blocked(vec![
                AdmissionDenialReason::InsufficientEvidenceForAuthority,
            ]);
        }
        // Suppress unused-read warning: `entry` is part of the gate's contract
        // (callers pass the record being promoted) and will be consulted once
        // promote carries per-kind rules. Today the decision is evidence-only.
        let _ = entry;
        AdmissionDecision::Allowed
    }
}

/// F06.2 (Candidato 1) — the policy a memory trust gate consumes. One typed
/// object, not scattered primitives: the convergent design of Zanzibar
/// ("uniform data model… hundreds of services"), Cedar ("validate policies
/// against the schema… performance, correctness, safety, analyzability"),
/// OPA ("decouple policy… declarative, updateable without recompile"), and
/// Kubernetes CEL (`x-kubernetes-validations` co-located with the resource
/// schema). A wide-primitive signature (`&[MemoryKind], &[String], usize`)
/// would be Ousterhout's shallow-module anti-pattern.
///
/// **No `Default`** on purpose: a "default policy" that permits nothing is
/// fail-closed-correct, but a permissive default would be the `AutoPromoted` bug
/// under another name. Callers must construct a policy explicitly — itself a
/// guardrail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryPolicy {
    /// Memory kinds permitted to enter the store at all. Empty = deny all
    /// (fail-closed). Membership-checked inside the gate, so order is
    /// irrelevant — the order-independence Cedar requires of evaluation,
    /// achieved without `BTreeSet` (this crate derives only `Eq`, never
    /// `Ord`/`Hash`; the codebase uses `Vec` throughout).
    pub permitted_kinds: Vec<MemoryKind>,
    /// Provenance/evidence fields that must be present (non-empty) for
    /// admission. Typed (not `Vec<String>`): a typo is a compile error, the
    /// Cedar "no stringly-typed attributes" discipline kept within convention.
    pub required_evidence_fields: Vec<EvidenceField>,
    /// Minimum number of distinct raw evidence refs required to promote to
    /// [`AuthorityLevel::Authority`]. Promotion to `Provisional` does not
    /// require this; promotion to `Authority` does. A value of 0 still demands
    /// at least one ref at the gate (never auto-promote — the F06 NFR).
    pub min_evidence_refs_for_authority: usize,
}

/// Named evidence fields the policy can require on a record's provenance.
/// Replaces stringly-typed field names (Cedar warns against them); a typo here
/// is a compile error, not a silent admission failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceField {
    ProvenanceEvidenceRef,
    SourceRunId,
    SourceAgent,
}

impl EvidenceField {
    /// Whether the named provenance field is present (and, for the string
    /// ref, non-empty) on `p`. Used by the admission gate; co-localised with
    /// the variant so the field↔provenance mapping lives in one place.
    #[must_use]
    pub fn is_present(self, p: &MemoryProvenance) -> bool {
        match self {
            // String ref must be present AND non-empty (empty is not evidence).
            Self::ProvenanceEvidenceRef => p.evidence_ref.as_deref().is_some_and(|s| !s.is_empty()),
            // StableId is a newtype; presence is sufficient.
            Self::SourceRunId => p.source_run_id.is_some(),
            Self::SourceAgent => p.source_agent.is_some(),
        }
    }
}

/// The raw evidence a caller offers to a gate. The F06 NFR requires RAW
/// evidence (a log line, a test-run id, a committed diff) — never an LLM
/// inference. The gate is field-gated (presence of a non-empty ref), not
/// content-scored: the policy names what counts as evidence; it does not
/// grade the artifact. See ADR 0002 addendum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdmissionEvidence {
    /// Distinct raw evidence refs offered (e.g. test-run ids, log anchors,
    /// commit diffs). The promote gate counts distinct non-empty values.
    pub evidence_refs: Vec<String>,
}

/// A trust-gate decision (Policy Decision Point return). Pure: carries no
/// authority and mutates nothing. Cedar-style diagnostics-as-data so a
/// decision is auditable and replayable without side effects. Fail-closed:
/// `Blocked` is `Blocked` even with an empty reason list (it never collapses
/// to `Allowed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[must_use]
pub enum AdmissionDecision {
    Allowed,
    Blocked(Vec<AdmissionDenialReason>),
}

impl AdmissionDecision {
    #[inline]
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    #[inline]
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Build the decision from a list of denials: empty ⇒ Allowed, else
    /// Blocked(reasons). Centralised so the fail-closed rule lives in one
    /// place (a future caller cannot accidentally invert it). (No
    /// `#[must_use]` here — the enum is already `#[must_use]`.)
    pub fn from_denials(denials: Vec<AdmissionDenialReason>) -> Self {
        if denials.is_empty() {
            Self::Allowed
        } else {
            Self::Blocked(denials)
        }
    }
}

/// Why a trust gate blocked. As-data (Cedar-style diagnostics). The
/// `PromoteTargetsReviewAxis` variant is the structural guard for the
/// orthogonality NFR: a promote path that ever touched the review axis would
/// re-introduce the Model B bug "through the back door" (ADR 0002). The pure
/// `can_promote` API cannot trigger it today; it exists so any future caller
/// that conflates the axes has a named denial to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionDenialReason {
    /// `MemoryKind` not in the policy's `permitted_kinds` (includes the
    /// deny-all empty-list default).
    KindNotPermitted,
    /// A policy-required evidence/provenance field was absent or empty.
    MissingRequiredEvidence,
    /// Promote offered too few distinct raw evidence refs for `Authority`
    /// (including zero — the never-auto-promote NFR).
    InsufficientEvidenceForAuthority,
    /// A promote attempt targeted the review axis instead of the authority
    /// axis. Structural guard; see type doc.
    PromoteTargetsReviewAxis,
}

/// Counts distinct, non-empty evidence refs. Order-independent by construction
/// (the count does not depend on ref order), honouring Cedar's
/// order-independence-of-evaluation property without needing a `HashSet`.
fn count_distinct_non_empty(refs: &[String]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for r in refs {
        let trimmed = r.trim();
        if trimmed.is_empty() || seen.contains(&trimmed) {
            continue;
        }
        seen.push(trimmed);
    }
    seen.len()
}

/// Membership check for the admission allow-list. `false` for both an empty
/// allow-list (the deny-all default) and a non-empty list that omits `kind`;
/// the two failure modes are intentionally indistinguishable to the caller.
fn kind_is_permitted(kind: MemoryKind, permitted: &[MemoryKind]) -> bool {
    !permitted.is_empty() && permitted.contains(&kind)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryScope {
    pub kind: MemoryScopeKind,
    pub target: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeKind {
    Project,
    Repo,
    User,
    AgentRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntry {
    pub entry_id: StableId,
    pub kind: MemoryKind,
    pub content: String,
    pub provenance: MemoryProvenance,
    pub freshness: Freshness,
    #[schemars(range(min = 0, max = 100))]
    #[serde(deserialize_with = "deserialize_confidence")]
    pub confidence: u8,
    pub approval: ApprovalState,
    pub supersedes: Option<StableId>,
    pub invalidation_reason: Option<String>,
    // --- F06 Trust Axes (ADR 0002). Additive, non-breaking: `None` = legacy
    // document written before F06.2. Both default to the trust floor so a
    // legacy record is never silently authoritative. ---
    /// Trust Axis 1 — authority. `None` = legacy; resolved via
    /// [`authority_level_effective`](Self::authority_level_effective).
    #[serde(default)]
    pub authority_level: Option<AuthorityLevel>,
    /// Trust Axis 2 — review. `None` = legacy; treated as `Unreviewed`.
    #[serde(default)]
    pub review_state: Option<ReviewState>,
    /// Who attested to the record (F07 principal attestation). `None` unless
    /// `review_state == Reviewed`. Reuses `StableId`, never a `PrincipalId`
    /// (which does not exist in this codebase — R8 discipline, ADR 0002).
    #[serde(default)]
    pub reviewed_by: Option<StableId>,
    /// When the review attestation was recorded (unix seconds string, matches
    /// the house `captured_at` convention). `None` unless `review_state ==
    /// Reviewed`.
    #[serde(default)]
    pub reviewed_at: Option<String>,
}

impl MemoryEntry {
    /// Bridge from the legacy single-axis [`ApprovalState`] to the F06
    /// [`AuthorityLevel`] (Trust Axis 1). Co-localised with the enum so the
    /// coexistence rule lives in one place, not in N callers.
    ///
    /// # Mapping (Opção A, ADR 0002)
    ///
    /// | legacy `approval`     | `authority_level` field | effective result      |
    /// |-----------------------|-------------------------|-----------------------|
    /// | (none set, `None`)    | —                       | `Raw` (legacy floor)  |
    /// | `Proposed`/`InReview` | —                       | `Raw`                 |
    /// | `Approved`            | —                       | `Provisional`         |
    /// | `Rejected`            | —                       | `Raw`                 |
    /// | `AutoPromoted`        | —                       | `Raw` + deprecated    |
    /// | (any)                 | `Some(x)`               | `x` (explicit wins)   |
    ///
    /// An explicit `authority_level` field always wins over the legacy
    /// `approval` mapping — this is how a migrated record opts into the new
    /// axis. `AutoPromoted` collapses to `Raw` (never `Authority`), honouring
    /// the F06 NFR that promote exige policy e evidência raw.
    pub fn authority_level_effective(&self) -> AuthorityLevel {
        if let Some(explicit) = self.authority_level {
            return explicit;
        }
        match self.approval {
            ApprovalState::Approved => AuthorityLevel::Provisional,
            ApprovalState::Proposed
            | ApprovalState::InReview
            | ApprovalState::Rejected
            | ApprovalState::AutoPromoted => AuthorityLevel::Raw,
        }
    }

    /// Effective review state (Trust Axis 2). `None` on the field is treated
    /// as `Unreviewed` (legacy floor).
    pub fn review_state_effective(&self) -> ReviewState {
        self.review_state.unwrap_or(ReviewState::Unreviewed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Preference,
    Decision,
    LessonLearned,
    PlaybookRule,
    FailurePattern,
    GlossaryTerm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryProvenance {
    pub source_run_id: Option<StableId>,
    pub source_agent: Option<StableId>,
    pub evidence_ref: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Freshness {
    pub ttl_seconds: Option<u64>,
    pub last_confirmed_at: String,
    pub stale: bool,
}

/// F06 Trust Axis 1 — authority. Whether the agent may treat a Memory
/// Document as ground truth for autonomous action. See `CONTEXT.md`
/// "Authority Axis". Gated by policy + raw evidence; **never auto-promoted**
/// (the F06 NFR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLevel {
    /// Freshly ingested, no evidence endorsement. Admitted for retrieval as
    /// context, never actionable as fact. Default on ingest.
    Raw,
    /// Evidence-backed candidate, pending stronger proof or review. May inform
    /// action but is not the final word.
    Provisional,
    /// The agent may act on it as ground truth. Requires non-empty
    /// `evidence_refs` AND a satisfied promote policy (F06.6).
    Authority,
}

/// F06 Trust Axis 2 — review. Orthogonal to authority: has a principal
/// attested to this record's curation? See `CONTEXT.md` "Review Axis".
/// Modelled as a principal attestation (`StableId`), not a magic boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// No principal has curated the record (default).
    Unreviewed,
    /// A principal attested to the record via `reviewed_by` + `reviewed_at`.
    Reviewed,
}

/// Legacy single-axis approval state. Superseded by the two-axis model
/// ([`AuthorityLevel`] + [`ReviewState`]) in ADR 0002. Retained for
/// zero-migration-cost backwards compatibility (Opção A); bridged to the
/// new axes by [`MemoryEntry::authority_level_effective`].
///
/// `AutoPromoted` is a **deprecated anti-pattern**: it violates the F06 NFR
/// ("nenhuma memória vira authority automaticamente"). It is NOT marked with
/// `#[deprecated]` because the six derives on this enum would trip clippy
/// (rust-lang/rust#92313); enforcement is instead via the
/// `deny_auto_promoted` risk-audit rule, which fails closed on the YAML
/// token `approval: auto_promoted` — a stronger gate than a compile warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Proposed,
    InReview,
    Approved,
    Rejected,
    /// Deprecated anti-pattern (F06 NFR violation). Detected by the
    /// `deny_auto_promoted` risk-audit rule. The bridge resolves it to
    /// [`AuthorityLevel::Raw`] if it ever reaches runtime.
    AutoPromoted,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(entry_id: &str, approval: ApprovalState, last_confirmed_at: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: StableId(entry_id.into()),
            kind: MemoryKind::PlaybookRule,
            content: "prefer typed YAML contracts with explicit provenance".into(),
            provenance: MemoryProvenance {
                source_run_id: Some(StableId("run.wave-3".into())),
                source_agent: Some(StableId("worker.memory".into())),
                evidence_ref: Some(
                    "contracts/research/community-trends-and-requested-features-v1.yaml".into(),
                ),
                captured_at: "1700000000".into(),
            },
            freshness: Freshness {
                ttl_seconds: Some(60),
                last_confirmed_at: last_confirmed_at.into(),
                stale: false,
            },
            confidence: 92,
            approval,
            supersedes: None,
            invalidation_reason: None,
            // Legacy helper: no explicit F06 axes — exercises the bridge.
            authority_level: None,
            review_state: None,
            reviewed_by: None,
            reviewed_at: None,
        }
    }

    fn contract() -> MemoryContractDocument {
        MemoryContractDocument {
            schema_version: "0.1".into(),
            memory_contract: MemoryContract {
                id: StableId("memory.project.forge".into()),
                scope: MemoryScope {
                    kind: MemoryScopeKind::Project,
                    target: StableId("forge-method-core".into()),
                },
                entries: vec![
                    entry("memory.entry.approved", ApprovalState::Approved, "100"),
                    entry("memory.entry.proposed", ApprovalState::Proposed, "200"),
                    entry("memory.entry.review", ApprovalState::InReview, "300"),
                ],
                superseded: vec![StableId("memory.project.old".into())],
            },
        }
    }

    #[test]
    fn round_trips_memory_contract() {
        let doc = contract();
        let yaml = yaml_serde::to_string(&doc).expect("serialize memory contract");
        let parsed: MemoryContractDocument =
            yaml_serde::from_str(&yaml).expect("deserialize memory contract");

        assert_eq!(doc, parsed);
        assert!(yaml.contains("playbook_rule"));
        assert!(yaml.contains("agent_role") || yaml.contains("project"));
    }

    #[test]
    fn example_memory_yaml_round_trips() {
        let yaml = include_str!("../../../contracts/examples/memory.yaml");
        let doc: MemoryContractDocument =
            yaml_serde::from_str(yaml).expect("deserialize memory example");
        let serialized = yaml_serde::to_string(&doc).expect("serialize memory example");
        let parsed: MemoryContractDocument =
            yaml_serde::from_str(&serialized).expect("deserialize serialized example");

        assert_eq!(doc, parsed);
    }

    #[test]
    fn denies_unknown_fields() {
        let yaml = r#"schema_version: "0.1"
memory_contract:
  id: memory.project.forge
  scope:
    kind: project
    target: forge-method-core
  entries: []
  superseded: []
  unknown: nope
"#;

        let err = yaml_serde::from_str::<MemoryContractDocument>(yaml).unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn rejects_memory_entry_confidence_above_100() {
        let yaml = include_str!("../../../contracts/examples/memory.yaml").replacen(
            "confidence: 96",
            "confidence: 101",
            1,
        );

        let err = yaml_serde::from_str::<MemoryContractDocument>(&yaml).unwrap_err();

        assert!(err.to_string().contains("confidence"));
    }

    #[test]
    fn approved_and_pending_review_filters_are_stable() {
        let doc = contract();

        let approved: Vec<_> = doc
            .memory_contract
            .approved()
            .map(|entry| entry.entry_id.0.as_str())
            .collect();
        assert_eq!(approved, vec!["memory.entry.approved"]);

        let pending: Vec<_> = doc
            .memory_contract
            .pending_review()
            .map(|entry| entry.entry_id.0.as_str())
            .collect();
        assert_eq!(
            pending,
            vec!["memory.entry.proposed", "memory.entry.review"]
        );
    }

    #[test]
    fn mark_stale_flips_only_elapsed_ttl_entries() {
        let mut memory = MemoryContract {
            id: StableId("memory.project.forge".into()),
            scope: MemoryScope {
                kind: MemoryScopeKind::Project,
                target: StableId("forge-method-core".into()),
            },
            entries: vec![
                entry("elapsed", ApprovalState::Approved, "100"),
                entry("fresh", ApprovalState::Approved, "1000"),
                MemoryEntry {
                    freshness: Freshness {
                        ttl_seconds: None,
                        last_confirmed_at: "1".into(),
                        stale: false,
                    },
                    ..entry("no-ttl", ApprovalState::Approved, "1")
                },
                MemoryEntry {
                    freshness: Freshness {
                        ttl_seconds: None,
                        last_confirmed_at: "1".into(),
                        stale: true,
                    },
                    ..entry("no-ttl-already-stale", ApprovalState::Approved, "1")
                },
                MemoryEntry {
                    freshness: Freshness {
                        ttl_seconds: Some(60),
                        last_confirmed_at: "not-a-unix-second".into(),
                        stale: false,
                    },
                    ..entry("parse-error", ApprovalState::Approved, "1")
                },
                MemoryEntry {
                    freshness: Freshness {
                        ttl_seconds: Some(u64::MAX),
                        last_confirmed_at: "1".into(),
                        stale: false,
                    },
                    ..entry("overflow", ApprovalState::Approved, "1")
                },
                MemoryEntry {
                    freshness: Freshness {
                        ttl_seconds: Some(60),
                        last_confirmed_at: "1000".into(),
                        stale: true,
                    },
                    ..entry("already-stale", ApprovalState::Approved, "1")
                },
            ],
            superseded: vec![],
        };

        memory.mark_stale(200);

        let stale_flags: Vec<_> = memory
            .entries
            .iter()
            .map(|entry| (entry.entry_id.0.as_str(), entry.freshness.stale))
            .collect();
        assert_eq!(
            stale_flags,
            vec![
                ("elapsed", true),
                ("fresh", false),
                ("no-ttl", false),
                ("no-ttl-already-stale", true),
                ("parse-error", true),
                ("overflow", true),
                ("already-stale", true),
            ]
        );
    }

    #[test]
    fn supersedes_chain_fields_are_present() {
        let yaml = r#"schema_version: "0.1"
memory_contract:
  id: memory.project.forge
  scope:
    kind: project
    target: forge-method-core
  entries:
    - entry_id: memory.entry.new
      kind: lesson_learned
      content: "Prefer small disjoint-file batches for parallel workers."
      provenance:
        source_run_id: run.wave-3
        source_agent: worker.memory
        evidence_ref: contracts/audit/comb-through-quality-v1.yaml
        captured_at: "1700000000"
      freshness:
        ttl_seconds: 86400
        last_confirmed_at: "1700000000"
        stale: false
      confidence: 88
      approval: auto_promoted
      supersedes: memory.entry.old
      invalidation_reason: null
  superseded:
    - memory.contract.old
"#;

        let doc: MemoryContractDocument = yaml_serde::from_str(yaml).expect("deserialize memory");
        assert_eq!(
            doc.memory_contract.entries[0].supersedes,
            Some(StableId("memory.entry.old".into()))
        );
        assert_eq!(
            doc.memory_contract.superseded,
            vec![StableId("memory.contract.old".into())]
        );
    }

    // --- F06 trust-axis bridge tests (ADR 0002, Opção A) ---

    #[test]
    fn bridge_legacy_approved_maps_to_provisional() {
        // Approved is the only legacy rung that earns any authority — but only
        // Provisional, never Authority (NFR: promote exige evidence).
        let e = entry("e", ApprovalState::Approved, "100");
        assert_eq!(e.authority_level_effective(), AuthorityLevel::Provisional);
    }

    #[test]
    fn bridge_legacy_floor_states_map_to_raw() {
        for approval in [
            ApprovalState::Proposed,
            ApprovalState::InReview,
            ApprovalState::Rejected,
            ApprovalState::AutoPromoted,
        ] {
            let e = entry("e", approval, "100");
            assert_eq!(
                e.authority_level_effective(),
                AuthorityLevel::Raw,
                "approval {approval:?} must bridge to Raw"
            );
        }
    }

    #[test]
    fn bridge_explicit_authority_field_wins_over_legacy_approval() {
        // A migrated record opts into the new axis by setting the field; the
        // legacy `approval` is ignored for authority resolution.
        let mut e = entry("e", ApprovalState::Approved, "100");
        e.authority_level = Some(AuthorityLevel::Authority);
        assert_eq!(e.authority_level_effective(), AuthorityLevel::Authority);
    }

    #[test]
    fn bridge_auto_promoted_never_reaches_authority() {
        // The deprecated anti-pattern must collapse to Raw even though its
        // name suggests promotion.
        let mut e = entry("e", ApprovalState::AutoPromoted, "100");
        e.authority_level = None;
        assert_eq!(e.authority_level_effective(), AuthorityLevel::Raw);
        // And an explicit Authority field on an AutoPromoted record still wins
        // (explicit always wins) — the deprecation is textual (risk-audit),
        // not a runtime downgrade of legitimate fields.
        e.authority_level = Some(AuthorityLevel::Authority);
        assert_eq!(e.authority_level_effective(), AuthorityLevel::Authority);
    }

    #[test]
    fn bridge_legacy_review_defaults_to_unreviewed() {
        // Legacy records have review_state: None → effective Unreviewed.
        let e = entry("e", ApprovalState::Approved, "100");
        assert_eq!(e.review_state_effective(), ReviewState::Unreviewed);
    }

    #[test]
    fn explicit_review_state_round_trips() {
        // A record written under F06.2 with explicit axes round-trips.
        let mut e = entry("e", ApprovalState::Approved, "100");
        e.authority_level = Some(AuthorityLevel::Authority);
        e.review_state = Some(ReviewState::Reviewed);
        e.reviewed_by = Some(StableId("principal.daniel".into()));
        e.reviewed_at = Some("1700000100".into());
        let yaml = yaml_serde::to_string(&e).expect("serialize entry");
        let parsed: MemoryEntry = yaml_serde::from_str(&yaml).expect("deserialize entry");
        assert_eq!(e, parsed);
        assert_eq!(
            parsed.authority_level_effective(),
            AuthorityLevel::Authority
        );
        assert_eq!(parsed.review_state_effective(), ReviewState::Reviewed);
    }

    #[test]
    fn legacy_yaml_without_axes_still_deserializes() {
        // A pre-F06.2 YAML (no authority_level/review_state fields) must still
        // parse under deny_unknown_fields + serde(default). This is the
        // zero-migration-cost guarantee (Opção A).
        let yaml = r#"schema_version: "0.1"
memory_contract:
  id: memory.project.forge
  scope:
    kind: project
    target: forge-method-core
  entries:
    - entry_id: memory.entry.legacy
      kind: preference
      content: "legacy record with no trust axes"
      provenance:
        source_run_id: null
        source_agent: worker.memory
        evidence_ref: null
        captured_at: "1700000000"
      freshness:
        ttl_seconds: null
        last_confirmed_at: "1700000000"
        stale: false
      confidence: 50
      approval: proposed
      supersedes: null
      invalidation_reason: null
  superseded: []
"#;
        let doc: MemoryContractDocument =
            yaml_serde::from_str(yaml).expect("legacy YAML must parse");
        let entry = &doc.memory_contract.entries[0];
        assert_eq!(entry.authority_level, None);
        assert_eq!(entry.review_state, None);
        assert_eq!(entry.authority_level_effective(), AuthorityLevel::Raw);
        assert_eq!(entry.review_state_effective(), ReviewState::Unreviewed);
    }

    // --- F06.2 (Candidato 1) trust-gate tests (ADR 0002 addendum) ---

    /// A permissive policy used by most tests: all kinds allowed, every
    /// evidence field required, one ref buys Authority.
    fn policy() -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: vec![
                MemoryKind::Preference,
                MemoryKind::Decision,
                MemoryKind::LessonLearned,
                MemoryKind::PlaybookRule,
                MemoryKind::FailurePattern,
                MemoryKind::GlossaryTerm,
            ],
            required_evidence_fields: vec![
                EvidenceField::ProvenanceEvidenceRef,
                EvidenceField::SourceRunId,
                EvidenceField::SourceAgent,
            ],
            min_evidence_refs_for_authority: 1,
        }
    }

    fn evidence(refs: &[&str]) -> AdmissionEvidence {
        AdmissionEvidence {
            evidence_refs: refs.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Variant of `entry` that lets a test pick the kind and strip evidence.
    fn entry_with(kind: MemoryKind, evidence_ref: Option<&str>) -> MemoryEntry {
        let mut e = entry("memory.entry.under-test", ApprovalState::Proposed, "100");
        e.kind = kind;
        e.provenance.evidence_ref = evidence_ref.map(str::to_string);
        e
    }

    #[test]
    fn can_admit_permits_permitted_kind_with_evidence() {
        // The legacy `entry()` helper carries all three evidence fields, so a
        // policy that requires them admits it.
        let e = entry("e", ApprovalState::Proposed, "100");
        let decision = MemoryContract::can_admit(&e, &policy());
        assert!(decision.is_allowed(), "{decision:?} should be Allowed");
    }

    #[test]
    fn can_admit_blocks_disallowed_kind() {
        // Allow only Preferences; a PlaybookRule (the legacy helper's kind) is
        // rejected.
        let mut policy = policy();
        policy.permitted_kinds = vec![MemoryKind::Preference];
        let e = entry("e", ApprovalState::Proposed, "100");
        let decision = MemoryContract::can_admit(&e, &policy);
        assert_eq!(
            decision,
            AdmissionDecision::Blocked(vec![AdmissionDenialReason::KindNotPermitted])
        );
    }

    #[test]
    fn can_admit_blocks_when_required_evidence_missing() {
        // Each EvidenceField variant, when absent, blocks admission.
        let mut e = entry("e", ApprovalState::Proposed, "100");

        // evidence_ref absent.
        e.provenance.evidence_ref = None;
        assert_eq!(
            MemoryContract::can_admit(&e, &policy()),
            AdmissionDecision::Blocked(vec![AdmissionDenialReason::MissingRequiredEvidence])
        );

        // source_run_id absent.
        let mut e = entry("e", ApprovalState::Proposed, "100");
        e.provenance.source_run_id = None;
        assert_eq!(
            MemoryContract::can_admit(&e, &policy()),
            AdmissionDecision::Blocked(vec![AdmissionDenialReason::MissingRequiredEvidence])
        );

        // source_agent absent.
        let mut e = entry("e", ApprovalState::Proposed, "100");
        e.provenance.source_agent = None;
        assert_eq!(
            MemoryContract::can_admit(&e, &policy()),
            AdmissionDecision::Blocked(vec![AdmissionDenialReason::MissingRequiredEvidence])
        );
    }

    #[test]
    fn can_admit_treats_empty_evidence_ref_as_absent() {
        // An empty string is not evidence (field-gated, not value-trusted).
        let mut policy = policy();
        policy.required_evidence_fields = vec![EvidenceField::ProvenanceEvidenceRef];
        let e = entry_with(MemoryKind::Decision, Some(""));
        assert!(!MemoryContract::can_admit(&e, &policy).is_allowed());
    }

    #[test]
    fn can_admit_fail_closed_on_empty_permitted_kinds() {
        // Deny-all default: an empty allow-list permits nothing, even for a
        // record that would otherwise satisfy every evidence requirement.
        let mut policy = policy();
        policy.permitted_kinds = vec![];
        let e = entry("e", ApprovalState::Proposed, "100");
        assert_eq!(
            MemoryContract::can_admit(&e, &policy),
            AdmissionDecision::Blocked(vec![AdmissionDenialReason::KindNotPermitted])
        );
    }

    #[test]
    fn can_admit_accumulates_multiple_denials() {
        // Wrong kind AND missing evidence → two distinct reasons in one
        // Blocked. Matches the repo's accumulating-diagnostics convention.
        let mut policy = policy();
        policy.permitted_kinds = vec![MemoryKind::Preference];
        let mut e = entry("e", ApprovalState::Proposed, "100");
        e.provenance.evidence_ref = None;
        let decision = MemoryContract::can_admit(&e, &policy);
        assert_eq!(
            decision,
            AdmissionDecision::Blocked(vec![
                AdmissionDenialReason::KindNotPermitted,
                AdmissionDenialReason::MissingRequiredEvidence,
            ])
        );
    }

    #[test]
    fn can_admit_never_consults_authority_or_review_fields() {
        // The admission gate decides ENTRY, not authority. A record whose
        // explicit authority is Authority / review is Reviewed is admitted on
        // the same terms as a floor record — the trust axes are orthogonal
        // (the F06 NFR).
        let mut e = entry("e", ApprovalState::Proposed, "100");
        e.authority_level = Some(AuthorityLevel::Authority);
        e.review_state = Some(ReviewState::Reviewed);
        e.reviewed_by = Some(StableId("principal.daniel".into()));
        e.reviewed_at = Some("1700000100".into());
        assert!(MemoryContract::can_admit(&e, &policy()).is_allowed());
        // And a floor record is also admitted — admission ignores axes.
        let floor = entry("floor", ApprovalState::Proposed, "100");
        assert!(MemoryContract::can_admit(&floor, &policy()).is_allowed());
    }

    #[test]
    fn can_promote_denies_without_evidence() {
        // The F06 NFR: no evidence ⇒ no promote, even with a zero threshold.
        let e = entry("e", ApprovalState::Proposed, "100");
        let mut policy = policy();
        policy.min_evidence_refs_for_authority = 0;
        let decision = MemoryContract::can_promote(&e, &policy, &evidence(&[]));
        assert_eq!(
            decision,
            AdmissionDecision::Blocked(vec![
                AdmissionDenialReason::InsufficientEvidenceForAuthority
            ])
        );
    }

    #[test]
    fn can_promote_partial_evidence_blocks_authority() {
        // One ref offered, threshold is two ⇒ blocked.
        let e = entry("e", ApprovalState::Proposed, "100");
        let mut policy = policy();
        policy.min_evidence_refs_for_authority = 2;
        let decision = MemoryContract::can_promote(&e, &policy, &evidence(&["run.alpha"]));
        assert!(decision.is_blocked());
    }

    #[test]
    fn can_promote_sufficient_evidence_allows_authority() {
        // Two distinct refs clear a threshold of two.
        let e = entry("e", ApprovalState::Proposed, "100");
        let mut policy = policy();
        policy.min_evidence_refs_for_authority = 2;
        let decision =
            MemoryContract::can_promote(&e, &policy, &evidence(&["run.alpha", "run.beta"]));
        assert!(decision.is_allowed(), "{decision:?} should be Allowed");
    }

    #[test]
    fn can_promote_counts_distinct_and_ignores_empty() {
        // Duplicates and empty strings collapse; only distinct non-empty refs
        // count toward the threshold.
        let e = entry("e", ApprovalState::Proposed, "100");
        let mut policy = policy();
        policy.min_evidence_refs_for_authority = 2;
        // "run.alpha" twice + "" + "  " + "run.beta" = 2 distinct non-empty.
        let ev = evidence(&["run.alpha", "run.alpha", "", "  ", "run.beta"]);
        assert!(MemoryContract::can_promote(&e, &policy, &ev).is_allowed());
        // But the same two distinct with threshold 3 ⇒ blocked.
        policy.min_evidence_refs_for_authority = 3;
        assert!(MemoryContract::can_promote(&e, &policy, &ev).is_blocked());
    }

    #[test]
    fn can_promote_does_not_touch_review_axis() {
        // Promote is authority-axis only. A record with no review fields stays
        // that way after a promote decision — the gate mutates nothing and
        // never signals a review transition. (The actual write is Candidato 2.)
        let mut e = entry("e", ApprovalState::Proposed, "100");
        e.review_state = None;
        e.reviewed_by = None;
        e.reviewed_at = None;
        let before = (e.review_state, e.reviewed_by.clone(), e.reviewed_at.clone());
        let _ = MemoryContract::can_promote(&e, &policy(), &evidence(&["run.alpha"]));
        let after = (e.review_state, e.reviewed_by.clone(), e.reviewed_at.clone());
        assert_eq!(before, after, "promote must not touch the review axis");
    }

    #[test]
    fn admission_decision_accessors_and_fail_closed() {
        // Allowed/block accessors round-trip.
        assert!(AdmissionDecision::Allowed.is_allowed());
        assert!(!AdmissionDecision::Allowed.is_blocked());
        let blocked = AdmissionDecision::Blocked(vec![AdmissionDenialReason::KindNotPermitted]);
        assert!(!blocked.is_allowed());
        assert!(blocked.is_blocked());

        // Fail-closed: from_denials([]) is Allowed, but a Blocked with an empty
        // reason list (constructed directly) is still Blocked, never Allowed —
        // it never collapses to Allowed via an empty list.
        assert!(AdmissionDecision::from_denials(vec![]).is_allowed());
        assert!(AdmissionDecision::Blocked(vec![]).is_blocked());
        assert_ne!(
            AdmissionDecision::Blocked(vec![]),
            AdmissionDecision::Allowed
        );
    }

    #[test]
    fn policy_and_evidence_round_trip_yaml() {
        // MemoryPolicy + AdmissionEvidence are typed contracts: they round-trip
        // under deny_unknown_fields, and unknown fields are rejected.
        let policy = MemoryPolicy {
            permitted_kinds: vec![MemoryKind::Decision, MemoryKind::LessonLearned],
            required_evidence_fields: vec![EvidenceField::ProvenanceEvidenceRef],
            min_evidence_refs_for_authority: 3,
        };
        let yaml = yaml_serde::to_string(&policy).expect("serialize policy");
        let parsed: MemoryPolicy = yaml_serde::from_str(&yaml).expect("deserialize policy");
        assert_eq!(policy, parsed);
        assert!(yaml.contains("permitted_kinds"));
        assert!(yaml.contains("min_evidence_refs_for_authority"));

        let ev = AdmissionEvidence {
            evidence_refs: vec!["run.alpha".into(), "run.beta".into()],
        };
        let yaml = yaml_serde::to_string(&ev).expect("serialize evidence");
        let parsed: AdmissionEvidence = yaml_serde::from_str(&yaml).expect("deserialize evidence");
        assert_eq!(ev, parsed);

        // Unknown field rejected.
        let err = yaml_serde::from_str::<MemoryPolicy>("permitted_kinds: []\nrequired_evidence_fields: []\nmin_evidence_refs_for_authority: 0\nbogus: true\n").unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn legacy_yaml_without_policy_field_still_parses() {
        // MemoryPolicy is a NEW type — no field was added to MemoryContract, so
        // legacy memory.yaml is unaffected. This test pins the additive
        // (zero-migration-cost) guarantee for the gate additions.
        let yaml = include_str!("../../../contracts/examples/memory.yaml");
        let doc: MemoryContractDocument =
            yaml_serde::from_str(yaml).expect("legacy memory YAML must still parse");
        assert!(!doc.memory_contract.entries.is_empty());
    }
}
