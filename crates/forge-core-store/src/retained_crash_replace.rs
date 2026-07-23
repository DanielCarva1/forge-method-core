//! Descriptor-relative crash replacement through exact Store-owned authority.

use crate::{
    crash_replace::{
        CrashReplaceError, CrashReplacePhase, CrashReplaceRecovery, CrashReplaceRecoveryAction,
        CrashReplaceResult,
    },
    retained_dir::{RetainedDirectory, RetainedFileIdentity, RetainedLeafPolicy},
    sha256_content_hash, EffectStoreLock, EffectStoreLockError,
};
use std::fmt;
use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Component, Path, PathBuf};

const PROTOCOL_VERSION: &str = "forge-crash-replace-v1";
const MARKER_MAX_BYTES: u64 = 512;
const ABSENCE_CLAIM_PREFIX: &[u8] = b"forge-crash-absence-v1\nnonce=";
const ABSENCE_CLAIM_BYTES: u64 = 62;

/// Opaque Store-created authority for one exact crash-replacement leaf.
///
/// The target root, parent directory, leaf, and producer lock are retained and
/// privately bound at construction. Callers cannot construct this type or swap
/// in a different leaf when invoking recovery or replacement.
#[derive(Debug)]
pub(crate) struct RetainedCrashReplaceTarget<'lock> {
    directory: RetainedDirectory,
    directory_relative_path: PathBuf,
    target_name: PathBuf,
    target_relative_path: PathBuf,
    target_path: PathBuf,
    directory_identity: RetainedFileIdentity,
    state_root_identity: RetainedFileIdentity,
    lock: &'lock EffectStoreLock,
}

impl<'lock> RetainedCrashReplaceTarget<'lock> {
    pub(crate) fn new(
        lock: &'lock EffectStoreLock,
        directory: RetainedDirectory,
        target_relative_path: PathBuf,
    ) -> io::Result<Self> {
        let directory_relative_path = target_relative_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let target_name = PathBuf::from(target_relative_path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained crash-replacement target has no leaf",
            )
        })?);
        let directory_identity = directory.identity()?;
        let state_root_identity = lock.state_root.identity()?;
        let target_path = lock.state_root.display_path().join(&target_relative_path);
        Ok(Self {
            directory,
            directory_relative_path,
            target_name,
            target_relative_path,
            target_path,
            directory_identity,
            state_root_identity,
            lock,
        })
    }

    fn validate(&self) -> io::Result<()> {
        self.lock
            .boundary
            .validate_root(self.lock.state_root.display_path())
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        self.lock
            .boundary
            .require_effect_authority()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        self.lock
            .validate_retained_lock_file()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        if self.lock.state_root.identity()? != self.state_root_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained crash-replacement root identity changed",
            ));
        }
        if self.directory.identity()? != self.directory_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained crash-replacement directory identity changed",
            ));
        }
        let current_directory = if self.directory_relative_path.as_os_str().is_empty() {
            self.lock.state_root.try_clone()
        } else {
            self.lock
                .state_root
                .open_directory(&self.directory_relative_path)
        }?;
        if current_directory.identity()? != self.directory_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained crash-replacement directory is no longer bound beneath the locked root",
            ));
        }
        if self.directory_relative_path.join(&self.target_name) != self.target_relative_path
            || self.target_path
                != self
                    .lock
                    .state_root
                    .display_path()
                    .join(&self.target_relative_path)
            || self.target_relative_path == self.lock.state_lock_relative_path
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained crash-replacement leaf binding changed",
            ));
        }
        self.lock
            .boundary
            .validate_root(self.lock.state_root.display_path())
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))
    }

    fn read_file_bounded(&self, name: &Path, maximum: u64) -> io::Result<Option<Vec<u8>>> {
        self.validate()?;
        match self.directory.read_authority_bounded(name, maximum) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn write_new_file_synced(&self, name: &Path, content: &[u8]) -> io::Result<()> {
        self.validate()?;
        let authority = self.directory.retain_authority()?;
        authority.write_new_file_synced(name, content)?;
        self.validate()
    }

    fn rename_file_if_digest(
        &self,
        from: &Path,
        to: &Path,
        expected_digest: &str,
        expected_identity: Option<&RetainedFileIdentity>,
        maximum: u64,
    ) -> io::Result<RetainedDigest> {
        self.validate()?;
        let authority = self.directory.retain_authority()?;
        let mut retained_source = None;
        authority.rename_file_noreplace_with_validation(from, to, |directory, source, _| {
            validate_expected_digest(directory, source, expected_digest, maximum)?;
            let mut file = directory.open_leaf_read(source, RetainedLeafPolicy::Authority)?;
            let identity = RetainedDirectory::identity_of(&file)?;
            directory.verify_retained_authority_binding(source, &file, &identity)?;
            if expected_identity.is_some_and(|expected| expected != &identity) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained crash-replacement source identity differs from the expected exact leaf",
                ));
            }
            let digest = read_retained_digest(&mut file, maximum)?;
            if digest != expected_digest {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained crash-replacement source digest changed before publication",
                ));
            }
            directory.verify_retained_authority_binding(source, &file, &identity)?;
            let link_count = retained_link_count(&file)?;
            retained_source = Some(RetainedDigest {
                file,
                identity,
                digest,
                link_count,
            });
            Ok(())
        })?;
        let mut retained = retained_source.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained crash-replacement publication produced no exact source witness",
            )
        })?;
        // Publication may add a Store-owned cleanup-debt alias on Unix. Bind the
        // installed witness to that post-publication link shape so later aliases
        // created outside this operation are still detected.
        retained.link_count = retained_link_count(&retained.file)?;
        self.revalidate_digest(to, &mut retained, maximum)?;
        self.validate()?;
        Ok(retained)
    }

    fn remove_file_if_digest_with_validation<F>(
        &self,
        name: &Path,
        expected_digest: &str,
        maximum: u64,
        mut validation: F,
    ) -> io::Result<()>
    where
        F: FnMut() -> io::Result<()>,
    {
        self.validate()?;
        let authority = self.directory.retain_authority()?;
        let _cleanup_debt = authority.remove_file_with_validation(name, |directory, source| {
            validate_expected_digest(directory, source, expected_digest, maximum)?;
            validation()
        })?;
        self.validate()
    }

    fn sync(&self) -> io::Result<()> {
        self.validate()?;
        self.directory.sync_root()
    }
}

fn validate_expected_digest(
    directory: &RetainedDirectory,
    source: &Path,
    expected_digest: &str,
    maximum: u64,
) -> io::Result<()> {
    let bytes = directory.read_authority_bounded(source, maximum)?;
    if sha256_content_hash(&bytes) == expected_digest {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement source digest changed before mutation",
        ))
    }
}

#[derive(Debug)]
struct RetainedDigest {
    file: File,
    identity: RetainedFileIdentity,
    digest: String,
    link_count: u64,
}

/// Exact expected target binding supplied by a higher-level retained Store
/// capability. Every production replacement is bound either to exact absence or
/// to the retained identity of the precise predecessor leaf.
pub(crate) enum RetainedExpectedTarget<'a> {
    Absent,
    ClaimedAbsent {
        file: &'a File,
        identity: &'a RetainedFileIdentity,
    },
    Exact {
        file: &'a File,
        identity: &'a RetainedFileIdentity,
    },
}

/// Crash-safe replacement result that keeps the exact installed target handle
/// alive for the higher-level retained Store witness.
pub(crate) struct RetainedCrashReplaceResult {
    pub(crate) result: CrashReplaceResult,
    pub(crate) installed_file: File,
    pub(crate) installed_identity: RetainedFileIdentity,
}

/// Store-owned one-shot authority for an already-reconciled replacement leaf.
///
/// The session owns the exact retained target capability used by reconciliation
/// together with either marker finalization's precise present handle or its
/// Store-minted absence authority. Callers may inspect the reconciled bytes and
/// digest while performing higher-level work, then consume the session exactly
/// once for replacement or an exact read. Neither consuming operation accepts a
/// pathname, lock, parent, or byte limit, so the authority cannot be retargeted
/// or reminted after reconciliation.
#[must_use = "reconciliation authority must be consumed or intentionally dropped"]
pub struct RetainedCrashReplaceSession<'lock> {
    lock: RetainedSessionLock<'lock>,
    target: RetainedCrashReplaceBinding,
    recovery: CrashReplaceRecovery,
    leaf: RetainedReconciledLeaf,
    maximum: u64,
}

/// A reconciliation session that owns the exact effect lock rather than
/// borrowing it. This form can be stored directly in higher-level authority
/// objects without a self-reference.
pub type OwnedRetainedCrashReplaceSession = RetainedCrashReplaceSession<'static>;

/// Exact present authority consumed from a reconciliation session.
///
/// The value owns marker finalization's file handle, exact bytes, retained
/// target binding, and either its caller-owned lock borrow or the Store-owned
/// lock moved into an owned session. It performs no pathname I/O when created.
#[must_use = "exact read authority must be inspected or intentionally dropped"]
pub struct RetainedCrashReplaceRead<'lock> {
    lock: RetainedSessionLock<'lock>,
    target: RetainedCrashReplaceBinding,
    retained: RetainedDigest,
    bytes: Vec<u8>,
    maximum: u64,
}

pub type OwnedRetainedCrashReplaceRead = RetainedCrashReplaceRead<'static>;

enum RetainedSessionLock<'lock> {
    Borrowed(&'lock EffectStoreLock),
    Owned(EffectStoreLock),
}

impl RetainedSessionLock<'_> {
    fn as_ref(&self) -> &EffectStoreLock {
        match self {
            Self::Borrowed(lock) => lock,
            Self::Owned(lock) => lock,
        }
    }
}

struct RetainedCrashReplaceBinding {
    directory: RetainedDirectory,
    directory_relative_path: PathBuf,
    target_name: PathBuf,
    target_relative_path: PathBuf,
    target_path: PathBuf,
    directory_identity: RetainedFileIdentity,
    state_root_identity: RetainedFileIdentity,
}

impl RetainedCrashReplaceBinding {
    fn from_target(target: RetainedCrashReplaceTarget<'_>) -> Self {
        Self {
            directory: target.directory,
            directory_relative_path: target.directory_relative_path,
            target_name: target.target_name,
            target_relative_path: target.target_relative_path,
            target_path: target.target_path,
            directory_identity: target.directory_identity,
            state_root_identity: target.state_root_identity,
        }
    }

    fn bind<'lock>(
        &self,
        lock: &'lock EffectStoreLock,
    ) -> io::Result<RetainedCrashReplaceTarget<'lock>> {
        Ok(RetainedCrashReplaceTarget {
            directory: self.directory.try_clone()?,
            directory_relative_path: self.directory_relative_path.clone(),
            target_name: self.target_name.clone(),
            target_relative_path: self.target_relative_path.clone(),
            target_path: self.target_path.clone(),
            directory_identity: self.directory_identity.clone(),
            state_root_identity: self.state_root_identity.clone(),
            lock,
        })
    }
}

enum RetainedReconciledLeaf {
    Present {
        retained: RetainedDigest,
        bytes: Vec<u8>,
    },
    Absent(RetainedAbsenceClaim),
}

