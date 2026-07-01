//! Claim lifecycle integration tests (Q04).
//!
//! Mirrors the temp-dir + request harness in `claim_e2e.rs`, but focuses on the
//! complete lifecycle surface: acquire, conflict, heartbeat, release, expiry, and
//! pure write-conflict checks over the persisted claims bus.

use forge_core_cli::claim::{
    load_claims, run_acquire, run_check_write, run_handoff, run_heartbeat, run_reconcile_once,
    run_release, run_status, save_claim,
};
use forge_core_contracts::{
    claim::{ActorRole, ClaimScopeKind, ClaimStatus},
    ClaimId, ExitReason, RepoPath, ScopeId, StableId,
};
use forge_core_engine::{
    check_write_against_claims, expire_stale, is_expired, is_live, project_active, unix_to_rfc3339,
    AcquireRequest, WriteCheck,
};
use forge_core_store::claim_wal::{claim_wal_path, recover_claim_wal, ClaimWalOperation};
use forge_core_store::WalDurability;
use std::fs;
use std::path::PathBuf;

const NOW: i64 = 1_800_000_000;
const TTL_SECONDS: u64 = 600;
const HEARTBEAT_INTERVAL_SECONDS: u64 = 120;

fn tmp_claims_dir(label: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "forge-claim-lifecycle-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn acquire_req(agent: &str, scope_id: &str, paths: &[&str]) -> AcquireRequest {
    AcquireRequest {
        scope_kind: ClaimScopeKind::Story,
        scope_id: ScopeId(scope_id.to_string()),
        agent_id: StableId(agent.to_string()),
        role: ActorRole::Worker,
        ttl_seconds: TTL_SECONDS,
        heartbeat_interval_seconds: HEARTBEAT_INTERVAL_SECONDS,
        paths: paths.iter().map(|p| RepoPath((*p).to_string())).collect(),
        product_area: None,
        expected_state_version: None,
    }
}

fn load_one_claim(dir: &std::path::Path) -> forge_core_contracts::claim::ClaimContract {
    let (claims, errors) = load_claims(dir);
    assert!(
        errors.is_empty(),
        "claims dir must load cleanly: {errors:?}"
    );
    assert_eq!(claims.len(), 1, "expected exactly one persisted claim");
    claims.into_iter().next().expect("one claim")
}

