//! Crash-recoverable replacement of one authority-bearing file.
//!
//! The protocol keeps fixed `next`, `previous`, and `transaction` siblings.
//! Every transition is digest-bound and must run while the caller retains the
//! exact OS lock supplied to this module. Recovery therefore chooses only the
//! marker-bound old or new bytes; ambiguous filesystem state fails closed.

use super::{
    ensure_resolved_parent_within_root, resolve_safe_repo_relative, sha256_content_hash,
    EffectStoreLock,
};
use std::fmt;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::{ffi::c_void, os::windows::ffi::OsStrExt};

#[cfg(windows)]
type WinHandle = *mut c_void;
#[cfg(windows)]
const GENERIC_WRITE: u32 = 0x4000_0000;
#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;
#[cfg(windows)]
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
#[cfg(windows)]
const FILE_SHARE_DELETE: u32 = 0x0000_0004;
#[cfg(windows)]
const OPEN_EXISTING: u32 = 3;
#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: WinHandle = -1_isize as WinHandle;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        file_name: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *mut c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: WinHandle,
    ) -> WinHandle;
    fn FlushFileBuffers(file: WinHandle) -> i32;
    fn CloseHandle(object: WinHandle) -> i32;
}

const PROTOCOL_VERSION: &str = "forge-crash-replace-v1";
const NEXT_SUFFIX: &str = "forge-next";
const PREVIOUS_SUFFIX: &str = "forge-previous";
const TRANSACTION_SUFFIX: &str = "forge-transaction";
const MARKER_MAX_BYTES: u64 = 512;

/// Durable phases exposed only so focused tests can simulate process loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplacePhase {
    NextSynced,
    TransactionSynced,
    PreviousInstalled,
    TargetInstalled,
}

/// The deterministic action taken while reconciling replacement sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplaceRecoveryAction {
    Noop,
    RemovedUncommittedNext,
    AbortedToPrevious,
    RestoredPrevious,
    CommittedInitial,
    CleanedCommitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReplaceRecovery {
    pub action: CrashReplaceRecoveryAction,
    pub target_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReplaceResult {
    pub previous_digest: Option<String>,
    pub installed_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplaceError {
    InvalidArgument {
        field: &'static str,
        reason: String,
    },
    InvalidPath {
        field: &'static str,
        path: String,
    },
    LockScopeMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    CompareAndSwapMismatch {
        expected: Option<String>,
        actual: Option<String>,
    },
    SizeLimit {
        path: PathBuf,
        found: u64,
        maximum: u64,
    },
    Protocol {
        reason: String,
    },
    Io {
        path: PathBuf,
        source: String,
    },
    InjectedFault {
        phase: CrashReplacePhase,
    },
}

impl fmt::Display for CrashReplaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { field, reason } => {
                write!(formatter, "invalid crash-replace {field}: {reason}")
            }
            Self::InvalidPath { field, path } => {
                write!(formatter, "invalid crash-replace {field} path {path}")
            }
            Self::LockScopeMismatch { expected, actual } => write!(
                formatter,
                "crash-replace lock scope mismatch: expected {}, actual {}",
                expected.display(),
                actual.display()
            ),
            Self::CompareAndSwapMismatch { expected, actual } => write!(
                formatter,
                "crash-replace compare-and-swap mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::SizeLimit {
                path,
                found,
                maximum,
            } => write!(
                formatter,
                "crash-replace file {} exceeds size limit: {found} > {maximum}",
                path.display()
            ),
            Self::Protocol { reason } => {
                write!(formatter, "crash-replace protocol error: {reason}")
            }
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "crash-replace I/O {} failed: {source}",
                    path.display()
                )
            }
            Self::InjectedFault { phase } => {
                write!(formatter, "injected crash-replace fault after {phase:?}")
            }
        }
    }
}

impl std::error::Error for CrashReplaceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementMarker {
    previous_digest: Option<String>,
    next_digest: String,
}

#[derive(Debug, Clone)]
struct ReplacementPaths {
    next: PathBuf,
    previous: PathBuf,
    transaction: PathBuf,
}

