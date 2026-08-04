//! Sealed, descriptor-relative project-tree snapshot authority.
//!
//! Admission opens the requested root once, retains every ancestor binding from
//! the filesystem anchor, then walks the accepted tree only through directory
//! handles. Every accepted directory and file handle, stable identity, namespace
//! entry, and exact file byte sequence remains owned by the capability until it
//! is dropped. Revalidation never resolves the project through its ambient path.

use crate::retained_dir::{
    RetainedDirectory, RetainedFileAnchorBinding, RetainedFileIdentity, RetainedFileLifetimeAnchor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

const EXCLUDED_ROOT_NAMES: &[&str] = &[".git", ".forge-method", "target", "node_modules"];
const WORKFLOW_LOCAL_ROOT_NAME: &str = ".local";
const DIRECTORY_DIGEST_MARKER: &str = "directory";
const MAX_RETAINED_PROJECT_DEPTH: usize = 256;
const PROJECT_CAPABILITY_NONCE_BYTES: usize = 32;
const RETAINED_PROJECT_ANCHOR_SCHEMA_VERSION: &str = "forge-retained-project-anchors-v1";
const MAX_GIT_IGNORED_PATH_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_GIT_DIAGNOSTIC_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy)]
enum RetainedProjectCapturePolicy {
    Generic,
    ProjectSnapshot,
    WorkflowLocalRootExcluded,
    StoreOwnedProjectSnapshot,
}

impl RetainedProjectCapturePolicy {
    fn includes_root_name(self, name: &str) -> bool {
        !EXCLUDED_ROOT_NAMES.contains(&name)
            && !(matches!(self, Self::WorkflowLocalRootExcluded)
                && name == WORKFLOW_LOCAL_ROOT_NAME)
    }

    const fn honors_git_ignored_paths(self) -> bool {
        matches!(
            self,
            Self::ProjectSnapshot
                | Self::WorkflowLocalRootExcluded
                | Self::StoreOwnedProjectSnapshot
        )
    }
}

fn is_top_level_workflow_local_path(relative_path: &str) -> bool {
    relative_path == WORKFLOW_LOCAL_ROOT_NAME
        || relative_path
            .strip_prefix(WORKFLOW_LOCAL_ROOT_NAME)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[derive(Debug, Clone, Copy)]
enum RetainedFileAliasPolicy {
    SingleLink,
    StableAliases,
    StoreOwnedAnchorMutation,
}

impl RetainedFileAliasPolicy {
    const fn allows_multiple_links(self) -> bool {
        !matches!(self, Self::SingleLink)
    }

    const fn allows_alias_metadata_changes(self) -> bool {
        matches!(self, Self::StoreOwnedAnchorMutation)
    }
}

/// Opaque Store-owned witness for one exact accepted project tree.
///
/// The type has no public fields or `Clone` implementation. Its display path is
/// diagnostic only; after admission all traversal and revalidation starts from
/// retained handles and retained parent/component bindings.
pub struct RetainedProjectTree {
    display_root: PathBuf,
    ancestry: Vec<RetainedAncestorDirectory>,
    directories: Vec<RetainedTreeDirectory>,
    files: Vec<RetainedTreeFile>,
    snapshot_digest: String,
    regular_file_snapshot_digest: String,
    capture_policy: RetainedProjectCapturePolicy,
    file_alias_policy: RetainedFileAliasPolicy,
    git_ignored_paths: BTreeSet<String>,
}

/// Read-only regular-file observation derived from one retained tree.
///
/// It contains no ambient path and no mutation handle. Callers must keep the
/// originating [`RetainedProjectTree`] alive and revalidate it before trusting
/// a projection assembled from these observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedProjectRegularFileObservation {
    pub relative_path: String,
    pub content_digest: String,
    pub byte_length: u64,
    pub metadata_fingerprint: String,
}

/// Read-only directory observation derived from one retained tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedProjectDirectoryObservation {
    pub relative_path: String,
    pub metadata_fingerprint: String,
}

/// Store-private binding between a lifecycle lock and the exact governed
/// project root selected by its trusted caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedProjectRootBinding {
    display_root: PathBuf,
    identity: FileIdentity,
}

/// Canonical persisted binding for the complete retained project tree.
///
/// This type remains crate-private because its random nonces are valid only while
/// the originating [`RetainedProjectTree`] capability remains alive. A fresh
/// capture cannot remint this same-handle completion binding from reusable
/// platform identifiers or identical bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedProjectCompletionBinding {
    pub(crate) project_root_path_digest: String,
    pub(crate) project_root_capability_nonce: String,
    pub(crate) ancestry_digest: String,
    pub(crate) snapshot_digest: String,
    pub(crate) inventory_digest: String,
    pub(crate) inventory: Vec<RetainedProjectInventoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedProjectInventoryEntry {
    pub(crate) relative_path: String,
    pub(crate) object_kind: String,
    pub(crate) object_capability_nonce: String,
    pub(crate) content_digest: String,
    pub(crate) byte_length: u64,
}

/// Canonical persisted binding for the exact regular files in one accepted
/// project snapshot. Each entry points to a Store-owned lifetime anchor created
/// directly from the originating retained project handle.
///
/// Fields remain private so callers can persist and return the serde value but
/// cannot mint one from reusable device/inode or volume/index identities. A
/// deserialized value acquires authority only when a fresh Store lock reopens and
/// cross-binds it through [`crate::EffectStoreLock::open_project_tree_anchors`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedProjectAnchorBinding {
    pub(crate) schema_version: String,
    pub(crate) project_root_path_digest: String,
    pub(crate) snapshot_digest: String,
    pub(crate) files: Vec<RetainedProjectFileAnchorBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedProjectFileAnchorBinding {
    pub(crate) relative_path: String,
    pub(crate) content_digest: String,
    pub(crate) byte_length: u64,
    pub(crate) anchor: RetainedFileAnchorBinding,
}

/// Opaque live half of a persisted project anchor binding.
///
/// The exact project handles and exact private anchor handles remain alive
/// together. A fresh lifecycle acquisition may reopen the persisted anchors,
/// but it can accept them only after cross-binding every current retained
/// project file to the exact anchored object.
pub struct RetainedProjectLifetimeAnchors {
    binding: RetainedProjectAnchorBinding,
    files: Vec<RetainedProjectAnchoredFile>,
}

struct RetainedProjectAnchoredFile {
    relative_path: String,
    file: File,
    identity: RetainedFileIdentity,
    anchor: RetainedFileLifetimeAnchor,
}

