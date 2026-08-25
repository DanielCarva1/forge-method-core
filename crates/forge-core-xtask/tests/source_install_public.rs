use serde_json::Value;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const VERSION: &str = "0.12.0-alpha.19";
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    install_root: PathBuf,
    target_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "forge-source-install-test-{}-{nonce}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("repo");
        let install_root = root.join("install");
        let target_dir = root.join("target");
        let crate_root = repo.join("crates").join("forge-core-cli");
        fs::create_dir_all(crate_root.join("src")).expect("create fixture crate");
        fs::write(
            repo.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [\"crates/forge-core-cli\"]\nresolver = \"2\"\n\n\
                 [workspace.package]\nversion = \"{VERSION}\"\nedition = \"2021\"\n"
            ),
        )
        .expect("write workspace manifest");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"forge-core-cli\"\nversion.workspace = true\n\
             edition.workspace = true\n\n[[bin]]\nname = \"forge-core\"\npath = \"src/main.rs\"\n",
        )
        .expect("write package manifest");
        let fixture = Self {
            root,
            repo,
            install_root,
            target_dir,
        };
        fixture.write_program("one", None);
        fs::write(fixture.repo.join("product.txt"), "checkpoint one\n")
            .expect("write product marker");
        Self::assert_success(&fixture.command("cargo", &["generate-lockfile"]));
        fixture.git(&["init"]);
        fixture.git(&["config", "user.name", "Forge Test"]);
        fixture.git(&["config", "user.email", "forge-test@example.invalid"]);
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-m", "checkpoint one"]);
        fixture
    }

    fn command(&self, program: &str, arguments: &[&str]) -> Output {
        Command::new(program)
            .args(arguments)
            .current_dir(&self.repo)
            .output()
            .unwrap_or_else(|error| panic!("run {program}: {error}"))
    }

    fn assert_success(output: &Output) -> String {
        assert!(
            output.status.success(),
            "command failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn git(&self, arguments: &[&str]) -> String {
        Self::assert_success(&self.command("git", arguments))
    }

    fn write_program(&self, marker: &str, reported_version: Option<&str>) {
        let version = reported_version.map_or_else(
            || "env!(\"CARGO_PKG_VERSION\")".to_owned(),
            |value| format!("\"{value}\""),
        );
        fs::write(
            self.repo
                .join("crates")
                .join("forge-core-cli")
                .join("src")
                .join("main.rs"),
            format!(
                "fn main() {{\n    if std::env::args().any(|arg| arg == \"--version\") {{\n\
                 println!(\"forge-core {{}}\", {version});\n    }} else {{\n        println!(\"{marker}\");\n    }}\n}}\n"
            ),
        )
        .expect("write fixture program");
    }

    fn install(&self, extra: &[&str], environment: &[(&str, &str)]) -> Output {
        self.install_command(true, extra, environment)
    }

    fn install_command(
        &self,
        include_target_override: bool,
        extra: &[&str],
        environment: &[(&str, &str)],
    ) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_forge-xtask"));
        command
            .arg("source-install")
            .arg("--repo-root")
            .arg(&self.repo)
            .arg("--install-root")
            .arg(&self.install_root)
            .current_dir(&self.repo)
            .env_remove("FORGE_SOURCE_INSTALL_TEST_FAIL_ACTIVE_REPLACE")
            .env_remove("FORGE_SOURCE_INSTALL_TEST_FAIL_ROLLBACK_REPLACE");
        if include_target_override {
            command.arg("--target-dir").arg(&self.target_dir);
        }
        command.args(extra);
        for (name, value) in environment {
            command.env(name, value);
        }
        command.output().expect("run source installer")
    }

    fn install_ok(&self) -> Value {
        let output = self.install(&[], &[]);
        assert!(
            output.status.success(),
            "install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("parse install report")
    }

    fn commit_program(&self, marker: &str, number: u8) -> String {
        self.write_program(marker, None);
        self.git(&["add", "."]);
        self.git(&["commit", "-m", &format!("checkpoint {number}")]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn commit_product_only(&self, number: u8) -> String {
        fs::write(
            self.repo.join("product.txt"),
            format!("checkpoint {number}\n"),
        )
        .expect("write product marker");
        self.git(&["add", "product.txt"]);
        self.git(&["commit", "-m", &format!("checkpoint {number}")]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn binary(&self, rollback: bool) -> PathBuf {
        let name = if cfg!(windows) {
            "forge-core.exe"
        } else {
            "forge-core"
        };
        if rollback {
            self.install_root
                .join("source-install")
                .join("rollback")
                .join(name)
        } else {
            self.install_root.join("bin").join(name)
        }
    }

    fn marker(&self, path: &Path) -> String {
        Self::assert_success(
            &Command::new(path)
                .current_dir(&self.repo)
                .output()
                .expect("run installed fixture binary"),
        )
    }

    fn receipts(&self) -> Vec<PathBuf> {
        let directory = self.install_root.join("source-install").join("receipts");
        let mut paths = fs::read_dir(directory)
            .expect("read receipts")
            .map(|entry| entry.expect("receipt entry").path())
            .filter(|path| path.extension() == Some(OsStr::new("json")))
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn state(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.install_root.join("source-install").join("state.json"))
                .expect("read state"),
        )
        .expect("parse state")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.starts_with(env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn three_same_version_checkpoints_leave_current_and_one_rollback() {
    let fixture = Fixture::new();
    let mut commits = vec![fixture.git(&["rev-parse", "HEAD"])];
    fixture.install_ok();
    commits.push(fixture.commit_program("two", 2));
    fixture.install_ok();
    commits.push(fixture.commit_program("three", 3));
    let report = fixture.install_ok();

    assert_eq!(report["source_commit"], commits[2]);
    assert_eq!(fixture.marker(&fixture.binary(false)), "three");
    assert_eq!(fixture.marker(&fixture.binary(true)), "two");
    let receipts = fixture.receipts();
    assert_eq!(receipts.len(), 2);
    let commits_in_receipts = receipts
        .iter()
        .map(|path| {
            serde_json::from_slice::<Value>(&fs::read(path).expect("read receipt"))
                .expect("parse receipt")["source_commit"]
                .as_str()
                .expect("source commit")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        commits_in_receipts,
        BTreeSet::from([commits[1].clone(), commits[2].clone()])
    );
    assert_eq!(
        fs::read_dir(fixture.install_root.join("source-install").join("staging"))
            .expect("read staging")
            .count(),
        0
    );
}

#[test]
fn exact_checkpoint_retry_is_idempotent() {
    let fixture = Fixture::new();
    fixture.install_ok();
    let retry = fixture.install_ok();
    assert_eq!(retry["status"], "already_installed");
    assert_eq!(fixture.receipts().len(), 1);
}

#[test]
fn different_commits_with_identical_binary_are_distinguished() {
    let fixture = Fixture::new();
    let first_commit = fixture.git(&["rev-parse", "HEAD"]);
    let first = fixture.install_ok();
    let second_commit = fixture.commit_product_only(2);
    let second = fixture.install_ok();
    assert_eq!(second["binary_sha256"], first["binary_sha256"]);
    assert_eq!(fixture.receipts().len(), 2);
    let state = fixture.state();
    assert!(state["active_receipt_id"]
        .as_str()
        .expect("active receipt")
        .starts_with(&second_commit));
    assert!(state["rollback_receipt_id"]
        .as_str()
        .expect("rollback receipt")
        .starts_with(&first_commit));
}

#[test]
fn dirty_checkout_is_rejected_before_installation() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.join("product.txt"), "uncommitted\n").expect("dirty checkout");
    let result = fixture.install(&[], &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("source checkout is dirty"));
    assert!(!fixture.install_root.join("bin").exists());
}

#[test]
fn cargo_configured_target_directory_is_reused() {
    let fixture = Fixture::new();
    let configured_target = fixture.root.join("configured-target");
    fs::create_dir_all(fixture.repo.join(".cargo")).expect("create cargo config directory");
    fs::write(
        fixture.repo.join(".cargo").join("config.toml"),
        format!(
            "[build]\ntarget-dir = '{}'\n",
            configured_target.display().to_string().replace('\\', "/")
        ),
    )
    .expect("write cargo config");
    fixture.git(&["add", ".cargo/config.toml"]);
    fixture.git(&["commit", "-m", "configure shared target"]);

    let output = fixture.install_command(false, &[], &[]);
    assert!(
        output.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(configured_target
        .join("release")
        .join(if cfg!(windows) {
            "forge-core.exe"
        } else {
            "forge-core"
        })
        .exists());
    assert!(!fixture.repo.join("target").exists());
}

#[test]
fn built_candidate_version_mismatch_preserves_current_binary() {
    let fixture = Fixture::new();
    fixture.install_ok();
    let before = fs::read(fixture.binary(false)).expect("read active");
    fixture.write_program("wrong", Some("9.9.9"));
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-m", "wrong version"]);
    let rejected = fixture.install(&[], &[]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("candidate version mismatch"));
    assert_eq!(
        fs::read(fixture.binary(false)).expect("read active"),
        before
    );
    assert_eq!(fixture.receipts().len(), 1);
}

#[test]
fn active_replace_failure_preserves_active_rollback_and_receipts() {
    let fixture = Fixture::new();
    fixture.install_ok();
    fixture.commit_program("two", 2);
    fixture.install_ok();
    assert_eq!(fixture.marker(&fixture.binary(false)), "two");
    assert_eq!(fixture.marker(&fixture.binary(true)), "one");
    fixture.commit_program("three", 3);
    let rejected = fixture.install(
        &[],
        &[("FORGE_SOURCE_INSTALL_TEST_FAIL_ACTIVE_REPLACE", "1")],
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert_eq!(fixture.marker(&fixture.binary(false)), "two");
    assert_eq!(fixture.marker(&fixture.binary(true)), "one");
    assert_eq!(fixture.receipts().len(), 2);
}

#[test]
fn post_replace_failure_reconciles_active_rollback_and_state() {
    let fixture = Fixture::new();
    fixture.install_ok();
    let second_commit = fixture.commit_program("two", 2);
    fixture.install_ok();
    let third_commit = fixture.commit_program("three", 3);
    let rejected = fixture.install(
        &[],
        &[("FORGE_SOURCE_INSTALL_TEST_FAIL_ROLLBACK_REPLACE", "1")],
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert_eq!(fixture.marker(&fixture.binary(false)), "three");
    assert_eq!(fixture.marker(&fixture.binary(true)), "two");
    assert_eq!(fixture.receipts().len(), 2);
    let state = fixture.state();
    assert!(state["active_receipt_id"]
        .as_str()
        .expect("active receipt")
        .starts_with(&third_commit));
    assert!(state["rollback_receipt_id"]
        .as_str()
        .expect("rollback receipt")
        .starts_with(&second_commit));
    assert!(!fixture
        .install_root
        .join("source-install")
        .join("pending.json")
        .exists());
    assert_eq!(fixture.install_ok()["status"], "already_installed");
}

#[test]
fn crash_left_atomic_temps_are_cleaned_without_growth() {
    let fixture = Fixture::new();
    let source_root = fixture.install_root.join("source-install");
    let receipts = source_root.join("receipts");
    fs::create_dir_all(&receipts).expect("create receipts");
    let state_temp = source_root.join(".state.json.123.tmp");
    let receipt_temp = receipts.join(format!(
        ".{}-{}.json.456.tmp",
        "a".repeat(40),
        "b".repeat(64)
    ));
    fs::write(&state_temp, "partial").expect("write state temp");
    fs::write(&receipt_temp, "partial").expect("write receipt temp");
    fixture.install_ok();
    assert!(!state_temp.exists());
    assert!(!receipt_temp.exists());
    assert_eq!(fixture.receipts().len(), 1);
}

#[test]
fn missing_active_with_retained_rollback_fails_closed() {
    let fixture = Fixture::new();
    fixture.install_ok();
    fixture.commit_program("two", 2);
    fixture.install_ok();
    let rollback_before = fs::read(fixture.binary(true)).expect("read rollback");
    let state_path = fixture
        .install_root
        .join("source-install")
        .join("state.json");
    let state_before = fs::read(&state_path).expect("read state");
    fs::remove_file(fixture.binary(false)).expect("remove active");
    let rejected = fixture.install(&[], &[]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("installed binary is missing"));
    assert!(!fixture.binary(false).exists());
    assert_eq!(
        fs::read(fixture.binary(true)).expect("read rollback"),
        rollback_before
    );
    assert_eq!(fs::read(state_path).expect("read state"), state_before);
}

#[test]
fn matching_unmanaged_binary_can_be_adopted_without_duplicate_rollback() {
    let fixture = Fixture::new();
    Fixture::assert_success(
        &Command::new("cargo")
            .args([
                "build",
                "--locked",
                "--release",
                "--package",
                "forge-core-cli",
                "--bin",
                "forge-core",
                "--target-dir",
            ])
            .arg(&fixture.target_dir)
            .current_dir(&fixture.repo)
            .output()
            .expect("build unmanaged fixture"),
    );
    fs::create_dir_all(fixture.binary(false).parent().expect("binary parent")).expect("create bin");
    fs::copy(
        fixture.target_dir.join("release").join(if cfg!(windows) {
            "forge-core.exe"
        } else {
            "forge-core"
        }),
        fixture.binary(false),
    )
    .expect("copy unmanaged binary");
    let output = fixture.install(&["--adopt-current"], &[]);
    assert!(
        output.status.success(),
        "adoption failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse adoption report");
    assert_eq!(report["status"], "adopted_current");
    assert_eq!(fixture.receipts().len(), 1);
    assert!(!fixture.binary(true).exists());
}