/// Replace one confined regular file after an exact digest CAS.
///
/// `expected_previous_digest = None` means the target must be absent. The
/// caller must retain `lock` for the complete call and must use the matching
/// lock scope for all readers and writers of this target.
///
/// # Errors
///
/// Fails closed on invalid paths/digests/lock scope, CAS mismatch, size limits,
/// ambiguous recovery state, or any durable filesystem operation failure.
pub fn replace_file_crash_safe_under_lock(
    root: impl AsRef<Path>,
    lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    target_relative_path: &str,
    expected_previous_digest: Option<&str>,
    content: &[u8],
    maximum_bytes: u64,
) -> Result<CrashReplaceResult, CrashReplaceError> {
    replace_file_crash_safe_under_lock_with_fault(
        root,
        lock,
        expected_lock_relative_path,
        target_relative_path,
        expected_previous_digest,
        content,
        maximum_bytes,
        None,
    )
}

/// Test seam for deterministic process-loss simulation.
///
/// An injected fault intentionally leaves protocol artifacts on disk so a
/// subsequent [`recover_file_crash_safe_under_lock`] call exercises recovery.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn replace_file_crash_safe_under_lock_with_fault(
    root: impl AsRef<Path>,
    lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    target_relative_path: &str,
    expected_previous_digest: Option<&str>,
    content: &[u8],
    maximum_bytes: u64,
    fault_after: Option<CrashReplacePhase>,
) -> Result<CrashReplaceResult, CrashReplaceError> {
    validate_maximum(maximum_bytes)?;
    let content_len = u64::try_from(content.len()).unwrap_or(u64::MAX);
    if content_len > maximum_bytes {
        return Err(CrashReplaceError::SizeLimit {
            path: PathBuf::from(target_relative_path),
            found: content_len,
            maximum: maximum_bytes,
        });
    }
    validate_optional_digest(expected_previous_digest)?;

    let root = root.as_ref();
    let target = resolve_confined(root, "target", target_relative_path)?;
    let expected_lock = validate_lock_scope(root, lock, expected_lock_relative_path)?;
    validate_target_name_and_scope(&target, &expected_lock)?;
    let parent = target
        .parent()
        .ok_or_else(|| CrashReplaceError::InvalidPath {
            field: "target",
            path: target_relative_path.to_owned(),
        })?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    ensure_resolved_parent_within_root(root, &target).map_err(|source| io_error(parent, source))?;

    let _ = reconcile_target(&target, maximum_bytes)?;
    let previous_digest = file_digest_if_regular(&target, maximum_bytes, "target")?;
    let expected = expected_previous_digest.map(str::to_owned);
    if previous_digest != expected {
        return Err(CrashReplaceError::CompareAndSwapMismatch {
            expected,
            actual: previous_digest,
        });
    }

    let paths = replacement_paths(&target)?;
    ensure_protocol_paths_absent(&paths)?;
    let marker = ReplacementMarker {
        previous_digest: expected_previous_digest.map(str::to_owned),
        next_digest: sha256_content_hash(content),
    };

    write_new_synced_file(&paths.next, content)?;
    sync_parent_dir(parent)?;
    inject_fault(fault_after, CrashReplacePhase::NextSynced)?;

    write_new_synced_file(&paths.transaction, &encode_marker(&marker))?;
    sync_parent_dir(parent)?;
    inject_fault(fault_after, CrashReplacePhase::TransactionSynced)?;

    if marker.previous_digest.is_some() {
        ensure_digest_matches(
            "target before previous install",
            file_digest_if_regular(&target, maximum_bytes, "target")?.as_deref(),
            marker.previous_digest.as_deref().expect("checked Some"),
        )?;
        fs::rename(&target, &paths.previous).map_err(|source| io_error(&target, source))?;
        sync_parent_dir(parent)?;
        ensure_digest_matches(
            "installed previous",
            file_digest_if_regular(&paths.previous, maximum_bytes, "previous")?.as_deref(),
            marker.previous_digest.as_deref().expect("checked Some"),
        )?;
        inject_fault(fault_after, CrashReplacePhase::PreviousInstalled)?;
    }

    fs::rename(&paths.next, &target).map_err(|source| io_error(&paths.next, source))?;
    sync_parent_dir(parent)?;
    ensure_digest_matches(
        "installed target",
        file_digest_if_regular(&target, maximum_bytes, "target")?.as_deref(),
        &marker.next_digest,
    )?;
    inject_fault(fault_after, CrashReplacePhase::TargetInstalled)?;

    if marker.previous_digest.is_some() {
        remove_regular_file(&paths.previous, "previous")?;
        sync_parent_dir(parent)?;
    }
    remove_regular_file(&paths.transaction, "transaction")?;
    sync_parent_dir(parent)?;

    Ok(CrashReplaceResult {
        previous_digest: marker.previous_digest,
        installed_digest: marker.next_digest,
    })
}

