//! Shared filesystem primitives for governance CLI modules.
//!
//! - [`atomic_write`]: write-then-rename so a reader never sees a truncated
//!   YAML (review S4.4 bug #2 — partial-write `DoS`).
//! - [`DirLock`]: an exclusive advisory lockfile over a directory, so the
//!   load->decide->write lifecycle of a mutating command is serialized across
//!   concurrent invocations (review S4.4 bug #1 — race condition).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DIR_LOCK_STALE_AFTER: Duration = Duration::from_secs(60);
const DIR_LOCK_RETRY_ATTEMPTS: u32 = 40;
const ATOMIC_WRITE_TEMP_ATTEMPTS: u32 = 16;

/// Write `bytes` to `target` atomically and durably: stage in a temp sibling
/// file, fsync it, rename it into place, then fsync the parent directory on
/// Unix. A reader therefore never observes a half-written contract file, and a
/// crash should not lose the committed rename on filesystems that support
/// directory fsync.
///
/// # Errors
///
/// Returns any filesystem error from creating, writing, syncing, renaming, or
/// parent-directory syncing the target file.
pub fn atomic_write(target: &Path, bytes: &str) -> std::io::Result<()> {
    let parent = target_parent(target);
    let (tmp, file) = create_atomic_temp_file(target, parent)?;
    let result = write_and_rename_atomically(&tmp, target, parent, bytes, file);
    if let Err(error) = result {
        return Err(cleanup_atomic_temp_after_error(error, &tmp, parent));
    }
    Ok(())
}

fn target_parent(target: &Path) -> &Path {
    target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn create_atomic_temp_file(
    target: &Path,
    parent: &Path,
) -> std::io::Result<(PathBuf, std::fs::File)> {
    use std::fs::OpenOptions;
    use std::io::ErrorKind;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let file_name = target
        .file_name()
        .map_or_else(|| "atomic-write".into(), std::ffi::OsStr::to_string_lossy);

    for attempt in 0..ATOMIC_WRITE_TEMP_ATTEMPTS {
        let tmp = parent.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            nonce,
            attempt
        ));
        match OpenOptions::new().create_new(true).write(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }

    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "failed to create unique temp file for atomic write to {} after {ATOMIC_WRITE_TEMP_ATTEMPTS} attempts",
            target.display()
        ),
    ))
}

fn write_and_rename_atomically(
    tmp: &Path,
    target: &Path,
    parent: &Path,
    bytes: &str,
    mut file: std::fs::File,
) -> std::io::Result<()> {
    use std::io::Write;

    file.write_all(bytes.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp, target)?;
    #[cfg(unix)]
    sync_parent_dir(parent)?;
    #[cfg(not(unix))]
    sync_parent_dir(parent);
    Ok(())
}