struct RetainedAbsenceClaim {
    directory: RetainedDirectory,
    target_name: PathBuf,
    file: File,
    identity: RetainedFileIdentity,
}

impl Drop for RetainedAbsenceClaim {
    fn drop(&mut self) {
        let Ok(authority) = self.directory.retain_authority() else {
            return;
        };
        let removal =
            authority.remove_file_with_validation(&self.target_name, |directory, source| {
                directory.verify_retained_authority_binding(source, &self.file, &self.identity)?;
                validate_absence_claim_handle(&self.file, &self.identity)
            });
        if removal.is_ok() {
            let _ = self.directory.sync_root();
        }
    }
}

/// Exact reconciled leaf transferred out of a consumed session without a
/// second target-pathname observation.
pub(crate) enum ConsumedRetainedCrashReplaceLeaf {
    Present {
        file: File,
        identity: RetainedFileIdentity,
        bytes: Vec<u8>,
        digest: String,
        maximum: u64,
    },
    Absent(ConsumedRetainedCrashReplaceAbsence),
}

/// Owned target binding transferred from one reconciled absence session.
pub(crate) struct ConsumedRetainedCrashReplaceAbsence {
    target: RetainedCrashReplaceBinding,
    claim: RetainedAbsenceClaim,
}

impl ConsumedRetainedCrashReplaceAbsence {
    pub(crate) fn revalidate_binding(
        &self,
        expected_lock: &EffectStoreLock,
        expected_target: &Path,
    ) -> io::Result<()> {
        if self.target.target_relative_path != expected_target {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "reconciled absence is bound to a different target",
            ));
        }
        let target = self.target.bind(expected_lock)?;
        target.validate_absence_claim(&self.claim)
    }

    pub(crate) fn expected_target(&self) -> RetainedExpectedTarget<'_> {
        RetainedExpectedTarget::ClaimedAbsent {
            file: &self.claim.file,
            identity: &self.claim.identity,
        }
    }
}

impl fmt::Debug for RetainedCrashReplaceSession<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCrashReplaceSession")
            .field("target_relative_path", &self.target.target_relative_path)
            .field("recovery", &self.recovery)
            .field("digest", &self.digest())
            .field("byte_length", &self.raw_bytes().map(<[u8]>::len))
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RetainedCrashReplaceRead<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCrashReplaceRead")
            .field("target_relative_path", &self.target.target_relative_path)
            .field("digest", &self.retained.digest)
            .field("byte_length", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl RetainedCrashReplaceTarget<'_> {
    fn retain_digest(&self, name: &Path, maximum: u64) -> io::Result<Option<RetainedDigest>> {
        self.validate()?;
        let mut file = match self
            .directory
            .open_leaf_read(name, RetainedLeafPolicy::Authority)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let identity = RetainedDirectory::identity_of(&file)?;
        self.directory
            .verify_retained_authority_binding(name, &file, &identity)?;
        let digest = read_retained_digest(&mut file, maximum)?;
        self.directory
            .verify_retained_authority_binding(name, &file, &identity)?;
        self.validate()?;
        let link_count = retained_link_count(&file)?;
        Ok(Some(RetainedDigest {
            file,
            identity,
            digest,
            link_count,
        }))
    }

    fn revalidate_digest(
        &self,
        name: &Path,
        retained: &mut RetainedDigest,
        maximum: u64,
    ) -> io::Result<()> {
        self.validate()?;
        self.directory.verify_retained_authority_binding(
            name,
            &retained.file,
            &retained.identity,
        )?;
        revalidate_retained_digest_handle(retained, maximum)?;
        self.directory.verify_retained_authority_binding(
            name,
            &retained.file,
            &retained.identity,
        )?;
        self.validate()
    }

    fn require_absent(&self, name: &Path) -> io::Result<()> {
        self.validate()?;
        match self
            .directory
            .open_leaf_read(name, RetainedLeafPolicy::Authority)
        {
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.validate(),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "retained crash-replacement protocol leaf unexpectedly exists",
            )),
            Err(error) => Err(error),
        }
    }

    fn claim_absence(&self) -> io::Result<RetainedAbsenceClaim> {
        self.validate()?;
        let staging = absence_claim_path()?;
        let mut file = self.directory.open_leaf_write_new_authority(&staging)?;
        file.write_all(&absence_claim_bytes()?)?;
        file.sync_all()?;
        let identity = RetainedDirectory::identity_of(&file)?;
        validate_absence_claim_handle(&file, &identity)?;
        let mut claim = RetainedAbsenceClaim {
            directory: self.directory.try_clone()?,
            target_name: staging.clone(),
            file,
            identity,
        };
        self.directory.sync_root()?;
        let authority = self.directory.retain_authority()?;
        let _cleanup_debt = authority.rename_file_noreplace_with_validation(
            &staging,
            &self.target_name,
            |directory, source, destination| {
                if destination != self.target_name {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "retained absence claim destination changed",
                    ));
                }
                directory.verify_retained_authority_binding(
                    source,
                    &claim.file,
                    &claim.identity,
                )?;
                validate_absence_claim_handle(&claim.file, &claim.identity)
            },
        )?;
        claim.target_name.clone_from(&self.target_name);
        self.validate_absence_claim(&claim)?;
        Ok(claim)
    }

    fn validate_absence_claim(&self, claim: &RetainedAbsenceClaim) -> io::Result<()> {
        self.validate_absence_claim_parts(&claim.file, &claim.identity)
    }

    fn validate_absence_claim_parts(
        &self,
        file: &File,
        identity: &RetainedFileIdentity,
    ) -> io::Result<()> {
        self.validate()?;
        validate_absence_claim_handle(file, identity)?;
        self.directory
            .verify_retained_authority_binding(&self.target_name, file, identity)?;
        validate_absence_claim_handle(file, identity)?;
        self.validate()
    }

    fn consume_absence_claim_parts(
        &self,
        file: &File,
        identity: &RetainedFileIdentity,
    ) -> io::Result<()> {
        self.validate_absence_claim_parts(file, identity)?;
        let authority = self.directory.retain_authority()?;
        let _cleanup_debt =
            authority.remove_file_with_validation(&self.target_name, |directory, source| {
                directory.verify_retained_authority_binding(source, file, identity)?;
                validate_absence_claim_handle(file, identity)
            })?;
        validate_absence_claim_handle(file, identity)?;
        self.require_absent(&self.target_name)
    }

    fn isolate_authoritative_name(&self, name: &Path) -> io::Result<()> {
        const ISOLATION_ATTEMPTS: usize = 32;
        self.validate()?;
        let authority = self.directory.retain_authority()?;
        for _ in 0..ISOLATION_ATTEMPTS {
            match authority.remove_file_with_validation(name, |_, _| Ok(())) {
                Ok(_cleanup_debt) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "bounded crash-replacement target isolation was continuously repopulated",
        ))
    }
}

fn retained_link_count(file: &File) -> io::Result<u64> {
    let metadata = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        Ok(metadata.nlink())
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        Ok(crate::windows_file_info::file_information(file)?.number_of_links)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Ok(1)
    }
}

fn revalidate_retained_digest_handle(
    retained: &mut RetainedDigest,
    maximum: u64,
) -> io::Result<()> {
    if RetainedDirectory::identity_of(&retained.file)? != retained.identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement handle changed identity",
        ));
    }
    if retained_link_count(&retained.file)? != retained.link_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement handle changed link count",
        ));
    }
    if read_retained_digest(&mut retained.file, maximum)? != retained.digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement handle changed digest",
        ));
    }
    Ok(())
}

fn validate_expected_target_binding(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    expected_previous: Option<&str>,
    expected_target: &RetainedExpectedTarget<'_>,
    maximum: u64,
) -> io::Result<()> {
    match expected_target {
        RetainedExpectedTarget::Absent => {
            if expected_previous.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "absent retained target binding has an expected previous digest",
                ));
            }
            target.require_absent(&names.target)
        }
        RetainedExpectedTarget::ClaimedAbsent { file, identity } => {
            if expected_previous.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "claimed-absent retained target binding has an expected previous digest",
                ));
            }
            target.validate_absence_claim_parts(file, identity)
        }
        RetainedExpectedTarget::Exact { file, identity } => {
            let expected_previous = expected_previous.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "exact retained target binding has no expected previous digest",
                )
            })?;
            target
                .directory
                .verify_retained_authority_binding(&names.target, file, identity)?;
            let mut retained = file.try_clone()?;
            if read_retained_digest(&mut retained, maximum)? != expected_previous {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "exact retained target digest changed before replacement",
                ));
            }
            target
                .directory
                .verify_retained_authority_binding(&names.target, file, identity)
        }
    }
}

fn read_retained_digest(file: &mut File, maximum: u64) -> io::Result<String> {
    let before = file.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained crash-replacement file exceeds byte limit",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained crash-replacement file exceeds byte limit",
        ));
    }
    let after = file.metadata()?;
    if after.len() != before.len() || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement file changed while read",
        ));
    }
    Ok(sha256_content_hash(&bytes))
}

#[derive(Clone)]
struct Names {
    target: PathBuf,
    next: PathBuf,
    previous: PathBuf,
    transaction: PathBuf,
}

#[derive(Clone)]
struct Marker {
    previous: Option<String>,
    next: String,
}

struct RecoveryWitness {
    target: Option<RetainedDigest>,
    transaction: Option<RetainedDigest>,
}

struct RetainedRecoveryResult {
    result: CrashReplaceRecovery,
    target: Option<RetainedDigest>,
}

impl RecoveryWitness {
    fn revalidate_bindings(
        &mut self,
        target: &RetainedCrashReplaceTarget<'_>,
        names: &Names,
        maximum: u64,
    ) -> io::Result<()> {
        match self.target.as_mut() {
            Some(retained_target) => {
                target.revalidate_digest(&names.target, retained_target, maximum)?;
            }
            None => target.require_absent(&names.target)?,
        }
        match self.transaction.as_mut() {
            Some(transaction) => {
                target.revalidate_digest(&names.transaction, transaction, MARKER_MAX_BYTES)?;
            }
            None => target.require_absent(&names.transaction)?,
        }
        Ok(())
    }

    fn revalidate(
        &mut self,
        target: &RetainedCrashReplaceTarget<'_>,
        names: &Names,
        maximum: u64,
    ) -> io::Result<()> {
        self.revalidate_bindings(target, names, maximum)?;
        target.require_absent(&names.next)?;
        target.require_absent(&names.previous)
    }