fn claim_id_from_acquire(
    agent: &str,
    dir: &std::path::Path,
    scope_id: &str,
    path: &str,
) -> StableId {
    let acquired = run_acquire(
        dir,
        &acquire_req(agent, scope_id, &[path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let claim_id = acquired
        .data
        .expect("successful acquire must carry a claim result")
        .claim_id;
    StableId(claim_id)
}

fn short_ttl_claim_id_from_acquire(
    agent: &str,
    dir: &std::path::Path,
    scope_id: &str,
    path: &str,
) -> StableId {
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req(agent, scope_id, &[path])
    };
    let acquired = run_acquire(dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    StableId(acquired.data.expect("successful acquire data").claim_id)
}

#[test]
fn acquire_success_persists_active_live_claim() {
    let dir = tmp_claims_dir("acquire");
    let req = acquire_req("alice", "S6.1", &["contracts/stories/S6.1.yaml"]);

    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);

    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    assert_eq!(acquired.exit_code(), ExitReason::Ok.as_code());
    let payload = acquired.data.expect("ok acquire must carry data");
    assert_eq!(payload.agent_id, "alice");
    assert_eq!(payload.scope_id, "S6.1");
    assert_eq!(payload.status, "active");

    let claim = load_one_claim(&dir);
    assert_eq!(claim.id, ClaimId("claim.story.S6.1.S6.1".to_string()));
    assert_eq!(claim.claim.claimant_agent_id, StableId("alice".to_string()));
    assert_eq!(
        claim.scope.paths,
        vec![RepoPath("contracts/stories/S6.1.yaml".to_string())]
    );
    assert!(is_live(&claim, NOW), "freshly acquired claim must be live");
}

#[test]
fn conflicting_acquire_same_path_and_scope_is_rejected() {
    let dir = tmp_claims_dir("conflict");
    let path = "contracts/stories/S6.2.yaml";
    let first = run_acquire(
        &dir,
        &acquire_req("alice", "S6.2", &[path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(first.ok, "first acquire should succeed: {:?}", first.error);

    let second = run_acquire(
        &dir,
        &acquire_req("bob", "S6.2", &[path]),
        NOW + 1,
        WalDurability::NoSync,
    );

    assert!(!second.ok, "second acquire must be rejected");
    assert_eq!(second.exit_code(), ExitReason::RejectedByGate.as_code());
    let err = second.error.expect("rejected acquire must carry an error");
    assert!(
        err.code.0.starts_with("already_claimed_by_other:"),
        "expected conflict rejection code, got {}",
        err.code.0
    );
    assert!(err.message.contains("AlreadyClaimedByOther"));
}

#[test]
fn heartbeat_refreshes_last_heartbeat_and_extends_expiry() {
    let dir = tmp_claims_dir("heartbeat");
    let claim_id = claim_id_from_acquire("alice", &dir, "S6.3", "contracts/stories/S6.3.yaml");
    let before = load_one_claim(&dir);
    let heartbeat_at = NOW + 60;

    let heartbeat = run_heartbeat(
        &dir,
        &claim_id,
        &StableId("alice".to_string()),
        heartbeat_at,
        WalDurability::NoSync,
    );

    assert!(
        heartbeat.ok,
        "heartbeat should succeed: {:?}",
        heartbeat.error
    );
    let after = load_one_claim(&dir);
    assert_eq!(after.lease.last_heartbeat_at, unix_to_rfc3339(heartbeat_at));
    assert_eq!(
        after.lease.expires_at,
        unix_to_rfc3339(heartbeat_at + i64::try_from(TTL_SECONDS).expect("ttl fits i64"))
    );
    assert_ne!(
        before.lease.last_heartbeat_at,
        after.lease.last_heartbeat_at
    );
    assert_ne!(before.lease.expires_at, after.lease.expires_at);
    assert!(is_live(&after, heartbeat_at + 1));
}

#[test]
fn release_transitions_claim_out_of_live_bus() {
    let dir = tmp_claims_dir("release");
    let claim_id = claim_id_from_acquire("alice", &dir, "S6.4", "contracts/stories/S6.4.yaml");

    let released = run_release(
        &dir,
        &claim_id,
        &StableId("alice".to_string()),
        NOW + 10,
        WalDurability::NoSync,
    );

    assert!(released.ok, "release should succeed: {:?}", released.error);
    assert_eq!(released.data.expect("release data").status, "released");
    let claim = load_one_claim(&dir);
    assert_eq!(claim.status.value, ClaimStatus::Released);
    assert!(!is_live(&claim, NOW + 11));
    let active = project_active(&[claim], NOW + 11);
    assert!(
        active.active.is_empty(),
        "released claim must leave active bus"
    );
}

#[test]
fn expired_claim_is_not_live_and_expiry_sweep_reports_handoff_required() {
    let dir = tmp_claims_dir("stale");
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "S6.5", &["contracts/stories/S6.5.yaml"])
    };
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let claim = load_one_claim(&dir);
    let after_expiry = NOW + 10;

    assert!(is_expired(&claim, after_expiry));
    assert!(!is_live(&claim, after_expiry));
    let active = project_active(std::slice::from_ref(&claim), after_expiry);
    assert!(
        active.active.is_empty(),
        "expired claim must not project as live"
    );

    let expired = expire_stale(std::slice::from_ref(&claim), after_expiry);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].claim_id, claim.id);
    assert_eq!(expired[0].transitioned_to, ClaimStatus::HandoffRequired);
}

#[test]
fn claim_status_reports_expired_handoff_required_claims() {
    let dir = tmp_claims_dir("status-expired-handoff");
    let path = "contracts/stories/S6.5-status.yaml";
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "S6.5-status", &[path])
    };
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let claim_id = acquired.data.expect("acquire data").claim_id;

    let status = run_status(&dir, NOW + 10);

    assert!(status.ok, "status should succeed: {:?}", status.error);
    let view = status.data.expect("status data");
    assert!(
        view.active.is_empty(),
        "expired claim must not be reported as active"
    );
    assert_eq!(view.expired_handoff_required.len(), 1);
    let blocker = &view.expired_handoff_required[0];
    assert_eq!(blocker.claim_id, claim_id);
    assert_eq!(blocker.scope_kind, "story");
    assert_eq!(blocker.scope_id, "S6.5-status");
    assert_eq!(blocker.agent_id, "alice");
    assert_eq!(blocker.paths, vec![path.to_string()]);
    assert_eq!(blocker.blocker_reason, "expired_requires_handoff");
    assert_eq!(blocker.status, "active");
    assert!(
        blocker
            .handoff_request_ref
            .as_deref()
            .is_some_and(|hint| hint.contains("claim-expiry-handoff-request")),
        "status should expose the configured handoff request ref: {blocker:?}"
    );
    assert!(
        blocker.handoff_hint.contains("forge-core claim handoff"),
        "status should include an actionable handoff hint: {blocker:?}"
    );
}

