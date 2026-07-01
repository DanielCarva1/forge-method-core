//! F06.8 — PEP-level integration tests for `forge-core-memory`.
//!
//! These tests drive the PEP API directly (not the CLI binary) to pin the
//! invariants that are awkward to assert over stdout: that a denial appends
//! nothing to the log, that forget is idempotent, that the before-image
//! `content_hash` is tamper-evident, and that the projection replays
//! deterministically after each operation. Complements the binary-level
//! `memory_cli_e2e.rs`.

use forge_core_contracts::{
    AdmissionDenialReason, AdmissionEvidence, ApprovalState, AuthorityLevel, EvidenceField,
    Freshness, MemoryEntry, MemoryKind, MemoryPolicy, MemoryProvenance, StableId,
};
use forge_core_memory::{
    admit, forget, list_now, project, promote, AdmissionStatus, ForgetStatus, PromoteStatus,
};
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("forge-memory-pep-{label}-{pid}-{nanos}"));
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn permissive_policy() -> MemoryPolicy {
    MemoryPolicy {
        permitted_kinds: vec![MemoryKind::Preference],
        required_evidence_fields: vec![EvidenceField::ProvenanceEvidenceRef],
        min_evidence_refs_for_authority: 1,
    }
}

fn deny_all_policy() -> MemoryPolicy {
    MemoryPolicy {
        permitted_kinds: vec![],
        required_evidence_fields: vec![],
        min_evidence_refs_for_authority: 1,
    }
}