    fn revalidate_after_target_removal(
        &mut self,
        target: &RetainedCrashReplaceTarget<'_>,
        names: &Names,
        maximum: u64,
    ) -> io::Result<()> {
        let retained_target = self.target.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained recovery target-removal witness is absent",
            )
        })?;
        revalidate_retained_digest_handle(retained_target, maximum)?;
        target.require_absent(&names.target)?;
        match self.transaction.as_mut() {
            Some(transaction) => {
                target.revalidate_digest(&names.transaction, transaction, MARKER_MAX_BYTES)
            }
            None => target.require_absent(&names.transaction),
        }
    }

    fn revalidate_after_marker_quarantine(
        &mut self,
        target: &RetainedCrashReplaceTarget<'_>,
        names: &Names,
        quarantine: &Path,
        maximum: u64,
    ) -> io::Result<()> {
        match self.target.as_mut() {
            Some(retained_target) => {
                target.revalidate_digest(&names.target, retained_target, maximum)?;
            }
            None => target.require_absent(&names.target)?,
        }
        let marker = self.transaction.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained marker-quarantine witness is absent",
            )
        })?;
        target.revalidate_digest(quarantine, marker, MARKER_MAX_BYTES)?;
        target.require_absent(&names.transaction)?;
        target.require_absent(&names.next)?;
        target.require_absent(&names.previous)
    }
}

fn recovery_mismatch_error(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    reason: impl fmt::Display,
) -> CrashReplaceError {
    let isolation = target.isolate_authoritative_name(&names.target);
    CrashReplaceError::Protocol {
        reason: format!(
            "retained recovery state mismatch: {reason}; authoritative target isolation result: {isolation:?}; transaction marker was preserved"
        ),
    }
}

fn recovery_protocol<T>(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    reason: &str,
) -> Result<T, CrashReplaceError> {
    Err(recovery_mismatch_error(target, names, reason))
}

fn retain_recovery_bindings(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    expected_target_digest: Option<&str>,
    expected_marker_digest: Option<&str>,
    maximum: u64,
) -> Result<RecoveryWitness, CrashReplaceError> {
    let retained = (|| -> io::Result<RecoveryWitness> {
        let target_leaf = if let Some(expected) = expected_target_digest {
            let target_leaf = target
                .retain_digest(&names.target, maximum)?
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "retained recovery target disappeared before finalization",
                    )
                })?;
            if target_leaf.digest != expected {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained recovery target digest changed before finalization",
                ));
            }
            Some(target_leaf)
        } else {
            target.require_absent(&names.target)?;
            None
        };
        let transaction = if let Some(expected) = expected_marker_digest {
            let transaction = target
                .retain_digest(&names.transaction, MARKER_MAX_BYTES)?
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "retained recovery transaction marker disappeared before finalization",
                    )
                })?;
            if transaction.digest != expected {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained recovery transaction marker digest changed before finalization",
                ));
            }
            Some(transaction)
        } else {
            target.require_absent(&names.transaction)?;
            None
        };
        let mut witness = RecoveryWitness {
            target: target_leaf,
            transaction,
        };
        witness.revalidate_bindings(target, names, maximum)?;
        Ok(witness)
    })();
    retained.map_err(|error| recovery_mismatch_error(target, names, error))
}

fn retain_recovery_witness(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    expected_target_digest: Option<&str>,
    expected_marker_digest: Option<&str>,
    maximum: u64,
) -> Result<RecoveryWitness, CrashReplaceError> {
    let mut witness = retain_recovery_bindings(
        target,
        names,
        expected_target_digest,
        expected_marker_digest,
        maximum,
    )?;
    witness
        .revalidate(target, names, maximum)
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    Ok(witness)
}

fn finish_recovery_without_marker(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    action: CrashReplaceRecoveryAction,
    target_digest: Option<String>,
    maximum: u64,
) -> Result<RetainedRecoveryResult, CrashReplaceError> {
    let mut witness =
        retain_recovery_witness(target, names, target_digest.as_deref(), None, maximum)?;
    witness
        .revalidate(target, names, maximum)
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    Ok(RetainedRecoveryResult {
        result: CrashReplaceRecovery {
            action,
            target_digest,
        },
        target: witness.target.take(),
    })
}

fn retain_exact_target_for_marker_finalization(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    expected_digest: &str,
    maximum: u64,
) -> Result<RetainedDigest, CrashReplaceError> {
    let retained = (|| -> io::Result<RetainedDigest> {
        let mut installed = target
            .retain_digest(&names.target, maximum)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "retained marker finalization target disappeared",
                )
            })?;
        if installed.digest != expected_digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained marker finalization target has the wrong digest",
            ));
        }
        target.revalidate_digest(&names.target, &mut installed, maximum)?;
        Ok(installed)
    })();
    retained.map_err(|error| recovery_mismatch_error(target, names, error))
}

fn quarantine_marker_after_revalidation(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    mut installed_target: RetainedDigest,
    expected_target_digest: &str,
    expected_marker_digest: &str,
    maximum: u64,
) -> Result<RetainedDigest, CrashReplaceError> {
    const MARKER_QUARANTINE_ATTEMPTS: usize = 32;
    let retained = (|| -> io::Result<RecoveryWitness> {
        if installed_target.digest != expected_target_digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained marker finalization received the wrong installed target digest",
            ));
        }
        target.revalidate_digest(&names.target, &mut installed_target, maximum)?;
        let transaction = target
            .retain_digest(&names.transaction, MARKER_MAX_BYTES)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "retained recovery transaction marker disappeared before finalization",
                )
            })?;
        if transaction.digest != expected_marker_digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained recovery transaction marker changed before finalization",
            ));
        }
        let mut witness = RecoveryWitness {
            target: Some(installed_target),
            transaction: Some(transaction),
        };
        witness.revalidate(target, names, maximum)?;
        Ok(witness)
    })();
    let mut witness = retained.map_err(|error| recovery_mismatch_error(target, names, error))?;
    let marker_digest = expected_marker_digest.to_owned();
    let authority = target
        .directory
        .retain_authority()
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    let nonce =
        marker_quarantine_nonce().map_err(|error| recovery_mismatch_error(target, names, error))?;

    let mut quarantined = None;
    for attempt in 0..MARKER_QUARANTINE_ATTEMPTS {
        let quarantine = marker_quarantine_path(names, nonce, attempt);
        match authority.rename_file_noreplace_with_validation(
            &names.transaction,
            &quarantine,
            |directory, source, _| {
                validate_expected_digest(directory, source, &marker_digest, MARKER_MAX_BYTES)?;
                witness.revalidate(target, names, maximum)
            },
        ) {
            Ok(cleanup_debt) => {
                let Some(transaction) = witness.transaction.as_mut() else {
                    return Err(recovery_mismatch_error(
                        target,
                        names,
                        "retained recovery transaction witness disappeared after quarantine",
                    ));
                };
                transaction.link_count = retained_link_count(&transaction.file)
                    .map_err(|error| recovery_mismatch_error(target, names, error))?;
                quarantined = Some((quarantine, cleanup_debt));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                let marker_recovery = ensure_discoverable_recovery_marker(
                    target,
                    names,
                    &authority,
                    Some(&quarantine),
                    witness.transaction.as_mut(),
                );
                let isolation = target.isolate_authoritative_name(&names.target);
                return Err(CrashReplaceError::Protocol {
                    reason: format!(
                        "retained marker quarantine failed: {error}; exact recovery-marker preservation result: {marker_recovery:?}; authoritative target isolation result: {isolation:?}"
                    ),
                });
            }
        }
    }
    let Some((quarantine, _cleanup_debt)) = quarantined else {
        return Err(recovery_mismatch_error(
            target,
            names,
            "retained marker-quarantine retry exhausted",
        ));
    };

    // Marker-name mutation is not the success point. The exact marker handle and
    // its discoverable Store quarantine remain retained while every target and
    // protocol binding is swept again. This closing sweep linearizes protocol
    // finalization and returns that same exact target witness directly. Callers
    // must not add a later target-pathname observation as a second success point.
    if let Err(error) =
        witness.revalidate_after_marker_quarantine(target, names, &quarantine, maximum)
    {
        let restoration = restore_quarantined_marker(
            target,
            names,
            &authority,
            &quarantine,
            &marker_digest,
            witness.transaction.as_mut(),
        );
        let marker_recovery = ensure_discoverable_recovery_marker(
            target,
            names,
            &authority,
            Some(&quarantine),
            witness.transaction.as_mut(),
        );
        let isolation = target.isolate_authoritative_name(&names.target);
        return Err(CrashReplaceError::Protocol {
            reason: format!(
                "retained recovery state changed after marker quarantine: {error}; exact marker restoration result: {restoration:?}; exact recovery-marker preservation result: {marker_recovery:?}; authoritative target isolation result: {isolation:?}"
            ),
        });
    }
    witness
        .target
        .take()
        .ok_or_else(|| CrashReplaceError::Protocol {
            reason: "retained marker finalization lost the exact installed target witness"
                .to_owned(),
        })
}

fn marker_quarantine_nonce() -> io::Result<u128> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "crash-replacement marker quarantine nonce failed: {error}"
        ))
    })?;
    Ok(u128::from_le_bytes(nonce))
}

fn absence_claim_path() -> io::Result<PathBuf> {
    Ok(PathBuf::from(format!(
        ".forge-crash-absence-claim-{}-{:032x}.placeholder",
        std::process::id(),
        marker_quarantine_nonce()?
    )))
}

fn absence_claim_bytes() -> io::Result<Vec<u8>> {
    let mut bytes = ABSENCE_CLAIM_PREFIX.to_vec();
    bytes.extend(format!("{:032x}\n", marker_quarantine_nonce()?).as_bytes());
    Ok(bytes)
}

fn is_absence_claim_bytes(bytes: &[u8]) -> bool {
    bytes.len() == usize::try_from(ABSENCE_CLAIM_BYTES).unwrap_or(usize::MAX)
        && bytes.starts_with(ABSENCE_CLAIM_PREFIX)
        && bytes.last() == Some(&b'\n')
        && bytes[ABSENCE_CLAIM_PREFIX.len()..bytes.len() - 1]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn validate_absence_claim_handle(file: &File, identity: &RetainedFileIdentity) -> io::Result<()> {
    if RetainedDirectory::identity_of(file)? != *identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement absence claim changed identity",
        ));
    }
    let bytes = crate::read_retained_effect_leaf(&mut file.try_clone()?, ABSENCE_CLAIM_BYTES)?;
    if !is_absence_claim_bytes(&bytes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained crash-replacement absence claim changed bytes",
        ));
    }
    Ok(())
}

fn marker_quarantine_path(names: &Names, nonce: u128, attempt: usize) -> PathBuf {
    let parent = names.transaction.parent().unwrap_or_else(|| Path::new(""));
    parent.join(format!(
        ".forge-crash-recovery-marker-{}-{nonce}-{attempt}.quarantine",
        std::process::id()
    ))
}

fn restore_quarantined_marker(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    authority: &crate::retained_dir::RetainedAuthorityDirectory<'_>,
    quarantine: &Path,
    marker_digest: &str,
    retained_marker: Option<&mut RetainedDigest>,
) -> io::Result<()> {
    let retained_marker = retained_marker.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "retained quarantined marker handle is absent",
        )
    })?;
    const RESTORE_ATTEMPTS: usize = 32;
    for _ in 0..RESTORE_ATTEMPTS {
        target.isolate_authoritative_name(&names.transaction)?;
        match authority.rename_file_noreplace_with_validation(
            quarantine,
            &names.transaction,
            |directory, source, _| {
                validate_expected_digest(directory, source, marker_digest, MARKER_MAX_BYTES)?;
                revalidate_retained_digest_handle(retained_marker, MARKER_MAX_BYTES)
            },
        ) {
            Ok(_cleanup_debt) => {
                retained_marker.link_count = retained_link_count(&retained_marker.file)?;
                target.revalidate_digest(&names.transaction, retained_marker, MARKER_MAX_BYTES)?;
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                target.isolate_authoritative_name(&names.transaction)?;
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "bounded exact marker restoration was continuously repopulated",
    ))
}

