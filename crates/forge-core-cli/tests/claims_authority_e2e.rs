//! Authority-invariant end-to-end tests for the claims cache-vs-WAL contract.
//!
//! These exercise the spec acceptance criteria that the WAL is the sole
//! authority and the editable `claims-active/*.yaml` cache is never trusted by
//! a decision path:
//! - **ac7** editing/adding `claims-active/*.yaml` directly does NOT change
//!   derived claim status (the authority is the WAL, not the cache).
//! - **ac5** the legacy cache read is reachable behind the explicit
//!   `--from-cache` debug flag (the escape hatch works).
//!
//! Drives the real `forge-core` binary via `assert_cmd` against a sidecar
//! layout set up exactly like `claim_cli_sidecar_e2e.rs`.

#![allow(clippy::too_many_lines)]

use assert_cmd::Command;
use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimContractDocument, ClaimIdentity, ClaimKind, ClaimLease,
    ClaimScope, ClaimScopeKind, ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy,
    ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId, ENVELOPE_SCHEMA_VERSION};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_800_000_000;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn fresh_parent(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root()
        .join("target")
        .join(format!("claims-authority-e2e-{label}-{n}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create fresh parent");
    root
}

struct ConsumerApp {
    app: PathBuf,
    state_root: PathBuf,
}

/// Set up a consumer app + sibling sidecar exactly the way
/// `claim_cli_sidecar_e2e::consumer_app` does: an app root with a project link
/// pointing at a sidecar state root.
fn consumer_app(label: &str) -> ConsumerApp {
    let parent = fresh_parent(label);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");

    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&state_root).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");

    ConsumerApp { app, state_root }
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &std::process::Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok: {json:#}");
    json
}

fn active_count(status_json: &Value) -> usize {
    status_json["data"]["active"].as_array().map_or_else(
        || panic!("active claims array missing: {status_json:#}"),
        Vec::len,
    )
}

/// Acquire one claim on `path`, returning the JSON envelope.
fn acquire(app: &Path, scope_id: &str, agent: &str, path: &str, now_unix: i64) -> Value {
    let app_arg = app.display().to_string();
    let output = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app_arg,
            "--scope",
            "story",
            "--id",
            scope_id,
            "--agent",
            agent,
            "--path",
            path,
            "--now-unix",
            &now_unix.to_string(),
            "--json",
        ])
        .output()
        .expect("run claim acquire");
    assert_success(&output, "claim acquire")
}

fn status(app: &Path, now_unix: i64, from_cache: bool) -> Value {
    let app_arg = app.display().to_string();
    let mut cmd = bin();
    cmd.args(["claim", "status", "--root", &app_arg, "--json"]);
    if from_cache {
        cmd.arg("--from-cache");
    }
    let output = cmd
        .args(["--now-unix", &now_unix.to_string()])
        .output()
        .expect("run claim status");
    assert_success(&output, "claim status")
}

/// Build a fully-formed, schema-valid forged claim document that an attacker
/// would plant directly into `claims-active/`. It claims a DIFFERENT path than
/// the real acquire, so if the cache were ever trusted the active count would
/// jump.
fn forged_claim_yaml(forged_path: &str) -> String {
    let claim = ClaimContract {
        id: ClaimId("claim.story.FORGED.FORGED".to_string()),
        contract_ref: RepoPath("claims-active/forged.yaml".to_string()),
        claim: ClaimIdentity {
            kind: ClaimKind::Story,
            claimant_agent_id: StableId("mallory".to_string()),
            claimant_role: ActorRole::Worker,
            registry_ref: None,
        },
        scope: ClaimScope {
            kind: ClaimScopeKind::Story,
            id: ScopeId("FORGED".to_string()),
            product_area: None,
            paths: vec![RepoPath(forged_path.to_string())],
        },
        lease: ClaimLease {
            acquired_at: "2027-01-15T08:00:00Z".to_string(),
            last_heartbeat_at: "2027-01-15T08:00:00Z".to_string(),
            expires_at: "9999-12-31T23:59:59Z".to_string(),
            ttl_seconds: 99_999_999,
            heartbeat_interval_seconds: 120,
            expected_state_version: 0,
        },
        status: ClaimStatusRecord {
            value: ClaimStatus::Active,
            evaluated_at: "2027-01-15T08:00:00Z".to_string(),
            reason_code: None,
        },
        expiry_policy: ExpiryPolicy {
            on_expiry: ExpiryAction::RecordHandoffRequest,
            handoff_required: true,
            release_without_handoff_allowed: false,
            reclaim_policy: ReclaimPolicy::DriverReview,
            handoff_request_ref: Some(RepoPath(
                "contracts/requests/claim-expiry-handoff-request.yaml".to_string(),
            )),
        },
        evidence_refs: Vec::new(),
    };
    let doc = ClaimContractDocument {
        schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
        claim_contract: claim,
    };
    yaml_serde::to_string(&doc).expect("serialize forged claim document")
}