fn cleanup_atomic_temp_after_error(
    original: std::io::Error,
    tmp: &Path,
    parent: &Path,
) -> std::io::Error {
    #[cfg(not(unix))]
    let _ = parent;
    match std::fs::remove_file(tmp) {
        Ok(()) => {
            #[cfg(unix)]
            if let Err(cleanup_error) = sync_parent_dir(parent) {
                return std::io::Error::new(
                    original.kind(),
                    format!(
                        "atomic write failed: {original}; removed temp {} but parent sync failed: {cleanup_error}",
                        tmp.display()
                    ),
                );
            }
            original
        }
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => original,
        Err(cleanup_error) => std::io::Error::new(
            original.kind(),
            format!(
                "atomic write failed: {original}; additionally failed to remove temp {}: {cleanup_error}",
                tmp.display()
            ),
        ),
    }
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> std::io::Result<()> {
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) {}

/// An authority directory retained as an opened OS handle.
///
/// Every projection is opened relative to this handle. The lexical path is
/// retained only for diagnostics: renaming the directory, installing a
/// replacement at its old name, and later restoring it cannot redirect reads
/// or lock ownership.
pub(crate) struct RetainedDirectoryIdentity {
    lexical_root: PathBuf,
    handle: std::fs::File,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    created: Option<SystemTime>,
}

impl Clone for RetainedDirectoryIdentity {
    fn clone(&self) -> Self {
        Self {
            lexical_root: self.lexical_root.clone(),
            handle: self
                .handle
                .try_clone()
                .expect("retained directory handle duplication must succeed"),
            #[cfg(unix)]
            device: self.device,
            #[cfg(unix)]
            inode: self.inode,
            #[cfg(windows)]
            volume_serial: self.volume_serial,
            #[cfg(windows)]
            file_index: self.file_index,
            #[cfg(not(any(unix, windows)))]
            created: self.created,
        }
    }
}

impl RetainedDirectoryIdentity {
    pub(crate) fn capture(root: &Path) -> std::io::Result<Self> {
        Self::from_open_handle(root.to_path_buf(), open_directory_no_follow(root)?)
    }

    fn from_open_handle(lexical_root: PathBuf, handle: std::fs::File) -> std::io::Result<Self> {
        use std::io::{Error, ErrorKind};
        let metadata = handle.metadata()?;
        if !metadata.is_dir() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "{} is not a retained authority directory",
                    lexical_root.display()
                ),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                lexical_root,
                handle,
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            reject_windows_reparse_point(&lexical_root, &metadata)?;
            let identity = windows_file_identity(&handle)?;
            Ok(Self {
                lexical_root,
                handle,
                volume_serial: identity.volume_serial,
                file_index: identity.file_index,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                lexical_root,
                handle,
                created: metadata.created().ok(),
            })
        }
    }

    /// Validate the retained handle itself, never the namespace path that
    /// originally named it.
    pub(crate) fn validate(&self) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind};
        let metadata = self.handle.metadata()?;
        if !metadata.is_dir() || !self.matches_open_handle(&self.handle, &metadata)? {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "retained authority directory {} changed identity",
                    self.lexical_root.display()
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn read_direct_file_bounded(
        &self,
        relative_file: &Path,
        maximum_bytes: u64,
    ) -> std::io::Result<Vec<u8>> {
        validate_direct_relative_file(relative_file)?;
        self.validate()?;
        let file = open_file_relative_no_follow(&self.handle, relative_file, RelativeOpen::Read)?;
        read_open_regular_file_bounded(file, &self.lexical_root.join(relative_file), maximum_bytes)
    }

    pub(crate) fn read_optional_direct_file_bounded(
        &self,
        relative_file: &Path,
        maximum_bytes: u64,
    ) -> std::io::Result<Option<Vec<u8>>> {
        match self.read_direct_file_bounded(relative_file, maximum_bytes) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn read_sorted_direct_files_bounded(
        &self,
        extension: &str,
        maximum_bytes: u64,
    ) -> std::io::Result<Vec<(PathBuf, Vec<u8>)>> {
        self.validate()?;
        let mut relative_paths = direct_directory_entries(&self.handle)?;
        relative_paths.retain(|path| {
            path.extension()
                .is_some_and(|candidate| candidate == extension)
        });
        relative_paths.sort();
        let mut projected = Vec::with_capacity(relative_paths.len());
        for relative_path in relative_paths {
            let raw = self.read_direct_file_bounded(&relative_path, maximum_bytes)?;
            projected.push((relative_path, raw));
        }
        Ok(projected)
    }

    pub(crate) fn create_new_direct_file(
        &self,
        relative_file: &Path,
    ) -> std::io::Result<std::fs::File> {
        validate_direct_relative_file(relative_file)?;
        self.validate()?;
        open_file_relative_no_follow(&self.handle, relative_file, RelativeOpen::CreateNew)
    }

    pub(crate) fn write_new_direct_file_synced(
        &self,
        relative_file: &Path,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        use std::io::Write as _;

        let mut file = self.create_new_direct_file(relative_file)?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            return match self.remove_direct_file(relative_file) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(std::io::Error::new(
                    error.kind(),
                    format!(
                        "create-only write failed: {error}; cleanup of {} also failed: {cleanup_error}",
                        relative_file.display()
                    ),
                )),
            };
        }
        self.handle.sync_all()?;
        self.validate()
    }

    pub(crate) fn open_or_create_direct_directory(
        &self,
        relative_directory: &Path,
    ) -> std::io::Result<Self> {
        validate_direct_relative_file(relative_directory)?;
        self.validate()?;
        let handle = open_file_relative_no_follow(
            &self.handle,
            relative_directory,
            RelativeOpen::DirectoryOpenOrCreate,
        )?;
        Self::from_open_handle(self.lexical_root.join(relative_directory), handle)
    }

    pub(crate) fn remove_direct_file(&self, relative_file: &Path) -> std::io::Result<()> {
        validate_direct_relative_file(relative_file)?;
        self.validate()?;
        remove_file_relative(&self.handle, relative_file)
    }

    pub(crate) fn direct_file_modified_age(&self, relative_file: &Path) -> Option<Duration> {
        let file =
            open_file_relative_no_follow(&self.handle, relative_file, RelativeOpen::Read).ok()?;
        file.metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
    }

    #[cfg(unix)]
    #[allow(clippy::unnecessary_wraps)]
    fn matches_open_handle(
        &self,
        _file: &std::fs::File,
        metadata: &std::fs::Metadata,
    ) -> std::io::Result<bool> {
        use std::os::unix::fs::MetadataExt;
        Ok(metadata.dev() == self.device && metadata.ino() == self.inode)
    }

    #[cfg(windows)]
    fn matches_open_handle(
        &self,
        file: &std::fs::File,
        _metadata: &std::fs::Metadata,
    ) -> std::io::Result<bool> {
        let identity = windows_file_identity(file)?;
        Ok(identity.volume_serial == self.volume_serial && identity.file_index == self.file_index)
    }

    #[cfg(not(any(unix, windows)))]
    fn matches_open_handle(
        &self,
        _file: &std::fs::File,
        metadata: &std::fs::Metadata,
    ) -> std::io::Result<bool> {
        Ok(metadata.created().ok() == self.created)
    }
}

pub(crate) fn acquire_effect_store_lock_retained(
    retained_root: &forge_core_store::RetainedEffectStoreRoot,
    lock_relative_path: &str,
) -> Result<forge_core_store::EffectStoreLock, forge_core_store::EffectStoreLockError> {
    retained_root.acquire_effect_store_lock(lock_relative_path)
}

#[derive(Clone, Copy)]
enum RelativeOpen {
    Read,
    CreateNew,
    DirectoryOpenOrCreate,
    #[cfg(windows)]
    Delete,
}

fn validate_direct_relative_file(relative_file: &Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let mut components = relative_file.components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "{} is not a direct retained-authority file",
                relative_file.display()
            ),
        ));
    }
    Ok(())
}

/// Read one exact regular file through a no-follow descriptor.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn read_regular_file_no_follow_bounded(
    path: &Path,
    maximum_bytes: u64,
) -> std::io::Result<Vec<u8>> {
    let file = open_path_file_no_follow(path)?;
    read_open_regular_file_bounded(file, path, maximum_bytes)
}