/// Reconcile a prior interrupted replacement while retaining the exact lock.
///
/// Ambiguous, unbound, oversized, non-regular, or digest-mismatched protocol
/// artifacts fail closed rather than guessing which bytes should become active.
///
/// # Errors
///
/// Returns a typed validation, lock-scope, protocol, size, or durable I/O error
/// when an exact prior/target state cannot be proven.
pub fn recover_file_crash_safe_under_lock(
    root: impl AsRef<Path>,
    lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    target_relative_path: &str,
    maximum_bytes: u64,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    validate_maximum(maximum_bytes)?;
    let root = root.as_ref();
    let target = resolve_confined(root, "target", target_relative_path)?;
    let expected_lock = validate_lock_scope(root, lock, expected_lock_relative_path)?;
    validate_target_name_and_scope(&target, &expected_lock)?;
    let Some(parent) = target.parent() else {
        return Err(CrashReplaceError::InvalidPath {
            field: "target",
            path: target_relative_path.to_owned(),
        });
    };
    if !parent.exists() {
        return Ok(CrashReplaceRecovery {
            action: CrashReplaceRecoveryAction::Noop,
            target_digest: None,
        });
    }
    ensure_resolved_parent_within_root(root, &target).map_err(|source| io_error(parent, source))?;
    reconcile_target(&target, maximum_bytes)
}

fn reconcile_target(
    target: &Path,
    maximum_bytes: u64,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    let paths = replacement_paths(target)?;
    let marker_bytes =
        read_regular_file_bounded(&paths.transaction, MARKER_MAX_BYTES, "transaction marker")?;
    let target_digest = file_digest_if_regular(target, maximum_bytes, "target")?;
    let next_digest = file_digest_if_regular(&paths.next, maximum_bytes, "next")?;
    let previous_digest = file_digest_if_regular(&paths.previous, maximum_bytes, "previous")?;

    let Some(marker_bytes) = marker_bytes else {
        if previous_digest.is_some() {
            return protocol_error("previous file exists without a transaction marker");
        }
        if next_digest.is_some() {
            if target_digest.is_none() {
                return protocol_error("next file exists without a marker or durable target");
            }
            remove_regular_file(&paths.next, "next")?;
            sync_target_parent(target)?;
            return Ok(CrashReplaceRecovery {
                action: CrashReplaceRecoveryAction::RemovedUncommittedNext,
                target_digest,
            });
        }
        return Ok(CrashReplaceRecovery {
            action: CrashReplaceRecoveryAction::Noop,
            target_digest,
        });
    };

    let marker = parse_marker(&marker_bytes)?;
    ensure_optional_digest_matches("next", next_digest.as_deref(), &marker.next_digest)?;
    if let Some(expected_previous) = marker.previous_digest.as_deref() {
        ensure_optional_digest_matches("previous", previous_digest.as_deref(), expected_previous)?;
    } else if previous_digest.is_some() {
        return protocol_error("unexpected previous file for an initially empty transaction");
    }

    match target_digest.as_deref() {
        Some(found) if found == marker.next_digest => {
            if next_digest.is_some() {
                return protocol_error("committed target coexists with a next file");
            }
            if previous_digest.is_some() {
                remove_regular_file(&paths.previous, "previous")?;
                sync_target_parent(target)?;
            }
            remove_regular_file(&paths.transaction, "transaction")?;
            sync_target_parent(target)?;
            Ok(CrashReplaceRecovery {
                action: CrashReplaceRecoveryAction::CleanedCommitted,
                target_digest: Some(marker.next_digest),
            })
        }
        Some(found) if marker.previous_digest.as_deref() == Some(found) => {
            if previous_digest.is_some() {
                return protocol_error("old target coexists with a previous file");
            }
            if next_digest.is_some() {
                remove_regular_file(&paths.next, "next")?;
                sync_target_parent(target)?;
            }
            remove_regular_file(&paths.transaction, "transaction")?;
            sync_target_parent(target)?;
            Ok(CrashReplaceRecovery {
                action: CrashReplaceRecoveryAction::AbortedToPrevious,
                target_digest: Some(found.to_owned()),
            })
        }
        Some(_) => protocol_error("target digest is not bound by the transaction marker"),
        None => recover_missing_target(
            target,
            &paths,
            &marker,
            next_digest.is_some(),
            previous_digest.is_some(),
        ),
    }
}

