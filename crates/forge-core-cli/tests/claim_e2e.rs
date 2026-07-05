//! Slice-4 governance end-to-end test — the multi-agent write-conflict loop.
//!
//! Exercises the layer-2 prevention promise (C5) as host agents would: agent A
//! acquires a scope declaring its write paths, then:
//! - agent A's write into its own path is ALLOWED (`governed_by_self`),
//! - agent B's write into A's path is BLOCKED (`WriteTargetClaimed`, exit 2),
//! - a write into an unclaimed path is BLOCKED until the writer owns a claim.
//!
//! This drives the REAL engine through the REAL CLI functions (`run_acquire` /
//! `run_check_write`) against on-disk claim files — no mocks. The acquisition is
//! serialized by the S4.4 lifecycle lock.

use forge_core_cli::claim::{run_acquire, run_check_write, run_release};
use forge_core_contracts::{
    claim::{ActorRole, ClaimScopeKind},
    RepoPath, ScopeId, StableId,
};
use forge_core_decisions::AcquireRequest;
use forge_core_store::WalDurability;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn tmp_claims_dir(label: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock must be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "forge-claim-e2e-{label}-{}-{timestamp_nanos}-{n}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn acquire_req(agent: &str, paths: &[&str]) -> AcquireRequest {
    AcquireRequest {
        scope_kind: ClaimScopeKind::Story,
        scope_id: ScopeId("S5.0".to_string()),
        agent_id: StableId(agent.to_string()),
        role: ActorRole::Worker,
        ttl_seconds: 600,
        heartbeat_interval_seconds: 120,
        paths: paths.iter().map(|p| RepoPath((*p).to_string())).collect(),
        product_area: None,
        expected_state_version: None,
    }
}

const NOW: i64 = 1_800_000_000;

#[test]
fn owner_write_into_own_claimed_path_is_allowed() {
    let dir = tmp_claims_dir("owner");
    let req = acquire_req("alice", &["contracts/stories/S5.0.yaml"]);
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);

    let check = run_check_write(
        &dir,
        &StableId("alice".to_string()),
        &["contracts/stories/S5.0.yaml".to_string()],
        NOW,
    );
    assert!(check.ok, "owner writing own path must be allowed");
    // Payload reports the write as governed by the writer's own claim.
    assert!(check.data.unwrap().governed_by_self.len() == 1);
}

#[test]
fn peer_write_into_claimed_path_is_blocked() {
    let dir = tmp_claims_dir("peer");
    let req = acquire_req("alice", &["contracts/stories/S5.0.yaml"]);
    let _ = run_acquire(&dir, &req, NOW, WalDurability::NoSync);

    // Bob is NOT alice and did not acquire anything.
    let check = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &["contracts/stories/S5.0.yaml".to_string()],
        NOW,
    );
    // Layer-2 prevention: blocked with exit 2 (RejectedByGate).
    assert!(!check.ok);
    assert_eq!(check.exit_code(), 2);
    assert_eq!(
        check.exit_reason.0,
        forge_core_contracts::ExitReason::RejectedByGate.as_str()
    );
    // M1: the structured payload is present even on rejection, so the writer
    // can self-correct programmatically (not just by parsing the message).
    let payload = check
        .data
        .expect("blocked verdict must carry structured data");
    assert!(!payload.allowed);
    assert_eq!(payload.blocks.len(), 1);
    assert_eq!(payload.blocks[0].claimant, "alice");
    assert_eq!(payload.blocks[0].conflict_code, "write_target_claimed");
    // M2: the blocked path echoes the EXACT target the writer submitted.
    assert_eq!(
        payload.blocks[0].blocked_path,
        "contracts/stories/S5.0.yaml"
    );
}