fn read_open_regular_file_bounded(
    mut file: std::fs::File,
    display_path: &Path,
    maximum_bytes: u64,
) -> std::io::Result<Vec<u8>> {
    use std::io::{Error, ErrorKind, Read};
    let before_metadata = file.metadata()?;
    let before = projected_file_identity(&file, &before_metadata)?;
    validate_projected_file_metadata(display_path, &before_metadata, &before, maximum_bytes)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(before_metadata.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(maximum_bytes).unwrap_or(usize::MAX)),
    );
    file.by_ref()
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{} exceeds {maximum_bytes} bytes", display_path.display()),
        ));
    }
    let after_metadata = file.metadata()?;
    let after = projected_file_identity(&file, &after_metadata)?;
    if before != after {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{} changed while it was projected", display_path.display()),
        ));
    }
    Ok(bytes)
}

fn validate_projected_file_metadata(
    path: &Path,
    metadata: &std::fs::Metadata,
    identity: &ProjectedFileIdentity,
    maximum_bytes: u64,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if identity.link_count() != 1 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{} has multiple hard links", path.display()),
        ));
    }
    if metadata.len() > maximum_bytes {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{} exceeds {maximum_bytes} bytes", path.display()),
        ));
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedFileIdentity {
    device: u64,
    inode: u64,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn projected_file_identity(
    _file: &std::fs::File,
    metadata: &std::fs::Metadata,
) -> std::io::Result<ProjectedFileIdentity> {
    use std::os::unix::fs::MetadataExt;
    Ok(ProjectedFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
    })
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedFileIdentity {
    volume_serial: u32,
    file_index: u64,
    links: u32,
    length: u64,
    last_write_time: u64,
}

#[cfg(windows)]
fn projected_file_identity(
    file: &std::fs::File,
    metadata: &std::fs::Metadata,
) -> std::io::Result<ProjectedFileIdentity> {
    use std::os::windows::fs::MetadataExt;
    let opened = windows_file_identity(file)?;
    Ok(ProjectedFileIdentity {
        volume_serial: opened.volume_serial,
        file_index: opened.file_index,
        links: opened.links,
        length: metadata.file_size(),
        last_write_time: metadata.last_write_time(),
    })
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedFileIdentity;

#[cfg(not(any(unix, windows)))]
fn projected_file_identity(
    _file: &std::fs::File,
    _metadata: &std::fs::Metadata,
) -> std::io::Result<ProjectedFileIdentity> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opened-handle identity is unsupported on this platform",
    ))
}

impl ProjectedFileIdentity {
    #[cfg(unix)]
    const fn link_count(&self) -> u64 {
        self.links
    }

    #[cfg(windows)]
    const fn link_count(&self) -> u64 {
        self.links as u64
    }

    #[cfg(not(any(unix, windows)))]
    const fn link_count(&self) -> u64 {
        0
    }
}

#[cfg(unix)]
fn open_directory_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

#[cfg(windows)]
fn open_directory_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0x0000_0001 | 0x0000_0002 | 0x0000_0004)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_directory_no_follow(_path: &Path) -> std::io::Result<std::fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "retained directory handles are unsupported on this platform",
    ))
}

#[cfg(unix)]
#[cfg_attr(not(test), allow(dead_code))]
fn open_path_file_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
}

#[cfg(windows)]
#[cfg_attr(not(test), allow(dead_code))]
fn open_path_file_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x0000_0001 | 0x0000_0002 | 0x0000_0004)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    reject_windows_reparse_point(path, &file.metadata()?)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_path_file_no_follow(_path: &Path) -> std::io::Result<std::fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no-follow file handles are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn open_file_relative_no_follow(
    directory: &std::fs::File,
    relative_file: &Path,
    mode: RelativeOpen,
) -> std::io::Result<std::fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(relative_file.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "file name contains NUL")
    })?;
    if matches!(mode, RelativeOpen::DirectoryOpenOrCreate) {
        // SAFETY: the retained fd and NUL-terminated direct child name are valid.
        if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
    }
    let flags = match mode {
        RelativeOpen::Read => {
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC
        }
        RelativeOpen::CreateNew => {
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
        RelativeOpen::DirectoryOpenOrCreate => {
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
    };
    // SAFETY: `name` is NUL terminated, the retained directory fd is live, and
    // the returned descriptor is owned exactly once by `File`.
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, 0o600) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }
}

