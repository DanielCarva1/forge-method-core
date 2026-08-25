//! Repository automation that must run outside the `forge-core` process.
//!
//! The source-checkpoint installer intentionally stays separate from the
//! trusted prebuilt-release lifecycle. It builds a clean checkout, then replaces
//! the installed executable from this external process so native Windows can
//! update safely.

use fs4::{FileExt, TryLockError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const RECEIPT_SCHEMA: &str = "forge_source_install_receipt_v1";
const STATE_SCHEMA: &str = "forge_source_install_state_v1";
const PENDING_SCHEMA: &str = "forge_source_install_pending_v1";
const KNOWN_BINARY_NAMES: [&str; 3] = ["forge-core.exe", "forge-core", "forge-core.cmd"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallError(String);

impl InstallError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InstallError {}

impl From<std::io::Error> for InstallError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<serde_json::Error> for InstallError {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

type Result<T> = std::result::Result<T, InstallError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInstallOptions {
    pub repo_root: PathBuf,
    pub install_root: PathBuf,
    pub target_dir: Option<PathBuf>,
    pub adopt_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Receipt {
    schema_version: String,
    source_commit: Option<String>,
    source_checkout_clean: bool,
    package_version: String,
    binary_name: String,
    binary_sha256: String,
    binary_size: u64,
    provenance: String,
    installed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InstallState {
    schema_version: String,
    active_receipt_id: String,
    rollback_receipt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PendingInstall {
    schema_version: String,
    candidate_receipt_id: String,
    prior_active_receipt_id: Option<String>,
    binary_name: String,
    staged_rollback_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceInstallReport {
    schema_version: &'static str,
    status: &'static str,
    source_commit: String,
    package_version: String,
    binary_path: PathBuf,
    binary_sha256: String,
    rollback_available: bool,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    target_directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

#[derive(Debug)]
struct InstallPaths {
    source_root: PathBuf,
    bin_dir: PathBuf,
    rollback_dir: PathBuf,
    receipts: PathBuf,
    staging: PathBuf,
}

struct CandidateCheckpoint<'a> {
    path: &'a Path,
    name: &'static str,
    commit: &'a str,
    version: &'a str,
    digest: String,
    receipt: Receipt,
}

struct ActiveCheckpoint {
    path: Option<PathBuf>,
    digest: Option<String>,
    receipt_id: Option<String>,
}

enum ActivePreparation {
    Complete(SourceInstallReport),
    Continue(ActiveCheckpoint),
}

struct InstallTransaction<'a> {
    paths: &'a InstallPaths,
    candidate: &'a CandidateCheckpoint<'a>,
    active: &'a ActiveCheckpoint,
    staged_candidate: PathBuf,
    staged_rollback: PathBuf,
    pending_path: PathBuf,
}

#[derive(Debug)]
struct InstallLock(File);

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

/// Run the repository-tool command line and return a process exit code.
pub fn run_cli(arguments: impl IntoIterator<Item = String>) -> i32 {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return 0;
    }
    match parse_options(&arguments).and_then(|options| install_source_checkpoint(&options)) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                println!("{json}");
                0
            }
            Err(error) => reject_cli(error),
        },
        Err(error) => reject_cli(error),
    }
}

fn reject_cli(error: impl fmt::Display) -> i32 {
    assert!(
        env::var_os("FORGE_SOURCE_INSTALL_DEBUG").as_deref() != Some(OsStr::new("1")),
        "source checkpoint install rejected: {error}"
    );
    eprintln!("source checkpoint install rejected: {error}");
    2
}

fn print_help() {
    println!(
        "forge-xtask source-install [--repo-root <path>] [--install-root <path>] \
         [--target-dir <path>] [--adopt-current]"
    );
}

fn parse_options(arguments: &[String]) -> Result<SourceInstallOptions> {
    if arguments.first().map(String::as_str) != Some("source-install") {
        return Err(InstallError::new(
            "expected subcommand 'source-install'; run with --help for usage",
        ));
    }
    let mut repo_root = env::current_dir()?;
    let mut install_root = default_install_root()?;
    let mut target_dir = None;
    let mut adopt_current = false;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--repo-root" | "--install-root" | "--target-dir" => {
                let flag = arguments[index].clone();
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| InstallError::new(format!("{flag} requires a path value")))?;
                if value.starts_with('-') {
                    return Err(InstallError::new(format!("{flag} requires a path value")));
                }
                match flag.as_str() {
                    "--repo-root" => repo_root = PathBuf::from(value),
                    "--install-root" => install_root = PathBuf::from(value),
                    "--target-dir" => target_dir = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
            }
            "--adopt-current" => adopt_current = true,
            unknown => return Err(InstallError::new(format!("unknown argument: {unknown}"))),
        }
        index += 1;
    }
    Ok(SourceInstallOptions {
        repo_root,
        install_root,
        target_dir,
        adopt_current,
    })
}

