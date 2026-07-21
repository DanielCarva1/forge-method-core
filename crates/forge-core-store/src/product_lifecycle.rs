//! Store-owned durable product lifecycle filesystem backend.
//!
//! This module is deliberately a narrow capability: a caller first acquires the
//! exact effect-store lock and then transfers it into [`ProductLifecycleStore`].
//! All later I/O is descriptor-relative to the retained product root. Candidate
//! release data remains data: this backend neither selects a host nor imports,
//! persists, or interprets signing, trust, broker, or private-key material.

use crate::{
    crash_replace::CrashReplaceError,
    retained_crash_replace::{
        reconcile_file_crash_safe_at_owned_retained_target, RetainedCrashReplaceTarget,
    },
    retained_dir::{RetainedDirectory, RetainedLeafPolicy},
    sha256_content_hash, EffectStoreLock,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

const ROOT: &str = "product-lifecycle";
const STATE: &str = "state.json";
const OWNER_MARKER: &str = "owner.json";
const OWNER_MARKER_BYTES: &[u8] = br#"{"owner":"forge-product-lifecycle-store-v1"}"#;
const RECEIPT_LEAF: &str = "receipt.json";
const MANIFEST_LEAF: &str = "generation.json";
const STAGING: &str = "staging";
const GENERATIONS: &str = "generations";
const MAX_STATE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;
const SCHEMA: &str = "forge-product-lifecycle-store-v1";

/// Compact Store-owned facade for lifecycle CLI adapters.
#[derive(Debug)]
pub struct ProductLifecycleStore {
    lock: EffectStoreLock,
    root: RetainedDirectory,
}

/// Immutable asset data accepted only after the caller has completed any policy
/// admission. This type grants no host, signing, release, or install authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLifecycleAssetInput {
    pub path: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// Candidate immutable generation data. `generation_id` is constrained to one
/// safe path component before it is ever used as a retained-directory child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLifecycleGenerationInput {
    pub generation_id: String,
    pub version: String,
    pub release_sha256: String,
    pub receipt: Vec<u8>,
    pub assets: Vec<ProductLifecycleAssetInput>,
}

/// Durable state selected through the exact crash-replacement state leaf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleState {
    pub schema_version: String,
    pub active_generation: Option<String>,
    pub previous_generation: Option<String>,
    pub generations: Vec<ProductLifecycleGeneration>,
}

impl ProductLifecycleState {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA.to_owned(),
            active_generation: None,
            previous_generation: None,
            generations: Vec::new(),
        }
    }
}

/// Product-owned inventory for one immutable generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleGeneration {
    pub generation_id: String,
    pub version: String,
    pub release_sha256: String,
    pub receipt_sha256: String,
    pub assets: Vec<ProductLifecycleAsset>,
}

/// An exact product-owned file. No selected-host or key fields exist here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleAsset {
    pub path: String,
    pub sha256: String,
    pub executable: bool,
}

/// Exact durable state observation used as the expected side of a state CAS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLifecycleStateRead {
    pub state: Option<ProductLifecycleState>,
    pub digest: Option<String>,
}

/// Descriptor-relative observation of one immutable generation asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductLifecycleAssetObservationStatus {
    Exact,
    Missing,
    DigestMismatch,
    MetadataMismatch,
    UnsafeFileType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLifecycleAssetObservation {
    pub observed_sha256: Option<String>,
    pub status: ProductLifecycleAssetObservationStatus,
}

/// Uninstall reports only product paths that were not removed because they were
/// modified, unsafe, or unknown. It never deletes them opportunistically.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductLifecycleUninstallReport {
    pub preserved_paths: Vec<PathBuf>,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProductLifecycleStoreError {
    Invalid {
        reason: String,
    },
    Conflict {
        expected: Option<String>,
        actual: Option<String>,
    },
    Integrity {
        path: PathBuf,
        reason: String,
    },
    CrashReplace(CrashReplaceError),
    Io {
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for ProductLifecycleStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { reason } => write!(f, "invalid product lifecycle input: {reason}"),
            Self::Conflict { expected, actual } => write!(
                f,
                "product lifecycle state compare-and-swap mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::Integrity { path, reason } => write!(
                f,
                "product lifecycle integrity failure at {}: {reason}", path.display()
            ),
            Self::CrashReplace(error) => error.fmt(f),
            Self::Io { path, source } => write!(f, "product lifecycle I/O {} failed: {source}", path.display()),
        }
    }
}