impl fmt::Debug for RetainedProjectLifetimeAnchors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedProjectLifetimeAnchors")
            .field("snapshot_digest", &self.binding.snapshot_digest)
            .field("file_count", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl RetainedProjectLifetimeAnchors {
    /// Borrow the durable, serde-capable binding for this exact anchor set.
    ///
    /// The binding contains only Store-relative private anchor names, random
    /// nonces, content digests, lengths, and project/snapshot digests. Reusable
    /// platform object identities and freshly minted project capability nonces
    /// are deliberately absent.
    #[must_use]
    pub fn binding(&self) -> &RetainedProjectAnchorBinding {
        &self.binding
    }

    /// Revalidate every live project handle against its exact Store-owned anchor.
    ///
    /// # Errors
    ///
    /// Fails closed if the persisted binding, private anchor namespace, retained
    /// project handle, exact file identity, bytes, or length changed.
    pub fn revalidate(&self) -> Result<(), RetainedProjectTreeError> {
        if self.binding.schema_version != RETAINED_PROJECT_ANCHOR_SCHEMA_VERSION
            || self.binding.files.is_empty()
            || self.binding.files.len() != self.files.len()
        {
            return Err(RetainedProjectTreeError::Identity {
                path: PathBuf::from("<retained-project-anchors>"),
                reason: "retained project anchor set differs from its persisted binding".to_owned(),
            });
        }
        for (expected, retained) in self.binding.files.iter().zip(&self.files) {
            let path = PathBuf::from(&retained.relative_path);
            if expected.relative_path != retained.relative_path
                || &expected.anchor != retained.anchor.binding()
                || expected.content_digest != expected.anchor.content_digest
                || expected.byte_length != expected.anchor.byte_length
            {
                return Err(identity_error(
                    &path,
                    "retained project anchor entry differs from its persisted binding",
                ));
            }
            retained
                .anchor
                .validate_retained_file(&retained.file, &retained.identity)
                .map_err(|error| io_error(&path, error))?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct RetainedProjectAncestryEntry {
    depth: usize,
    name_digest: Option<String>,
    capability_nonce: String,
}

impl fmt::Debug for RetainedProjectTree {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedProjectTree")
            .field("snapshot_digest", &self.snapshot_digest)
            .field("directory_count", &self.directories.len())
            .field("file_count", &self.files.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct RetainedAncestorDirectory {
    handle: File,
    identity: FileIdentity,
    capability_nonce: String,
    name_in_parent: Option<OsString>,
}

#[derive(Debug, Clone)]
struct RetainedMetadata {
    metadata: Metadata,
    #[cfg(windows)]
    file_information: crate::windows_file_info::WindowsFileInformation,
}

impl RetainedMetadata {
    fn capture(file: &File, path: &Path) -> Result<Self, RetainedProjectTreeError> {
        let metadata = file.metadata().map_err(|error| io_error(path, error))?;
        #[cfg(windows)]
        let file_information = crate::windows_file_info::file_information(file)
            .map_err(|error| io_error(path, error))?;
        Ok(Self {
            metadata,
            #[cfg(windows)]
            file_information,
        })
    }
}

#[derive(Debug)]
struct RetainedTreeDirectory {
    handle: File,
    metadata: RetainedMetadata,
    capability_nonce: String,
    relative_path: String,
    parent: Option<usize>,
    name_in_parent: Option<OsString>,
    entries: Vec<RetainedTreeEntry>,
}

#[derive(Debug)]
struct RetainedTreeFile {
    handle: File,
    metadata: RetainedMetadata,
    capability_nonce: String,
    relative_path: String,
    parent: usize,
    name_in_parent: OsString,
    content_digest: String,
    exact_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RetainedTreeEntry {
    name: OsString,
    object_id: u64,
    kind: RetainedTreeEntryKind,
    witness_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetainedTreeEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryEntry {
    name: OsString,
    object_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FileIdentity {
    platform: PlatformIdentity,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PlatformIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PlatformIdentity {
    volume: u32,
    index: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PlatformIdentity;

/// Failure while retaining or revalidating a sealed project tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetainedProjectTreeError {
    InvalidRoot {
        path: PathBuf,
        reason: String,
    },
    ResourceLimit {
        resource: &'static str,
        maximum: u64,
    },
    Identity {
        path: PathBuf,
        reason: String,
    },
    Io {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for RetainedProjectTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot { path, reason } => {
                write!(
                    formatter,
                    "invalid retained project root {}: {reason}",
                    path.display()
                )
            }
            Self::ResourceLimit { resource, maximum } => {
                write!(
                    formatter,
                    "retained project {resource} exceeds limit {maximum}"
                )
            }
            Self::Identity { path, reason } => write!(
                formatter,
                "retained project identity {} changed: {reason}",
                path.display()
            ),
            Self::Io { path, reason } => {
                write!(
                    formatter,
                    "retained project I/O {} failed: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for RetainedProjectTreeError {}

impl RetainedProjectTree {
    /// Capture one bounded project tree and retain its complete accepted witness.
    ///
    /// The root is made absolute lexically, opened from the filesystem anchor,
    /// and reached one no-follow component at a time. The accepted tree excludes
    /// the fixed Forge/VCS/build/cache root names used by the lifecycle adapter.
    /// Links, reparse points, special objects, hard-linked files, non-UTF-8 names,
    /// duplicate entries, and identity instability fail closed.
    ///
    /// # Errors
    ///
    /// Returns a typed path, resource, identity, or descriptor-relative I/O error.
    pub fn capture(
        project_root: impl AsRef<Path>,
        maximum_entries: usize,
        maximum_bytes: u64,
    ) -> Result<Self, RetainedProjectTreeError> {
        Self::capture_with_file_alias_policy(
            project_root.as_ref(),
            maximum_entries,
            None,
            maximum_bytes,
            RetainedProjectCapturePolicy::Generic,
            RetainedFileAliasPolicy::SingleLink,
        )
    }
    /// Capture a strict Project Snapshot with Git-aware content selection.
    ///
    /// Existing ignored and untracked paths are excluded before no-follow traversal;
    /// tracked paths and relevant links remain governed. This is the shared selection
    /// used by promotion, workflow governance, and Domain Pack lifecycle capture.
    pub fn capture_project_snapshot(
        project_root: impl AsRef<Path>,
        maximum_entries: usize,
        maximum_bytes: u64,
    ) -> Result<Self, RetainedProjectTreeError> {
        Self::capture_with_file_alias_policy(
            project_root.as_ref(),
            maximum_entries,
            None,
            maximum_bytes,
            RetainedProjectCapturePolicy::ProjectSnapshot,
            RetainedFileAliasPolicy::SingleLink,
        )
    }

    /// Capture a read-only project observation that may already have file aliases.
    ///
    /// Existing hard links are accepted, but their exact link count and change
    /// metadata are frozen at capture and rechecked at every later boundary. This
    /// policy is suitable only for ephemeral observation; it grants no anchor or
    /// mutation authority.
    pub fn capture_allowing_stable_file_aliases(
        project_root: impl AsRef<Path>,
        maximum_entries: usize,
        maximum_files: usize,
        maximum_bytes: u64,
    ) -> Result<Self, RetainedProjectTreeError> {
        Self::capture_with_file_alias_policy(
            project_root.as_ref(),
            maximum_entries,
            Some(maximum_files),
            maximum_bytes,
            RetainedProjectCapturePolicy::Generic,
            RetainedFileAliasPolicy::StableAliases,
        )
    }

    /// Capture a workflow-governance Project Snapshot.
    ///
    /// In a Git repository, existing ignored and untracked paths are classified
    /// before no-follow traversal and excluded. A tracked path is never excluded,
    /// even when an ignore pattern matches it. Without a root `.git` control entry,
    /// capture remains conservative and governs the complete accepted tree. The
    /// already-existing top-level `.local` namespace is also excluded; nested
    /// `.local` directories remain governed.
    pub fn capture_workflow_snapshot_allowing_stable_file_aliases(
        project_root: impl AsRef<Path>,
        maximum_entries: usize,
        maximum_files: usize,
        maximum_bytes: u64,
    ) -> Result<Self, RetainedProjectTreeError> {
        Self::capture_with_file_alias_policy(
            project_root.as_ref(),
            maximum_entries,
            Some(maximum_files),
            maximum_bytes,
            RetainedProjectCapturePolicy::WorkflowLocalRootExcluded,
            RetainedFileAliasPolicy::StableAliases,
        )
    }

    /// Capture a Domain Pack Project Snapshot that may already carry Store-owned
    /// lifetime anchor links from an earlier committed completion.
    ///
    /// Git-aware selection is identical to the workflow snapshot: ignored untracked
    /// paths are excluded before traversal, while tracked paths remain governed.
    /// This constructor does not itself accept existing anchor links as authority;
    /// the lifecycle reader must cross-bind the returned exact handles to the
    /// independently selected persisted project anchor set.
    pub fn capture_allowing_store_owned_file_anchors(
        project_root: impl AsRef<Path>,
        maximum_entries: usize,
        maximum_bytes: u64,
    ) -> Result<Self, RetainedProjectTreeError> {
        Self::capture_with_file_alias_policy(
            project_root.as_ref(),
            maximum_entries,
            None,
            maximum_bytes,
            RetainedProjectCapturePolicy::StoreOwnedProjectSnapshot,
            RetainedFileAliasPolicy::StoreOwnedAnchorMutation,
        )
    }

    fn capture_with_file_alias_policy(
        project_root: &Path,
        maximum_entries: usize,
        maximum_files: Option<usize>,
        maximum_bytes: u64,
        capture_policy: RetainedProjectCapturePolicy,
        file_alias_policy: RetainedFileAliasPolicy,
    ) -> Result<Self, RetainedProjectTreeError> {
        if maximum_entries == 0 {
            return Err(RetainedProjectTreeError::ResourceLimit {
                resource: "snapshot entries",
                maximum: 0,
            });
        }
        if maximum_files == Some(0) {
            return Err(RetainedProjectTreeError::ResourceLimit {
                resource: "snapshot files",
                maximum: 0,
            });
        }
        if maximum_bytes == 0 {
            return Err(RetainedProjectTreeError::ResourceLimit {
                resource: "snapshot bytes",
                maximum: 0,
            });
        }
        let display_root = absolute_lexical_path(project_root)?;
        let ancestry = open_ancestry(&display_root)?;
        let root = ancestry
            .last()
            .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
                path: display_root.clone(),
                reason: "project root has no retained directory witness".to_owned(),
            })?;
        let root_handle = root
            .handle
            .try_clone()
            .map_err(|error| io_error(&display_root, error))?;
        let root_metadata = RetainedMetadata::capture(&root_handle, &display_root)?;
        validate_directory_metadata(&root_metadata, &display_root)?;
        let git_ignored_paths = if capture_policy.honors_git_ignored_paths() {
            discover_git_ignored_paths(&display_root)?
        } else {
            BTreeSet::new()
        };
        let root_capability_nonce = project_capability_nonce(&display_root)?;
        let mut tree = Self {
            display_root,
            ancestry,
            directories: vec![RetainedTreeDirectory {
                handle: root_handle,
                metadata: root_metadata,
                capability_nonce: root_capability_nonce,
                relative_path: String::new(),
                parent: None,
                name_in_parent: None,
                entries: Vec::new(),
            }],
            files: Vec::new(),
            snapshot_digest: String::new(),
            regular_file_snapshot_digest: String::new(),
            capture_policy,
            file_alias_policy,
            git_ignored_paths,
        };
        let mut digest_entries = Vec::new();
        let mut accepted_entries = 0usize;
        let mut accepted_files = 0usize;
        let mut accepted_bytes = 0u64;
        tree.capture_directory(
            0,
            maximum_entries,
            maximum_files,
            maximum_bytes,
            &mut accepted_entries,
            &mut accepted_files,
            &mut accepted_bytes,
            &mut digest_entries,
        )?;
        tree.validate_ancestry()?;
        tree.validate_namespace_pass(tree.file_alias_policy)?;
        let mut regular_file_entries = tree
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.content_digest.clone()))
            .collect::<Vec<_>>();
        regular_file_entries.sort();
        tree.regular_file_snapshot_digest = digest_entries_digest(&regular_file_entries)?;
        digest_entries.sort();
        tree.snapshot_digest = digest_entries_digest(&digest_entries)?;
        Ok(tree)
    }

    /// Digest of the exact accepted bytes and directory namespace retained by
    /// this capability.
    #[must_use]
    pub fn snapshot_digest(&self) -> &str {
        &self.snapshot_digest
    }

    /// Digest of the retained regular-file path/content pairs only.
    ///
    /// Workflow governance uses this compatibility projection to preserve its
    /// persisted v0 snapshot digest contract for link-free projects. Directory
    /// namespace and every ancestor binding remain retained and are still checked
    /// by [`Self::revalidate`]; links and special objects fail closed at capture.
    #[must_use]
    pub fn regular_file_snapshot_digest(&self) -> &str {
        &self.regular_file_snapshot_digest
    }

    /// Sorted exact regular-file projection for read-only planning.
    #[must_use]
    pub fn regular_file_observations(&self) -> Vec<RetainedProjectRegularFileObservation> {
        let mut observations = self
            .files
            .iter()
            .map(|file| RetainedProjectRegularFileObservation {
                relative_path: file.relative_path.clone(),
                content_digest: file.content_digest.clone(),
                byte_length: file.metadata.metadata.len(),
                metadata_fingerprint: promotion_metadata_fingerprint(&file.metadata),
            })
            .collect::<Vec<_>>();
        observations.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        observations
    }

    /// Sorted workflow-governance regular-file view of this retained tree.
    ///
    /// The complete tree remains retained, but the already-existing top-level
    /// `.local` namespace is omitted from the persisted workflow snapshot
    /// projection. Nested `.local` paths remain governed.
    #[must_use]
    pub fn workflow_regular_file_observations(&self) -> Vec<RetainedProjectRegularFileObservation> {
        self.regular_file_observations()
            .into_iter()
            .filter(|observation| !is_top_level_workflow_local_path(&observation.relative_path))
            .collect()
    }

    /// Digest the workflow-governance regular-file view retained by this tree.
    ///
    /// This preserves the persisted workflow v0 digest when the underlying
    /// capability was captured with the broader Domain Pack project policy.
    pub fn workflow_regular_file_snapshot_digest(
        &self,
    ) -> Result<String, RetainedProjectTreeError> {
        let entries = self
            .workflow_regular_file_observations()
            .into_iter()
            .map(|observation| (observation.relative_path, observation.content_digest))
            .collect::<Vec<_>>();
        retained_regular_file_projection_digest(&entries)
    }

    /// Copy bytes from the already-retained exact file handle for one normalized
    /// project-relative path.
    ///
    /// This never reopens the ambient pathname. The complete retained tree is
    /// revalidated before and after the copy so a promotion payload cannot be
    /// sourced from a substituted worktree entry.
    pub fn exact_regular_file_bytes(
        &self,
        relative_path: &str,
    ) -> Result<Option<Vec<u8>>, RetainedProjectTreeError> {
        if relative_path.is_empty()
            || relative_path.starts_with('/')
            || relative_path.starts_with('\\')
            || relative_path.contains('\\')
            || relative_path
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(RetainedProjectTreeError::InvalidRoot {
                path: self.display_root.join(relative_path),
                reason: "exact-byte lookup requires a normalized project-relative path".to_owned(),
            });
        }
        self.revalidate()?;
        let bytes = self
            .files
            .iter()
            .find(|file| file.relative_path == relative_path)
            .map(|file| file.exact_bytes.clone());
        self.revalidate()?;
        Ok(bytes)
    }

    /// Digest of the exact live destination capability used by promotion apply.
    ///
    /// The random root nonce exists only for this retained capture, so a later
    /// ambient pathname reopen cannot recreate this binding even if it points
    /// at byte-identical content.
    pub fn exact_mutation_capability_digest(&self) -> Result<String, RetainedProjectTreeError> {
        self.revalidate()?;
        let root =
            self.directories
                .first()
                .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
                    path: self.display_root.clone(),
                    reason: "retained project tree has no root capability".to_owned(),
                })?;
        let mut hasher = Sha256::new();
        hasher.update(b"forge.retained-project.exact-mutation-capability.v1\0");
        hasher.update(os_path_digest(&self.display_root).as_bytes());
        hasher.update(b"\0");
        hasher.update(root.capability_nonce.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.snapshot_digest.as_bytes());
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }

    /// Prove that one exact admitted regular file can be mutated through a
    /// retained read/write handle without reopening the project root.
    ///
    /// Ticket 10 deliberately supports Unix only. Other platforms fail before
    /// durable intent until their exact-handle metadata-preserving write path
    /// has equivalent evidence.
    pub fn preflight_exact_regular_file_write(
        &self,
        relative_path: &str,
        expected: &[u8],
    ) -> Result<(), RetainedProjectTreeError> {
        self.revalidate()?;
        let file_index = self.exact_regular_file_index(relative_path)?;
        let retained = &self.files[file_index];
        if retained.exact_bytes != expected {
            return Err(identity_error(
                &self.display_root.join(relative_path_to_path(relative_path)),
                "exact mutation preflight bytes differ from the admitted destination",
            ));
        }
        if hard_link_count(&retained.metadata) != 1 {
            return Err(identity_error(
                &self.display_root.join(relative_path_to_path(relative_path)),
                "exact promotion mutation requires a single-link destination file",
            ));
        }
        let parent = &self.directories[retained.parent];
        let display_path = self.display_root.join(relative_path_to_path(relative_path));
        let writable = platform::open_child_for_mutation(&parent.handle, &retained.name_in_parent)
            .map_err(|error| io_error(&display_path, error))?;
        let writable_metadata = RetainedMetadata::capture(&writable, &display_path)?;
        validate_file_metadata(&writable_metadata, &display_path, self.file_alias_policy)?;
        if !same_stable_file_metadata(
            &retained.metadata,
            &writable_metadata,
            self.file_alias_policy,
        ) {
            return Err(identity_error(
                &display_path,
                "writable handle does not name the exact admitted destination file",
            ));
        }
        let bytes = read_retained_file(&writable, retained.metadata.metadata.len(), &display_path)?;
        if bytes != expected {
            return Err(identity_error(
                &display_path,
                "writable handle bytes differ from the exact admitted destination",
            ));
        }
        self.revalidate()
    }

    /// Mutate one exact admitted regular file in place and advance this live
    /// retained capability to the new bytes.
    ///
    /// In-place mutation preserves the admitted object, Unix mode, uid, gid,
    /// and link count and therefore creates no hidden quarantine namespace.
    pub(crate) fn apply_exact_regular_file_write(
        &mut self,
        relative_path: &str,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<(), RetainedProjectTreeError> {
        self.preflight_exact_regular_file_write(relative_path, expected)?;
        let file_index = self.exact_regular_file_index(relative_path)?;
        let parent_index = self.files[file_index].parent;
        let name = self.files[file_index].name_in_parent.clone();
        let before_metadata = self.files[file_index].metadata.clone();
        let display_path = self.display_root.join(relative_path_to_path(relative_path));
        let mut writable =
            platform::open_child_for_mutation(&self.directories[parent_index].handle, &name)
                .map_err(|error| io_error(&display_path, error))?;
        let rebound_metadata = RetainedMetadata::capture(&writable, &display_path)?;
        if !same_stable_file_metadata(&before_metadata, &rebound_metadata, self.file_alias_policy) {
            return Err(identity_error(
                &display_path,
                "destination changed before exact retained mutation",
            ));
        }
        writable
            .seek(SeekFrom::Start(0))
            .and_then(|_| writable.write_all(replacement))
            .and_then(|_| writable.set_len(u64::try_from(replacement.len()).unwrap_or(u64::MAX)))
            .and_then(|_| writable.sync_all())
            .map_err(|error| io_error(&display_path, error))?;
        let after_metadata = RetainedMetadata::capture(&writable, &display_path)?;
        validate_file_metadata(&after_metadata, &display_path, self.file_alias_policy)?;
        if !same_exact_file_and_admitted_metadata_after_mutation(
            &before_metadata,
            &after_metadata,
            self.file_alias_policy,
        ) {
            return Err(identity_error(
                &display_path,
                "exact mutation changed admitted file identity, mode, owner, or link policy",
            ));
        }
        let observed = read_retained_file(
            &writable,
            u64::try_from(replacement.len()).unwrap_or(u64::MAX),
            &display_path,
        )?;
        if observed != replacement {
            return Err(identity_error(
                &display_path,
                "exact retained mutation readback differs from requested bytes",
            ));
        }
        self.files[file_index].handle = writable;
        self.files[file_index].metadata = after_metadata;
        self.files[file_index].exact_bytes = replacement.to_vec();
        self.refresh_snapshot_digests()?;
        self.revalidate()
    }

    fn exact_regular_file_index(
        &self,
        relative_path: &str,
    ) -> Result<usize, RetainedProjectTreeError> {
        if relative_path.is_empty()
            || relative_path.starts_with('/')
            || relative_path.starts_with('\\')
            || relative_path.contains('\\')
            || relative_path
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(RetainedProjectTreeError::InvalidRoot {
                path: self.display_root.join(relative_path),
                reason: "exact mutation requires a normalized project-relative path".to_owned(),
            });
        }
        self.files
            .iter()
            .position(|file| file.relative_path == relative_path)
            .ok_or_else(|| RetainedProjectTreeError::Identity {
                path: self.display_root.join(relative_path_to_path(relative_path)),
                reason: "exact mutation target is not an admitted regular file".to_owned(),
            })
    }

    fn refresh_snapshot_digests(&mut self) -> Result<(), RetainedProjectTreeError> {
        for file in &mut self.files {
            file.content_digest = crate::sha256_content_hash(&file.exact_bytes);
        }
        let mut regular = self
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.content_digest.clone()))
            .collect::<Vec<_>>();
        regular.sort();
        self.regular_file_snapshot_digest = digest_entries_digest(&regular)?;
        let mut complete = regular;
        complete.extend(self.directories.iter().skip(1).map(|directory| {
            (
                directory.relative_path.clone(),
                DIRECTORY_DIGEST_MARKER.to_owned(),
            )
        }));
        complete.sort();
        self.snapshot_digest = digest_entries_digest(&complete)?;
        Ok(())
    }

    /// Sorted directory projection for empty-directory and mode diagnostics.
    ///
    /// The root itself is omitted; every returned path is project-relative.
    #[must_use]
    pub fn directory_observations(&self) -> Vec<RetainedProjectDirectoryObservation> {
        let mut observations = self
            .directories
            .iter()
            .filter(|directory| !directory.relative_path.is_empty())
            .map(|directory| RetainedProjectDirectoryObservation {
                relative_path: directory.relative_path.clone(),
                metadata_fingerprint: promotion_metadata_fingerprint(&directory.metadata),
            })
            .collect::<Vec<_>>();
        observations.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        observations
    }

    /// Compare retained root identities without exposing reusable platform ids.
    pub fn aliases_root(&self, other: &Self) -> Result<bool, RetainedProjectTreeError> {
        Ok(identity(&self.directories[0].metadata)? == identity(&other.directories[0].metadata)?)
    }

    /// Detect any exact regular-file object shared across two retained trees.
    /// Promotion uses this as defense in depth in addition to single-link capture.
    pub fn shares_regular_file_object_with(
        &self,
        other: &Self,
    ) -> Result<bool, RetainedProjectTreeError> {
        let left = self
            .files
            .iter()
            .map(|file| identity(&file.metadata))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        for right in &other.files {
            if left.contains(&identity(&right.metadata)?) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Build the canonical exact-identity inventory consumed by a Store-owned
    /// lifecycle completion record.
    ///
    /// The full retained tree is revalidated before projection. The returned
    /// inventory includes every accepted directory and file, a random nonce
    /// owned by its still-live exact handle, and every file content digest. A
    /// later pathname reopen cannot remint this binding from reusable platform
    /// identifiers, even when replacement bytes are identical.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn completion_binding(
        &self,
    ) -> Result<RetainedProjectCompletionBinding, RetainedProjectTreeError> {
        self.revalidate()?;

        let ancestry = self
            .ancestry
            .iter()
            .enumerate()
            .map(|(depth, directory)| {
                Ok(RetainedProjectAncestryEntry {
                    depth,
                    name_digest: directory.name_in_parent.as_deref().map(os_component_digest),
                    capability_nonce: directory.capability_nonce.clone(),
                })
            })
            .collect::<Result<Vec<_>, RetainedProjectTreeError>>()?;
        let ancestry_bytes = serde_json_canonicalizer::to_vec(&ancestry).map_err(|error| {
            RetainedProjectTreeError::InvalidRoot {
                path: self.display_root.clone(),
                reason: format!("project ancestry canonicalization failed: {error}"),
            }
        })?;

        let mut inventory = self
            .directories
            .iter()
            .map(|directory| {
                Ok(RetainedProjectInventoryEntry {
                    relative_path: directory.relative_path.clone(),
                    object_kind: DIRECTORY_DIGEST_MARKER.to_owned(),
                    object_capability_nonce: directory.capability_nonce.clone(),
                    content_digest: DIRECTORY_DIGEST_MARKER.to_owned(),
                    byte_length: 0,
                })
            })
            .chain(self.files.iter().map(|file| {
                Ok(RetainedProjectInventoryEntry {
                    relative_path: file.relative_path.clone(),
                    object_kind: "file".to_owned(),
                    object_capability_nonce: file.capability_nonce.clone(),
                    content_digest: file.content_digest.clone(),
                    byte_length: u64::try_from(file.exact_bytes.len()).unwrap_or(u64::MAX),
                })
            }))
            .collect::<Result<Vec<_>, RetainedProjectTreeError>>()?;
        inventory.sort_by(|left, right| {
            left.relative_path
                .cmp(&right.relative_path)
                .then_with(|| left.object_kind.cmp(&right.object_kind))
        });
        let inventory_bytes = serde_json_canonicalizer::to_vec(&inventory).map_err(|error| {
            RetainedProjectTreeError::InvalidRoot {
                path: self.display_root.clone(),
                reason: format!("project inventory canonicalization failed: {error}"),
            }
        })?;
        let root =
            self.directories
                .first()
                .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
                    path: self.display_root.clone(),
                    reason: "project completion inventory has no retained root".to_owned(),
                })?;
        Ok(RetainedProjectCompletionBinding {
            project_root_path_digest: os_path_digest(&self.display_root),
            project_root_capability_nonce: root.capability_nonce.clone(),
            ancestry_digest: crate::sha256_content_hash(&ancestry_bytes),
            snapshot_digest: self.snapshot_digest.clone(),
            inventory_digest: crate::sha256_content_hash(&inventory_bytes),
            inventory,
        })
    }

    /// Validate a persisted project binding using this same opaque lifetime
    /// capability. Freshly captured handles carry fresh nonces and cannot remint
    /// authority from byte-identical paths or reusable platform identifiers.
    pub(crate) fn validate_completion_binding(
        &self,
        expected: &RetainedProjectCompletionBinding,
    ) -> Result<(), RetainedProjectTreeError> {
        let current = self.completion_binding()?;
        if &current == expected {
            Ok(())
        } else {
            Err(identity_error(
                &self.display_root,
                "retained project capability differs from the persisted completion binding",
            ))
        }
    }

    /// Transfer this exact project snapshot into durable Store-owned file anchors.
    ///
    /// Every accepted regular file is linked directly from its retained handle;
    /// no project pathname is reopened to mint authority. The returned opaque set
    /// keeps both sides of every exact-file binding alive through selector commit.
    pub(crate) fn retain_completion_anchors(
        &self,
        authority_root: &RetainedDirectory,
        anchor_directory: &Path,
    ) -> Result<RetainedProjectLifetimeAnchors, RetainedProjectTreeError> {
        self.revalidate()?;
        if self.files.is_empty() {
            return Err(identity_error(
                &self.display_root,
                "project has no regular file that can carry a durable exact-object anchor",
            ));
        }
        let mut project_files = self.files.iter().collect::<Vec<_>>();
        project_files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let mut bindings = Vec::with_capacity(project_files.len());
        let mut retained = Vec::with_capacity(project_files.len());
        for project_file in project_files {
            let display_path = self
                .display_root
                .join(relative_path_to_path(&project_file.relative_path));
            let identity = RetainedDirectory::identity_of(&project_file.handle)
                .map_err(|error| io_error(&display_path, error))?;
            let content_digest = crate::sha256_content_hash(&project_file.exact_bytes);
            let byte_length = u64::try_from(project_file.exact_bytes.len()).unwrap_or(u64::MAX);
            let entry_directory =
                anchor_directory.join(project_anchor_entry_directory(&project_file.relative_path));
            let anchor = authority_root
                .retain_file_lifetime_anchor(
                    &entry_directory,
                    &project_file.handle,
                    &identity,
                    &content_digest,
                    byte_length,
                )
                .map_err(|error| io_error(&display_path, error))?;
            bindings.push(RetainedProjectFileAnchorBinding {
                relative_path: project_file.relative_path.clone(),
                content_digest,
                byte_length,
                anchor: anchor.binding().clone(),
            });
            retained.push(RetainedProjectAnchoredFile {
                relative_path: project_file.relative_path.clone(),
                file: project_file
                    .handle
                    .try_clone()
                    .map_err(|error| io_error(&display_path, error))?,
                identity,
                anchor,
            });
        }
        self.revalidate()?;
        let anchors = RetainedProjectLifetimeAnchors {
            binding: RetainedProjectAnchorBinding {
                schema_version: RETAINED_PROJECT_ANCHOR_SCHEMA_VERSION.to_owned(),
                project_root_path_digest: os_path_digest(&self.display_root),
                snapshot_digest: self.snapshot_digest.clone(),
                files: bindings,
            },
            files: retained,
        };
        anchors.revalidate()?;
        Ok(anchors)
    }

    /// Reopen a persisted project anchor set and cross-bind it to this exact
    /// freshly retained project tree. Byte-identical replacement cannot satisfy
    /// the exact-file comparisons against the lifetime anchors.
    pub(crate) fn open_completion_anchors(
        &self,
        authority_root: &RetainedDirectory,
        binding: &RetainedProjectAnchorBinding,
    ) -> Result<RetainedProjectLifetimeAnchors, RetainedProjectTreeError> {
        self.revalidate()?;
        if binding.schema_version != RETAINED_PROJECT_ANCHOR_SCHEMA_VERSION
            || binding.project_root_path_digest != os_path_digest(&self.display_root)
            || binding.snapshot_digest != self.snapshot_digest
            || binding.files.is_empty()
            || binding
                .files
                .windows(2)
                .any(|pair| pair[0].relative_path >= pair[1].relative_path)
        {
            return Err(identity_error(
                &self.display_root,
                "persisted project anchor binding is non-canonical or belongs to another snapshot",
            ));
        }
        let mut project_files = self.files.iter().collect::<Vec<_>>();
        project_files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        if project_files.len() != binding.files.len() {
            return Err(identity_error(
                &self.display_root,
                "persisted project anchor inventory differs from the retained project",
            ));
        }
        let mut retained = Vec::with_capacity(project_files.len());
        for (project_file, expected) in project_files.into_iter().zip(&binding.files) {
            let display_path = self
                .display_root
                .join(relative_path_to_path(&project_file.relative_path));
            let content_digest = crate::sha256_content_hash(&project_file.exact_bytes);
            let byte_length = u64::try_from(project_file.exact_bytes.len()).unwrap_or(u64::MAX);
            if expected.relative_path != project_file.relative_path
                || expected.content_digest != content_digest
                || expected.byte_length != byte_length
                || expected.anchor.content_digest != content_digest
                || expected.anchor.byte_length != byte_length
            {
                return Err(identity_error(
                    &display_path,
                    "persisted project file anchor differs from the retained project file",
                ));
            }
            expected
                .anchor
                .canonical_digest()
                .map_err(|error| io_error(&display_path, error))?;
            let anchor = authority_root
                .open_file_lifetime_anchor(&expected.anchor)
                .map_err(|error| io_error(&display_path, error))?;
            let identity = RetainedDirectory::identity_of(&project_file.handle)
                .map_err(|error| io_error(&display_path, error))?;
            anchor
                .validate_retained_file(&project_file.handle, &identity)
                .map_err(|error| io_error(&display_path, error))?;
            retained.push(RetainedProjectAnchoredFile {
                relative_path: project_file.relative_path.clone(),
                file: project_file
                    .handle
                    .try_clone()
                    .map_err(|error| io_error(&display_path, error))?,
                identity,
                anchor,
            });
        }
        self.revalidate()?;
        let anchors = RetainedProjectLifetimeAnchors {
            binding: binding.clone(),
            files: retained,
        };
        anchors.revalidate()?;
        Ok(anchors)
    }

    /// Rehash and revalidate the same retained tree without ambient path reopen.
    ///
    /// Both namespace passes re-enumerate each retained directory handle and
    /// reopen every retained child relative to its retained parent. The file pass
    /// rereads the original retained file handles and requires exact admitted
    /// bytes plus unchanged stable metadata. Ancestor bindings are checked before
    /// and after the full-tree passes.
    ///
    /// # Errors
    ///
    /// Fails closed on root/component substitution, whole-root rename, missing or
    /// extra accepted entries, byte changes, byte-identical replacement, metadata
    /// drift, links, special objects, or descriptor-relative I/O failure.
    pub fn revalidate(&self) -> Result<(), RetainedProjectTreeError> {
        self.revalidate_with_file_alias_policy(self.file_alias_policy)
    }

    fn revalidate_with_file_alias_policy(
        &self,
        file_alias_policy: RetainedFileAliasPolicy,
    ) -> Result<(), RetainedProjectTreeError> {
        self.validate_ancestry()?;
        self.validate_namespace_pass(file_alias_policy)?;
        let mut digest_entries = self.revalidate_files(file_alias_policy)?;
        self.validate_namespace_pass(file_alias_policy)?;
        self.validate_ancestry()?;
        digest_entries.sort();
        let actual_regular_file_digest = digest_entries_digest(&digest_entries)?;
        if actual_regular_file_digest != self.regular_file_snapshot_digest {
            return Err(identity_error(
                &self.display_root,
                "retained regular-file digest differs from its admitted snapshot",
            ));
        }
        for directory in self.directories.iter().skip(1) {
            digest_entries.push((
                directory.relative_path.clone(),
                DIRECTORY_DIGEST_MARKER.to_owned(),
            ));
        }
        digest_entries.sort();
        let actual_digest = digest_entries_digest(&digest_entries)?;
        if actual_digest != self.snapshot_digest {
            return Err(identity_error(
                &self.display_root,
                "retained project digest differs from its admitted snapshot",
            ));
        }
        Ok(())
    }

    /// Revalidate exact bytes and namespace while permitting only the file aliases
    /// already present at capture.
    ///
    /// Unlike the lifecycle mutation policy, this freezes each retained file's
    /// link count and change metadata. It is suitable for a shared read-only
    /// observation consumed by workflow governance.
    pub fn revalidate_with_stable_file_aliases(&self) -> Result<(), RetainedProjectTreeError> {
        self.revalidate_with_file_alias_policy(RetainedFileAliasPolicy::StableAliases)
    }

    /// Require that no project file relies on the lifecycle-only allowance for
    /// Store-owned anchor links. Pristine lifecycle state uses this check so an
    /// unselected hard-linked project cannot be treated as clean bootstrap.
    pub fn revalidate_without_store_owned_file_anchors(
        &self,
    ) -> Result<(), RetainedProjectTreeError> {
        self.revalidate_with_file_alias_policy(RetainedFileAliasPolicy::SingleLink)
    }

    #[must_use]
    pub(crate) fn display_root(&self) -> &Path {
        &self.display_root
    }

    pub(crate) fn root_binding(
        &self,
    ) -> Result<RetainedProjectRootBinding, RetainedProjectTreeError> {
        let root =
            self.directories
                .first()
                .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
                    path: self.display_root.clone(),
                    reason: "retained project tree has no root directory witness".to_owned(),
                })?;
        Ok(RetainedProjectRootBinding {
            display_root: self.display_root.clone(),
            identity: identity(&root.metadata)?,
        })
    }

    pub(crate) fn validate_root_binding(
        &self,
        expected: &RetainedProjectRootBinding,
    ) -> Result<(), RetainedProjectTreeError> {
        let actual = self.root_binding()?;
        if &actual != expected {
            return Err(RetainedProjectTreeError::Identity {
                path: self.display_root.clone(),
                reason: "retained project tree differs from the lifecycle-bound project root"
                    .to_owned(),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_directory(
        &mut self,
        directory_index: usize,
        maximum_entries: usize,
        maximum_files: Option<usize>,
        maximum_bytes: u64,
        accepted_entries: &mut usize,
        accepted_files: &mut usize,
        accepted_bytes: &mut u64,
        digest_entries: &mut Vec<(String, String)>,
    ) -> Result<(), RetainedProjectTreeError> {
        let parent_handle = self.directories[directory_index]
            .handle
            .try_clone()
            .map_err(|error| io_error(&self.directory_display_path(directory_index), error))?;
        let parent_display = self.directory_display_path(directory_index);
        let parent_relative = self.directories[directory_index].relative_path.clone();
        validate_stable_directory(
            &parent_handle,
            &self.directories[directory_index].metadata,
            &parent_display,
        )?;
        let initial = included_entries(
            self.capture_policy,
            directory_index == 0,
            &parent_relative,
            &self.git_ignored_paths,
            read_directory_entries(&parent_handle, &parent_display)?,
        );
        let mut retained_entries = Vec::with_capacity(initial.len());
        for entry in &initial {
            *accepted_entries = (*accepted_entries).saturating_add(1);
            if *accepted_entries > maximum_entries {
                return Err(RetainedProjectTreeError::ResourceLimit {
                    resource: "snapshot entries",
                    maximum: maximum_entries as u64,
                });
            }
            let name = utf8_component(&entry.name, &parent_display)?;
            let relative_path = join_relative(&parent_relative, name);
            let display_path = self
                .display_root
                .join(relative_path_to_path(&relative_path));
            let handle = platform::open_child(&parent_handle, &entry.name)
                .map_err(|error| io_error(&display_path, error))?;
            let metadata = RetainedMetadata::capture(&handle, &display_path)?;
            if object_id(&metadata)? != entry.object_id {
                return Err(identity_error(
                    &display_path,
                    "directory entry changed before its handle was retained",
                ));
            }
            if is_directory(&metadata) {
                validate_directory_metadata(&metadata, &display_path)?;
                if relative_path.split('/').count() > MAX_RETAINED_PROJECT_DEPTH {
                    return Err(RetainedProjectTreeError::ResourceLimit {
                        resource: "snapshot depth",
                        maximum: MAX_RETAINED_PROJECT_DEPTH as u64,
                    });
                }
                let child_index = self.directories.len();
                self.directories.push(RetainedTreeDirectory {
                    handle,
                    metadata,
                    capability_nonce: project_capability_nonce(&display_path)?,
                    relative_path: relative_path.clone(),
                    parent: Some(directory_index),
                    name_in_parent: Some(entry.name.clone()),
                    entries: Vec::new(),
                });
                retained_entries.push(RetainedTreeEntry {
                    name: entry.name.clone(),
                    object_id: entry.object_id,
                    kind: RetainedTreeEntryKind::Directory,
                    witness_index: child_index,
                });
                digest_entries.push((relative_path, DIRECTORY_DIGEST_MARKER.to_owned()));
                self.capture_directory(
                    child_index,
                    maximum_entries,
                    maximum_files,
                    maximum_bytes,
                    accepted_entries,
                    accepted_files,
                    accepted_bytes,
                    digest_entries,
                )?;
            } else if is_regular_file(&metadata) {
                *accepted_files = (*accepted_files).saturating_add(1);
                if maximum_files.is_some_and(|maximum| *accepted_files > maximum) {
                    return Err(RetainedProjectTreeError::ResourceLimit {
                        resource: "snapshot files",
                        maximum: maximum_files.unwrap_or_default() as u64,
                    });
                }
                validate_file_metadata(&metadata, &display_path, self.file_alias_policy)?;
                let length = metadata.metadata.len();
                *accepted_bytes = (*accepted_bytes).saturating_add(length);
                if *accepted_bytes > maximum_bytes {
                    return Err(RetainedProjectTreeError::ResourceLimit {
                        resource: "snapshot bytes",
                        maximum: maximum_bytes,
                    });
                }
                let exact_bytes = read_retained_file(&handle, length, &display_path)?;
                validate_stable_file(&handle, &metadata, &display_path, self.file_alias_policy)?;
                let rebound = platform::open_child(&parent_handle, &entry.name)
                    .map_err(|error| io_error(&display_path, error))?;
                let rebound_metadata = RetainedMetadata::capture(&rebound, &display_path)?;
                validate_file_metadata(&rebound_metadata, &display_path, self.file_alias_policy)?;
                if !same_stable_file_metadata(&metadata, &rebound_metadata, self.file_alias_policy)
                {
                    return Err(identity_error(
                        &display_path,
                        "file namespace changed while its bytes were retained",
                    ));
                }
                let file_index = self.files.len();
                let content_digest = crate::sha256_content_hash(&exact_bytes);
                digest_entries.push((relative_path.clone(), content_digest.clone()));
                self.files.push(RetainedTreeFile {
                    handle,
                    metadata,
                    capability_nonce: project_capability_nonce(&display_path)?,
                    relative_path,
                    parent: directory_index,
                    name_in_parent: entry.name.clone(),
                    content_digest,
                    exact_bytes,
                });
                retained_entries.push(RetainedTreeEntry {
                    name: entry.name.clone(),
                    object_id: entry.object_id,
                    kind: RetainedTreeEntryKind::File,
                    witness_index: file_index,
                });
            } else {
                return Err(RetainedProjectTreeError::Identity {
                    path: display_path,
                    reason: "project snapshot contains a link, reparse point, or special object"
                        .to_owned(),
                });
            }
        }
        let after = included_entries(
            self.capture_policy,
            directory_index == 0,
            &parent_relative,
            &self.git_ignored_paths,
            read_directory_entries(&parent_handle, &parent_display)?,
        );
        if after != initial {
            return Err(identity_error(
                &parent_display,
                "directory entries changed during retained traversal",
            ));
        }
        validate_stable_directory(
            &parent_handle,
            &self.directories[directory_index].metadata,
            &parent_display,
        )?;
        self.directories[directory_index].entries = retained_entries;
        Ok(())
    }

    fn validate_ancestry(&self) -> Result<(), RetainedProjectTreeError> {
        for (index, directory) in self.ancestry.iter().enumerate() {
            let current = RetainedMetadata::capture(&directory.handle, &self.display_root)?;
            validate_directory_metadata(&current, &self.display_root)?;
            if identity(&current)? != directory.identity {
                return Err(identity_error(
                    &self.display_root,
                    "retained project ancestor handle changed identity",
                ));
            }
            if let Some(name) = directory.name_in_parent.as_deref() {
                let parent = &self.ancestry[index - 1];
                let rebound = platform::open_child(&parent.handle, name)
                    .map_err(|error| io_error(&self.display_root, error))?;
                let rebound_metadata = RetainedMetadata::capture(&rebound, &self.display_root)?;
                validate_directory_metadata(&rebound_metadata, &self.display_root)?;
                if identity(&rebound_metadata)? != directory.identity {
                    return Err(identity_error(
                        &self.display_root,
                        "project ancestor namespace no longer names the retained directory",
                    ));
                }
            }
        }
        let retained_root =
            self.ancestry
                .last()
                .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
                    path: self.display_root.clone(),
                    reason: "project ancestry is empty".to_owned(),
                })?;
        let retained_tree_root =
            RetainedMetadata::capture(&self.directories[0].handle, &self.display_root)?;
        if identity(&retained_tree_root)? != retained_root.identity {
            return Err(identity_error(
                &self.display_root,
                "retained project-tree root differs from its ancestor binding",
            ));
        }
        Ok(())
    }

    fn validate_namespace_pass(
        &self,
        file_alias_policy: RetainedFileAliasPolicy,
    ) -> Result<(), RetainedProjectTreeError> {
        for (directory_index, directory) in self.directories.iter().enumerate() {
            let display_path = self.directory_display_path(directory_index);
            validate_stable_directory(&directory.handle, &directory.metadata, &display_path)?;
            if let (Some(parent_index), Some(name)) =
                (directory.parent, directory.name_in_parent.as_deref())
            {
                let rebound = platform::open_child(&self.directories[parent_index].handle, name)
                    .map_err(|error| io_error(&display_path, error))?;
                let metadata = RetainedMetadata::capture(&rebound, &display_path)?;
                validate_directory_metadata(&metadata, &display_path)?;
                if !same_stable_metadata(&directory.metadata, &metadata) {
                    return Err(identity_error(
                        &display_path,
                        "directory component was substituted",
                    ));
                }
            }
            let actual = included_entries(
                self.capture_policy,
                directory_index == 0,
                &directory.relative_path,
                &self.git_ignored_paths,
                read_directory_entries(&directory.handle, &display_path)?,
            );
            let expected = directory
                .entries
                .iter()
                .map(|entry| DirectoryEntry {
                    name: entry.name.clone(),
                    object_id: entry.object_id,
                })
                .collect::<Vec<_>>();
            if actual != expected {
                return Err(identity_error(
                    &display_path,
                    "accepted directory entries are missing, additional, renamed, or substituted",
                ));
            }
            for entry in &directory.entries {
                let child_display = display_path.join(&entry.name);
                let rebound = platform::open_child(&directory.handle, &entry.name)
                    .map_err(|error| io_error(&child_display, error))?;
                let metadata = RetainedMetadata::capture(&rebound, &child_display)?;
                if object_id(&metadata)? != entry.object_id {
                    return Err(identity_error(
                        &child_display,
                        "accepted child identity differs from the retained entry",
                    ));
                }
                match entry.kind {
                    RetainedTreeEntryKind::Directory => {
                        validate_directory_metadata(&metadata, &child_display)?;
                        if !same_stable_metadata(
                            &self.directories[entry.witness_index].metadata,
                            &metadata,
                        ) {
                            return Err(identity_error(
                                &child_display,
                                "accepted directory was replaced or changed metadata",
                            ));
                        }
                    }
                    RetainedTreeEntryKind::File => {
                        validate_file_metadata(&metadata, &child_display, file_alias_policy)?;
                        if !same_stable_file_metadata(
                            &self.files[entry.witness_index].metadata,
                            &metadata,
                            file_alias_policy,
                        ) {
                            return Err(identity_error(
                                &child_display,
                                "accepted file was replaced or changed metadata",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn revalidate_files(
        &self,
        file_alias_policy: RetainedFileAliasPolicy,
    ) -> Result<Vec<(String, String)>, RetainedProjectTreeError> {
        const MAX_WORKERS: usize = 8;
        const MIN_FILES_PER_WORKER: usize = 64;

        let available_workers = std::thread::available_parallelism().map_or(1, usize::from);
        let useful_workers = self.files.len().div_ceil(MIN_FILES_PER_WORKER).max(1);
        let worker_count = available_workers.min(MAX_WORKERS).min(useful_workers);
        self.revalidate_files_with_worker_count(file_alias_policy, worker_count)
    }

    fn revalidate_files_with_worker_count(
        &self,
        file_alias_policy: RetainedFileAliasPolicy,
        worker_count: usize,
    ) -> Result<Vec<(String, String)>, RetainedProjectTreeError> {
        let worker_count = worker_count.max(1).min(self.files.len().max(1));
        let chunk_size = self.files.len().div_ceil(worker_count).max(1);
        std::thread::scope(|scope| {
            let workers = self
                .files
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|file| self.revalidate_file(file, file_alias_policy))
                            .collect::<Result<Vec<_>, _>>()
                    })
                })
                .collect::<Vec<_>>();
            let mut digest_entries = Vec::with_capacity(self.files.len());
            for worker in workers {
                let result = match worker.join() {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }?;
                digest_entries.extend(result);
            }
            Ok(digest_entries)
        })
    }

    fn revalidate_file(
        &self,
        file: &RetainedTreeFile,
        file_alias_policy: RetainedFileAliasPolicy,
    ) -> Result<(String, String), RetainedProjectTreeError> {
        let display_path = self
            .display_root
            .join(relative_path_to_path(&file.relative_path));
        validate_stable_file(
            &file.handle,
            &file.metadata,
            &display_path,
            file_alias_policy,
        )?;
        let parent = &self.directories[file.parent];
        let rebound = platform::open_child(&parent.handle, &file.name_in_parent)
            .map_err(|error| io_error(&display_path, error))?;
        let rebound_metadata = RetainedMetadata::capture(&rebound, &display_path)?;
        validate_file_metadata(&rebound_metadata, &display_path, file_alias_policy)?;
        if !same_stable_file_metadata(&file.metadata, &rebound_metadata, file_alias_policy) {
            return Err(identity_error(
                &display_path,
                "file namespace no longer names the retained file",
            ));
        }
        let bytes = read_retained_file(
            &file.handle,
            u64::try_from(file.exact_bytes.len()).unwrap_or(u64::MAX),
            &display_path,
        )?;
        if bytes != file.exact_bytes {
            return Err(identity_error(
                &display_path,
                "retained file bytes differ from the admitted snapshot",
            ));
        }
        validate_stable_file(
            &file.handle,
            &file.metadata,
            &display_path,
            file_alias_policy,
        )?;
        let rebound_after = platform::open_child(&parent.handle, &file.name_in_parent)
            .map_err(|error| io_error(&display_path, error))?;
        let rebound_after_metadata = RetainedMetadata::capture(&rebound_after, &display_path)?;
        if !same_stable_file_metadata(&file.metadata, &rebound_after_metadata, file_alias_policy) {
            return Err(identity_error(
                &display_path,
                "file namespace changed while rehashing retained bytes",
            ));
        }
        Ok((file.relative_path.clone(), file.content_digest.clone()))
    }

    fn directory_display_path(&self, index: usize) -> PathBuf {
        let relative = &self.directories[index].relative_path;
        if relative.is_empty() {
            self.display_root.clone()
        } else {
            self.display_root.join(relative_path_to_path(relative))
        }
    }
}

fn open_ancestry(
    project_root: &Path,
) -> Result<Vec<RetainedAncestorDirectory>, RetainedProjectTreeError> {
    let (anchor, components) = split_absolute(project_root)?;
    if components.is_empty() {
        return Err(RetainedProjectTreeError::InvalidRoot {
            path: project_root.to_path_buf(),
            reason: "the filesystem anchor cannot be used as a project root".to_owned(),
        });
    }
    let anchor_handle = platform::open_root(&anchor).map_err(|error| io_error(&anchor, error))?;
    let anchor_metadata = RetainedMetadata::capture(&anchor_handle, &anchor)?;
    validate_directory_metadata(&anchor_metadata, &anchor)?;
    let mut ancestry = vec![RetainedAncestorDirectory {
        handle: anchor_handle,
        identity: identity(&anchor_metadata)?,
        capability_nonce: project_capability_nonce(&anchor)?,
        name_in_parent: None,
    }];
    let mut display = anchor;
    for component in components {
        display.push(&component);
        let parent = ancestry
            .last()
            .expect("filesystem anchor initializes project ancestry");
        let handle = platform::open_child(&parent.handle, &component)
            .map_err(|error| io_error(&display, error))?;
        let metadata = RetainedMetadata::capture(&handle, &display)?;
        validate_directory_metadata(&metadata, &display)?;
        ancestry.push(RetainedAncestorDirectory {
            handle,
            identity: identity(&metadata)?,
            capability_nonce: project_capability_nonce(&display)?,
            name_in_parent: Some(component),
        });
    }
    Ok(ancestry)
}

fn absolute_lexical_path(path: &Path) -> Result<PathBuf, RetainedProjectTreeError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| io_error(path, error))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(RetainedProjectTreeError::InvalidRoot {
                        path: absolute,
                        reason: "project root escapes the filesystem anchor".to_owned(),
                    });
                }
            }
        }
    }
    if !normalized.is_absolute() {
        return Err(RetainedProjectTreeError::InvalidRoot {
            path: normalized,
            reason: "project root is not absolute after lexical normalization".to_owned(),
        });
    }
    Ok(normalized)
}

fn split_absolute(path: &Path) -> Result<(PathBuf, Vec<OsString>), RetainedProjectTreeError> {
    let mut anchor = PathBuf::new();
    let mut components = Vec::new();
    let mut rooted = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
            Component::RootDir => {
                anchor.push(Path::new(std::path::MAIN_SEPARATOR_STR));
                rooted = true;
            }
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::CurDir | Component::ParentDir => {
                return Err(RetainedProjectTreeError::InvalidRoot {
                    path: path.to_path_buf(),
                    reason: "project root is not lexically normalized".to_owned(),
                });
            }
        }
    }
    if !rooted || !anchor.is_absolute() {
        return Err(RetainedProjectTreeError::InvalidRoot {
            path: path.to_path_buf(),
            reason: "project root has no filesystem anchor".to_owned(),
        });
    }
    Ok((anchor, components))
}