fn recover_missing_target(
    target: &Path,
    paths: &ReplacementPaths,
    marker: &ReplacementMarker,
    next_exists: bool,
    previous_exists: bool,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    if let Some(previous_digest) = marker.previous_digest.as_deref() {
        if !previous_exists {
            return protocol_error("target and marker-bound previous file are both missing");
        }
        fs::rename(&paths.previous, target).map_err(|source| io_error(&paths.previous, source))?;
        sync_target_parent(target)?;
        if next_exists {
            remove_regular_file(&paths.next, "next")?;
            sync_target_parent(target)?;
        }
        remove_regular_file(&paths.transaction, "transaction")?;
        sync_target_parent(target)?;
        return Ok(CrashReplaceRecovery {
            action: CrashReplaceRecoveryAction::RestoredPrevious,
            target_digest: Some(previous_digest.to_owned()),
        });
    }
    if previous_exists || !next_exists {
        return protocol_error("initial replacement transaction is incomplete or inconsistent");
    }
    fs::rename(&paths.next, target).map_err(|source| io_error(&paths.next, source))?;
    sync_target_parent(target)?;
    remove_regular_file(&paths.transaction, "transaction")?;
    sync_target_parent(target)?;
    Ok(CrashReplaceRecovery {
        action: CrashReplaceRecoveryAction::CommittedInitial,
        target_digest: Some(marker.next_digest.clone()),
    })
}

fn validate_maximum(maximum_bytes: u64) -> Result<(), CrashReplaceError> {
    if maximum_bytes == 0 {
        return Err(CrashReplaceError::InvalidArgument {
            field: "maximum_bytes",
            reason: "must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn validate_optional_digest(value: Option<&str>) -> Result<(), CrashReplaceError> {
    if let Some(value) = value {
        validate_digest(value)?;
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), CrashReplaceError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return protocol_error("digest has no sha256 prefix");
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return protocol_error("digest is not lowercase sha256 hex");
    }
    Ok(())
}

fn resolve_confined(
    root: &Path,
    field: &'static str,
    relative_path: &str,
) -> Result<PathBuf, CrashReplaceError> {
    resolve_safe_repo_relative(root, relative_path).map_err(|_| CrashReplaceError::InvalidPath {
        field,
        path: relative_path.to_owned(),
    })
}

fn validate_lock_scope(
    root: &Path,
    lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
) -> Result<PathBuf, CrashReplaceError> {
    let expected = resolve_confined(root, "lock", expected_lock_relative_path)?;
    let expected = expected.canonicalize().unwrap_or(expected);
    let actual = lock
        .path()
        .canonicalize()
        .unwrap_or_else(|_| lock.path().to_path_buf());
    if expected != actual {
        return Err(CrashReplaceError::LockScopeMismatch { expected, actual });
    }
    Ok(expected)
}

fn validate_target_name_and_scope(
    target: &Path,
    expected_lock: &Path,
) -> Result<(), CrashReplaceError> {
    if target == expected_lock {
        return Err(CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target cannot be the retained lock file".to_owned(),
        });
    }
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target must have a UTF-8 file name".to_owned(),
        })?;
    if [NEXT_SUFFIX, PREVIOUS_SUFFIX, TRANSACTION_SUFFIX]
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return Err(CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target name collides with reserved protocol suffix".to_owned(),
        });
    }
    Ok(())
}