fn default_install_root() -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(root) = env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(root).join("Programs").join("forge-core"));
    }
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| InstallError::new("cannot determine the user home directory"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("forge-core"))
}

/// Build and install one exact clean source checkpoint.
///
/// # Errors
///
/// Returns [`InstallError`] when the checkout is not one clean Git checkpoint,
/// the candidate build or version check fails, installed state is unsafe or
/// inconsistent, or the atomic install and recovery transaction cannot finish.
pub fn install_source_checkpoint(options: &SourceInstallOptions) -> Result<SourceInstallReport> {
    let (repo, commit, version, cargo_target_dir) = clean_checkpoint(&options.repo_root)?;
    let install_root = absolute(&options.install_root)?;
    reject_link_components(&install_root)?;
    if install_root.exists() && !install_root.is_dir() {
        return Err(InstallError::new("install root must be a directory"));
    }
    fs::create_dir_all(&install_root)?;
    reject_link_components(&install_root)?;

    let paths = InstallPaths {
        source_root: install_root.join("source-install"),
        bin_dir: install_root.join("bin"),
        rollback_dir: install_root.join("source-install").join("rollback"),
        receipts: install_root.join("source-install").join("receipts"),
        staging: install_root.join("source-install").join("staging"),
    };
    for directory in [
        &paths.source_root,
        &paths.bin_dir,
        &paths.rollback_dir,
        &paths.receipts,
        &paths.staging,
    ] {
        fs::create_dir_all(directory)?;
        reject_link_components(directory)?;
    }

    let _lock = acquire_install_lock(&paths.source_root.join(".install.lock"))?;
    clean_atomic_temps(&paths)?;
    recover_pending(&paths)?;
    clean_staging(&paths.staging)?;
    let reconciled_state = load_state(&paths)?;
    let retained = state_receipt_ids(reconciled_state.as_ref());
    prune_receipts(&paths.receipts, &retained)?;

    let configured_target = options
        .target_dir
        .clone()
        .or_else(|| env::var_os("CARGO_TARGET_DIR").map(PathBuf::from));
    let target_dir = absolute(&configured_target.unwrap_or(cargo_target_dir))?;
    let candidate = build_candidate(&repo, &target_dir)?;
    let observed_version = run_candidate_version(&candidate)?;
    let expected_version = format!("forge-core {version}");
    if observed_version != expected_version {
        return Err(InstallError::new(format!(
            "candidate version mismatch: expected {expected_version:?}, got {observed_version:?}"
        )));
    }
    let (_, observed_commit, observed_package, _) = clean_checkpoint(&repo)?;
    if observed_commit != commit || observed_package != version {
        return Err(InstallError::new("source checkpoint changed during build"));
    }

    install_candidate(&paths, &candidate, &commit, &version, options.adopt_current)
}

fn install_candidate(
    paths: &InstallPaths,
    candidate: &Path,
    commit: &str,
    version: &str,
    adopt_current: bool,
) -> Result<SourceInstallReport> {
    let name = binary_name(candidate);
    let candidate_digest = sha256(candidate)?;
    let candidate_receipt = receipt_for(
        Some(commit.to_owned()),
        version.to_owned(),
        candidate_digest.clone(),
        fs::metadata(candidate)?.len(),
        name.to_owned(),
        "repo_owned_release_build".to_owned(),
    )?;
    let checkpoint = CandidateCheckpoint {
        path: candidate,
        name,
        commit,
        version,
        digest: candidate_digest,
        receipt: candidate_receipt,
    };
    let active = match prepare_active(paths, &checkpoint, adopt_current)? {
        ActivePreparation::Complete(report) => return Ok(report),
        ActivePreparation::Continue(active) => active,
    };
    install_transaction(paths, &checkpoint, &active)
}

fn prepare_active(
    paths: &InstallPaths,
    candidate: &CandidateCheckpoint<'_>,
    adopt_current: bool,
) -> Result<ActivePreparation> {
    let active = current_binary(&paths.bin_dir)?;
    let state = load_state(paths)?;
    let Some(active_path) = active else {
        let stale_rollback = KNOWN_BINARY_NAMES
            .iter()
            .any(|known| paths.rollback_dir.join(known).exists());
        if state.is_some() || stale_rollback {
            return Err(InstallError::new(
                "installed binary is missing while source-install state or rollback remains",
            ));
        }
        return Ok(ActivePreparation::Continue(ActiveCheckpoint {
            path: None,
            digest: None,
            receipt_id: None,
        }));
    };
    if active_path.file_name() != Some(OsStr::new(candidate.name)) {
        return Err(InstallError::new(format!(
            "installed binary name {:?} differs from candidate name {:?}",
            active_path.file_name().unwrap_or_default(),
            candidate.name
        )));
    }
    prepare_existing_active(paths, candidate, active_path, state.as_ref(), adopt_current)
}