fn read_directory_entries(
    directory: &File,
    display_path: &Path,
) -> Result<Vec<DirectoryEntry>, RetainedProjectTreeError> {
    let mut entries =
        platform::read_entries(directory).map_err(|error| io_error(display_path, error))?;
    for entry in &entries {
        validate_component(&entry.name, display_path)?;
        if entry.object_id == 0 {
            return Err(identity_error(
                &display_path.join(&entry.name),
                "directory entry has no stable object identity",
            ));
        }
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    if entries.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(identity_error(
            display_path,
            "directory enumeration returned duplicate names",
        ));
    }
    Ok(entries)
}

fn discover_git_ignored_paths(root: &Path) -> Result<BTreeSet<String>, RetainedProjectTreeError> {
    let git_entry = root.join(".git");
    let metadata = match std::fs::symlink_metadata(&git_entry) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => return Err(io_error(&git_entry, error)),
    };
    if metadata.file_type().is_symlink() || !(metadata.is_dir() || metadata.is_file()) {
        return Err(RetainedProjectTreeError::Identity {
            path: git_entry,
            reason: "Git control entry must be a regular file or directory, never a link or special object"
                .to_owned(),
        });
    }

    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("--no-replace-objects")
        .arg("--no-pager")
        .args(["-c", "core.fsmonitor=false"])
        .args(["-c", "core.untrackedCache=false"])
        .args(["-c", "submodule.recurse=false"])
        .arg("-c")
        .arg(format!("core.excludesFile={null_device}"))
        .args([
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "--no-empty-directory",
            "-z",
        ])
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_PAGER", "")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_NAMESPACE")
        .env_remove("GIT_PREFIX")
        .env_remove("GIT_CONFIG")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .stdin(Stdio::null());
    let output = run_bounded_git_command(&mut command, root)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: format!(
                "Git ignored-path discovery failed with status {:?}: {}",
                output.status.code(),
                stderr.trim()
            ),
        });
    }
    if output.stdout.len() > MAX_GIT_IGNORED_PATH_OUTPUT_BYTES {
        return Err(RetainedProjectTreeError::ResourceLimit {
            resource: "Git ignored-path output bytes",
            maximum: MAX_GIT_IGNORED_PATH_OUTPUT_BYTES as u64,
        });
    }

    let mut paths = BTreeSet::new();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
    {
        let value =
            std::str::from_utf8(raw).map_err(|error| RetainedProjectTreeError::InvalidRoot {
                path: root.to_path_buf(),
                reason: format!("Git ignored path is not UTF-8: {error}"),
            })?;
        let value = value.trim_end_matches('/');
        let path = Path::new(value);
        if value.is_empty()
            || path.is_absolute()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(RetainedProjectTreeError::InvalidRoot {
                path: root.join(path),
                reason: "Git returned a non-normalized ignored project path".to_owned(),
            });
        }
        paths.insert(value.to_owned());
    }
    Ok(paths)
}
fn run_bounded_git_command(
    command: &mut Command,
    root: &Path,
) -> Result<std::process::Output, RetainedProjectTreeError> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: format!("Git ignored-path discovery could not start: {error}"),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: "Git ignored-path discovery has no stdout pipe".to_owned(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: "Git ignored-path discovery has no stderr pipe".to_owned(),
        })?;
    let stdout_reader =
        thread::spawn(move || read_bounded_stream(stdout, MAX_GIT_IGNORED_PATH_OUTPUT_BYTES));
    let stderr_reader =
        thread::spawn(move || read_bounded_stream(stderr, MAX_GIT_DIAGNOSTIC_OUTPUT_BYTES));
    let status = child.wait().map_err(|error| RetainedProjectTreeError::Io {
        path: root.to_path_buf(),
        reason: format!("Git ignored-path discovery wait failed: {error}"),
    })?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: "Git ignored-path stdout reader panicked".to_owned(),
        })?
        .map_err(|error| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: format!("Git ignored-path stdout read failed: {error}"),
        })?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: "Git ignored-path stderr reader panicked".to_owned(),
        })?
        .map_err(|error| RetainedProjectTreeError::Io {
            path: root.to_path_buf(),
            reason: format!("Git ignored-path stderr read failed: {error}"),
        })?;
    if stdout.len() > MAX_GIT_IGNORED_PATH_OUTPUT_BYTES {
        return Err(RetainedProjectTreeError::ResourceLimit {
            resource: "Git ignored-path output bytes",
            maximum: MAX_GIT_IGNORED_PATH_OUTPUT_BYTES as u64,
        });
    }
    if stderr.len() > MAX_GIT_DIAGNOSTIC_OUTPUT_BYTES {
        return Err(RetainedProjectTreeError::ResourceLimit {
            resource: "Git diagnostic output bytes",
            maximum: MAX_GIT_DIAGNOSTIC_OUTPUT_BYTES as u64,
        });
    }
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn read_bounded_stream(mut stream: impl io::Read, maximum_bytes: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .by_ref()
        .take(maximum_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn included_entries(
    capture_policy: RetainedProjectCapturePolicy,
    root: bool,
    parent_relative: &str,
    git_ignored_paths: &BTreeSet<String>,
    entries: Vec<DirectoryEntry>,
) -> Vec<DirectoryEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            let Some(name) = entry.name.to_str() else {
                return true;
            };
            if root && !capture_policy.includes_root_name(name) {
                return false;
            }
            !git_ignored_paths.contains(&join_relative(parent_relative, name))
        })
        .collect()
}