fn ensure_discoverable_recovery_marker(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    authority: &crate::retained_dir::RetainedAuthorityDirectory<'_>,
    preferred_quarantine: Option<&Path>,
    retained_marker: Option<&mut RetainedDigest>,
) -> io::Result<PathBuf> {
    let retained_marker = retained_marker.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "retained recovery marker handle is absent",
        )
    })?;
    if target
        .revalidate_digest(&names.transaction, retained_marker, MARKER_MAX_BYTES)
        .is_ok()
    {
        return Ok(names.transaction.clone());
    }
    if let Some(preferred) = preferred_quarantine {
        if target
            .revalidate_digest(preferred, retained_marker, MARKER_MAX_BYTES)
            .is_ok()
        {
            return Ok(preferred.to_path_buf());
        }
    }

    let nonce = marker_quarantine_nonce()?;
    const RECOVERY_MARKER_ATTEMPTS: usize = 32;
    for attempt in 0..RECOVERY_MARKER_ATTEMPTS {
        let recovery = marker_quarantine_path(names, nonce, attempt);
        match authority.publish_retained_handle_noreplace(
            &retained_marker.file,
            &retained_marker.identity,
            &recovery,
        ) {
            Ok(()) => {
                retained_marker.link_count = retained_link_count(&retained_marker.file)?;
                target.revalidate_digest(&recovery, retained_marker, MARKER_MAX_BYTES)?;
                return Ok(recovery);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "bounded Store recovery-marker publication was continuously repopulated",
    ))
}

fn remove_recovery_sidecar_after_revalidation(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    name: &Path,
    expected_digest: &str,
    expected_target_digest: Option<&str>,
    expected_marker_digest: Option<&str>,
    maximum: u64,
) -> Result<(), CrashReplaceError> {
    let mut witness = retain_recovery_bindings(
        target,
        names,
        expected_target_digest,
        expected_marker_digest,
        maximum,
    )?;
    target
        .remove_file_if_digest_with_validation(name, expected_digest, maximum, || {
            witness.revalidate_bindings(target, names, maximum)
        })
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    witness
        .revalidate_bindings(target, names, maximum)
        .map_err(|error| recovery_mismatch_error(target, names, error))
}

fn remove_recovery_target_after_revalidation(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    expected_target_digest: &str,
    expected_marker_digest: Option<&str>,
    maximum: u64,
) -> Result<(), CrashReplaceError> {
    let mut witness = retain_recovery_bindings(
        target,
        names,
        Some(expected_target_digest),
        expected_marker_digest,
        maximum,
    )?;
    target
        .remove_file_if_digest_with_validation(
            &names.target,
            expected_target_digest,
            maximum,
            || witness.revalidate_bindings(target, names, maximum),
        )
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    witness
        .revalidate_after_target_removal(target, names, maximum)
        .map_err(|error| recovery_mismatch_error(target, names, error))
}

fn revalidate_reconciled_present(
    retained: &mut RetainedDigest,
    expected_bytes: &[u8],
    maximum: u64,
) -> io::Result<()> {
    revalidate_retained_digest_handle(retained, maximum)?;
    let actual = crate::read_retained_effect_leaf(&mut retained.file, maximum)?;
    if actual != expected_bytes || sha256_content_hash(&actual) != retained.digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained reconciled target bytes changed",
        ));
    }
    revalidate_retained_digest_handle(retained, maximum)
}

fn reconcile_file_crash_safe_into_parts(
    target: &RetainedCrashReplaceTarget<'_>,
    maximum: u64,
) -> Result<(CrashReplaceRecovery, RetainedReconciledLeaf), CrashReplaceError> {
    validate_common(target, maximum)?;
    let names = names(target)?;
    let RetainedRecoveryResult {
        result: recovery,
        target: recovered_target,
    } = reconcile(target, &names, maximum)?;
    let leaf = (|| -> io::Result<RetainedReconciledLeaf> {
        if let Some(mut retained) = recovered_target {
            if recovery.target_digest.as_deref() != Some(retained.digest.as_str()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reconciled target differs from marker finalization authority",
                ));
            }
            revalidate_retained_digest_handle(&mut retained, maximum)?;
            let bytes = crate::read_retained_effect_leaf(&mut retained.file, maximum)?;
            if sha256_content_hash(&bytes) != retained.digest {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reconciled target bytes differ from marker finalization digest",
                ));
            }
            revalidate_retained_digest_handle(&mut retained, maximum)?;
            Ok(RetainedReconciledLeaf::Present { retained, bytes })
        } else {
            if recovery.target_digest.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reconciled absence differs from marker finalization result",
                ));
            }
            Ok(RetainedReconciledLeaf::Absent(target.claim_absence()?))
        }
    })()
    .map_err(|error| io_error(&target.target_path, error))?;
    Ok((recovery, leaf))
}

pub(crate) fn reconcile_file_crash_safe_at_owned_retained_target(
    target: RetainedCrashReplaceTarget<'_>,
    maximum: u64,
) -> Result<RetainedCrashReplaceSession<'_>, CrashReplaceError> {
    let lock = target.lock;
    let (recovery, leaf) = reconcile_file_crash_safe_into_parts(&target, maximum)?;
    Ok(RetainedCrashReplaceSession {
        lock: RetainedSessionLock::Borrowed(lock),
        target: RetainedCrashReplaceBinding::from_target(target),
        recovery,
        leaf,
        maximum,
    })
}

impl<'lock> RetainedCrashReplaceSession<'lock> {
    /// Recovery action and exact target digest produced when this session was
    /// reconciled.
    #[must_use]
    pub fn recovery(&self) -> &CrashReplaceRecovery {
        &self.recovery
    }

    /// Digest of the exact reconciled target, or `None` for Store-minted absence.
    #[must_use]
    pub fn digest(&self) -> Option<&str> {
        match &self.leaf {
            RetainedReconciledLeaf::Present { retained, .. } => Some(&retained.digest),
            RetainedReconciledLeaf::Absent(_) => None,
        }
    }

    /// Exact bytes read only from marker finalization's retained target handle.
    #[must_use]
    pub fn raw_bytes(&self) -> Option<&[u8]> {
        match &self.leaf {
            RetainedReconciledLeaf::Present { bytes, .. } => Some(bytes),
            RetainedReconciledLeaf::Absent(_) => None,
        }
    }

    /// Revalidate the exact reconciled present handle or Store-minted absence
    /// claim without consuming the session or reopening the target pathname.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the retained lock, root, parent, leaf, identity,
    /// bytes, digest, link shape, or absence claim changed.
    pub fn revalidate(&mut self) -> io::Result<()> {
        let target = self.target.bind(self.lock.as_ref())?;
        target.validate()?;
        match &mut self.leaf {
            RetainedReconciledLeaf::Present { retained, bytes } => {
                target.directory.verify_retained_authority_binding(
                    &target.target_name,
                    &retained.file,
                    &retained.identity,
                )?;
                revalidate_reconciled_present(retained, bytes, self.maximum)?;
                target.directory.verify_retained_authority_binding(
                    &target.target_name,
                    &retained.file,
                    &retained.identity,
                )?;
            }
            RetainedReconciledLeaf::Absent(claim) => target.validate_absence_claim(claim)?,
        }
        target.validate()
    }

    /// Derive retained Store I/O from the exact effect lock carried by this
    /// session. Owned sessions keep the lock alive without exposing ownership to
    /// higher-level callers.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] if the retained lock or namespace binding
    /// changed.
    pub fn retained_store_io(
        &self,
    ) -> Result<crate::RetainedEffectStoreIo<'_>, EffectStoreLockError> {
        self.lock.as_ref().retained_store_io()
    }

    /// Consume this session into its exact reconciled leaf for a higher-level
    /// Store capability bound to the same lock and target.
    ///
    /// The absent variant is transferred directly from reconciliation. This
    /// conversion does not reopen the target pathname or mint a later absence.
    pub(crate) fn consume_reconciled_leaf(
        mut self,
        expected_lock: &EffectStoreLock,
        expected_target: &Path,
    ) -> Result<ConsumedRetainedCrashReplaceLeaf, CrashReplaceError> {
        if !std::ptr::eq(self.lock.as_ref(), expected_lock)
            || self.target.target_relative_path != expected_target
        {
            return Err(CrashReplaceError::InvalidArgument {
                field: "reconciliation_session",
                reason: "retained session is bound to a different lock, root, parent, or leaf"
                    .to_owned(),
            });
        }
        let target = self
            .target
            .bind(expected_lock)
            .map_err(|error| io_error(&self.target.target_path, error))?;
        target
            .validate()
            .map_err(|error| io_error(&self.target.target_path, error))?;
        match &mut self.leaf {
            RetainedReconciledLeaf::Present { retained, bytes } => {
                revalidate_reconciled_present(retained, bytes, self.maximum)
                    .map_err(|error| io_error(&self.target.target_path, error))?;
            }
            RetainedReconciledLeaf::Absent(claim) => {
                target
                    .validate_absence_claim(claim)
                    .map_err(|error| io_error(&self.target.target_path, error))?;
            }
        }
        drop(target);
        match self.leaf {
            RetainedReconciledLeaf::Present { retained, bytes } => {
                Ok(ConsumedRetainedCrashReplaceLeaf::Present {
                    file: retained.file,
                    identity: retained.identity,
                    bytes,
                    digest: retained.digest,
                    maximum: self.maximum,
                })
            }
            RetainedReconciledLeaf::Absent(claim) => Ok(ConsumedRetainedCrashReplaceLeaf::Absent(
                ConsumedRetainedCrashReplaceAbsence {
                    target: self.target,
                    claim,
                },
            )),
        }
    }

    /// Consume this session as one exact read.
    ///
    /// A present result retains marker finalization's exact handle. Absence
    /// consumes the Store-minted absence authority and returns `None`. This
    /// operation performs no target-pathname I/O after reconciliation; it only
    /// re-reads and verifies the retained present handle.
    ///
    /// # Errors
    ///
    /// Returns [`CrashReplaceError`] if a retained present handle changed bytes,
    /// digest, or identity after reconciliation.
    pub fn read_exact(
        mut self,
    ) -> Result<Option<RetainedCrashReplaceRead<'lock>>, CrashReplaceError> {
        match &mut self.leaf {
            RetainedReconciledLeaf::Present { retained, bytes } => {
                revalidate_reconciled_present(retained, bytes, self.maximum)
                    .map_err(|error| io_error(&self.target.target_path, error))?;
            }
            RetainedReconciledLeaf::Absent(claim) => {
                let target = self
                    .target
                    .bind(self.lock.as_ref())
                    .map_err(|error| io_error(&self.target.target_path, error))?;
                target
                    .validate_absence_claim(claim)
                    .map_err(|error| io_error(&self.target.target_path, error))?;
                return Ok(None);
            }
        }
        match self.leaf {
            RetainedReconciledLeaf::Present { retained, bytes } => {
                Ok(Some(RetainedCrashReplaceRead {
                    lock: self.lock,
                    target: self.target,
                    retained,
                    bytes,
                    maximum: self.maximum,
                }))
            }
            RetainedReconciledLeaf::Absent(_) => {
                unreachable!("absence returned before exact-read assembly")
            }
        }
    }

    /// Consume this session for one crash-safe replacement.
    ///
    /// The replacement reuses the exact target capability and present handle or
    /// absence authority retained by reconciliation. The caller supplies only new
    /// bytes; it cannot change the lock, root, parent, leaf, or byte limit. Success
    /// returns marker finalization's exact installed handle without reopening the
    /// target pathname.
    ///
    /// # Errors
    ///
    /// Returns [`CrashReplaceError`] if the retained authority is no longer the
    /// current exact target or durable replacement cannot complete.
    pub fn replace(
        mut self,
        content: &[u8],
    ) -> Result<RetainedCrashReplaceRead<'lock>, CrashReplaceError> {
        if let RetainedReconciledLeaf::Present { retained, bytes } = &mut self.leaf {
            revalidate_reconciled_present(retained, bytes, self.maximum)
                .map_err(|error| io_error(&self.target.target_path, error))?;
        }
        let target = self
            .target
            .bind(self.lock.as_ref())
            .map_err(|error| io_error(&self.target.target_path, error))?;
        let retained = {
            let (expected_previous, expected_target) = match &self.leaf {
                RetainedReconciledLeaf::Present { retained, .. } => (
                    Some(retained.digest.as_str()),
                    RetainedExpectedTarget::Exact {
                        file: &retained.file,
                        identity: &retained.identity,
                    },
                ),
                RetainedReconciledLeaf::Absent(claim) => (
                    None,
                    RetainedExpectedTarget::ClaimedAbsent {
                        file: &claim.file,
                        identity: &claim.identity,
                    },
                ),
            };
            replace_file_crash_safe_at_retained_target_inner(
                &target,
                expected_previous,
                expected_target,
                content,
                self.maximum,
                None,
            )?
        };
        drop(target);
        if retained.result.installed_digest != sha256_content_hash(content) {
            return Err(CrashReplaceError::Protocol {
                reason: "retained reconciliation session returned a mismatched installed digest"
                    .to_owned(),
            });
        }
        let RetainedCrashReplaceResult {
            result,
            installed_file,
            installed_identity,
        } = retained;
        let link_count = retained_link_count(&installed_file)
            .map_err(|error| io_error(&self.target.target_path, error))?;
        Ok(RetainedCrashReplaceRead {
            lock: self.lock,
            target: self.target,
            retained: RetainedDigest {
                file: installed_file,
                identity: installed_identity,
                digest: result.installed_digest,
                link_count,
            },
            bytes: content.to_vec(),
            maximum: self.maximum,
        })
    }
}

