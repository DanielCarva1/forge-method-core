//! Retention — lazy TTL sweep on read + explicit forget.
//!
//! Two operations:
//! - [`list_now`]: the lazy TTL path (Redis passive-expiry model — no background
//!   daemon). Acquires the lock, reads the projection, calls the pure
//!   [`MemoryContract::mark_stale`](forge_core_contracts::MemoryContract::mark_stale)
//!   on a materialized `MemoryContract`, persists any newly-flipped `stale`
//!   flags by appending a corrected-state event, and returns the non-stale
//!   entries. The sweep happens at read time under the lock so a concurrent
//!   writer cannot invalidate a value mid-read (the stale-read race, CWE-367).
//! - [`forget`]: explicit, append-only. Records the FULL before-image in a
//!   [`Forgotten`](crate::MemoryEvent::Forgotten) event (Debezium `before` /
//!   Postgres `REPLICA IDENTITY FULL`) and removes the entry from the projection.
//!
//! Both hold the exclusive lock across read-and-write.

use std::path::Path;

use forge_core_contracts::{
    MemoryContract, MemoryContractDocument, MemoryEntry, MemoryScope, MemoryScopeKind, StableId,
};
use forge_core_store::{append_json_line_with_durability, WalDurability};

use crate::{
    next_sequence, now_unix, project_locked, ForgetError, MemoryEvent, MemoryProjection,
    MEMORY_LOCK_RELATIVE_PATH, MEMORY_LOG_RELATIVE_PATH,
};