fn validate_component(name: &OsStr, parent: &Path) -> Result<(), RetainedProjectTreeError> {
    let mut components = Path::new(name).components();
    let valid = matches!(components.next(), Some(Component::Normal(value)) if value == name)
        && components.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(RetainedProjectTreeError::InvalidRoot {
            path: parent.join(name),
            reason: "directory enumeration returned a non-child component".to_owned(),
        })
    }
}

fn utf8_component<'name>(
    name: &'name OsStr,
    parent: &Path,
) -> Result<&'name str, RetainedProjectTreeError> {
    name.to_str()
        .ok_or_else(|| RetainedProjectTreeError::InvalidRoot {
            path: parent.join(name),
            reason: "project snapshot names must be UTF-8".to_owned(),
        })
}

fn join_relative(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn relative_path_to_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

fn project_anchor_entry_directory(relative: &str) -> String {
    let digest = crate::sha256_content_hash(relative.as_bytes());
    format!(
        "file-{}",
        digest.strip_prefix("sha256:").unwrap_or(digest.as_str())
    )
}

fn read_retained_file(
    file: &File,
    expected_length: u64,
    display_path: &Path,
) -> Result<Vec<u8>, RetainedProjectTreeError> {
    let before = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if before.len() != expected_length {
        return Err(identity_error(
            display_path,
            "retained file length differs from its admitted length",
        ));
    }
    let mut reader = file
        .try_clone()
        .map_err(|error| io_error(display_path, error))?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(display_path, error))?;
    let mut bytes = Vec::with_capacity(usize::try_from(expected_length).unwrap_or(0));
    std::io::Read::by_ref(&mut reader)
        .take(expected_length.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(display_path, error))?;
    let after = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected_length
        || after.len() != expected_length
    {
        return Err(identity_error(
            display_path,
            "retained file changed while reading exact bytes",
        ));
    }
    Ok(bytes)
}

