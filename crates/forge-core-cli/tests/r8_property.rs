//! Property test pinning the R8 invariant (slice-6 Frente A).
//!
//! R8 (slice-5 live demo): `release`/`heartbeat` resolved `--id` only against
//! the full derived claim id, never the operator-typed scope id. The point-fix
//! (`resolve_claim`, now `ClaimRef`-based) accepts both; this property test
//! STRUCTURALLY forbids the circular oracle from coming back.
//!
//! The generic test pattern that hid R8 was: extract the canonical claim id
//! from `acquire`'s output and feed it straight back into `release`. That is a
//! circular oracle — the test's input was derived from the system under test.
//! Per `contracts/research/rust-testing-defenses-v1.yaml` F3/F7, the antidote
//! is a property-based test whose `Strategy` generates the SCOPE id
//! INDEPENDENTLY of `acquire`'s output and asserts the lookup roundtrips for
//! every generated value. proptest (not quickcheck) is used because per-value
//! `Strategy` objects can model the operator/canonical split inside one token
//! space.

use forge_core_cli::claim::{parse_claim_ref, run_acquire, run_heartbeat, run_release, ClaimRef};
use forge_core_contracts::{
    claim::{ActorRole, ClaimScopeKind},
    RepoPath, ScopeId, StableId,
};
use forge_core_engine::AcquireRequest;
use proptest::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const T0: i64 = 1_779_000_000;

/// Generate an operator-shaped scope id INDEPENDENTLY of any `acquire` output.
///
/// R8's hiding place was that tests reused the canonical id (`claim.lane.s1.s1`)
/// the engine produced. This strategy never sees that id — it emits short
/// operator tokens (`s1`, `abc`, `lane-2`, ...) exactly as a human/agent types
/// them on argv. If `release`/`heartbeat` ever stop accepting the scope form,
/// this property fails.
fn scope_id_strategy() -> impl Strategy<Value = String> {
    // Operator-typed scope ids: ascii letters/digits/dash/underscore, no leading
    // dot, never starting with "claim." (that prefix is the canonical marker).
    // Bounded length keeps the generated filenames filesystem-safe.
    "[a-zA-Z0-9_][a-zA-Z0-9_-]{0,18}"
}

fn tmp_claims_dir(label: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("forge-prop-r8-{label}-{n}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn acquire_req(scope_id: &str, agent: &str) -> AcquireRequest {
    AcquireRequest {
        scope_kind: ClaimScopeKind::Story,
        scope_id: ScopeId(scope_id.to_string()),
        agent_id: StableId(agent.to_string()),
        role: ActorRole::Worker,
        ttl_seconds: 600,
        heartbeat_interval_seconds: 120,
        paths: vec![RepoPath(format!("contracts/stories/{scope_id}.yaml"))],
        product_area: None,
        expected_state_version: None,
    }
}

proptest! {
    /// R8 core property: for ANY operator-typed scope id, after acquire the
    /// SAME scope id must resolve a release. The input (scope_id) is generated
    /// independently of acquire's output (the canonical claim id), so this
    /// cannot pass by circularity — it only passes if `ClaimRef::Scope` lookup
    /// genuinely works.
    #[test]
    fn release_by_scope_id_roundtrips_for_any_operator_token(scope_id in scope_id_strategy()) {
        let dir = tmp_claims_dir("release");
        let agent = StableId("agentA".to_string());

        let acquired = run_acquire(&dir, &acquire_req(&scope_id, "agentA"), T0);
        prop_assert!(acquired.ok, "acquire must succeed: {:?}", acquired.error);

        // The canonical id produced by the engine must DIFFER from the
        // operator's scope id for this test to mean anything (else it's the
        // trivial circular case). This is the R8 setup guard.
        let canonical = acquired.data.as_ref().unwrap().claim_id.clone();
        prop_assert_ne!(
            canonical, scope_id.clone(),
            "test is vacuous unless canonical id != scope id"
        );

        // Operator releases by the SCOPE id they actually typed — not the
        // canonical id the engine derived. This is the line R8 broke.
        let released = run_release(&dir, &StableId(scope_id.clone()), &agent, T0);
        prop_assert!(
            released.ok,
            "release by scope id '{}' must succeed: {:?}",
            scope_id,
            released.error
        );
        prop_assert_eq!(&released.data.as_ref().unwrap().status, "released");

        // Cross-check the structural layer: parse_claim_ref must classify the
        // operator token as the Scope variant (not Full), or the type split is
        // not doing its job.
        prop_assert!(
            matches!(parse_claim_ref(&scope_id), ClaimRef::Scope(_)),
            "operator token must parse to ClaimRef::Scope"
        );
    }

    /// R8 echo for heartbeat: the same independent-scope-id property must hold
    /// for the heartbeat path (shares `resolve_claim`).
    #[test]
    fn heartbeat_by_scope_id_roundtrips_for_any_operator_token(scope_id in scope_id_strategy()) {
        let dir = tmp_claims_dir("heartbeat");
        let agent = StableId("agentA".to_string());

        let _ = run_acquire(&dir, &acquire_req(&scope_id, "agentA"), T0);

        let hb = run_heartbeat(&dir, &StableId(scope_id.clone()), &agent, T0);
        prop_assert!(
            hb.ok,
            "heartbeat by scope id '{}' must succeed: {:?}",
            scope_id,
            hb.error
        );
        prop_assert_eq!(&hb.data.as_ref().unwrap().status, "active");
    }

    /// Backwards-compat property: the canonical (full) id form must STILL
    /// resolve after the type split, so existing consumers that hold the
    /// canonical id are not broken. Generates scope ids, acquires, then
    /// releases by the CANONICAL id returned by acquire.
    #[test]
    fn release_by_full_canonical_id_still_works(scope_id in scope_id_strategy()) {
        let dir = tmp_claims_dir("full");
        let agent = StableId("agentA".to_string());

        let acquired = run_acquire(&dir, &acquire_req(&scope_id, "agentA"), T0);
        let canonical = acquired.data.as_ref().unwrap().claim_id.clone();

        // The canonical id MUST parse to the Full variant.
        prop_assert!(
            matches!(parse_claim_ref(&canonical), ClaimRef::Full(_)),
            "canonical id '{}' must parse to ClaimRef::Full",
            canonical
        );

        let released = run_release(&dir, &StableId(canonical), &agent, T0);
        prop_assert!(released.ok, "release by full id must still work: {:?}", released.error);
    }
}