fn entry(id: &str) -> MemoryEntry {
    MemoryEntry {
        entry_id: StableId(id.into()),
        kind: MemoryKind::Preference,
        content: "c".into(),
        provenance: MemoryProvenance {
            source_run_id: Some(StableId("run.1".into())),
            source_agent: Some(StableId("agent.1".into())),
            evidence_ref: Some("e.yaml".into()),
            captured_at: "1".into(),
        },
        freshness: Freshness {
            ttl_seconds: None,
            last_confirmed_at: "1".into(),
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

/// A denied admission appends NOTHING to the log and does not consume a
/// sequence number. (ADR-0002: the PEP never writes on a denial.)
#[test]
fn denied_admission_appends_nothing_and_consumes_no_sequence() {
    let root = temp_root("deny-noop");
    let denied = admit(&root, entry("e.denied"), &deny_all_policy());
    assert!(matches!(denied.status, AdmissionStatus::DeniedByGate(_)));
    // The log file does not exist.
    assert!(!root.join("memory/events.ndjson").exists());
    // A subsequent allowed admit is sequence 1 (the denial did not consume one).
    let allowed = admit(&root, entry("e.ok"), &permissive_policy());
    let AdmissionStatus::Admitted { sequence } = allowed.status else {
        panic!("expected Admitted: {:?}", allowed.status);
    };
    assert_eq!(sequence, 1);
}

/// The denial reasons carry the correct typed variant (`KindNotPermitted` for
/// the deny-all policy), so a caller can branch on the cause.
#[test]
fn denied_admission_carries_typed_reason() {
    let root = temp_root("deny-reason");
    let result = admit(&root, entry("e.x"), &deny_all_policy());
    match result.status {
        AdmissionStatus::DeniedByGate(reasons) => {
            assert!(
                reasons
                    .iter()
                    .any(|r| { matches!(r, AdmissionDenialReason::KindNotPermitted) }),
                "deny-all policy must yield KindNotPermitted: {reasons:?}"
            );
        }
        other => panic!("expected DeniedByGate, got {other:?}"),
    }
}

/// Idempotent forget: a second forget of the same id is `AlreadyForgotten` and
/// does NOT append another event (the before-image is logged exactly once).
#[test]
fn forget_is_idempotent_second_call_appends_nothing() {
    let root = temp_root("idempotent-forget");
    admit(&root, entry("e.one"), &permissive_policy());
    let r1 = forget(&root, StableId("e.one".into()));
    assert!(matches!(r1.status, ForgetStatus::Forgotten { .. }));
    let seq_after_first = project(&root).expect("project").sequence;
    let r2 = forget(&root, StableId("e.one".into()));
    assert!(matches!(r2.status, ForgetStatus::AlreadyForgotten));
    let seq_after_second = project(&root).expect("project").sequence;
    assert_eq!(
        seq_after_first, seq_after_second,
        "second forget must not append"
    );
}

/// The forget before-image `content_hash` is tamper-evident: replaying the log
/// yields a projection whose forgotten entry's hash matches what the event
/// recorded. (A manual edit to the log would change the entry but not the
/// recorded hash — the divergence is detectable.)
#[test]
fn forget_before_image_hash_matches_replayed_entry() {
    let root = temp_root("before-image-hash");
    let before = entry("e.h");
    let expected_hash = forge_core_memory::MemoryEvent::content_hash_of(&before);
    admit(&root, before, &permissive_policy());
    let r = forget(&root, StableId("e.h".into()));
    let ForgetStatus::Forgotten { .. } = r.status else {
        panic!("expected Forgotten: {:?}", r.status);
    };
    // Read the raw log and find the Forgotten event; its content_hash must
    // match the hash of the entry as admitted.
    let log = std::fs::read_to_string(root.join("memory/events.ndjson")).expect("read log");
    let forgotten = log
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse event"))
        .find(|v| v.get("Forgotten").is_some())
        .expect("a Forgotten event exists");
    let recorded_hash = forgotten["Forgotten"]["content_hash"]
        .as_str()
        .expect("content_hash");
    assert_eq!(
        recorded_hash, expected_hash,
        "the recorded before-image hash must match content_hash_of"
    );
    assert!(recorded_hash.starts_with("sha256:"));
}

/// Promote never touches the review axis: after a successful promote, the
/// entry's `review_state`/`reviewed_by`/`reviewed_at` are still at the admission
/// floor. (The Model-B-back-door guard, ADR-0002.)
#[test]
fn promote_leaves_review_axis_untouched() {
    let root = temp_root("promote-no-review");
    admit(&root, entry("e.p"), &permissive_policy());
    let r = promote(
        &root,
        StableId("e.p".into()),
        &permissive_policy(),
        &AdmissionEvidence {
            evidence_refs: vec!["run.alpha".into()],
        },
    );
    assert!(matches!(r.status, PromoteStatus::Promoted { .. }));
    let projection = project(&root).expect("project");
    let entry = &projection.entries["e.p"];
    assert_eq!(entry.authority_level, Some(AuthorityLevel::Authority));
    assert_eq!(entry.review_state, None);
    assert_eq!(entry.reviewed_by, None);
    assert_eq!(entry.reviewed_at, None);
}

/// Promote with insufficient evidence is denied and appends nothing.
#[test]
fn promote_without_evidence_is_denied_and_appends_nothing() {
    let root = temp_root("promote-denied");
    admit(&root, entry("e.p"), &permissive_policy());
    let seq_before = project(&root).expect("project").sequence;
    let r = promote(
        &root,
        StableId("e.p".into()),
        &permissive_policy(),
        &AdmissionEvidence {
            evidence_refs: vec![],
        },
    );
    match r.status {
        PromoteStatus::DeniedByGate(reasons) => {
            assert!(
                reasons
                    .iter()
                    .any(|r| matches!(r, AdmissionDenialReason::InsufficientEvidenceForAuthority)),
                "expected InsufficientEvidenceForAuthority: {reasons:?}"
            );
        }
        other => panic!("expected DeniedByGate: {other:?}"),
    }
    let seq_after = project(&root).expect("project").sequence;
    assert_eq!(seq_before, seq_after, "denied promote must not append");
    // Authority unchanged.
    assert_eq!(
        project(&root).expect("project").entries["e.p"].authority_level,
        None
    );
}

/// The lazy TTL sweep persists flipped stale flags so a second read is stable
/// (the sweep is idempotent — re-reading after the flip does not re-flip).
#[test]
fn list_sweep_is_idempotent_across_reads() {
    let root = temp_root("sweep-idempotent");
    let mut e = entry("e.ttl");
    e.freshness.ttl_seconds = Some(60);
    e.freshness.last_confirmed_at = "100".into();
    admit(&root, e, &permissive_policy());
    // First read at now=1000 flips it stale.
    let r1 = list_now(&root, 1000);
    let (flipped1, live1) = match r1.status {
        forge_core_memory::ListStatus::Ok { flipped, entries } => (flipped, entries.len()),
        forge_core_memory::ListStatus::StoreError(err) => panic!("expected Ok: {err:?}"),
    };
    assert_eq!(flipped1, 1);
    assert_eq!(live1, 0);
    // Second read at the same now — nothing new to flip.
    let r2 = list_now(&root, 1000);
    let (flipped2, live2) = match r2.status {
        forge_core_memory::ListStatus::Ok { flipped, entries } => (flipped, entries.len()),
        forge_core_memory::ListStatus::StoreError(err) => panic!("expected Ok: {err:?}"),
    };
    assert_eq!(flipped2, 0, "idempotent sweep flips nothing on re-read");
    assert_eq!(live2, 0);
}

/// The projection is rebuildable from the log: after N operations, projecting
/// from scratch yields the same state as the live store. (Fowler replay.)
#[test]
fn projection_replays_deterministically_after_mixed_operations() {
    let root = temp_root("replay-mixed");
    admit(&root, entry("e.a"), &permissive_policy());
    admit(&root, entry("e.b"), &permissive_policy());
    promote(
        &root,
        StableId("e.a".into()),
        &permissive_policy(),
        &AdmissionEvidence {
            evidence_refs: vec!["r1".into()],
        },
    );
    forget(&root, StableId("e.b".into()));
    let p1 = project(&root).expect("project");
    // Re-project (cold read again) — must be identical.
    let p2 = project(&root).expect("project");
    assert_eq!(p1, p2);
    // e.a is Authority, e.b is forgotten.
    assert_eq!(
        p1.entries["e.a"].authority_level,
        Some(AuthorityLevel::Authority)
    );
    assert!(!p1.entries.contains_key("e.b"));
    assert!(p1.superseded.contains("e.b"));
    assert_eq!(p1.sequence, 4, "admit,admit,promote,forget = seq 4");
}