/// The status of a [`forget`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgetStatus {
    /// The entry was forgotten; the `Forgotten` before-image event was appended.
    Forgotten { sequence: u64 },
    /// The entry was already forgotten (id is in the superseded set). A
    /// repeat forget is a no-op, not an error — appending another before-image
    /// would be misleading (there is nothing to remove).
    AlreadyForgotten,
    /// The entry is not present (never admitted, or forgotten by an earlier call).
    NotFound,
    /// A storage error prevented the forget.
    StoreError(ForgetError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgetResult {
    pub status: ForgetStatus,
    pub entry_id: StableId,
}

impl ForgetResult {
    #[must_use]
    pub fn is_forgotten(&self) -> bool {
        matches!(self.status, ForgetStatus::Forgotten { .. })
    }
}

/// The status of a [`list_now`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListStatus {
    /// The sweep ran; `flipped` entries had their stale flag newly set and
    /// persisted. `entries` are the live (non-stale) records.
    Ok {
        flipped: usize,
        entries: Vec<MemoryEntry>,
    },
    /// A storage error prevented the sweep.
    StoreError(ForgetError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResult {
    pub status: ListStatus,
}

/// Forget `entry_id`: append a `Forgotten` before-image event and remove the
/// entry from the store. Idempotent — a second forget of the same id is
/// [`ForgetStatus::AlreadyForgotten`] (no event appended). Durability defaults
/// to `SyncOnAppend`.
pub fn forget(root: impl AsRef<Path>, entry_id: StableId) -> ForgetResult {
    forget_with_durability(root, entry_id, WalDurability::default())
}

/// As [`forget`] with an explicit durability knob.
#[allow(clippy::needless_pass_by_value)]
pub fn forget_with_durability(
    root: impl AsRef<Path>,
    entry_id: StableId,
    durability: WalDurability,
) -> ForgetResult {
    let root = root.as_ref();

    let _lock = match forge_core_store::acquire_effect_store_lock(root, MEMORY_LOCK_RELATIVE_PATH) {
        Ok(lock) => lock,
        Err(source) => {
            return ForgetResult {
                status: ForgetStatus::StoreError(ForgetError::Lock {
                    path: root.join(MEMORY_LOCK_RELATIVE_PATH),
                    source: source.to_string(),
                }),
                entry_id,
            };
        }
    };

    let projection = match project_locked(root) {
        Ok(projection) => projection,
        Err(source) => {
            return ForgetResult {
                status: ForgetStatus::StoreError(ForgetError::Read {
                    path: root.join(MEMORY_LOG_RELATIVE_PATH),
                    source: source.to_string(),
                }),
                entry_id,
            };
        }
    };

    // Already forgotten? Idempotent no-op.
    if projection.superseded.contains(&entry_id.0) {
        return ForgetResult {
            status: ForgetStatus::AlreadyForgotten,
            entry_id,
        };
    }
    let Some(before) = projection.entries.get(&entry_id.0).cloned() else {
        return ForgetResult {
            status: ForgetStatus::NotFound,
            entry_id,
        };
    };

    let sequence = next_sequence(&projection);
    let content_hash = MemoryEvent::content_hash_of(&before);
    let event = MemoryEvent::Forgotten {
        sequence,
        at_unix: now_unix(),
        before,
        content_hash,
    };
    let serialized = match serde_json::to_vec(&event) {
        Ok(serialized) => serialized,
        Err(source) => {
            return ForgetResult {
                status: ForgetStatus::StoreError(ForgetError::Serialize {
                    source: source.to_string(),
                }),
                entry_id,
            };
        }
    };
    match append_bytes(root, &serialized, durability) {
        Ok(()) => ForgetResult {
            status: ForgetStatus::Forgotten { sequence },
            entry_id,
        },
        Err(err) => ForgetResult {
            status: ForgetStatus::StoreError(err),
            entry_id,
        },
    }
}

/// List live (non-stale) entries as of `now_unix`, performing the lazy TTL
/// sweep. Entries whose TTL has elapsed are marked stale and the flip is
/// persisted (via a corrective `Admitted`-shaped event that re-states the
/// entry with `stale: true`); stale entries are excluded from the returned
/// list. No background thread (Redis passive-expiry model).
///
/// **Persistence note:** the sweep persists flipped `stale` flags by appending
/// a fresh `Admitted` event for each flipped entry with the updated freshness.
/// This keeps the log append-only (no in-place edits — rerun.io invariant) at
/// the cost of a re-admit-shaped record. A dedicated `FreshnessFlipped` event
/// variant is a future refinement once the CLI needs to distinguish them.
pub fn list_now(root: impl AsRef<Path>, now_unix: u64) -> ListResult {
    list_now_with_durability(root, now_unix, WalDurability::default())
}

/// As [`list_now`] with an explicit durability knob.
pub fn list_now_with_durability(
    root: impl AsRef<Path>,
    now_unix: u64,
    durability: WalDurability,
) -> ListResult {
    let root = root.as_ref();

    let _lock = match forge_core_store::acquire_effect_store_lock(root, MEMORY_LOCK_RELATIVE_PATH) {
        Ok(lock) => lock,
        Err(source) => {
            return ListResult {
                status: ListStatus::StoreError(ForgetError::Lock {
                    path: root.join(MEMORY_LOCK_RELATIVE_PATH),
                    source: source.to_string(),
                }),
            };
        }
    };

    let projection = match project_locked(root) {
        Ok(projection) => projection,
        Err(source) => {
            return ListResult {
                status: ListStatus::StoreError(ForgetError::Read {
                    path: root.join(MEMORY_LOG_RELATIVE_PATH),
                    source: source.to_string(),
                }),
            };
        }
    };

    // Materialize a MemoryContract to run the pure mark_stale sweep. The
    // projection's entries become the contract's entries; the sweep mutates
    // their freshness.stale flags in place.
    let mut contract_doc = projection_to_contract_doc(&projection);
    let before_stale_count = contract_doc
        .memory_contract
        .entries
        .iter()
        .filter(|e| e.freshness.stale)
        .count();
    contract_doc.memory_contract.mark_stale(now_unix);
    let after_stale_count = contract_doc
        .memory_contract
        .entries
        .iter()
        .filter(|e| e.freshness.stale)
        .count();
    let flipped = after_stale_count.saturating_sub(before_stale_count);

    // Persist each flipped entry by appending a fresh Admitted event with the
    // updated freshness (append-only; no in-place edits). Re-read the projection
    // sequence once; subsequent appends increment locally.
    if flipped > 0 {
        let mut seq = next_sequence(&projection);
        for entry in &contract_doc.memory_contract.entries {
            if entry.freshness.stale {
                let event = MemoryEvent::Admitted {
                    sequence: seq,
                    at_unix: now_unix,
                    entry: entry.clone(),
                };
                if let Ok(serialized) = serde_json::to_vec(&event) {
                    let value: serde_json::Value = match serde_json::from_slice(&serialized) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let _ = append_json_line_with_durability(
                        root,
                        MEMORY_LOG_RELATIVE_PATH,
                        &value,
                        durability,
                    );
                    seq = seq.saturating_add(1);
                }
            }
        }
    }

    // Return only non-stale entries.
    let entries = contract_doc
        .memory_contract
        .entries
        .into_iter()
        .filter(|e| !e.freshness.stale)
        .collect();

    ListResult {
        status: ListStatus::Ok { flipped, entries },
    }
}

/// Build a `MemoryContractDocument` from a projection so the pure `mark_stale`
/// can run over it. The contract id/scope are synthetic (the sweep does not
/// consult them); only the entries matter.
fn projection_to_contract_doc(projection: &MemoryProjection) -> MemoryContractDocument {
    MemoryContractDocument {
        schema_version: "0.1".into(),
        memory_contract: MemoryContract {
            id: StableId("memory.sweep.synthetic".into()),
            scope: MemoryScope {
                kind: MemoryScopeKind::Project,
                target: StableId("sweep".into()),
            },
            entries: projection.entries.values().cloned().collect(),
            superseded: projection
                .superseded
                .iter()
                .map(|s| StableId(s.clone()))
                .collect(),
        },
    }
}

fn append_bytes(
    root: &Path,
    serialized: &[u8],
    durability: WalDurability,
) -> Result<(), ForgetError> {
    let value: serde_json::Value =
        serde_json::from_slice(serialized).map_err(|source| ForgetError::Serialize {
            source: source.to_string(),
        })?;
    append_json_line_with_durability(root, MEMORY_LOG_RELATIVE_PATH, &value, durability).map_err(
        |source| ForgetError::Append {
            path: root.join(MEMORY_LOG_RELATIVE_PATH),
            source: source.to_string(),
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{admit, project, AdmissionStatus};
    use forge_core_contracts::{
        AdmissionEvidence, ApprovalState, Freshness as FreshnessType, MemoryEntry, MemoryKind,
        MemoryPolicy, MemoryProvenance,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("forge-memory-retention-{label}-{pid}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn permissive_policy() -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: vec![MemoryKind::Preference],
            required_evidence_fields: vec![],
            min_evidence_refs_for_authority: 1,
        }
    }

    /// An entry whose TTL has elapsed by `now`. `last_confirmed_at` = 100,
    /// `ttl_seconds` = 60 ⇒ expires at 160.
    fn stale_entry(id: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: StableId(id.into()),
            kind: MemoryKind::Preference,
            content: "stale".into(),
            provenance: MemoryProvenance {
                source_run_id: Some(StableId("run.1".into())),
                source_agent: Some(StableId("agent.1".into())),
                evidence_ref: Some("e".into()),
                captured_at: "100".into(),
            },
            freshness: FreshnessType {
                ttl_seconds: Some(60),
                last_confirmed_at: "100".into(),
                stale: false,
            },
            confidence: 80,
            approval: ApprovalState::Proposed,
            supersedes: None,
            invalidation_reason: None,
            authority_level: None,
            review_state: None,
            reviewed_by: None,
            reviewed_at: None,
        }
    }

    fn fresh_entry(id: &str) -> MemoryEntry {
        let mut e = stale_entry(id);
        e.freshness.ttl_seconds = Some(9_999_999_999);
        e
    }

    #[test]
    fn list_now_sweeps_elapsed_ttl_and_excludes_stale() {
        let root = temp_root("list-sweep");
        // Admit two entries directly via the PEP, then test the sweep.
        let r1 = admit(&root, stale_entry("e.stale"), &permissive_policy());
        let r2 = admit(&root, fresh_entry("e.fresh"), &permissive_policy());
        assert!(matches!(r1.status, AdmissionStatus::Admitted { .. }));
        assert!(matches!(r2.status, AdmissionStatus::Admitted { .. }));

        // now = 1000: the stale entry (expires at 160) is elapsed; fresh is not.
        let result = list_now(&root, 1000);
        let ListStatus::Ok { flipped, entries } = result.status else {
            panic!("expected Ok, got {:?}", result.status);
        };
        assert_eq!(flipped, 1, "exactly the stale entry flipped");
        assert_eq!(entries.len(), 1, "only the fresh entry remains");
        assert_eq!(entries[0].entry_id.0, "e.fresh");
    }

    #[test]
    fn list_now_with_no_ttl_entries_flips_nothing() {
        let root = temp_root("list-no-ttl");
        let mut no_ttl = fresh_entry("e.nottl");
        no_ttl.freshness.ttl_seconds = None;
        admit(&root, no_ttl, &permissive_policy());
        let result = list_now(&root, 1_000_000_000);
        let ListStatus::Ok { flipped, entries } = result.status else {
            panic!();
        };
        assert_eq!(flipped, 0, "no-TTL entries never flip");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn list_now_empty_store_returns_empty_ok() {
        let root = temp_root("list-empty");
        let result = list_now(&root, 1000);
        let ListStatus::Ok { flipped, entries } = result.status else {
            panic!();
        };
        assert_eq!(flipped, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn forget_appends_before_image_and_removes_entry() {
        let root = temp_root("forget-one");
        admit(&root, fresh_entry("e.one"), &permissive_policy());
        let result = forget(&root, StableId("e.one".into()));
        let ForgetStatus::Forgotten { sequence } = result.status else {
            panic!("expected Forgotten, got {:?}", result.status);
        };
        assert_eq!(sequence, 2, "admit=1, forget=2");
        let projection = project(&root).expect("project");
        assert!(projection.entries.is_empty(), "entry must be gone");
        assert!(projection.superseded.contains("e.one"));
    }

    #[test]
    fn forget_twice_is_already_forgotten_noop() {
        let root = temp_root("forget-twice");
        admit(&root, fresh_entry("e.one"), &permissive_policy());
        let r1 = forget(&root, StableId("e.one".into()));
        assert!(r1.is_forgotten());
        let seq_after_first = project(&root).expect("project").sequence;
        let r2 = forget(&root, StableId("e.one".into()));
        assert!(matches!(r2.status, ForgetStatus::AlreadyForgotten));
        let seq_after_second = project(&root).expect("project").sequence;
        assert_eq!(
            seq_after_first, seq_after_second,
            "idempotent forget must not append another event"
        );
    }

    #[test]
    fn forget_unknown_entry_is_not_found() {
        let root = temp_root("forget-missing");
        admit(&root, fresh_entry("e.present"), &permissive_policy());
        let result = forget(&root, StableId("e.absent".into()));
        assert!(matches!(result.status, ForgetStatus::NotFound));
    }

    #[test]
    fn list_then_forget_then_list_shows_removal() {
        let root = temp_root("list-forget-list");
        admit(&root, fresh_entry("e.a"), &permissive_policy());
        admit(&root, fresh_entry("e.b"), &permissive_policy());
        // Both present initially.
        let ListStatus::Ok {
            entries: before, ..
        } = list_now(&root, 100).status
        else {
            panic!();
        };
        assert_eq!(before.len(), 2);
        forget(&root, StableId("e.a".into()));
        let ListStatus::Ok { entries: after, .. } = list_now(&root, 100).status else {
            panic!();
        };
        assert_eq!(after.len(), 1, "forgotten entry excluded");
        assert_eq!(after[0].entry_id.0, "e.b");
    }

    // Suppress unused-import warning: AdmissionEvidence kept for parity with
    // the promote tests (this module may grow TTL+promote interaction tests).
    #[allow(dead_code)]
    fn _evidence_anchor() -> AdmissionEvidence {
        AdmissionEvidence {
            evidence_refs: vec![],
        }
    }
}