fn digest_entries_digest(entries: &[(String, String)]) -> Result<String, RetainedProjectTreeError> {
    let bytes = serde_json_canonicalizer::to_vec(&entries).map_err(|error| {
        RetainedProjectTreeError::InvalidRoot {
            path: PathBuf::from("<retained-project-tree>"),
            reason: format!("project snapshot canonicalization failed: {error}"),
        }
    })?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Derive the same sorted path/content digest used by a retained project's
/// regular-file snapshot.
///
/// Promotion planning uses this to bind the exact logical result before any
/// write occurs. Callers must provide one sorted, duplicate-free projection.
pub fn retained_regular_file_projection_digest(
    entries: &[(String, String)],
) -> Result<String, RetainedProjectTreeError> {
    if entries.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(RetainedProjectTreeError::InvalidRoot {
            path: PathBuf::from("<retained-project-tree-projection>"),
            reason: "regular-file projection must be strictly path-sorted and duplicate-free"
                .to_owned(),
        });
    }
    digest_entries_digest(entries)
}

fn project_capability_nonce(path: &Path) -> Result<String, RetainedProjectTreeError> {
    let mut nonce = [0_u8; PROJECT_CAPABILITY_NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|error| RetainedProjectTreeError::Io {
        path: path.to_path_buf(),
        reason: format!("retained project capability nonce generation failed: {error}"),
    })?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(PROJECT_CAPABILITY_NONCE_BYTES * 2);
    for byte in nonce {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

#[cfg(unix)]
fn os_component_digest(value: &OsStr) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    crate::sha256_content_hash(value.as_bytes())
}

#[cfg(windows)]
fn os_component_digest(value: &OsStr) -> String {
    use std::os::windows::ffi::OsStrExt as _;
    let bytes = value
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    crate::sha256_content_hash(&bytes)
}

#[cfg(not(any(unix, windows)))]
fn os_component_digest(value: &OsStr) -> String {
    crate::sha256_content_hash(value.to_string_lossy().as_bytes())
}

fn os_path_digest(path: &Path) -> String {
    os_component_digest(path.as_os_str())
}

fn validate_stable_directory(
    handle: &File,
    expected: &RetainedMetadata,
    display_path: &Path,
) -> Result<(), RetainedProjectTreeError> {
    let current = RetainedMetadata::capture(handle, display_path)?;
    validate_directory_metadata(&current, display_path)?;
    if same_stable_metadata(expected, &current) {
        Ok(())
    } else {
        Err(identity_error(
            display_path,
            "retained directory identity or stable metadata changed",
        ))
    }
}

fn validate_stable_file(
    handle: &File,
    expected: &RetainedMetadata,
    display_path: &Path,
    file_alias_policy: RetainedFileAliasPolicy,
) -> Result<(), RetainedProjectTreeError> {
    let current = RetainedMetadata::capture(handle, display_path)?;
    validate_file_metadata(&current, display_path, file_alias_policy)?;
    if same_stable_file_metadata(expected, &current, file_alias_policy) {
        Ok(())
    } else {
        Err(identity_error(
            display_path,
            "retained file identity or stable metadata changed",
        ))
    }
}

fn validate_directory_metadata(
    metadata: &RetainedMetadata,
    display_path: &Path,
) -> Result<(), RetainedProjectTreeError> {
    if is_directory(metadata) && !is_reparse(metadata) {
        Ok(())
    } else {
        Err(RetainedProjectTreeError::Identity {
            path: display_path.to_path_buf(),
            reason: "project ancestor must be a no-follow non-reparse directory".to_owned(),
        })
    }
}

fn validate_file_metadata(
    metadata: &RetainedMetadata,
    display_path: &Path,
    file_alias_policy: RetainedFileAliasPolicy,
) -> Result<(), RetainedProjectTreeError> {
    let links = hard_link_count(metadata);
    if is_regular_file(metadata)
        && !is_reparse(metadata)
        && (links == 1 || (file_alias_policy.allows_multiple_links() && links > 1))
    {
        Ok(())
    } else {
        Err(RetainedProjectTreeError::Identity {
            path: display_path.to_path_buf(),
            reason: if file_alias_policy.allows_multiple_links() {
                "project file must be a no-follow regular file with at least one live name"
                    .to_owned()
            } else {
                "project file must be one no-follow single-link regular file".to_owned()
            },
        })
    }
}

#[cfg(not(windows))]
fn is_directory(metadata: &RetainedMetadata) -> bool {
    metadata.metadata.is_dir() && !metadata.metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_directory(metadata: &RetainedMetadata) -> bool {
    const FILE_ATTRIBUTE_DIRECTORY: u64 = 0x0000_0010;
    metadata.file_information.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
}

#[cfg(not(windows))]
fn is_regular_file(metadata: &RetainedMetadata) -> bool {
    metadata.metadata.is_file() && !metadata.metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_regular_file(metadata: &RetainedMetadata) -> bool {
    const FILE_ATTRIBUTE_DIRECTORY: u64 = 0x0000_0010;
    metadata.file_information.file_attributes & FILE_ATTRIBUTE_DIRECTORY == 0
}

#[cfg(windows)]
fn is_reparse(metadata: &RetainedMetadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u64 = 0x0000_0400;
    metadata.file_information.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &RetainedMetadata) -> bool {
    false
}

#[cfg(unix)]
fn hard_link_count(metadata: &RetainedMetadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    metadata.metadata.nlink()
}

#[cfg(windows)]
fn hard_link_count(metadata: &RetainedMetadata) -> u64 {
    metadata.file_information.number_of_links
}

#[cfg(not(any(unix, windows)))]
fn hard_link_count(_metadata: &RetainedMetadata) -> u64 {
    0
}

#[cfg(unix)]
fn identity(metadata: &RetainedMetadata) -> Result<FileIdentity, RetainedProjectTreeError> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(FileIdentity {
        platform: PlatformIdentity {
            device: metadata.metadata.dev(),
            inode: metadata.metadata.ino(),
        },
    })
}

#[cfg(windows)]
fn identity(metadata: &RetainedMetadata) -> Result<FileIdentity, RetainedProjectTreeError> {
    Ok(FileIdentity {
        platform: PlatformIdentity {
            volume: metadata.file_information.volume_serial_number,
            index: metadata.file_information.file_index,
        },
    })
}

#[cfg(not(any(unix, windows)))]
fn identity(_metadata: &RetainedMetadata) -> Result<FileIdentity, RetainedProjectTreeError> {
    Err(RetainedProjectTreeError::InvalidRoot {
        path: PathBuf::from("<retained-project-tree>"),
        reason: "retained project identity is unsupported on this platform".to_owned(),
    })
}

#[cfg(unix)]
fn object_id(metadata: &RetainedMetadata) -> Result<u64, RetainedProjectTreeError> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(metadata.metadata.ino())
}

#[cfg(windows)]
fn object_id(metadata: &RetainedMetadata) -> Result<u64, RetainedProjectTreeError> {
    Ok(metadata.file_information.file_index)
}

#[cfg(not(any(unix, windows)))]
fn object_id(_metadata: &RetainedMetadata) -> Result<u64, RetainedProjectTreeError> {
    Err(RetainedProjectTreeError::InvalidRoot {
        path: PathBuf::from("<retained-project-tree>"),
        reason: "retained project identity is unsupported on this platform".to_owned(),
    })
}

#[cfg(unix)]
fn same_stable_metadata(left: &RetainedMetadata, right: &RetainedMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let left = &left.metadata;
    let right = &right.metadata;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.file_type() == right.file_type()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn same_stable_metadata(left: &RetainedMetadata, right: &RetainedMetadata) -> bool {
    left.file_information.volume_serial_number == right.file_information.volume_serial_number
        && left.file_information.file_index == right.file_information.file_index
        && left.file_information.file_attributes == right.file_information.file_attributes
        && left.file_information.creation_time == right.file_information.creation_time
        && left.file_information.number_of_links == right.file_information.number_of_links
        && left.file_information.file_size == right.file_information.file_size
        && left.file_information.last_write_time == right.file_information.last_write_time
        && !is_reparse(left)
        && !is_reparse(right)
}

#[cfg(not(any(unix, windows)))]
fn same_stable_metadata(_left: &RetainedMetadata, _right: &RetainedMetadata) -> bool {
    false
}

#[cfg(unix)]
fn same_stable_file_metadata(
    left: &RetainedMetadata,
    right: &RetainedMetadata,
    file_alias_policy: RetainedFileAliasPolicy,
) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let left = &left.metadata;
    let right = &right.metadata;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.file_type() == right.file_type()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && (file_alias_policy.allows_alias_metadata_changes()
            || (left.nlink() == right.nlink()
                && left.ctime() == right.ctime()
                && left.ctime_nsec() == right.ctime_nsec()))
}

#[cfg(windows)]
fn same_stable_file_metadata(
    left: &RetainedMetadata,
    right: &RetainedMetadata,
    file_alias_policy: RetainedFileAliasPolicy,
) -> bool {
    left.file_information.volume_serial_number == right.file_information.volume_serial_number
        && left.file_information.file_index == right.file_information.file_index
        && left.file_information.file_attributes == right.file_information.file_attributes
        && left.file_information.creation_time == right.file_information.creation_time
        && left.file_information.file_size == right.file_information.file_size
        && left.file_information.last_write_time == right.file_information.last_write_time
        && (file_alias_policy.allows_alias_metadata_changes()
            || left.file_information.number_of_links == right.file_information.number_of_links)
        && !is_reparse(left)
        && !is_reparse(right)
}

#[cfg(not(any(unix, windows)))]
fn same_stable_file_metadata(
    _left: &RetainedMetadata,
    _right: &RetainedMetadata,
    _file_alias_policy: RetainedFileAliasPolicy,
) -> bool {
    false
}

#[cfg(unix)]
fn same_exact_file_and_admitted_metadata_after_mutation(
    before: &RetainedMetadata,
    after: &RetainedMetadata,
    file_alias_policy: RetainedFileAliasPolicy,
) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let before = &before.metadata;
    let after = &after.metadata;
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.file_type() == after.file_type()
        && before.mode() == after.mode()
        && before.uid() == after.uid()
        && before.gid() == after.gid()
        && (file_alias_policy.allows_alias_metadata_changes() || before.nlink() == after.nlink())
}