#[test]
fn claim_status_preserves_active_claims_while_reporting_handoff_blockers() {
    let dir = tmp_claims_dir("status-active-plus-blocker");
    let expired_path = "contracts/stories/S6.5-expired-status.yaml";
    let active_path = "contracts/stories/S6.5-active-status.yaml";
    let expired_req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "S6.5-expired-status", &[expired_path])
    };
    let expired = run_acquire(&dir, &expired_req, NOW, WalDurability::NoSync);
    assert!(
        expired.ok,
        "expired acquire should succeed: {:?}",
        expired.error
    );
    let active = run_acquire(
        &dir,
        &acquire_req("bob", "S6.5-active-status", &[active_path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(
        active.ok,
        "active acquire should succeed: {:?}",
        active.error
    );

    let status = run_status(&dir, NOW + 10);

    assert!(status.ok, "status should succeed: {:?}", status.error);
    let view = status.data.expect("status data");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].scope_id, "S6.5-active-status");
    assert_eq!(view.active[0].paths, vec![active_path.to_string()]);
    assert_eq!(view.expired_handoff_required.len(), 1);
    assert_eq!(
        view.expired_handoff_required[0].scope_id,
        "S6.5-expired-status"
    );
    assert_eq!(
        view.expired_handoff_required[0].blocker_reason,
        "expired_requires_handoff"
    );
}

#[test]
fn claim_status_reports_only_unresolved_handoff_blocker_statuses() {
    let dir = tmp_claims_dir("status-handoff-statuses");
    let _stale_id = short_ttl_claim_id_from_acquire(
        "alice",
        &dir,
        "S6.5-stale",
        "contracts/stories/S6.5-stale.yaml",
    );
    let _second_expired_id = short_ttl_claim_id_from_acquire(
        "bob",
        &dir,
        "S6.5-second-expired",
        "contracts/stories/S6.5-second-expired.yaml",
    );
    let released_id = short_ttl_claim_id_from_acquire(
        "cara",
        &dir,
        "S6.5-released",
        "contracts/stories/S6.5-released.yaml",
    );
    let handoff_recorded_id = short_ttl_claim_id_from_acquire(
        "drew",
        &dir,
        "S6.5-handoff-recorded",
        "contracts/stories/S6.5-handoff-recorded.yaml",
    );
    let released = run_release(
        &dir,
        &released_id,
        &StableId("cara".to_string()),
        NOW + 1,
        WalDurability::NoSync,
    );
    assert!(released.ok, "release should succeed: {:?}", released.error);
    let handoff = run_handoff(
        &dir,
        &handoff_recorded_id,
        &StableId("drew".to_string()),
        "worker crashed; recovery evidence recorded",
        &[],
        NOW + 10,
        WalDurability::NoSync,
    );
    assert!(handoff.ok, "handoff should succeed: {:?}", handoff.error);

    let status = run_status(&dir, NOW + 10);

    assert!(status.ok, "status should succeed: {:?}", status.error);
    let view = status.data.expect("status data");
    assert!(
        view.active.is_empty(),
        "expired/materialized handoff claims must not be reported active"
    );
    assert_eq!(view.expired_handoff_required.len(), 2);
    let blockers: Vec<(&str, &str)> = view
        .expired_handoff_required
        .iter()
        .map(|claim| (claim.scope_id.as_str(), claim.blocker_reason.as_str()))
        .collect();
    assert!(
        blockers.contains(&("S6.5-stale", "expired_requires_handoff")),
        "expired stale claim should require handoff: {blockers:?}"
    );
    assert!(
        blockers.contains(&("S6.5-second-expired", "expired_requires_handoff")),
        "second expired active claim should require handoff: {blockers:?}"
    );
    let blocker_ids: Vec<&str> = view
        .expired_handoff_required
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect();
    assert!(
        !blocker_ids.contains(&released_id.0.as_str()),
        "released claims must not remain handoff blockers: {blocker_ids:?}"
    );
    assert!(
        !blocker_ids.contains(&handoff_recorded_id.0.as_str()),
        "handoff_recorded claims must not remain handoff blockers: {blocker_ids:?}"
    );
}

