//! Descriptor-relative, no-follow access beneath a retained directory handle.
//!
//! Every component is opened relative to the preceding retained directory.
//! The ambient pathname is retained only for diagnostics; it is never used to
//! resolve a child after the root handle is retained.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::path::{Component, Path, PathBuf};

const RETAINED_FILE_ANCHOR_SCHEMA_VERSION: &str = "forge-retained-file-anchor-v1";
const RETAINED_FILE_ANCHOR_ATTEMPTS: usize = 32;
const RETAINED_FILE_ANCHOR_NONCE_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedFileIdentity {
    platform: PlatformIdentity,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformIdentity {
    volume: u32,
    index: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformIdentity {
    length: u64,
    modified: Option<std::time::SystemTime>,
}

impl RetainedFileIdentity {
    /// Persisting a device/inode or volume/index digest is intentionally
    /// unsupported because those identifiers can be reused after the exact
    /// handle is dropped.
    ///
    /// Ordinary descriptor-lifetime comparisons continue to use this type.
    /// Persisted authority must instead carry a [`RetainedFileAnchorBinding`]
    /// whose Store-owned anchor keeps the exact object alive.
    pub(crate) fn canonical_digest(&self) -> io::Result<String> {
        let _ = self;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "reusable platform file identities cannot authorize persisted state; retain a Store-owned file anchor",
        ))
    }
}

/// Canonical persisted description of one Store-owned exact-file anchor.
///
/// The random nonce names a private no-replace hard-link beneath an authority
/// root. The content digest and length prevent mutation of the retained object.
/// Platform device/inode and volume/index values are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedFileAnchorBinding {
    pub(crate) schema_version: String,
    pub(crate) anchor_relative_path: String,
    pub(crate) nonce: String,
    pub(crate) content_digest: String,
    pub(crate) byte_length: u64,
}

impl RetainedFileAnchorBinding {
    pub(crate) fn canonical_digest(&self) -> io::Result<String> {
        self.validate()?;
        let bytes = serde_json_canonicalizer::to_vec(self).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("canonical retained anchor encoding failed: {error}"),
            )
        })?;
        Ok(crate::sha256_content_hash(&bytes))
    }

    fn validate(&self) -> io::Result<PathBuf> {
        if self.schema_version != RETAINED_FILE_ANCHOR_SCHEMA_VERSION
            || !is_lower_hex(&self.nonce, RETAINED_FILE_ANCHOR_NONCE_BYTES * 2)
            || !is_sha256_digest(&self.content_digest)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained file anchor binding has an invalid schema, nonce, or content digest",
            ));
        }
        let path = PathBuf::from(&self.anchor_relative_path);
        let normalized = normalized_relative_path(&path)?;
        let expected_leaf = format!(".forge-retained-file-{}.anchor", self.nonce);
        if normalized != self.anchor_relative_path
            || path.file_name() != Some(OsStr::new(&expected_leaf))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained file anchor binding has a non-canonical private path",
            ));
        }
        Ok(path)
    }
}

/// Opaque Store-owned lifetime capability for one exact immutable file.
///
/// The capability keeps the private anchor handle open and revalidates both its
/// namespace name and content binding. On supported Unix and Windows targets,
/// the private exact-handle hard link also prevents object-identifier reuse while
/// persisted authority is eligible for acceptance. Other platforms fail closed
/// rather than falling back to reusable platform identifiers.
pub(crate) struct RetainedFileLifetimeAnchor {
    authority_root: RetainedDirectory,
    anchor_file: File,
    anchor_identity: RetainedFileIdentity,
    binding: RetainedFileAnchorBinding,
}

impl std::fmt::Debug for RetainedFileLifetimeAnchor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedFileLifetimeAnchor")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

impl RetainedFileLifetimeAnchor {
    pub(crate) fn binding(&self) -> &RetainedFileAnchorBinding {
        &self.binding
    }

    pub(crate) fn revalidate(&self) -> io::Result<()> {
        let anchor_path = self.binding.validate()?;
        self.authority_root.verify_retained_anchor_binding(
            &anchor_path,
            &self.anchor_file,
            &self.anchor_identity,
        )?;
        validate_retained_content(
            &self.anchor_file,
            &self.anchor_identity,
            &self.binding.content_digest,
            self.binding.byte_length,
        )?;
        self.authority_root.verify_retained_anchor_binding(
            &anchor_path,
            &self.anchor_file,
            &self.anchor_identity,
        )
    }

    /// Require an already-retained target handle to be the exact object kept
    /// alive by this anchor, with the same immutable content binding.
    pub(crate) fn validate_retained_file(
        &self,
        retained: &File,
        expected_identity: &RetainedFileIdentity,
    ) -> io::Result<()> {
        self.revalidate()?;
        if *expected_identity != self.anchor_identity
            || RetainedDirectory::identity_of(retained)? != self.anchor_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained target is not the exact Store-anchored file",
            ));
        }
        validate_retained_content(
            retained,
            expected_identity,
            &self.binding.content_digest,
            self.binding.byte_length,
        )?;
        self.revalidate()
    }

    /// Reopen and retain the current target pathname only after proving that it
    /// still names the exact anchored object. Hidden or substituted names fail.
    pub(crate) fn retain_target(
        &self,
        target_root: &RetainedDirectory,
        target_relative_path: &Path,
    ) -> io::Result<(File, RetainedFileIdentity)> {
        self.revalidate()?;
        let file = target_root
            .open_leaf_read(target_relative_path, RetainedLeafPolicy::AnchoredAuthority)?;
        let identity = RetainedDirectory::identity_of(&file)?;
        target_root.verify_retained_anchor_binding(target_relative_path, &file, &identity)?;
        self.validate_retained_file(&file, &identity)?;
        target_root.verify_retained_anchor_binding(target_relative_path, &file, &identity)?;
        self.revalidate()?;
        Ok((file, identity))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetainedLeafPolicy {
    /// Read-only caller input; hard links are allowed.
    SourceRead,
    /// Identity-bound Store authority. Unix may carry Store-owned hard-link
    /// cleanup debt from exact-handle publication; callers must not mutate it.
    Authority,
    /// Immutable Store authority kept alive by one or more exact lifetime-anchor
    /// links. Content and namespace bindings are revalidated before every use.
    AnchoredAuthority,
    /// In-place mutation authority; the exact retained file must be singly linked.
    MutableAuthority,
}

#[derive(Debug)]
pub(crate) struct RetainedDirectory {
    handle: File,
    display_path: PathBuf,
}

/// Store-owned authority bound to one exact retained directory handle.
///
/// The private fields keep mutation primitives out of path-only call sites.
/// Callers must first validate their higher-level lock or producer capability,
/// then retain this sealed directory authority for the mutation itself.
#[derive(Debug)]
pub(crate) struct RetainedAuthorityDirectory<'directory> {
    directory: &'directory RetainedDirectory,
    identity: RetainedFileIdentity,
}

/// Bounded cleanup debt recorded as exact objects isolated under random,
/// Store-created quarantine names. Unix never unlinks these mutable names after
/// dropping the exact source handle; a later trusted maintenance pass may
/// account for them without weakening the committing operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RetainedCleanupDebt {
    isolated_paths: Vec<PathBuf>,
}

impl RetainedCleanupDebt {
    fn none() -> Self {
        Self::default()
    }

    fn one(path: PathBuf) -> Self {
        Self {
            isolated_paths: vec![path],
        }
    }

    fn two(first: PathBuf, second: PathBuf) -> Self {
        Self {
            isolated_paths: vec![first, second],
        }
    }

    pub(crate) fn into_paths(self) -> Vec<PathBuf> {
        self.isolated_paths
    }

    #[cfg(test)]
    fn paths(&self) -> &[PathBuf] {
        &self.isolated_paths
    }
}

#[derive(Debug)]
struct RetainedParentBinding {
    handle: File,
    relative_path: PathBuf,
    identity: RetainedFileIdentity,
}

#[derive(Debug)]
struct RetainedAuthorityLeaf {
    parent: RetainedParentBinding,
    leaf: OsString,
    path: PathBuf,
    file: File,
    identity: RetainedFileIdentity,
}

fn normalized_relative_path(path: &Path) -> io::Result<String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "retained anchor path must be a non-empty relative path",
        ));
    }
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_owned).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "retained anchor path must use UTF-8 components",
                )
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained anchor path is not normalized",
            )),
        })
        .collect::<io::Result<Vec<_>>>()
        .map(|components| components.join("/"))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| is_lower_hex(hex, 64))
}

fn random_hex_nonce(byte_length: usize) -> io::Result<String> {
    let mut nonce = vec![0_u8; byte_length];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "retained file anchor nonce generation failed: {error}"
        ))
    })?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(byte_length.saturating_mul(2));
    for byte in nonce {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn retained_content_digest(file: &File, expected_length: u64) -> io::Result<String> {
    let before = file.metadata()?;
    if before.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained anchored file length differs from its content binding",
        ));
    }
    let mut reader = file.try_clone()?;
    reader.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut remaining = expected_length;
    let mut buffer = vec![0_u8; 64 * 1024];
    while remaining > 0 {
        let maximum = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded anchor read length fits usize");
        let read = reader.read(&mut buffer[..maximum])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "retained anchored file ended before its content binding",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining = remaining.saturating_sub(read as u64);
    }
    let mut extra = [0_u8; 1];
    if reader.read(&mut extra)? != 0 || file.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained anchored file changed while hashing",
        ));
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn validate_retained_content(
    file: &File,
    expected_identity: &RetainedFileIdentity,
    expected_digest: &str,
    expected_length: u64,
) -> io::Result<()> {
    if !is_sha256_digest(expected_digest)
        || RetainedDirectory::identity_of(file)? != *expected_identity
        || retained_content_digest(file, expected_length)? != expected_digest
        || RetainedDirectory::identity_of(file)? != *expected_identity
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained anchored file changed identity, bytes, or length",
        ));
    }
    Ok(())
}

