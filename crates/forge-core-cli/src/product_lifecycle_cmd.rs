//! Owned product setup, diagnostics, install, update, rollback, and uninstall.
//!
//! The lifecycle root is separate from consumer projects and Forge sidecars. A
//! closed ownership marker is required before mutation. Releases are verified
//! completely before publication into immutable generation directories, and
//! uninstall removes only files whose exact path and digest remain in the
//! product-owned inventory. Unknown or modified files are retained.

use crate::cli_error::ExitError;
use crate::io_util::{atomic_write, read_regular_file_no_follow_bounded};
use crate::{
    run_host_adapter_artifact_verification, run_host_adapter_distribution_admission,
    run_host_adapter_provenance_verification,
    run_host_adapter_sigstore_dsse_in_toto_subject_verification,
    HostAdapterArtifactVerificationInput, HostAdapterArtifactVerificationStatus,
    HostAdapterDistributionAdmissionStatus, HostAdapterDistributionEvidence,
    HostAdapterProvenanceVerificationInput, HostAdapterProvenanceVerificationStatus,
    HostAdapterSigstoreDsseInTotoSubjectVerificationInput,
    HostAdapterSigstoreDsseInTotoSubjectVerificationStatus, HostAdapterUpdateChannel,
};
use forge_core_command_surface::COMMAND_LIFECYCLE;
use forge_core_contracts::{
    CliEnvelope, ExitReason, ProductLifecycleAssetKind, ProductLifecycleChange,
    ProductLifecycleChannel, ProductLifecycleReleaseDocument,
    ProductLifecycleTrustedVerificationInputDocument, RuntimeKind,
};
use forge_core_store::{
    acquire_effect_store_lock,
    product_lifecycle::{
        ProductLifecycleAsset as StoreProductLifecycleAsset, ProductLifecycleAssetInput,
        ProductLifecycleAssetObservationStatus, ProductLifecycleGeneration,
        ProductLifecycleGenerationInput, ProductLifecycleState as StoreProductLifecycleState,
        ProductLifecycleStore, ProductLifecycleStoreError,
    },
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

const LIFECYCLE_SCHEMA_VERSION: &str = "0.1";
const LIFECYCLE_OWNER: &str = "forge-core-product-lifecycle";
const MARKER_FILE: &str = ".forge-product-lifecycle.json";
const LOCK_FILE: &str = ".lifecycle.lock";
const STORE_ROOT_DIR: &str = "product-lifecycle";
const MAX_RELEASE_DOCUMENT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleOwnershipMarker {
    schema_version: String,
    owner: String,
    product: String,
    authority_boundary: String,
}

impl LifecycleOwnershipMarker {
    fn canonical() -> Self {
        Self {
            schema_version: LIFECYCLE_SCHEMA_VERSION.to_owned(),
            owner: LIFECYCLE_OWNER.to_owned(),
            product: "forge-core".to_owned(),
            authority_boundary: "owns only exact inventory below this install root; consumer projects, Forge sidecars, operator anchors, backups, external broker keys, signing keys, trust roots, and private keys remain outside lifecycle custody".to_owned(),
        }
    }

    fn valid(&self) -> bool {
        self == &Self::canonical()
    }
}

/// Rich release metadata projected from immutable Store receipts. This view is
/// never serialized and cannot select a durable active or previous generation;
/// those identities come only from `ProductLifecycleStore::read_state`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LifecycleReportingState {
    active_generation: Option<String>,
    previous_generation: Option<String>,
    generations: Vec<InstalledGeneration>,
}