fn prepare_existing_active(
    paths: &InstallPaths,
    candidate: &CandidateCheckpoint<'_>,
    active_path: PathBuf,
    state: Option<&InstallState>,
    adopt_current: bool,
) -> Result<ActivePreparation> {
    let digest = sha256(&active_path)?;
    let mut receipt_id = state.map(|value| value.active_receipt_id.clone());
    let mut receipt = match &receipt_id {
        Some(identity) => load_receipt(&paths.receipts, identity)?,
        None => None,
    };
    if receipt
        .as_ref()
        .is_some_and(|value| value.binary_sha256 != digest)
    {
        return Err(InstallError::new(
            "installed binary does not match source-install state",
        ));
    }
    if receipt.is_none() {
        if !adopt_current {
            return Err(InstallError::new(
                "installed binary is unmanaged; rerun once with --adopt-current to preserve it as rollback",
            ));
        }
        if digest == candidate.digest {
            let (candidate_id, _) = write_receipt(&paths.receipts, &candidate.receipt)?;
            write_state(paths, &candidate_id, None)?;
            prune_receipts(&paths.receipts, &BTreeSet::from([candidate_id]))?;
            return Ok(ActivePreparation::Complete(report(
                "adopted_current",
                candidate.commit,
                candidate.version,
                &active_path,
                digest,
                false,
            )));
        }
        let adopted = adopt_unmanaged(&active_path, candidate.name, digest.clone())?;
        let (identity, _) = write_receipt(&paths.receipts, &adopted)?;
        receipt_id = Some(identity);
        receipt = Some(adopted);
    }
    if digest == candidate.digest
        && receipt
            .as_ref()
            .and_then(|value| value.source_commit.as_deref())
            == Some(candidate.commit)
    {
        return already_installed(paths, candidate, &active_path, digest, receipt_id, state)
            .map(ActivePreparation::Complete);
    }
    Ok(ActivePreparation::Continue(ActiveCheckpoint {
        path: Some(active_path),
        digest: Some(digest),
        receipt_id,
    }))
}

fn adopt_unmanaged(active_path: &Path, name: &str, digest: String) -> Result<Receipt> {
    let active_version = run_candidate_version(active_path)?;
    let package_version = active_version
        .strip_prefix("forge-core ")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            InstallError::new("unmanaged installed binary has an invalid version response")
        })?;
    receipt_for(
        None,
        package_version.to_owned(),
        digest,
        fs::metadata(active_path)?.len(),
        name.to_owned(),
        "adopted_unmanaged_binary".to_owned(),
    )
}

fn already_installed(
    paths: &InstallPaths,
    candidate: &CandidateCheckpoint<'_>,
    active_path: &Path,
    digest: String,
    receipt_id: Option<String>,
    state: Option<&InstallState>,
) -> Result<SourceInstallReport> {
    let identity = receipt_id
        .ok_or_else(|| InstallError::new("installed checkpoint is missing its receipt identity"))?;
    let mut retained = BTreeSet::from([identity]);
    let rollback_path = paths.rollback_dir.join(candidate.name);
    let rollback_available = rollback_path.exists();
    if rollback_available {
        let rollback_file = require_regular_file(&rollback_path, "rollback binary")?;
        let rollback_digest = sha256(&rollback_file)?;
        let rollback_identity = state.and_then(|value| value.rollback_receipt_id.clone());
        let rollback_receipt = match &rollback_identity {
            Some(identity) => load_receipt(&paths.receipts, identity)?,
            None => None,
        };
        if rollback_receipt
            .as_ref()
            .map(|receipt| receipt.binary_sha256.as_str())
            != Some(rollback_digest.as_str())
        {
            return Err(InstallError::new(
                "rollback binary has no matching source-install receipt",
            ));
        }
        retained.insert(rollback_identity.expect("receipt was checked"));
    }
    prune_receipts(&paths.receipts, &retained)?;
    Ok(report(
        "already_installed",
        candidate.commit,
        candidate.version,
        active_path,
        digest,
        rollback_available,
    ))
}