impl std::error::Error for ProductLifecycleStoreError {}

impl From<CrashReplaceError> for ProductLifecycleStoreError {
    fn from(error: CrashReplaceError) -> Self {
        Self::CrashReplace(error)
    }
}

impl ProductLifecycleStore {
    /// Seal an exact effect lock into the fixed product-lifecycle namespace.
    ///
    /// The namespace is created descriptor-relatively and its directory handle
    /// is retained. A symlink, reparse point, file, or substituted directory is
    /// rejected by `RetainedDirectory` before this constructor succeeds.
    pub fn from_effect_lock(lock: EffectStoreLock) -> Result<Self, ProductLifecycleStoreError> {
        lock.boundary
            .require_effect_authority()
            .map_err(|error| invalid(error.to_string()))?;
        lock.validate_retained_lock_file()
            .map_err(|error| invalid(error.to_string()))?;
        lock.state_root
            .create_dir_all(Path::new(ROOT))
            .and_then(|()| lock.state_root.sync_root())
            .map_err(|error| io_error(&lock, Path::new(ROOT), error))?;
        let root = lock
            .state_root
            .open_directory(Path::new(ROOT))
            .map_err(|error| io_error(&lock, Path::new(ROOT), error))?;
        let store = Self { lock, root };
        store.validate_current()?;
        Ok(store)
    }

    /// Create and durably sync the fixed lifecycle layout without accepting any
    /// candidate material or selecting a host.
    pub fn setup(&self) -> Result<(), ProductLifecycleStoreError> {
        self.validate_current()?;
        self.write_staged(Path::new(OWNER_MARKER), OWNER_MARKER_BYTES)?;
        for name in [STAGING, GENERATIONS] {
            self.root
                .create_dir_all(Path::new(name))
                .and_then(|()| self.root.sync_directory(Path::new(name)))
                .map_err(|error| self.io(Path::new(name), error))?;
        }
        self.root
            .sync_root()
            .map_err(|error| self.io(Path::new(""), error))?;
        self.validate_current()
    }

    fn verify_owner_marker(&self) -> Result<(), ProductLifecycleStoreError> {
        if self.read_exact_required(Path::new(OWNER_MARKER), MAX_RECEIPT_BYTES)?
            != OWNER_MARKER_BYTES
        {
            return Err(self.integrity(Path::new(OWNER_MARKER), "ownership marker differs"));
        }
        Ok(())
    }

    /// Reconcile the state replacement protocol and return the exact durable
    /// state bytes/digest observed by that protocol.
    pub fn read_state(&self) -> Result<ProductLifecycleStateRead, ProductLifecycleStoreError> {
        self.setup()?;
        let session = self.state_session()?;
        let digest = session.digest().map(str::to_owned);
        let state = match session.read_exact()? {
            Some(read) => Some(parse_state(
                read.raw_bytes(),
                &self.display_path(Path::new(STATE)),
            )?),
            None => None,
        };
        if let Some(state) = state.as_ref() {
            self.validate_state_shape(state)?;
        }
        self.validate_current()?;
        Ok(ProductLifecycleStateRead { state, digest })
    }

    /// Publish all staged material durably, then publish the generation receipt
    /// as the no-replace generation commit leaf. A partially staged directory is
    /// never an installed generation because it lacks that exact receipt.
    pub fn publish_generation(
        &self,
        input: &ProductLifecycleGenerationInput,
    ) -> Result<ProductLifecycleGeneration, ProductLifecycleStoreError> {
        self.setup()?;
        validate_input(input)?;
        let generation = generation_from_input(input)?;
        let final_prefix = Path::new(GENERATIONS).join(&input.generation_id);
        let manifest = canonical_json(&generation)?;
        let final_receipt = final_prefix.join(RECEIPT_LEAF);
        let final_manifest = final_prefix.join(MANIFEST_LEAF);

        if self
            .read_exact_optional(&final_receipt, MAX_RECEIPT_BYTES)?
            .is_some()
        {
            self.verify_generation(&generation)?;
            return Ok(generation);
        }

        let stage_prefix = Path::new(STAGING).join(&input.generation_id);
        for asset in &input.assets {
            let stage = stage_prefix.join("assets").join(&asset.path);
            self.write_staged(&stage, &asset.bytes)?;
            self.set_staged_asset_mode(&stage, asset.executable)?;
            self.publish_staged_noreplace(&stage, &final_prefix.join("assets").join(&asset.path))?;
        }
        self.write_staged(&stage_prefix.join(MANIFEST_LEAF), &manifest)?;
        self.publish_staged_noreplace(&stage_prefix.join(MANIFEST_LEAF), &final_manifest)?;
        self.write_staged(&stage_prefix.join(RECEIPT_LEAF), &input.receipt)?;
        // Receipt is deliberately last: its successful no-replace publication
        // is the immutable generation's visibility/linearization point.
        self.publish_staged_noreplace(&stage_prefix.join(RECEIPT_LEAF), &final_receipt)?;
        self.root
            .sync_directory(&final_prefix)
            .map_err(|error| self.io(&final_prefix, error))?;
        self.root
            .sync_root()
            .map_err(|error| self.io(Path::new(""), error))?;
        self.verify_generation(&generation)?;
        Ok(generation)
    }

