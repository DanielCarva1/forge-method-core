//! Real-binary proof for the opaque, kernel-owned workflow release pin.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

struct LegacyProject {
    parent: PathBuf,
    app: PathBuf,
    state: PathBuf,
}

impl LegacyProject {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let parent = std::env::temp_dir().join(format!(
            "forge-release-cli-{label}-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent);
        let app = parent.join("app");
        let state = parent.join("forge-app/.forge-method");
        fs::create_dir_all(&app).expect("legacy project");
        fs::create_dir_all(&state).expect("sidecar state");
        fs::write(app.join("README.md"), "legacy consumer\n").expect("project content");
        fs::write(
            app.join(".forge-method.yaml"),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
        )
        .expect("project link");
        Self { parent, app, state }
    }

    fn run(&self, tail: &[&str]) -> Output {
        let mut args = vec![
            "workflow".to_owned(),
            tail[0].to_owned(),
            "--root".to_owned(),
            self.app.display().to_string(),
        ];
        args.extend(tail[1..].iter().map(|value| (*value).to_owned()));
        bin().args(args).output().expect("workflow command")
    }

    fn wal(&self) -> PathBuf {
        self.state.join("wal/workflow-governance.ndjson")
    }

    fn install_hostile_local_release_files(&self) {
        write(
            &self
                .app
                .join("contracts/migration/workflow-governance-release-registry-v0.yaml"),
            "authority: caller_override\nreleases: attacker_selected\n",
        );
        write(
            &self
                .app
                .join("contracts/workflow-governance/runtime-release-foundation-v0.yaml"),
            "bundle_id: attacker.bundle\npolicy_set_digest: attacker\n",
        );
    }
}

impl Drop for LegacyProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.parent);
    }
}

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
    fs::write(path, body).expect("fixture write");
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid envelope: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_ok(output: &Output, command: &str) -> Value {
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value = envelope(output);
    assert_eq!(value["command"], command);
    assert_eq!(value["ok"], true);
    value
}

fn run_owned(args: &[String]) -> Output {
    bin().args(args).output().expect("workflow command")
}

fn required_string<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string at {pointer}: {value:#}"))
}

