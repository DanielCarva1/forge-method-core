//! Private stream transaction engine.  Descriptors are closed and every
//! state-dependent mutation remains inside this crate.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use forge_core_store::{
    acquire_effect_store_lock_under_boundary,
    producer_quiescence::{admit_effect_producer, EffectProducerGuard, HostQuiescenceGuard},
    EffectStoreLock, WalDurability,
};
use serde::de::DeserializeOwned;

use crate::{EventEnvelope, EventLogError};

#[derive(Clone, Copy)]
pub(crate) enum StreamId {
    Memory,
    Research,
    Governance,
}

impl StreamId {
    const fn log_path(self) -> &'static str {
        match self {
            Self::Memory => "memory/events.ndjson",
            Self::Research => "research/sources.ndjson",
            Self::Governance => "governance/conflicts.ndjson",
        }
    }

    const fn lock_path(self) -> &'static str {
        match self {
            Self::Memory => "locks/memory.log.lock",
            Self::Research => "locks/research.sources.lock",
            Self::Governance => "locks/governance.conflicts.lock",
        }
    }
}

/// One exact `EventLog` member captured while host quiescence and all three
/// designated stream locks remain held. Private fields prevent callers from
/// minting a successful capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuiescedEventLogMember {
    relative_path: &'static str,
    bytes: Vec<u8>,
}

impl QuiescedEventLogMember {
    #[must_use]
    pub const fn relative_path(&self) -> &'static str {
        self.relative_path
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Complete typed capture of the three closed `EventLog` streams. A missing
/// member means that stream has not been initialized; an empty byte vector is
/// an initialized empty stream and remains distinct from absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuiescedEventLogSnapshot {
    memory: Option<QuiescedEventLogMember>,
    research: Option<QuiescedEventLogMember>,
    governance: Option<QuiescedEventLogMember>,
}

impl QuiescedEventLogSnapshot {
    #[must_use]
    pub fn memory(&self) -> Option<&QuiescedEventLogMember> {
        self.memory.as_ref()
    }

    #[must_use]
    pub fn research(&self) -> Option<&QuiescedEventLogMember> {
        self.research.as_ref()
    }

    #[must_use]
    pub fn governance(&self) -> Option<&QuiescedEventLogMember> {
        self.governance.as_ref()
    }

    pub fn members(&self) -> impl Iterator<Item = &QuiescedEventLogMember> {
        [
            self.memory.as_ref(),
            self.research.as_ref(),
            self.governance.as_ref(),
        ]
        .into_iter()
        .flatten()
    }

    pub fn into_members(self) -> impl Iterator<Item = QuiescedEventLogMember> {
        [self.memory, self.research, self.governance]
            .into_iter()
            .flatten()
    }
}

/// Capture exact bytes for every initialized closed stream under one exact-root
/// host-quiescence capability. The function acquires all designated stream
/// locks in stable order, validates strict append framing and contiguous
/// sequence numbers, then repeats the whole enumeration before returning.
/// Any member that appears, disappears, changes, is substituted, is partial,
/// or is observed under a cross-root guard fails closed instead of yielding a
/// caller-mintable success capability.
///
/// # Errors
///
/// Returns a typed error when the retained root or quiescence authority is
/// invalid, a designated stream cannot be locked or read, or either capture
/// pass detects malformed, partial, replaced, or changing event-log state.
pub fn capture_quiesced_event_logs(
    root: impl AsRef<Path>,
    quiescence: &HostQuiescenceGuard,
) -> Result<QuiescedEventLogSnapshot, EventLogError> {
    let root = root.as_ref();
    let locks = SnapshotLocks::acquire(root, quiescence)?;
    let first = CapturedSnapshotPass::capture(root, &locks)?;
    let second = CapturedSnapshotPass::capture(root, &locks)?;
    first.ensure_matches(root, &second)?;
    Ok(first.into_snapshot())
}

/// Private trait used only by the three first-party folds.  It is intentionally
/// not exported: a downstream crate cannot mint a descriptor or select a log.
pub(crate) trait EventSourced {
    type Event: serde::Serialize + DeserializeOwned + Clone + EventEnvelope;
    type Projection: Default + Clone;
    type Diagnostic: Clone;