    /// Commit `state` only if the just-reconciled exact state object has the
    /// requested digest. This is both a digest CAS and an identity CAS: the
    /// crash-replace session carries the retained exact prior handle/absence
    /// claim through final replacement rather than reopening the pathname.
    pub fn compare_and_swap_state(
        &self,
        expected_digest: Option<&str>,
        state: &ProductLifecycleState,
    ) -> Result<String, ProductLifecycleStoreError> {
        self.setup()?;
        self.validate_state_for_publish(state)?;
        let bytes = canonical_json(state)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_STATE_BYTES {
            return Err(invalid("state exceeds maximum byte length"));
        }
        let session = self.state_session()?;
        let actual = session.digest().map(str::to_owned);
        if actual.as_deref() != expected_digest {
            return Err(ProductLifecycleStoreError::Conflict {
                expected: expected_digest.map(str::to_owned),
                actual,
            });
        }
        let installed = session.replace(&bytes)?;
        if installed.digest() != digest(&bytes) {
            return Err(ProductLifecycleStoreError::Integrity {
                path: self.display_path(Path::new(STATE)),
                reason: "state replacement returned a different digest".to_owned(),
            });
        }
        self.validate_current()?;
        Ok(installed.digest().to_owned())
    }

    /// Verify the exact receipt, manifest, and every product-owned asset through
    /// retained descriptor handles. Candidate files, unknown files, and state
    /// fields cannot establish a selected host or any trust/key authority.
    pub fn verify_generation(
        &self,
        generation: &ProductLifecycleGeneration,
    ) -> Result<(), ProductLifecycleStoreError> {
        self.validate_current()?;
        self.verify_owner_marker()?;
        validate_generation(generation)?;
        let prefix = Path::new(GENERATIONS).join(&generation.generation_id);
        let receipt = self.read_exact_required(&prefix.join(RECEIPT_LEAF), MAX_RECEIPT_BYTES)?;
        if digest(&receipt) != generation.receipt_sha256 {
            return Err(self.integrity(&prefix.join(RECEIPT_LEAF), "receipt digest differs"));
        }
        let manifest = self.read_exact_required(&prefix.join(MANIFEST_LEAF), MAX_RECEIPT_BYTES)?;
        if canonical_json(generation)? != manifest {
            return Err(self.integrity(
                &prefix.join(MANIFEST_LEAF),
                "manifest is not exact canonical inventory",
            ));
        }
        for asset in &generation.assets {
            let path = prefix.join("assets").join(&asset.path);
            let bytes = self.read_exact_required(&path, MAX_ASSET_BYTES)?;
            if digest(&bytes) != asset.sha256 {
                return Err(self.integrity(&path, "asset digest differs"));
            }
            if !self.asset_mode_matches(&path, asset.executable)? {
                return Err(self.integrity(&path, "asset executable metadata differs"));
            }
        }
        Ok(())
    }

    /// Read the exact immutable receipt used for non-authoritative rich reporting.
    /// The receipt digest is checked against the Store-owned generation inventory;
    /// callers cannot use receipt fields to select an active generation.
    pub fn read_generation_receipt(
        &self,
        generation: &ProductLifecycleGeneration,
    ) -> Result<Vec<u8>, ProductLifecycleStoreError> {
        self.validate_current()?;
        self.verify_owner_marker()?;
        validate_generation(generation)?;
        let path = Path::new(GENERATIONS)
            .join(&generation.generation_id)
            .join(RECEIPT_LEAF);
        let bytes = self.read_exact_required(&path, MAX_RECEIPT_BYTES)?;
        if digest(&bytes) != generation.receipt_sha256 {
            return Err(self.integrity(&path, "receipt digest differs"));
        }
        Ok(bytes)
    }