fn install_transaction(
    paths: &InstallPaths,
    candidate: &CandidateCheckpoint<'_>,
    active: &ActiveCheckpoint,
) -> Result<SourceInstallReport> {
    let nonce = format!("{}-{}", std::process::id(), unix_nanos()?);
    let transaction = InstallTransaction {
        paths,
        candidate,
        active,
        staged_candidate: paths.staging.join(format!("candidate-{nonce}")),
        staged_rollback: paths.staging.join(format!("rollback-{nonce}")),
        pending_path: paths.source_root.join("pending.json"),
    };
    match transaction.execute() {
        Ok(report) => {
            transaction.cleanup()?;
            Ok(report)
        }
        Err(error) => {
            if let Err(recovery_error) = recover_pending(paths) {
                return Err(InstallError::new(format!(
                    "{error}; recovery also failed: {recovery_error}"
                )));
            }
            transaction.cleanup()?;
            Err(error)
        }
    }
}

impl InstallTransaction<'_> {
    fn execute(&self) -> Result<SourceInstallReport> {
        copy_synced(self.candidate.path, &self.staged_candidate)?;
        if sha256(&self.staged_candidate)? != self.candidate.digest {
            return Err(InstallError::new(
                "staged candidate digest changed during copy",
            ));
        }
        if let (Some(active_path), Some(digest)) = (&self.active.path, &self.active.digest) {
            let rollback_path = self.paths.rollback_dir.join(self.candidate.name);
            if rollback_path.exists() {
                require_regular_file(&rollback_path, "rollback binary")?;
            }
            copy_synced(active_path, &self.staged_rollback)?;
            if sha256(&self.staged_rollback)? != *digest {
                return Err(InstallError::new(
                    "staged rollback digest changed during copy",
                ));
            }
        }
        let (candidate_id, _) = write_receipt(&self.paths.receipts, &self.candidate.receipt)?;
        atomic_json(
            &self.pending_path,
            &PendingInstall {
                schema_version: PENDING_SCHEMA.to_owned(),
                candidate_receipt_id: candidate_id.clone(),
                prior_active_receipt_id: self.active.receipt_id.clone(),
                binary_name: self.candidate.name.to_owned(),
                staged_rollback_name: self.active.path.as_ref().map(|_| {
                    self.staged_rollback
                        .file_name()
                        .expect("staged rollback has a name")
                        .to_string_lossy()
                        .into_owned()
                }),
            },
        )?;

        if env::var_os("FORGE_SOURCE_INSTALL_TEST_FAIL_ACTIVE_REPLACE").as_deref()
            == Some(OsStr::new("1"))
        {
            return Err(InstallError::new("injected active replacement failure"));
        }
        let destination = self.paths.bin_dir.join(self.candidate.name);
        replace_path(&self.staged_candidate, &destination)?;
        if sha256(&destination)? != self.candidate.digest {
            return Err(InstallError::new(
                "installed binary digest differs after atomic replacement",
            ));
        }
        let rollback_receipt_id = self
            .active
            .path
            .as_ref()
            .and(self.active.receipt_id.clone());
        if self.active.path.is_some() && self.active.digest.is_some() {
            if env::var_os("FORGE_SOURCE_INSTALL_TEST_FAIL_ROLLBACK_REPLACE").as_deref()
                == Some(OsStr::new("1"))
            {
                return Err(InstallError::new("injected rollback replacement failure"));
            }
            replace_path(
                &self.staged_rollback,
                &self.paths.rollback_dir.join(self.candidate.name),
            )?;
        }
        write_state(self.paths, &candidate_id, rollback_receipt_id.as_deref())?;
        let mut retained = BTreeSet::from([candidate_id]);
        if let Some(identity) = rollback_receipt_id {
            retained.insert(identity);
        }
        prune_receipts(&self.paths.receipts, &retained)?;
        fs::remove_file(&self.pending_path)?;
        Ok(report(
            "installed",
            self.candidate.commit,
            self.candidate.version,
            &destination,
            self.candidate.digest.clone(),
            self.active.digest.is_some(),
        ))
    }

    fn cleanup(&self) -> Result<()> {
        cleanup_staged_if_reconciled(
            &self.pending_path,
            [&self.staged_candidate, &self.staged_rollback],
        )
    }
}

fn report(
    status: &'static str,
    commit: &str,
    version: &str,
    binary_path: &Path,
    binary_sha256: String,
    rollback_available: bool,
) -> SourceInstallReport {
    SourceInstallReport {
        schema_version: RECEIPT_SCHEMA,
        status,
        source_commit: commit.to_owned(),
        package_version: version.to_owned(),
        binary_path: binary_path.to_path_buf(),
        binary_sha256,
        rollback_available,
    }
}