#[test]
fn reconcile_once_noops_before_heartbeat_deadline() {
    let dir = tmp_claims_dir("reconcile-before-deadline");
    let path = "contracts/stories/P2.3-before-deadline.yaml";
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "P2.3-before-deadline", &[path])
    };
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);

    let reconciled = run_reconcile_once(&dir, NOW + 4, WalDurability::NoSync);

    assert!(
        reconciled.ok,
        "reconcile should succeed: {:?}",
        reconciled.error
    );
    let data = reconciled.data.expect("reconcile data");
    assert_eq!(data.scanned, 1);
    assert_eq!(data.changed, 0);
    assert!(data.transitions.is_empty());
    let recovery = recover_claim_wal(&dir, false).expect("recover WAL");
    assert_eq!(recovery.records.len(), 1);
    assert_eq!(recovery.records[0].operation, ClaimWalOperation::Acquire);
}

#[test]
fn reconcile_once_materializes_stale_and_stale_remains_write_authority() {
    let dir = tmp_claims_dir("reconcile-stale");
    let path = "contracts/stories/P2.3-stale.yaml";
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "P2.3-stale", &[path])
    };
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);

    let reconciled = run_reconcile_once(&dir, NOW + 5, WalDurability::NoSync);

    assert!(
        reconciled.ok,
        "reconcile should succeed: {:?}",
        reconciled.error
    );
    let data = reconciled.data.expect("reconcile data");
    assert_eq!(data.changed, 1);
    assert_eq!(data.transitions[0].from, "active");
    assert_eq!(data.transitions[0].to, "stale");
    assert_eq!(data.transitions[0].reason_code, "heartbeat_overdue");

    let status = run_status(&dir, NOW + 6);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let view = status.data.expect("status data");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].scope_id, "P2.3-stale");
    assert_eq!(view.active[0].status, "stale");
    assert!(
        view.expired_handoff_required.is_empty(),
        "stale but unexpired claim is not a handoff blocker yet"
    );

    let peer_write = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &[path.to_string()],
        NOW + 6,
    );
    assert!(!peer_write.ok, "stale claims must still block peer writes");
    assert_eq!(peer_write.exit_code(), ExitReason::RejectedByGate.as_code());

    let self_write = run_check_write(
        &dir,
        &StableId("alice".to_string()),
        &[path.to_string()],
        NOW + 6,
    );
    assert!(
        self_write.ok,
        "stale claims must still authorize the claimant until expiry: {:?}",
        self_write.error
    );

    let second = run_reconcile_once(&dir, NOW + 6, WalDurability::NoSync);
    assert!(
        second.ok,
        "second reconcile should succeed: {:?}",
        second.error
    );
    assert_eq!(
        second.data.expect("second reconcile data").changed,
        0,
        "stale materialization must be idempotent"
    );

    let recovery = recover_claim_wal(&dir, false).expect("recover WAL");
    let operations: Vec<_> = recovery
        .records
        .iter()
        .map(|record| record.operation)
        .collect();
    assert_eq!(
        operations,
        vec![
            ClaimWalOperation::Acquire,
            ClaimWalOperation::ReconcileStatus,
        ]
    );
}