impl RetainedCrashReplaceRead<'_> {
    /// Exact bytes retained from marker finalization's target handle.
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// SHA-256 digest of [`Self::raw_bytes`].
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.retained.digest
    }

    /// Revalidate the exact lock, root, parent, leaf, identity, bytes, and digest.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the selector no longer names this exact retained
    /// handle or any retained authority binding changed.
    pub fn revalidate(&mut self) -> io::Result<()> {
        let target = self.target.bind(self.lock.as_ref())?;
        target.validate()?;
        target.directory.verify_retained_authority_binding(
            &target.target_name,
            &self.retained.file,
            &self.retained.identity,
        )?;
        revalidate_reconciled_present(&mut self.retained, &self.bytes, self.maximum)?;
        target.directory.verify_retained_authority_binding(
            &target.target_name,
            &self.retained.file,
            &self.retained.identity,
        )?;
        target.validate()
    }

    /// Derive retained Store I/O from this exact read's effect lock.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] if the retained lock or namespace binding
    /// changed.
    pub fn retained_store_io(
        &self,
    ) -> Result<crate::RetainedEffectStoreIo<'_>, EffectStoreLockError> {
        self.lock.as_ref().retained_store_io()
    }
}

/// Reconcile one descriptor-relative target into Store-owned one-shot authority.
///
/// The returned session owns the exact retained target capability plus marker
/// finalization's exact present handle or exact absence authority. Higher-level
/// work may inspect it, then consume it once for replacement or an exact read.
pub fn reconcile_file_crash_safe_under_retained_lock<'lock>(
    lock: &'lock EffectStoreLock,
    target_relative: &Path,
    maximum: u64,
) -> Result<RetainedCrashReplaceSession<'lock>, CrashReplaceError> {
    let target = bind_target(lock, target_relative)?;
    reconcile_file_crash_safe_at_owned_retained_target(target, maximum)
}

/// Move one exact effect lock into a Store-owned reconciliation session.
///
/// Unlike the borrowed constructor, this form has no caller-owned lifetime and
/// can be stored directly in higher-level lifecycle or CLI authority objects.
/// The retained target binding is detached from the temporary lock borrow before
/// the lock is moved, so the result contains no self-reference.
pub fn reconcile_file_crash_safe_under_owned_lock(
    lock: EffectStoreLock,
    target_relative: &Path,
    maximum: u64,
) -> Result<OwnedRetainedCrashReplaceSession, CrashReplaceError> {
    let target = bind_target(&lock, target_relative)?;
    let (recovery, leaf) = reconcile_file_crash_safe_into_parts(&target, maximum)?;
    let target = RetainedCrashReplaceBinding::from_target(target);
    Ok(RetainedCrashReplaceSession {
        lock: RetainedSessionLock::Owned(lock),
        target,
        recovery,
        leaf,
        maximum,
    })
}

/// Recover one descriptor-relative target while retaining the exact Store lock.
/// The leaf-bound authority is minted and consumed entirely inside Store.
pub fn recover_file_crash_safe_under_retained_lock(
    lock: &EffectStoreLock,
    target_relative: &Path,
    maximum: u64,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    let target = bind_target(lock, target_relative)?;
    recover_file_crash_safe_at_retained_target(&target, maximum)
}

pub(crate) fn recover_file_crash_safe_at_retained_target(
    target: &RetainedCrashReplaceTarget<'_>,
    maximum: u64,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    validate_common(target, maximum)?;
    let names = names(target)?;
    reconcile(target, &names, maximum).map(|recovered| recovered.result)
}

/// Reconcile and retain exact expected-state authority for one replacement leaf.
///
/// The result is either the exact present [`crate::RetainedEffectStoreLeafWitness`]
/// or a non-constructible exact absence witness. Both are bound to this precise
/// `EffectStoreLock`, retained root, parent, and leaf.
pub fn retain_file_crash_safe_expected_leaf_under_retained_lock<'lock>(
    lock: &'lock EffectStoreLock,
    target_relative: &Path,
    maximum: u64,
) -> Result<crate::RetainedEffectStoreExpectedLeaf<'lock>, CrashReplaceError> {
    let target = bind_target(lock, target_relative)?;
    retain_file_crash_safe_expected_leaf_at_retained_target(&target, maximum)
}

pub(crate) fn retain_file_crash_safe_expected_leaf_at_retained_target<'lock>(
    target: &RetainedCrashReplaceTarget<'lock>,
    maximum: u64,
) -> Result<crate::RetainedEffectStoreExpectedLeaf<'lock>, CrashReplaceError> {
    validate_common(target, maximum)?;
    let names = names(target)?;
    let recovered = reconcile(target, &names, maximum)?;
    (|| -> io::Result<crate::RetainedEffectStoreExpectedLeaf<'lock>> {
        if let Some(mut retained) = recovered.target {
            if recovered.result.target_digest.as_deref() != Some(retained.digest.as_str()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained expected target differs from completed recovery",
                ));
            }
            // Recovery or marker finalization already performed the closing
            // namespace sweep. Preserve that same target witness and inspect
            // only its retained handle while materializing exact public bytes.
            revalidate_retained_digest_handle(&mut retained, maximum)?;
            let bytes = crate::read_retained_effect_leaf(&mut retained.file, maximum)?;
            if sha256_content_hash(&bytes) != retained.digest {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained expected target bytes differ from its exact digest",
                ));
            }
            revalidate_retained_digest_handle(&mut retained, maximum)?;
            Ok(crate::RetainedEffectStoreExpectedLeaf::Present(
                crate::RetainedEffectStoreLeafWitness {
                    lock: target.lock,
                    parent: target.directory.try_clone()?,
                    parent_relative_path: target.directory_relative_path.clone(),
                    state_root_identity: target.state_root_identity.clone(),
                    parent_identity: target.directory_identity.clone(),
                    relative_path: names.target,
                    file: retained.file,
                    leaf_identity: retained.identity,
                    bytes,
                    digest: retained.digest,
                    maximum,
                },
            ))
        } else {
            if recovered.result.target_digest.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "completed recovery returned inconsistent absence authority",
                ));
            }
            Ok(crate::RetainedEffectStoreExpectedLeaf::Absent(
                crate::RetainedEffectStoreLeafAbsenceWitness {
                    lock: target.lock,
                    parent: target.directory.try_clone()?,
                    parent_relative_path: target.directory_relative_path.clone(),
                    state_root_identity: target.state_root_identity.clone(),
                    parent_identity: target.directory_identity.clone(),
                    relative_path: names.target,
                },
            ))
        }
    })()
    .map_err(|error| io_error(&target.target_path, error))
}

/// Replace one descriptor-relative target from exact present or absence authority.
///
/// Digest-only compare-and-swap is intentionally unavailable. `expected` must
/// have been minted by Store for the same exact lock/root/parent/leaf. Success
/// returns the precise installed handle retained through marker finalization;
/// Store performs no later target-pathname read.
pub fn replace_file_crash_safe_under_retained_lock<'lock>(
    lock: &'lock EffectStoreLock,
    target_relative: &Path,
    expected: &mut crate::RetainedEffectStoreExpectedLeaf<'lock>,
    content: &[u8],
    maximum: u64,
) -> Result<crate::RetainedEffectStoreLeafWitness<'lock>, CrashReplaceError> {
    replace_file_crash_safe_under_retained_lock_inner(
        lock,
        target_relative,
        expected,
        content,
        maximum,
        None,
    )
    .map(|(installed, _)| installed)
}

pub(crate) fn replace_file_crash_safe_at_retained_target_with_witness(
    target: &RetainedCrashReplaceTarget<'_>,
    expected_previous: Option<&str>,
    expected_target: RetainedExpectedTarget<'_>,
    content: &[u8],
    maximum: u64,
) -> Result<RetainedCrashReplaceResult, CrashReplaceError> {
    replace_file_crash_safe_at_retained_target_inner(
        target,
        expected_previous,
        expected_target,
        content,
        maximum,
        None,
    )
}