    fn apply(projection: &mut Self::Projection, event: &Self::Event);
    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic);
    fn sequence_of(projection: &Self::Projection) -> u64;
    fn advance_sequence(projection: &mut Self::Projection, sequence: u64);
    fn diagnostic_out_of_order_event_ignored(
        event_seq: u64,
        projection_seq: u64,
    ) -> Self::Diagnostic;
    fn diagnostic_torn_final_line_skipped(
        line_number: usize,
        source: &serde_json::Error,
    ) -> Self::Diagnostic;
}

struct SnapshotLocks {
    memory: EffectStoreLock,
    research: EffectStoreLock,
    governance: EffectStoreLock,
}

impl SnapshotLocks {
    fn acquire(root: &Path, quiescence: &HostQuiescenceGuard) -> Result<Self, EventLogError> {
        // This order is part of the facade: callers cannot select or reorder
        // stream locks, and all three remain held through both capture passes.
        let memory = snapshot_lock(root, StreamId::Memory, quiescence)?;
        let research = snapshot_lock(root, StreamId::Research, quiescence)?;
        let governance = snapshot_lock(root, StreamId::Governance, quiescence)?;
        Ok(Self {
            memory,
            research,
            governance,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedSnapshotMember {
    identity: SnapshotFileIdentity,
    member: QuiescedEventLogMember,
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedSnapshotPass {
    memory: Option<CapturedSnapshotMember>,
    research: Option<CapturedSnapshotMember>,
    governance: Option<CapturedSnapshotMember>,
}

impl CapturedSnapshotPass {
    fn capture(root: &Path, locks: &SnapshotLocks) -> Result<Self, EventLogError> {
        Ok(Self {
            memory: capture_snapshot_member::<crate::memory::MemoryDomain>(
                root,
                StreamId::Memory,
                &locks.memory,
            )?,
            research: capture_snapshot_member::<crate::research::ResearchDomain>(
                root,
                StreamId::Research,
                &locks.research,
            )?,
            governance: capture_snapshot_member::<crate::governance::GovernanceDomain>(
                root,
                StreamId::Governance,
                &locks.governance,
            )?,
        })
    }

    fn ensure_matches(&self, root: &Path, other: &Self) -> Result<(), EventLogError> {
        for (id, stable) in [
            (StreamId::Memory, self.memory == other.memory),
            (StreamId::Research, self.research == other.research),
            (StreamId::Governance, self.governance == other.governance),
        ] {
            if !stable {
                return Err(EventLogError::Read {
                    path: root.join(id.log_path()),
                    source: "event-log member changed during stable quiesced re-enumeration"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }

    fn into_snapshot(self) -> QuiescedEventLogSnapshot {
        QuiescedEventLogSnapshot {
            memory: self.memory.map(|captured| captured.member),
            research: self.research.map(|captured| captured.member),
            governance: self.governance.map(|captured| captured.member),
        }
    }
}

#[cfg(unix)]
type SnapshotFileIdentity = (u64, u64);
#[cfg(windows)]
type SnapshotFileIdentity = (u32, u64);
#[cfg(not(any(unix, windows)))]
type SnapshotFileIdentity = ();

/// One retained producer boundary, designated exclusive lock, and log handle.
/// The same retained handle replays and appends; no path is resolved after
/// `begin` succeeds.
pub(crate) struct StreamTxn {
    display_log_path: PathBuf,
    retained_root_path: PathBuf,
    root_identity: Option<(u64, u64)>,
    _producer: EffectProducerGuard,
    stream_lock: EffectStoreLock,
    log: File,
}

impl StreamTxn {
    pub(crate) fn begin<D: Clone>(root: &Path, id: StreamId) -> Result<Self, EventLogError<D>> {
        let producer =
            admit_effect_producer(root, false).map_err(|source| lock_error(root, id, source))?;
        Self::begin_under_effect_authority(root, id, producer)
    }

    /// Recovery-only read seam for a caller already holding host quiescence.
    /// It creates no log and returns data, not transaction authority. Acquiring
    /// the designated lock binds the opaque guard to this exact state root.
    #[allow(dead_code)]
    pub(crate) fn snapshot_under_quiescence<E: EventSourced>(
        root: &Path,
        id: StreamId,
        quiescence: &HostQuiescenceGuard,
    ) -> Result<E::Projection, EventLogError<E::Diagnostic>> {
        let stream_lock =
            acquire_effect_store_lock_under_boundary(quiescence, root, id.lock_path())
                .map_err(|source| lock_error(root, id, source))?;
        let path = root.join(id.log_path());
        let mut log = match stream_lock
            .retained_state_root()
            .open_read(Path::new(id.log_path()))
        {
            Ok(log) => log,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(E::Projection::default());
            }
            Err(source) => {
                return Err(EventLogError::Read {
                    path,
                    source: source.to_string(),
                });
            }
        };
        validate_leaf(&log).map_err(|source| EventLogError::Read {
            path: path.clone(),
            source,
        })?;
        let mut bytes = Vec::new();
        log.read_to_end(&mut bytes)
            .map_err(|source| EventLogError::Read {
                path: path.clone(),
                source: source.to_string(),
            })?;
        stream_lock
            .validate_retained_lock_file()
            .map_err(|source| EventLogError::Read {
                path: path.clone(),
                source: format!("stream lock replacement detected during snapshot: {source}"),
            })?;
        replay_bytes::<E>(&path, &bytes)
    }

    fn begin_under_effect_authority<D: Clone>(
        root: &Path,
        id: StreamId,
        producer: EffectProducerGuard,
    ) -> Result<Self, EventLogError<D>> {
        let stream_lock = acquire_effect_store_lock_under_boundary(&producer, root, id.lock_path())
            .map_err(|source| lock_error(root, id, source))?;

        #[cfg(test)]
        test_hooks::after_stream_lock_acquired(root);

        // Revalidate the exact lock identity through the lock capability itself.
        // `RetainedStateRoot` is intentionally scoped to the EventLog member and
        // cannot be widened into a generic handle for reopening the lock leaf.
        stream_lock
            .validate_retained_lock_file()
            .map_err(|source| lock_error(root, id, source))?;

        let log = stream_lock
            .retained_state_root()
            .open_read_write_create(Path::new(id.log_path()))
            .map_err(|source| append_error(root, id, source))?;
        validate_leaf(&log).map_err(|source| append_error(root, id, source))?;

        Ok(Self {
            display_log_path: root.join(id.log_path()),
            retained_root_path: stream_lock
                .retained_state_root()
                .display_path()
                .to_path_buf(),
            root_identity: path_identity(root),
            _producer: producer,
            stream_lock,
            log,
        })
    }

    pub(crate) fn project<E: EventSourced>(
        &mut self,
    ) -> Result<E::Projection, EventLogError<E::Diagnostic>> {
        self.log
            .seek(SeekFrom::Start(0))
            .and_then(|_| {
                let mut bytes = Vec::new();
                self.log.read_to_end(&mut bytes)?;
                Ok(bytes)
            })
            .map_err(|source| EventLogError::Read {
                path: self.display_log_path.clone(),
                source: source.to_string(),
            })
            .and_then(|bytes| replay_bytes::<E>(&self.display_log_path, &bytes))
    }

    pub(crate) fn append<E: EventSourced>(
        &mut self,
        event: &E::Event,
        durability: WalDurability,
    ) -> Result<(), EventLogError<E::Diagnostic>> {
        self.reject_detected_replacement()?;
        let mut line = serde_json::to_vec(event).map_err(|source| EventLogError::Serialize {
            source: source.to_string(),
        })?;
        let _: serde_json::Value =
            serde_json::from_slice(&line).map_err(|source| EventLogError::Serialize {
                source: source.to_string(),
            })?;
        line.push(b'\n');
        self.log
            .seek(SeekFrom::End(0))
            .and_then(|_| self.log.write_all(&line))
            .and_then(|()| self.log.flush())
            .and_then(|()| match durability {
                WalDurability::SyncOnAppend => self.log.sync_data(),
                WalDurability::NoSync => Ok(()),
            })
            .map_err(|source| EventLogError::Append {
                path: self.display_log_path.clone(),
                source: source.to_string(),
            })
    }

    /// A detected replacement fails closed. This observation is deliberately
    /// not a hostile-race guarantee: a same-user namespace editor can still
    /// race any observation, which is outside the cooperative contract.
    fn reject_detected_replacement<D: Clone>(&self) -> Result<(), EventLogError<D>> {
        self.stream_lock
            .validate_retained_lock_file()
            .map_err(|source| EventLogError::Append {
                path: self.display_log_path.clone(),
                source: format!(
                    "stream lock replacement detected; outcome indeterminate: {source}"
                ),
            })?;
        if let Some(expected) = self.root_identity {
            if path_identity(&self.retained_root_path) != Some(expected) {
                return Err(EventLogError::Append {
                    path: self.display_log_path.clone(),
                    source: "state-root replacement detected; outcome indeterminate".to_owned(),
                });
            }
        }
        Ok(())
    }
}

fn snapshot_lock(
    root: &Path,
    id: StreamId,
    quiescence: &HostQuiescenceGuard,
) -> Result<EffectStoreLock, EventLogError> {
    acquire_effect_store_lock_under_boundary(quiescence, root, id.lock_path())
        .map_err(|source| lock_error(root, id, source))
}

fn capture_snapshot_member<E: EventSourced>(
    root: &Path,
    id: StreamId,
    stream_lock: &EffectStoreLock,
) -> Result<Option<CapturedSnapshotMember>, EventLogError> {
    stream_lock
        .validate_retained_lock_file()
        .map_err(|source| snapshot_read_error(root, id, source))?;
    let path = root.join(id.log_path());
    let mut file = match stream_lock
        .retained_state_root()
        .open_read(Path::new(id.log_path()))
    {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(snapshot_read_error(root, id, source)),
    };
    validate_leaf(&file).map_err(|source| snapshot_read_error(root, id, source))?;
    let identity =
        snapshot_file_identity(&file).map_err(|source| snapshot_read_error(root, id, source))?;
    let before_length = file
        .metadata()
        .map_err(|source| snapshot_read_error(root, id, source))?
        .len();
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_to_end(&mut bytes))
        .map_err(|source| snapshot_read_error(root, id, source))?;
    let after_length = file
        .metadata()
        .map_err(|source| snapshot_read_error(root, id, source))?
        .len();
    let after_identity =
        snapshot_file_identity(&file).map_err(|source| snapshot_read_error(root, id, source))?;
    let captured_length = u64::try_from(bytes.len()).map_err(|_| EventLogError::Read {
        path: path.clone(),
        source: "event-log member is too large to represent".to_owned(),
    })?;
    if before_length != after_length
        || after_length != captured_length
        || identity != after_identity
    {
        return Err(EventLogError::Read {
            path,
            source: "event-log member changed while its quiesced stream lock was held".to_owned(),
        });
    }
    stream_lock
        .validate_retained_lock_file()
        .map_err(|source| snapshot_read_error(root, id, source))?;
    validate_strict_snapshot_stream::<E>(&root.join(id.log_path()), &bytes)?;
    Ok(Some(CapturedSnapshotMember {
        identity,
        member: QuiescedEventLogMember {
            relative_path: id.log_path(),
            bytes,
        },
    }))
}

#[allow(clippy::naive_bytecount)]
fn validate_strict_snapshot_stream<E: EventSourced>(
    path: &Path,
    bytes: &[u8],
) -> Result<(), EventLogError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let line_number = |offset: usize| {
        bytes[..offset.min(bytes.len())]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1
    };
    if !bytes.ends_with(b"\n") {
        return Err(EventLogError::Parse {
            path: path.to_path_buf(),
            line_number: line_number(bytes.len()),
            source: "event-log snapshot ends without a complete newline-delimited record"
                .to_owned(),
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|source| EventLogError::Parse {
        path: path.to_path_buf(),
        line_number: line_number(source.valid_up_to()),
        source: format!("event-log snapshot is not UTF-8: {source}"),
    })?;
    let mut previous_sequence: Option<u64> = None;
    for (index, raw_line) in text.split_terminator('\n').enumerate() {
        let current_line = index + 1;
        if raw_line.trim().is_empty() {
            return Err(EventLogError::Parse {
                path: path.to_path_buf(),
                line_number: current_line,
                source: "event-log snapshot contains an empty record".to_owned(),
            });
        }
        let event: E::Event =
            serde_json::from_str(raw_line).map_err(|source| EventLogError::Parse {
                path: path.to_path_buf(),
                line_number: current_line,
                source: source.to_string(),
            })?;
        let expected = match previous_sequence {
            None => 1,
            Some(previous) => previous
                .checked_add(1)
                .ok_or(EventLogError::SequenceExhausted)?,
        };
        if event.sequence() != expected {
            return Err(EventLogError::Parse {
                path: path.to_path_buf(),
                line_number: current_line,
                source: format!(
                    "event-log snapshot sequence is {}, expected {expected}",
                    event.sequence()
                ),
            });
        }
        previous_sequence = Some(event.sequence());
    }
    Ok(())
}

fn snapshot_read_error(root: &Path, id: StreamId, source: impl std::fmt::Display) -> EventLogError {
    EventLogError::Read {
        path: root.join(id.log_path()),
        source: source.to_string(),
    }
}

#[cfg(unix)]
fn snapshot_file_identity(file: &File) -> std::io::Result<SnapshotFileIdentity> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn snapshot_file_identity(file: &File) -> std::io::Result<SnapshotFileIdentity> {
    let information = winapi_util::file::information(file)?;
    let volume = u32::try_from(information.volume_serial_number())
        .expect("Windows volume serial number is represented by a u32");
    Ok((volume, information.file_index()))
}

#[cfg(not(any(unix, windows)))]
fn snapshot_file_identity(_file: &File) -> std::io::Result<SnapshotFileIdentity> {
    Ok(())
}

pub(crate) fn next_sequence<E: EventSourced>(
    projection: &E::Projection,
) -> Result<u64, EventLogError<E::Diagnostic>> {
    E::sequence_of(projection)
        .checked_add(1)
        .ok_or(EventLogError::SequenceExhausted)
}

pub(crate) fn apply_event<E: EventSourced>(projection: &mut E::Projection, event: &E::Event) {
    let event_seq = event.sequence();
    let current = E::sequence_of(projection);
    if current > 0 && event_seq <= current {
        E::record_diagnostic(
            projection,
            E::diagnostic_out_of_order_event_ignored(event_seq, current),
        );
        return;
    }
    E::apply(projection, event);
    E::advance_sequence(projection, event_seq);
}

/// Pure replay is kept crate-private so tests and domain folds share the same
/// monotonicity behavior without exposing a caller-mintable stream authority.
pub(crate) fn replay<E: EventSourced>(events: impl IntoIterator<Item = E::Event>) -> E::Projection {
    let mut projection = E::Projection::default();
    for event in events {
        apply_event::<E>(&mut projection, &event);
    }
    projection
}

fn replay_bytes<E: EventSourced>(
    path: &Path,
    bytes: &[u8],
) -> Result<E::Projection, EventLogError<E::Diagnostic>> {
    let text = String::from_utf8_lossy(bytes);
    let total_lines = text.lines().count();
    let mut projection = E::Projection::default();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let event = match serde_json::from_str(line) {
            Ok(event) => event,
            Err(source) => {
                let final_line = index + 1 >= total_lines;
                let torn = final_line && (!line.starts_with('{') || !line.ends_with('}'));
                if torn {
                    E::record_diagnostic(
                        &mut projection,
                        E::diagnostic_torn_final_line_skipped(index + 1, &source),
                    );
                    break;
                }
                return Err(EventLogError::Parse {
                    path: path.to_path_buf(),
                    line_number: index + 1,
                    source: source.to_string(),
                });
            }
        };
        apply_event::<E>(&mut projection, &event);
    }
    Ok(projection)
}

/// Best-effort root identity used only to report a replacement that is already
/// observable. It is not used to promise hostile-race exclusion.
#[cfg(unix)]
fn path_identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|metadata| (metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn path_identity(_path: &Path) -> Option<(u64, u64)> {
    None
}
fn validate_leaf(file: &File) -> Result<(), String> {
    let metadata = file.metadata().map_err(|source| source.to_string())?;
    if !metadata.is_file() {
        return Err("event-log leaf must be a regular non-symlink file".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.nlink() != 1 {
            return Err("event-log leaf must have exactly one link".to_owned());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("event-log leaf must not be a reparse point".to_owned());
        }
        let information =
            winapi_util::file::information(file).map_err(|source| source.to_string())?;
        if information.number_of_links() != 1 {
            return Err("event-log leaf must have exactly one link".to_owned());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("event-log leaf validation is unsupported on this platform".to_owned());
    }
    Ok(())
}

fn lock_error<D: Clone>(
    root: &Path,
    id: StreamId,
    source: impl std::fmt::Display,
) -> EventLogError<D> {
    EventLogError::Lock {
        path: root.join(id.lock_path()),
        source: source.to_string(),
    }
}

fn append_error<D: Clone>(
    root: &Path,
    id: StreamId,
    source: impl std::fmt::Display,
) -> EventLogError<D> {
    EventLogError::Append {
        path: root.join(id.log_path()),
        source: source.to_string(),
    }
}

#[cfg(test)]
mod test_hooks {
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, OnceLock,
    };

    struct Hook {
        root: PathBuf,
        first_lock_acquired: Sender<()>,
        release_first: Mutex<Receiver<()>>,
        first: AtomicBool,
    }

    static HOOK: OnceLock<Mutex<Option<Arc<Hook>>>> = OnceLock::new();

    pub(crate) struct StreamLockRendezvous {
        pub(crate) first_lock_acquired: Receiver<()>,
        pub(crate) release_first: Sender<()>,
    }

    impl Drop for StreamLockRendezvous {
        fn drop(&mut self) {
            *HOOK
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("test hook mutex") = None;
        }
    }

    pub(crate) fn install(root: &Path) -> StreamLockRendezvous {
        let (first_lock_acquired_tx, first_lock_acquired) = mpsc::channel();
        let (release_first, release_first_rx) = mpsc::channel();
        let hook = Arc::new(Hook {
            root: root.to_path_buf(),
            first_lock_acquired: first_lock_acquired_tx,
            release_first: Mutex::new(release_first_rx),
            first: AtomicBool::new(false),
        });
        let mut slot = HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("test hook mutex");
        assert!(
            slot.is_none(),
            "only one stream-lock rendezvous may be active"
        );
        *slot = Some(hook);
        StreamLockRendezvous {
            first_lock_acquired,
            release_first,
        }
    }

    pub(crate) fn after_stream_lock_acquired(root: &Path) {
        let hook = HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("test hook mutex")
            .as_ref()
            .filter(|hook| hook.root == root)
            .cloned();
        let Some(hook) = hook else {
            return;
        };
        if hook
            .first
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            hook.first_lock_acquired
                .send(())
                .expect("test must await first stream lock");
            hook.release_first
                .lock()
                .expect("test hook release mutex")
                .recv()
                .expect("test must release first stream lock");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        admit_with_durability, AdmissionStatus, MemoryDomain, MemoryEvent, MEMORY_LOG_RELATIVE_PATH,
    };
    use forge_core_contracts::{
        ApprovalState, Freshness, MemoryEntry, MemoryKind, MemoryPolicy, MemoryProvenance, StableId,
    };
    use std::sync::{atomic::AtomicBool, mpsc};
    use std::time::Duration;

    fn root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-eventlog-tcb-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        std::fs::create_dir_all(&path).expect("create test root");
        path
    }

    fn entry(id: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: StableId(id.into()),
            kind: MemoryKind::Preference,
            content: "TCB regression".into(),
            provenance: MemoryProvenance {
                source_run_id: None,
                source_agent: None,
                evidence_ref: None,
                captured_at: "0".into(),
            },
            freshness: Freshness {
                ttl_seconds: None,
                last_confirmed_at: "0".into(),
                stale: false,
            },
            confidence: 1,
            approval: ApprovalState::Proposed,
            supersedes: None,
            invalidation_reason: None,
            authority_level: None,
            review_state: None,
            reviewed_by: None,
            reviewed_at: None,
        }
    }

    fn policy(permitted: Vec<MemoryKind>) -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: permitted,
            required_evidence_fields: Vec::new(),
            min_evidence_refs_for_authority: 1,
        }
    }

    #[test]
    fn concurrent_memory_sequences_are_contiguous_without_loss() {
        let root = root("concurrent-contiguous");
        let allowed = policy(vec![MemoryKind::Preference]);
        let rendezvous = test_hooks::install(&root);

        let first_root = root.clone();
        let first_policy = allowed.clone();
        let first = std::thread::spawn(move || {
            admit_with_durability(
                &first_root,
                entry("one"),
                &first_policy,
                WalDurability::NoSync,
            )
        });

        rendezvous
            .first_lock_acquired
            .recv_timeout(Duration::from_secs(4))
            .expect("first producer must hold the designated stream lock");

        let second_root = root.clone();
        let second_policy = allowed;
        let (second_observed_tx, second_observed) = mpsc::channel();
        let second = std::thread::spawn(move || {
            let blocked = forge_core_store::try_acquire_effect_store_lock(
                &second_root,
                StreamId::Memory.lock_path(),
            )
            .expect_err("real second try-lock must observe the held stream lock");
            assert!(matches!(
                blocked,
                forge_core_store::EffectStoreLockError::WouldBlock { .. }
            ));
            second_observed_tx
                .send(())
                .expect("main test must await second try-lock observation");
            admit_with_durability(
                &second_root,
                entry("two"),
                &second_policy,
                WalDurability::NoSync,
            )
        });

        second_observed
            .recv_timeout(Duration::from_secs(4))
            .expect("second producer must observe WouldBlock before first releases");
        rendezvous
            .release_first
            .send(())
            .expect("release first producer");

        let first_sequence = match first.join().expect("first producer").status {
            AdmissionStatus::Admitted { sequence } => sequence,
            status => panic!("first concurrent admission failed: {status:?}"),
        };
        let second_sequence = match second.join().expect("second producer").status {
            AdmissionStatus::Admitted { sequence } => sequence,
            status => panic!("second concurrent admission failed: {status:?}"),
        };
        assert_eq!([first_sequence, second_sequence], [1, 2]);

        let projection = crate::memory::project(&root).expect("project concurrent writes");
        assert_eq!(projection.sequence, 2);
        assert_eq!(projection.entries.len(), 2, "neither admission may be lost");
        assert!(projection.entries.contains_key("one"));
        assert!(projection.entries.contains_key("two"));

        let logged: Vec<(u64, String)> =
            std::fs::read_to_string(root.join(MEMORY_LOG_RELATIVE_PATH))
                .expect("read log")
                .lines()
                .map(
                    |line| match serde_json::from_str::<MemoryEvent>(line).expect("event") {
                        MemoryEvent::Admitted {
                            sequence, entry, ..
                        } => (sequence, entry.entry_id.0),
                        _ => unreachable!("admission only writes admitted events"),
                    },
                )
                .collect();
        assert_eq!(logged, vec![(1, "one".to_owned()), (2, "two".to_owned())]);
    }

    #[test]
    fn denied_gate_appends_no_event_or_sequence() {
        let root = root("gate-bypass");
        let denied = admit_with_durability(
            &root,
            entry("denied"),
            &policy(Vec::new()),
            WalDurability::NoSync,
        );
        assert!(matches!(denied.status, AdmissionStatus::DeniedByGate(_)));
        assert!(!root.join(MEMORY_LOG_RELATIVE_PATH).exists());

        let allowed = admit_with_durability(
            &root,
            entry("allowed"),
            &policy(vec![MemoryKind::Preference]),
            WalDurability::NoSync,
        );
        assert!(matches!(
            allowed.status,
            AdmissionStatus::Admitted { sequence: 1 }
        ));
    }

    #[test]
    fn quiesced_snapshot_is_read_only_and_does_not_create_a_missing_log() {
        let root = root("quiesced-snapshot");
        let cancellation = AtomicBool::new(false);
        let quiescence =
            forge_core_store::producer_quiescence::quiesce_host_producers(&root, &cancellation)
                .expect("quiesce producers");

        let projection = StreamTxn::snapshot_under_quiescence::<MemoryDomain>(
            &root,
            StreamId::Memory,
            &quiescence,
        )
        .expect("snapshot missing stream");
        assert!(projection.is_empty());
        assert!(!root.join(MEMORY_LOG_RELATIVE_PATH).exists());
    }

    #[test]
    fn typed_quiesced_capture_returns_exact_initialized_members() {
        let root = root("typed-quiesced-capture");
        let admitted = admit_with_durability(
            &root,
            entry("captured"),
            &policy(vec![MemoryKind::Preference]),
            WalDurability::NoSync,
        );
        assert!(admitted.is_admitted());
        let expected = std::fs::read(root.join(MEMORY_LOG_RELATIVE_PATH)).expect("read memory log");
        let cancellation = AtomicBool::new(false);
        let quiescence =
            forge_core_store::producer_quiescence::quiesce_host_producers(&root, &cancellation)
                .expect("quiesce producers");

        let snapshot = capture_quiesced_event_logs(&root, &quiescence).expect("capture event logs");
        assert_eq!(
            snapshot.memory().expect("memory member").bytes(),
            expected.as_slice()
        );
        assert!(snapshot.research().is_none());
        assert!(snapshot.governance().is_none());
        assert_eq!(snapshot.members().count(), 1);
    }

    #[test]
    fn typed_quiesced_capture_rejects_cross_root_guard() {
        let guarded_root = root("typed-quiesced-guarded");
        let other_root = root("typed-quiesced-other");
        let cancellation = AtomicBool::new(false);
        let quiescence = forge_core_store::producer_quiescence::quiesce_host_producers(
            &guarded_root,
            &cancellation,
        )
        .expect("quiesce guarded root");

        let result = capture_quiesced_event_logs(&other_root, &quiescence);
        assert!(matches!(result, Err(EventLogError::Lock { .. })));
    }

    #[test]
    fn typed_quiesced_capture_rejects_partial_final_record() {
        let root = root("typed-quiesced-partial");
        std::fs::create_dir_all(root.join("memory")).expect("create memory directory");
        let event = MemoryEvent::Admitted {
            sequence: 1,
            at_unix: 1,
            entry: entry("partial"),
        };
        std::fs::write(
            root.join(MEMORY_LOG_RELATIVE_PATH),
            serde_json::to_vec(&event).expect("serialize event"),
        )
        .expect("seed partial log");
        let cancellation = AtomicBool::new(false);
        let quiescence =
            forge_core_store::producer_quiescence::quiesce_host_producers(&root, &cancellation)
                .expect("quiesce producers");

        let result = capture_quiesced_event_logs(&root, &quiescence);
        assert!(matches!(result, Err(EventLogError::Parse { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn preexisting_hard_linked_log_is_rejected_without_touching_outside_file() {
        let root = root("hard-link");
        let outside = root.with_extension("outside");
        std::fs::write(&outside, b"outside bytes").expect("seed outside file");
        std::fs::create_dir_all(root.join("memory")).expect("create memory directory");
        std::fs::hard_link(&outside, root.join(MEMORY_LOG_RELATIVE_PATH)).expect("hard link log");

        let result = admit_with_durability(
            &root,
            entry("blocked"),
            &policy(vec![MemoryKind::Preference]),
            WalDurability::NoSync,
        );
        assert!(matches!(result.status, AdmissionStatus::StoreError(_)));
        assert_eq!(
            std::fs::read(&outside).expect("read outside file"),
            b"outside bytes"
        );
    }
}