fn assert_rejections_do_not_mutate(project: &LegacyProject, status: &Value, initial_wal: &[u8]) {
    let target = required_string(status, "/data/available_successor/release_id");
    let current = required_string(status, "/data/active/release/release_digest");
    let head = required_string(status, "/data/ledger_head_digest");
    let snapshot = required_string(status, "/data/snapshot_digest");
    let stale_head = format!("sha256:{}", "0".repeat(64));
    let stale = project.run(&[
        "release-upgrade",
        "--target-release-id",
        target,
        "--expected-current-release-digest",
        current,
        "--expected-head-digest",
        &stale_head,
        "--expected-snapshot-digest",
        snapshot,
        "--json",
    ]);
    assert_eq!(stale.status.code(), Some(4));
    assert_eq!(envelope(&stale)["exit_reason"], "conflict");
    assert_eq!(fs::read(project.wal()).expect("stale WAL"), initial_wal);

    let unknown = project.run(&[
        "release-upgrade",
        "--target-release-id",
        "workflow-governance.release.unknown",
        "--expected-current-release-digest",
        current,
        "--expected-head-digest",
        head,
        "--expected-snapshot-digest",
        snapshot,
        "--json",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(envelope(&unknown)["exit_reason"], "rejected_by_gate");
    assert_eq!(fs::read(project.wal()).expect("unknown WAL"), initial_wal);
}

#[test]
fn legacy_project_upgrade_is_opaque_cas_bound_resumable_and_idempotent() {
    let project = LegacyProject::new("golden");
    project.install_hostile_local_release_files();
    assert_ok(&project.run(&["init", "--json"]), "workflow.init");

    let initial_wal = fs::read(project.wal()).expect("initialized WAL");
    let status = assert_ok(
        &project.run(&["release-status", "--json"]),
        "workflow.release_status",
    );
    assert_eq!(
        required_string(&status, "/data/active/release/release_id"),
        "workflow-governance.release.p5c-implicit-v0"
    );
    assert_eq!(
        required_string(&status, "/data/available_successor/release_id"),
        "workflow-governance.release.foundation-v0"
    );
    assert_eq!(fs::read(project.wal()).expect("status WAL"), initial_wal);

    let upgrade_argv = status["data"]["upgrade_argv"]
        .as_array()
        .expect("upgrade argv")
        .iter()
        .map(|value| value.as_str().expect("argv string").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        &upgrade_argv[..3],
        ["forge-core", "workflow", "release-upgrade"]
    );
    assert_eq!(upgrade_argv.len(), 13);
    assert!(!upgrade_argv.iter().any(|arg| {
        [
            "--registry-path",
            "--manifest-file",
            "--batch-path",
            "--bundle-path",
        ]
        .contains(&arg.as_str())
    }));

    let target = required_string(&status, "/data/available_successor/release_id").to_owned();
    assert_rejections_do_not_mutate(&project, &status, &initial_wal);

    let upgraded = assert_ok(&run_owned(&upgrade_argv[1..]), "workflow.release_upgrade");
    assert_eq!(upgraded["data"]["status"], "upgraded");
    assert_eq!(
        required_string(&upgraded, "/data/active/release/release_id"),
        target
    );
    let upgraded_wal = fs::read(project.wal()).expect("upgraded WAL");
    assert_ne!(upgraded_wal, initial_wal);
    assert_eq!(
        String::from_utf8_lossy(&upgraded_wal)
            .matches("release_upgraded")
            .count(),
        1
    );

    let resumed = assert_ok(&project.run(&["resume", "--json"]), "workflow.resume");
    assert_eq!(
        required_string(&resumed, "/data/release/release/release_id"),
        target
    );

    let stale_digest = format!("sha256:{}", "f".repeat(64));
    let replay = assert_ok(
        &project.run(&[
            "release-upgrade",
            "--target-release-id",
            &target,
            "--expected-current-release-digest",
            &stale_digest,
            "--expected-head-digest",
            &stale_digest,
            "--expected-snapshot-digest",
            &stale_digest,
            "--json",
        ]),
        "workflow.release_upgrade",
    );
    assert_eq!(replay["data"]["status"], "already_pinned");
    assert_eq!(replay["data"]["transition_record"], Value::Null);
    assert_eq!(
        replay["data"]["ledger_head_digest"],
        upgraded["data"]["ledger_head_digest"]
    );
    assert_eq!(fs::read(project.wal()).expect("replay WAL"), upgraded_wal);
}

#[test]
fn release_upgrade_rejects_path_authority_and_malformed_cas_before_adapter_use() {
    let missing = bin()
        .args([
            "workflow",
            "release-upgrade",
            "--root",
            "definitely-missing-project",
            "--json",
        ])
        .output()
        .expect("missing arguments command");
    assert_eq!(missing.status.code(), Some(3));
    assert_eq!(envelope(&missing)["exit_reason"], "invalid_decision_shape");

    let malformed = bin()
        .args([
            "workflow",
            "release-upgrade",
            "--root",
            "definitely-missing-project",
            "--target-release-id",
            "workflow-governance.release.foundation-v0",
            "--expected-current-release-digest",
            "ABC",
            "--expected-head-digest",
            "abc",
            "--expected-snapshot-digest",
            "abc",
            "--json",
        ])
        .output()
        .expect("malformed digest command");
    assert_eq!(malformed.status.code(), Some(3));
    assert_eq!(
        envelope(&malformed)["exit_reason"],
        "invalid_decision_shape"
    );

    let digest = format!("sha256:{}", "a".repeat(64));
    let unavailable = bin()
        .args([
            "workflow",
            "release-upgrade",
            "--root",
            "definitely-missing-project",
            "--target-release-id",
            "workflow-governance.release.foundation-v0",
            "--expected-current-release-digest",
            &digest,
            "--expected-head-digest",
            &digest,
            "--expected-snapshot-digest",
            &digest,
            "--json",
        ])
        .output()
        .expect("unavailable environment command");
    assert_eq!(unavailable.status.code(), Some(5));
    assert_eq!(envelope(&unavailable)["exit_reason"], "env_config");

    for flag in [
        "--registry-path",
        "--manifest-file",
        "--batch-path",
        "--bundle-path",
        "--release-file",
    ] {
        let forbidden = bin()
            .args([
                "workflow",
                "release-upgrade",
                flag,
                "attacker-controlled.yaml",
                "--json",
            ])
            .output()
            .expect("forbidden override command");
        assert_eq!(forbidden.status.code(), Some(3), "flag={flag}");
        let value = envelope(&forbidden);
        assert_eq!(value["exit_reason"], "invalid_decision_shape");
        assert!(required_string(&value, "/error/message").contains("forbidden"));
    }
}