#[cfg(not(unix))]
fn same_exact_file_and_admitted_metadata_after_mutation(
    _before: &RetainedMetadata,
    _after: &RetainedMetadata,
    _file_alias_policy: RetainedFileAliasPolicy,
) -> bool {
    false
}

#[cfg(unix)]
fn promotion_metadata_fingerprint(metadata: &RetainedMetadata) -> String {
    use std::os::unix::fs::MetadataExt as _;
    format!(
        "unix:mode={:o};uid={};gid={}",
        metadata.metadata.mode(),
        metadata.metadata.uid(),
        metadata.metadata.gid()
    )
}

#[cfg(windows)]
fn promotion_metadata_fingerprint(metadata: &RetainedMetadata) -> String {
    format!(
        "windows:attributes={:08x}",
        metadata.file_information.file_attributes
    )
}

#[cfg(not(any(unix, windows)))]
fn promotion_metadata_fingerprint(metadata: &RetainedMetadata) -> String {
    format!(
        "portable:readonly={}",
        metadata.metadata.permissions().readonly()
    )
}

fn io_error(path: &Path, error: io::Error) -> RetainedProjectTreeError {
    RetainedProjectTreeError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

fn identity_error(path: &Path, reason: &str) -> RetainedProjectTreeError {
    RetainedProjectTreeError::Identity {
        path: path.to_path_buf(),
        reason: reason.to_owned(),
    }
}

#[cfg(unix)]
mod platform {
    use super::{io, DirectoryEntry, File, OsStr, OsString, Path};
    use std::os::unix::ffi::OsStringExt as _;

    pub(super) fn open_root(path: &Path) -> io::Result<File> {
        use rustix::fs::{open, Mode, OFlags};
        let descriptor = open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(File::from(descriptor))
    }

    pub(super) fn open_child(parent: &File, name: &OsStr) -> io::Result<File> {
        use rustix::fs::{openat, Mode, OFlags};
        let descriptor = openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(File::from(descriptor))
    }

    pub(super) fn open_child_for_mutation(parent: &File, name: &OsStr) -> io::Result<File> {
        use rustix::fs::{openat, Mode, OFlags};
        let descriptor = openat(
            parent,
            name,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(File::from(descriptor))
    }

    pub(super) fn read_entries(directory: &File) -> io::Result<Vec<DirectoryEntry>> {
        let mut stream = rustix::fs::Dir::read_from(directory).map_err(io::Error::from)?;
        let mut entries = Vec::new();
        for entry in &mut stream {
            let entry = entry.map_err(io::Error::from)?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." {
                entries.push(DirectoryEntry {
                    name: OsString::from_vec(name.to_vec()),
                    object_id: entry.ino(),
                });
            }
        }
        Ok(entries)
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, RawHandle};

    type Handle = *mut std::ffi::c_void;
    type NtStatus = i32;
    const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const GENERIC_READ: u32 = 0x8000_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_OPEN: u32 = 1;
    const OPEN_EXISTING: u32 = 3;
    const FILE_DIRECTORY_FILE: u32 = 0x1;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: Handle,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut std::ffi::c_void,
        security_quality_of_service: *mut std::ffi::c_void,
    }

    #[repr(C)]
    union IoStatusValue {
        status: NtStatus,
        pointer: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        value: IoStatusValue,
        information: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *const std::ffi::c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: Handle,
        ) -> Handle;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            file_handle: *mut Handle,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut std::ffi::c_void,
            ea_length: u32,
        ) -> NtStatus;
        fn RtlNtStatusToDosError(status: NtStatus) -> u32;
    }

    pub(super) fn open_root(path: &Path) -> io::Result<File> {
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: `wide` is NUL-terminated and live for the call.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: successful CreateFileW returned one newly-owned handle.
            Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
        }
    }

    #[allow(clippy::cast_possible_wrap)] // RtlNtStatusToDosError returns a Win32 DOS error code (u32) that is documented to fit i32 for io::Error::from_raw_os_error.
    pub(super) fn open_child(parent: &File, name: &OsStr) -> io::Result<File> {
        let mut wide = name.encode_wide().collect::<Vec<_>>();
        let byte_len = wide
            .len()
            .checked_mul(2)
            .and_then(|length| u16::try_from(length).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "project name too long"))?;
        let mut name = UnicodeString {
            length: byte_len,
            maximum_length: byte_len,
            buffer: wide.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: u32::try_from(std::mem::size_of::<ObjectAttributes>())
                .expect("OBJECT_ATTRIBUTES size"),
            root_directory: parent.as_raw_handle().cast(),
            object_name: std::ptr::from_mut(&mut name),
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut io_status = IoStatusBlock {
            value: IoStatusValue { status: 0 },
            information: 0,
        };
        let mut handle: Handle = std::ptr::null_mut();
        // SAFETY: all pointers reference initialized storage for the call.
        let status = unsafe {
            NtCreateFile(
                std::ptr::from_mut(&mut handle),
                GENERIC_READ | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                std::ptr::from_mut(&mut attributes),
                std::ptr::from_mut(&mut io_status),
                std::ptr::null_mut(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_OPEN,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            // SAFETY: pure NTSTATUS conversion.
            Err(io::Error::from_raw_os_error(
                unsafe { RtlNtStatusToDosError(status) } as i32,
            ))
        } else {
            // SAFETY: successful NtCreateFile returned one newly-owned handle.
            Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
        }
    }

    pub(super) fn open_child_for_mutation(_parent: &File, _name: &OsStr) -> io::Result<File> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exact metadata-preserving promotion mutation is not admitted on Windows yet",
        ))
    }

    #[allow(clippy::cast_ptr_alignment)]
    // the directory-query buffer is file-read and intentionally unaligned; the cast is paired with read_unaligned.
    #[allow(clippy::cast_sign_loss)] // FILE_ID_BOTH_DIR_INFO.FileId is an i64 whose bit pattern is treated as an opaque Windows FILE_ID (unsigned semantics).
    pub(super) fn read_entries(directory: &File) -> io::Result<Vec<DirectoryEntry>> {
        use windows_sys::Win32::Storage::FileSystem::{
            FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo, GetFileInformationByHandleEx,
            FILE_ID_BOTH_DIR_INFO,
        };

        const ERROR_NO_MORE_FILES: i32 = 18;
        const BUFFER_BYTES: usize = 64 * 1024;
        let mut restart = true;
        let mut entries = Vec::new();
        loop {
            let mut buffer = vec![0_u8; BUFFER_BYTES];
            let class = if restart {
                FileIdBothDirectoryRestartInfo
            } else {
                FileIdBothDirectoryInfo
            };
            // SAFETY: the retained directory handle and writable buffer are live.
            let result = unsafe {
                GetFileInformationByHandleEx(
                    directory.as_raw_handle().cast(),
                    class,
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(buffer.len()).expect("directory query buffer length"),
                )
            };
            if result == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                    return Ok(entries);
                }
                return Err(error);
            }
            restart = false;
            let mut offset = 0usize;
            loop {
                let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if offset
                    .checked_add(header)
                    .is_none_or(|end| end > buffer.len())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "truncated retained project directory entry",
                    ));
                }
                let entry_ptr =
                    unsafe { buffer.as_ptr().add(offset) }.cast::<FILE_ID_BOTH_DIR_INFO>();
                // SAFETY: the fixed header was bounds-checked; the buffer may be unaligned.
                let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };
                let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name length overflow")
                })?;
                if name_bytes % 2 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory entry has an odd UTF-16 byte length",
                    ));
                }
                let name_start = offset.checked_add(header).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name offset overflow")
                })?;
                let name_end = name_start.checked_add(name_bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name length overflow")
                })?;
                if name_end > buffer.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory entry name exceeds query buffer",
                    ));
                }
                let wide = buffer[name_start..name_end]
                    .chunks_exact(2)
                    .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>();
                let name = OsString::from_wide(&wide);
                if name != "." && name != ".." {
                    entries.push(DirectoryEntry {
                        name,
                        object_id: entry.FileId as u64,
                    });
                }
                if entry.NextEntryOffset == 0 {
                    break;
                }
                offset = offset
                    .checked_add(usize::try_from(entry.NextEntryOffset).expect("u32 fits usize"))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "directory offset overflow")
                    })?;
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::*;

    fn unsupported<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "retained project-tree access is unsupported on this platform",
        ))
    }

    pub(super) fn open_root(_path: &Path) -> io::Result<File> {
        unsupported()
    }

    pub(super) fn open_child(_parent: &File, _name: &OsStr) -> io::Result<File> {
        unsupported()
    }

    pub(super) fn open_child_for_mutation(_parent: &File, _name: &OsStr) -> io::Result<File> {
        unsupported()
    }

    pub(super) fn read_entries(_directory: &File) -> io::Result<Vec<DirectoryEntry>> {
        unsupported()
    }
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn project_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "forge-retained-project-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
    #[cfg(unix)]
    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("git command starts");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn parallel_file_revalidation_checks_every_retained_handle() {
        let root = project_root("parallel-file-revalidation");
        fs::create_dir_all(&root).unwrap();
        for index in 0..130 {
            fs::write(root.join(format!("file-{index:03}.txt")), b"before").unwrap();
        }
        let retained = RetainedProjectTree::capture(&root, 256, 4096).unwrap();
        let entries = retained
            .revalidate_files_with_worker_count(retained.file_alias_policy, 4)
            .expect("unchanged retained files revalidate in parallel");
        assert_eq!(entries.len(), 130);

        fs::write(root.join("file-129.txt"), b"after!").unwrap();
        assert!(retained
            .revalidate_files_with_worker_count(retained.file_alias_policy, 4)
            .is_err());

        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn regular_file_snapshot_digest_preserves_file_only_contract_and_exact_limit() {
        let root = project_root("regular-file-digest");
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), b"root").unwrap();
        fs::write(root.join("src/lib.rs"), b"exact").unwrap();

        let retained = RetainedProjectTree::capture(&root, 16, 9).unwrap();
        let mut entries = vec![
            ("README.md".to_owned(), crate::sha256_content_hash(b"root")),
            (
                "src/lib.rs".to_owned(),
                crate::sha256_content_hash(b"exact"),
            ),
        ];
        entries.sort();
        assert_eq!(
            retained.regular_file_snapshot_digest(),
            digest_entries_digest(&entries).unwrap()
        );
        retained.revalidate().unwrap();
        drop(retained);

        assert!(matches!(
            RetainedProjectTree::capture(&root, 16, 8),
            Err(RetainedProjectTreeError::ResourceLimit {
                resource: "snapshot bytes",
                maximum: 8,
            })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generic_capture_governs_top_level_local_content() {
        let root = project_root("generic-local-root");
        fs::create_dir_all(root.join(".local")).unwrap();
        fs::write(root.join("README.md"), b"governed\n").unwrap();
        fs::write(root.join(".local/resume.json"), b"first local report\n").unwrap();

        let first = RetainedProjectTree::capture(&root, 16, 1024).unwrap();
        assert_eq!(
            first
                .exact_regular_file_bytes(".local/resume.json")
                .unwrap()
                .as_deref(),
            Some(b"first local report\n".as_slice())
        );
        let first_digest = first.regular_file_snapshot_digest().to_owned();
        drop(first);

        fs::write(root.join(".local/resume.json"), b"updated local report\n").unwrap();
        let second = RetainedProjectTree::capture(&root, 16, 1024).unwrap();
        assert_ne!(second.regular_file_snapshot_digest(), first_digest);

        drop(second);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workflow_capture_ignores_preexisting_top_level_local_content() {
        let root = project_root("workflow-local-root");
        fs::create_dir_all(root.join(".local")).unwrap();
        fs::write(root.join("README.md"), b"governed\n").unwrap();
        fs::write(root.join(".local/resume.json"), b"first local report\n").unwrap();

        let retained = RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
            &root, 16, 16, 1024,
        )
        .unwrap();
        let digest = retained.regular_file_snapshot_digest().to_owned();
        assert_eq!(
            retained
                .exact_regular_file_bytes(".local/resume.json")
                .unwrap(),
            None,
            "top-level local state must not become workflow evidence"
        );

        fs::write(root.join(".local/resume.json"), b"updated local report\n").unwrap();
        retained
            .revalidate()
            .expect("updating pre-existing local state must not stale workflow evidence");
        let recaptured =
            RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
                &root, 16, 16, 1024,
            )
            .unwrap();
        assert_eq!(recaptured.regular_file_snapshot_digest(), digest);

        drop(recaptured);
        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn broader_domain_capture_projects_the_exact_workflow_regular_file_view() {
        let root = project_root("shared-domain-workflow-view");
        fs::create_dir_all(root.join(".local")).unwrap();
        fs::create_dir_all(root.join("src/.local")).unwrap();
        fs::write(root.join("README.md"), b"governed\n").unwrap();
        fs::write(root.join(".local/resume.json"), b"local report\n").unwrap();
        fs::write(root.join("src/.local/config.json"), b"nested governed\n").unwrap();

        let shared =
            RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 32, 4096)
                .unwrap();
        let workflow = RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
            &root, 32, 32, 4096,
        )
        .unwrap();

        assert_eq!(
            shared.workflow_regular_file_snapshot_digest().unwrap(),
            workflow.regular_file_snapshot_digest()
        );
        let shared_paths = shared
            .workflow_regular_file_observations()
            .into_iter()
            .map(|observation| observation.relative_path)
            .collect::<Vec<_>>();
        assert_eq!(
            shared_paths,
            vec!["README.md".to_owned(), "src/.local/config.json".to_owned()]
        );
        assert!(shared
            .regular_file_observations()
            .iter()
            .any(|observation| observation.relative_path == ".local/resume.json"));

        drop(workflow);
        drop(shared);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn workflow_capture_excludes_git_ignored_symlinks_but_never_tracked_files() {
        use std::os::unix::fs::symlink;

        let root = project_root("workflow-git-ignored-symlink");
        let cache_target = project_root("workflow-git-ignored-cache-target");
        fs::create_dir_all(root.join("fuzz")).unwrap();
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::create_dir_all(&cache_target).unwrap();
        fs::write(root.join(".gitignore"), b"fuzz/target\ngenerated/\n").unwrap();
        fs::write(root.join("README.md"), b"governed\n").unwrap();
        fs::write(root.join("generated/keep.txt"), b"tracked despite ignore\n").unwrap();
        fs::write(root.join("generated/cache.bin"), b"ignored cache\n").unwrap();
        symlink(&cache_target, root.join("fuzz/target")).unwrap();

        git(&root, &["init", "-q", "-b", "master"]);
        git(&root, &["add", ".gitignore", "README.md"]);
        git(&root, &["add", "-f", "generated/keep.txt"]);
        assert!(
            RetainedProjectTree::capture(&root, 32, 4096).is_err(),
            "generic retained trees must not silently adopt project-snapshot exclusions"
        );

        let retained = RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
            &root, 32, 32, 4096,
        )
        .expect("ignored cache symlink must be classified before no-follow traversal");
        assert_eq!(
            retained
                .exact_regular_file_bytes("generated/keep.txt")
                .unwrap()
                .as_deref(),
            Some(b"tracked despite ignore\n".as_slice()),
            "Git-tracked content must remain governed even when an ignore rule matches"
        );
        assert_eq!(
            retained
                .exact_regular_file_bytes("generated/cache.bin")
                .unwrap(),
            None
        );
        assert_eq!(
            retained.exact_regular_file_bytes("fuzz/target").unwrap(),
            None
        );

        let workflow_digest = retained.regular_file_snapshot_digest().to_owned();
        fs::write(cache_target.join("incremental.bin"), b"volatile\n").unwrap();
        fs::write(root.join("generated/cache.bin"), b"updated ignored cache\n").unwrap();
        retained.revalidate().expect(
            "changes to ignored files or behind ignored cache symlinks must not stale the snapshot",
        );
        drop(retained);
        let recaptured =
            RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
                &root, 32, 32, 4096,
            )
            .unwrap();
        assert_eq!(recaptured.regular_file_snapshot_digest(), workflow_digest);
        drop(recaptured);
        let domain_pack_snapshot =
            RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 32, 4096)
                .expect("Domain Pack project snapshots must share Git-aware selection");
        assert_eq!(
            domain_pack_snapshot
                .exact_regular_file_bytes("generated/keep.txt")
                .unwrap()
                .as_deref(),
            Some(b"tracked despite ignore\n".as_slice())
        );
        assert_eq!(
            domain_pack_snapshot
                .exact_regular_file_bytes("generated/cache.bin")
                .unwrap(),
            None
        );
        drop(domain_pack_snapshot);

        symlink(&cache_target, root.join("fuzz/relevant-link")).unwrap();
        assert!(
            RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
                &root, 32, 32, 4096
            )
            .is_err(),
            "a relevant non-ignored symlink must continue to fail closed"
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_target).unwrap();
    }

    #[test]
    fn workflow_capture_governs_nested_local_content() {
        let root = project_root("workflow-nested-local");
        fs::create_dir_all(root.join("src/.local")).unwrap();
        fs::write(root.join("src/.local/config.json"), b"governed\n").unwrap();

        let retained = RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
            &root, 16, 16, 1024,
        )
        .unwrap();
        assert_eq!(
            retained
                .exact_regular_file_bytes("src/.local/config.json")
                .unwrap()
                .as_deref(),
            Some(b"governed\n".as_slice())
        );
        fs::write(root.join("src/.local/config.json"), b"changed!\n").unwrap();
        assert!(retained.revalidate().is_err());

        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workflow_capture_rejects_top_level_local_namespace_creation_and_removal() {
        let root = project_root("workflow-local-namespace");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), b"governed\n").unwrap();

        let without_local =
            RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
                &root, 16, 16, 1024,
            )
            .unwrap();
        fs::create_dir(root.join(".local")).unwrap();
        assert!(
            without_local.revalidate().is_err(),
            "creating the top-level local namespace after capture remains drift"
        );
        drop(without_local);

        fs::write(root.join(".local/resume.json"), b"local\n").unwrap();
        let with_local =
            RetainedProjectTree::capture_workflow_snapshot_allowing_stable_file_aliases(
                &root, 16, 16, 1024,
            )
            .unwrap();
        fs::remove_dir_all(root.join(".local")).unwrap();
        assert!(
            with_local.revalidate().is_err(),
            "removing the top-level local namespace after capture remains drift"
        );

        drop(with_local);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_regular_file_bytes_use_retained_handle_and_fail_on_drift() {
        let root = project_root("exact-bytes");
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"retained").unwrap();
        let retained = RetainedProjectTree::capture(&root, 8, 32).unwrap();
        assert_eq!(
            retained
                .exact_regular_file_bytes("README.md")
                .unwrap()
                .as_deref(),
            Some(b"retained".as_slice())
        );
        fs::write(&leaf, b"changed!").unwrap();
        assert!(retained.exact_regular_file_bytes("README.md").is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn stable_file_alias_snapshot_freezes_existing_alias_metadata() {
        let root = project_root("stable-file-aliases");
        let anchors = project_root("stable-file-aliases-anchor");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&anchors).unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"stable").unwrap();
        fs::hard_link(&leaf, anchors.join("before")).unwrap();

        let retained =
            RetainedProjectTree::capture_allowing_stable_file_aliases(&root, 16, 1, 6).unwrap();
        retained.revalidate().unwrap();
        fs::hard_link(&leaf, anchors.join("after")).unwrap();

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(anchors).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn broader_domain_capture_can_close_a_read_only_stable_alias_observation() {
        let root = project_root("shared-stable-file-aliases");
        let anchors = project_root("shared-stable-file-aliases-anchor");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&anchors).unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"stable").unwrap();
        fs::hard_link(&leaf, anchors.join("before")).unwrap();

        let retained =
            RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 16, 6).unwrap();
        retained.revalidate_with_stable_file_aliases().unwrap();
        fs::hard_link(&leaf, anchors.join("after")).unwrap();

        assert!(retained.revalidate_with_stable_file_aliases().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(anchors).unwrap();
    }

    #[test]
    fn retained_snapshot_rejects_in_place_file_growth() {
        let root = project_root("file-growth");
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"fixed").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();

        let mut writer = fs::OpenOptions::new().append(true).open(&leaf).unwrap();
        writer.write_all(b" growth").unwrap();
        writer.sync_all().unwrap();
        drop(writer);

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retained_snapshot_rejects_byte_identical_file_replacement() {
        let root = project_root("file-replacement");
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"same bytes\n").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();

        fs::remove_file(&leaf).unwrap();
        fs::write(&leaf, b"same bytes\n").unwrap();

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn retained_snapshot_rejects_regular_file_to_symlink_replacement() {
        let root = project_root("file-to-symlink");
        let outside = project_root("file-to-symlink-target");
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, b"same bytes\n").unwrap();
        let leaf = root.join("README.md");
        fs::write(&leaf, b"same bytes\n").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();

        fs::remove_file(&leaf).unwrap();
        std::os::unix::fs::symlink(&outside, &leaf).unwrap();

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn retained_snapshot_rejects_root_namespace_replacement() {
        let root = project_root("root-replacement");
        let displaced = root.with_extension("displaced");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), b"same bytes\n").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();

        fs::rename(&root, &displaced).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), b"same bytes\n").unwrap();

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(displaced).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn retained_snapshot_rejects_parent_namespace_replacement() {
        let container = project_root("parent-replacement");
        let parent = container.join("parent");
        let displaced = container.join("parent-displaced");
        let root = parent.join("project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), b"same bytes\n").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();

        fs::rename(&parent, &displaced).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), b"same bytes\n").unwrap();

        assert!(retained.revalidate().is_err());
        drop(retained);
        fs::remove_dir_all(container).unwrap();
    }

    #[test]
    fn completion_binding_is_stable_only_for_the_originating_capability() {
        let root = project_root("capability-binding");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), b"pub fn exact() {}\n").unwrap();
        let first = RetainedProjectTree::capture(&root, 16, 1024).unwrap();
        let binding = first.completion_binding().unwrap();
        assert_eq!(first.completion_binding().unwrap(), binding);
        first.validate_completion_binding(&binding).unwrap();

        let second = RetainedProjectTree::capture(&root, 16, 1024).unwrap();
        assert_eq!(first.snapshot_digest(), second.snapshot_digest());
        let reminted = second.completion_binding().unwrap();
        assert_ne!(
            reminted, binding,
            "fresh handles must not remint persisted authority from reusable identifiers"
        );
        assert!(second.validate_completion_binding(&binding).is_err());
        drop(second);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn completion_binding_rejects_byte_identical_namespace_replacement() {
        let root = project_root("byte-identical-replacement");
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("authority.txt");
        fs::write(&leaf, b"same bytes\n").unwrap();
        let retained = RetainedProjectTree::capture(&root, 16, 1024).unwrap();
        let binding = retained.completion_binding().unwrap();
        fs::remove_file(&leaf).unwrap();
        fs::write(&leaf, b"same bytes\n").unwrap();
        assert!(retained.revalidate().is_err());
        assert!(retained.validate_completion_binding(&binding).is_err());
        drop(retained);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    #[test]
    fn persisted_completion_anchors_cross_bind_fresh_project_handles() {
        let root = project_root("persisted-anchor-transfer");
        let authority_path = project_root("persisted-anchor-authority");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&authority_path).unwrap();
        let leaf = root.join("authority.txt");
        fs::write(&leaf, b"same bytes\n").unwrap();
        let authority = RetainedDirectory::open_root(&authority_path).unwrap();

        let first = RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 16, 1024)
            .unwrap();
        let anchors = first
            .retain_completion_anchors(&authority, Path::new("project"))
            .unwrap();
        let binding = anchors.binding().clone();
        anchors.revalidate().unwrap();
        drop(anchors);
        drop(first);

        let current =
            RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 16, 1024)
                .unwrap();
        let reopened = current
            .open_completion_anchors(&authority, &binding)
            .unwrap();
        reopened.revalidate().unwrap();
        drop(reopened);
        drop(current);

        fs::rename(&leaf, root.join("authority.txt.displaced")).unwrap();
        fs::write(&leaf, b"same bytes\n").unwrap();
        let replacement =
            RetainedProjectTree::capture_allowing_store_owned_file_anchors(&root, 16, 1024)
                .unwrap();
        assert!(replacement
            .open_completion_anchors(&authority, &binding)
            .is_err());
        drop(replacement);
        drop(authority);
        fs::remove_dir_all(authority_path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