impl LifecycleReportingState {
    fn generation(&self, id: &str) -> Option<&InstalledGeneration> {
        self.generations
            .iter()
            .find(|generation| generation.generation_id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledGeneration {
    generation_id: String,
    release_id: String,
    version: String,
    release_sha256: String,
    receipt_sha256: String,
    channel: ProductLifecycleChannel,
    assets: Vec<InstalledAsset>,
    changes: Vec<ProductLifecycleChange>,
}

impl InstalledGeneration {
    fn validate(&self) -> Result<(), LifecycleError> {
        if !safe_generation_id(&self.generation_id)
            || self.release_id.trim().is_empty()
            || !valid_sha256(&self.release_sha256)
            || !valid_sha256(&self.receipt_sha256)
            || Version::parse(&self.version).is_err()
        {
            return Err(LifecycleError::invalid(format!(
                "generation '{}' has invalid immutable identity metadata",
                self.generation_id
            )));
        }
        let mut paths = BTreeSet::new();
        for asset in &self.assets {
            let host_shape_valid = match asset.kind {
                ProductLifecycleAssetKind::HostAdapter
                | ProductLifecycleAssetKind::HostConfiguration => asset.host.is_some(),
                ProductLifecycleAssetKind::CoreBinary | ProductLifecycleAssetKind::Wrapper => {
                    asset.host.is_none()
                }
            };
            if asset.asset_id.trim().is_empty()
                || !safe_relative_path(&asset.install_path)
                || !valid_sha256(&asset.sha256)
                || !paths.insert(asset.install_path.as_str())
                || !host_shape_valid
            {
                return Err(LifecycleError::invalid(format!(
                    "generation '{}' has invalid or duplicate asset inventory",
                    self.generation_id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledAsset {
    asset_id: String,
    kind: ProductLifecycleAssetKind,
    install_path: String,
    sha256: String,
    executable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<RuntimeKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleGenerationReceipt {
    schema_version: String,
    owner: String,
    generation_id: String,
    release_id: String,
    version: String,
    release_sha256: String,
    channel: ProductLifecycleChannel,
    assets: Vec<InstalledAsset>,
    changes: Vec<ProductLifecycleChange>,
    verification_boundary: String,
}

impl LifecycleGenerationReceipt {
    fn into_reporting_generation(self, receipt_sha256: String) -> InstalledGeneration {
        InstalledGeneration {
            generation_id: self.generation_id,
            release_id: self.release_id,
            version: self.version,
            release_sha256: self.release_sha256,
            receipt_sha256,
            channel: self.channel,
            assets: self.assets,
            changes: self.changes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductLifecycleStatus {
    Setup,
    AlreadySetup,
    NotSetup,
    UnmanagedRoot,
    Installed,
    AlreadyInstalled,
    Updated,
    RolledBack,
    Uninstalled,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostConfigurationStatus {
    Exact,
    Missing,
    DigestMismatch,
    UnsafeFileType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostConfigurationObservation {
    pub host: RuntimeKind,
    pub install_path: String,
    pub expected_sha256: String,
    pub observed_sha256: Option<String>,
    pub status: HostConfigurationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductLifecycleReport {
    pub status: ProductLifecycleStatus,
    pub install_root: String,
    pub marker_valid: bool,
    pub active_generation: Option<String>,
    pub active_version: Option<String>,
    pub previous_generation: Option<String>,
    pub previous_version: Option<String>,
    pub selected_host: Option<RuntimeKind>,
    pub host_configurations: Vec<HostConfigurationObservation>,
    pub release_notes: Vec<ProductLifecycleChange>,
    pub diagnostics: Vec<String>,
    pub preserved_paths: Vec<String>,
    pub verification_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LifecycleError {
    exit_reason: ExitReason,
    message: String,
}

impl LifecycleError {
    fn rejected(message: impl Into<String>) -> Self {
        Self {
            exit_reason: ExitReason::RejectedByGate,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            exit_reason: ExitReason::InvalidDecisionShape,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            exit_reason: ExitReason::Conflict,
            message: message.into(),
        }
    }

    fn environment(message: impl Into<String>) -> Self {
        Self {
            exit_reason: ExitReason::EnvConfig,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

fn open_lifecycle_store(root: &Path) -> Result<ProductLifecycleStore, LifecycleError> {
    let lock = acquire_effect_store_lock(root, LOCK_FILE).map_err(|error| {
        LifecycleError::conflict(format!("acquire lifecycle Store lock: {error}"))
    })?;
    ProductLifecycleStore::from_effect_lock(lock)
        .map_err(|error| LifecycleError::environment(format!("open lifecycle Store: {error}")))
}

#[derive(Debug, Clone)]
struct LifecycleArgs {
    install_root: PathBuf,
    release_file: Option<PathBuf>,
    trusted_verification_file: Option<PathBuf>,
    explicit_canary_opt_in: bool,
    want_json: bool,
}

/// Dispatch the public `forge-core lifecycle` command family.
///
/// # Errors
///
/// Returns usage errors for malformed argv and envelope-derived errors when a
/// lifecycle gate, filesystem operation, or integrity check fails.
///
/// # Panics
///
/// Panics only if the internal command-surface specification marks an install
/// or update subcommand as valid without its required `--release-file` value.
pub fn run_product_lifecycle_command(args: &[String]) -> Result<(), ExitError> {
    let subcommand = args.get(1).map_or("--help", String::as_str);
    if matches!(subcommand, "--help" | "-h" | "help") {
        println!("{}", lifecycle_usage());
        return Ok(());
    }
    if !matches!(
        subcommand,
        "setup" | "status" | "doctor" | "install" | "update" | "rollback" | "uninstall"
    ) {
        return Err(ExitError::usage(format!(
            "forge-core lifecycle: unknown subcommand '{subcommand}'. Try: {}",
            COMMAND_LIFECYCLE.concrete_subcommand_hint()
        )));
    }
    let parsed = parse_lifecycle_args(subcommand, &args[2..])?;
    let command = format!("lifecycle {subcommand}");
    let result = match subcommand {
        "setup" => setup(&parsed.install_root),
        "status" => inspect(&parsed.install_root, false),
        "doctor" => inspect(&parsed.install_root, true),
        "install" => install_or_update(
            &parsed.install_root,
            parsed
                .release_file
                .as_deref()
                .expect("parser requires release-file"),
            parsed
                .trusted_verification_file
                .as_deref()
                .expect("parser requires trusted-verification-file"),
            parsed.explicit_canary_opt_in,
            InstallMode::Install,
        ),
        "update" => install_or_update(
            &parsed.install_root,
            parsed
                .release_file
                .as_deref()
                .expect("parser requires release-file"),
            parsed
                .trusted_verification_file
                .as_deref()
                .expect("parser requires trusted-verification-file"),
            parsed.explicit_canary_opt_in,
            InstallMode::Update,
        ),
        "rollback" => rollback(&parsed.install_root),
        "uninstall" => uninstall(&parsed.install_root),
        _ => unreachable!("closed subcommand match"),
    };
    emit_lifecycle_result(&command, result, parsed.want_json)
}

fn parse_lifecycle_args(subcommand: &str, args: &[String]) -> Result<LifecycleArgs, ExitError> {
    let mut install_root = None;
    let mut release_file = None;
    let mut trusted_verification_file = None;
    let mut explicit_canary_opt_in = false;
    let mut want_json = true;
    let mut cursor = crate::cli_util::ArgvCursor::new(args, 0, "lifecycle");
    while let Some(flag) = cursor.peek_flag() {
        match flag {
            "--install-root" => {
                install_root = Some(PathBuf::from(cursor.expect_value("install-root")?));
            }
            "--release-file" => {
                release_file = Some(PathBuf::from(cursor.expect_value("release-file")?));
            }
            "--trusted-verification-file" => {
                trusted_verification_file = Some(PathBuf::from(
                    cursor.expect_value("trusted-verification-file")?,
                ));
            }
            "--explicit-canary-opt-in" => {
                explicit_canary_opt_in = true;
                cursor.advance();
            }
            "--json" => {
                want_json = true;
                cursor.advance();
            }
            "--no-json" | "--text" => {
                want_json = false;
                cursor.advance();
            }
            "--help" | "-h" => {
                return Err(ExitError::usage(lifecycle_subcommand_usage(subcommand)));
            }
            _ => return Err(ExitError::usage(lifecycle_subcommand_usage(subcommand))),
        }
    }
    let install_root =
        install_root.ok_or_else(|| ExitError::usage(lifecycle_subcommand_usage(subcommand)))?;
    let needs_release = matches!(subcommand, "install" | "update");
    if needs_release != release_file.is_some()
        || needs_release != trusted_verification_file.is_some()
        || (!needs_release && explicit_canary_opt_in)
        || install_root.as_os_str().is_empty()
    {
        return Err(ExitError::usage(lifecycle_subcommand_usage(subcommand)));
    }
    Ok(LifecycleArgs {
        install_root,
        release_file,
        trusted_verification_file,
        explicit_canary_opt_in,
        want_json,
    })
}

fn lifecycle_usage() -> String {
    let mut usage = String::from("forge-core lifecycle <subcommand> [options]");
    for line in COMMAND_LIFECYCLE.local_usage_lines() {
        usage.push('\n');
        usage.push_str("  ");
        usage.push_str(line);
    }
    usage
}

fn lifecycle_subcommand_usage(subcommand: &str) -> String {
    COMMAND_LIFECYCLE
        .usage_line_for_subcommand(subcommand)
        .map_or_else(lifecycle_usage, |line| format!("usage:\n  {line}"))
}

fn emit_lifecycle_result(
    command: &str,
    result: Result<ProductLifecycleReport, LifecycleError>,
    want_json: bool,
) -> Result<(), ExitError> {
    let envelope = match result {
        Ok(report) => CliEnvelope::ok(command, report),
        Err(error) => CliEnvelope::err(command, error.exit_reason, error.message),
    };
    crate::cli_util::emit_envelope(envelope, want_json)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupCheckpoint {
    Continue,
    #[cfg(test)]
    InterruptAfterMarkerValidation,
}

fn setup(root: &Path) -> Result<ProductLifecycleReport, LifecycleError> {
    setup_with_checkpoint(root, SetupCheckpoint::Continue)
}

fn setup_with_checkpoint(
    root: &Path,
    checkpoint: SetupCheckpoint,
) -> Result<ProductLifecycleReport, LifecycleError> {
    #[cfg(not(test))]
    let _ = checkpoint;
    reject_symlink_ancestors(root)?;
    if root.exists() {
        let metadata =
            fs::symlink_metadata(root).map_err(io_environment("inspect install root"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(LifecycleError::invalid(format!(
                "install root '{}' must be a real directory",
                root.display()
            )));
        }
    } else {
        fs::create_dir_all(root).map_err(io_environment("create install root"))?;
    }

    let marker_path = root.join(MARKER_FILE);
    let status = if marker_path.exists() {
        load_marker(root)?;
        ProductLifecycleStatus::AlreadySetup
    } else {
        let unexpected = direct_entries_except(root, &[])?;
        if !unexpected.is_empty() {
            return Err(LifecycleError::rejected(format!(
                "refusing to claim nonempty unmanaged install root '{}': {}",
                root.display(),
                unexpected.join(", ")
            )));
        }
        // Publish the exact root claim before creating Store-owned layout. A
        // retry can therefore complete interrupted setup without adopting any
        // retained unknown files that appear after the claim.
        write_json(&marker_path, &LifecycleOwnershipMarker::canonical())?;
        load_marker(root)?;
        #[cfg(test)]
        if checkpoint == SetupCheckpoint::InterruptAfterMarkerValidation {
            return Err(LifecycleError::environment(
                "setup interrupted after durable ownership marker publication",
            ));
        }
        ProductLifecycleStatus::Setup
    };

    let store = open_lifecycle_store(root)?;
    store
        .setup()
        .map_err(store_environment("initialize lifecycle Store"))?;
    let read = store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?;
    let reporting = read
        .state
        .as_ref()
        .map(|state| load_reporting_state(&store, state))
        .transpose()?;
    Ok(report_for_state(
        root,
        status,
        reporting,
        Vec::new(),
        Vec::new(),
    ))
}

fn inspect(root: &Path, doctor: bool) -> Result<ProductLifecycleReport, LifecycleError> {
    if !root.exists() {
        return Ok(base_report(root, ProductLifecycleStatus::NotSetup, false));
    }
    reject_symlink_ancestors(root)?;
    let marker_path = root.join(MARKER_FILE);
    if !marker_path.exists() {
        let mut report = base_report(root, ProductLifecycleStatus::UnmanagedRoot, false);
        report
            .diagnostics
            .push("ownership_marker_missing_no_mutation_allowed".to_owned());
        return Ok(report);
    }
    let marker = load_marker(root)?;
    if !root.join(STORE_ROOT_DIR).join("owner.json").is_file() {
        let mut report = base_report(root, ProductLifecycleStatus::Degraded, marker.valid());
        report
            .diagnostics
            .push("lifecycle_store_setup_incomplete".to_owned());
        return Ok(report);
    }

    let store = open_lifecycle_store(root)?;
    let read = store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?;
    let reporting = read
        .state
        .as_ref()
        .map(|state| load_reporting_state(&store, state))
        .transpose()?;
    let observation = match (&read.state, &reporting) {
        (Some(state), Some(reporting)) => inspect_state(&store, state, reporting, doctor)?,
        _ => StateInspection::default(),
    };
    let status = if observation.diagnostics.is_empty() {
        ProductLifecycleStatus::Healthy
    } else {
        ProductLifecycleStatus::Degraded
    };
    let mut report = report_for_state(root, status, reporting, observation.diagnostics, Vec::new());
    report.marker_valid = marker.valid();
    report.host_configurations = observation.host_configurations;
    Ok(report)
}

#[derive(Default)]
struct StateInspection {
    diagnostics: Vec<String>,
    host_configurations: Vec<HostConfigurationObservation>,
}

fn inspect_state(
    store: &ProductLifecycleStore,
    state: &StoreProductLifecycleState,
    reporting: &LifecycleReportingState,
    doctor: bool,
) -> Result<StateInspection, LifecycleError> {
    let mut inspection = StateInspection::default();
    let Some(active_id) = state.active_generation.as_deref() else {
        if state.previous_generation.is_some() {
            inspection
                .diagnostics
                .push("previous_generation_present_without_active_generation".to_owned());
        }
        return Ok(inspection);
    };
    let active_store = state
        .generations
        .iter()
        .find(|generation| generation.generation_id == active_id)
        .ok_or_else(|| {
            LifecycleError::invalid("active generation is absent from Store inventory")
        })?;
    let active = reporting
        .generation(active_id)
        .ok_or_else(|| LifecycleError::invalid("active generation receipt is absent"))?;
    for asset in &active.assets {
        let stored_asset = store_asset_for(active_store, asset)?;
        let observed = store
            .observe_generation_asset(active_store, stored_asset)
            .map_err(store_environment("observe lifecycle Store asset"))?;
        let status = host_status_from_store(observed.status);
        if status != HostConfigurationStatus::Exact {
            inspection.diagnostics.push(format!(
                "active_asset_not_exact:{}:{}",
                asset.install_path,
                host_configuration_status_name(status)
            ));
        }
        if asset.kind == ProductLifecycleAssetKind::HostConfiguration {
            inspection
                .host_configurations
                .push(HostConfigurationObservation {
                    host: asset.host.ok_or_else(|| {
                        LifecycleError::invalid("host configuration receipt omits its host")
                    })?,
                    install_path: asset.install_path.clone(),
                    expected_sha256: asset.sha256.clone(),
                    observed_sha256: observed.observed_sha256,
                    status,
                });
        }
    }
    if doctor {
        for generation in &state.generations {
            if let Err(error) = store.verify_generation(generation) {
                inspection.diagnostics.push(format!(
                    "generation_not_exact:{}:{error}",
                    generation.generation_id
                ));
            }
        }
    }
    Ok(inspection)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallMode {
    Install,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublicationCheckpoint {
    Continue,
    #[cfg(test)]
    InterruptAfterGenerationPublication,
}

#[derive(Debug, Clone, Copy)]
enum TrustedVerificationMode<'a> {
    Required(&'a Path),
    #[cfg(test)]
    LifecycleMechanicsFixture,
}

fn install_or_update(
    root: &Path,
    release_file: &Path,
    trusted_verification_file: &Path,
    explicit_canary_opt_in: bool,
    mode: InstallMode,
) -> Result<ProductLifecycleReport, LifecycleError> {
    install_or_update_with_checkpoint(
        root,
        release_file,
        trusted_verification_file,
        explicit_canary_opt_in,
        mode,
        PublicationCheckpoint::Continue,
    )
}

fn install_or_update_with_checkpoint(
    root: &Path,
    release_file: &Path,
    trusted_verification_file: &Path,
    explicit_canary_opt_in: bool,
    mode: InstallMode,
    checkpoint: PublicationCheckpoint,
) -> Result<ProductLifecycleReport, LifecycleError> {
    install_or_update_with_verification(
        root,
        release_file,
        TrustedVerificationMode::Required(trusted_verification_file),
        explicit_canary_opt_in,
        mode,
        checkpoint,
    )
}

#[cfg(test)]
fn install_or_update_lifecycle_mechanics_fixture(
    root: &Path,
    release_file: &Path,
    trusted_verification_file: &Path,
    explicit_canary_opt_in: bool,
    mode: InstallMode,
) -> Result<ProductLifecycleReport, LifecycleError> {
    install_or_update_lifecycle_mechanics_fixture_with_checkpoint(
        root,
        release_file,
        trusted_verification_file,
        explicit_canary_opt_in,
        mode,
        PublicationCheckpoint::Continue,
    )
}

#[cfg(test)]
fn install_or_update_lifecycle_mechanics_fixture_with_checkpoint(
    root: &Path,
    release_file: &Path,
    trusted_verification_file: &Path,
    explicit_canary_opt_in: bool,
    mode: InstallMode,
    checkpoint: PublicationCheckpoint,
) -> Result<ProductLifecycleReport, LifecycleError> {
    let _ = trusted_verification_file;
    install_or_update_with_verification(
        root,
        release_file,
        TrustedVerificationMode::LifecycleMechanicsFixture,
        explicit_canary_opt_in,
        mode,
        checkpoint,
    )
}

fn install_or_update_with_verification(
    root: &Path,
    release_file: &Path,
    trusted_verification: TrustedVerificationMode<'_>,
    explicit_canary_opt_in: bool,
    mode: InstallMode,
    checkpoint: PublicationCheckpoint,
) -> Result<ProductLifecycleReport, LifecycleError> {
    #[cfg(not(test))]
    let _ = checkpoint;
    reject_symlink_ancestors(root)?;
    load_marker(root)?;

    // Candidate bytes and all production trusted evidence are freshly verified
    // before any Store generation or durable selector can be mutated. The
    // test-only mode isolates lifecycle state-machine tests from cryptographic
    // fixture construction; no production caller can select it.
    let loaded = load_and_verify_release_with_mode(
        release_file,
        trusted_verification,
        explicit_canary_opt_in,
    )?;

    let store = open_lifecycle_store(root)?;
    store
        .setup()
        .map_err(store_environment("initialize lifecycle Store"))?;
    let store_read = store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?;
    let mut store_state = store_read
        .state
        .clone()
        .unwrap_or_else(StoreProductLifecycleState::empty);
    let mut reporting = load_reporting_state(&store, &store_state)?;

    if let Some(existing) = store_state
        .generations
        .iter()
        .find(|generation| generation.version == loaded.version)
    {
        if existing.release_sha256 != loaded.release_sha256 {
            return Err(LifecycleError::conflict(format!(
                "release version {} already exists with different immutable content",
                loaded.version
            )));
        }
        if store_state.active_generation.as_deref() == Some(existing.generation_id.as_str()) {
            store
                .verify_generation(existing)
                .map_err(store_conflict("verify exact lifecycle Store generation"))?;
            return Ok(report_for_state(
                root,
                ProductLifecycleStatus::AlreadyInstalled,
                Some(reporting),
                vec!["exact_release_retry_did_not_append_a_generation".to_owned()],
                Vec::new(),
            ));
        }
    }

    let active = store_state.active_generation.as_deref().and_then(|id| {
        store_state
            .generations
            .iter()
            .find(|generation| generation.generation_id == id)
    });
    match mode {
        InstallMode::Install if active.is_some() => {
            return Err(LifecycleError::rejected(
                "an active release already exists; use lifecycle update",
            ));
        }
        InstallMode::Update if active.is_none() => {
            return Err(LifecycleError::rejected(
                "no active release exists; use lifecycle install",
            ));
        }
        InstallMode::Update => {
            let current =
                Version::parse(&active.expect("checked active").version).map_err(|error| {
                    LifecycleError::invalid(format!("active version invalid: {error}"))
                })?;
            let candidate = Version::parse(&loaded.version).expect("release version validated");
            if candidate <= current {
                return Err(LifecycleError::rejected(format!(
                    "update version {candidate} must be newer than active version {current}"
                )));
            }
        }
        InstallMode::Install => {}
    }

    let mut verified_selected = BTreeSet::new();
    for selected in [
        store_state.active_generation.as_deref(),
        store_state.previous_generation.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if verified_selected.insert(selected) {
            let generation = store_state
                .generations
                .iter()
                .find(|generation| generation.generation_id == selected)
                .ok_or_else(|| {
                    LifecycleError::invalid("selected generation is absent from Store inventory")
                })?;
            store
                .verify_generation(generation)
                .map_err(store_conflict("verify selected lifecycle Store generation"))?;
        }
    }

    let receipt = LifecycleGenerationReceipt {
        schema_version: LIFECYCLE_SCHEMA_VERSION.to_owned(),
        owner: LIFECYCLE_OWNER.to_owned(),
        generation_id: loaded.generation_id.clone(),
        release_id: loaded
            .document
            .product_lifecycle_release
            .release_id
            .0
            .clone(),
        version: loaded.version.clone(),
        release_sha256: loaded.release_sha256.clone(),
        channel: loaded.document.product_lifecycle_release.channel,
        assets: loaded
            .assets
            .iter()
            .map(|asset| asset.inventory.clone())
            .collect(),
        changes: loaded.document.product_lifecycle_release.changes.clone(),
        verification_boundary: verification_boundary().to_owned(),
    };
    let receipt_bytes = canonical_json(&receipt)?;
    let generation = receipt
        .clone()
        .into_reporting_generation(digest(&receipt_bytes));
    generation.validate()?;
    let store_generation = store
        .publish_generation(&ProductLifecycleGenerationInput {
            generation_id: generation.generation_id.clone(),
            version: generation.version.clone(),
            release_sha256: generation.release_sha256.clone(),
            receipt: receipt_bytes,
            assets: loaded
                .assets
                .iter()
                .map(|asset| ProductLifecycleAssetInput {
                    path: asset.inventory.install_path.clone(),
                    sha256: asset.inventory.sha256.clone(),
                    bytes: asset.bytes.clone(),
                    executable: asset.inventory.executable,
                })
                .collect(),
        })
        .map_err(store_environment("publish lifecycle Store generation"))?;
    #[cfg(test)]
    if checkpoint == PublicationCheckpoint::InterruptAfterGenerationPublication {
        return Err(LifecycleError::environment(
            "lifecycle update interrupted after immutable generation publication",
        ));
    }

    let previous = store_state.active_generation.clone();
    match store_state
        .generations
        .iter()
        .find(|stored| stored.generation_id == store_generation.generation_id)
    {
        Some(stored) if stored != &store_generation => {
            return Err(LifecycleError::conflict(
                "published generation differs from durable Store inventory",
            ));
        }
        Some(_) => {}
        None => {
            store_state.generations.push(store_generation);
            store_state
                .generations
                .sort_by(|left, right| left.generation_id.cmp(&right.generation_id));
        }
    }
    store_state.previous_generation.clone_from(&previous);
    store_state.active_generation = Some(generation.generation_id.clone());
    store
        .compare_and_swap_state(store_read.digest.as_deref(), &store_state)
        .map_err(store_conflict("publish lifecycle Store state"))?;

    if reporting.generation(&generation.generation_id).is_none() {
        reporting.generations.push(generation.clone());
        reporting
            .generations
            .sort_by(|left, right| left.generation_id.cmp(&right.generation_id));
    }
    reporting.previous_generation = previous;
    reporting.active_generation = Some(generation.generation_id);
    Ok(report_for_state(
        root,
        match mode {
            InstallMode::Install => ProductLifecycleStatus::Installed,
            InstallMode::Update => ProductLifecycleStatus::Updated,
        },
        Some(reporting),
        Vec::new(),
        Vec::new(),
    ))
}

struct LoadedRelease {
    document: ProductLifecycleReleaseDocument,
    release_sha256: String,
    generation_id: String,
    version: String,
    assets: Vec<VerifiedAsset>,
}

struct VerifiedAsset {
    inventory: InstalledAsset,
    bytes: Vec<u8>,
}

fn load_and_verify_release_with_mode(
    release_file: &Path,
    trusted_verification: TrustedVerificationMode<'_>,
    explicit_canary_opt_in: bool,
) -> Result<LoadedRelease, LifecycleError> {
    reject_symlink_ancestors(release_file)?;
    let release_bytes =
        read_regular_file_no_follow_bounded(release_file, MAX_RELEASE_DOCUMENT_BYTES)
            .map_err(io_environment("read release document"))?;
    let release_text = std::str::from_utf8(&release_bytes)
        .map_err(|_| LifecycleError::invalid("release document is not UTF-8"))?;
    let document: ProductLifecycleReleaseDocument =
        yaml_serde::from_str(release_text).map_err(|error| {
            LifecycleError::invalid(format!("release document is invalid: {error}"))
        })?;
    let issues = document.validation_issues();
    if !issues.is_empty() {
        return Err(LifecycleError::invalid(format!(
            "release document failed validation: {}",
            issues.join("; ")
        )));
    }
    let release = &document.product_lifecycle_release;
    let trusted_verification = match trusted_verification {
        TrustedVerificationMode::Required(path) => {
            Some(load_trusted_verification_input(path, &document)?)
        }
        #[cfg(test)]
        TrustedVerificationMode::LifecycleMechanicsFixture => None,
    };
    let version = Version::parse(&release.version)
        .map_err(|error| LifecycleError::invalid(format!("release version is invalid: {error}")))?;
    let compatibility = VersionReq::parse(&release.compatible_core_version).map_err(|error| {
        LifecycleError::invalid(format!("compatible_core_version is invalid: {error}"))
    })?;
    let running = Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("workspace package version is valid semver");
    if !compatibility.matches(&running) {
        return Err(LifecycleError::rejected(format!(
            "release requires core version '{}' but running core is {}",
            release.compatible_core_version, running
        )));
    }
    let canonical_release = canonical_json(&document)?;
    let release_sha256 = digest(&canonical_release);
    let generation_id = format!("generation-{}", &release_sha256[7..]);
    let bundle_root = release_file
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut assets = Vec::with_capacity(release.assets.len());
    for asset in &release.assets {
        let source = bundle_root.join(&asset.source_path.0);
        reject_symlink_ancestors(&source)?;
        let bytes = read_regular_file_no_follow_bounded(&source, MAX_ASSET_BYTES)
            .map_err(io_environment("read release asset"))?;
        let target = asset.host.unwrap_or(RuntimeKind::ForgeStandalone);
        let admission = run_host_adapter_distribution_admission(HostAdapterDistributionEvidence {
            target,
            channel: channel_for_distribution(release.channel),
            artifact_name: asset.asset_id.0.clone(),
            artifact_sha256: Some(asset.sha256.clone()),
            signature_ref: release.signature_ref.clone(),
            provenance_ref: Some(release.provenance_ref.clone()),
            source_ref: Some(release.source_ref.clone()),
            version: Some(release.version.clone()),
            compatible_core_version: Some(running.to_string()),
            rollback_ref: Some(release.rollback_ref.clone()),
            update_summary_ref: Some(format!("release:{}#typed-changes", release.release_id.0)),
            explicit_canary_opt_in,
        });
        if admission.status != HostAdapterDistributionAdmissionStatus::Allowed {
            return Err(LifecycleError::rejected(format!(
                "distribution admission blocked asset '{}': {}",
                asset.asset_id.0,
                admission.reasons.join(", ")
            )));
        }
        let verification =
            run_host_adapter_artifact_verification(HostAdapterArtifactVerificationInput {
                artifact_path: source.clone(),
                expected_sha256: asset.sha256.clone(),
                signature_ref: release.signature_ref.clone(),
                provenance_ref: Some(release.provenance_ref.clone()),
                source_ref: Some(release.source_ref.clone()),
                version: Some(release.version.clone()),
                compatible_core_version: Some(running.to_string()),
                rollback_ref: Some(release.rollback_ref.clone()),
                update_summary_ref: Some(format!("release:{}#typed-changes", release.release_id.0)),
            });
        if verification.status != HostAdapterArtifactVerificationStatus::Passed {
            return Err(LifecycleError::rejected(format!(
                "artifact verification failed for '{}': {}",
                asset.asset_id.0,
                verification.reasons.join(", ")
            )));
        }
        if let Some(trusted_verification) = &trusted_verification {
            let trusted_asset = trusted_verification
                .product_lifecycle_trusted_verification_input
                .assets
                .iter()
                .find(|input| input.asset_id == asset.asset_id)
                .ok_or_else(|| {
                    LifecycleError::invalid(format!(
                        "trusted verification input omits asset '{}' after validation",
                        asset.asset_id.0
                    ))
                })?;
            verify_trusted_asset(bundle_root, &source, trusted_asset)?;
        }
        let verified_bytes = read_regular_file_no_follow_bounded(&source, MAX_ASSET_BYTES)
            .map_err(io_environment("re-read verified release asset"))?;
        if verified_bytes != bytes || digest(&verified_bytes) != asset.sha256 {
            return Err(LifecycleError::conflict(format!(
                "asset '{}' changed during fresh trusted verification",
                asset.asset_id.0
            )));
        }
        assets.push(VerifiedAsset {
            inventory: InstalledAsset {
                asset_id: asset.asset_id.0.clone(),
                kind: asset.kind,
                install_path: asset.install_path.0.clone(),
                sha256: asset.sha256.clone(),
                executable: asset.executable,
                host: asset.host,
            },
            bytes,
        });
    }
    Ok(LoadedRelease {
        document,
        release_sha256,
        generation_id,
        version: version.to_string(),
        assets,
    })
}

fn load_trusted_verification_input(
    path: &Path,
    release: &ProductLifecycleReleaseDocument,
) -> Result<ProductLifecycleTrustedVerificationInputDocument, LifecycleError> {
    reject_symlink_ancestors(path)?;
    let bytes = read_regular_file_no_follow_bounded(path, MAX_RELEASE_DOCUMENT_BYTES)
        .map_err(io_environment("read trusted verification document"))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| LifecycleError::invalid("trusted verification document is not UTF-8"))?;
    let input = yaml_serde::from_str::<ProductLifecycleTrustedVerificationInputDocument>(text)
        .map_err(|error| {
            LifecycleError::invalid(format!("trusted verification document is invalid: {error}"))
        })?;
    input.validate_for_release(release).map_err(|error| {
        LifecycleError::invalid(format!(
            "trusted verification input does not bind this release: {error:?}"
        ))
    })?;
    Ok(input)
}

fn verify_trusted_asset(
    bundle_root: &Path,
    artifact_path: &Path,
    input: &forge_core_contracts::ProductLifecycleAssetTrustedVerificationInput,
) -> Result<(), LifecycleError> {
    let provenance = &input.detached_provenance;
    let sigstore = &input.sigstore;
    let evidence = |path: &forge_core_contracts::RepoPath| -> Result<PathBuf, LifecycleError> {
        let resolved = bundle_root.join(&path.0);
        reject_symlink_ancestors(&resolved)?;
        Ok(resolved)
    };
    let provenance_path = evidence(&provenance.provenance_path)?;
    let signature_path = evidence(&provenance.signature_path)?;
    let public_key_path = evidence(&provenance.public_key_path)?;
    let transparency_log_path = evidence(&provenance.transparency_log_path)?;
    let bundle_path = evidence(&sigstore.bundle_path)?;
    let trust_policy_path = evidence(&sigstore.trust_policy_path)?;
    let certificate_path = evidence(&sigstore.certificate_path)?;
    let issuer_certificate_paths = sigstore
        .issuer_certificate_paths
        .iter()
        .map(evidence)
        .collect::<Result<Vec<_>, _>>()?;
    let rekor_log_entry_path = evidence(&sigstore.rekor_log_entry_path)?;
    let rekor_public_key_path = evidence(&sigstore.rekor_public_key_path)?;

    let provenance_result =
        run_host_adapter_provenance_verification(HostAdapterProvenanceVerificationInput {
            artifact_path: artifact_path.to_path_buf(),
            provenance_path,
            signature_path,
            public_key_path,
            transparency_log_path,
            expected_sha256: input.expected_sha256.clone(),
            expected_builder_id: provenance.expected_builder_id.clone(),
            expected_source_uri: provenance.expected_source_uri.clone(),
            expected_source_ref: provenance.expected_source_ref.clone(),
        });
    if provenance_result.status != HostAdapterProvenanceVerificationStatus::Passed {
        return Err(LifecycleError::rejected(format!(
            "detached provenance verification failed for '{}': {}",
            input.asset_id.0,
            provenance_result.reasons.join(", ")
        )));
    }

    // The lifecycle Sigstore contract names one DSSE/in-toto bundle and its
    // matching Rekor entry. A messageSignature bundle is a different Sigstore
    // artifact with a different Rekor body shape, so requiring both verifiers
    // against this one evidence pair would make every valid input impossible.
    let dsse_result = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path,
            artifact_path: artifact_path.to_path_buf(),
            trust_policy_path,
            certificate_path,
            issuer_certificate_paths,
            rekor_log_entry_path,
            rekor_public_key_path,
            expected_rekor_log_id: sigstore.expected_rekor_log_id.clone(),
            expected_predicate_type: Some(sigstore.expected_predicate_type.clone()),
        },
    );
    if dsse_result.status != HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Passed {
        return Err(LifecycleError::rejected(format!(
            "Sigstore DSSE provenance verification failed for '{}': {}",
            input.asset_id.0,
            dsse_result.reasons.join(", ")
        )));
    }
    Ok(())
}

fn load_reporting_state(
    store: &ProductLifecycleStore,
    state: &StoreProductLifecycleState,
) -> Result<LifecycleReportingState, LifecycleError> {
    let mut generations = Vec::with_capacity(state.generations.len());
    for stored in &state.generations {
        let receipt_bytes = store
            .read_generation_receipt(stored)
            .map_err(store_environment("read lifecycle Store receipt"))?;
        let receipt: LifecycleGenerationReceipt =
            serde_json::from_slice(&receipt_bytes).map_err(|error| {
                LifecycleError::invalid(format!("lifecycle Store receipt is invalid: {error}"))
            })?;
        if canonical_json(&receipt)? != receipt_bytes
            || receipt.schema_version != LIFECYCLE_SCHEMA_VERSION
            || receipt.owner != LIFECYCLE_OWNER
            || receipt.generation_id != stored.generation_id
            || receipt.version != stored.version
            || receipt.release_sha256 != stored.release_sha256
            || receipt.verification_boundary != verification_boundary()
            || receipt.assets.len() != stored.assets.len()
        {
            return Err(LifecycleError::invalid(
                "lifecycle Store receipt does not bind its immutable generation",
            ));
        }
        let generation = receipt.into_reporting_generation(stored.receipt_sha256.clone());
        generation.validate()?;
        for asset in &generation.assets {
            let stored_asset = store_asset_for(stored, asset)?;
            if stored_asset.sha256 != asset.sha256 || stored_asset.executable != asset.executable {
                return Err(LifecycleError::invalid(
                    "lifecycle Store receipt asset differs from immutable inventory",
                ));
            }
        }
        generations.push(generation);
    }
    Ok(LifecycleReportingState {
        active_generation: state.active_generation.clone(),
        previous_generation: state.previous_generation.clone(),
        generations,
    })
}

fn store_asset_for<'a>(
    generation: &'a ProductLifecycleGeneration,
    asset: &InstalledAsset,
) -> Result<&'a StoreProductLifecycleAsset, LifecycleError> {
    generation
        .assets
        .iter()
        .find(|stored| stored.path == asset.install_path)
        .ok_or_else(|| LifecycleError::invalid("receipt asset is absent from Store inventory"))
}

fn host_status_from_store(
    status: ProductLifecycleAssetObservationStatus,
) -> HostConfigurationStatus {
    match status {
        ProductLifecycleAssetObservationStatus::Exact => HostConfigurationStatus::Exact,
        ProductLifecycleAssetObservationStatus::Missing => HostConfigurationStatus::Missing,
        ProductLifecycleAssetObservationStatus::DigestMismatch => {
            HostConfigurationStatus::DigestMismatch
        }
        ProductLifecycleAssetObservationStatus::MetadataMismatch
        | ProductLifecycleAssetObservationStatus::UnsafeFileType => {
            HostConfigurationStatus::UnsafeFileType
        }
    }
}

fn load_marker(root: &Path) -> Result<LifecycleOwnershipMarker, LifecycleError> {
    let path = root.join(MARKER_FILE);
    let bytes = read_regular_file_no_follow_bounded(&path, 64 * 1024).map_err(|error| {
        LifecycleError::environment(format!(
            "cannot read lifecycle ownership marker '{}': {error}",
            path.display()
        ))
    })?;
    let marker: LifecycleOwnershipMarker = serde_json::from_slice(&bytes).map_err(|error| {
        LifecycleError::invalid(format!("lifecycle ownership marker is invalid: {error}"))
    })?;
    if !marker.valid() {
        return Err(LifecycleError::rejected(
            "lifecycle ownership marker does not match this product and no mutation is allowed",
        ));
    }
    Ok(marker)
}

fn rollback(root: &Path) -> Result<ProductLifecycleReport, LifecycleError> {
    reject_symlink_ancestors(root)?;
    load_marker(root)?;
    let store = open_lifecycle_store(root)?;
    let read = store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?;
    let mut state = read
        .state
        .ok_or_else(|| LifecycleError::rejected("no installed lifecycle state exists"))?;
    let mut reporting = load_reporting_state(&store, &state)?;
    let previous = state
        .previous_generation
        .clone()
        .ok_or_else(|| LifecycleError::rejected("no prior working generation is available"))?;
    let generation = state
        .generations
        .iter()
        .find(|generation| generation.generation_id == previous)
        .ok_or_else(|| {
            LifecycleError::invalid("prior generation is absent from Store inventory")
        })?;
    store
        .verify_generation(generation)
        .map_err(store_conflict("verify lifecycle Store rollback generation"))?;
    let old_active = state.active_generation.replace(previous.clone());
    state.previous_generation.clone_from(&old_active);
    store
        .compare_and_swap_state(read.digest.as_deref(), &state)
        .map_err(store_conflict("publish lifecycle Store rollback"))?;
    reporting.active_generation = Some(previous);
    reporting.previous_generation = old_active;
    Ok(report_for_state(
        root,
        ProductLifecycleStatus::RolledBack,
        Some(reporting),
        Vec::new(),
        Vec::new(),
    ))
}

fn uninstall(root: &Path) -> Result<ProductLifecycleReport, LifecycleError> {
    reject_symlink_ancestors(root)?;
    load_marker(root)?;
    let store = open_lifecycle_store(root)?;
    let read = store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?;
    let mut preserved = Vec::new();
    if let Some(state) = read.state.as_ref() {
        // Empty state is the uninstall linearization point. If exact deletion is
        // later interrupted, retained files remain unknown and are preserved
        // rather than being silently re-adopted as an active generation.
        store
            .compare_and_swap_state(read.digest.as_deref(), &StoreProductLifecycleState::empty())
            .map_err(store_conflict("clear lifecycle Store state"))?;
        let report = store
            .uninstall_exact(state)
            .map_err(store_environment("uninstall lifecycle Store inventory"))?;
        preserved.extend(
            report
                .preserved_paths
                .into_iter()
                .map(|path| path.display().to_string()),
        );
    }
    preserved.sort();
    preserved.dedup();
    Ok(report_for_state(
        root,
        ProductLifecycleStatus::Uninstalled,
        None,
        Vec::new(),
        preserved,
    ))
}

fn report_for_state(
    root: &Path,
    status: ProductLifecycleStatus,
    state: Option<LifecycleReportingState>,
    diagnostics: Vec<String>,
    preserved_paths: Vec<String>,
) -> ProductLifecycleReport {
    let active = state.as_ref().and_then(|state| {
        state
            .active_generation
            .as_deref()
            .and_then(|id| state.generation(id))
    });
    let previous = state.as_ref().and_then(|state| {
        state
            .previous_generation
            .as_deref()
            .and_then(|id| state.generation(id))
    });
    ProductLifecycleReport {
        status,
        install_root: root.display().to_string(),
        marker_valid: !matches!(
            status,
            ProductLifecycleStatus::NotSetup | ProductLifecycleStatus::UnmanagedRoot
        ),
        active_generation: active.map(|generation| generation.generation_id.clone()),
        active_version: active.map(|generation| generation.version.clone()),
        previous_generation: previous.map(|generation| generation.generation_id.clone()),
        previous_version: previous.map(|generation| generation.version.clone()),
        selected_host: None,
        host_configurations: Vec::new(),
        release_notes: active.map_or_else(Vec::new, |generation| generation.changes.clone()),
        diagnostics,
        preserved_paths,
        verification_boundary: verification_boundary().to_owned(),
    }
}

fn base_report(
    root: &Path,
    status: ProductLifecycleStatus,
    marker_valid: bool,
) -> ProductLifecycleReport {
    ProductLifecycleReport {
        status,
        install_root: root.display().to_string(),
        marker_valid,
        active_generation: None,
        active_version: None,
        previous_generation: None,
        previous_version: None,
        selected_host: None,
        host_configurations: Vec::new(),
        release_notes: Vec::new(),
        diagnostics: Vec::new(),
        preserved_paths: Vec::new(),
        verification_boundary: verification_boundary().to_owned(),
    }
}

fn verification_boundary() -> &'static str {
    "Local closed-document validation, distribution admission, no-follow byte loading, exact SHA-256, immutable source reference, version compatibility, rollback metadata, typed update summary, and product-owned inventory are enforced before mutation. This does not by itself establish signer identity, provenance predicate semantics, transparency inclusion, publication, support, host selection, release authority, or custody of external private keys."
}

fn channel_for_distribution(channel: ProductLifecycleChannel) -> HostAdapterUpdateChannel {
    match channel {
        ProductLifecycleChannel::Stable => HostAdapterUpdateChannel::Stable,
        ProductLifecycleChannel::Canary => HostAdapterUpdateChannel::Canary,
        ProductLifecycleChannel::Dev => HostAdapterUpdateChannel::Dev,
    }
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, LifecycleError> {
    serde_json_canonicalizer::to_vec(value).map_err(|error| {
        LifecycleError::invalid(format!("cannot canonicalize lifecycle document: {error}"))
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), LifecycleError> {
    let mut bytes = canonical_json(value)?;
    bytes.push(b'\n');
    let text = String::from_utf8(bytes).expect("canonical JSON is UTF-8");
    atomic_write(path, &text).map_err(io_environment("write lifecycle metadata"))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", crate::hex_sha256(bytes))
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), LifecycleError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(io_environment("resolve current directory"))?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(LifecycleError::rejected(format!(
                    "symlink path component is forbidden for lifecycle I/O: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LifecycleError::environment(format!(
                    "cannot inspect lifecycle path '{}': {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn safe_generation_id(value: &str) -> bool {
    if !value.starts_with("generation-") || value.len() > 128 || value.contains('\\') {
        return false;
    }
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(name)) if name == value)
        && components.next().is_none()
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !value.contains('\\')
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn direct_entries_except(root: &Path, excluded: &[&str]) -> Result<Vec<String>, LifecycleError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(io_environment("read install root"))? {
        let entry = entry.map_err(io_environment("read install root entry"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !excluded.contains(&name.as_str()) {
            entries.push(name);
        }
    }
    entries.sort();
    Ok(entries)
}

fn host_configuration_status_name(status: HostConfigurationStatus) -> &'static str {
    match status {
        HostConfigurationStatus::Exact => "exact",
        HostConfigurationStatus::Missing => "missing",
        HostConfigurationStatus::DigestMismatch => "digest_mismatch",
        HostConfigurationStatus::UnsafeFileType => "unsafe_file_type",
    }
}

fn io_environment(action: &'static str) -> impl FnOnce(std::io::Error) -> LifecycleError {
    move |error| LifecycleError::environment(format!("{action}: {error}"))
}

fn store_environment(
    action: &'static str,
) -> impl FnOnce(ProductLifecycleStoreError) -> LifecycleError {
    move |error| LifecycleError::environment(format!("{action}: {error}"))
}

fn store_conflict(
    action: &'static str,
) -> impl FnOnce(ProductLifecycleStoreError) -> LifecycleError {
    move |error| LifecycleError::conflict(format!("{action}: {error}"))
}

#[cfg(test)]
const GENERATIONS_DIR: &str = "product-lifecycle/generations";
#[cfg(test)]
const RECEIPTS_DIR: &str = "product-lifecycle/generations";
#[cfg(test)]
const STATE_FILE: &str = "product-lifecycle/state.json";
#[cfg(test)]
type ProductLifecycleState = StoreProductLifecycleState;

#[cfg(test)]
fn load_state(root: &Path) -> Result<StoreProductLifecycleState, LifecycleError> {
    let store = open_lifecycle_store(root)?;
    store
        .read_state()
        .map_err(store_environment("read lifecycle Store state"))?
        .state
        .ok_or_else(|| LifecycleError::rejected("no installed lifecycle state exists"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        ProductLifecycleAsset, ProductLifecycleChangeKind, ProductLifecycleRelease, RepoPath,
        StableId, PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION,
    };

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "forge-product-lifecycle-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn release_document(
        version: &str,
        binary_digest: &str,
        host_config_digest: &str,
    ) -> ProductLifecycleReleaseDocument {
        ProductLifecycleReleaseDocument {
            schema_version: PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION.to_owned(),
            product_lifecycle_release: ProductLifecycleRelease {
                release_id: StableId(format!("forge-core-{version}")),
                version: version.to_owned(),
                compatible_core_version: env!("CARGO_PKG_VERSION").to_owned(),
                channel: ProductLifecycleChannel::Stable,
                source_ref:
                    "git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567"
                        .to_owned(),
                provenance_ref: "provenance.json".to_owned(),
                signature_ref: Some("release.sig".to_owned()),
                rollback_ref: "previous-working-generation".to_owned(),
                changes: vec![ProductLifecycleChange {
                    change_id: StableId(format!("change-{version}")),
                    kind: ProductLifecycleChangeKind::Changed,
                    summary: format!("Release {version}"),
                }],
                assets: vec![
                    ProductLifecycleAsset {
                        asset_id: StableId("forge-core-binary".to_owned()),
                        kind: ProductLifecycleAssetKind::CoreBinary,
                        source_path: RepoPath("bundle/forge-core".to_owned()),
                        install_path: RepoPath("bin/forge-core".to_owned()),
                        sha256: binary_digest.to_owned(),
                        executable: true,
                        host: None,
                    },
                    ProductLifecycleAsset {
                        asset_id: StableId("codex-host-config".to_owned()),
                        kind: ProductLifecycleAssetKind::HostConfiguration,
                        source_path: RepoPath("bundle/codex-config.json".to_owned()),
                        install_path: RepoPath("hosts/codex/config.json".to_owned()),
                        sha256: host_config_digest.to_owned(),
                        executable: false,
                        host: Some(RuntimeKind::Codex),
                    },
                ],
            },
        }
    }

    fn write_release(root: &Path, version: &str, bytes: &[u8]) -> PathBuf {
        const HOST_CONFIG: &[u8] = b"{\"command\":\"forge-core\",\"mode\":\"governed\"}\n";
        fs::create_dir_all(root.join("bundle")).unwrap();
        fs::write(root.join("bundle/forge-core"), bytes).unwrap();
        fs::write(root.join("bundle/codex-config.json"), HOST_CONFIG).unwrap();
        let document = release_document(version, &digest(bytes), &digest(HOST_CONFIG));
        let path = root.join("release.yaml");
        fs::write(&path, yaml_serde::to_string(&document).unwrap()).unwrap();
        path
    }

    #[test]
    fn generation_ids_are_single_safe_path_components() {
        assert!(safe_generation_id("generation-0123456789abcdef"));
        assert!(!safe_generation_id("generation-../outside"));
        assert!(!safe_generation_id("generation-a/b"));
        assert!(!safe_generation_id("generation-a\\b"));
    }

    #[test]
    fn setup_is_idempotent_and_refuses_nonempty_unmanaged_roots() {
        let root = temp_root("setup");
        assert_eq!(setup(&root).unwrap().status, ProductLifecycleStatus::Setup);
        assert_eq!(
            setup(&root).unwrap().status,
            ProductLifecycleStatus::AlreadySetup
        );
        let unmanaged = temp_root("unmanaged");
        fs::create_dir_all(unmanaged.join("consumer-project/.forge")).unwrap();
        fs::create_dir_all(unmanaged.join("backups")).unwrap();
        fs::write(unmanaged.join("consumer-project/project.toml"), b"keep").unwrap();
        fs::write(
            unmanaged.join("consumer-project/.forge/sidecar.json"),
            b"keep",
        )
        .unwrap();
        fs::write(unmanaged.join("operator-anchor.json"), b"keep").unwrap();
        fs::write(unmanaged.join("backups/restore.tar"), b"keep").unwrap();
        assert!(setup(&unmanaged).is_err());
        assert!(unmanaged.join("consumer-project/project.toml").exists());
        assert!(unmanaged
            .join("consumer-project/.forge/sidecar.json")
            .exists());
        assert!(unmanaged.join("operator-anchor.json").exists());
        assert!(unmanaged.join("backups/restore.tar").exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(unmanaged);
    }

    #[test]
    fn interrupted_setup_publishes_ownership_before_owned_layout_and_retries_exactly() {
        let root = temp_root("interrupted-setup");
        let interrupted =
            setup_with_checkpoint(&root, SetupCheckpoint::InterruptAfterMarkerValidation);
        assert!(interrupted.is_err());
        assert!(root.join(MARKER_FILE).is_file());
        assert!(!root.join(GENERATIONS_DIR).exists());
        assert!(!root.join(RECEIPTS_DIR).exists());

        let retry = setup(&root).unwrap();
        assert_eq!(retry.status, ProductLifecycleStatus::AlreadySetup);
        assert!(root.join(GENERATIONS_DIR).is_dir());
        assert!(root.join(RECEIPTS_DIR).is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_generation_update_recovers_on_exact_retry() {
        let root = temp_root("interrupted-publication");
        let initial_bundle = temp_root("interrupted-publication-initial");
        let update_bundle = temp_root("interrupted-publication-update");
        setup(&root).unwrap();
        let initial = write_release(&initial_bundle, env!("CARGO_PKG_VERSION"), b"initial");
        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let updated_version =
            Version::new(current.major, current.minor, current.patch + 1).to_string();
        let update = write_release(&update_bundle, &updated_version, b"updated");
        let installed = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &initial,
            &initial,
            false,
            InstallMode::Install,
        )
        .unwrap();

        let interrupted = install_or_update_lifecycle_mechanics_fixture_with_checkpoint(
            &root,
            &update,
            &update,
            false,
            InstallMode::Update,
            PublicationCheckpoint::InterruptAfterGenerationPublication,
        );
        assert!(interrupted.is_err());
        assert_eq!(
            load_state(&root).unwrap().active_generation,
            installed.active_generation
        );
        assert_eq!(fs::read_dir(root.join(GENERATIONS_DIR)).unwrap().count(), 2);

        let retry = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &update,
            &update,
            false,
            InstallMode::Update,
        )
        .unwrap();
        assert_eq!(retry.status, ProductLifecycleStatus::Updated);
        assert_eq!(
            retry.active_version.as_deref(),
            Some(updated_version.as_str())
        );
        assert_eq!(fs::read_dir(root.join(GENERATIONS_DIR)).unwrap().count(), 2);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(initial_bundle);
        let _ = fs::remove_dir_all(update_bundle);
    }

    #[test]
    fn install_update_rollback_and_exact_retry_preserve_generations() {
        let root = temp_root("transitions");
        let bundle_one = temp_root("bundle-one");
        let bundle_two = temp_root("bundle-two");
        setup(&root).unwrap();
        let release_one = write_release(&bundle_one, env!("CARGO_PKG_VERSION"), b"one");
        let first = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &release_one,
            &release_one,
            false,
            InstallMode::Install,
        )
        .unwrap();
        assert_eq!(first.status, ProductLifecycleStatus::Installed);
        let retry = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &release_one,
            &release_one,
            false,
            InstallMode::Install,
        )
        .unwrap();
        assert_eq!(retry.status, ProductLifecycleStatus::AlreadyInstalled);

        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let next = Version::new(current.major, current.minor, current.patch + 1).to_string();
        let release_two = write_release(&bundle_two, &next, b"two");
        let updated = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &release_two,
            &release_two,
            false,
            InstallMode::Update,
        )
        .unwrap();
        assert_eq!(updated.status, ProductLifecycleStatus::Updated);
        let rolled_back = rollback(&root).unwrap();
        assert_eq!(rolled_back.status, ProductLifecycleStatus::RolledBack);
        assert_eq!(
            rolled_back.active_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(bundle_one);
        let _ = fs::remove_dir_all(bundle_two);
    }

    #[test]
    fn update_refuses_semantic_downgrades() {
        let root = temp_root("semantic-downgrade");
        let current_bundle = temp_root("semantic-downgrade-current");
        let newer_bundle = temp_root("semantic-downgrade-newer");
        setup(&root).unwrap();
        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let newer = Version::new(current.major, current.minor, current.patch + 1).to_string();
        let current_release = write_release(&current_bundle, &current.to_string(), b"current");
        let newer_release = write_release(&newer_bundle, &newer, b"newer");
        install_or_update_lifecycle_mechanics_fixture(
            &root,
            &current_release,
            &current_release,
            false,
            InstallMode::Install,
        )
        .unwrap();
        let updated = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &newer_release,
            &newer_release,
            false,
            InstallMode::Update,
        )
        .unwrap();

        let downgrade = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &current_release,
            &current_release,
            false,
            InstallMode::Update,
        );
        assert_eq!(
            downgrade.unwrap_err().exit_reason,
            ExitReason::RejectedByGate
        );
        assert_eq!(
            load_state(&root).unwrap().active_generation,
            updated.active_generation
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(current_bundle);
        let _ = fs::remove_dir_all(newer_bundle);
    }

    #[test]
    fn status_reports_wrapper_and_configuration_digest_mismatches_without_selecting_host() {
        let root = temp_root("host-status");
        let bundle = temp_root("host-status-bundle");
        setup(&root).unwrap();
        let release = write_release(&bundle, env!("CARGO_PKG_VERSION"), b"owned");
        let installed = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &release,
            &release,
            false,
            InstallMode::Install,
        )
        .unwrap();
        let exact = inspect(&root, false).unwrap();
        assert_eq!(exact.selected_host, None);
        assert_eq!(exact.host_configurations.len(), 1);
        assert_eq!(
            exact.host_configurations[0].status,
            HostConfigurationStatus::Exact
        );

        let generation = installed.active_generation.unwrap();
        let config = root
            .join(GENERATIONS_DIR)
            .join(&generation)
            .join("assets/hosts/codex/config.json");
        fs::write(config, b"operator-modified").unwrap();
        let stale_configuration = inspect(&root, false).unwrap();
        assert_eq!(stale_configuration.status, ProductLifecycleStatus::Degraded);
        assert_eq!(
            stale_configuration.host_configurations[0].status,
            HostConfigurationStatus::DigestMismatch
        );

        let wrapper = root
            .join(GENERATIONS_DIR)
            .join(&generation)
            .join("assets/bin/forge-core");
        fs::write(wrapper, b"operator-modified").unwrap();
        let stale_wrapper = inspect(&root, false).unwrap();
        assert!(stale_wrapper
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("bin/forge-core")));
        assert_eq!(stale_wrapper.selected_host, None);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(bundle);
    }

    #[test]
    fn uninstall_preserves_modified_and_unknown_files() {
        let root = temp_root("uninstall");
        let bundle = temp_root("uninstall-bundle");
        setup(&root).unwrap();
        let release = write_release(&bundle, env!("CARGO_PKG_VERSION"), b"owned");
        let installed = install_or_update_lifecycle_mechanics_fixture(
            &root,
            &release,
            &release,
            false,
            InstallMode::Install,
        )
        .unwrap();
        let generation = installed.active_generation.unwrap();
        let binary = root
            .join(GENERATIONS_DIR)
            .join(generation)
            .join("assets/bin/forge-core");
        fs::write(&binary, b"operator-modified").unwrap();
        fs::create_dir_all(root.join("consumer-project/.forge")).unwrap();
        fs::create_dir_all(root.join("backups")).unwrap();
        fs::write(root.join("consumer-project/project.toml"), b"keep").unwrap();
        fs::write(root.join("consumer-project/.forge/sidecar.json"), b"keep").unwrap();
        fs::write(root.join("operator-anchor.json"), b"keep").unwrap();
        fs::write(root.join("backups/restore.tar"), b"keep").unwrap();
        let report = uninstall(&root).unwrap();
        assert_eq!(report.status, ProductLifecycleStatus::Uninstalled);
        assert!(binary.exists());
        assert!(root.join("consumer-project/project.toml").exists());
        assert!(root.join("consumer-project/.forge/sidecar.json").exists());
        assert!(root.join("operator-anchor.json").exists());
        assert!(root.join("backups/restore.tar").exists());
        let state = load_state(&root).unwrap();
        assert!(state.active_generation.is_none());
        assert!(state.previous_generation.is_none());
        assert!(state.generations.is_empty());
        assert!(root.join(STATE_FILE).is_file());
        assert!(root.join(MARKER_FILE).is_file());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(bundle);
    }

    #[test]
    fn state_and_release_deserialization_cannot_select_a_host_or_import_keys() {
        let raw = r#"{
            "schema_version":"0.1",
            "owner":"forge-core-product-lifecycle",
            "active_generation":null,
            "previous_generation":null,
            "generations":[],
            "selected_host":"claude",
            "private_key":"forbidden"
        }"#;
        assert!(serde_json::from_str::<ProductLifecycleState>(raw).is_err());
    }
}