#[test]
fn reconcile_once_materializes_handoff_required_and_preserves_recovery_path() {
    let dir = tmp_claims_dir("reconcile-handoff-required");
    let path = "contracts/stories/P2.3-handoff.yaml";
    let req = AcquireRequest {
        ttl_seconds: 10,
        heartbeat_interval_seconds: 5,
        ..acquire_req("alice", "P2.3-handoff", &[path])
    };
    let acquired = run_acquire(&dir, &req, NOW, WalDurability::NoSync);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let claim_id = acquired.data.expect("acquire data").claim_id;

    let stale = run_reconcile_once(&dir, NOW + 5, WalDurability::NoSync);
    assert!(
        stale.ok,
        "stale reconcile should succeed: {:?}",
        stale.error
    );
    let expired = run_reconcile_once(&dir, NOW + 10, WalDurability::NoSync);

    assert!(
        expired.ok,
        "expired reconcile should succeed: {:?}",
        expired.error
    );
    let data = expired.data.expect("expired reconcile data");
    assert_eq!(data.changed, 1);
    assert_eq!(data.transitions[0].from, "stale");
    assert_eq!(data.transitions[0].to, "handoff_required");
    assert_eq!(data.transitions[0].reason_code, "lease_expired");

    let status = run_status(&dir, NOW + 10);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let view = status.data.expect("status data");
    assert!(view.active.is_empty());
    assert_eq!(view.expired_handoff_required.len(), 1);
    assert_eq!(view.expired_handoff_required[0].claim_id, claim_id);
    assert_eq!(
        view.expired_handoff_required[0].blocker_reason,
        "handoff_required"
    );
    assert_eq!(view.expired_handoff_required[0].status, "handoff_required");

    let heartbeat = run_heartbeat(
        &dir,
        &StableId(claim_id.clone()),
        &StableId("alice".to_string()),
        NOW + 10,
        WalDurability::NoSync,
    );
    assert!(!heartbeat.ok, "handoff_required heartbeat must fail closed");
    assert!(heartbeat
        .error
        .as_ref()
        .expect("heartbeat error")
        .code
        .0
        .starts_with("expired_requires_handoff"));
    assert!(heartbeat
        .error
        .as_ref()
        .expect("heartbeat error")
        .message
        .contains("forge-core claim handoff"));

    let handoff = run_handoff(
        &dir,
        &StableId(claim_id.clone()),
        &StableId("driver".to_string()),
        "expired claim reconciled; handoff evidence recorded",
        &[],
        NOW + 11,
        WalDurability::NoSync,
    );
    assert!(handoff.ok, "handoff should recover: {:?}", handoff.error);
    let reacquire = run_acquire(
        &dir,
        &acquire_req("bob", "P2.3-handoff", &[path]),
        NOW + 12,
        WalDurability::NoSync,
    );
    assert!(
        reacquire.ok,
        "handoff_recorded claim must not block reacquire: {:?}",
        reacquire.error
    );

    let recovery = recover_claim_wal(&dir, false).expect("recover WAL");
    let operations: Vec<_> = recovery
        .records
        .iter()
        .map(|record| record.operation)
        .collect();
    assert_eq!(
        operations,
        vec![
            ClaimWalOperation::Acquire,
            ClaimWalOperation::ReconcileStatus,
            ClaimWalOperation::ReconcileStatus,
            ClaimWalOperation::HandoffRecorded,
            ClaimWalOperation::Acquire,
        ]
    );
}

