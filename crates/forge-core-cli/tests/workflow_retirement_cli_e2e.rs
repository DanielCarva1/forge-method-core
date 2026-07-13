//! Real-binary proof for the read-only verified-retirement audit projection.

use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ARTIFACTS: [&str; 3] = [
    "contracts/migration/workflow-retirement-evidence-index-v0.yaml",
    "contracts/migration/workflow-retirement-tombstones-v0.yaml",
    "contracts/migration/workflow-governance-final-scorecard-v0.yaml",
];

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!(
        "forge-retirement-status-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    for artifact in ARTIFACTS {
        let destination = root.join(artifact);
        fs::create_dir_all(destination.parent().expect("artifact parent"))
            .expect("artifact directory");
        fs::copy(repo_root().join(artifact), destination).expect("copy retirement artifact");
    }
    root
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, directory: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).expect("read fixture directory") {
            let path = entry.expect("fixture entry").path();
            if path.is_dir() {
                collect(root, &path, snapshot);
            } else {
                let relative = path.strip_prefix(root).expect("fixture-relative path");
                snapshot.insert(
                    relative.to_path_buf(),
                    fs::read(&path).expect("read fixture file"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

#[test]
fn retirement_status_is_verified_and_performs_zero_writes() {
    let root = fixture_root();
    let before = snapshot(&root);
    let output = bin()
        .args(["workflow", "retirement-status", "--root"])
        .arg(&root)
        .arg("--json")
        .output()
        .expect("retirement-status command");

    assert!(
        output.status.success(),
        "retirement-status failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("status envelope");
    assert_eq!(
        envelope["data"]["authority"],
        "verified_retirement_checkpoint"
    );
    assert_eq!(
        envelope["data"]["authorization_projection"],
        "non_authoritative_audit_of_opaque_capability"
    );
    assert_eq!(envelope["data"]["verified_retirement_count"], 42);
    assert_eq!(envelope["data"]["operational_workflow_count"], 68);
    assert!(envelope["data"]["payload_digest"]
        .as_str()
        .expect("payload digest")
        .starts_with("sha256:"));
    assert_eq!(snapshot(&root), before, "status command must be read-only");

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn retirement_status_ignores_caller_tombstone_overrides() {
    let root = fixture_root();
    fs::write(
        root.join(ARTIFACTS[1]),
        "schema_version: '0.1'\nworkflow_retirement_tombstone_catalog: invalid\n",
    )
    .expect("tamper tombstones");

    let output = bin()
        .args(["workflow", "retirement-status", "--root"])
        .arg(&root)
        .arg("--json")
        .output()
        .expect("retirement-status command");
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("success envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(
        envelope["data"]["authority"],
        "verified_retirement_checkpoint"
    );
    assert_eq!(envelope["data"]["verified_retirement_count"], 42);

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn every_tombstone_replacement_argv_runs_in_an_empty_external_consumer() {
    let describe = bin()
        .args(["guide", "describe", "--json"])
        .current_dir(std::env::temp_dir())
        .output()
        .expect("zero-config describe");
    assert!(
        describe.status.success(),
        "describe failed outside repository: {}",
        String::from_utf8_lossy(&describe.stderr)
    );
    let envelope: Value = serde_json::from_slice(&describe.stdout).expect("describe envelope");
    let tombstones = envelope["data"]["retired_workflows"]
        .as_array()
        .expect("retired workflows");
    assert_eq!(tombstones.len(), 42);

    for (ordinal, tombstone) in tombstones.iter().enumerate() {
        let argv = tombstone["replacement_argv"]
            .as_array()
            .expect("replacement argv")
            .iter()
            .map(|value| value.as_str().expect("string argv"))
            .collect::<Vec<_>>();
        assert_eq!(argv.first().copied(), Some("forge-core"));
        assert_eq!(
            argv,
            ["forge-core", "start", "--root", ".", "--json"],
            "replacement must be install-location and repository independent"
        );

        let consumer = std::env::temp_dir().join(format!(
            "forge-retired-replacement-{}-{ordinal}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&consumer);
        fs::create_dir_all(&consumer).expect("empty consumer root");
        let output = bin()
            .args(&argv[1..])
            .current_dir(&consumer)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "execute replacement for {}: {error}",
                    tombstone["workflow_id"]
                )
            });
        assert!(
            output.status.success(),
            "replacement for {} failed\nstdout:\n{}\nstderr:\n{}",
            tombstone["workflow_id"],
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let replacement: Value =
            serde_json::from_slice(&output.stdout).expect("replacement JSON envelope");
        assert_eq!(replacement["ok"], true);
        assert_eq!(replacement["command"], "start");
        assert!(
            replacement["data"]["next_step"]["argv"]
                .as_array()
                .is_some_and(|argv| !argv.is_empty()),
            "replacement did not return an actionable next step"
        );
        fs::remove_dir_all(&consumer).expect("remove consumer fixture");
    }
}
