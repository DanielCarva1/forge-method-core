//! Claim lifecycle integration tests (Q04).
//!
//! Mirrors the temp-dir + request harness in `claim_e2e.rs`, but focuses on the
//! complete lifecycle surface: acquire, conflict, heartbeat, release, expiry, and
//! pure write-conflict checks over the persisted claims bus.

use forge_core_cli::claim::{load_claims, run_acquire, run_heartbeat, run_release};
use forge_core_contracts::{
    claim::{ActorRole, ClaimScopeKind, ClaimStatus},
    ClaimId, ExitReason, RepoPath, ScopeId, StableId,
};
use forge_core_engine::{
    check_write_against_claims, expire_stale, is_expired, is_live, project_active, unix_to_rfc3339,
    AcquireRequest, WriteCheck,
};
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
    let acquired = run_acquire(dir, &acquire_req(agent, scope_id, &[path]), NOW);
    assert!(acquired.ok, "acquire should succeed: {:?}", acquired.error);
    let claim_id = acquired
        .data
        .expect("successful acquire must carry a claim result")
        .claim_id;
    StableId(claim_id)
}

#[test]
fn acquire_success_persists_active_live_claim() {
    let dir = tmp_claims_dir("acquire");
    let req = acquire_req("alice", "S6.1", &["contracts/stories/S6.1.yaml"]);

    let acquired = run_acquire(&dir, &req, NOW);

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
    let first = run_acquire(&dir, &acquire_req("alice", "S6.2", &[path]), NOW);
    assert!(first.ok, "first acquire should succeed: {:?}", first.error);

    let second = run_acquire(&dir, &acquire_req("bob", "S6.2", &[path]), NOW + 1);

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

    let released = run_release(&dir, &claim_id, &StableId("alice".to_string()), NOW + 10);

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
    let acquired = run_acquire(&dir, &req, NOW);
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
fn check_write_denies_peer_claimed_path_and_allows_unclaimed_path() {
    let dir = tmp_claims_dir("check-write");
    let path = "contracts/stories/S6.6.yaml";
    let acquired = run_acquire(&dir, &acquire_req("alice", "S6.6", &[path]), NOW);
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
