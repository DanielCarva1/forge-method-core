use assert_cmd::Command;
use forge_core_contracts::claim::ClaimStatus;
use forge_core_store::claim_wal::{
    recover_claim_wal, replay_claim_wal, ClaimWalOperation, ClaimWalStopReason,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

const NOW: i64 = 1_800_000_000;
const DEFAULT_PARALLEL_ACQUIRES: usize = 16;
const FULL_STRESS_PARALLEL_ACQUIRES: usize = 50;
const SINGLE_CLI_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct AcquireOutcome {
    index: usize,
    output: Output,
    attempts: usize,
    lock_contentions: usize,
}

struct StressExpectations {
    command_claim_ids: BTreeSet<String>,
    expected_agents: BTreeSet<String>,
    expected_paths: BTreeSet<String>,
}

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

fn fresh_state(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root().join("target").join(format!(
        "claim-wal-stress-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("claims-active")).expect("create claims dir");
    root
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok");
    json
}

fn is_retryable_lock_contention(output: &Output) -> bool {
    if output.status.success() {
        return false;
    }
    let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) else {
        return false;
    };
    let code = json["error"]["code"].as_str();
    let message = json["error"]["message"].as_str().unwrap_or_default();
    code == Some("env_config")
        && message.contains("claims lock")
        && message.contains(".forge-claim.lock")
}

fn acquire_with_bounded_retries(
    index: usize,
    claims_arg: &str,
    overall_timeout: Duration,
) -> AcquireOutcome {
    let scope_id = format!("P3-CLI-STRESS-{index:03}");
    let agent_id = format!("stress-agent-{index:03}");
    let claim_path = format!("stress/{index:03}/owned.txt");
    let now_unix = NOW.to_string();
    let deadline = Instant::now() + overall_timeout;
    let mut attempts = 0usize;
    let mut lock_contentions = 0usize;

    loop {
        attempts += 1;
        let mut command = bin();
        command.timeout(SINGLE_CLI_ATTEMPT_TIMEOUT).args([
            "claim",
            "acquire",
            "--claims-dir",
            claims_arg,
            "--scope",
            "story",
            "--id",
            &scope_id,
            "--agent",
            &agent_id,
            "--path",
            &claim_path,
            "--ttl",
            "600",
            "--now-unix",
            &now_unix,
        ]);

        let output = command.output().unwrap_or_else(|err| {
            panic!("run concurrent claim acquire #{index} attempt {attempts}: {err}");
        });

        if output.status.success() || !is_retryable_lock_contention(&output) {
            return AcquireOutcome {
                index,
                output,
                attempts,
                lock_contentions,
            };
        }

        lock_contentions += 1;
        assert!(
            Instant::now() < deadline,
            "claim acquire #{index} exhausted retry budget after {attempts} attempts\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let jitter_ms = 10 + u64::try_from((index + attempts) % 7).expect("jitter fits in u64") * 5;
        thread::sleep(Duration::from_millis(jitter_ms));
    }
}

fn spawn_acquire_workers(
    process_count: usize,
    claims_arg: &Arc<String>,
    timeout: Duration,
) -> Vec<AcquireOutcome> {
    let barrier = Arc::new(Barrier::new(process_count));
    let handles: Vec<_> = (0..process_count)
        .map(|index| {
            let barrier = Arc::clone(&barrier);
            let claims_arg = Arc::clone(claims_arg);
            thread::spawn(move || {
                barrier.wait();
                acquire_with_bounded_retries(index, claims_arg.as_str(), timeout)
            })
        })
        .collect();

    let mut outcomes = Vec::with_capacity(process_count);
    for handle in handles {
        outcomes.push(
            handle
                .join()
                .unwrap_or_else(|payload| panic!("claim acquire thread panicked: {payload:?}")),
        );
    }
    outcomes.sort_by_key(|outcome| outcome.index);
    outcomes
}

fn collect_acquire_expectations(
    outcomes: &[AcquireOutcome],
    process_count: usize,
) -> StressExpectations {
    let mut command_claim_ids = BTreeSet::new();
    let mut expected_agents = BTreeSet::new();
    let mut expected_paths = BTreeSet::new();
    let mut total_lock_contentions = 0usize;

    for outcome in outcomes {
        let index = outcome.index;
        let output_label = format!("claim acquire #{index}");
        let json = assert_success(&outcome.output, &output_label);
        let claim_id = json["data"]["claim_id"]
            .as_str()
            .expect("claim id")
            .to_owned();
        let agent_id = format!("stress-agent-{index:03}");
        let claim_path = format!("stress/{index:03}/owned.txt");

        assert!(
            command_claim_ids.insert(claim_id),
            "duplicate claim id in command outputs"
        );
        expected_agents.insert(agent_id);
        expected_paths.insert(claim_path);
        assert!(
            outcome.attempts > 0,
            "each worker should execute at least one CLI attempt"
        );
        total_lock_contentions += outcome.lock_contentions;
    }
    assert!(
        total_lock_contentions < process_count * 100,
        "retry loop should not spin excessively"
    );

    StressExpectations {
        command_claim_ids,
        expected_agents,
        expected_paths,
    }
}

fn assert_recovered_wal(
    state_root: &Path,
    process_count: usize,
    expectations: &StressExpectations,
) {
    let recovery = recover_claim_wal(state_root, false).expect("recover claim WAL");
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert!(!recovery.repaired, "read-only recovery should not repair");
    assert_eq!(recovery.records.len(), process_count);
    assert_eq!(recovery.valid_record_count, process_count);
    assert_eq!(
        recovery.last_observed_seq,
        u64::try_from(process_count).expect("process count fits in u64")
    );

    let mut wal_claim_ids = BTreeSet::new();
    for (record_index, record) in recovery.records.iter().enumerate() {
        assert_eq!(
            record.seq,
            u64::try_from(record_index + 1).expect("record index fits in u64"),
            "WAL sequence should be gapless and monotonic"
        );
        assert_eq!(record.operation, ClaimWalOperation::Acquire);
        assert_eq!(
            record.payload.claim_contract.status.value,
            ClaimStatus::Active
        );

        let claim_id = &record.payload.claim_contract.id.0;
        assert!(
            expectations.command_claim_ids.contains(claim_id),
            "WAL claim id should come from a successful CLI acquire"
        );
        assert!(
            wal_claim_ids.insert(claim_id.clone()),
            "duplicate claim id in WAL records"
        );

        let agent_id = &record.payload.claim_contract.claim.claimant_agent_id.0;
        assert!(
            expectations.expected_agents.contains(agent_id),
            "WAL agent id should come from the stress set"
        );

        let paths = &record.payload.claim_contract.scope.paths;
        assert_eq!(paths.len(), 1, "stress claim should own one path");
        assert!(
            expectations.expected_paths.contains(&paths[0].0),
            "WAL path should come from the stress set"
        );
    }
    assert_eq!(wal_claim_ids, expectations.command_claim_ids);
}

fn assert_projection(state_root: &Path, process_count: usize, expectations: &StressExpectations) {
    let projection = replay_claim_wal(state_root, false).expect("replay claim WAL");
    assert!(
        projection.diagnostics.is_empty(),
        "projection should not report diagnostics: {:?}",
        projection.diagnostics
    );
    assert_eq!(projection.latest_by_claim_id.len(), process_count);
    assert_eq!(projection.active_by_claim_id.len(), process_count);
    for claim_id in &expectations.command_claim_ids {
        assert!(
            projection.active_by_claim_id.contains_key(claim_id),
            "projection should keep claim active: {claim_id}"
        );
    }
}

fn assert_status(claims_arg: &str, process_count: usize, expectations: &StressExpectations) {
    let status = bin()
        .args([
            "claim",
            "status",
            "--claims-dir",
            claims_arg,
            "--now-unix",
            &(NOW + 1).to_string(),
        ])
        .output()
        .expect("run claim status after stress");
    let status_json = assert_success(&status, "claim status after stress");
    let active = status_json["data"]["active"]
        .as_array()
        .expect("active claims array");
    assert_eq!(active.len(), process_count);

    let mut status_claim_ids = BTreeSet::new();
    for claim in active {
        let claim_id = claim["claim_id"].as_str().expect("status claim id");
        assert!(
            expectations.command_claim_ids.contains(claim_id),
            "status claim id should come from stress set"
        );
        assert!(
            status_claim_ids.insert(claim_id.to_owned()),
            "duplicate claim id in status output"
        );
    }
    assert_eq!(status_claim_ids, expectations.command_claim_ids);
}

fn run_parallel_acquire_stress(process_count: usize, label: &str, timeout: Duration) {
    assert!(process_count > 0, "process count must be nonzero");

    let state_root = fresh_state(label);
    let claims_dir = state_root.join("claims-active");
    let claims_arg = Arc::new(claims_dir.display().to_string());
    let outcomes = spawn_acquire_workers(process_count, &claims_arg, timeout);
    let expectations = collect_acquire_expectations(&outcomes, process_count);

    assert_recovered_wal(&state_root, process_count, &expectations);
    assert_projection(&state_root, process_count, &expectations);
    assert_status(claims_arg.as_str(), process_count, &expectations);
}

#[test]
fn claim_wal_cli_parallel_acquire_preserves_clean_monotonic_wal() {
    run_parallel_acquire_stress(
        DEFAULT_PARALLEL_ACQUIRES,
        "default-parallel-acquire",
        Duration::from_secs(45),
    );
}

#[test]
#[ignore = "full 50-process stress; run explicitly when validating WAL contention"]
fn claim_wal_cli_50_process_parallel_acquire_preserves_clean_monotonic_wal() {
    run_parallel_acquire_stress(
        FULL_STRESS_PARALLEL_ACQUIRES,
        "full-50-parallel-acquire",
        Duration::from_secs(120),
    );
}