fn replace_file_crash_safe_under_retained_lock_inner<'lock>(
    lock: &'lock EffectStoreLock,
    target_relative: &Path,
    expected: &mut crate::RetainedEffectStoreExpectedLeaf<'lock>,
    content: &[u8],
    maximum: u64,
    fault_after: Option<CrashReplacePhase>,
) -> Result<
    (
        crate::RetainedEffectStoreLeafWitness<'lock>,
        CrashReplaceResult,
    ),
    CrashReplaceError,
> {
    let target = bind_target(lock, target_relative)?;
    let names = names(&target)?;
    let installed_parent = target
        .directory
        .try_clone()
        .map_err(|error| io_error(&target.target_path, error))?;
    let bound = match expected {
        crate::RetainedEffectStoreExpectedLeaf::Present(witness) => {
            if !std::ptr::eq(witness.lock, target.lock)
                || witness.parent_relative_path != target.directory_relative_path
                || witness.state_root_identity != target.state_root_identity
                || witness.parent_identity != target.directory_identity
                || witness.relative_path != names.target
            {
                return Err(CrashReplaceError::InvalidArgument {
                    field: "expected_previous",
                    reason: "retained witness is bound to a different lock, root, parent, or leaf"
                        .to_owned(),
                });
            }
            witness
                .revalidate()
                .map_err(|error| io_error(&target.target_path, error))?;
            (
                Some(witness.digest.clone()),
                RetainedExpectedTarget::Exact {
                    file: &witness.file,
                    identity: &witness.leaf_identity,
                },
            )
        }
        crate::RetainedEffectStoreExpectedLeaf::Absent(witness) => {
            if !std::ptr::eq(witness.lock, target.lock)
                || witness.parent_relative_path != target.directory_relative_path
                || witness.state_root_identity != target.state_root_identity
                || witness.parent_identity != target.directory_identity
                || witness.relative_path != names.target
            {
                return Err(CrashReplaceError::InvalidArgument {
                    field: "expected_previous",
                    reason: "retained absence authority is bound to a different lock, root, parent, or leaf"
                        .to_owned(),
                });
            }
            witness
                .revalidate()
                .map_err(|error| io_error(&target.target_path, error))?;
            (None, RetainedExpectedTarget::Absent)
        }
    };
    let (expected_digest, expected_target) = bound;
    let retained = replace_file_crash_safe_at_retained_target_inner(
        &target,
        expected_digest.as_deref(),
        expected_target,
        content,
        maximum,
        fault_after,
    )?;
    if retained.result.installed_digest != sha256_content_hash(content) {
        return Err(CrashReplaceError::Protocol {
            reason: "retained crash replacement returned a mismatched installed digest".to_owned(),
        });
    }
    let result = retained.result;
    let installed = crate::RetainedEffectStoreLeafWitness {
        lock: target.lock,
        parent: installed_parent,
        parent_relative_path: target.directory_relative_path.clone(),
        state_root_identity: target.state_root_identity.clone(),
        parent_identity: target.directory_identity.clone(),
        relative_path: names.target,
        file: retained.installed_file,
        leaf_identity: retained.installed_identity,
        bytes: content.to_vec(),
        digest: result.installed_digest.clone(),
        maximum,
    };
    Ok((installed, result))
}

/// Crate-private deterministic process-loss seam for Store unit tests.
#[cfg(test)]
pub(crate) fn replace_file_crash_safe_under_retained_lock_with_fault(
    lock: &EffectStoreLock,
    target_relative: &Path,
    expected_previous: Option<&str>,
    content: &[u8],
    maximum: u64,
    fault_after: Option<CrashReplacePhase>,
) -> Result<CrashReplaceResult, CrashReplaceError> {
    let mut expected =
        retain_file_crash_safe_expected_leaf_under_retained_lock(lock, target_relative, maximum)?;
    if expected.digest() != expected_previous {
        return Err(CrashReplaceError::CompareAndSwapMismatch {
            expected: expected_previous.map(str::to_owned),
            actual: expected.digest().map(str::to_owned),
        });
    }
    replace_file_crash_safe_under_retained_lock_inner(
        lock,
        target_relative,
        &mut expected,
        content,
        maximum,
        fault_after,
    )
    .map(|(_, result)| result)
}

fn bind_target<'lock>(
    lock: &'lock EffectStoreLock,
    target_relative: &Path,
) -> Result<RetainedCrashReplaceTarget<'lock>, CrashReplaceError> {
    lock.retained_crash_replace_target(target_relative)
        .map_err(|error| match error {
            EffectStoreLockError::InvalidRelativePath { path } => CrashReplaceError::InvalidPath {
                field: "target",
                path,
            },
            EffectStoreLockError::ReservedStatePath { path, reserved } => {
                CrashReplaceError::ReservedStatePath {
                    field: "target",
                    path,
                    reserved,
                }
            }
            error => io_error(
                &lock.retained_state_root_path().join(target_relative),
                io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()),
            ),
        })
}

