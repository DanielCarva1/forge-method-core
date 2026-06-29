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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Proposed,
    InReview,
    Approved,
    Rejected,
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
        let yaml = serde_yaml::to_string(&doc).expect("serialize memory contract");
        let parsed: MemoryContractDocument =
            serde_yaml::from_str(&yaml).expect("deserialize memory contract");

        assert_eq!(doc, parsed);
        assert!(yaml.contains("playbook_rule"));
        assert!(yaml.contains("agent_role") || yaml.contains("project"));
    }

    #[test]
    fn example_memory_yaml_round_trips() {
        let yaml = include_str!("../../../contracts/examples/memory.yaml");
        let doc: MemoryContractDocument =
            serde_yaml::from_str(yaml).expect("deserialize memory example");
        let serialized = serde_yaml::to_string(&doc).expect("serialize memory example");
        let parsed: MemoryContractDocument =
            serde_yaml::from_str(&serialized).expect("deserialize serialized example");

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

        let err = serde_yaml::from_str::<MemoryContractDocument>(yaml).unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn rejects_memory_entry_confidence_above_100() {
        let yaml = include_str!("../../../contracts/examples/memory.yaml").replacen(
            "confidence: 96",
            "confidence: 101",
            1,
        );

        let err = serde_yaml::from_str::<MemoryContractDocument>(&yaml).unwrap_err();

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

        let doc: MemoryContractDocument = serde_yaml::from_str(yaml).expect("deserialize memory");
        assert_eq!(
            doc.memory_contract.entries[0].supersedes,
            Some(StableId("memory.entry.old".into()))
        );
        assert_eq!(
            doc.memory_contract.superseded,
            vec![StableId("memory.contract.old".into())]
        );
    }
}
