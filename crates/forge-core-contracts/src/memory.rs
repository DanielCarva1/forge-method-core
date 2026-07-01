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
        assert_eq!(parsed.authority_level_effective(), AuthorityLevel::Authority);
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
}