fn replace_file_crash_safe_at_retained_target_inner(
    target: &RetainedCrashReplaceTarget<'_>,
    expected_previous: Option<&str>,
    expected_target: RetainedExpectedTarget<'_>,
    content: &[u8],
    maximum: u64,
    fault_after: Option<CrashReplacePhase>,
) -> Result<RetainedCrashReplaceResult, CrashReplaceError> {
    validate_common(target, maximum)?;
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > maximum {
        return Err(CrashReplaceError::SizeLimit {
            path: target.target_path.clone(),
            found: u64::try_from(content.len()).unwrap_or(u64::MAX),
            maximum,
        });
    }
    validate_optional_digest(expected_previous)?;
    let names = names(target)?;
    // Validate caller authority before recovery can finalize any old marker.
    validate_expected_target_binding(target, &names, expected_previous, &expected_target, maximum)
        .map_err(|error| io_error(&target.target_path, error))?;
    if !matches!(
        &expected_target,
        RetainedExpectedTarget::ClaimedAbsent { .. }
    ) {
        let mut recovered = reconcile(target, &names, maximum)?;
        let previous = recovered.result.target_digest.clone();
        if previous.as_deref() != expected_previous {
            return Err(CrashReplaceError::CompareAndSwapMismatch {
                expected: expected_previous.map(str::to_owned),
                actual: previous,
            });
        }
        let recovered_binding = match (&expected_target, recovered.target.as_mut()) {
            (RetainedExpectedTarget::Absent, None) => Ok(()),
            (RetainedExpectedTarget::ClaimedAbsent { .. }, _) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "claimed absence unexpectedly entered pathname reconciliation",
            )),
            (RetainedExpectedTarget::Exact { identity, .. }, Some(retained))
                if &retained.identity == *identity
                    && expected_previous == Some(retained.digest.as_str()) =>
            {
                // Recovery returned the exact target handle from its closing
                // sweep. Validate only that retained handle here; do not reopen
                // the finalized target pathname before beginning this transaction.
                revalidate_retained_digest_handle(retained, maximum)
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "completed recovery does not match exact replacement authority",
            )),
        };
        recovered_binding.map_err(|error| io_error(&target.target_path, error))?;
    }
    for (name, label, limit) in [
        (&names.next, "next", maximum),
        (&names.previous, "previous", maximum),
        (&names.transaction, "transaction", MARKER_MAX_BYTES),
    ] {
        if read(target, name, limit)?.is_some() {
            return protocol(&format!("{label} sidecar remains after reconciliation"));
        }
    }
    let marker = Marker {
        previous: expected_previous.map(str::to_owned),
        next: sha256_content_hash(content),
    };
    let marker_bytes = encode_marker(&marker);
    let marker_digest = sha256_content_hash(&marker_bytes);
    write(target, &names.next, content)?;
    target
        .sync()
        .map_err(|error| io_error(&target.target_path, error))?;
    inject_fault(fault_after, CrashReplacePhase::NextSynced)?;
    write(target, &names.transaction, &marker_bytes)?;
    target
        .sync()
        .map_err(|error| io_error(&target.target_path, error))?;
    inject_fault(fault_after, CrashReplacePhase::TransactionSynced)?;

    if let RetainedExpectedTarget::ClaimedAbsent { file, identity } = &expected_target {
        target
            .consume_absence_claim_parts(file, identity)
            .and_then(|()| target.sync())
            .map_err(|error| io_error(&target.target_path, error))?;
    }

    if let Some(previous_digest) = marker.previous.as_deref() {
        ensure_digest(
            "target before previous install",
            digest(target, &names.target, maximum)?.as_deref(),
            previous_digest,
        )?;
        let mut retained_previous = rename(
            target,
            &names.target,
            &names.previous,
            previous_digest,
            match &expected_target {
                RetainedExpectedTarget::Exact { identity, .. } => Some(*identity),
                RetainedExpectedTarget::Absent | RetainedExpectedTarget::ClaimedAbsent { .. } => {
                    None
                }
            },
            maximum,
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
        target
            .revalidate_digest(&names.previous, &mut retained_previous, maximum)
            .map_err(|error| io_error(&target.target_path, error))?;
        ensure_digest(
            "installed previous",
            digest(target, &names.previous, maximum)?.as_deref(),
            previous_digest,
        )?;
        inject_fault(fault_after, CrashReplacePhase::PreviousInstalled)?;
    }
    let installed = rename(
        target,
        &names.next,
        &names.target,
        &marker.next,
        None,
        maximum,
    )?;
    target
        .sync()
        .map_err(|error| io_error(&target.target_path, error))?;
    ensure_digest(
        "installed target",
        digest(target, &names.target, maximum)?.as_deref(),
        &marker.next,
    )?;
    inject_fault(fault_after, CrashReplacePhase::TargetInstalled)?;
    if let Some(previous_digest) = marker.previous.as_deref() {
        remove_recovery_sidecar_after_revalidation(
            target,
            &names,
            &names.previous,
            previous_digest,
            Some(&marker.next),
            Some(&marker_digest),
            maximum,
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
    }
    let installed = quarantine_marker_after_revalidation(
        target,
        &names,
        installed,
        &marker.next,
        &marker_digest,
        maximum,
    )?;
    Ok(RetainedCrashReplaceResult {
        result: CrashReplaceResult {
            previous_digest: marker.previous,
            installed_digest: marker.next,
        },
        installed_file: installed.file,
        installed_identity: installed.identity,
    })
}

fn reconcile(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    maximum: u64,
) -> Result<RetainedRecoveryResult, CrashReplaceError> {
    let marker_bytes = read(target, &names.transaction, MARKER_MAX_BYTES)?;
    let marker_digest = marker_bytes.as_deref().map(sha256_content_hash);
    let marker = marker_bytes
        .as_deref()
        .map(parse_marker)
        .transpose()
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    let target_bytes = read(target, &names.target, maximum.max(ABSENCE_CLAIM_BYTES))?;
    let target_is_absence_claim = target_bytes.as_deref().is_some_and(is_absence_claim_bytes);
    if !target_is_absence_claim
        && target_bytes
            .as_ref()
            .is_some_and(|bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum)
    {
        return Err(CrashReplaceError::SizeLimit {
            path: target.target_path.clone(),
            found: target_bytes
                .as_ref()
                .map_or(0, |bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            maximum,
        });
    }
    let mut target_digest = target_bytes.as_deref().map(sha256_content_hash);
    let mut next_digest = digest(target, &names.next, maximum)?;
    let previous_digest = digest(target, &names.previous, maximum)?;
    let empty_digest = sha256_content_hash(&[]);
    let Some(marker) = marker else {
        if previous_digest.is_some() {
            return protocol("previous file exists without a transaction marker");
        }
        if let Some(next_digest) = next_digest {
            if target_is_absence_claim {
                let claim_digest = target_digest.as_deref().ok_or_else(|| {
                    protocol_error("retained absence claim has no content digest")
                })?;
                remove_recovery_target_after_revalidation(
                    target,
                    names,
                    claim_digest,
                    None,
                    maximum.max(ABSENCE_CLAIM_BYTES),
                )?;
                target
                    .sync()
                    .map_err(|error| io_error(&target.target_path, error))?;
                target_digest = None;
            } else if target_digest.is_none() {
                return protocol("next file exists without a marker or durable target");
            }
            remove_recovery_sidecar_after_revalidation(
                target,
                names,
                &names.next,
                &next_digest,
                target_digest.as_deref(),
                None,
                maximum,
            )?;
            target
                .sync()
                .map_err(|error| io_error(&target.target_path, error))?;
            return finish_recovery_without_marker(
                target,
                names,
                CrashReplaceRecoveryAction::RemovedUncommittedNext,
                target_digest,
                maximum,
            );
        }
        if target_is_absence_claim {
            let claim_digest = target_digest
                .as_deref()
                .ok_or_else(|| protocol_error("retained absence claim has no content digest"))?;
            remove_recovery_target_after_revalidation(
                target,
                names,
                claim_digest,
                None,
                maximum.max(ABSENCE_CLAIM_BYTES),
            )?;
            target
                .sync()
                .map_err(|error| io_error(&target.target_path, error))?;
            target_digest = None;
        }
        return finish_recovery_without_marker(
            target,
            names,
            CrashReplaceRecoveryAction::Noop,
            target_digest,
            maximum,
        );
    };
    let marker_digest = marker_digest.expect("marker bytes exist when marker parsed");
    if target_digest.as_deref() == Some(marker.next.as_str())
        && next_digest.as_deref() == Some(empty_digest.as_str())
    {
        remove_recovery_sidecar_after_revalidation(
            target,
            names,
            &names.next,
            &empty_digest,
            Some(&marker.next),
            Some(&marker_digest),
            maximum,
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
        next_digest = None;
    }
    if target_is_absence_claim
        && marker.previous.is_none()
        && next_digest.as_deref() == Some(marker.next.as_str())
    {
        let claim_digest = target_digest
            .as_deref()
            .ok_or_else(|| protocol_error("retained absence claim has no content digest"))?;
        remove_recovery_target_after_revalidation(
            target,
            names,
            claim_digest,
            Some(&marker_digest),
            maximum.max(ABSENCE_CLAIM_BYTES),
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
        target_digest = None;
    }
    if target_digest.as_deref() == Some(empty_digest.as_str())
        && marker
            .previous
            .as_deref()
            .is_some_and(|expected| previous_digest.as_deref() == Some(expected))
    {
        remove_recovery_target_after_revalidation(
            target,
            names,
            &empty_digest,
            Some(&marker_digest),
            maximum,
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
        target_digest = None;
    }
    ensure_optional_digest("next", next_digest.as_deref(), &marker.next)
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    if let Some(expected) = marker.previous.as_deref() {
        ensure_optional_digest("previous", previous_digest.as_deref(), expected)
            .map_err(|error| recovery_mismatch_error(target, names, error))?;
    } else if previous_digest.is_some() {
        return recovery_protocol(
            target,
            names,
            "unexpected previous file for an initially empty transaction",
        );
    }
    match target_digest.as_deref() {
        Some(found) if found == marker.next => {
            if next_digest.is_some() {
                return recovery_protocol(
                    target,
                    names,
                    "committed target coexists with a next file",
                );
            }
            if let Some(previous_digest) = previous_digest {
                remove_recovery_sidecar_after_revalidation(
                    target,
                    names,
                    &names.previous,
                    &previous_digest,
                    Some(&marker.next),
                    Some(&marker_digest),
                    maximum,
                )?;
                target
                    .sync()
                    .map_err(|error| io_error(&target.target_path, error))?;
            }
            let installed =
                retain_exact_target_for_marker_finalization(target, names, &marker.next, maximum)?;
            let installed = quarantine_marker_after_revalidation(
                target,
                names,
                installed,
                &marker.next,
                &marker_digest,
                maximum,
            )?;
            Ok(RetainedRecoveryResult {
                result: CrashReplaceRecovery {
                    action: CrashReplaceRecoveryAction::CleanedCommitted,
                    target_digest: Some(marker.next),
                },
                target: Some(installed),
            })
        }
        Some(found) if marker.previous.as_deref() == Some(found) => {
            if previous_digest.is_some() {
                return recovery_protocol(
                    target,
                    names,
                    "old target coexists with a previous file",
                );
            }
            if let Some(next_digest) = next_digest {
                remove_recovery_sidecar_after_revalidation(
                    target,
                    names,
                    &names.next,
                    &next_digest,
                    Some(found),
                    Some(&marker_digest),
                    maximum,
                )?;
                target
                    .sync()
                    .map_err(|error| io_error(&target.target_path, error))?;
            }
            let installed =
                retain_exact_target_for_marker_finalization(target, names, found, maximum)?;
            let installed = quarantine_marker_after_revalidation(
                target,
                names,
                installed,
                found,
                &marker_digest,
                maximum,
            )?;
            Ok(RetainedRecoveryResult {
                result: CrashReplaceRecovery {
                    action: CrashReplaceRecoveryAction::AbortedToPrevious,
                    target_digest: Some(found.to_owned()),
                },
                target: Some(installed),
            })
        }
        Some(_) => recovery_protocol(
            target,
            names,
            "target digest is not bound by the transaction marker",
        ),
        None => recover_missing(
            target,
            names,
            marker,
            marker_digest,
            next_digest,
            previous_digest,
            maximum,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn recover_missing(
    target: &RetainedCrashReplaceTarget<'_>,
    names: &Names,
    marker: Marker,
    marker_digest: String,
    next_digest: Option<String>,
    previous_digest: Option<String>,
    maximum: u64,
) -> Result<RetainedRecoveryResult, CrashReplaceError> {
    if let Some(expected_previous) = marker.previous {
        let Some(observed_previous) = previous_digest else {
            return recovery_protocol(
                target,
                names,
                "target and marker-bound previous file are both missing",
            );
        };
        ensure_digest("previous", Some(&observed_previous), &expected_previous)
            .map_err(|error| recovery_mismatch_error(target, names, error))?;
        let restored = rename(
            target,
            &names.previous,
            &names.target,
            &observed_previous,
            None,
            maximum,
        )?;
        target
            .sync()
            .map_err(|error| io_error(&target.target_path, error))?;
        if let Some(next_digest) = next_digest {
            remove_recovery_sidecar_after_revalidation(
                target,
                names,
                &names.next,
                &next_digest,
                Some(&expected_previous),
                Some(&marker_digest),
                maximum,
            )?;
            target
                .sync()
                .map_err(|error| io_error(&target.target_path, error))?;
        }
        let restored = quarantine_marker_after_revalidation(
            target,
            names,
            restored,
            &expected_previous,
            &marker_digest,
            maximum,
        )?;
        return Ok(RetainedRecoveryResult {
            result: CrashReplaceRecovery {
                action: CrashReplaceRecoveryAction::RestoredPrevious,
                target_digest: Some(expected_previous),
            },
            target: Some(restored),
        });
    }
    if previous_digest.is_some() {
        return recovery_protocol(
            target,
            names,
            "initial replacement transaction has an unexpected previous file",
        );
    }
    let Some(next_digest) = next_digest else {
        return recovery_protocol(
            target,
            names,
            "initial replacement transaction is incomplete or inconsistent",
        );
    };
    ensure_digest("next", Some(&next_digest), &marker.next)
        .map_err(|error| recovery_mismatch_error(target, names, error))?;
    let installed = rename(
        target,
        &names.next,
        &names.target,
        &next_digest,
        None,
        maximum,
    )?;
    target
        .sync()
        .map_err(|error| io_error(&target.target_path, error))?;
    let installed = quarantine_marker_after_revalidation(
        target,
        names,
        installed,
        &marker.next,
        &marker_digest,
        maximum,
    )?;
    Ok(RetainedRecoveryResult {
        result: CrashReplaceRecovery {
            action: CrashReplaceRecoveryAction::CommittedInitial,
            target_digest: Some(marker.next),
        },
        target: Some(installed),
    })
}

fn validate_common(
    target: &RetainedCrashReplaceTarget<'_>,
    maximum: u64,
) -> Result<(), CrashReplaceError> {
    target
        .validate()
        .map_err(|error| io_error(&target.target_path, error))?;
    if maximum == 0 {
        return Err(CrashReplaceError::InvalidArgument {
            field: "maximum_bytes",
            reason: "must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn names(target: &RetainedCrashReplaceTarget<'_>) -> Result<Names, CrashReplaceError> {
    if target
        .target_name
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CrashReplaceError::InvalidPath {
            field: "target",
            path: target.target_path.display().to_string(),
        });
    }
    let file = target
        .target_name
        .to_str()
        .ok_or_else(|| CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target must have a UTF-8 file name".to_owned(),
        })?;
    if ["forge-next", "forge-previous", "forge-transaction"]
        .iter()
        .any(|suffix| file.ends_with(suffix))
    {
        return Err(CrashReplaceError::InvalidArgument {
            field: "target_relative_path",
            reason: "target name collides with reserved protocol suffix".to_owned(),
        });
    }
    Ok(Names {
        target: target.target_name.clone(),
        next: PathBuf::from(format!(".{file}.forge-next")),
        previous: PathBuf::from(format!(".{file}.forge-previous")),
        transaction: PathBuf::from(format!(".{file}.forge-transaction")),
    })
}

fn encode_marker(marker: &Marker) -> Vec<u8> {
    format!(
        "{PROTOCOL_VERSION}\nprevious={}\nnext={}\n",
        marker.previous.as_deref().unwrap_or("absent"),
        marker.next
    )
    .into_bytes()
}

fn parse_marker(bytes: &[u8]) -> Result<Marker, CrashReplaceError> {
    let text = std::str::from_utf8(bytes).map_err(|_| protocol_error("marker is not UTF-8"))?;
    let lines = text.lines().collect::<Vec<_>>();
    if !text.ends_with('\n') || lines.len() != 3 || lines[0] != PROTOCOL_VERSION {
        return protocol("transaction marker has unsupported shape or version");
    }
    let previous =
        lines[1]
            .strip_prefix("previous=")
            .ok_or_else(|| CrashReplaceError::Protocol {
                reason: "transaction marker has no previous digest".to_owned(),
            })?;
    let next = lines[2]
        .strip_prefix("next=")
        .ok_or_else(|| CrashReplaceError::Protocol {
            reason: "transaction marker has no next digest".to_owned(),
        })?;
    validate_digest(next)?;
    let previous = if previous == "absent" {
        None
    } else {
        validate_digest(previous)?;
        Some(previous.to_owned())
    };
    Ok(Marker {
        previous,
        next: next.to_owned(),
    })
}

fn validate_optional_digest(value: Option<&str>) -> Result<(), CrashReplaceError> {
    if let Some(value) = value {
        validate_digest(value)?;
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), CrashReplaceError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(protocol_error("digest has no sha256 prefix"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(protocol_error("digest is not lowercase sha256 hex"));
    }
    Ok(())
}

fn read(
    target: &RetainedCrashReplaceTarget<'_>,
    name: &Path,
    maximum: u64,
) -> Result<Option<Vec<u8>>, CrashReplaceError> {
    target
        .read_file_bounded(name, maximum)
        .map_err(|error| io_error(&target.target_path.with_file_name(name), error))
}

fn digest(
    target: &RetainedCrashReplaceTarget<'_>,
    name: &Path,
    maximum: u64,
) -> Result<Option<String>, CrashReplaceError> {
    Ok(read(target, name, maximum)?.map(|bytes| sha256_content_hash(&bytes)))
}

fn write(
    target: &RetainedCrashReplaceTarget<'_>,
    name: &Path,
    bytes: &[u8],
) -> Result<(), CrashReplaceError> {
    target
        .write_new_file_synced(name, bytes)
        .map_err(|error| io_error(&target.target_path.with_file_name(name), error))
}

fn rename(
    target: &RetainedCrashReplaceTarget<'_>,
    from: &Path,
    to: &Path,
    expected_digest: &str,
    expected_identity: Option<&RetainedFileIdentity>,
    maximum: u64,
) -> Result<RetainedDigest, CrashReplaceError> {
    target
        .rename_file_if_digest(from, to, expected_digest, expected_identity, maximum)
        .map_err(|error| io_error(&target.target_path.with_file_name(from), error))
}

fn ensure_optional_digest(
    label: &str,
    found: Option<&str>,
    expected: &str,
) -> Result<(), CrashReplaceError> {
    if found.is_some_and(|digest| digest != expected) {
        protocol(&format!("{label} digest does not match transaction marker"))
    } else {
        Ok(())
    }
}

fn ensure_digest(
    label: &str,
    found: Option<&str>,
    expected: &str,
) -> Result<(), CrashReplaceError> {
    if found == Some(expected) {
        Ok(())
    } else {
        protocol(&format!("{label} digest does not match transaction marker"))
    }
}

fn inject_fault(
    fault_after: Option<CrashReplacePhase>,
    phase: CrashReplacePhase,
) -> Result<(), CrashReplaceError> {
    if fault_after == Some(phase) {
        Err(CrashReplaceError::InjectedFault { phase })
    } else {
        Ok(())
    }
}

fn protocol<T>(reason: &str) -> Result<T, CrashReplaceError> {
    Err(protocol_error(reason))
}

fn protocol_error(reason: &str) -> CrashReplaceError {
    CrashReplaceError::Protocol {
        reason: reason.to_owned(),
    }
}

fn io_error(path: &Path, source: io::Error) -> CrashReplaceError {
    CrashReplaceError::Io {
        path: path.to_path_buf(),
        source: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquire_effect_store_lock;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const LOCK: &str = "locks/domain-packs.lifecycle.lock";
    const TARGET: &str = "packs/active.lock.yaml";
    const MAX_BYTES: u64 = 64 * 1024;

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-retained-crash-unit-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn target_path(root: &Path) -> PathBuf {
        root.join(TARGET)
    }

    fn sidecars(root: &Path) -> [PathBuf; 3] {
        let parent = target_path(root)
            .parent()
            .expect("target parent")
            .to_path_buf();
        [
            parent.join(".active.lock.yaml.forge-next"),
            parent.join(".active.lock.yaml.forge-previous"),
            parent.join(".active.lock.yaml.forge-transaction"),
        ]
    }

    fn assert_no_sidecars(root: &Path) {
        for sidecar in sidecars(root) {
            assert!(
                fs::symlink_metadata(&sidecar).is_err(),
                "protocol sidecar must be cleaned: {}",
                sidecar.display()
            );
        }
    }

    fn install_for_test(lock: &EffectStoreLock, content: &[u8]) {
        replace_file_crash_safe_under_retained_lock_with_fault(
            lock,
            Path::new(TARGET),
            None,
            content,
            MAX_BYTES,
            None,
        )
        .expect("install initial target");
    }

    #[test]
    fn every_replacement_phase_recovers_exact_old_or_new_bytes() {
        let old = b"revision: 1\n";
        let new = b"revision: 2\n";
        for phase in [
            CrashReplacePhase::NextSynced,
            CrashReplacePhase::TransactionSynced,
            CrashReplacePhase::PreviousInstalled,
            CrashReplacePhase::TargetInstalled,
        ] {
            let root = temp_root(&format!("phase-{phase:?}"));
            let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
            install_for_test(&lock, old);
            let old_digest = sha256_content_hash(old);

            let error = replace_file_crash_safe_under_retained_lock_with_fault(
                &lock,
                Path::new(TARGET),
                Some(&old_digest),
                new,
                MAX_BYTES,
                Some(phase),
            )
            .expect_err("fault must interrupt replacement");
            assert_eq!(error, CrashReplaceError::InjectedFault { phase });

            let recovery =
                recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                    .expect("recover interrupted replacement");
            let bytes = fs::read(target_path(&root)).expect("recovered active bytes");
            assert!(
                bytes == old || bytes == new,
                "phase {phase:?} recovered neither exact old nor exact new bytes"
            );
            if phase == CrashReplacePhase::TargetInstalled {
                assert_eq!(bytes, new, "installed target is the commit point");
                assert_eq!(
                    recovery.action,
                    CrashReplaceRecoveryAction::CleanedCommitted
                );
            } else {
                assert_eq!(bytes, old, "pre-commit failure must preserve old bytes");
            }
            assert_no_sidecars(&root);

            let second =
                recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                    .expect("recovery is idempotent");
            assert_eq!(second.action, CrashReplaceRecoveryAction::Noop);
            drop(lock);
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn initial_transaction_after_durable_marker_is_completed_by_recovery() {
        let root = temp_root("initial-recovery");
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        let content = b"revision: 1\n";
        replace_file_crash_safe_under_retained_lock_with_fault(
            &lock,
            Path::new(TARGET),
            None,
            content,
            MAX_BYTES,
            Some(CrashReplacePhase::TransactionSynced),
        )
        .expect_err("fault after initial marker");

        let recovery =
            recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                .expect("finish marker-bound initial transaction");
        assert_eq!(
            recovery.action,
            CrashReplaceRecoveryAction::CommittedInitial
        );
        assert_eq!(fs::read(target_path(&root)).expect("active bytes"), content);
        assert_no_sidecars(&root);
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_handles_previous_publication_placeholder_window() {
        let root = temp_root("previous-placeholder");
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        let old = b"revision: 1\n";
        let new = b"revision: 2\n";
        install_for_test(&lock, old);
        let old_digest = sha256_content_hash(old);
        replace_file_crash_safe_under_retained_lock_with_fault(
            &lock,
            Path::new(TARGET),
            Some(&old_digest),
            new,
            MAX_BYTES,
            Some(CrashReplacePhase::TransactionSynced),
        )
        .expect_err("leave durable transaction");

        let protocol = sidecars(&root);
        fs::rename(target_path(&root), &protocol[1]).expect("publish exact previous");
        fs::write(target_path(&root), b"").expect("leave publication placeholder");
        let recovery =
            recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                .expect("recover previous-publication placeholder");
        assert_eq!(
            recovery.action,
            CrashReplaceRecoveryAction::RestoredPrevious
        );
        assert_eq!(fs::read(target_path(&root)).expect("restored bytes"), old);
        assert_no_sidecars(&root);
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_handles_target_publication_placeholder_window() {
        let root = temp_root("target-placeholder");
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        let old = b"revision: 1\n";
        let new = b"revision: 2\n";
        install_for_test(&lock, old);
        let old_digest = sha256_content_hash(old);
        replace_file_crash_safe_under_retained_lock_with_fault(
            &lock,
            Path::new(TARGET),
            Some(&old_digest),
            new,
            MAX_BYTES,
            Some(CrashReplacePhase::TransactionSynced),
        )
        .expect_err("leave durable transaction");

        let protocol = sidecars(&root);
        fs::rename(target_path(&root), &protocol[1]).expect("publish exact previous");
        fs::rename(&protocol[0], target_path(&root)).expect("publish exact target");
        fs::write(&protocol[0], b"").expect("leave publication placeholder");
        let recovery =
            recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                .expect("recover target-publication placeholder");
        assert_eq!(
            recovery.action,
            CrashReplaceRecoveryAction::CleanedCommitted
        );
        assert_eq!(fs::read(target_path(&root)).expect("committed bytes"), new);
        assert_no_sidecars(&root);
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_mismatch_preserves_marker_and_isolates_authoritative_target() {
        let root = temp_root("recovery-mismatch");
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        let old = b"revision: 1\n";
        let new = b"revision: 2\n";
        install_for_test(&lock, old);
        let old_digest = sha256_content_hash(old);
        replace_file_crash_safe_under_retained_lock_with_fault(
            &lock,
            Path::new(TARGET),
            Some(&old_digest),
            new,
            MAX_BYTES,
            Some(CrashReplacePhase::TransactionSynced),
        )
        .expect_err("leave durable transaction");

        fs::remove_file(target_path(&root)).expect("replace authoritative target name");
        fs::write(target_path(&root), b"substitute\n").expect("write substitute target");
        let marker = sidecars(&root)[2].clone();
        let error =
            recover_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                .expect_err("mismatched target must fail closed");
        assert!(matches!(error, CrashReplaceError::Protocol { .. }));
        assert!(marker.is_file(), "transaction marker must be preserved");
        assert!(
            !target_path(&root).exists(),
            "mismatched authoritative target must be isolated"
        );
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn reconciliation_session_retains_marker_finalizations_exact_target() {
        let root = temp_root("session-marker-finalization");
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        let old = b"revision: 1\n";
        let new = b"revision: 2\n";
        install_for_test(&lock, old);
        let old_digest = sha256_content_hash(old);
        replace_file_crash_safe_under_retained_lock_with_fault(
            &lock,
            Path::new(TARGET),
            Some(&old_digest),
            new,
            MAX_BYTES,
            Some(CrashReplacePhase::TargetInstalled),
        )
        .expect_err("leave committed target and durable marker");

        let session =
            reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
                .expect("finalize marker into one-shot session");
        assert_eq!(
            session.recovery().action,
            CrashReplaceRecoveryAction::CleanedCommitted
        );
        assert_eq!(session.raw_bytes(), Some(&new[..]));
        let exact = session
            .read_exact()
            .expect("consume exact marker-finalization handle")
            .expect("committed target is present");
        assert_eq!(exact.raw_bytes(), new);
        assert_eq!(exact.digest(), sha256_content_hash(new));
        assert_no_sidecars(&root);
        drop(exact);
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