fn clean_checkpoint(repo_value: &Path) -> Result<(PathBuf, String, String, PathBuf)> {
    let repo = fs::canonicalize(repo_value)
        .map_err(|error| InstallError::new(format!("cannot resolve repository root: {error}")))?;
    let top = fs::canonicalize(git(&repo, &["rev-parse", "--show-toplevel"])?)
        .map_err(|error| InstallError::new(format!("cannot resolve Git root: {error}")))?;
    if !paths_equal(&top, &repo) {
        return Err(InstallError::new(format!(
            "--repo-root must be the exact Git root: {}",
            top.display()
        )));
    }
    let status = git(
        &repo,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    if !status.is_empty() {
        return Err(InstallError::new(
            "source checkout is dirty; commit or remove changes before installation",
        ));
    }
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    if !is_lower_hex(&commit, 40) {
        return Err(InstallError::new(
            "Git did not return one full SHA-1 source commit",
        ));
    }
    let (version, target_directory) = workspace_metadata(&repo)?;
    Ok((repo, commit, version, target_directory))
}

fn workspace_metadata(repo: &Path) -> Result<(String, PathBuf)> {
    let output = run_command(
        Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .current_dir(external_process_path(repo)),
    )?;
    require_success(&output, "cargo metadata failed").and_then(|stdout| {
        let metadata: CargoMetadata = serde_json::from_str(&stdout).map_err(|error| {
            InstallError::new(format!("cannot read workspace package version: {error}"))
        })?;
        let mut versions = metadata
            .packages
            .into_iter()
            .filter(|package| package.name == "forge-core-cli")
            .map(|package| package.version);
        let version = versions
            .next()
            .ok_or_else(|| InstallError::new("workspace does not contain forge-core-cli"))?;
        if versions.next().is_some() || version.trim().is_empty() {
            return Err(InstallError::new(
                "workspace package version must resolve to one non-empty value",
            ));
        }
        Ok((version, metadata.target_directory))
    })
}

fn build_candidate(repo: &Path, target_dir: &Path) -> Result<PathBuf> {
    reject_link_components(target_dir)?;
    fs::create_dir_all(target_dir)?;
    let output = run_command(
        Command::new("cargo")
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
            .arg(target_dir)
            .current_dir(external_process_path(repo)),
    )?;
    require_success(&output, "cargo release build failed")?;
    let name = if cfg!(windows) {
        "forge-core.exe"
    } else {
        "forge-core"
    };
    require_regular_file(&target_dir.join("release").join(name), "built candidate")
}

fn run_candidate_version(candidate: &Path) -> Result<String> {
    let mut command;
    #[cfg(windows)]
    {
        let extension = candidate
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if extension == "cmd" || extension == "bat" {
            command = Command::new(env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()));
            command
                .args(["/d", "/c", "call"])
                .arg(candidate)
                .arg("--version");
        } else {
            command = Command::new(candidate);
            command.arg("--version");
        }
    }
    #[cfg(not(windows))]
    {
        command = Command::new(candidate);
        command.arg("--version");
    }
    let output = run_command(&mut command)?;
    require_success(&output, "candidate --version failed")
}

fn binary_name(candidate: &Path) -> &'static str {
    match candidate
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "exe" => "forge-core.exe",
        "cmd" | "bat" => "forge-core.cmd",
        _ => "forge-core",
    }
}

fn git(repo: &Path, arguments: &[&str]) -> Result<String> {
    let output = run_command(
        Command::new("git")
            .args(arguments)
            .current_dir(external_process_path(repo)),
    )?;
    require_success(&output, &format!("git {} failed", arguments.join(" ")))
}

fn external_process_path(path: &Path) -> &Path {
    dunce::simplified(path)
}

fn run_command(command: &mut Command) -> Result<Output> {
    command.output().map_err(InstallError::from)
}

fn require_success(output: &Output, fallback: &str) -> Result<String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Err(InstallError::new(if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        fallback.to_owned()
    }))
}

fn acquire_install_lock(path: &Path) -> Result<InstallLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    if file.metadata()?.len() == 0 {
        file.write_all(&[0])?;
        file.sync_all()?;
    }
    match FileExt::try_lock(&file) {
        Ok(()) => Ok(InstallLock(file)),
        Err(TryLockError::WouldBlock) => Err(InstallError::new(
            "another source-checkpoint installation is running",
        )),
        Err(TryLockError::Error(error)) => Err(InstallError::new(format!(
            "cannot lock source-checkpoint installation: {error}"
        ))),
    }
}

fn receipt_for(
    commit: Option<String>,
    version: String,
    digest: String,
    size: u64,
    name: String,
    provenance: String,
) -> Result<Receipt> {
    Ok(Receipt {
        schema_version: RECEIPT_SCHEMA.to_owned(),
        source_checkout_clean: commit.is_some(),
        source_commit: commit,
        package_version: version,
        binary_name: name,
        binary_sha256: digest,
        binary_size: size,
        provenance,
        installed_at_unix: unix_seconds()?,
    })
}

fn receipt_id(commit: Option<&str>, digest: &str) -> String {
    format!("{}-{digest}", commit.unwrap_or("adopted"))
}