fn replacement_paths(target: &Path) -> Result<ReplacementPaths, CrashReplaceError> {
    let parent = target
        .parent()
        .ok_or_else(|| CrashReplaceError::InvalidPath {
            field: "target",
            path: target.display().to_string(),
        })?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target must have a UTF-8 file name".to_owned(),
        })?;
    Ok(ReplacementPaths {
        next: parent.join(format!(".{file_name}.{NEXT_SUFFIX}")),
        previous: parent.join(format!(".{file_name}.{PREVIOUS_SUFFIX}")),
        transaction: parent.join(format!(".{file_name}.{TRANSACTION_SUFFIX}")),
    })
}

fn ensure_protocol_paths_absent(paths: &ReplacementPaths) -> Result<(), CrashReplaceError> {
    for (path, label) in [
        (&paths.next, "next"),
        (&paths.previous, "previous"),
        (&paths.transaction, "transaction"),
    ] {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return protocol_error(&format!("{label} sidecar remains after reconciliation"));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(path, error)),
        }
    }
    Ok(())
}

fn encode_marker(marker: &ReplacementMarker) -> Vec<u8> {
    let previous = marker.previous_digest.as_deref().unwrap_or("absent");
    format!(
        "{PROTOCOL_VERSION}\nprevious={previous}\nnext={}\n",
        marker.next_digest
    )
    .into_bytes()
}

fn parse_marker(bytes: &[u8]) -> Result<ReplacementMarker, CrashReplaceError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| protocol_io_error("transaction marker is not UTF-8"))?;
    if !text.ends_with('\n') {
        return protocol_error("transaction marker has a torn tail");
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() != 3 || lines[0] != PROTOCOL_VERSION {
        return protocol_error("transaction marker has unsupported shape or version");
    }
    let previous = lines[1]
        .strip_prefix("previous=")
        .ok_or_else(|| protocol_io_error("transaction marker has no previous digest"))?;
    let next = lines[2]
        .strip_prefix("next=")
        .ok_or_else(|| protocol_io_error("transaction marker has no next digest"))?;
    let previous_digest = if previous == "absent" {
        None
    } else {
        validate_digest(previous)?;
        Some(previous.to_owned())
    };
    validate_digest(next)?;
    Ok(ReplacementMarker {
        previous_digest,
        next_digest: next.to_owned(),
    })
}

fn file_digest_if_regular(
    path: &Path,
    maximum: u64,
    label: &str,
) -> Result<Option<String>, CrashReplaceError> {
    read_regular_file_bounded(path, maximum, label)
        .map(|content| content.map(|bytes| sha256_content_hash(&bytes)))
}

fn read_regular_file_bounded(
    path: &Path,
    maximum: u64,
    label: &str,
) -> Result<Option<Vec<u8>>, CrashReplaceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    if !metadata.file_type().is_file() {
        return protocol_error(&format!("{label} is not a confined regular file"));
    }
    if metadata.len() > maximum {
        return Err(CrashReplaceError::SizeLimit {
            path: path.to_path_buf(),
            found: metadata.len(),
            maximum,
        });
    }
    fs::read(path)
        .map(Some)
        .map_err(|source| io_error(path, source))
}

fn write_new_synced_file(path: &Path, content: &[u8]) -> Result<(), CrashReplaceError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    if let Err(source) = file.write_all(content).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(io_error(path, source));
    }
    Ok(())
}