#[cfg(windows)]
fn open_file_relative_no_follow(
    directory: &std::fs::File,
    relative_file: &Path,
    mode: RelativeOpen,
) -> std::io::Result<std::fs::File> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::{ffi::c_void, ptr};

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: *mut c_void,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut c_void,
        security_quality_of_service: *mut c_void,
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: isize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            file_handle: *mut *mut c_void,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut c_void,
            ea_length: u32,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }

    let mut name = relative_file.as_os_str().encode_wide().collect::<Vec<_>>();
    let byte_len = name
        .len()
        .checked_mul(2)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "file name too long")
        })?;
    let mut unicode = UnicodeString {
        length: byte_len,
        maximum_length: byte_len,
        buffer: name.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: u32::try_from(std::mem::size_of::<ObjectAttributes>()).expect("size fits u32"),
        root_directory: directory.as_raw_handle(),
        object_name: ptr::addr_of_mut!(unicode),
        attributes: 0x0000_0040,
        security_descriptor: ptr::null_mut(),
        security_quality_of_service: ptr::null_mut(),
    };
    let (desired_access, disposition, type_option, file_attributes) = match mode {
        RelativeOpen::Read => (0x0012_0089, 1, 0x0000_0040, 0x0000_0080),
        RelativeOpen::CreateNew => (0x0012_0196, 2, 0x0000_0040, 0x0000_0080),
        RelativeOpen::DirectoryOpenOrCreate => (0x0012_019f, 3, 0x0000_0001, 0),
        RelativeOpen::Delete => (0x0011_0080, 1, 0x0000_0040, 0x0000_0080),
    };
    let mut handle = ptr::null_mut();
    let mut io_status = IoStatusBlock {
        status: 0,
        information: 0,
    };
    // SAFETY: all pointers refer to initialized storage for the duration of the
    // synchronous call. `RootDirectory` is the retained directory handle.
    let status = unsafe {
        NtCreateFile(
            ptr::addr_of_mut!(handle),
            desired_access,
            ptr::addr_of_mut!(attributes),
            ptr::addr_of_mut!(io_status),
            ptr::null_mut(),
            file_attributes,
            0x0000_0001 | 0x0000_0002 | 0x0000_0004,
            disposition,
            type_option | 0x0000_0020 | 0x0020_0000,
            ptr::null_mut(),
            0,
        )
    };
    if status < 0 {
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(std::io::Error::from_raw_os_error(
            i32::try_from(code).unwrap_or(i32::MAX),
        ));
    }
    let file = unsafe { std::fs::File::from_raw_handle(handle) };
    reject_windows_reparse_point(relative_file, &file.metadata()?)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_file_relative_no_follow(
    _directory: &std::fs::File,
    _relative_file: &Path,
    _mode: RelativeOpen,
) -> std::io::Result<std::fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "descriptor-relative file opens are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn remove_file_relative(directory: &std::fs::File, relative_file: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(relative_file.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "file name contains NUL")
    })?;
    // SAFETY: the retained fd and NUL-terminated direct child name are valid.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn remove_file_relative(directory: &std::fs::File, relative_file: &Path) -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: u8,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            class: u32,
            information: *const c_void,
            size: u32,
        ) -> i32;
    }
    let file = open_file_relative_no_follow(directory, relative_file, RelativeOpen::Delete)?;
    let disposition = FileDispositionInfo { delete_file: 1 };
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            4,
            std::ptr::addr_of!(disposition).cast(),
            u32::try_from(std::mem::size_of::<FileDispositionInfo>()).expect("size fits"),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn remove_file_relative(_directory: &std::fs::File, _relative_file: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "descriptor-relative removal is unsupported on this platform",
    ))
}

#[cfg(all(unix, any(target_os = "linux", target_os = "android")))]
fn clear_readdir_errno() {
    // SAFETY: the platform errno accessor returns this thread's live errno slot.
    unsafe { *libc::__errno_location() = 0 };
}

#[cfg(all(
    unix,
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )
))]
fn clear_readdir_errno() {
    // SAFETY: the platform errno accessor returns this thread's live errno slot.
    unsafe { *libc::__error() = 0 };
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))
))]
fn clear_readdir_errno() {}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )
))]
fn readdir_error() -> Option<std::io::Error> {
    let error = std::io::Error::last_os_error();
    (error.raw_os_error() != Some(0)).then_some(error)
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))
))]
fn readdir_error() -> Option<std::io::Error> {
    None
}
#[cfg(unix)]
fn direct_directory_entries(directory: &std::fs::File) -> std::io::Result<Vec<PathBuf>> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStringExt;
    // SAFETY: dup creates an owned descriptor; fdopendir takes that ownership,
    // and closedir releases it on every normal exit below.
    let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(duplicate) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(duplicate) };
        return Err(error);
    }
    unsafe { libc::rewinddir(stream) };
    let mut entries = Vec::new();
    loop {
        clear_readdir_errno();
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            let error = readdir_error();
            unsafe { libc::rewinddir(stream) };
            unsafe { libc::closedir(stream) };
            return error.map_or_else(|| Ok(entries), Err);
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name != b"." && name != b".." {
            entries.push(PathBuf::from(std::ffi::OsString::from_vec(name.to_vec())));
        }
    }
}

#[cfg(windows)]
fn direct_directory_entries(directory: &std::fs::File) -> std::io::Result<Vec<PathBuf>> {
    use std::os::windows::io::AsRawHandle;
    use std::{ffi::c_void, os::windows::ffi::OsStringExt};
    #[repr(C)]
    struct IoStatusBlock {
        status: isize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryDirectoryFile(
            file_handle: *mut c_void,
            event: *mut c_void,
            apc_routine: *mut c_void,
            apc_context: *mut c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut c_void,
            length: u32,
            file_information_class: u32,
            return_single_entry: u8,
            file_name: *mut c_void,
            restart_scan: u8,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }
    const STATUS_NO_MORE_FILES: i32 = 0x8000_0006_u32 as i32;
    let mut entries = Vec::new();
    let mut restart = 1_u8;
    loop {
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut io_status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let status = unsafe {
            NtQueryDirectoryFile(
                directory.as_raw_handle(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!(io_status),
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).expect("directory buffer fits"),
                12,
                0,
                std::ptr::null_mut(),
                restart,
            )
        };
        restart = 0;
        if status == STATUS_NO_MORE_FILES {
            return Ok(entries);
        }
        if status < 0 {
            let code = unsafe { RtlNtStatusToDosError(status) };
            return Err(std::io::Error::from_raw_os_error(
                i32::try_from(code).unwrap_or(i32::MAX),
            ));
        }
        let mut offset = 0_usize;
        while offset < io_status.information {
            let record = &buffer[offset..io_status.information];
            if record.len() < 12 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "truncated directory entry",
                ));
            }
            let next = u32::from_ne_bytes(record[0..4].try_into().expect("slice length"));
            let name_bytes = usize::try_from(u32::from_ne_bytes(
                record[8..12].try_into().expect("slice length"),
            ))
            .expect("u32 fits usize");
            let end = 12_usize.checked_add(name_bytes).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "directory entry overflow")
            })?;
            if end > record.len() || name_bytes % 2 != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid directory entry name",
                ));
            }
            let wide = record[12..end]
                .chunks_exact(2)
                .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let name = std::ffi::OsString::from_wide(&wide);
            if name != "." && name != ".." {
                entries.push(PathBuf::from(name));
            }
            if next == 0 {
                break;
            }
            offset = offset
                .checked_add(usize::try_from(next).expect("u32 fits usize"))
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "directory offset overflow",
                    )
                })?;
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn direct_directory_entries(_directory: &std::fs::File) -> std::io::Result<Vec<PathBuf>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "descriptor-relative directory enumeration is unsupported on this platform",
    ))
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowsFileIdentity {
    volume_serial: u32,
    file_index: u64,
    links: u32,
}