    /// Observe one Store-inventoried asset without turning an integrity mismatch
    /// into mutation authority. The generation and asset must match the exact
    /// durable inventory supplied by [`ProductLifecycleStateRead`].
    pub fn observe_generation_asset(
        &self,
        generation: &ProductLifecycleGeneration,
        asset: &ProductLifecycleAsset,
    ) -> Result<ProductLifecycleAssetObservation, ProductLifecycleStoreError> {
        self.validate_current()?;
        self.verify_owner_marker()?;
        validate_generation(generation)?;
        if !generation.assets.iter().any(|candidate| candidate == asset) {
            return Err(invalid("asset is absent from the generation inventory"));
        }
        let path = Path::new(GENERATIONS)
            .join(&generation.generation_id)
            .join("assets")
            .join(&asset.path);
        let bytes = match self.root.read_authority_bounded(&path, MAX_ASSET_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ProductLifecycleAssetObservation {
                    observed_sha256: None,
                    status: ProductLifecycleAssetObservationStatus::Missing,
                });
            }
            Err(_) => {
                return Ok(ProductLifecycleAssetObservation {
                    observed_sha256: None,
                    status: ProductLifecycleAssetObservationStatus::UnsafeFileType,
                });
            }
        };
        let observed_sha256 = digest(&bytes);
        let status = if observed_sha256 != asset.sha256 {
            ProductLifecycleAssetObservationStatus::DigestMismatch
        } else if !self.asset_mode_matches(&path, asset.executable)? {
            ProductLifecycleAssetObservationStatus::MetadataMismatch
        } else {
            ProductLifecycleAssetObservationStatus::Exact
        };
        Ok(ProductLifecycleAssetObservation {
            observed_sha256: Some(observed_sha256),
            status,
        })
    }

    /// Remove only exact, unmodified product-owned leaves. Unsafe, modified,
    /// missing, and unknown files are retained and reported; directories are not
    /// recursively removed, so no consumer, backup, anchor, or broker content is
    /// ever selected for deletion.
    pub fn uninstall_exact(
        &self,
        state: &ProductLifecycleState,
    ) -> Result<ProductLifecycleUninstallReport, ProductLifecycleStoreError> {
        self.validate_current()?;
        self.verify_owner_marker()?;
        self.validate_state_shape(state)?;
        let mut report = ProductLifecycleUninstallReport::default();
        for generation in &state.generations {
            let prefix = Path::new(GENERATIONS).join(&generation.generation_id);
            for asset in &generation.assets {
                self.remove_if_exact(
                    &prefix.join("assets").join(&asset.path),
                    &asset.sha256,
                    &mut report,
                )?;
            }
            self.remove_if_exact(
                &prefix.join(MANIFEST_LEAF),
                &digest(&canonical_json(generation)?),
                &mut report,
            )?;
            self.remove_if_exact(
                &prefix.join(RECEIPT_LEAF),
                &generation.receipt_sha256,
                &mut report,
            )?;
        }
        report.preserved_paths.sort();
        report.preserved_paths.dedup();
        Ok(report)
    }

    fn state_session(
        &self,
    ) -> Result<crate::RetainedCrashReplaceSession<'_>, ProductLifecycleStoreError> {
        self.validate_current()?;
        let target = RetainedCrashReplaceTarget::new(
            &self.lock,
            self.root
                .try_clone()
                .map_err(|error| self.io(Path::new(""), error))?,
            PathBuf::from(ROOT).join(STATE),
        )
        .map_err(|error| self.io(Path::new(STATE), error))?;
        Ok(reconcile_file_crash_safe_at_owned_retained_target(
            target,
            MAX_STATE_BYTES,
        )?)
    }

    fn validate_current(&self) -> Result<(), ProductLifecycleStoreError> {
        self.lock
            .boundary
            .require_effect_authority()
            .map_err(|error| invalid(error.to_string()))?;
        self.lock
            .validate_retained_lock_file()
            .map_err(|error| invalid(error.to_string()))?;
        let current = self
            .lock
            .state_root
            .open_directory(Path::new(ROOT))
            .map_err(|error| self.io(Path::new(ROOT), error))?;
        if current
            .identity()
            .map_err(|error| self.io(Path::new(ROOT), error))?
            != self
                .root
                .identity()
                .map_err(|error| self.io(Path::new(ROOT), error))?
        {
            return Err(self.integrity(Path::new(ROOT), "retained lifecycle root identity changed"));
        }
        Ok(())
    }

    fn validate_state_shape(
        &self,
        state: &ProductLifecycleState,
    ) -> Result<(), ProductLifecycleStoreError> {
        if state.schema_version != SCHEMA {
            return Err(invalid("unsupported state schema"));
        }
        let mut ids = BTreeSet::new();
        for generation in &state.generations {
            validate_generation(generation)?;
            if !ids.insert(generation.generation_id.as_str()) {
                return Err(invalid("state duplicates a generation id"));
            }
        }
        for selected in [&state.active_generation, &state.previous_generation] {
            if selected.as_deref().is_some_and(|id| !ids.contains(id)) {
                return Err(invalid("state selects a generation not in its inventory"));
            }
        }
        Ok(())
    }

    fn validate_state_for_publish(
        &self,
        state: &ProductLifecycleState,
    ) -> Result<(), ProductLifecycleStoreError> {
        self.validate_state_shape(state)?;
        let mut verified = BTreeSet::new();
        for selected in [&state.active_generation, &state.previous_generation]
            .into_iter()
            .flatten()
        {
            if verified.insert(selected.as_str()) {
                let generation = state
                    .generations
                    .iter()
                    .find(|generation| generation.generation_id == *selected)
                    .ok_or_else(|| invalid("selected generation is absent from state"))?;
                self.verify_generation(generation)?;
            }
        }
        Ok(())
    }

    fn write_staged(&self, path: &Path, bytes: &[u8]) -> Result<(), ProductLifecycleStoreError> {
        self.sync_parent_chain(path)?;
        let authority = self
            .root
            .retain_authority()
            .map_err(|error| self.io(path, error))?;
        match authority.write_new_file_synced(path, bytes) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if self.read_exact_required(path, MAX_ASSET_BYTES)? == bytes {
                    Ok(())
                } else {
                    Err(self.integrity(path, "staged leaf collision has different bytes"))
                }
            }
            Err(error) => Err(self.io(path, error)),
        }
    }

    #[cfg(unix)]
    fn set_staged_asset_mode(
        &self,
        path: &Path,
        executable: bool,
    ) -> Result<(), ProductLifecycleStoreError> {
        use std::os::unix::fs::PermissionsExt;

        let file = self
            .root
            .open_leaf_read(path, RetainedLeafPolicy::Authority)
            .map_err(|error| self.io(path, error))?;
        let mode = if executable { 0o755 } else { 0o644 };
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .and_then(|()| file.sync_all())
            .map_err(|error| self.io(path, error))
    }

    #[cfg(not(unix))]
    fn set_staged_asset_mode(
        &self,
        _path: &Path,
        _executable: bool,
    ) -> Result<(), ProductLifecycleStoreError> {
        Ok(())
    }

    #[cfg(unix)]
    fn asset_mode_matches(
        &self,
        path: &Path,
        executable: bool,
    ) -> Result<bool, ProductLifecycleStoreError> {
        use std::os::unix::fs::PermissionsExt;

        let file = self
            .root
            .open_leaf_read(path, RetainedLeafPolicy::Authority)
            .map_err(|error| self.io(path, error))?;
        let observed = file
            .metadata()
            .map_err(|error| self.io(path, error))?
            .permissions()
            .mode()
            & 0o111
            != 0;
        Ok(observed == executable)
    }

    #[cfg(not(unix))]
    fn asset_mode_matches(
        &self,
        _path: &Path,
        _executable: bool,
    ) -> Result<bool, ProductLifecycleStoreError> {
        Ok(true)
    }

    fn publish_staged_noreplace(
        &self,
        stage: &Path,
        final_path: &Path,
    ) -> Result<(), ProductLifecycleStoreError> {
        self.sync_parent_chain(final_path)?;
        let authority = self
            .root
            .retain_authority()
            .map_err(|error| self.io(final_path, error))?;
        match authority.rename_file_noreplace_with_validation(
            stage,
            final_path,
            |directory, source, destination| {
                if source != stage || destination != final_path {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "generation publication target changed",
                    ));
                }
                let file = directory.open_leaf_read(source, RetainedLeafPolicy::Authority)?;
                let identity = RetainedDirectory::identity_of(&file)?;
                directory.verify_retained_authority_binding(source, &file, &identity)
            },
        ) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let staged = self.read_exact_required(stage, MAX_ASSET_BYTES)?;
                let final_bytes = self.read_exact_required(final_path, MAX_ASSET_BYTES)?;
                if staged == final_bytes {
                    Ok(())
                } else {
                    Err(self.integrity(
                        final_path,
                        "no-replace generation collision has different bytes",
                    ))
                }
            }
            Err(error) => Err(self.io(final_path, error)),
        }
    }

    fn sync_parent_chain(&self, path: &Path) -> Result<(), ProductLifecycleStoreError> {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        if parent.as_os_str().is_empty() {
            return self
                .root
                .sync_root()
                .map_err(|error| self.io(Path::new(""), error));
        }
        self.root
            .create_dir_all(parent)
            .map_err(|error| self.io(parent, error))?;
        let mut current = PathBuf::new();
        let mut directories = Vec::new();
        for component in parent.components() {
            let Component::Normal(segment) = component else {
                return Err(invalid("lifecycle parent path is not normalized"));
            };
            current.push(segment);
            directories.push(current.clone());
        }
        for directory in directories.into_iter().rev() {
            self.root
                .sync_directory(&directory)
                .map_err(|error| self.io(&directory, error))?;
        }
        self.root
            .sync_root()
            .map_err(|error| self.io(Path::new(""), error))
    }

    fn read_exact_optional(
        &self,
        path: &Path,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, ProductLifecycleStoreError> {
        match self.root.read_authority_bounded(path, maximum) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(self.io(path, error)),
        }
    }

    fn read_exact_required(
        &self,
        path: &Path,
        maximum: u64,
    ) -> Result<Vec<u8>, ProductLifecycleStoreError> {
        self.read_exact_optional(path, maximum)?
            .ok_or_else(|| self.integrity(path, "required exact product leaf is missing"))
    }

    fn remove_if_exact(
        &self,
        path: &Path,
        expected: &str,
        report: &mut ProductLifecycleUninstallReport,
    ) -> Result<(), ProductLifecycleStoreError> {
        let actual = match self.read_exact_optional(path, MAX_ASSET_BYTES) {
            Ok(Some(bytes)) => digest(&bytes),
            Ok(None) => return Ok(()),
            Err(_) => {
                report.preserved_paths.push(self.display_path(path));
                return Ok(());
            }
        };
        if actual != expected {
            report.preserved_paths.push(self.display_path(path));
            return Ok(());
        }
        let authority = self
            .root
            .retain_authority()
            .map_err(|error| self.io(path, error))?;
        if authority
            .remove_file_with_validation(path, |directory, target| {
                let bytes = directory.read_authority_bounded(target, MAX_ASSET_BYTES)?;
                if digest(&bytes) != expected {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "owned leaf changed before exact uninstall",
                    ));
                }
                Ok(())
            })
            .is_err()
        {
            report.preserved_paths.push(self.display_path(path));
        }
        Ok(())
    }

    fn display_path(&self, path: &Path) -> PathBuf {
        self.lock.state_root.display_path().join(ROOT).join(path)
    }
    fn io(&self, path: &Path, error: io::Error) -> ProductLifecycleStoreError {
        ProductLifecycleStoreError::Io {
            path: self.display_path(path),
            source: error.to_string(),
        }
    }
    fn integrity(&self, path: &Path, reason: &str) -> ProductLifecycleStoreError {
        ProductLifecycleStoreError::Integrity {
            path: self.display_path(path),
            reason: reason.to_owned(),
        }
    }
}