impl RetainedDirectory {
    pub(crate) fn open_root(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        let handle = {
            use std::os::unix::fs::OpenOptionsExt as _;
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
                .open(path)?
        };
        #[cfg(windows)]
        let handle = {
            use std::os::windows::fs::OpenOptionsExt as _;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)?
        };
        #[cfg(not(any(unix, windows)))]
        let handle = File::open(path)?;
        let metadata = handle.metadata()?;
        #[cfg(windows)]
        let is_supported_directory = {
            use std::os::windows::fs::MetadataExt as _;
            metadata.is_dir() && metadata.file_attributes() & 0x400 != 0x400
        };
        #[cfg(not(windows))]
        let is_supported_directory = metadata.is_dir();
        if !is_supported_directory {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained root is not a real, non-reparse directory",
            ));
        }
        Ok(Self::from_handle(handle, path.to_path_buf()))
    }

    pub(crate) fn from_handle(handle: File, display_path: PathBuf) -> Self {
        Self {
            handle,
            display_path,
        }
    }

    pub(crate) fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub(crate) fn identity(&self) -> io::Result<RetainedFileIdentity> {
        Self::identity_of(&self.handle)
    }

    pub(crate) fn identity_of(file: &File) -> io::Result<RetainedFileIdentity> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let metadata = file.metadata()?;
            Ok(RetainedFileIdentity {
                platform: PlatformIdentity {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                },
            })
        }
        #[cfg(windows)]
        {
            let information = crate::windows_file_info::file_information(file)?;
            Ok(RetainedFileIdentity {
                platform: PlatformIdentity {
                    volume: information.volume_serial_number,
                    index: information.file_index,
                },
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let metadata = file.metadata()?;
            Ok(RetainedFileIdentity {
                platform: PlatformIdentity {
                    length: metadata.len(),
                    modified: metadata.modified().ok(),
                },
            })
        }
    }

    /// Publish one retained regular-file handle at a vacant descriptor-relative
    /// destination. Supported Unix callers share this primitive so publication
    /// never selects the source object through a mutable namespace name.
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    pub(crate) fn link_retained_file_noreplace(
        retained_source: &File,
        destination_parent: &File,
        destination_leaf: &OsStr,
    ) -> io::Result<()> {
        platform::link_exact_noreplace(retained_source, destination_parent, destination_leaf)
    }

    /// Retain sealed Store mutation authority for this exact directory.
    ///
    /// The higher-level Store owner must validate its lock or producer boundary
    /// immediately before calling this constructor.
    pub(crate) fn retain_authority(&self) -> io::Result<RetainedAuthorityDirectory<'_>> {
        Ok(RetainedAuthorityDirectory {
            directory: self,
            identity: self.identity()?,
        })
    }

    /// Create a private generation-safe anchor for one exact immutable file.
    ///
    /// `anchor_directory` is a normalized relative directory owned by the
    /// higher-level Store authority. The source is selected only through its
    /// retained handle. Successful creation atomically hard-links that exact
    /// object beneath a random Store nonce, syncs the anchor parent, and retains
    /// the anchor handle for subsequent claim/lifecycle/restore validation.
    pub(crate) fn retain_file_lifetime_anchor(
        &self,
        anchor_directory: &Path,
        retained: &File,
        expected_identity: &RetainedFileIdentity,
        expected_content_digest: &str,
        expected_byte_length: u64,
    ) -> io::Result<RetainedFileLifetimeAnchor> {
        if anchor_directory.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained file anchor directory is empty",
            ));
        }
        normalized_relative_path(anchor_directory)?;
        if !is_sha256_digest(expected_content_digest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained file anchor content digest is not canonical sha256",
            ));
        }
        Self::verify_retained_handle(retained, expected_identity)?;
        validate_retained_content(
            retained,
            expected_identity,
            expected_content_digest,
            expected_byte_length,
        )?;
        self.create_dir_all(anchor_directory)?;
        let parent = self
            .open_parent_bound(&anchor_directory.join("anchor"), false)?
            .0;
        self.verify_parent_binding(&parent)?;

        for _ in 0..RETAINED_FILE_ANCHOR_ATTEMPTS {
            let nonce = random_hex_nonce(RETAINED_FILE_ANCHOR_NONCE_BYTES)?;
            let leaf = OsString::from(format!(".forge-retained-file-{nonce}.anchor"));
            let path = anchor_directory.join(&leaf);
            match platform::link_lifetime_anchor_noreplace(retained, &parent.handle, &leaf) {
                Ok(()) => {
                    let anchor_file =
                        platform::open_file(&parent.handle, &leaf, platform::FileMode::Read)?;
                    Self::validate_leaf(&anchor_file, RetainedLeafPolicy::AnchoredAuthority)?;
                    let anchor_identity = Self::identity_of(&anchor_file)?;
                    if anchor_identity != *expected_identity {
                        return Err(Self::authority_identity_changed(
                            "lifetime anchor publication",
                        ));
                    }
                    let binding = RetainedFileAnchorBinding {
                        schema_version: RETAINED_FILE_ANCHOR_SCHEMA_VERSION.to_owned(),
                        anchor_relative_path: normalized_relative_path(&path)?,
                        nonce,
                        content_digest: expected_content_digest.to_owned(),
                        byte_length: expected_byte_length,
                    };
                    validate_retained_content(
                        &anchor_file,
                        &anchor_identity,
                        &binding.content_digest,
                        binding.byte_length,
                    )?;
                    self.sync_parent_binding(&parent)?;
                    let anchor = RetainedFileLifetimeAnchor {
                        authority_root: self.try_clone()?,
                        anchor_file,
                        anchor_identity,
                        binding,
                    };
                    anchor.revalidate()?;
                    anchor.validate_retained_file(retained, expected_identity)?;
                    return Ok(anchor);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "retained file anchor nonce retry exhausted",
        ))
    }

    /// Reopen a previously persisted exact-file anchor. A missing, hidden,
    /// non-canonical, content-modified, or unsupported anchor fails closed.
    pub(crate) fn open_file_lifetime_anchor(
        &self,
        binding: &RetainedFileAnchorBinding,
    ) -> io::Result<RetainedFileLifetimeAnchor> {
        let anchor_path = binding.validate()?;
        let anchor_file =
            self.open_leaf_read(&anchor_path, RetainedLeafPolicy::AnchoredAuthority)?;
        let anchor_identity = Self::identity_of(&anchor_file)?;
        validate_retained_content(
            &anchor_file,
            &anchor_identity,
            &binding.content_digest,
            binding.byte_length,
        )?;
        let anchor = RetainedFileLifetimeAnchor {
            authority_root: self.try_clone()?,
            anchor_file,
            anchor_identity,
            binding: binding.clone(),
        };
        anchor.revalidate()?;
        Ok(anchor)
    }

    fn components(&self, path: &Path) -> io::Result<Vec<OsString>> {
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.display_path).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "path escapes retained root",
                )
            })?
        } else {
            path
        };
        let mut result = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(value) => result.push(value.to_os_string()),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path is not a normalized relative child",
                    ));
                }
            }
        }
        if result.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty child path",
            ));
        }
        Ok(result)
    }

    fn open_parent_bound(
        &self,
        path: &Path,
        create: bool,
    ) -> io::Result<(RetainedParentBinding, OsString)> {
        let mut components = self.components(path)?;
        let leaf = components.pop().expect("non-empty checked above");
        let mut directory = self.handle.try_clone()?;
        let mut relative_path = PathBuf::new();
        for component in components {
            relative_path.push(&component);
            directory = if create {
                platform::open_or_create_directory(&directory, &component)?
            } else {
                platform::open_directory(&directory, &component)?
            };
        }
        let binding = RetainedParentBinding {
            identity: Self::identity_of(&directory)?,
            handle: directory,
            relative_path,
        };
        self.verify_parent_binding(&binding)?;
        Ok((binding, leaf))
    }

    fn reopen_directory(&self, relative_path: &Path) -> io::Result<File> {
        let mut directory = self.handle.try_clone()?;
        if relative_path.as_os_str().is_empty() {
            return Ok(directory);
        }
        for component in self.components(relative_path)? {
            directory = platform::open_directory(&directory, &component)?;
        }
        Ok(directory)
    }

    fn verify_parent_binding(&self, binding: &RetainedParentBinding) -> io::Result<()> {
        if Self::identity_of(&binding.handle)? != binding.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained authority parent handle changed identity",
            ));
        }
        let current = self.reopen_directory(&binding.relative_path)?;
        if Self::identity_of(&current)? != binding.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained authority parent namespace changed identity",
            ));
        }
        Ok(())
    }

    pub fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut directory = self.handle.try_clone()?;
        for component in self.components(path)? {
            directory = platform::open_or_create_directory(&directory, &component)?;
        }
        Ok(())
    }

    pub fn open_directory(&self, path: &Path) -> io::Result<Self> {
        let mut directory = self.handle.try_clone()?;
        for component in self.components(path)? {
            directory = platform::open_directory(&directory, &component)?;
        }
        Ok(Self {
            handle: directory,
            display_path: self.display_path.join(path),
        })
    }

    pub(crate) fn directory_entry_count_bounded(
        &self,
        path: &Path,
        maximum: usize,
    ) -> io::Result<usize> {
        let directory = self.open_directory(path)?;
        Self::validate_directory_handle(&directory.handle)?;
        let expected = directory.identity()?;
        let count = platform::directory_entry_count(&directory.handle, maximum)?;
        if directory.identity()? != expected {
            return Err(Self::authority_identity_changed(
                "directory changed during bounded enumeration",
            ));
        }
        Ok(count)
    }

    fn validate_directory_handle(directory: &File) -> io::Result<()> {
        let metadata = directory.metadata()?;
        #[cfg(windows)]
        let supported = {
            use std::os::windows::fs::MetadataExt as _;
            metadata.is_dir() && metadata.file_attributes() & 0x400 != 0x400
        };
        #[cfg(not(windows))]
        let supported = metadata.is_dir();
        if supported {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained directory is not a real, non-reparse directory",
            ))
        }
    }

    fn remove_authority_empty_directory(&self, path: &Path) -> io::Result<()> {
        let (parent, leaf) = self.open_parent_bound(path, false)?;
        self.verify_parent_binding(&parent)?;
        let retained = platform::open_directory_delete(&parent.handle, &leaf)?;
        Self::validate_directory_handle(&retained)?;
        let expected = Self::identity_of(&retained)?;
        let current = platform::open_directory(&parent.handle, &leaf)?;
        Self::validate_directory_handle(&current)?;
        if Self::identity_of(&current)? != expected {
            return Err(Self::authority_identity_changed("empty-directory removal"));
        }

        #[cfg(unix)]
        {
            const QUARANTINE_ATTEMPTS: usize = 32;
            let nonce = Self::quarantine_nonce()?;
            let mut quarantine_leaf = None;
            for attempt in 0..QUARANTINE_ATTEMPTS {
                let candidate = Self::quarantine_leaf("empty-directory-delete", nonce, attempt);
                match platform::rename_any_noreplace(
                    &parent.handle,
                    &leaf,
                    &parent.handle,
                    &candidate,
                ) {
                    Ok(()) => {
                        quarantine_leaf = Some(candidate);
                        break;
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
            }
            let quarantine_leaf = quarantine_leaf.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "retained directory quarantine-name retry exhausted",
                )
            })?;
            let quarantined = platform::open_directory_delete(&parent.handle, &quarantine_leaf)?;
            Self::validate_directory_handle(&quarantined)?;
            if Self::identity_of(&quarantined)? != expected {
                return Err(Self::authority_identity_changed(
                    "empty-directory quarantine",
                ));
            }
            match platform::open_directory(&parent.handle, &leaf) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(Self::authority_identity_changed(
                        "empty-directory quarantine source",
                    ));
                }
                Err(error) => return Err(error),
            }
            self.sync_parent_binding(&parent)?;
            if let Err(error) = platform::remove_empty_directory(
                &parent.handle,
                &quarantine_leaf,
                quarantined,
                &expected,
            ) {
                let rollback = match platform::open_directory(&parent.handle, &leaf) {
                    Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => {
                        platform::rename_any_noreplace(
                            &parent.handle,
                            &quarantine_leaf,
                            &parent.handle,
                            &leaf,
                        )
                    }
                    Ok(_) => Err(Self::authority_identity_changed(
                        "empty-directory rollback destination",
                    )),
                    Err(source_error) => Err(source_error),
                };
                if let Err(rollback_error) = rollback {
                    return Err(io::Error::new(
                        rollback_error.kind(),
                        format!(
                            "empty-directory cleanup failed ({error}); quarantine rollback failed ({rollback_error})"
                        ),
                    ));
                }
                let restored = platform::open_directory(&parent.handle, &leaf)?;
                Self::validate_directory_handle(&restored)?;
                if Self::identity_of(&restored)? != expected {
                    return Err(Self::authority_identity_changed(
                        "empty-directory rollback result",
                    ));
                }
                self.sync_parent_binding(&parent)?;
                return Err(error);
            }
            for candidate in [&leaf, &quarantine_leaf] {
                match platform::open_directory(&parent.handle, candidate) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Ok(_) => {
                        return Err(Self::authority_identity_changed(
                            "empty-directory quarantine final state",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            self.sync_parent_binding(&parent)?;
            return Ok(());
        }

        #[cfg(windows)]
        {
            platform::remove_empty_directory(&parent.handle, &leaf, retained, &expected)?;
            match platform::open_directory(&parent.handle, &leaf) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(Self::authority_identity_changed(
                        "empty-directory removal final state",
                    ));
                }
                Err(error) => return Err(error),
            }
            return self.sync_parent_binding(&parent);
        }

        #[cfg(not(any(unix, windows)))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exact empty-directory deletion is unsupported on this platform",
        ))
    }

    fn remove_authority_reclaimable_file<H>(
        &self,
        path: &Path,
        mut before_commit: H,
    ) -> io::Result<()>
    where
        H: FnMut(&Self, &Path) -> io::Result<()>,
    {
        let source = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::Authority,
            platform::FileMode::ReadDeleteRename,
            false,
        )?;
        before_commit(self, &source.path)?;
        self.verify_leaf_binding(&source)?;

        #[cfg(unix)]
        {
            let quarantine_path = self.quarantine_named_leaf_noreplace(
                &source.parent,
                &source.leaf,
                &source.file,
                &source.identity,
                "reclaimable-file-delete",
            )?;
            let (quarantine_parent, quarantine_leaf) =
                self.open_parent_bound(&quarantine_path, false)?;
            let quarantined = platform::open_file(
                &quarantine_parent.handle,
                &quarantine_leaf,
                platform::FileMode::ReadDeleteRename,
            )?;
            if Self::identity_of(&quarantined)? != source.identity {
                return Err(Self::authority_identity_changed(
                    "reclaimable-file quarantine",
                ));
            }
            if let Err(error) = platform::remove_named_file(
                &quarantine_parent.handle,
                &quarantine_leaf,
                quarantined,
                &source.identity,
            ) {
                return Err(self.rollback_noreplace_or_isolate(
                    &source,
                    &quarantine_parent,
                    &quarantine_leaf,
                    &error,
                ));
            }
            if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?
                .is_some()
                || Self::direct_optional_authority_identity(
                    &quarantine_parent.handle,
                    &quarantine_leaf,
                )?
                .is_some()
            {
                return Err(Self::authority_identity_changed(
                    "reclaimable-file quarantine final state",
                ));
            }
            self.sync_parent_binding(&source.parent)?;
            return Ok(());
        }

        #[cfg(windows)]
        {
            let parent = source.parent;
            let leaf = source.leaf;
            platform::remove_exact(source.file)?;
            if Self::direct_optional_authority_identity(&parent.handle, &leaf)?.is_some() {
                return Err(Self::authority_identity_changed(
                    "handle-bound reclaimable-file delete",
                ));
            }
            self.sync_parent_binding(&parent)?;
            return Ok(());
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = source;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "exact reclaimable-file deletion is unsupported on this platform",
            ))
        }
    }

    pub fn sync_directory(&self, path: &Path) -> io::Result<()> {
        self.open_directory(path)?.handle.sync_all()
    }

    pub(crate) fn sync_root(&self) -> io::Result<()> {
        self.handle.sync_all()
    }

    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self::from_handle(
            self.handle.try_clone()?,
            self.display_path.clone(),
        ))
    }

    fn validate_leaf(file: &File, policy: RetainedLeafPolicy) -> io::Result<()> {
        let metadata = file.metadata()?;
        let regular = metadata.is_file() && !metadata.file_type().is_symlink();
        #[cfg(windows)]
        let regular = regular && {
            use std::os::windows::fs::MetadataExt as _;
            metadata.file_attributes() & 0x400 != 0x400
        };
        #[cfg(windows)]
        let link_count =
            u64::from(crate::windows_file_info::file_information(file)?.number_of_links);
        #[cfg(unix)]
        let link_count = {
            use std::os::unix::fs::MetadataExt as _;
            metadata.nlink()
        };
        #[cfg(not(any(unix, windows)))]
        let link_count = 0;
        let link_shape_supported = match policy {
            RetainedLeafPolicy::SourceRead => true,
            // Exact-handle Unix publication deliberately leaves the old link as
            // discoverable Store cleanup debt when no fd-bound unlink exists.
            RetainedLeafPolicy::Authority => {
                #[cfg(unix)]
                {
                    link_count >= 1
                }
                #[cfg(not(unix))]
                {
                    link_count == 1
                }
            }
            RetainedLeafPolicy::AnchoredAuthority => link_count >= 1,
            RetainedLeafPolicy::MutableAuthority => link_count == 1,
        };
        if !regular || !link_shape_supported {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained leaf is not a regular, non-reparse file with the required link shape",
            ));
        }
        Ok(())
    }

    fn open_leaf_bound(
        &self,
        path: &Path,
        policy: RetainedLeafPolicy,
        mode: platform::FileMode,
        create_parent: bool,
    ) -> io::Result<RetainedAuthorityLeaf> {
        let (parent, leaf) = self.open_parent_bound(path, create_parent)?;
        let file = platform::open_file(&parent.handle, &leaf, mode)?;
        Self::validate_leaf(&file, policy)?;
        let identity = Self::identity_of(&file)?;
        self.verify_parent_binding(&parent)?;
        let reopened = platform::open_file(&parent.handle, &leaf, platform::FileMode::Read)?;
        Self::validate_leaf(&reopened, policy)?;
        if Self::identity_of(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained leaf namespace changed during open",
            ));
        }
        self.verify_parent_binding(&parent)?;
        Ok(RetainedAuthorityLeaf {
            parent,
            leaf,
            path: path.to_path_buf(),
            file,
            identity,
        })
    }

    fn direct_authority_identity(parent: &File, leaf: &OsStr) -> io::Result<RetainedFileIdentity> {
        let file = platform::open_file(parent, leaf, platform::FileMode::Read)?;
        Self::validate_leaf(&file, RetainedLeafPolicy::Authority)?;
        Self::identity_of(&file)
    }

    fn direct_optional_authority_identity(
        parent: &File,
        leaf: &OsStr,
    ) -> io::Result<Option<RetainedFileIdentity>> {
        match Self::direct_authority_identity(parent, leaf) {
            Ok(identity) => Ok(Some(identity)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn verify_leaf_binding(&self, leaf: &RetainedAuthorityLeaf) -> io::Result<()> {
        Self::validate_leaf(&leaf.file, RetainedLeafPolicy::Authority)?;
        if Self::identity_of(&leaf.file)? != leaf.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained authority leaf handle changed identity",
            ));
        }
        if Self::direct_authority_identity(&leaf.parent.handle, &leaf.leaf)? != leaf.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained authority leaf namespace changed identity",
            ));
        }
        self.verify_parent_binding(&leaf.parent)
    }

    fn verify_mutable_leaf_binding(&self, leaf: &RetainedAuthorityLeaf) -> io::Result<()> {
        Self::validate_leaf(&leaf.file, RetainedLeafPolicy::MutableAuthority)?;
        self.verify_leaf_binding(leaf)
    }

    pub(crate) fn verify_retained_authority_binding(
        &self,
        path: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        Self::verify_retained_handle(retained, expected)?;
        let rebound = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::Authority,
            platform::FileMode::Read,
            false,
        )?;
        if rebound.identity != *expected {
            return Err(Self::authority_identity_changed(
                "retained namespace binding",
            ));
        }
        self.verify_leaf_binding(&rebound)
    }

    fn verify_retained_anchor_binding(
        &self,
        path: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        Self::validate_leaf(retained, RetainedLeafPolicy::AnchoredAuthority)?;
        if Self::identity_of(retained)? != *expected {
            return Err(Self::authority_identity_changed("anchored handle"));
        }
        let rebound = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::AnchoredAuthority,
            platform::FileMode::Read,
            false,
        )?;
        if rebound.identity != *expected {
            return Err(Self::authority_identity_changed(
                "anchored namespace binding",
            ));
        }
        Self::validate_leaf(&rebound.file, RetainedLeafPolicy::AnchoredAuthority)?;
        self.verify_parent_binding(&rebound.parent)
    }

    pub(crate) fn verify_mutable_authority_binding(
        &self,
        path: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        Self::validate_leaf(retained, RetainedLeafPolicy::MutableAuthority)?;
        if Self::identity_of(retained)? != *expected {
            return Err(Self::authority_identity_changed("mutable handle"));
        }
        let rebound = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::Read,
            false,
        )?;
        if rebound.identity != *expected {
            return Err(Self::authority_identity_changed(
                "mutable namespace binding",
            ));
        }
        Self::validate_leaf(&rebound.file, RetainedLeafPolicy::MutableAuthority)?;
        self.verify_parent_binding(&rebound.parent)
    }

    pub(crate) fn mutate_authority_file<R, F>(
        &self,
        path: &Path,
        retained: &mut File,
        expected: &RetainedFileIdentity,
        mutation: F,
    ) -> io::Result<R>
    where
        F: FnOnce(&mut File) -> io::Result<R>,
    {
        self.verify_mutable_authority_binding(path, retained, expected)?;
        let mutation_result = mutation(retained);
        let validation_result = self.verify_mutable_authority_binding(path, retained, expected);
        match (mutation_result, validation_result) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(mutation_error), Err(validation_error)) => Err(io::Error::new(
                validation_error.kind(),
                format!(
                    "mutation failed ({mutation_error}); post-mutation binding validation failed ({validation_error})"
                ),
            )),
        }
    }

    pub(crate) fn open_leaf_read(
        &self,
        path: &Path,
        policy: RetainedLeafPolicy,
    ) -> io::Result<File> {
        self.open_leaf_bound(path, policy, platform::FileMode::Read, false)
            .map(|leaf| leaf.file)
    }

    pub(crate) fn open_leaf_read_write_existing(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_bound(
            path,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadWrite,
            false,
        )
        .map(|leaf| leaf.file)
    }

    pub(crate) fn open_leaf_read_write_create_authority(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_bound(
            path,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadWriteCreate,
            true,
        )
        .map(|leaf| leaf.file)
    }

    /// Opens a single-link lock leaf without sharing delete access on Windows.
    #[cfg(windows)]
    pub(crate) fn open_retained_lock(&self, path: &Path) -> io::Result<File> {
        let (parent, leaf) = self.open_parent_bound(path, true)?;
        let file = platform::open_retained_lock(&parent.handle, &leaf)?;
        Self::validate_leaf(&file, RetainedLeafPolicy::MutableAuthority)?;
        let identity = Self::identity_of(&file)?;
        self.verify_parent_binding(&parent)?;
        if Self::direct_authority_identity(&parent.handle, &leaf)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained lock namespace changed during open",
            ));
        }
        Ok(file)
    }

    pub(crate) fn open_leaf_write_new_authority(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_bound(
            path,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadWriteNewDelete,
            true,
        )
        .map(|leaf| leaf.file)
    }

    pub(crate) fn open_leaf_read_delete_rename_authority(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_bound(
            path,
            RetainedLeafPolicy::Authority,
            platform::FileMode::ReadDeleteRename,
            false,
        )
        .map(|leaf| leaf.file)
    }

    pub(crate) fn read_authority_bounded(&self, path: &Path, limit: u64) -> io::Result<Vec<u8>> {
        use std::io::Read as _;

        let mut retained = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::Authority,
            platform::FileMode::Read,
            false,
        )?;
        let before = retained.file.metadata()?;
        if before.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "file exceeds byte limit",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        retained
            .file
            .by_ref()
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "file exceeds byte limit",
            ));
        }
        let after = retained.file.metadata()?;
        if after.len() != before.len()
            || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained authority leaf changed while read",
            ));
        }
        self.verify_leaf_binding(&retained)?;
        Ok(bytes)
    }

    fn authority_identity_changed(action: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("retained authority {action} identity changed"),
        )
    }

    fn quarantine_nonce() -> io::Result<u128> {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|error| {
            io::Error::other(format!(
                "retained authority quarantine nonce generation failed: {error}"
            ))
        })?;
        Ok(u128::from_le_bytes(nonce))
    }

    fn quarantine_leaf(purpose: &str, nonce: u128, attempt: usize) -> OsString {
        OsString::from(format!(
            ".forge-retained-{purpose}-{}-{nonce}-{attempt}.quarantine",
            std::process::id()
        ))
    }

    fn verify_retained_handle(file: &File, expected: &RetainedFileIdentity) -> io::Result<()> {
        Self::validate_leaf(file, RetainedLeafPolicy::Authority)?;
        if Self::identity_of(file)? == *expected {
            Ok(())
        } else {
            Err(Self::authority_identity_changed("handle"))
        }
    }

    fn verify_exact_bytes(file: &mut File, expected: &[u8]) -> io::Result<()> {
        use std::io::{Read as _, Seek as _, SeekFrom};

        let expected_len = u64::try_from(expected.len()).unwrap_or(u64::MAX);
        if file.metadata()?.len() != expected_len {
            return Err(Self::authority_identity_changed("content length"));
        }
        file.seek(SeekFrom::Start(0))?;
        let mut actual = Vec::with_capacity(expected.len());
        file.by_ref()
            .take(expected_len.saturating_add(1))
            .read_to_end(&mut actual)?;
        if actual != expected || file.metadata()?.len() != expected_len {
            return Err(Self::authority_identity_changed("content"));
        }
        Ok(())
    }

    fn sync_parent_binding(&self, parent: &RetainedParentBinding) -> io::Result<()> {
        self.verify_parent_binding(parent)?;
        parent.handle.sync_all()?;
        self.verify_parent_binding(parent)
    }

    fn verify_destination_state(
        &self,
        parent: &RetainedParentBinding,
        leaf: &OsStr,
        expected: Option<&RetainedFileIdentity>,
    ) -> io::Result<()> {
        let actual = Self::direct_optional_authority_identity(&parent.handle, leaf)?;
        if actual.as_ref() != expected {
            return Err(Self::authority_identity_changed("destination"));
        }
        self.verify_parent_binding(parent)
    }

    fn create_quarantine_leaf<F>(
        &self,
        parent_path: &Path,
        mut candidate: F,
    ) -> io::Result<RetainedAuthorityLeaf>
    where
        F: FnMut(usize) -> PathBuf,
    {
        const CREATE_ATTEMPTS: usize = 32;
        for attempt in 0..CREATE_ATTEMPTS {
            let path = candidate(attempt);
            if path.parent().unwrap_or_else(|| Path::new("")) != parent_path {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "retained quarantine candidate changed parent",
                ));
            }
            match self.open_leaf_bound(
                &path,
                RetainedLeafPolicy::MutableAuthority,
                platform::FileMode::ReadWriteNewDelete,
                true,
            ) {
                Ok(leaf) => return Ok(leaf),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "retained authority quarantine-name retry exhausted",
        ))
    }

    fn quarantine_named_leaf_noreplace(
        &self,
        parent: &RetainedParentBinding,
        source_leaf: &OsStr,
        retained_source: &File,
        expected: &RetainedFileIdentity,
        purpose: &str,
    ) -> io::Result<PathBuf> {
        const QUARANTINE_ATTEMPTS: usize = 32;
        let nonce = Self::quarantine_nonce()?;
        Self::verify_retained_handle(retained_source, expected)?;
        if Self::direct_authority_identity(&parent.handle, source_leaf)? != *expected {
            return Err(Self::authority_identity_changed("quarantine source"));
        }
        self.verify_parent_binding(parent)?;
        for attempt in 0..QUARANTINE_ATTEMPTS {
            let quarantine_leaf = Self::quarantine_leaf(purpose, nonce, attempt);
            match platform::rename_noreplace(
                &parent.handle,
                source_leaf,
                &parent.handle,
                &quarantine_leaf,
                retained_source,
            ) {
                Ok(()) => {
                    if Self::direct_authority_identity(&parent.handle, &quarantine_leaf)?
                        != *expected
                        || Self::direct_optional_authority_identity(&parent.handle, source_leaf)?
                            .is_some()
                    {
                        return Err(Self::authority_identity_changed("quarantine commit"));
                    }
                    self.sync_parent_binding(parent)?;
                    return Ok(parent.relative_path.join(quarantine_leaf));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "retained authority quarantine-name retry exhausted",
        ))
    }

    #[cfg(unix)]
    fn quarantine_any_name_noreplace(&self, path: &Path, purpose: &str) -> io::Result<PathBuf> {
        const QUARANTINE_ATTEMPTS: usize = 32;
        let (parent, source_leaf) = self.open_parent_bound(path, false)?;
        let nonce = Self::quarantine_nonce()?;
        self.verify_parent_binding(&parent)?;
        for attempt in 0..QUARANTINE_ATTEMPTS {
            let quarantine_leaf = Self::quarantine_leaf(purpose, nonce, attempt);
            match platform::rename_any_noreplace(
                &parent.handle,
                &source_leaf,
                &parent.handle,
                &quarantine_leaf,
            ) {
                Ok(()) => {
                    self.sync_parent_binding(&parent)?;
                    return Ok(parent.relative_path.join(quarantine_leaf));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "retained untrusted-name quarantine retry exhausted",
        ))
    }

    fn rollback_noreplace_or_isolate(
        &self,
        source: &RetainedAuthorityLeaf,
        destination_parent: &RetainedParentBinding,
        destination_leaf: &OsStr,
        cause: &io::Error,
    ) -> io::Error {
        let destination_is_source =
            Self::direct_optional_authority_identity(&destination_parent.handle, destination_leaf)
                .is_ok_and(|identity| identity.as_ref() == Some(&source.identity));
        let source_is_source =
            Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)
                .is_ok_and(|identity| identity.as_ref() == Some(&source.identity));

        if destination_is_source {
            let rollback = platform::rename_noreplace(
                &destination_parent.handle,
                destination_leaf,
                &source.parent.handle,
                &source.leaf,
                &source.file,
            )
            .and_then(|()| self.verify_leaf_binding(source))
            .and_then(|()| self.sync_parent_binding(&source.parent));
            if rollback.is_ok() {
                return io::Error::new(
                    cause.kind(),
                    format!(
                        "{cause}; exact retained source rolled back to {}",
                        source.path.display()
                    ),
                );
            }
            let isolation = self.quarantine_named_leaf_noreplace(
                destination_parent,
                destination_leaf,
                &source.file,
                &source.identity,
                "failed-publication",
            );
            return io::Error::other(
                match isolation {
                    Ok(path) => format!(
                        "{cause}; rollback failed and exact retained source was isolated at {}",
                        path.display()
                    ),
                    Err(isolation) => format!(
                        "{cause}; rollback failed and exact retained source isolation failed: {isolation}"
                    ),
                },
            );
        }
        if source_is_source {
            return io::Error::new(
                cause.kind(),
                format!(
                    "{cause}; exact retained source remains isolated at {}",
                    source.path.display()
                ),
            );
        }
        io::Error::other(
            format!(
                "{cause}; exact retained source became a bounded unnamed orphan while its handle was retained"
            ),
        )
    }

    fn rollback_exchange_or_isolate(
        &self,
        source: &RetainedAuthorityLeaf,
        destination_parent: &RetainedParentBinding,
        destination_leaf: &OsStr,
        cause: &io::Error,
    ) -> io::Error {
        let destination_is_source =
            Self::direct_optional_authority_identity(&destination_parent.handle, destination_leaf)
                .is_ok_and(|identity| identity.as_ref() == Some(&source.identity));
        let source_present =
            Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)
                .is_ok_and(|identity| identity.is_some());
        if destination_is_source && source_present {
            let rollback = platform::exchange(
                &destination_parent.handle,
                destination_leaf,
                &source.parent.handle,
                &source.leaf,
            )
            .and_then(|()| self.verify_leaf_binding(source))
            .and_then(|()| self.sync_parent_binding(&source.parent));
            if rollback.is_ok() {
                return io::Error::new(
                    cause.kind(),
                    format!(
                        "{cause}; exact retained source rolled back to {}",
                        source.path.display()
                    ),
                );
            }
        }
        self.rollback_noreplace_or_isolate(source, destination_parent, destination_leaf, cause)
    }

    fn retained_leaf_at(
        &self,
        retained: &File,
        identity: &RetainedFileIdentity,
        parent: &RetainedParentBinding,
        leaf: &OsStr,
        path: &Path,
    ) -> io::Result<RetainedAuthorityLeaf> {
        let rebound = RetainedAuthorityLeaf {
            parent: RetainedParentBinding {
                handle: parent.handle.try_clone()?,
                relative_path: parent.relative_path.clone(),
                identity: parent.identity.clone(),
            },
            leaf: leaf.to_os_string(),
            path: path.to_path_buf(),
            file: retained.try_clone()?,
            identity: identity.clone(),
        };
        self.verify_leaf_binding(&rebound)?;
        Ok(rebound)
    }

    fn isolate_authority_name(&self, path: &Path) -> io::Result<RetainedCleanupDebt> {
        const ISOLATION_ATTEMPTS: usize = 32;
        let mut isolated_paths = Vec::new();
        for _ in 0..ISOLATION_ATTEMPTS {
            match self.remove_authority_file(path, |_, _| Ok(())) {
                Ok(debt) => isolated_paths.extend(debt.into_paths()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(RetainedCleanupDebt { isolated_paths });
                }
                #[cfg(unix)]
                Err(authority_error) => {
                    match self.quarantine_any_name_noreplace(path, "untrusted-occupant") {
                        Ok(path) => isolated_paths.push(path),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            return Ok(RetainedCleanupDebt { isolated_paths });
                        }
                        Err(error) => {
                            return Err(io::Error::new(
                                error.kind(),
                                format!(
                                    "retained authority isolation failed ({authority_error}); untrusted-name isolation also failed: {error}"
                                ),
                            ));
                        }
                    }
                }
                #[cfg(not(unix))]
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "bounded retained authority isolation was continuously repopulated",
        ))
    }

    fn fail_publication_after_move(
        &self,
        source: &Path,
        destination: &Path,
        cause: io::Error,
    ) -> io::Error {
        let destination_isolation = self.isolate_authority_name(destination);
        let source_isolation = self.isolate_authority_name(source);
        io::Error::other(
            format!(
                "{cause}; post-move destination isolation result: {destination_isolation:?}; source placeholder isolation result: {source_isolation:?}"
            ),
        )
    }

    fn publish_retained_leaf_noreplace(
        &self,
        source: &RetainedAuthorityLeaf,
        destination_parent: &RetainedParentBinding,
        destination_leaf: &OsStr,
        destination_path: &Path,
    ) -> io::Result<RetainedCleanupDebt> {
        self.verify_leaf_binding(source)?;
        self.verify_destination_state(destination_parent, destination_leaf, None)?;

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            self.verify_leaf_binding(source)?;
            self.verify_destination_state(destination_parent, destination_leaf, None)?;
            if let Err(error) = platform::link_exact_noreplace(
                &source.file,
                &destination_parent.handle,
                destination_leaf,
            ) {
                let destination_isolation = self.isolate_authority_name(destination_path);
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "{error}; exact-handle no-replace publication failed and destination isolation result was {destination_isolation:?}"
                    ),
                ));
            }

            // Linearization point: successful fd-bound no-replace link creation.
            // The destination names the exact retained inode from this point on;
            // the mutable source name is never used to select what is published.
            let commit = (|| {
                self.verify_retained_authority_binding(
                    destination_path,
                    &source.file,
                    &source.identity,
                )?;
                self.sync_parent_binding(destination_parent)?;
                self.verify_retained_authority_binding(
                    destination_path,
                    &source.file,
                    &source.identity,
                )?;

                // Exact unlink-by-handle is unavailable on supported Unix. Retire
                // the old source namespace only after publication; the exact old
                // link remains discoverable under Store quarantine as cleanup
                // debt instead of deleting a separately validated mutable name.
                let published_source = self.retained_leaf_at(
                    &source.file,
                    &source.identity,
                    &source.parent,
                    &source.leaf,
                    &source.path,
                )?;
                let debt = self.remove_retained_authority_leaf(published_source, |_, _| Ok(()))?;
                if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?
                    .is_some()
                {
                    return Err(Self::authority_identity_changed(
                        "exact-handle publication source retirement",
                    ));
                }
                if source.parent.identity != destination_parent.identity {
                    self.sync_parent_binding(&source.parent)?;
                }
                self.verify_retained_authority_binding(
                    destination_path,
                    &source.file,
                    &source.identity,
                )?;
                Ok(debt)
            })();
            commit.map_err(|error| {
                self.fail_publication_after_move(&source.path, destination_path, error)
            })
        }

        #[cfg(windows)]
        {
            platform::rename_noreplace(
                &source.parent.handle,
                &source.leaf,
                &destination_parent.handle,
                destination_leaf,
                &source.file,
            )?;
            let commit = (|| {
                self.verify_retained_authority_binding(
                    destination_path,
                    &source.file,
                    &source.identity,
                )?;
                if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?
                    .is_some()
                {
                    return Err(Self::authority_identity_changed(
                        "Windows publication source cleanup",
                    ));
                }
                self.sync_parent_binding(destination_parent)?;
                if source.parent.identity != destination_parent.identity {
                    self.sync_parent_binding(&source.parent)?;
                }
                Ok(RetainedCleanupDebt::none())
            })();
            return commit.map_err(|error| {
                self.fail_publication_after_move(&source.path, destination_path, error)
            });
        }

        #[cfg(all(
            unix,
            not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos"
            ))
        ))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "retained no-replace publication requires exact handle-bound move or verified placeholder exchange",
            ))
        }

        #[cfg(not(any(unix, windows)))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "exact retained no-replace publication is unsupported on this platform",
            ))
        }
    }

    /// Publish an exact retained handle at a vacant destination without using a
    /// source namespace name. On Unix this is a hard-link publication and the
    /// caller must keep the old named link as discoverable cleanup debt. The
    /// successful fd-bound no-replace operation is the linearization point.
    fn publish_retained_handle_noreplace(
        &self,
        retained: &File,
        expected: &RetainedFileIdentity,
        destination: &Path,
    ) -> io::Result<()> {
        Self::verify_retained_handle(retained, expected)?;
        let (destination_parent, destination_leaf) = self.open_parent_bound(destination, true)?;
        self.verify_destination_state(&destination_parent, &destination_leaf, None)?;

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        let publication =
            platform::link_exact_noreplace(retained, &destination_parent.handle, &destination_leaf);

        #[cfg(windows)]
        let publication = platform::rename_noreplace(
            &destination_parent.handle,
            OsStr::new(""),
            &destination_parent.handle,
            &destination_leaf,
            retained,
        );

        #[cfg(all(
            unix,
            not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos"
            ))
        ))]
        let publication: io::Result<()> = Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exact retained handle publication is unsupported on this Unix target",
        ));

        #[cfg(not(any(unix, windows)))]
        let publication: io::Result<()> = Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exact retained handle publication is unsupported on this platform",
        ));

        if let Err(error) = publication {
            let isolation = self.isolate_authority_name(destination);
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; exact retained handle publication failed and destination isolation result was {isolation:?}"
                ),
            ));
        }
        self.verify_retained_authority_binding(destination, retained, expected)?;
        self.sync_parent_binding(&destination_parent)?;
        self.verify_retained_authority_binding(destination, retained, expected)
    }

    fn force_retained_placeholder_at(
        &self,
        retained: &File,
        expected: &RetainedFileIdentity,
        target: &Path,
    ) -> io::Result<RetainedCleanupDebt> {
        Self::verify_retained_handle(retained, expected)?;

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            const FORCE_ATTEMPTS: usize = 32;
            let (parent, leaf) = self.open_parent_bound(target, true)?;
            let nonce = Self::quarantine_nonce()?;
            let mut isolated_paths = Vec::new();
            for attempt in 0..FORCE_ATTEMPTS {
                match platform::link_exact_noreplace(retained, &parent.handle, &leaf) {
                    Ok(()) => {
                        self.verify_retained_authority_binding(target, retained, expected)?;
                        self.sync_parent_binding(&parent)?;
                        return Ok(RetainedCleanupDebt { isolated_paths });
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }

                let staging = Self::quarantine_leaf("rollback-placeholder", nonce, attempt);
                match platform::link_exact_noreplace(retained, &parent.handle, &staging) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(error),
                }
                match platform::exchange(&parent.handle, &staging, &parent.handle, &leaf) {
                    Ok(()) => {
                        isolated_paths.push(parent.relative_path.join(&staging));
                        if self
                            .verify_retained_authority_binding(target, retained, expected)
                            .is_ok()
                        {
                            self.sync_parent_binding(&parent)?;
                            return Ok(RetainedCleanupDebt { isolated_paths });
                        }
                        let debt = self.isolate_authority_name(target)?;
                        isolated_paths.extend(debt.into_paths());
                    }
                    Err(error) => {
                        isolated_paths.push(parent.relative_path.join(staging));
                        if error.kind() != io::ErrorKind::NotFound {
                            return Err(error);
                        }
                    }
                }
            }
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "bounded exact Store placeholder installation was continuously substituted",
            ))
        }

        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        )))]
        {
            let debt = self.isolate_authority_name(target)?;
            self.publish_retained_handle_noreplace(retained, expected, target)?;
            Ok(debt)
        }
    }

    #[cfg(windows)]
    fn replace_existing_authority_windows(
        &self,
        mut temp: RetainedAuthorityLeaf,
        destination_parent: RetainedParentBinding,
        destination_leaf: OsString,
        destination_file: File,
        destination_identity: RetainedFileIdentity,
        bytes: &[u8],
    ) -> io::Result<RetainedCleanupDebt> {
        let old_destination = self.quarantine_named_leaf_noreplace(
            &destination_parent,
            &destination_leaf,
            &destination_file,
            &destination_identity,
            "replaced-destination",
        )?;
        let old_leaf = old_destination.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained destination quarantine has no leaf",
            )
        })?;
        if let Err(error) = platform::rename_noreplace(
            &temp.parent.handle,
            &temp.leaf,
            &destination_parent.handle,
            &destination_leaf,
            &temp.file,
        ) {
            let rollback = platform::rename_noreplace(
                &destination_parent.handle,
                old_leaf,
                &destination_parent.handle,
                &destination_leaf,
                &destination_file,
            )
            .and_then(|()| {
                self.verify_destination_state(
                    &destination_parent,
                    &destination_leaf,
                    Some(&destination_identity),
                )
            })
            .and_then(|()| self.sync_parent_binding(&destination_parent));
            return Err(io::Error::new(
                io::ErrorKind::Other,
                match rollback {
                    Ok(()) => format!(
                        "{error}; exact previous destination was restored and replacement remains isolated at {}",
                        temp.path.display()
                    ),
                    Err(rollback) => format!(
                        "{error}; previous destination remained isolated at {} and rollback failed: {rollback}",
                        old_destination.display()
                    ),
                },
            ));
        }
        let commit = (|| {
            if Self::direct_authority_identity(&destination_parent.handle, &destination_leaf)?
                != temp.identity
                || Self::direct_optional_authority_identity(&temp.parent.handle, &temp.leaf)?
                    .is_some()
                || Self::direct_authority_identity(&destination_parent.handle, old_leaf)?
                    != destination_identity
            {
                return Err(Self::authority_identity_changed(
                    "Windows replacement commit",
                ));
            }
            Self::verify_retained_handle(&temp.file, &temp.identity)?;
            Self::verify_retained_handle(&destination_file, &destination_identity)?;
            Self::verify_exact_bytes(&mut temp.file, bytes)?;
            self.sync_parent_binding(&destination_parent)
        })();
        if let Err(error) = commit {
            let replacement_rollback = platform::rename_noreplace(
                &destination_parent.handle,
                &destination_leaf,
                &temp.parent.handle,
                &temp.leaf,
                &temp.file,
            )
            .and_then(|()| self.verify_leaf_binding(&temp));
            let destination_rollback = platform::rename_noreplace(
                &destination_parent.handle,
                old_leaf,
                &destination_parent.handle,
                &destination_leaf,
                &destination_file,
            )
            .and_then(|()| {
                self.verify_destination_state(
                    &destination_parent,
                    &destination_leaf,
                    Some(&destination_identity),
                )
            });
            let synchronization = self.sync_parent_binding(&destination_parent);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{error}; Windows handle-bound rollback results: replacement={replacement_rollback:?}, destination={destination_rollback:?}, sync={synchronization:?}"
                ),
            ));
        }
        Ok(RetainedCleanupDebt::one(old_destination))
    }

    fn replace_authority_atomically<H>(
        &self,
        path: &Path,
        bytes: &[u8],
        before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&Self, &Path, &Path) -> io::Result<()>,
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let nonce = Self::quarantine_nonce()?;
        self.replace_authority_atomically_with_candidates(
            path,
            bytes,
            move |attempt| {
                parent.join(format!(
                    ".forge-retained-replacement-{}-{nonce}-{attempt}.quarantine",
                    std::process::id()
                ))
            },
            before_commit,
        )
    }

    fn replace_authority_atomically_with_candidates<F, H>(
        &self,
        path: &Path,
        bytes: &[u8],
        temp_candidate: F,
        mut before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        F: FnMut(usize) -> PathBuf,
        H: FnMut(&Self, &Path, &Path) -> io::Result<()>,
    {
        use std::io::Write as _;

        let parent_path = path.parent().unwrap_or_else(|| Path::new(""));
        if !parent_path.as_os_str().is_empty() {
            self.create_dir_all(parent_path)?;
        }
        let (destination_parent, destination_leaf) = self.open_parent_bound(path, true)?;
        let destination = match platform::open_file(
            &destination_parent.handle,
            &destination_leaf,
            platform::FileMode::ReadDeleteRename,
        ) {
            Ok(file) => {
                Self::validate_leaf(&file, RetainedLeafPolicy::Authority)?;
                let identity = Self::identity_of(&file)?;
                if Self::direct_authority_identity(&destination_parent.handle, &destination_leaf)?
                    != identity
                {
                    return Err(Self::authority_identity_changed("destination open"));
                }
                Some((file, identity))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let mut temp = self.create_quarantine_leaf(parent_path, temp_candidate)?;
        if temp.parent.identity != destination_parent.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained replacement temporary parent changed identity",
            ));
        }
        if let Err(error) = self
            .mutate_authority_file(&temp.path, &mut temp.file, &temp.identity, |file| {
                file.write_all(bytes)?;
                file.sync_all()
            })
            .and_then(|()| Self::verify_exact_bytes(&mut temp.file, bytes))
            .and_then(|()| self.verify_mutable_leaf_binding(&temp))
            .and_then(|()| self.sync_parent_binding(&temp.parent))
        {
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; partial replacement retained as cleanup debt at {}",
                    temp.path.display()
                ),
            ));
        }
        let expected_destination = destination.as_ref().map(|(_, identity)| identity);
        if let Err(error) = before_commit(self, &temp.path, path)
            .and_then(|()| self.verify_leaf_binding(&temp))
            .and_then(|()| {
                self.verify_destination_state(
                    &destination_parent,
                    &destination_leaf,
                    expected_destination,
                )
            })
            .and_then(|()| Self::verify_exact_bytes(&mut temp.file, bytes))
        {
            let destination_isolation = expected_destination
                .is_none()
                .then(|| self.isolate_authority_name(path));
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; exact replacement remains isolated at {} or as a bounded retained orphan; create-new destination isolation result: {destination_isolation:?}",
                    temp.path.display()
                ),
            ));
        }

        #[cfg(target_os = "macos")]
        if destination.is_none() {
            if let Err(error) =
                self.move_retained_file_noreplace(&temp.path, &temp.file, &temp.identity, path)
            {
                return Err(self.fail_publication_after_move(&temp.path, path, error));
            }
            let commit = (|| {
                self.verify_mutable_authority_binding(path, &temp.file, &temp.identity)?;
                Self::verify_exact_bytes(&mut temp.file, bytes)?;
                self.verify_mutable_authority_binding(path, &temp.file, &temp.identity)
            })();
            if let Err(error) = commit {
                return Err(self.fail_publication_after_move(&temp.path, path, error));
            }
            return Ok(RetainedCleanupDebt::none());
        }

        if let Some((destination_file, destination_identity)) = destination {
            Self::verify_retained_handle(&destination_file, &destination_identity)?;
            #[cfg(windows)]
            {
                return self.replace_existing_authority_windows(
                    temp,
                    destination_parent,
                    destination_leaf,
                    destination_file,
                    destination_identity,
                    bytes,
                );
            }
            #[cfg(not(windows))]
            {
                if let Err(error) = platform::exchange(
                    &temp.parent.handle,
                    &temp.leaf,
                    &destination_parent.handle,
                    &destination_leaf,
                ) {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "{error}; exact replacement remains isolated at {}",
                            temp.path.display()
                        ),
                    ));
                }
                let commit = (|| {
                    if Self::direct_authority_identity(
                        &destination_parent.handle,
                        &destination_leaf,
                    )? != temp.identity
                        || Self::direct_authority_identity(&temp.parent.handle, &temp.leaf)?
                            != destination_identity
                    {
                        return Err(Self::authority_identity_changed("exchange commit"));
                    }
                    Self::verify_retained_handle(&temp.file, &temp.identity)?;
                    Self::verify_retained_handle(&destination_file, &destination_identity)?;
                    Self::verify_exact_bytes(&mut temp.file, bytes)?;
                    self.sync_parent_binding(&destination_parent)?;
                    if Self::direct_authority_identity(
                        &destination_parent.handle,
                        &destination_leaf,
                    )? != temp.identity
                        || Self::direct_authority_identity(&temp.parent.handle, &temp.leaf)?
                            != destination_identity
                    {
                        return Err(Self::authority_identity_changed("post-sync exchange"));
                    }
                    Ok(())
                })();
                if let Err(error) = commit {
                    let previous_at_temp =
                        Self::direct_optional_authority_identity(&temp.parent.handle, &temp.leaf)
                            .is_ok_and(|identity| identity.as_ref() == Some(&destination_identity));
                    let destination_present = Self::direct_optional_authority_identity(
                        &destination_parent.handle,
                        &destination_leaf,
                    )
                    .is_ok_and(|identity| identity.is_some());
                    if previous_at_temp && destination_present {
                        let rollback = platform::exchange(
                            &temp.parent.handle,
                            &temp.leaf,
                            &destination_parent.handle,
                            &destination_leaf,
                        )
                        .and_then(|()| {
                            self.verify_destination_state(
                                &destination_parent,
                                &destination_leaf,
                                Some(&destination_identity),
                            )
                        })
                        .and_then(|()| self.sync_parent_binding(&destination_parent));
                        return Err(io::Error::other(
                            match rollback {
                                Ok(()) => format!(
                                    "{error}; exact previous destination was restored while the replacement remained isolated or became a bounded retained orphan"
                                ),
                                Err(rollback) => format!(
                                    "{error}; exact previous destination rollback failed: {rollback}"
                                ),
                            },
                        ));
                    }
                    return Err(self.rollback_exchange_or_isolate(
                        &temp,
                        &destination_parent,
                        &destination_leaf,
                        &error,
                    ));
                }
                return Ok(RetainedCleanupDebt::one(temp.path));
            }
        }

        let publication_debt = self.publish_retained_leaf_noreplace(
            &temp,
            &destination_parent,
            &destination_leaf,
            path,
        )?;
        let commit = (|| {
            self.verify_retained_authority_binding(path, &temp.file, &temp.identity)?;
            Self::verify_exact_bytes(&mut temp.file, bytes)?;
            self.sync_parent_binding(&destination_parent)?;
            self.verify_retained_authority_binding(path, &temp.file, &temp.identity)
        })();
        if let Err(error) = commit {
            return Err(self.fail_publication_after_move(&temp.path, path, error));
        }
        Ok(publication_debt)
    }

    fn rename_authority_file_noreplace<H>(
        &self,
        from: &Path,
        to: &Path,
        mut before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&Self, &Path, &Path) -> io::Result<()>,
    {
        let source = self.open_leaf_bound(
            from,
            RetainedLeafPolicy::Authority,
            platform::FileMode::ReadDeleteRename,
            false,
        )?;
        let (destination_parent, destination_leaf) = self.open_parent_bound(to, true)?;
        self.verify_destination_state(&destination_parent, &destination_leaf, None)?;
        let prepared = before_commit(self, from, to)
            .and_then(|()| self.verify_leaf_binding(&source))
            .and_then(|()| {
                self.verify_destination_state(&destination_parent, &destination_leaf, None)
            });
        if let Err(error) = prepared {
            let destination_isolation = self.isolate_authority_name(to);
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; no-replace destination changed after vacancy validation; destination isolation result: {destination_isolation:?}"
                ),
            ));
        }
        self.publish_retained_leaf_noreplace(&source, &destination_parent, &destination_leaf, to)
    }

    fn move_retained_file_noreplace(
        &self,
        from: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
        to: &Path,
    ) -> io::Result<()> {
        self.move_retained_file_noreplace_with_hooks(
            from,
            retained,
            expected,
            to,
            |_, _, _| Ok(()),
            |_, _, _| Ok(()),
        )
    }

    fn move_retained_file_noreplace_with_hooks<B, A>(
        &self,
        from: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
        to: &Path,
        mut before_move: B,
        mut after_move: A,
    ) -> io::Result<()>
    where
        B: FnMut(&Self, &Path, &Path) -> io::Result<()>,
        A: FnMut(&Self, &Path, &Path) -> io::Result<()>,
    {
        if from == to {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained no-replace move source and destination are identical",
            ));
        }
        Self::validate_leaf(retained, RetainedLeafPolicy::MutableAuthority)?;
        if Self::identity_of(retained)? != *expected {
            return Err(Self::authority_identity_changed("move source handle"));
        }
        let source = self.open_leaf_bound(
            from,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadDeleteRename,
            false,
        )?;
        if source.identity != *expected || Self::identity_of(&source.file)? != *expected {
            return Err(Self::authority_identity_changed("move source namespace"));
        }
        let (destination_parent, destination_leaf) = self.open_parent_bound(to, true)?;
        self.verify_destination_state(&destination_parent, &destination_leaf, None)?;
        before_move(self, from, to)?;
        self.verify_mutable_leaf_binding(&source)?;
        Self::validate_leaf(retained, RetainedLeafPolicy::MutableAuthority)?;
        if Self::identity_of(retained)? != *expected {
            return Err(Self::authority_identity_changed("move source handle"));
        }
        self.verify_destination_state(&destination_parent, &destination_leaf, None)?;

        // Linearization point: one atomic no-replace namespace move. The source
        // is a Store-created single-link staging name bound to the exact retained
        // handle immediately before and after this operation.
        platform::rename_noreplace(
            &source.parent.handle,
            &source.leaf,
            &destination_parent.handle,
            &destination_leaf,
            retained,
        )?;
        after_move(self, from, to)?;

        // A failure from this point leaves the committed destination in place.
        // Retry must recognize and verify it; deleting or isolating it could erase
        // the only durable publication after the linearization point.
        self.verify_mutable_authority_binding(to, retained, expected)?;
        if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?.is_some()
        {
            return Err(Self::authority_identity_changed("move source retirement"));
        }
        self.sync_parent_binding(&destination_parent)?;
        if source.parent.identity != destination_parent.identity {
            self.sync_parent_binding(&source.parent)?;
        }
        self.verify_mutable_authority_binding(to, retained, expected)?;
        if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?.is_some()
        {
            return Err(Self::authority_identity_changed("move source retirement"));
        }
        self.verify_parent_binding(&source.parent)?;
        self.verify_parent_binding(&destination_parent)
    }

    fn write_new_authority_file_synced(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write as _;

        let mut leaf = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadWriteNewDelete,
            true,
        )?;
        let result = self
            .mutate_authority_file(&leaf.path, &mut leaf.file, &leaf.identity, |file| {
                file.write_all(bytes)?;
                file.sync_all()
            })
            .and_then(|()| Self::verify_exact_bytes(&mut leaf.file, bytes))
            .and_then(|()| self.verify_mutable_leaf_binding(&leaf))
            .and_then(|()| self.sync_parent_binding(&leaf.parent));
        if let Err(error) = result {
            let cleanup = self.remove_retained_authority_leaf(leaf, |_, _| Ok(()));
            return Err(io::Error::other(match cleanup {
                Ok(debt) => format!(
                    "{error}; failed write was isolated as bounded cleanup debt {:?}",
                    debt.isolated_paths
                ),
                Err(cleanup) => {
                    format!("{error}; failed write isolation also failed: {cleanup}")
                }
            }));
        }
        Ok(())
    }

    fn remove_authority_file<H>(
        &self,
        path: &Path,
        before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&Self, &Path) -> io::Result<()>,
    {
        let source = self.open_leaf_bound(
            path,
            RetainedLeafPolicy::Authority,
            platform::FileMode::ReadDeleteRename,
            false,
        )?;
        self.remove_retained_authority_leaf(source, before_commit)
    }

    fn remove_retained_authority_leaf<H>(
        &self,
        source: RetainedAuthorityLeaf,
        mut before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&Self, &Path) -> io::Result<()>,
    {
        before_commit(self, &source.path)?;
        self.verify_leaf_binding(&source)?;

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            let parent_path = source.path.parent().unwrap_or_else(|| Path::new(""));
            let nonce = Self::quarantine_nonce()?;
            let placeholder = self.create_quarantine_leaf(parent_path, |attempt| {
                parent_path.join(Self::quarantine_leaf("delete-swap", nonce, attempt))
            })?;
            if placeholder.parent.identity != source.parent.identity {
                return Err(Self::authority_identity_changed(
                    "delete placeholder parent",
                ));
            }
            placeholder.file.sync_all()?;
            self.sync_parent_binding(&source.parent)?;
            platform::exchange(
                &source.parent.handle,
                &source.leaf,
                &placeholder.parent.handle,
                &placeholder.leaf,
            )?;
            let exchanged =
                Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)
                    .is_ok_and(|identity| identity.as_ref() == Some(&placeholder.identity))
                    && Self::direct_optional_authority_identity(
                        &placeholder.parent.handle,
                        &placeholder.leaf,
                    )
                    .is_ok_and(|identity| identity.as_ref() == Some(&source.identity));
            if !exchanged {
                let source_is_placeholder =
                    Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)
                        .is_ok_and(|identity| identity.as_ref() == Some(&placeholder.identity));
                let quarantine_present = Self::direct_optional_authority_identity(
                    &placeholder.parent.handle,
                    &placeholder.leaf,
                )
                .is_ok_and(|identity| identity.is_some());
                let rollback = if source_is_placeholder && quarantine_present {
                    platform::exchange(
                        &placeholder.parent.handle,
                        &placeholder.leaf,
                        &source.parent.handle,
                        &source.leaf,
                    )
                    .and_then(|()| self.sync_parent_binding(&source.parent))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "delete exchange state could not be rolled back by name",
                    ))
                };
                return Err(io::Error::other(
                    match rollback {
                        Ok(()) => "retained authority delete exchange changed identity and its namespace state was rolled back; the exact source remains a bounded retained orphan".to_owned(),
                        Err(rollback) => format!(
                            "retained authority delete exchange changed identity and rollback failed: {rollback}"
                        ),
                    },
                ));
            }
            let placeholder_debt = match self.quarantine_named_leaf_noreplace(
                &source.parent,
                &source.leaf,
                &placeholder.file,
                &placeholder.identity,
                "delete-placeholder",
            ) {
                Ok(path) => path,
                Err(error) => {
                    let rollback = platform::exchange(
                        &placeholder.parent.handle,
                        &placeholder.leaf,
                        &source.parent.handle,
                        &source.leaf,
                    )
                    .and_then(|()| self.verify_leaf_binding(&source))
                    .and_then(|()| self.sync_parent_binding(&source.parent));
                    return Err(io::Error::other(
                        match rollback {
                            Ok(()) => format!(
                                "{error}; exact delete source was restored to {}",
                                source.path.display()
                            ),
                            Err(rollback) => format!(
                                "{error}; delete rollback failed while exact source remained isolated at {}: {rollback}",
                                placeholder.path.display()
                            ),
                        },
                    ));
                }
            };
            if Self::direct_optional_authority_identity(&source.parent.handle, &source.leaf)?
                .is_some()
                || Self::direct_authority_identity(&placeholder.parent.handle, &placeholder.leaf)?
                    != source.identity
            {
                return Err(Self::authority_identity_changed("delete final state"));
            }
            self.sync_parent_binding(&source.parent)?;
            Ok(RetainedCleanupDebt::two(placeholder.path, placeholder_debt))
        }

        #[cfg(all(
            unix,
            not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos"
            ))
        ))]
        {
            let path = self.quarantine_named_leaf_noreplace(
                &source.parent,
                &source.leaf,
                &source.file,
                &source.identity,
                "delete",
            )?;
            return Ok(RetainedCleanupDebt::one(path));
        }

        #[cfg(windows)]
        {
            let parent = source.parent;
            let leaf = source.leaf;
            platform::remove_exact(source.file)?;
            if Self::direct_optional_authority_identity(&parent.handle, &leaf)?.is_some() {
                return Err(Self::authority_identity_changed("handle-bound delete"));
            }
            self.sync_parent_binding(&parent)?;
            return Ok(RetainedCleanupDebt::none());
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = source;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "exact retained authority deletion is unsupported on this platform",
            ))
        }
    }

    /// Generic retained reads remain source reads. Store authority call sites
    /// must use `open_leaf_read(..., Authority)`.
    pub fn open_read(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_read(path, RetainedLeafPolicy::SourceRead)
    }

    pub(crate) fn open_read_write(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_read_write_existing(path)
    }

    pub fn open_read_write_create(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_read_write_create_authority(path)
    }

    pub fn open_write_new(&self, path: &Path) -> io::Result<File> {
        self.open_leaf_write_new_authority(path)
    }

    pub fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        self.open_leaf_read(path, RetainedLeafPolicy::SourceRead)?
            .metadata()
    }

    pub fn exists(&self, path: &Path) -> io::Result<bool> {
        match self.open_read(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl RetainedAuthorityDirectory<'_> {
    fn validate(&self) -> io::Result<()> {
        if self.directory.identity()? == self.identity {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sealed retained authority directory changed identity",
            ))
        }
    }

    pub(crate) fn replace_file_with_validation<H>(
        &self,
        path: &Path,
        bytes: &[u8],
        validation: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self
            .directory
            .replace_authority_atomically(path, bytes, validation);
        self.validate()?;
        result
    }

    pub(crate) fn write_new_file_synced(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.validate()?;
        let result = self.directory.write_new_authority_file_synced(path, bytes);
        self.validate()?;
        result
    }

    pub(crate) fn rename_file_noreplace_with_validation<H>(
        &self,
        from: &Path,
        to: &Path,
        validation: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self
            .directory
            .rename_authority_file_noreplace(from, to, validation);
        self.validate()?;
        result
    }

    pub(crate) fn move_retained_file_noreplace(
        &self,
        from: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
        to: &Path,
    ) -> io::Result<()> {
        self.validate()?;
        let result = self
            .directory
            .move_retained_file_noreplace(from, retained, expected, to);
        self.validate()?;
        result
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn move_retained_file_noreplace_with_validation<H>(
        &self,
        from: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
        to: &Path,
        validation: H,
    ) -> io::Result<()>
    where
        H: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self.directory.move_retained_file_noreplace_with_hooks(
            from,
            retained,
            expected,
            to,
            validation,
            |_, _, _| Ok(()),
        );
        self.validate()?;
        result
    }

    pub(crate) fn move_mutable_file_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.validate()?;
        let source = self.directory.open_leaf_bound(
            from,
            RetainedLeafPolicy::MutableAuthority,
            platform::FileMode::ReadDeleteRename,
            false,
        )?;
        let result =
            self.directory
                .move_retained_file_noreplace(from, &source.file, &source.identity, to);
        self.validate()?;
        result
    }

    pub(crate) fn remove_file_with_validation<H>(
        &self,
        path: &Path,
        validation: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&RetainedDirectory, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self.directory.remove_authority_file(path, validation);
        self.validate()?;
        result
    }

    pub(crate) fn remove_empty_directory(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        let result = self.directory.remove_authority_empty_directory(path);
        self.validate()?;
        result
    }

    pub(crate) fn remove_reclaimable_file_with_validation<H>(
        &self,
        path: &Path,
        validation: H,
    ) -> io::Result<()>
    where
        H: FnMut(&RetainedDirectory, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self
            .directory
            .remove_authority_reclaimable_file(path, validation);
        self.validate()?;
        result
    }

    pub(crate) fn publish_retained_handle_noreplace(
        &self,
        retained: &File,
        expected: &RetainedFileIdentity,
        destination: &Path,
    ) -> io::Result<()> {
        self.validate()?;
        let result =
            self.directory
                .publish_retained_handle_noreplace(retained, expected, destination);
        self.validate()?;
        result
    }

    pub(crate) fn isolate_name(&self, path: &Path) -> io::Result<RetainedCleanupDebt> {
        self.validate()?;
        let result = self.directory.isolate_authority_name(path);
        self.validate()?;
        result
    }

    pub(crate) fn force_retained_placeholder_at(
        &self,
        retained: &File,
        expected: &RetainedFileIdentity,
        target: &Path,
    ) -> io::Result<RetainedCleanupDebt> {
        self.validate()?;
        let result = self
            .directory
            .force_retained_placeholder_at(retained, expected, target);
        self.validate()?;
        result
    }

    #[cfg(test)]
    fn replace_file_with_candidates<F, H>(
        &self,
        path: &Path,
        bytes: &[u8],
        candidates: F,
        before_commit: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        F: FnMut(usize) -> PathBuf,
        H: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self.directory.replace_authority_atomically_with_candidates(
            path,
            bytes,
            candidates,
            before_commit,
        );
        self.validate()?;
        result
    }

    #[cfg(test)]
    fn rename_file_noreplace_with_hook<H>(
        &self,
        from: &Path,
        to: &Path,
        hook: H,
    ) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self
            .directory
            .rename_authority_file_noreplace(from, to, hook);
        self.validate()?;
        result
    }

    #[cfg(test)]
    pub(crate) fn move_retained_file_noreplace_with_hooks<B, A>(
        &self,
        from: &Path,
        retained: &File,
        expected: &RetainedFileIdentity,
        to: &Path,
        before_move: B,
        after_move: A,
    ) -> io::Result<()>
    where
        B: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
        A: FnMut(&RetainedDirectory, &Path, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self.directory.move_retained_file_noreplace_with_hooks(
            from,
            retained,
            expected,
            to,
            before_move,
            after_move,
        );
        self.validate()?;
        result
    }

    #[cfg(test)]
    fn remove_file_with_hook<H>(&self, path: &Path, hook: H) -> io::Result<RetainedCleanupDebt>
    where
        H: FnMut(&RetainedDirectory, &Path) -> io::Result<()>,
    {
        self.validate()?;
        let result = self.directory.remove_authority_file(path, hook);
        self.validate()?;
        result
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::io::{AsRawFd as _, FromRawFd as _};

    pub enum FileMode {
        Read,
        ReadWrite,
        ReadWriteCreate,
        WriteNew,
        ReadDeleteRename,
        ReadWriteNewDelete,
    }

    fn name(value: &OsStr) -> io::Result<CString> {
        CString::new(value.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path component"))
    }

    fn openat(parent: &File, value: &OsStr, flags: i32, mode: libc::mode_t) -> io::Result<File> {
        let value = name(value)?;
        let promoted_mode: libc::c_uint = mode.into();
        // SAFETY: parent and CString remain valid for the call; successful fd ownership transfers.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), value.as_ptr(), flags, promoted_mode) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: `fd` is newly owned after successful openat.
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    pub fn open_directory(parent: &File, value: &OsStr) -> io::Result<File> {
        openat(
            parent,
            value,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )
    }

    pub fn open_directory_delete(parent: &File, value: &OsStr) -> io::Result<File> {
        open_directory(parent, value)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn clear_readdir_errno() {
        // SAFETY: the platform accessor returns this thread's errno slot.
        unsafe { *libc::__errno_location() = 0 };
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    fn clear_readdir_errno() {
        // SAFETY: the platform accessor returns this thread's errno slot.
        unsafe { *libc::__error() = 0 };
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    fn clear_readdir_errno() {}

    pub fn directory_entry_count(directory: &File, maximum: usize) -> io::Result<usize> {
        // SAFETY: dup creates an owned descriptor and fdopendir takes ownership.
        let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
        if duplicate < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: duplicate is a newly owned directory descriptor.
        let stream = unsafe { libc::fdopendir(duplicate) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: fdopendir did not take ownership on failure.
            unsafe { libc::close(duplicate) };
            return Err(error);
        }
        // SAFETY: stream remains valid until closed below.
        unsafe { libc::rewinddir(stream) };
        let result = (|| {
            let mut count = 0_usize;
            loop {
                clear_readdir_errno();
                // SAFETY: stream remains valid for this call.
                let entry = unsafe { libc::readdir(stream) };
                if entry.is_null() {
                    let error = io::Error::last_os_error();
                    return if error.raw_os_error() == Some(0) {
                        Ok(count)
                    } else {
                        Err(error)
                    };
                }
                // SAFETY: readdir returned a live dirent with a NUL-terminated name.
                let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if name != b"." && name != b".." {
                    count = count.saturating_add(1);
                    if count > maximum {
                        return Ok(count);
                    }
                }
            }
        })();
        // SAFETY: closedir releases the stream and duplicated descriptor exactly once.
        unsafe { libc::closedir(stream) };
        result
    }

    pub fn remove_empty_directory(
        parent: &File,
        value: &OsStr,
        retained: File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        if RetainedDirectory::identity_of(&retained)? != *expected {
            return Err(RetainedDirectory::authority_identity_changed(
                "empty-directory retained handle",
            ));
        }
        let value = name(value)?;
        // SAFETY: parent and CString remain valid for the descriptor-relative call.
        let result =
            unsafe { libc::unlinkat(parent.as_raw_fd(), value.as_ptr(), libc::AT_REMOVEDIR) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        drop(retained);
        Ok(())
    }

    pub fn remove_named_file(
        parent: &File,
        value: &OsStr,
        retained: File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        if RetainedDirectory::identity_of(&retained)? != *expected {
            return Err(RetainedDirectory::authority_identity_changed(
                "reclaimable-file retained handle",
            ));
        }
        let value = name(value)?;
        // SAFETY: parent and CString remain valid for the descriptor-relative call.
        let result = unsafe { libc::unlinkat(parent.as_raw_fd(), value.as_ptr(), 0) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        drop(retained);
        Ok(())
    }

    pub fn open_or_create_directory(parent: &File, value: &OsStr) -> io::Result<File> {
        match open_directory(parent, value) {
            Ok(file) => Ok(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let value_c = name(value)?;
                // SAFETY: parent and CString remain valid for the call.
                let result = unsafe { libc::mkdirat(parent.as_raw_fd(), value_c.as_ptr(), 0o700) };
                if result < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                open_directory(parent, value)
            }
            Err(error) => Err(error),
        }
    }

    pub fn open_file(parent: &File, value: &OsStr, mode: FileMode) -> io::Result<File> {
        let flags = match mode {
            FileMode::Read | FileMode::ReadDeleteRename => libc::O_RDONLY,
            FileMode::ReadWrite => libc::O_RDWR,
            FileMode::ReadWriteCreate => libc::O_RDWR | libc::O_CREAT,
            FileMode::WriteNew => libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            FileMode::ReadWriteNewDelete => libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
        } | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            // Opening a FIFO can block before metadata validation. Nonblocking
            // is inert for regular files and lets every hostile special leaf
            // fail closed at `validate_leaf` instead of hanging the observer.
            | libc::O_NONBLOCK;
        openat(parent, value, flags, 0o600)
    }

    /// Publish one exact retained regular-file inode without consulting its
    /// mutable namespace name. `linkat(AT_EMPTY_PATH)` is attempted where the
    /// kernel exposes it; the process-owned fd alias is the unprivileged
    /// equivalent on supported Unix systems. Both forms are atomic no-replace.
    pub fn link_exact_noreplace(
        retained_source: &File,
        to_parent: &File,
        to: &OsStr,
    ) -> io::Result<()> {
        RetainedDirectory::validate_leaf(retained_source, RetainedLeafPolicy::Authority)?;
        let to = name(to)?;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let empty = b"\0";
            // SAFETY: both descriptors and C strings remain live for the call.
            let result = unsafe {
                libc::linkat(
                    retained_source.as_raw_fd(),
                    empty.as_ptr().cast(),
                    to_parent.as_raw_fd(),
                    to.as_ptr(),
                    libc::AT_EMPTY_PATH,
                )
            };
            if result == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if !error.raw_os_error().is_some_and(|code| {
                code == libc::EPERM
                    || code == libc::EINVAL
                    || code == libc::ENOENT
                    || code == libc::ENOSYS
            }) {
                return Err(error);
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let fd_alias = format!("/proc/self/fd/{}", retained_source.as_raw_fd());
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        let fd_alias = format!("/dev/fd/{}", retained_source.as_raw_fd());
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            let fd_alias = CString::new(fd_alias).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid retained fd alias")
            })?;
            // SAFETY: the process-owned fd alias resolves to retained_source for
            // the duration of this call, and the destination is descriptor-relative.
            let result = unsafe {
                libc::linkat(
                    libc::AT_FDCWD,
                    fd_alias.as_ptr(),
                    to_parent.as_raw_fd(),
                    to.as_ptr(),
                    libc::AT_SYMLINK_FOLLOW,
                )
            };
            if result == 0 {
                return Ok(());
            }
            Err(io::Error::last_os_error())
        }

        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        )))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "fd-bound no-replace regular-file publication is unsupported on this Unix target",
        ))
    }

    pub fn link_lifetime_anchor_noreplace(
        retained_source: &File,
        to_parent: &File,
        to: &OsStr,
    ) -> io::Result<()> {
        link_exact_noreplace(retained_source, to_parent, to)
    }

    /// Name move used only for quarantine or verified rollback after exact
    /// publication. It is never the publication linearization primitive.
    pub fn rename_noreplace(
        from_parent: &File,
        from: &OsStr,
        to_parent: &File,
        to: &OsStr,
        retained_source: &File,
    ) -> io::Result<()> {
        RetainedDirectory::validate_leaf(retained_source, RetainedLeafPolicy::Authority)?;
        let retained_identity = RetainedDirectory::identity_of(retained_source)?;
        let named = open_file(from_parent, from, FileMode::Read)?;
        RetainedDirectory::validate_leaf(&named, RetainedLeafPolicy::Authority)?;
        if RetainedDirectory::identity_of(&named)? != retained_identity {
            return Err(RetainedDirectory::authority_identity_changed(
                "quarantine or rollback source",
            ));
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "redox"
        ))]
        {
            rustix::fs::renameat_with(
                from_parent,
                Path::new(from),
                to_parent,
                Path::new(to),
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "redox"
        )))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace retained rename is unsupported on this Unix target",
        ))
    }

    pub fn rename_any_noreplace(
        from_parent: &File,
        from: &OsStr,
        to_parent: &File,
        to: &OsStr,
    ) -> io::Result<()> {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "redox"
        ))]
        {
            rustix::fs::renameat_with(
                from_parent,
                Path::new(from),
                to_parent,
                Path::new(to),
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "redox"
        )))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace untrusted-name isolation is unsupported on this Unix target",
        ))
    }

    pub fn exchange(
        left_parent: &File,
        left: &OsStr,
        right_parent: &File,
        right: &OsStr,
    ) -> io::Result<()> {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            rustix::fs::renameat_with(
                left_parent,
                Path::new(left),
                right_parent,
                Path::new(right),
                rustix::fs::RenameFlags::EXCHANGE,
            )
            .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        )))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic retained exchange is unsupported on this Unix target",
        ))
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, RawHandle};

    type Handle = *mut std::ffi::c_void;
    type NtStatus = i32;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_OPEN_IF: u32 = 3;
    const FILE_DIRECTORY_FILE: u32 = 0x1;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x40;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

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
    #[repr(C)]
    struct FileRenameInfo {
        replace_if_exists: u8,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
    }
    #[repr(C)]
    struct FileLinkInformation {
        replace_if_exists: u8,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
    }
    #[repr(C)]
    struct FileDispositionInfoEx {
        flags: u32,
    }
    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: i32,
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
        fn NtSetInformationFile(
            file_handle: Handle,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
        ) -> NtStatus;
        #[allow(clashing_extern_declarations)]
        fn NtQueryDirectoryFile(
            file_handle: Handle,
            event: Handle,
            apc_routine: *mut std::ffi::c_void,
            apc_context: *mut std::ffi::c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
            return_single_entry: u8,
            file_name: *mut std::ffi::c_void,
            restart_scan: u8,
        ) -> NtStatus;
        fn RtlNtStatusToDosError(status: NtStatus) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            handle: Handle,
            information_class: u32,
            information: *const std::ffi::c_void,
            buffer_size: u32,
        ) -> i32;
    }

    pub enum FileMode {
        Read,
        ReadWrite,
        ReadWriteCreate,
        WriteNew,
        ReadDeleteRename,
        ReadWriteNewDelete,
    }

    #[allow(clippy::cast_possible_wrap)] // RtlNtStatusToDosError returns a Win32 DOS error code (u32) that is documented to fit i32 for io::Error::from_raw_os_error.
    fn relative_open_with_share(
        parent: &File,
        value: &OsStr,
        access: u32,
        disposition: u32,
        options: u32,
        share_access: u32,
    ) -> io::Result<File> {
        let mut wide: Vec<u16> = value.encode_wide().collect();
        let byte_len = wide
            .len()
            .checked_mul(2)
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "path component too long")
            })?;
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
        // SAFETY: all pointers reference initialized storage for the duration of the call.
        let status = unsafe {
            NtCreateFile(
                std::ptr::from_mut(&mut handle),
                access | SYNCHRONIZE,
                std::ptr::from_mut(&mut attributes),
                std::ptr::from_mut(&mut io_status),
                std::ptr::null_mut(),
                0,
                share_access,
                disposition,
                options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            // SAFETY: pure NTSTATUS conversion.
            return Err(io::Error::from_raw_os_error(
                unsafe { RtlNtStatusToDosError(status) } as i32,
            ));
        }
        // SAFETY: successful NtCreateFile returned one newly-owned handle.
        Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
    }

    fn relative_open(
        parent: &File,
        value: &OsStr,
        access: u32,
        disposition: u32,
        options: u32,
    ) -> io::Result<File> {
        relative_open_with_share(parent, value, access, disposition, options, FILE_SHARE_ALL)
    }

    pub fn open_directory(parent: &File, value: &OsStr) -> io::Result<File> {
        relative_open(
            parent,
            value,
            FILE_READ_ATTRIBUTES | GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )
    }
    pub fn open_directory_delete(parent: &File, value: &OsStr) -> io::Result<File> {
        relative_open(
            parent,
            value,
            FILE_READ_ATTRIBUTES | GENERIC_READ | GENERIC_WRITE | DELETE_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn directory_entry_count(directory: &File, maximum: usize) -> io::Result<usize> {
        const STATUS_NO_MORE_FILES: NtStatus = 0x8000_0006_u32 as NtStatus;
        let mut count = 0_usize;
        let mut restart = 1_u8;
        loop {
            let mut buffer = vec![0_u8; 64 * 1024];
            let mut io_status = IoStatusBlock {
                value: IoStatusValue { status: 0 },
                information: 0,
            };
            // SAFETY: all pointers refer to live buffers for the duration of the call.
            let status = unsafe {
                NtQueryDirectoryFile(
                    directory.as_raw_handle(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::addr_of_mut!(io_status),
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(buffer.len()).expect("directory buffer fits u32"),
                    12,
                    0,
                    std::ptr::null_mut(),
                    restart,
                )
            };
            restart = 0;
            if status == STATUS_NO_MORE_FILES {
                return Ok(count);
            }
            if status < 0 {
                // SAFETY: pure NTSTATUS conversion.
                let code = unsafe { RtlNtStatusToDosError(status) };
                return Err(io::Error::from_raw_os_error(
                    i32::try_from(code).unwrap_or(i32::MAX),
                ));
            }
            let mut offset = 0_usize;
            while offset < io_status.information {
                let record = &buffer[offset..io_status.information];
                if record.len() < 12 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "truncated directory entry",
                    ));
                }
                let next = u32::from_ne_bytes(record[0..4].try_into().expect("slice length"));
                let name_bytes = usize::try_from(u32::from_ne_bytes(
                    record[8..12].try_into().expect("slice length"),
                ))
                .expect("u32 fits usize");
                let end = 12_usize.checked_add(name_bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory entry overflow")
                })?;
                if end > record.len() || name_bytes % 2 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid directory entry name",
                    ));
                }
                let wide = record[12..end]
                    .chunks_exact(2)
                    .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>();
                if wide.as_slice() != [u16::from(b'.')]
                    && wide.as_slice() != [u16::from(b'.'), u16::from(b'.')]
                {
                    count = count.saturating_add(1);
                    if count > maximum {
                        return Ok(count);
                    }
                }
                if next == 0 {
                    break;
                }
                offset = offset
                    .checked_add(usize::try_from(next).expect("u32 fits usize"))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "directory offset overflow")
                    })?;
            }
        }
    }

    pub fn remove_empty_directory(
        _parent: &File,
        _value: &OsStr,
        retained: File,
        expected: &RetainedFileIdentity,
    ) -> io::Result<()> {
        if RetainedDirectory::identity_of(&retained)? != *expected {
            return Err(RetainedDirectory::authority_identity_changed(
                "empty-directory retained handle",
            ));
        }
        remove_exact(retained)
    }
    pub fn open_or_create_directory(parent: &File, value: &OsStr) -> io::Result<File> {
        relative_open(
            parent,
            value,
            FILE_READ_ATTRIBUTES | GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN_IF,
            FILE_DIRECTORY_FILE,
        )
    }
    pub fn open_file(parent: &File, value: &OsStr, mode: FileMode) -> io::Result<File> {
        let (access, disposition) = match mode {
            FileMode::Read => (GENERIC_READ, FILE_OPEN),
            FileMode::ReadWrite => (GENERIC_READ | GENERIC_WRITE, FILE_OPEN),
            FileMode::ReadWriteCreate => (GENERIC_READ | GENERIC_WRITE, FILE_OPEN_IF),
            FileMode::WriteNew => (GENERIC_WRITE, FILE_CREATE),
            FileMode::ReadDeleteRename => (GENERIC_READ | DELETE_ACCESS, FILE_OPEN),
            FileMode::ReadWriteNewDelete => {
                (GENERIC_READ | GENERIC_WRITE | DELETE_ACCESS, FILE_CREATE)
            }
        };
        relative_open(parent, value, access, disposition, FILE_NON_DIRECTORY_FILE)
    }

    /// Lock handles withhold `FILE_SHARE_DELETE` to prevent replacement.
    pub fn open_retained_lock(parent: &File, value: &OsStr) -> io::Result<File> {
        relative_open_with_share(
            parent,
            value,
            GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN_IF,
            FILE_NON_DIRECTORY_FILE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
        )
    }

    #[allow(clippy::cast_ptr_alignment)]
    // the lifetime-anchor buffer is a freshly-allocated u8 vector that is intentionally unaligned; the header is written field-by-field before the call.
    #[allow(clippy::cast_possible_wrap)] // RtlNtStatusToDosError returns a Win32 DOS error code (u32) that is documented to fit i32 for io::Error::from_raw_os_error.
    pub fn link_lifetime_anchor_noreplace(
        retained_source: &File,
        to_parent: &File,
        to: &OsStr,
    ) -> io::Result<()> {
        // FileLinkInformation uses the BOOLEAN ReplaceIfExists layout below.
        // FileLinkInformationEx is class 72 and requires the ULONG Flags layout.
        const FILE_LINK_INFORMATION_CLASS: u32 = 11;

        RetainedDirectory::validate_leaf(retained_source, RetainedLeafPolicy::AnchoredAuthority)?;
        let wide: Vec<u16> = to.encode_wide().collect();
        if wide.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "lifetime-anchor target is empty",
            ));
        }
        let name_bytes = wide
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lifetime-anchor target too long",
                )
            })?;
        let size = std::mem::size_of::<FileLinkInformation>()
            .checked_add(name_bytes)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lifetime-anchor target too long",
                )
            })?;
        let mut storage = vec![0_u8; size];
        let info = storage.as_mut_ptr().cast::<FileLinkInformation>();
        // SAFETY: storage has the computed FILE_LINK_INFORMATION header and trailing UTF-16 bytes.
        unsafe {
            (*info).replace_if_exists = 0;
            (*info).root_directory = to_parent.as_raw_handle().cast();
            (*info).file_name_length = u32::try_from(name_bytes).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lifetime-anchor target too long",
                )
            })?;
            std::ptr::copy_nonoverlapping(
                wide.as_ptr(),
                (*info).file_name.as_mut_ptr(),
                wide.len(),
            );
        }
        let mut io_status = IoStatusBlock {
            value: IoStatusValue { status: 0 },
            information: 0,
        };
        // SAFETY: the exact source and destination-parent handles remain live and
        // storage is initialized for the duration of the native call.
        let status = unsafe {
            NtSetInformationFile(
                retained_source.as_raw_handle().cast(),
                std::ptr::from_mut(&mut io_status),
                info.cast(),
                u32::try_from(size).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "lifetime-anchor buffer too long",
                    )
                })?,
                FILE_LINK_INFORMATION_CLASS,
            )
        };
        if status >= 0 {
            return Ok(());
        }
        // SAFETY: pure NTSTATUS conversion.
        let raw = unsafe { RtlNtStatusToDosError(status) } as i32;
        // Windows parity: `NtSetInformationFile(FileLinkInformation)` with
        // `replace_if_exists = 0` surfaces a destination collision as one of
        // two errors that Unix `linkat` over an existing leaf reports as
        // `EEXIST` (`AlreadyExists`): `STATUS_ACCESS_DENIED` (raw os error 5)
        // for file→file collisions, and `STATUS_INVALID_PARAMETER` (raw os
        // error 87) when the destination exists with an incompatible type.
        // The lifetime-anchor publication helpers only treat `AlreadyExists`
        // as a collision/retry condition, so a Windows collision would
        // otherwise propagate as a hard failure. Re-probe the destination: if
        // it now exists, the error was a collision — translate to
        // `AlreadyExists` (chaining the original error). If the probe cannot
        // confirm the destination exists, propagate the original error
        // unchanged so genuine permission/parameter errors are never masked.
        const ERROR_ACCESS_DENIED: i32 = 5;
        const ERROR_INVALID_PARAMETER: i32 = 87;
        if matches!(raw, ERROR_ACCESS_DENIED | ERROR_INVALID_PARAMETER) {
            // The destination may be either a file or a directory, so the probe
            // sets neither FILE_NONDIRECTORY_FILE nor FILE_DIRECTORY_FILE.
            match relative_open_with_share(
                to_parent,
                to,
                GENERIC_READ,
                FILE_OPEN,
                0,
                FILE_SHARE_ALL,
            ) {
                Ok(_handle) => {
                    // The destination file exists; the access denial was a
                    // collision. Drop the probe handle and report AlreadyExists.
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "link_lifetime_anchor_noreplace destination exists \
                             (Windows no-replace parity); original error: {}",
                            io::Error::from_raw_os_error(raw)
                        ),
                    ));
                }
                Err(probe_error) if probe_error.kind() == io::ErrorKind::NotFound => {
                    // Destination absent — the access denial was genuine.
                }
                Err(_) => {
                    // Probe was ambiguous — never mask a genuine permission
                    // error. Fall through to report the original ACCESS_DENIED.
                }
            }
        }
        Err(io::Error::from_raw_os_error(raw))
    }

    #[allow(clippy::cast_ptr_alignment)]
    // the rename buffer is a freshly-allocated u8 vector that is intentionally unaligned; the header is written field-by-field before the call.
    #[allow(clippy::cast_possible_wrap)] // RtlNtStatusToDosError returns a Win32 DOS error code (u32) that is documented to fit i32 for io::Error::from_raw_os_error.
    pub fn rename_noreplace(
        _from_parent: &File,
        _from: &OsStr,
        to_parent: &File,
        to: &OsStr,
        retained_source: &File,
    ) -> io::Result<()> {
        const FILE_RENAME_INFORMATION_CLASS: u32 = 10;

        let wide: Vec<u16> = to.encode_wide().collect();
        if wide.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "rename target is empty",
            ));
        }
        let name_bytes = wide
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename target too long"))?;
        let size = std::mem::size_of::<FileRenameInfo>()
            .checked_add(name_bytes)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename target too long"))?;
        let mut storage = vec![0_u8; size];
        let info = storage.as_mut_ptr().cast::<FileRenameInfo>();
        // SAFETY: storage has the FILE_RENAME_INFORMATION header and trailing UTF-16 bytes.
        unsafe {
            (*info).replace_if_exists = 0;
            (*info).root_directory = to_parent.as_raw_handle().cast();
            (*info).file_name_length = u32::try_from(name_bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "rename target too long")
            })?;
            std::ptr::copy_nonoverlapping(
                wide.as_ptr(),
                (*info).file_name.as_mut_ptr(),
                wide.len(),
            );
        }
        let mut io_status = IoStatusBlock {
            value: IoStatusValue { status: 0 },
            information: 0,
        };
        // SAFETY: retained_source is the exact DELETE-capable source handle, the
        // destination parent remains retained, and storage is initialized.
        let status = unsafe {
            NtSetInformationFile(
                retained_source.as_raw_handle().cast(),
                std::ptr::from_mut(&mut io_status),
                info.cast(),
                u32::try_from(size).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "rename buffer too long")
                })?,
                FILE_RENAME_INFORMATION_CLASS,
            )
        };
        if status >= 0 {
            return Ok(());
        }
        // SAFETY: pure NTSTATUS conversion.
        let raw = unsafe { RtlNtStatusToDosError(status) } as i32;
        // Windows parity: `NtSetInformationFile(FileRenameInformation)` with
        // `replace_if_exists = 0` surfaces a destination collision as one of
        // two errors that Unix `renameat_with(NOREPLACE)` reports as `EEXIST`
        // (`AlreadyExists`): `STATUS_ACCESS_DENIED` (raw os error 5) for
        // file→file collisions, and `STATUS_INVALID_PARAMETER` (raw os error
        // 87) when the destination exists with an incompatible type. The
        // crash-replace and restore protocols only treat `AlreadyExists` as a
        // collision/retry condition, so a Windows collision would otherwise
        // propagate as a hard failure. Re-probe the destination: if it now
        // exists, the error was a collision — translate to `AlreadyExists`
        // (chaining the original error). If the probe cannot confirm the
        // destination exists, propagate the original error unchanged so genuine
        // permission/parameter errors are never masked.
        const ERROR_ACCESS_DENIED: i32 = 5;
        const ERROR_INVALID_PARAMETER: i32 = 87;
        if matches!(raw, ERROR_ACCESS_DENIED | ERROR_INVALID_PARAMETER) {
            // The destination may be either a file or a directory, so the probe
            // sets neither FILE_NON_DIRECTORY_FILE nor FILE_DIRECTORY_FILE.
            match relative_open_with_share(
                to_parent,
                to,
                GENERIC_READ,
                FILE_OPEN,
                0,
                FILE_SHARE_ALL,
            ) {
                Ok(_handle) => {
                    // The destination file exists; the access denial was a
                    // collision. Drop the probe handle and report AlreadyExists.
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "rename_noreplace destination exists (Windows no-replace parity); \
                             original error: {}",
                            io::Error::from_raw_os_error(raw)
                        ),
                    ));
                }
                Err(probe_error) if probe_error.kind() == io::ErrorKind::NotFound => {
                    // Destination absent — the access denial was genuine.
                }
                Err(_) => {
                    // Probe was ambiguous — never mask a genuine permission
                    // error. Fall through to report the original ACCESS_DENIED.
                }
            }
        }
        Err(io::Error::from_raw_os_error(raw))
    }

    pub fn exchange(_: &File, _: &OsStr, _: &File, _: &OsStr) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "name-based exchange is not used for Windows retained authority",
        ))
    }

    pub fn remove_exact(source: File) -> io::Result<()> {
        const FILE_DISPOSITION_INFO_EX_CLASS: u32 = 21;
        const FILE_DISPOSITION_INFO_CLASS: u32 = 4;
        const FILE_DISPOSITION_FLAG_DELETE: u32 = 0x1;
        const FILE_DISPOSITION_FLAG_POSIX_SEMANTICS: u32 = 0x2;
        let info = FileDispositionInfoEx {
            flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        };
        // SAFETY: source is the live exact-entry handle and info has the documented layout.
        let result = unsafe {
            SetFileInformationByHandle(
                source.as_raw_handle().cast(),
                FILE_DISPOSITION_INFO_EX_CLASS,
                (&raw const info).cast(),
                u32::try_from(std::mem::size_of::<FileDispositionInfoEx>())
                    .expect("disposition size"),
            )
        };
        if result != 0 {
            drop(source);
            return Ok(());
        }
        let extended_error = io::Error::last_os_error();
        if !matches!(extended_error.raw_os_error(), Some(1 | 50 | 87)) {
            return Err(extended_error);
        }

        let fallback = FileDispositionInfo { delete_file: 1 };
        // SAFETY: older Windows/filesystem combinations may not support
        // FileDispositionInfoEx. The legacy disposition uses the same exact
        // DELETE-capable handle.
        let result = unsafe {
            SetFileInformationByHandle(
                source.as_raw_handle().cast(),
                FILE_DISPOSITION_INFO_CLASS,
                (&raw const fallback).cast(),
                u32::try_from(std::mem::size_of::<FileDispositionInfo>())
                    .expect("disposition size"),
            )
        };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            drop(source);
            Ok(())
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::*;
    pub enum FileMode {
        Read,
        ReadWrite,
        ReadWriteCreate,
        WriteNew,
        ReadDeleteRename,
        ReadWriteNewDelete,
    }
    fn unsupported<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "retained directory access unsupported",
        ))
    }
    pub fn open_directory(_: &File, _: &OsStr) -> io::Result<File> {
        unsupported()
    }
    pub fn open_directory_delete(_: &File, _: &OsStr) -> io::Result<File> {
        unsupported()
    }
    pub fn directory_entry_count(_: &File, _: usize) -> io::Result<usize> {
        unsupported()
    }
    pub fn remove_empty_directory(
        _: &File,
        _: &OsStr,
        _: File,
        _: &RetainedFileIdentity,
    ) -> io::Result<()> {
        unsupported()
    }
    pub fn open_or_create_directory(_: &File, _: &OsStr) -> io::Result<File> {
        unsupported()
    }
    pub fn open_file(_: &File, _: &OsStr, _: FileMode) -> io::Result<File> {
        unsupported()
    }
    pub fn link_lifetime_anchor_noreplace(_: &File, _: &File, _: &OsStr) -> io::Result<()> {
        unsupported()
    }
    pub fn rename_noreplace(_: &File, _: &OsStr, _: &File, _: &OsStr, _: &File) -> io::Result<()> {
        unsupported()
    }
    pub fn exchange(_: &File, _: &OsStr, _: &File, _: &OsStr) -> io::Result<()> {
        unsupported()
    }
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write as _;

    fn test_root_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forge-retained-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn retained_test_link_count(file: &File) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            file.metadata().unwrap().nlink()
        }
        #[cfg(windows)]
        {
            u64::from(
                crate::windows_file_info::file_information(file)
                    .unwrap()
                    .number_of_links,
            )
        }
    }

    #[test]
    fn persisted_platform_identity_digest_fails_closed() {
        let root_path = test_root_path("persisted-identity");
        fs::create_dir_all(&root_path).unwrap();
        fs::write(root_path.join("leaf"), b"authority").unwrap();
        let leaf = File::open(root_path.join("leaf")).unwrap();
        let identity = RetainedDirectory::identity_of(&leaf).unwrap();
        let error = identity.canonical_digest().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("Store-owned file anchor"));
        drop(leaf);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    #[test]
    fn lifetime_anchor_rejects_byte_identical_target_substitution() {
        let root_path = test_root_path("lifetime-anchor-substitution");
        fs::create_dir_all(&root_path).unwrap();
        let target = Path::new("authority/selected.json");
        fs::create_dir_all(root_path.join("authority")).unwrap();
        fs::write(root_path.join(target), b"immutable authority\n").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let file = root
            .open_leaf_read(target, RetainedLeafPolicy::Authority)
            .unwrap();
        let identity = RetainedDirectory::identity_of(&file).unwrap();
        let digest = crate::sha256_content_hash(b"immutable authority\n");
        let anchor = root
            .retain_file_lifetime_anchor(
                Path::new("private/anchors"),
                &file,
                &identity,
                &digest,
                20,
            )
            .unwrap();
        let binding = anchor.binding().clone();
        assert!(binding.canonical_digest().is_ok());
        anchor.retain_target(&root, target).unwrap();

        fs::remove_file(root_path.join(target)).unwrap();
        fs::write(root_path.join(target), b"immutable authority\n").unwrap();
        let error = anchor.retain_target(&root, target).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        anchor.revalidate().unwrap();

        let reopened = root.open_file_lifetime_anchor(&binding).unwrap();
        let error = reopened.retain_target(&root, target).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        drop(reopened);
        drop(anchor);
        drop(file);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    #[test]
    fn lifetime_anchor_fails_closed_when_private_name_is_hidden_or_replaced() {
        let root_path = test_root_path("lifetime-anchor-hidden");
        fs::create_dir_all(&root_path).unwrap();
        fs::write(root_path.join("selected"), b"authority").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let file = root
            .open_leaf_read(Path::new("selected"), RetainedLeafPolicy::Authority)
            .unwrap();
        let identity = RetainedDirectory::identity_of(&file).unwrap();
        let digest = crate::sha256_content_hash(b"authority");
        let anchor = root
            .retain_file_lifetime_anchor(Path::new("private/anchors"), &file, &identity, &digest, 9)
            .unwrap();
        let anchor_path = root_path.join(&anchor.binding().anchor_relative_path);
        fs::remove_file(&anchor_path).unwrap();
        assert!(anchor.revalidate().is_err());
        fs::remove_file(root_path.join("selected")).unwrap();
        fs::write(root_path.join("selected"), b"authority").unwrap();
        fs::hard_link(root_path.join("selected"), &anchor_path).unwrap();
        assert!(anchor.revalidate().is_err());
        drop(anchor);
        drop(file);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn authority_replacement_retries_temp_collision_without_promoting_sentinel() {
        let root_path = test_root_path("collision");
        fs::create_dir_all(&root_path).unwrap();
        let first = PathBuf::from(".authority.7-8-0.forge-tmp");
        let second = PathBuf::from(".authority.7-8-1.forge-tmp");
        fs::write(root_path.join(&first), b"sentinel").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let debt = authority
            .replace_file_with_candidates(
                Path::new("authority"),
                b"replacement",
                |attempt| {
                    if attempt == 0 {
                        first.clone()
                    } else {
                        second.clone()
                    }
                },
                |_, _, _| Ok(()),
            )
            .unwrap();
        #[cfg(any(windows, target_os = "macos"))]
        assert!(debt.paths().is_empty());
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert_eq!(debt.paths().len(), 2);
            let mut debt_bytes = debt
                .paths()
                .iter()
                .map(|path| fs::read(root_path.join(path)).unwrap())
                .collect::<Vec<_>>();
            debt_bytes.sort();
            assert_eq!(debt_bytes, vec![Vec::<u8>::new(), b"replacement".to_vec()]);
        }
        assert_eq!(
            fs::read(root_path.join("authority")).unwrap(),
            b"replacement"
        );
        assert_eq!(fs::read(root_path.join(&first)).unwrap(), b"sentinel");
        assert!(!root_path.join(&second).exists());
        #[cfg(target_os = "macos")]
        {
            let published = File::open(root_path.join("authority")).unwrap();
            assert_eq!(retained_test_link_count(&published), 1);
        }
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn authority_replacement_collision_exhaustion_preserves_destination_and_temps() {
        let root_path = test_root_path("collision-exhaustion");
        fs::create_dir_all(&root_path).unwrap();
        let target = PathBuf::from("authority");
        fs::write(root_path.join(&target), b"destination sentinel").unwrap();
        for attempt in 0..32 {
            fs::write(
                root_path.join(format!(".authority.7-8-{attempt}.forge-tmp")),
                b"temp sentinel",
            )
            .unwrap();
        }
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let error = authority
            .replace_file_with_candidates(
                &target,
                b"replacement",
                |attempt| PathBuf::from(format!(".authority.7-8-{attempt}.forge-tmp")),
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            error.to_string(),
            "retained authority quarantine-name retry exhausted"
        );
        assert_eq!(
            fs::read(root_path.join(&target)).unwrap(),
            b"destination sentinel"
        );
        for attempt in 0..32 {
            assert_eq!(
                fs::read(root_path.join(format!(".authority.7-8-{attempt}.forge-tmp"))).unwrap(),
                b"temp sentinel"
            );
        }
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authority_replacement_rejects_temp_substitution_before_commit() {
        let root_path = test_root_path("temp-substitution");
        fs::create_dir_all(&root_path).unwrap();
        let temp = PathBuf::from(".authority.7-8-0.forge-tmp");
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let error = authority
            .replace_file_with_candidates(
                Path::new("authority"),
                b"replacement",
                |_| temp.clone(),
                |_, candidate, _| {
                    let candidate = root_path.join(candidate);
                    fs::remove_file(&candidate)?;
                    fs::write(candidate, b"temp sentinel")
                },
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!root_path.join("authority").exists());
        assert_eq!(fs::read(root_path.join(&temp)).unwrap(), b"temp sentinel");
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authority_replacement_rejects_destination_substitution_before_commit() {
        let root_path = test_root_path("destination-substitution");
        fs::create_dir_all(&root_path).unwrap();
        let target = PathBuf::from("authority");
        let temp = PathBuf::from(".authority.7-8-0.forge-tmp");
        fs::write(root_path.join(&target), b"old destination").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let error = authority
            .replace_file_with_candidates(
                &target,
                b"replacement",
                |_| temp.clone(),
                |_, _, destination| {
                    let destination = root_path.join(destination);
                    fs::remove_file(&destination)?;
                    fs::write(destination, b"destination sentinel")
                },
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(root_path.join(&target)).unwrap(),
            b"destination sentinel"
        );
        assert_eq!(fs::read(root_path.join(&temp)).unwrap(), b"replacement");
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn authority_replacement_isolates_exact_previous_destination_as_debt() {
        let root_path = test_root_path("replacement-debt");
        fs::create_dir_all(&root_path).unwrap();
        let target = PathBuf::from("authority");
        let temp = PathBuf::from(".authority.replacement.quarantine");
        fs::write(root_path.join(&target), b"previous").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let debt = authority
            .replace_file_with_candidates(
                &target,
                b"replacement",
                |_| temp.clone(),
                |_, _, _| Ok(()),
            )
            .unwrap();
        assert_eq!(fs::read(root_path.join(&target)).unwrap(), b"replacement");
        assert_eq!(debt.paths().len(), 1);
        assert_eq!(
            fs::read(root_path.join(&debt.paths()[0])).unwrap(),
            b"previous"
        );
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authority_delete_isolates_exact_source_and_placeholder_without_unlinking() {
        let root_path = test_root_path("delete-debt");
        fs::create_dir_all(&root_path).unwrap();
        let target = PathBuf::from("authority");
        fs::write(root_path.join(&target), b"delete me").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let debt = authority
            .remove_file_with_validation(&target, |directory, source| {
                (directory.read_authority_bounded(source, 9)? == b"delete me")
                    .then_some(())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "source changed"))
            })
            .unwrap();
        assert!(!root_path.join(&target).exists());
        assert_eq!(debt.paths().len(), 2);
        let mut contents = debt
            .paths()
            .iter()
            .map(|path| fs::read(root_path.join(path)).unwrap())
            .collect::<Vec<_>>();
        contents.sort();
        assert_eq!(contents, vec![Vec::<u8>::new(), b"delete me".to_vec()]);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authority_delete_rejects_source_substitution_without_unlinking_substitute() {
        let root_path = test_root_path("delete-substitution");
        fs::create_dir_all(&root_path).unwrap();
        let target = PathBuf::from("authority");
        fs::write(root_path.join(&target), b"original").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let error = authority
            .remove_file_with_hook(&target, |_, path| {
                let path = root_path.join(path);
                fs::remove_file(&path)?;
                fs::write(path, b"substitute")
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(root_path.join(&target)).unwrap(), b"substitute");
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn retained_noreplace_move_publishes_exact_single_link_source() {
        let root_path = test_root_path("exact-move");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from(".source.forge-staging");
        let destination = PathBuf::from("destination");
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let mut retained = root.open_leaf_write_new_authority(&source).unwrap();
        retained.write_all(b"original").unwrap();
        retained.sync_all().unwrap();
        let identity = RetainedDirectory::identity_of(&retained).unwrap();
        root.verify_mutable_authority_binding(&source, &retained, &identity)
            .unwrap();
        let authority = root.retain_authority().unwrap();

        authority
            .move_retained_file_noreplace(&source, &retained, &identity, &destination)
            .unwrap();

        assert!(!root_path.join(&source).exists());
        assert_eq!(fs::read(root_path.join(&destination)).unwrap(), b"original");
        let published = File::open(root_path.join(&destination)).unwrap();
        assert_eq!(
            RetainedDirectory::identity_of(&published).unwrap(),
            identity
        );
        assert_eq!(retained_test_link_count(&published), 1);
        drop(published);
        drop(authority);
        drop(retained);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn retained_noreplace_move_preserves_destination_race() {
        let root_path = test_root_path("exact-move-destination-race");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from(".source.forge-staging");
        let destination = PathBuf::from("destination");
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let mut retained = root.open_leaf_write_new_authority(&source).unwrap();
        retained.write_all(b"original").unwrap();
        retained.sync_all().unwrap();
        let identity = RetainedDirectory::identity_of(&retained).unwrap();
        let authority = root.retain_authority().unwrap();

        let error = authority
            .move_retained_file_noreplace_with_hooks(
                &source,
                &retained,
                &identity,
                &destination,
                |_, _, to| fs::write(root_path.join(to), b"destination sentinel"),
                |_, _, _| Ok(()),
            )
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(root_path.join(&source)).unwrap(), b"original");
        assert_eq!(
            fs::read(root_path.join(&destination)).unwrap(),
            b"destination sentinel"
        );
        drop(authority);
        drop(retained);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn retained_noreplace_move_keeps_committed_destination_after_post_move_failure() {
        let root_path = test_root_path("exact-move-post-commit-failure");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from(".source.forge-staging");
        let destination = PathBuf::from("destination");
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let mut retained = root.open_leaf_write_new_authority(&source).unwrap();
        retained.write_all(b"original").unwrap();
        retained.sync_all().unwrap();
        let identity = RetainedDirectory::identity_of(&retained).unwrap();
        let authority = root.retain_authority().unwrap();

        let error = authority
            .move_retained_file_noreplace_with_hooks(
                &source,
                &retained,
                &identity,
                &destination,
                |_, _, _| Ok(()),
                |_, _, _| Err(io::Error::other("injected post-move failure")),
            )
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!root_path.join(&source).exists());
        assert_eq!(fs::read(root_path.join(&destination)).unwrap(), b"original");
        let published = File::open(root_path.join(&destination)).unwrap();
        assert_eq!(
            RetainedDirectory::identity_of(&published).unwrap(),
            identity
        );
        assert_eq!(retained_test_link_count(&published), 1);
        drop(published);
        drop(authority);
        drop(retained);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    #[test]
    fn authority_noreplace_rename_publishes_exact_source_and_accounts_cleanup_debt() {
        let root_path = test_root_path("rename-publication");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from("source");
        let destination = PathBuf::from("destination");
        fs::write(root_path.join(&source), b"original").unwrap();
        let retained_source = File::open(root_path.join(&source)).unwrap();
        let retained_source_identity = RetainedDirectory::identity_of(&retained_source).unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let debt = authority
            .rename_file_noreplace_with_validation(
                &source,
                &destination,
                |directory, retained_source, _| {
                    (directory.read_authority_bounded(retained_source, 8)? == b"original")
                        .then_some(())
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "source changed"))
                },
            )
            .unwrap();
        assert!(!root_path.join(&source).exists());
        assert_eq!(fs::read(root_path.join(&destination)).unwrap(), b"original");
        let published = File::open(root_path.join(&destination)).unwrap();
        assert_eq!(
            RetainedDirectory::identity_of(&published).unwrap(),
            retained_source_identity,
            "publication must name the exact retained source inode"
        );
        #[cfg(windows)]
        assert!(debt.paths().is_empty());
        #[cfg(unix)]
        {
            assert_eq!(debt.paths().len(), 2);
            let mut debt_bytes = debt
                .paths()
                .iter()
                .map(|path| fs::read(root_path.join(path)).unwrap())
                .collect::<Vec<_>>();
            debt_bytes.sort();
            assert_eq!(debt_bytes, vec![Vec::<u8>::new(), b"original".to_vec()]);
        }
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authority_noreplace_rename_rejects_source_substitution() {
        let root_path = test_root_path("rename-substitution");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from("source");
        let destination = PathBuf::from("destination");
        fs::write(root_path.join(&source), b"original").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();
        let error = authority
            .rename_file_noreplace_with_hook(&source, &destination, |_, from, _| {
                let from = root_path.join(from);
                fs::remove_file(&from)?;
                fs::write(from, b"substitute")
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(root_path.join(&source)).unwrap(), b"substitute");
        assert!(!root_path.join(&destination).exists());
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
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
    fn authority_noreplace_rename_isolates_destination_that_races_vacancy_check() {
        let root_path = test_root_path("rename-destination-race");
        fs::create_dir_all(&root_path).unwrap();
        let source = PathBuf::from("source");
        let destination = PathBuf::from("destination");
        fs::write(root_path.join(&source), b"original").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let authority = root.retain_authority().unwrap();

        authority
            .rename_file_noreplace_with_hook(&source, &destination, |_, _, to| {
                fs::write(root_path.join(to), b"racing destination")
            })
            .expect_err("a destination race must fail closed");

        assert_eq!(fs::read(root_path.join(&source)).unwrap(), b"original");
        assert!(!root_path.join(&destination).exists());
        assert!(fs::read_dir(&root_path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path() != root_path.join(&source))
            .any(|entry| {
                fs::read(entry.path()).ok().as_deref() == Some(&b"racing destination"[..])
            }));
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn mutable_authority_rejects_hard_links_while_read_authority_keeps_cleanup_debt_visible() {
        let root_path = std::env::temp_dir().join(format!(
            "forge-retained-leaf-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root_path).unwrap();
        let outside = root_path.join("outside");
        fs::write(&outside, b"sentinel").unwrap();
        fs::hard_link(&outside, root_path.join("linked")).unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        assert!(root
            .open_leaf_read(Path::new("linked"), RetainedLeafPolicy::SourceRead)
            .is_ok());
        #[cfg(unix)]
        assert!(root
            .open_leaf_read(Path::new("linked"), RetainedLeafPolicy::Authority)
            .is_ok());
        #[cfg(windows)]
        assert!(root
            .open_leaf_read(Path::new("linked"), RetainedLeafPolicy::Authority)
            .is_err());
        assert!(root
            .open_leaf_read(Path::new("linked"), RetainedLeafPolicy::MutableAuthority,)
            .is_err());
        assert!(root.open_read_write(Path::new("linked")).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"sentinel");
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn mutable_authority_rechecks_link_shape_at_the_write_boundary() {
        let root_path = test_root_path("mutable-link-boundary");
        fs::create_dir_all(&root_path).unwrap();
        let target = Path::new("authority.wal");
        let alias = root_path.join("outside-alias");
        fs::write(root_path.join(target), b"sentinel").unwrap();
        let root = RetainedDirectory::open_root(&root_path).unwrap();
        let mut file = root.open_read_write(target).unwrap();
        let identity = RetainedDirectory::identity_of(&file).unwrap();

        fs::hard_link(root_path.join(target), &alias).unwrap();
        let mut mutation_called = false;
        let error = root
            .mutate_authority_file(target, &mut file, &identity, |_| {
                mutation_called = true;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!mutation_called);
        assert_eq!(fs::read(&alias).unwrap(), b"sentinel");

        fs::remove_file(&alias).unwrap();
        let post_error_alias = root_path.join("outside-post-error-alias");
        let error = root
            .mutate_authority_file(target, &mut file, &identity, |_| {
                fs::hard_link(root_path.join(target), &post_error_alias)?;
                Err::<(), _>(io::Error::other("injected mutation failure"))
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("injected mutation failure"));
        assert!(error
            .to_string()
            .contains("post-mutation binding validation failed"));
        assert_eq!(fs::read(&post_error_alias).unwrap(), b"sentinel");

        drop(file);
        drop(root);
        fs::remove_dir_all(root_path).unwrap();
    }
}