#[cfg(windows)]
fn windows_file_identity(file: &std::fs::File) -> std::io::Result<WindowsFileIdentity> {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation_time: FileTime,
        access_time: FileTime,
        write_time: FileTime,
        volume_serial: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }
    let mut information = std::mem::MaybeUninit::<ByHandleFileInformation>::uninit();
    let result =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if result == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let information = unsafe { information.assume_init() };
    Ok(WindowsFileIdentity {
        volume_serial: information.volume_serial,
        file_index: (u64::from(information.index_high) << 32) | u64::from(information.index_low),
        links: information.links,
    })
}
#[cfg(windows)]
fn reject_windows_reparse_point(path: &Path, metadata: &std::fs::Metadata) -> std::io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is a reparse point", path.display()),
        ))
    } else {
        Ok(())
    }
}
/// An exclusive advisory lock over a directory, materialized as a lockfile
/// named `lockfile_name` inside that directory. Acquired via atomic
/// `create_new`; removed on drop only when the on-disk ownership token still
/// matches this lock instance. Serializes lifecycle transitions so two racing
/// mutating commands cannot both win.
pub struct DirLock {
    relative_path: PathBuf,
    owner_token: String,
    directory_identity: RetainedDirectoryIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirLockFileState {
    pid: u32,
    acquired_unix: u64,
    token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirLockReclaimReason {
    DeadPid { pid: u32 },
    LegacyOrCorruptStale { age_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirLockLiveOwner {
    pid: u32,
    acquired_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirLockInspection {
    Reclaim(DirLockReclaimReason),
    HeldByLiveOwner(DirLockLiveOwner),
    Contended,
}

impl DirLock {
    /// Acquire the lock at `<dir>/<lockfile_name>`, retrying with backoff up to
    /// a bounded contention window. Orphan lockfiles are reclaimed when their
    /// owner pid is not alive. Legacy/corrupt lockfiles without a readable pid
    /// are reclaimed only after the stale threshold.
    ///
    /// # Errors
    ///
    /// Returns filesystem errors from creating/removing the lockfile, or
    /// `WouldBlock` when the lock is held by a live owner or remains contended.
    pub fn acquire(dir: &Path, lockfile_name: &str) -> std::io::Result<Self> {
        use std::io::ErrorKind;
        std::fs::create_dir_all(dir)?;
        let directory_identity = RetainedDirectoryIdentity::capture(dir)?;
        let relative_path = PathBuf::from(lockfile_name);
        validate_direct_relative_file(&relative_path)?;
        let lock_path = dir.join(lockfile_name);
        for attempt in 0..DIR_LOCK_RETRY_ATTEMPTS {
            let owner_token = new_owner_token();
            match directory_identity.create_new_direct_file(&relative_path) {
                Ok(mut f) => {
                    let state = DirLockFileState {
                        pid: std::process::id(),
                        acquired_unix: now_unix_seconds(),
                        token: owner_token.clone(),
                    };
                    if let Err(error) = write_lock_state(&mut f, &state) {
                        let _ = directory_identity.remove_direct_file(&relative_path);
                        return Err(error);
                    }
                    return Ok(DirLock {
                        relative_path,
                        owner_token,
                        directory_identity,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    match inspect_lock(&directory_identity, &relative_path)? {
                        DirLockInspection::Reclaim(reason) => {
                            match directory_identity.remove_direct_file(&relative_path) {
                                Ok(()) => continue,
                                Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {
                                    continue;
                                }
                                Err(remove_error) => {
                                    return Err(std::io::Error::new(
                                        remove_error.kind(),
                                        format!(
                                            "failed to reclaim stale directory lock {} ({}) because removal failed: {}",
                                            lock_path.display(),
                                            reason.describe(),
                                            remove_error
                                        ),
                                    ));
                                }
                            }
                        }
                        DirLockInspection::HeldByLiveOwner(owner) => {
                            return Err(std::io::Error::new(
                                ErrorKind::WouldBlock,
                                live_owner_lock_message(dir, &lock_path, lockfile_name, &owner),
                            ));
                        }
                        DirLockInspection::Contended => {}
                    }
                    let shift = attempt.min(5);
                    let backoff_ms = 2_u64.checked_shl(shift).unwrap_or(64);
                    std::thread::sleep(Duration::from_millis(backoff_ms));
                }
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            ErrorKind::WouldBlock,
            lock_contention_message(
                dir,
                &lock_path,
                lockfile_name,
                &directory_identity,
                &relative_path,
            ),
        ))
    }

    pub(crate) const fn directory_identity(&self) -> &RetainedDirectoryIdentity {
        &self.directory_identity
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let Ok(contents) = self
            .directory_identity
            .read_direct_file_bounded(&self.relative_path, 4096)
        else {
            return;
        };
        let Ok(contents) = std::str::from_utf8(&contents) else {
            return;
        };
        let Some(state) = parse_lock_state(contents) else {
            return;
        };
        if state.token == self.owner_token {
            let _ = self
                .directory_identity
                .remove_direct_file(&self.relative_path);
        }
    }
}

impl DirLockReclaimReason {
    fn describe(&self) -> String {
        match self {
            DirLockReclaimReason::DeadPid { pid } => format!("owner pid {pid} is not alive"),
            DirLockReclaimReason::LegacyOrCorruptStale { age_seconds } => {
                format!("legacy/corrupt lock age {age_seconds}s exceeds stale threshold")
            }
        }
    }
}

fn write_lock_state(f: &mut std::fs::File, state: &DirLockFileState) -> std::io::Result<()> {
    use std::io::Write;
    writeln!(f, "pid={}", state.pid)?;
    writeln!(f, "acquired_unix={}", state.acquired_unix)?;
    writeln!(f, "token={}", state.token)?;
    f.sync_all()
}

fn parse_lock_state(contents: &str) -> Option<DirLockFileState> {
    let mut pid = None;
    let mut acquired_unix = None;
    let mut token = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("pid=") {
            pid = value.parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("acquired_unix=") {
            acquired_unix = value.parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("token=") {
            if !value.is_empty() {
                token = Some(value.to_owned());
            }
        }
    }
    Some(DirLockFileState {
        pid: pid?,
        acquired_unix: acquired_unix?,
        token: token?,
    })
}

fn inspect_lock(
    directory: &RetainedDirectoryIdentity,
    relative_path: &Path,
) -> std::io::Result<DirLockInspection> {
    let contents = match directory.read_direct_file_bounded(relative_path, 4096) {
        Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "lockfile is not UTF-8")
        })?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(state) = contents.as_deref().and_then(parse_lock_state) {
        if process_may_be_alive(state.pid) {
            return Ok(DirLockInspection::HeldByLiveOwner(DirLockLiveOwner {
                pid: state.pid,
                acquired_unix: state.acquired_unix,
            }));
        }
        return Ok(DirLockInspection::Reclaim(DirLockReclaimReason::DeadPid {
            pid: state.pid,
        }));
    }

    Ok(directory.direct_file_modified_age(relative_path).map_or(
        DirLockInspection::Contended,
        |age| {
            let age_seconds = age.as_secs();
            if age >= DIR_LOCK_STALE_AFTER {
                DirLockInspection::Reclaim(DirLockReclaimReason::LegacyOrCorruptStale {
                    age_seconds,
                })
            } else {
                DirLockInspection::Contended
            }
        },
    ))
}

fn live_owner_lock_message(
    dir: &Path,
    lock_path: &Path,
    lockfile_name: &str,
    owner: &DirLockLiveOwner,
) -> String {
    format!(
        "directory lock contention on {}: {} is held by live owner pid={}, acquired_unix={}. Retry later; do not remove {} while that process is alive",
        dir.display(),
        lockfile_name,
        owner.pid,
        owner.acquired_unix,
        lock_path.display()
    )
}

fn lock_contention_message(
    dir: &Path,
    lock_path: &Path,
    lockfile_name: &str,
    directory: &RetainedDirectoryIdentity,
    relative_path: &Path,
) -> String {
    let lock_description = directory
        .read_direct_file_bounded(relative_path, 4096)
        .ok()
        .and_then(|contents| String::from_utf8(contents).ok())
        .and_then(|contents| parse_lock_state(&contents))
        .map_or_else(
            || "owner unknown (legacy/corrupt lockfile)".to_owned(),
            |state| {
                format!(
                    "owner pid={}, acquired_unix={}, token={}",
                    state.pid, state.acquired_unix, state.token
                )
            },
        );
    format!(
        "directory lock contention on {}: {} holds {}. Retry later, confirm the owner process is alive, or force-remove {} only after verifying it is orphaned",
        dir.display(),
        lock_description,
        lockfile_name,
        lock_path.display()
    )
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn new_owner_token() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{nonce}", std::process::id())
}

fn process_may_be_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    process_may_be_alive_platform(pid)
}

#[cfg(unix)]
fn process_may_be_alive_platform(pid: u32) -> bool {
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }

    let Ok(pid) = c_int::try_from(pid) else {
        return false;
    };

    // SAFETY: `kill` is called with signal 0, which performs permission and
    // existence checks without delivering a signal. `pid` was range-checked for
    // the platform C integer type above.
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }

    // The EPERM (Some(1)) arm and the catch-all `_` arm both return true on
    // purpose: EPERM means the process exists but we may not signal it, and an
    // unknown errno is treated conservatively as "alive" so a stale lock never
    // looks reusable. clippy::match_same_arms would merge them and lose the
    // EPERM comment that documents this safety-relevant distinction.
    #[allow(clippy::match_same_arms)]
    match std::io::Error::last_os_error().raw_os_error() {
        Some(1) => true,  // EPERM: process exists, but is not signalable.
        Some(3) => false, // ESRCH: no such process.
        _ => true,        // Unknown errors are treated conservatively as live.
    }
}

#[cfg(windows)]
fn process_may_be_alive_platform(pid: u32) -> bool {
    use std::ffi::c_void;
    type RawHandle = *mut c_void;

    const ERROR_INVALID_PARAMETER: u32 = 87;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "OpenProcess"]
        fn open_process(desired_access: u32, inherit_handle: i32, process_id: u32) -> RawHandle;
        #[link_name = "GetExitCodeProcess"]
        fn get_exit_code_process(process: RawHandle, exit_code: *mut u32) -> i32;
        #[link_name = "CloseHandle"]
        fn close_handle(object: RawHandle) -> i32;
        #[link_name = "GetLastError"]
        fn get_last_error() -> u32;
    }

    // SAFETY: The FFI calls use documented Win32 APIs. The handle returned by
    // `OpenProcess` is checked for null before use and is closed exactly once.
    unsafe {
        let handle = open_process(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return get_last_error() != ERROR_INVALID_PARAMETER;
        }

        let mut exit_code = 0;
        let alive = get_exit_code_process(handle, std::ptr::addr_of_mut!(exit_code)) != 0
            && exit_code == STILL_ACTIVE;
        let _ = close_handle(handle);
        alive
    }
}

#[cfg(not(any(unix, windows)))]
fn process_may_be_alive_platform(_pid: u32) -> bool {
    // Unsupported platforms are treated conservatively so a potentially live
    // owner is not stolen.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "forge-core-cli-io-util-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn write_state(path: &Path, state: &DirLockFileState) {
        let mut file = std::fs::File::create(path).expect("create lockfile");
        write_lock_state(&mut file, state).expect("write lock state");
    }