fn generation_from_input(
    input: &ProductLifecycleGenerationInput,
) -> Result<ProductLifecycleGeneration, ProductLifecycleStoreError> {
    Ok(ProductLifecycleGeneration {
        generation_id: input.generation_id.clone(),
        version: input.version.clone(),
        release_sha256: input.release_sha256.clone(),
        receipt_sha256: digest(&input.receipt),
        assets: input
            .assets
            .iter()
            .map(|asset| ProductLifecycleAsset {
                path: asset.path.clone(),
                sha256: asset.sha256.clone(),
                executable: asset.executable,
            })
            .collect(),
    })
}

fn validate_input(
    input: &ProductLifecycleGenerationInput,
) -> Result<(), ProductLifecycleStoreError> {
    if !safe_generation_id(&input.generation_id)
        || !valid_version_label(&input.version)
        || !valid_digest(&input.release_sha256)
        || input.receipt.len() as u64 > MAX_RECEIPT_BYTES
    {
        return Err(invalid(
            "invalid generation identity, version, release digest, or receipt length",
        ));
    }
    let mut paths = BTreeSet::new();
    for asset in &input.assets {
        if !safe_relative_path(&asset.path)
            || !valid_digest(&asset.sha256)
            || !paths.insert(asset.path.as_str())
            || u64::try_from(asset.bytes.len()).unwrap_or(u64::MAX) > MAX_ASSET_BYTES
            || digest(&asset.bytes) != asset.sha256
        {
            return Err(invalid(
                "invalid, duplicate, oversized, or digest-mismatched candidate asset",
            ));
        }
    }
    Ok(())
}