#[test]
fn editing_cache_yaml_does_not_change_claim_status() {
    // AC7: planting a forged active claim directly into the editable
    // claims-active/*.yaml cache must NOT change derived status, because the
    // authority is the WAL. Status reads the WAL both before and after the
    // tamper and must report the same single real active claim.
    let fixture = consumer_app("cache-tamper");
    let claims_dir = fixture.state_root.join("claims-active");

    // Real authority: one acquire on src/main.rs.
    acquire(&fixture.app, "AC7-real", "alice", "src/main.rs", NOW);

    // Authority baseline: exactly one active claim, the real one.
    let before = status(&fixture.app, NOW + 1, false);
    assert_eq!(
        active_count(&before),
        1,
        "baseline: one real active claim: {before:#}"
    );
    assert!(
        before["data"]["active"]
            .as_array()
            .expect("active array")
            .iter()
            .any(|c| c["agent_id"] == "alice"
                && c["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|p| p == "src/main.rs"))),
        "baseline must show the real alice claim: {before:#}"
    );

    // Attack: drop a forged, schema-valid active lease for a DIFFERENT path
    // straight into the cache dir (bypassing the WAL entirely).
    fs::create_dir_all(&claims_dir).expect("ensure claims-active dir");
    let forged_path = forged_claim_yaml("src/attacker-controlled.rs");
    fs::write(claims_dir.join("forged.yaml"), forged_path).expect("write forged cache YAML");
    assert!(
        claims_dir.join("forged.yaml").is_file(),
        "forged cache YAML must exist for the tamper to be meaningful"
    );

    // After the tamper, authority status must be UNCHANGED — still one active
    // claim, still alice's. The forged cache entry is ignored.
    let after = status(&fixture.app, NOW + 2, false);
    assert_eq!(
        active_count(&after),
        1,
        "forged cache YAML must not change authority status: {after:#}"
    );
    assert!(
        after["data"]["active"]
            .as_array()
            .expect("active array")
            .iter()
            .all(|c| c["agent_id"] != "mallory"),
        "the forged 'mallory' claim must never appear in authority status: {after:#}"
    );
    assert!(
        after["data"]["active"]
            .as_array()
            .expect("active array")
            .iter()
            .all(|c| c["paths"]
                .as_array()
                .is_none_or(|paths| !paths.iter().any(|p| p == "src/attacker-controlled.rs"))),
        "the forged path must never appear in authority status: {after:#}"
    );

    // Sanity: the forged YAML IS visible through the --from-cache debug path,
    // proving the tamper landed on disk and that only the authority path
    // ignores it.
    let cached = status(&fixture.app, NOW + 3, true);
    assert!(
        cached["data"]["active"]
            .as_array()
            .expect("cached active array")
            .iter()
            .any(|c| c["agent_id"] == "mallory"),
        "sanity: --from-cache debug path should see the forged claim on disk: {cached:#}"
    );
}

#[test]
fn from_cache_flag_reads_legacy_yaml() {
    // AC5 debug side: `--from-cache` is the explicit escape hatch that reads
    // the legacy claims-active/*.yaml cache directly (not the WAL). After a
    // normal acquire the cache holds the claim, so --from-cache must find it.
    let fixture = consumer_app("from-cache");
    acquire(&fixture.app, "AC5-cache", "alice", "src/main.rs", NOW);

    let cached = status(&fixture.app, NOW + 1, true);
    assert_eq!(
        cached["command"], "claim.status",
        "--from-cache status should still report the claim.status command: {cached:#}"
    );
    let active = cached["data"]["active"]
        .as_array()
        .expect("active claims array from cache");
    assert!(
        active.iter().any(|c| c["agent_id"] == "alice"
            && c["paths"]
                .as_array()
                .is_some_and(|paths| paths.iter().any(|p| p == "src/main.rs"))),
        "--from-cache must surface the cached alice claim: {cached:#}"
    );
}