    #[test]
    fn atomic_write_writes_final_content_and_removes_temp_file() {
        let dir = temp_dir("atomic-write");
        let target = dir.join("contract.yaml");

        atomic_write(&target, "status: accepted\n").expect("atomic write");

        let contents = std::fs::read_to_string(&target).expect("read target");
        assert_eq!(contents, "status: accepted\n");
        let entries = std::fs::read_dir(&dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("read dir entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![std::ffi::OsString::from("contract.yaml")]);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_records_pid_timestamp_and_token() {
        let dir = temp_dir("records");
        let lock_path = dir.join(".lock");
        let lock = DirLock::acquire(&dir, ".lock").expect("acquire lock");

        let contents = std::fs::read_to_string(&lock_path).expect("read lockfile");
        let state = parse_lock_state(&contents).expect("parse lock state");
        assert_eq!(state.pid, std::process::id());
        assert!(state.acquired_unix > 0);
        assert!(!state.token.is_empty());

        drop(lock);
        assert!(!lock_path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_reclaims_dead_pid_lockfile() {
        let dir = temp_dir("dead-pid");
        let lock_path = dir.join(".lock");
        write_state(
            &lock_path,
            &DirLockFileState {
                pid: 0,
                acquired_unix: now_unix_seconds(),
                token: "dead-owner".to_owned(),
            },
        );

        let lock = DirLock::acquire(&dir, ".lock").expect("reclaim dead pid lock");
        let contents = std::fs::read_to_string(&lock_path).expect("read new lock");
        let state = parse_lock_state(&contents).expect("parse new lock");
        assert_eq!(state.pid, std::process::id());
        assert_ne!(state.token, "dead-owner");

        drop(lock);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_does_not_reclaim_stale_lockfile_owned_by_current_process() {
        let dir = temp_dir("stale-live-owner");
        let lock_path = dir.join(".lock");
        write_state(
            &lock_path,
            &DirLockFileState {
                pid: std::process::id(),
                acquired_unix: 1,
                token: "stale-live-owner".to_owned(),
            },
        );

        let Err(error) = DirLock::acquire(&dir, ".lock") else {
            panic!("live owner must not be reclaimed");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(
            error.to_string().contains("held by live owner"),
            "unexpected error: {error}"
        );
        let contents = std::fs::read_to_string(&lock_path).expect("read lock");
        let state = parse_lock_state(&contents).expect("parse lock state");
        assert_eq!(state.pid, std::process::id());
        assert_eq!(state.token, "stale-live-owner");

        let _ = std::fs::remove_file(&lock_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_drop_only_removes_matching_owner_token() {
        let dir = temp_dir("drop-token");
        let lock_path = dir.join(".lock");
        let lock = DirLock::acquire(&dir, ".lock").expect("acquire lock");

        write_state(
            &lock_path,
            &DirLockFileState {
                pid: std::process::id(),
                acquired_unix: now_unix_seconds(),
                token: "different-owner".to_owned(),
            },
        );

        drop(lock);
        assert!(lock_path.exists());
        let _ = std::fs::remove_file(&lock_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retained_projection_reader_preserves_exact_bytes() {
        let dir = temp_dir("projection");
        let path = dir.join("registry.yaml");
        let expected = b"schema_version: test\n# exact comment\n";
        std::fs::write(&path, expected).unwrap();
        assert_eq!(
            read_regular_file_no_follow_bounded(&path, 1024).unwrap(),
            expected
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retained_directory_projection_reads_original_after_parent_swap() {
        let dir = temp_dir("projection-parent-swap");
        std::fs::write(dir.join("registry.yaml"), b"trusted: true\n").unwrap();
        let identity = RetainedDirectoryIdentity::capture(&dir).unwrap();
        let moved = dir.with_extension("moved");
        std::fs::rename(&dir, &moved).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("registry.yaml"), b"attacker: true\n").unwrap();

        let bytes = identity
            .read_direct_file_bounded(Path::new("registry.yaml"), 1024)
            .unwrap();
        assert_eq!(bytes, b"trusted: true\n");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(moved);
    }
    #[test]
    fn retained_swap_read_restore_aba_cannot_substitute_bytes() {
        let dir = temp_dir("swap-read-restore");
        let moved = dir.with_extension("retained");
        let replacement = dir.with_extension("replacement");
        std::fs::write(dir.join("registry.yaml"), b"trusted: true\n").unwrap();
        let identity = RetainedDirectoryIdentity::capture(&dir).unwrap();
        std::fs::rename(&dir, &moved).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("registry.yaml"), b"attacker: true\n").unwrap();

        let file = open_file_relative_no_follow(
            &identity.handle,
            Path::new("registry.yaml"),
            RelativeOpen::Read,
        )
        .unwrap();
        std::fs::rename(&dir, &replacement).unwrap();
        std::fs::rename(&moved, &dir).unwrap();
        let bytes = read_open_regular_file_bounded(file, &dir.join("registry.yaml"), 1024).unwrap();
        assert_eq!(bytes, b"trusted: true\n");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(replacement);
    }

    #[test]
    fn retained_lock_restore_aba_blocks_exact_producer_path() {
        let dir = temp_dir("lock-restore");
        let moved = dir.with_extension("retained");
        let replacement = dir.with_extension("replacement");
        let retained_root = forge_core_store::RetainedEffectStoreRoot::acquire(&dir).unwrap();
        std::fs::rename(&dir, &moved).unwrap();
        std::fs::create_dir_all(&dir).unwrap();

        let retained =
            acquire_effect_store_lock_retained(&retained_root, ".producer.lock").unwrap();
        std::fs::rename(&dir, &replacement).unwrap();
        std::fs::rename(&moved, &dir).unwrap();

        let error = forge_core_store::try_acquire_effect_store_lock(&dir, ".producer.lock")
            .expect_err("restored producer path must remain locked");
        assert!(matches!(
            error,
            forge_core_store::EffectStoreLockError::WouldBlock { .. }
        ));
        drop(retained);
        drop(retained_root);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(replacement);
    }
    #[test]
    fn retained_crash_replace_uses_exact_lock_and_target_handles() {
        use forge_core_store::retained_crash_replace::{
            recover_file_crash_safe_under_retained_lock,
            replace_file_crash_safe_under_retained_lock,
            retain_file_crash_safe_expected_leaf_under_retained_lock,
        };
        use forge_core_store::RetainedEffectStoreExpectedLeaf;
        let dir = temp_dir("retained-crash-replace");
        let retained_root = forge_core_store::RetainedEffectStoreRoot::acquire(&dir).unwrap();
        let lock = acquire_effect_store_lock_retained(&retained_root, ".producer.lock").unwrap();
        let recovered =
            recover_file_crash_safe_under_retained_lock(&lock, Path::new("authority.yaml"), 1024)
                .unwrap();
        assert!(recovered.target_digest.is_none());
        let mut expected = retain_file_crash_safe_expected_leaf_under_retained_lock(
            &lock,
            Path::new("authority.yaml"),
            1024,
        )
        .unwrap();
        assert!(expected.digest().is_none());
        let mut installed = replace_file_crash_safe_under_retained_lock(
            &lock,
            Path::new("authority.yaml"),
            &mut expected,
            b"trusted: true\n",
            1024,
        )
        .unwrap();
        let installed_digest = installed.digest().to_owned();
        assert_eq!(installed.raw_bytes(), b"trusted: true\n");
        installed.revalidate().unwrap();
        assert_eq!(
            recover_file_crash_safe_under_retained_lock(&lock, Path::new("authority.yaml"), 1024,)
                .unwrap()
                .target_digest,
            Some(installed_digest.clone())
        );
        let mut expected = RetainedEffectStoreExpectedLeaf::Present(installed);
        let mut replaced = replace_file_crash_safe_under_retained_lock(
            &lock,
            Path::new("authority.yaml"),
            &mut expected,
            b"trusted: next\n",
            1024,
        )
        .unwrap();
        assert_eq!(
            replaced.digest(),
            forge_core_store::sha256_content_hash(b"trusted: next\n")
        );
        assert_eq!(replaced.raw_bytes(), b"trusted: next\n");
        replaced.revalidate().unwrap();
        drop(lock);
        drop(retained_root);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn retained_projection_reader_rejects_hard_links() {
        let dir = temp_dir("projection-hard-link");
        let path = dir.join("registry.yaml");
        std::fs::write(&path, b"public: true\n").unwrap();
        std::fs::hard_link(&path, dir.join("alias.yaml")).unwrap();
        let error = read_regular_file_no_follow_bounded(&path, 1024).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[cfg(windows)]
    #[test]
    fn windows_retained_handles_reject_file_and_directory_reparse_points() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let dir = temp_dir("windows-reparse");
        let target = dir.join("target.yaml");
        let file_link = dir.join("link.yaml");
        std::fs::write(&target, b"trusted: true\n").unwrap();
        if let Err(error) = symlink_file(&target, &file_link) {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                let _ = std::fs::remove_dir_all(dir);
                return;
            }
            panic!("create file reparse point: {error}");
        }
        assert!(read_regular_file_no_follow_bounded(&file_link, 1024).is_err());

        let real_directory = dir.join("real-directory");
        let directory_link = dir.join("directory-link");
        std::fs::create_dir_all(&real_directory).unwrap();
        symlink_dir(&real_directory, &directory_link).unwrap();
        assert!(RetainedDirectoryIdentity::capture(&directory_link).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