fn validate_generation(
    generation: &ProductLifecycleGeneration,
) -> Result<(), ProductLifecycleStoreError> {
    if !safe_generation_id(&generation.generation_id)
        || !valid_version_label(&generation.version)
        || !valid_digest(&generation.release_sha256)
        || !valid_digest(&generation.receipt_sha256)
    {
        return Err(invalid("invalid immutable generation identity"));
    }
    let mut paths = BTreeSet::new();
    for asset in &generation.assets {
        if !safe_relative_path(&asset.path)
            || !valid_digest(&asset.sha256)
            || !paths.insert(asset.path.as_str())
        {
            return Err(invalid("invalid or duplicate immutable asset inventory"));
        }
    }
    Ok(())
}

fn safe_generation_id(value: &str) -> bool {
    if value.len() <= 11
        || value.len() > 128
        || !value.starts_with("generation-")
        || value.contains('\\')
    {
        return false;
    }
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(name)) if name == value)
        && components.next().is_none()
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.contains('\\')
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_version_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn digest(bytes: &[u8]) -> String {
    sha256_content_hash(bytes)
}
fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ProductLifecycleStoreError> {
    serde_json_canonicalizer::to_vec(value)
        .map_err(|error| invalid(format!("canonical lifecycle encoding failed: {error}")))
}
fn parse_state(
    bytes: &[u8],
    path: &Path,
) -> Result<ProductLifecycleState, ProductLifecycleStoreError> {
    let state: ProductLifecycleState =
        serde_json::from_slice(bytes).map_err(|error| ProductLifecycleStoreError::Integrity {
            path: path.to_path_buf(),
            reason: format!("state is not valid JSON: {error}"),
        })?;
    if canonical_json(&state)? != bytes {
        return Err(ProductLifecycleStoreError::Integrity {
            path: path.to_path_buf(),
            reason: "state is not canonical".to_owned(),
        });
    }
    Ok(state)
}
fn invalid(reason: impl Into<String>) -> ProductLifecycleStoreError {
    ProductLifecycleStoreError::Invalid {
        reason: reason.into(),
    }
}
fn io_error(lock: &EffectStoreLock, path: &Path, error: io::Error) -> ProductLifecycleStoreError {
    ProductLifecycleStoreError::Io {
        path: lock.state_root.display_path().join(path),
        source: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_id_is_one_safe_component() {
        assert!(safe_generation_id("generation-0123456789abcdef"));
        assert!(!safe_generation_id("generation-../outside"));
        assert!(!safe_generation_id("generation-a/b"));
        assert!(!safe_generation_id("generation-a\\b"));
    }

    #[test]
    fn candidate_data_has_no_host_or_private_key_authority() {
        let raw = r#"{"schema_version":"forge-product-lifecycle-store-v1","active_generation":null,"previous_generation":null,"generations":[],"selected_host":"codex","private_key":"forbidden"}"#;
        assert!(serde_json::from_str::<ProductLifecycleState>(raw).is_err());
    }

    #[test]
    fn candidate_assets_require_exact_digest_and_safe_path() {
        let input = ProductLifecycleGenerationInput {
            generation_id: "generation-0123456789abcdef".to_owned(),
            version: "0.12.0".to_owned(),
            release_sha256: format!("sha256:{}", "a".repeat(64)),
            receipt: b"receipt".to_vec(),
            assets: vec![ProductLifecycleAssetInput {
                path: "../outside".to_owned(),
                sha256: format!("sha256:{}", "b".repeat(64)),
                bytes: b"asset".to_vec(),
                executable: false,
            }],
        };
        assert!(validate_input(&input).is_err());
    }
}