fn write_receipt(receipts: &Path, receipt: &Receipt) -> Result<(String, bool)> {
    if !is_lower_hex(&receipt.binary_sha256, 64) {
        return Err(InstallError::new("receipt binary digest is invalid"));
    }
    if let Some(commit) = &receipt.source_commit {
        if !is_lower_hex(commit, 40) {
            return Err(InstallError::new("receipt source commit is invalid"));
        }
    }
    let identity = receipt_id(receipt.source_commit.as_deref(), &receipt.binary_sha256);
    let path = receipts.join(format!("{identity}.json"));
    if path.exists() {
        let existing = load_receipt(receipts, &identity)?
            .ok_or_else(|| InstallError::new("existing source-install receipt disappeared"))?;
        let mut stable_existing = existing;
        let mut stable_new = receipt.clone();
        stable_existing.installed_at_unix = 0;
        stable_new.installed_at_unix = 0;
        if stable_existing != stable_new {
            return Err(InstallError::new(
                "receipt identity has conflicting source-install metadata",
            ));
        }
        return Ok((identity, false));
    }
    atomic_json(&path, receipt)?;
    Ok((identity, true))
}

fn load_receipt(receipts: &Path, identity: &str) -> Result<Option<Receipt>> {
    if !valid_receipt_id(identity) {
        return Err(InstallError::new(
            "source-install receipt identity is invalid",
        ));
    }
    let path = receipts.join(format!("{identity}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let receipt: Receipt = load_json_regular(&path, "source-install receipt")?;
    if receipt.schema_version != RECEIPT_SCHEMA
        || receipt_id(receipt.source_commit.as_deref(), &receipt.binary_sha256) != identity
    {
        return Err(InstallError::new(format!(
            "source-install receipt does not bind its filename: {}",
            path.display()
        )));
    }
    Ok(Some(receipt))
}

fn load_state(paths: &InstallPaths) -> Result<Option<InstallState>> {
    let path = paths.source_root.join("state.json");
    if !path.exists() {
        return Ok(None);
    }
    let state: InstallState = load_json_regular(&path, "source-install state")?;
    if state.schema_version != STATE_SCHEMA {
        return Err(InstallError::new("invalid source-install state schema"));
    }
    if load_receipt(&paths.receipts, &state.active_receipt_id)?.is_none() {
        return Err(InstallError::new(
            "source-install state has an invalid active_receipt_id",
        ));
    }
    if let Some(identity) = &state.rollback_receipt_id {
        if load_receipt(&paths.receipts, identity)?.is_none() {
            return Err(InstallError::new(
                "source-install state has an invalid rollback_receipt_id",
            ));
        }
    }
    Ok(Some(state))
}

fn write_state(paths: &InstallPaths, active: &str, rollback: Option<&str>) -> Result<()> {
    atomic_json(
        &paths.source_root.join("state.json"),
        &InstallState {
            schema_version: STATE_SCHEMA.to_owned(),
            active_receipt_id: active.to_owned(),
            rollback_receipt_id: rollback.map(str::to_owned),
        },
    )
}

fn load_pending(paths: &InstallPaths) -> Result<Option<PendingInstall>> {
    let path = paths.source_root.join("pending.json");
    if !path.exists() {
        return Ok(None);
    }
    let pending: PendingInstall = load_json_regular(&path, "source-install pending transaction")?;
    let fields_valid = pending.schema_version == PENDING_SCHEMA
        && load_receipt(&paths.receipts, &pending.candidate_receipt_id)?.is_some()
        && match &pending.prior_active_receipt_id {
            Some(identity) => load_receipt(&paths.receipts, identity)?.is_some(),
            None => true,
        }
        && KNOWN_BINARY_NAMES.contains(&pending.binary_name.as_str())
        && match &pending.staged_rollback_name {
            Some(name) => valid_staging_name(name, "rollback-"),
            None => true,
        };
    if !fields_valid {
        return Err(InstallError::new(
            "invalid source-install pending transaction fields",
        ));
    }
    Ok(Some(pending))
}

fn recover_pending(paths: &InstallPaths) -> Result<()> {
    let Some(pending) = load_pending(paths)? else {
        return Ok(());
    };
    let candidate_receipt = load_receipt(&paths.receipts, &pending.candidate_receipt_id)?
        .ok_or_else(|| InstallError::new("pending candidate receipt is missing"))?;
    let prior_receipt = match &pending.prior_active_receipt_id {
        Some(identity) => load_receipt(&paths.receipts, identity)?,
        None => None,
    };
    let active = current_binary(&paths.bin_dir)?;
    let active_digest = active.as_deref().map(sha256).transpose()?;
    let prior_digest = prior_receipt
        .as_ref()
        .map(|receipt| receipt.binary_sha256.as_str());
    let pending_path = paths.source_root.join("pending.json");

    if active_digest.as_deref() == Some(candidate_receipt.binary_sha256.as_str()) {
        let rollback_id = pending.prior_active_receipt_id.clone();
        if let Some(prior) = prior_receipt {
            let rollback_path = paths.rollback_dir.join(&pending.binary_name);
            if let Some(staged_name) = &pending.staged_rollback_name {
                let staged_path = paths.staging.join(staged_name);
                if staged_path.exists() {
                    let staged = require_regular_file(&staged_path, "staged rollback")?;
                    if sha256(&staged)? != prior.binary_sha256 {
                        return Err(InstallError::new(
                            "pending staged rollback digest is invalid",
                        ));
                    }
                    replace_path(&staged, &rollback_path)?;
                }
            }
            let rollback = require_regular_file(&rollback_path, "rollback binary")?;
            if sha256(&rollback)? != prior.binary_sha256 {
                return Err(InstallError::new(
                    "pending rollback does not match prior active receipt",
                ));
            }
        }
        write_state(paths, &pending.candidate_receipt_id, rollback_id.as_deref())?;
        let mut retained = BTreeSet::from([pending.candidate_receipt_id]);
        if let Some(identity) = rollback_id {
            retained.insert(identity);
        }
        prune_receipts(&paths.receipts, &retained)?;
        fs::remove_file(pending_path)?;
        return Ok(());
    }

    if active_digest.as_deref() == prior_digest || (active.is_none() && prior_receipt.is_none()) {
        let state_ids = state_receipt_ids(load_state(paths)?.as_ref());
        if !state_ids.contains(&pending.candidate_receipt_id) {
            fs::remove_file(
                paths
                    .receipts
                    .join(format!("{}.json", pending.candidate_receipt_id)),
            )?;
        }
        fs::remove_file(pending_path)?;
        return Ok(());
    }

    Err(InstallError::new(
        "cannot safely reconcile interrupted source installation",
    ))
}

fn state_receipt_ids(state: Option<&InstallState>) -> BTreeSet<String> {
    let mut retained = BTreeSet::new();
    if let Some(state) = state {
        retained.insert(state.active_receipt_id.clone());
        if let Some(identity) = &state.rollback_receipt_id {
            retained.insert(identity.clone());
        }
    }
    retained
}

fn current_binary(bin_dir: &Path) -> Result<Option<PathBuf>> {
    let found = KNOWN_BINARY_NAMES
        .iter()
        .map(|name| bin_dir.join(name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    if found.len() > 1 {
        return Err(InstallError::new(
            "install root contains more than one forge-core binary name",
        ));
    }
    found
        .first()
        .map(|path| require_regular_file(path, "installed binary"))
        .transpose()
}

fn prune_receipts(receipts: &Path, retained: &BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(receipts)? {
        let path = entry?.path();
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| InstallError::new("source-install receipt name is not UTF-8"))?;
        let identity = file_name
            .strip_suffix(".json")
            .filter(|value| valid_receipt_id(value))
            .ok_or_else(|| {
                InstallError::new(format!(
                    "unknown source-install receipt entry: {}",
                    path.display()
                ))
            })?;
        require_regular_file(&path, "source-install receipt entry")?;
        load_receipt(receipts, identity)?;
        if !retained.contains(identity) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn clean_staging(staging: &Path) -> Result<()> {
    fs::create_dir_all(staging)?;
    for entry in fs::read_dir(staging)? {
        let path = entry?.path();
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if !(valid_staging_name(name, "candidate-") || valid_staging_name(name, "rollback-")) {
            return Err(InstallError::new(format!(
                "unknown source-install staging entry: {}",
                path.display()
            )));
        }
        require_regular_file(&path, "source-install staging entry")?;
        fs::remove_file(path)?;
    }
    Ok(())
}

fn clean_atomic_temps(paths: &InstallPaths) -> Result<()> {
    for entry in fs::read_dir(&paths.source_root)? {
        let path = entry?.path();
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if valid_atomic_state_temp(name) {
            require_regular_file(&path, "source-install temporary")?;
            fs::remove_file(path)?;
        }
    }
    for entry in fs::read_dir(&paths.receipts)? {
        let path = entry?.path();
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if valid_atomic_receipt_temp(name) {
            require_regular_file(&path, "source-install temporary")?;
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn cleanup_staged_if_reconciled<'a>(
    pending_path: &Path,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<()> {
    if pending_path.exists() {
        return Ok(());
    }
    for path in paths {
        if path.exists() {
            require_regular_file(path, "reconciled staging file")?;
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn copy_synced(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination)?;
    fs::set_permissions(destination, fs::metadata(source)?.permissions())?;
    File::options().write(true).open(destination)?.sync_all()?;
    Ok(())
}

fn atomic_json(path: &Path, document: &impl Serialize) -> Result<()> {
    let mut payload = serde_json::to_vec_pretty(document)?;
    payload.push(b'\n');
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| InstallError::new("JSON state path has no UTF-8 filename"))?;
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&payload)?;
        file.sync_all()?;
        replace_path(&temporary, path)
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn load_json_regular<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T> {
    let path = require_regular_file(path, label)?;
    let bytes = fs::read(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| InstallError::new(format!("invalid {label} {}: {error}", path.display())))
}

fn sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn require_regular_file(path: &Path, label: &str) -> Result<PathBuf> {
    let absolute = absolute(path)?;
    let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
        InstallError::new(format!(
            "{label} must be an existing regular non-link file: {} ({error})",
            absolute.display()
        ))
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(InstallError::new(format!(
            "{label} must be an existing regular non-link file: {}",
            absolute.display()
        )));
    }
    Ok(absolute)
}

fn reject_link_components(path: &Path) -> Result<()> {
    let mut current = Some(absolute(path)?);
    while let Some(component) = current {
        match fs::symlink_metadata(&component) {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(InstallError::new(format!(
                    "source install path contains a link or reparse point: {}",
                    component.display()
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        current = component.parent().map(Path::to_path_buf);
    }
    Ok(())
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn valid_receipt_id(value: &str) -> bool {
    if let Some(digest) = value.strip_prefix("adopted-") {
        return is_lower_hex(digest, 64);
    }
    let bytes = value.as_bytes();
    bytes.len() == 105
        && lower_hex_bytes(&bytes[..40])
        && bytes[40] == b'-'
        && lower_hex_bytes(&bytes[41..])
}

fn valid_staging_name(value: &str, prefix: &str) -> bool {
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    let mut parts = rest.split('-');
    matches!((parts.next(), parts.next(), parts.next()), (Some(a), Some(b), None) if digits(a) && digits(b))
}

fn valid_atomic_state_temp(value: &str) -> bool {
    [".state.json.", ".pending.json."].iter().any(|prefix| {
        value
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(".tmp"))
            .is_some_and(digits)
    })
}

fn valid_atomic_receipt_temp(value: &str) -> bool {
    let Some(rest) = value.strip_prefix('.') else {
        return false;
    };
    let Some((identity, suffix)) = rest.rsplit_once(".json.") else {
        return false;
    };
    valid_receipt_id(identity) && suffix.strip_suffix(".tmp").is_some_and(digits)
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length && lower_hex_bytes(value.as_bytes())
}

fn lower_hex_bytes(value: &[u8]) -> bool {
    value
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn unix_seconds() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| InstallError::new(format!("system clock is before Unix epoch: {error}")))
}

fn unix_nanos() -> Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|error| InstallError::new(format!("system clock is before Unix epoch: {error}")))
}

#[cfg(not(windows))]
fn replace_path(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).map_err(InstallError::from)
}

#[cfg(windows)]
fn replace_path(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let destination_display = destination.display().to_string();
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are NUL-terminated and live for the duration of the call.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(InstallError::new(format!(
            "cannot atomically replace {}: {}",
            destination_display,
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{acquire_install_lock, valid_receipt_id};
    use std::{fs, path::PathBuf, time::SystemTime};

    #[test]
    fn install_lock_rejects_a_second_writer() {
        let root = std::env::temp_dir().join(format!(
            "forge-source-install-lock-{}-{:?}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create lock root");
        let path = root.join(".install.lock");
        let first = acquire_install_lock(&path).expect("first writer acquires lock");
        let second = acquire_install_lock(&path).expect_err("second writer must be rejected");
        assert!(
            second
                .to_string()
                .contains("another source-checkpoint installation is running"),
            "{second}"
        );
        drop(first);
        acquire_install_lock(&path).expect("lock is reusable after release");
        cleanup_temp_root(root);
    }

    #[test]
    fn non_ascii_receipt_name_is_rejected_without_slicing_panic() {
        let value = format!("{}é{}", "a".repeat(39), "b".repeat(64));
        assert_eq!(value.len(), 105);
        assert!(!valid_receipt_id(&value));
    }

    #[cfg(windows)]
    #[test]
    fn external_processes_receive_a_legacy_windows_path_when_safe() {
        let canonical = PathBuf::from(r"\\?\D:\Forge-method-core");
        assert_eq!(
            super::external_process_path(&canonical),
            PathBuf::from(r"D:\Forge-method-core")
        );
    }

    fn cleanup_temp_root(root: PathBuf) {
        assert!(root.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(root).expect("remove lock root");
    }
}