#[test]
fn reconcile_once_cache_only_without_wal_fails_closed() {
    let dir = tmp_claims_dir("reconcile-cache-only-no-wal");
    let acquired = run_acquire(
        &dir,
        &acquire_req(
            "alice",
            "P2.3-cache-only",
            &["contracts/stories/P2.3-cache-only.yaml"],
        ),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    fs::remove_file(claim_wal_path(&dir)).expect("remove authoritative WAL");

    let reconciled = run_reconcile_once(&dir, NOW + 1, WalDurability::NoSync);

    assert!(!reconciled.ok, "cache-only state must fail closed");
    assert_eq!(reconciled.exit_code(), ExitReason::EnvConfig.as_code());
    let error = reconciled.error.expect("env config error");
    assert!(
        error.message.contains("authoritative WAL") && error.message.contains("missing"),
        "error should explain missing WAL authority: {}",
        error.message
    );
}

#[test]
fn check_write_denies_peer_claimed_path_and_allows_unclaimed_path() {
    let dir = tmp_claims_dir("check-write");
    let path = "contracts/stories/S6.6.yaml";
    let acquired = run_acquire(
        &dir,
        &acquire_req("alice", "S6.6", &[path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let (claims, errors) = load_claims(&dir);
    assert!(
        errors.is_empty(),
        "claims dir must load cleanly: {errors:?}"
    );

    let blocked = check_write_against_claims(
        &[RepoPath(path.to_string())],
        &StableId("bob".to_string()),
        &claims,
        NOW + 1,
    );
    match blocked {
        WriteCheck::Blocked { blocks } => {
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].blocked_path, RepoPath(path.to_string()));
            assert_eq!(blocks[0].claimant, StableId("alice".to_string()));
        }
        WriteCheck::Ok { .. } => panic!("peer write to claimed path must be blocked"),
    }

    let allowed = check_write_against_claims(
        &[RepoPath("docs/unclaimed.md".to_string())],
        &StableId("bob".to_string()),
        &claims,
        NOW + 1,
    );
    match allowed {
        WriteCheck::Ok {
            governed_by_self,
            ungoverned,
        } => {
            assert!(governed_by_self.is_empty());
            assert_eq!(ungoverned, vec![RepoPath("docs/unclaimed.md".to_string())]);
        }
        WriteCheck::Blocked { blocks } => panic!("unclaimed path must be allowed: {blocks:?}"),
    }
}

#[test]
fn wal_authority_survives_missing_yaml_cache() {
    let dir = tmp_claims_dir("wal-authority-missing-cache");
    let path = "contracts/stories/S6.7.yaml";
    let acquired = run_acquire(
        &dir,
        &acquire_req("alice", "S6.7", &[path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    for entry in fs::read_dir(&dir).expect("read cache dir") {
        let entry = entry.expect("cache dir entry");
        if entry.path().extension().is_some_and(|ext| ext == "yaml") {
            fs::remove_file(entry.path()).expect("remove YAML cache file");
        }
    }

    let status = run_status(&dir, NOW + 1);

    assert!(status.ok, "status must replay WAL: {:?}", status.error);
    let view = status.data.expect("status data");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].scope_id, "S6.7");
    let write = run_check_write(
        &dir,
        &StableId("bob".to_string()),
        &[path.to_string()],
        NOW + 1,
    );
    assert!(!write.ok, "peer write must remain blocked by WAL authority");
    assert_eq!(write.exit_code(), ExitReason::RejectedByGate.as_code());
}

#[test]
fn wal_authority_ignores_stale_yaml_cache_after_release() {
    let dir = tmp_claims_dir("wal-authority-stale-cache");
    let path = "contracts/stories/S6.8.yaml";
    let acquired = run_acquire(
        &dir,
        &acquire_req("alice", "S6.8", &[path]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let stale_active_claim = load_one_claim(&dir);
    let claim_id = StableId(acquired.data.expect("acquire data").claim_id);
    let released = run_release(
        &dir,
        &claim_id,
        &StableId("alice".to_string()),
        NOW + 1,
        WalDurability::NoSync,
    );
    assert!(released.ok, "release should succeed: {:?}", released.error);
    save_claim(&dir, &stale_active_claim).expect("overwrite cache with stale active claim");

    let status = run_status(&dir, NOW + 2);

    assert!(
        status.ok,
        "status must ignore stale cache: {:?}",
        status.error
    );
    let view = status.data.expect("status data");
    assert!(view.active.is_empty(), "released WAL state must win");
    let reacquired = run_acquire(
        &dir,
        &acquire_req("bob", "S6.8", &[path]),
        NOW + 2,
        WalDurability::NoSync,
    );
    assert!(
        reacquired.ok,
        "released WAL state must not resurrect stale cache blocker: {:?}",
        reacquired.error
    );
}

#[test]
fn cache_only_claim_without_wal_fails_closed() {
    let dir = tmp_claims_dir("cache-only-no-wal");
    let acquired = run_acquire(
        &dir,
        &acquire_req("alice", "S6.9", &["contracts/stories/S6.9.yaml"]),
        NOW,
        WalDurability::NoSync,
    );
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    fs::remove_file(claim_wal_path(&dir)).expect("remove authoritative WAL");

    let status = run_status(&dir, NOW + 1);

    assert!(!status.ok, "cache-only state must fail closed");
    assert_eq!(status.exit_code(), ExitReason::EnvConfig.as_code());
    let error = status.error.expect("env config error");
    assert!(
        error.message.contains("authoritative WAL") && error.message.contains("missing"),
        "error should explain missing WAL authority: {}",
        error.message
    );
}