#[test]
fn write_into_unclaimed_path_is_blocked_until_claimed() {
    let dir = tmp_claims_dir("ungov");
    let req = acquire_req("alice", &["contracts/stories/S5.0.yaml"]);
    let _ = run_acquire(&dir, &req, NOW, WalDurability::NoSync);

    // A totally different path is governed by no claim.
    let check = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &["docs/unrelated.md".to_string()],
        NOW,
    );
    assert!(!check.ok);
    assert_eq!(check.exit_code(), 2);
    let payload = check.data.unwrap();
    assert!(!payload.allowed);
    assert_eq!(payload.ungoverned.len(), 1);
    assert!(payload.governed_by_self.is_empty());
    assert!(payload.blocks.is_empty());
}

#[test]
fn two_agents_non_overlapping_claims_both_write_freely() {
    // The multi-agent promise: two agents claim DISJOINT scopes and both can
    // write into their own paths without interfering. This is the C5 scenario.
    let dir = tmp_claims_dir("disjoint");

    let alice_req = AcquireRequest {
        scope_id: ScopeId("S5.0".to_string()),
        ..acquire_req("alice", &["src/feature_a.rs"])
    };
    let bob_req = AcquireRequest {
        scope_id: ScopeId("S5.1".to_string()),
        ..acquire_req("bob", &["src/feature_b.rs"])
    };
    let a = run_acquire(&dir, &alice_req, NOW, WalDurability::NoSync);
    let b = run_acquire(&dir, &bob_req, NOW, WalDurability::NoSync);
    assert!(a.ok && b.ok, "both disjoint acquires must succeed");

    // Each agent writes its own file — neither is blocked by the other.
    let alice_check = run_check_write(
        &dir,
        &StableId("alice".to_string()),
        &["src/feature_a.rs".to_string()],
        NOW,
    );
    let bob_check = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &["src/feature_b.rs".to_string()],
        NOW,
    );
    assert!(alice_check.ok);
    assert!(bob_check.ok);

    // Cross-write is blocked both ways.
    let alice_into_bob = run_check_write(
        &dir,
        &StableId("alice".to_string()),
        &["src/feature_b.rs".to_string()],
        NOW,
    );
    assert!(!alice_into_bob.ok, "alice must not write bob's path");
}

#[test]
fn released_claim_no_longer_blocks_peer_but_write_still_requires_claim() {
    // After alice releases, her path becomes ungoverned: bob is no longer
    // blocked by alice, but still must acquire his own claim before writing.
    let dir = tmp_claims_dir("released");
    let req = acquire_req("alice", &["contracts/stories/S5.0.yaml"]);
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    let claim_id = match acquired.data {
        Some(d) => d.claim_id,
        None => panic!("expected claim id"),
    };
    let _ = run_release(
        &dir,
        &StableId(claim_id),
        &StableId("alice".to_string()),
        NOW,
        WalDurability::NoSync,
    );

    let check = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &["contracts/stories/S5.0.yaml".to_string()],
        NOW,
    );
    assert!(
        !check.ok,
        "released claim should be ungoverned until bob claims it"
    );
    let payload = check.data.expect("ungoverned rejection carries payload");
    assert!(payload.blocks.is_empty(), "alice no longer blocks bob");
    assert_eq!(
        payload.ungoverned,
        vec!["contracts/stories/S5.0.yaml".to_string()]
    );

    let bob_req = AcquireRequest {
        scope_id: ScopeId("S5.1".to_string()),
        ..acquire_req("bob", &["contracts/stories/S5.0.yaml"])
    };
    let bob_claim = run_acquire(&dir, &bob_req, NOW + 1, WalDurability::NoSync);
    assert!(
        bob_claim.ok,
        "bob should be able to claim released path: {:?}",
        bob_claim.error
    );
    let after_bob_claim = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &["contracts/stories/S5.0.yaml".to_string()],
        NOW + 1,
    );
    assert!(
        after_bob_claim.ok,
        "bob can write after acquiring his own claim: {:?}",
        after_bob_claim.error
    );
}