fn remove_regular_file(path: &Path, label: &str) -> Result<(), CrashReplaceError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.file_type().is_file() {
        return protocol_error(&format!("{label} is not a confined regular file"));
    }
    fs::remove_file(path).map_err(|source| io_error(path, source))
}

fn ensure_optional_digest_matches(
    label: &str,
    found: Option<&str>,
    expected: &str,
) -> Result<(), CrashReplaceError> {
    if found.is_some_and(|digest| digest != expected) {
        return protocol_error(&format!("{label} digest does not match transaction marker"));
    }
    Ok(())
}

fn ensure_digest_matches(
    label: &str,
    found: Option<&str>,
    expected: &str,
) -> Result<(), CrashReplaceError> {
    if found == Some(expected) {
        Ok(())
    } else {
        protocol_error(&format!("{label} digest does not match transaction marker"))
    }
}

fn inject_fault(
    configured: Option<CrashReplacePhase>,
    current: CrashReplacePhase,
) -> Result<(), CrashReplaceError> {
    if configured == Some(current) {
        Err(CrashReplaceError::InjectedFault { phase: current })
    } else {
        Ok(())
    }
}

fn sync_target_parent(target: &Path) -> Result<(), CrashReplaceError> {
    let parent = target
        .parent()
        .ok_or_else(|| CrashReplaceError::InvalidPath {
            field: "target",
            path: target.display().to_string(),
        })?;
    sync_parent_dir(parent)
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> Result<(), CrashReplaceError> {
    File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|source| io_error(parent, source))
}

#[cfg(windows)]
fn sync_parent_dir(parent: &Path) -> Result<(), CrashReplaceError> {
    if parent.as_os_str().is_empty() {
        return Err(CrashReplaceError::InvalidPath {
            field: "parent",
            path: parent.display().to_string(),
        });
    }
    let mut wide = parent.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    // SAFETY: `wide` is a live NUL-terminated UTF-16 path; all optional
    // pointers are null; constants follow the CreateFileW contract. The
    // returned handle is checked and closed exactly once below.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io_error(parent, io::Error::last_os_error()));
    }

    // SAFETY: `handle` is a successful CreateFileW directory handle and stays
    // live until the immediately following CloseHandle call.
    let flush_error = if unsafe { FlushFileBuffers(handle) } == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    // SAFETY: `handle` is valid and has not previously been closed.
    let close_error = if unsafe { CloseHandle(handle) } == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    if let Some(source) = flush_error {
        return Err(io_error(parent, source));
    }
    if let Some(source) = close_error {
        return Err(io_error(parent, source));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn sync_parent_dir(parent: &Path) -> Result<(), CrashReplaceError> {
    Err(io_error(
        parent,
        io::Error::new(
            io::ErrorKind::Unsupported,
            "durable directory synchronization is unsupported on this platform",
        ),
    ))
}

fn protocol_error<T>(reason: &str) -> Result<T, CrashReplaceError> {
    Err(protocol_io_error(reason))
}

fn protocol_io_error(reason: &str) -> CrashReplaceError {
    CrashReplaceError::Protocol {
        reason: reason.to_owned(),
    }
}

#[allow(clippy::needless_pass_by_value)] // Terminal I/O errors are reduced to stable owned text.
fn io_error(path: &Path, source: io::Error) -> CrashReplaceError {
    CrashReplaceError::Io {
        path: path.to_path_buf(),
        source: source.to_string(),
    }
}

#[cfg(all(test, windows))]
mod windows_directory_sync_tests {
    use super::{sync_parent_dir, CrashReplaceError};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn directory_flush_succeeds_and_open_failure_is_propagated() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "forge-crash-replace-directory-sync-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create directory");

        sync_parent_dir(&directory).expect("flush real Windows directory");
        fs::remove_dir(&directory).expect("remove directory");

        let error = sync_parent_dir(&directory).expect_err("missing directory must fail");
        assert!(matches!(
            error,
            CrashReplaceError::Io { path, .. } if path == directory
        ));
    }
}
